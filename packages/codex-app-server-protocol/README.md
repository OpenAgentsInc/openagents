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

Generation also emits compact runtime JSON Schema documents from the reviewed
Effect schemas. Import the small `./decode` API at application boundaries;
application packages should not import the multi-megabyte generated schema
modules directly. The decoder covers every response, reverse request, and
notification independently for each lane and returns a classified failure for
unknown or malformed payloads instead of throwing.

`fixtures/current-source-notifications.json` covers all 72 current
notifications, and `fixtures/current-source-thread-items.json` covers all 18
`ThreadItem` variants. `pnpm fixtures:generate` refreshes those corpora from
the generated wire documents. `check:generated` verifies that every reviewed
method has a corresponding runtime decoder.
