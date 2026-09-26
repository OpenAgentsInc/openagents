# Coder iOS simulator verification

The existing `com.openagents.coder` app, version `0.5.0 (38)`, built and ran
with the public Rust reader and thin SwiftUI adapter. Four native UI tests
passed on the iPhone 17 Pro simulator with iOS 26.5, using Xcode 26.6 and Rust
1.97.1. The final UI test execution took 49.004 seconds.

The fixture contains 32 synthetic records across two cached pages. It uses a
separate Keychain identity and cache directory. It opens no real conversation,
connects to no reader host, and makes no model or benchmark call.

## Evidence

- [Receipt](receipt.json): commands, tool versions, scope, source hashes, and
  artifact digests.
- [Full build and UI test log](simulator-tests.log).
- [XCTest attachment metadata](attachments.json).
- [Readable transcript screenshot](synthetic-timeline.png).
- [Exact-source screenshot](synthetic-source.png).

The complete `.xcresult` bundle remains at the local path recorded in the
receipt. The checked-in screenshots contain only the synthetic fixture.

## Checks

| Check | Result |
| --- | --- |
| Native saved-chat button opens the Rust-projected transcript | Passed |
| Exact source bytes can replace the readable record | Passed |
| Follow toggle pauses across an explicit refresh and resumes | Passed |
| Earlier and later page selections stay pinned until **Latest** is selected | Passed |
| Device public key survives app termination and relaunch | Passed |
| Read-only label is present, with no message field or send action | Passed |
| Generic decoder preserves Unicode, source Markdown, row keys, disabled buttons, and RGBA | Passed |
| Decoder refuses an unsupported native element | Passed |
| Native source typechecks against the real iOS Simulator SDK | Passed |

The native UI test code is in
[ReaderUITests.swift](../../host/UITests/ReaderUITests.swift). Run it with
`scripts/build-coder-mobile.sh sim-test` against an already booted simulator.
The Swift decoder check is separate from the UI tests; it does not mount an
app. Rust state and transport tests have their own results and are not counted
as native UI tests here.

An earlier UI run exposed a real accessibility-identifier problem: assigning
an identifier to a container replaced identifiers on its descendants. The
adapter now assigns identifiers to leaf controls and text. The final tests
verify those controls in the actual app. An earlier paging assertion also
incorrectly expected **Later** to resume following; the test now requires
**Latest**, matching the Rust page-pinning contract.

## Boundaries

This is simulator evidence, not physical-device or distribution acceptance.
It does not establish locked-device Keychain behavior, VoiceOver completion,
every keyboard/input method, real phone-to-host pairing, or TestFlight
processing. The inline Markdown adapter preserves original source but is not
a complete CommonMark block renderer. Archive and upload receipts must
separately identify their committed source and distribution results.

## TestFlight delivery

[Build 38's distribution receipt](testflight-build38.json) records the clean
source commit, archive/package hashes, and App Store Connect result. Coder
`0.5.0 (38)` is `VALID` and `IN_BETA_TESTING` for the existing internal group.
This is internal TestFlight delivery; physical-device acceptance and external
beta/App Store review are separate.
