# Coder for iOS

This is the existing Coder app identity with a new public read-only chat
reader. Rust owns pairing, protocol verification, synchronization, encrypted
cache, and application projection. The thin SwiftUI host renders Rust Native
views, keeps a device identity in Keychain, and forwards bounded UI events.
It does not call models, execute tasks, submit chat messages, or grant approvals.

**Available in internal TestFlight:** Coder `0.5.0 (38)`, built from
[`2f84cf2627`](https://github.com/OpenAgentsInc/openagents/commit/2f84cf2627777f40639411f079c26a048039f792).
[Connect your phone](../../docs/coder/guides/mobile-readonly.md) and review the
[distribution receipt](verification/2026-09-26/testflight-build38.json).

## App identity and source boundary

The user-authorized app metadata and icon preserve the existing Xcode setup:

| Setting | Value |
| --- | --- |
| Project, scheme, target, product | `Coder` |
| Bundle identifier | `com.openagents.coder` |
| Development team | `HQWSG26L43` |
| Marketing version and build | `0.5.0` / `38` |
| Minimum OS and device family | iOS 17 / iPhone |
| Swift language setting | `5.10` |
| App Store Connect app | `6807250813` |
| Archive signing | Manual, Apple Distribution, `OpenAgents Coder App Store` profile |

The native reader and Rust interface are newly implemented in this public
repository. The old private renderer, authentication flow, service endpoints,
and backend are not imported. The existing `coder` URL scheme stays registered
for app compatibility; this read-only shell does not process old authentication
callbacks. The retained `ExportOptions.plist` records distribution metadata;
the build helper never uploads the app.

Build `38` follows the existing App Store Connect build `0.5.0 (37)`. The app
identity, marketing version, signing team, and distribution profile stay the
same; the build number advances for this replacement implementation.

## Build and install

Prerequisites: the pinned Rust toolchain, `aarch64-apple-ios` and
`aarch64-apple-ios-sim` targets, Xcode with the iOS SDK, and `xcodegen`.
The helper builds `coder-mobile` as a static library, generates the existing
`Coder.xcodeproj`, and invokes `xcodebuild`. Build products stay outside the
checkout in the worktree's target directory. Compiler environments exclude
provider and service credentials.

```sh
# Build only; do not install or launch.
scripts/build-coder-mobile.sh sim-build

# Install and launch a synthetic reader on an already booted simulator.
scripts/build-coder-mobile.sh sim --synthetic

# Run native UI checks against the separate synthetic fixture and identity.
scripts/build-coder-mobile.sh sim-test

# Build for an existing paired device, with development signing.
scripts/build-coder-mobile.sh device
```

`bins/coder-ios/build.sh` is the equivalent app-local entry point.
`CARGO_TARGET_DIR` selects the per-worktree Rust output directory;
`CODER_IOS_OUTPUT` selects Xcode's output directory. For the simulator,
`CODER_IOS_DEVICE` defaults to `booted` and can name an explicit simulator
UDID. For a development device, `CODER_IOS_DEV_PROFILE` selects an existing
development profile; otherwise Xcode's automatic development signing is used.
Device builds do not install automatically. You can open the generated project
in Xcode, select the device, and use development signing to install it. An App
Store distribution archive cannot be installed directly as a development app.

For an authorized release, `archive` keeps the existing manual distribution
settings and writes `Coder.xcarchive`. Device and archive commands use Rust's
optimized release profile; simulator checks use the development profile.
Commit the release source before archiving. The helper records the commit,
tracked-diff digest, workspace status, Cargo lock digest, compiler versions,
and archived executable digest beside the archive. A nonempty workspace status
must be reviewed rather than described as an exact committed-source build.
Set `CODER_IOS_BUILD_NUMBER` to a new
positive build number after checking the existing App Store Connect builds;
the checked-in build is `38`. The `export` command produces a local
distribution package using the existing export settings with destination
changed to `export`. Upload is a separate operator action using the retained
upload configuration and protected App Store Connect credentials. Never put
API key material in a command argument, log, or repository file.

`sim-test` uses the same booted simulator selection, always launches with
`--synthetic`, and retains an `.xcresult` bundle under `CODER_IOS_OUTPUT`.
The UI tests exercise a real linked app, not a second Swift implementation of
the Rust feature state. The standalone decoder checks in
`host/Tests/NativeContractChecks.swift` test the generic view mapping without
starting an iOS app.

The simulator install uses the existing bundle identifier and can update an
existing simulator copy of Coder. It does not uninstall the app or erase other
app data. Synthetic mode uses a separate Keychain account and cache directory;
it neither reads real conversations nor contacts a host. Normal mode opens the
cached view before it requests a refresh.

## Connect and read

1. Open **Connect** and share the displayed public key with the computer's
   reader host. The private key never leaves this device's Keychain.
2. Paste the connection code generated by that host and select **Connect to
   computer**. Rust verifies the code and controls the read-only protocol.
3. Select a saved chat. Use the Rust-provided earlier-page and full-text
   controls to reach original content; a partial page must remain labeled.
4. Keep **Follow new messages** enabled to move to the latest visible record.
   Dragging the transcript pauses following and tells Rust to pin the exact
   cached page that was visible. Enable the toggle to resume. The opaque page
   identity is captured when the interaction occurs, so a refresh that is
   already in flight cannot silently select a different page to pin.

The native shell requests refreshes every five seconds while active and
coalesces them while a Rust call is in progress. Backgrounding stops that
timer's refresh requests. Rust owns freshness and connection status; cached
content must never be presented as a live acknowledgment.

Inline Markdown uses native selectable attributed text and preserves source
whitespace, with **Show original Markdown** as an exact-source fallback.
This first adapter does not implement a complete CommonMark block renderer;
headings, tables, and fenced blocks retain their source syntax. Links are inert
in the reader. Tool
expansion and full-text paging are Rust-projected controls, not a second set
of Swift domain operations. A UI activation carries only view instance,
revision, and node identity; Rust resolves the current intent.

**Disconnect and erase cached chats** asks Rust to remove the local connection
and cached conversations. It leaves the computer's files and this device's
Keychain identity unchanged. The reader directory uses complete file
protection and is excluded from backups. Keychain access uses
`WhenUnlockedThisDeviceOnly`; locked-device behavior still needs separate
physical-device acceptance.

The privacy manifest declares app-container file metadata (`C617.1`) for the
encrypted cache and elapsed-time APIs (`35F9.1`) for in-app timers and network
deadlines. These are the corresponding
[Apple required-reason API categories](https://developer.apple.com/documentation/bundleresources/app-privacy-configuration/nsprivacyaccessedapitypes/nsprivacyaccessedapitype).
The manifest is not an App Store privacy-label review or a declaration that
relay transport has no visible metadata.

## Verification boundaries

The [2026-09-26 simulator receipt](verification/2026-09-26/README.md) records
four passing native UI tests, full logs, source hashes, and synthetic
screenshots for build `38`.

The implementation must retain code/build results separately from simulator
and physical-device behavior. Native controls do not automatically establish
VoiceOver, all keyboard/input methods, very large record navigation, or every
supported iOS version. No real transcript publication, model call, benchmark
run, or physical-device release result is implied by a synthetic smoke check.

See [issue #9696](https://github.com/OpenAgentsInc/openagents/issues/9696), the
[Rust Native contract](../../docs/coder/rust-native/architecture.md), and the
[suite tracker](../../docs/coder/migration-status.md) for scope and acceptance.
