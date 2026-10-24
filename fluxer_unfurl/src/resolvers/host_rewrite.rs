// SPDX-License-Identifier: AGPL-3.0-or-later

use super::default_resolver;
use super::{ResolveContext, Resolver, ResolverResult};
use crate::network_policy;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use url::Url;

const REWRITES_ENV: &str = "FLUXER_UNFURL_HOST_REWRITES";
const DEFAULT_REWRITES: &str = "tiktok.com=tnktok.com,instagram.com=oginstagram.com,facebook.com=facebed.com";
// Embed fixers only serve embeds to crawlers they recognise and redirect everyone else.
const USER_AGENT: &str = "Mozilla/5.0 (compatible; Fluxerbot/1.0; +https://fluxer.app) Discordbot/2.0";
const DROPPED_SUBDOMAINS: &[&str] = &["www", "m", "mobile", "web"];

pub struct HostRewriteResolver;

impl Resolver for HostRewriteResolver {
    fn matches(&self, url: &Url) -> bool {
        rewrite_url(url).is_some()
    }

    fn resolve<'a>(
        &'a self,
        ctx: &'a ResolveContext<'_>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<ResolverResult>> + Send + 'a>> {
        Box::pin(async move {
            let Some(rewritten) = rewrite_url(&ctx.original_url) else {
                return Ok(ResolverResult { embeds: vec![] });
            };
            let rewritten_ctx = ResolveContext {
                url: rewritten,
                original_url: ctx.original_url.clone(),
                http_client: http_client().clone(),
                nsfw_mode: ctx.nsfw_mode,
                media_proxy: ctx.media_proxy,
                self_hosted: ctx.self_hosted,
                youtube_api_key: ctx.youtube_api_key.clone(),
                klipy_api_key: ctx.klipy_api_key.clone(),
            };
            let mut result = default_resolver::resolve_html(&rewritten_ctx).await?;
            let Some(fixer_url) = result.embeds.first().and_then(|embed| embed.url.clone()) else {
                return Ok(result);
            };
            for embed in &mut result.embeds {
                if embed.url.as_deref() == Some(fixer_url.as_str()) {
                    embed.url = Some(ctx.original_url.to_string());
                }
            }
            Ok(result)
        })
    }
}

fn rewrites() -> &'static [(String, String)] {
    static REWRITES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    REWRITES.get_or_init(|| {
        let raw = std::env::var(REWRITES_ENV).unwrap_or_else(|_| DEFAULT_REWRITES.to_owned());
        parse_rewrites(&raw)
    })
}

fn parse_rewrites(raw: &str) -> Vec<(String, String)> {
    raw.split(',')
        .filter_map(|rule| {
            let (from, to) = rule.split_once('=')?;
            let (from, to) = (from.trim(), to.trim());
            if from.is_empty() || to.is_empty() {
                return None;
            }
            Some((from.to_ascii_lowercase(), to.to_ascii_lowercase()))
        })
        .collect()
}

fn rewrite_url(url: &Url) -> Option<Url> {
    rewrite_url_with(url, rewrites())
}

fn rewrite_url_with(url: &Url, rules: &[(String, String)]) -> Option<Url> {
    let host = url.host_str()?.to_ascii_lowercase();
    let (prefix, target) = rules.iter().find_map(|(from, to)| {
        if host == *from {
            return Some(("", to));
        }
        let prefix = host.strip_suffix(from.as_str())?.strip_suffix('.')?;
        Some((prefix, to))
    })?;
    let new_host = if prefix.is_empty() || DROPPED_SUBDOMAINS.contains(&prefix) {
        target.clone()
    } else {
        format!("{prefix}.{target}")
    };
    let mut rewritten = url.clone();
    rewritten.set_host(Some(&new_host)).ok()?;
    Some(rewritten)
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .dns_resolver(Arc::new(network_policy::PinnedDnsResolver))
            .build()
            .expect("failed to build host rewrite HTTP client")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite(url: &str) -> Option<String> {
        let rules = parse_rewrites(DEFAULT_REWRITES);
        rewrite_url_with(&Url::parse(url).unwrap(), &rules).map(|url| url.to_string())
    }

    #[test]
    fn rewrites_known_hosts_and_keeps_meaningful_subdomains() {
        assert_eq!(
            rewrite("https://www.instagram.com/reel/C6bfaFzIqqR/?igsh=abc").as_deref(),
            Some("https://oginstagram.com/reel/C6bfaFzIqqR/?igsh=abc")
        );
        assert_eq!(
            rewrite("https://vm.tiktok.com/ZMhvqSbGs/").as_deref(),
            Some("https://vm.tnktok.com/ZMhvqSbGs/")
        );
        assert_eq!(
            rewrite("https://m.facebook.com/reel/578721235067082").as_deref(),
            Some("https://facebed.com/reel/578721235067082")
        );
    }

    #[test]
    fn ignores_other_hosts() {
        assert_eq!(rewrite("https://notinstagram.com/p/x"), None);
        assert_eq!(rewrite("https://example.com/"), None);
    }

    #[test]
    fn empty_config_disables_rewrites() {
        assert!(parse_rewrites("").is_empty());
    }
}
