# Codex app-server protocol

This package is the version-bound protocol authority for OpenAgents' Codex
app-server client. It generates Effect schemas and method maps from two exact
upstream identities:

- current source commit `1bbdb32789e1f79932df44941236ea3658f6e965`;
- Desktop's bundled Codex `0.144.1` source commit
  `44918ea10c0f99151c6710411b4322c2f5c96bea`.

The two manifests are intentionally independent. Never use the current-source
denominator to claim support for the shipped Desktop executable.

Run `pnpm generate` to refresh generated artifacts from the pinned commits and
`pnpm check:generated` to fail on drift. The generator follows the
programmatic JSON Schema → Effect Schema architecture studied in T3 Code's
`effect-codex-app-server` package. Portions of the generator are adapted from
that MIT-licensed implementation; see `THIRD_PARTY_NOTICES.md`.
