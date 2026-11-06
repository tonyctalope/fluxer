# fluxer-unfurl-fx

Personal fork of [fluxerapp/fluxer](https://github.com/fluxerapp/fluxer) carrying a few
`fluxer_unfurl` patches for a self-hosted instance. Not intended for upstream.

## Patches

- x.com / twitter.com status links are resolved through the FxTwitter API, like fxtwitter.com links already are.
- tiktok.com, instagram.com and facebook.com links are fetched from embed fixers
  (tnktok.com, oginstagram.com, facebed.com). The original link stays the embed URL; if the fixer
  fails, the normal unfurl runs. Override the list with
  `FLUXER_UNFURL_HOST_REWRITES=tiktok.com=tnktok.com,instagram.com=vxinstagram.com,...`
  (empty value disables it).

## Images

`.github/workflows/unfurl-fx.yaml` checks every hour which upstream commit
`ghcr.io/fluxerapp/fluxer-unfurl:v1` was built from, applies this branch's commits on top of it,
runs the unfurl tests and pushes a multi-arch image (linux/amd64 and linux/arm64):

- `ghcr.io/tonyctalope/fluxer-unfurl:v1-fx`: follows upstream `v1`
- `ghcr.io/tonyctalope/fluxer-unfurl:fx-<upstream sha12>`: a specific upstream revision
  (build one with *Run workflow* and the `revision` input)

Use it in the self-hosted compose stack by overriding the `unfurl` and `unfurl-shard` images.
If upstream changes conflict with the patches, the run fails: rebase `unfurl-fx` onto upstream `main`.
