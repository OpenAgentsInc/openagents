# OpenAgents mobile — v0.5.2 build 116: persona-neutral Khala correction

- Date: 2026-07-10
- Source: current `main` mobile correction scope under issue #8597
- App identity: `OpenAgents` / `com.openagents.app` / Team `HQWSG26L43`
- App Store Connect build: `116`
- Distribution state: signed upload accepted; App Store Connect build record
  ingestion pending

## Outcome

Build 116 removes the named-persona mobile path completely: relationship and
prospect state, local session/thread persistence, SSE adapter, tests, demo
videos, and presentation-only purchase sheet. This follows Sol roadmap rev-24:
those compatibility routes are not the product front door or acceptance path.

The visible Khala surface has exactly one composer. `GlassComposer` is the
native SwiftUI Liquid Glass input: it owns text entry, draft-change events, and
submit events, which enter the typed Khala intent registry. The duplicate
Effect Native composer inside the transcript is removed.

## Verification before signing

- `bun run --cwd apps/openagents-mobile typecheck` — passed.
- `bun run --cwd apps/openagents-mobile test` — passed: 20 tests / 69
  expectations.
- `expo prebuild --platform ios --clean` produced `CFBundleVersion=116`.
- A clean unsigned Release archive completed, including compilation of the
  changed SwiftUI module.

## Distribution proof

- The signed archive and manual App Store export completed successfully.
- App Store delivery accepted build number 116. A retry was correctly rejected
  as a duplicate because 116 is already reserved by the accepted delivery.
- At this receipt time, the App Store Connect builds API has not yet exposed
  the build ID or processing state; do not label it `VALID` until it does.

On device, verify a real Khala turn from the one native composer and confirm
the removed persona/demo/catalog surfaces do not return.
