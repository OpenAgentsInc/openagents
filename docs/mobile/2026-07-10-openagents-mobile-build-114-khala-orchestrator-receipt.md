# OpenAgents mobile — v0.5.2 build 114: Khala orchestrator mode

- Date: 2026-07-10
- Source commit: `23aba8270a8bf371d7193d2ae82bd1271bd287fb`
- App identity: `OpenAgents` / `com.openagents.app` / Team `HQWSG26L43`
- App Store Connect build: `114`
- ASC build id: `79fd0ef2-bbc1-4eac-9f66-87eac1371eda`
- Processing state: `VALID`

> Superseded for visual acceptance by build 115. Build 114 is a valid Khala
> transport build, but its native SwiftUI Liquid Glass module was absent and it
> therefore showed an opaque React Native fallback for the top-level chrome.

## Outcome

The existing Liquid Glass mode picker now contains **Khala**. Selecting it
opens a real typed transcript and composer backed by the public generic Khala
stream endpoint, `POST https://openagents.com/api/khala/chat`. Each turn sends
the running transcript to the server; the server owns the single
`openagents/khala` orchestration model and all backing-lane routing.

## Claims deliberately not made

- The client does not name or fabricate the backing model, Pylon, validator,
  receipt, FleetRun, account, spend, or settlement outcome.
- This is not Sarah: it does not mint or restore a prospect relationship and
  does not alter Sarah's persisted conversation catalog.
- The generic Khala endpoint is stateless. Mobile Khala transcript persistence,
  authenticated keys/credits, and cross-device Sync are future scoped work.

## Verification

- `bun run --cwd apps/openagents-mobile typecheck` — passed.
- `bun run --cwd apps/openagents-mobile test` — passed, 40 tests after the
  concurrent Effect Native chrome landing was rebased in.
- The new `openagents_mobile.khala_surface.v1` contract drives mode selection,
  a typed turn through a deterministic orchestration-client seam, transcript
  completion, and the rendered Khala composer.
- Local iOS CNG prebuild reported `CFBundleVersion=114`; the archive and manual
  App Store export succeeded. The exported IPA was validated and uploaded with
  the configured App Store Connect API key.
- App Store Connect API confirms build 114 as `VALID`.

## Distribution note

This was a local Xcode/ASC build (`expo prebuild --clean`, `xcodebuild archive`,
manual App Store profile export, `xcrun altool`), never EAS. A first owner
device interaction/pixel receipt remains separate from build validity.
