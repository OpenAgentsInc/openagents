# Coder for iOS

Coder opens into **Verse**. Walk up to the computer in the world to pair by
QR code and open the read-only **Chats** reader. These are public Rust-owned
surfaces under the existing Coder app identity. Rust owns
pairing, protocol verification, synchronization, encrypted reader cache, world
state, movement, rendering, and application projection. The thin SwiftUI host
renders Rust Native views, keeps separate device identities in Keychain, and
forwards bounded UI events. It does not call models, execute tasks, submit chat
messages, or grant approvals.

**Available in internal TestFlight:** Coder `0.5.0 (39)`, built from
[`d695c1dfc7`](https://github.com/OpenAgentsInc/openagents/commit/d695c1dfc708d2b8f592a4640ad0cee5b9844de5).
[Connect your phone](../../docs/coder/guides/mobile-readonly.md) and review the
[distribution receipt](verification/2026-09-26-verse/testflight-build39.json).

## App identity and source boundary

The user-authorized app metadata and icon preserve the existing Xcode setup:

| Setting | Value |
| --- | --- |
| Project, scheme, target, product | `Coder` |
| Bundle identifier | `com.openagents.coder` |
| Development team | `HQWSG26L43` |
| Marketing version and build | `0.5.0` / `40` |
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

Build `40` makes Verse the home screen and adds a world computer with QR
pairing. Build `39` introduced the shared world; build `38` introduced the
reader. The app
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

# Install and launch synthetic Chats and an offline Verse world on a booted simulator.
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
the checked-in build is `40`. The `export` command produces a local
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

1. Walk toward the computer in Verse and tap **Use computer**. On your
   physical computer, run `cargo run --release -p coder-connect -- connect`
   from the OpenAgents checkout. Keep the command running.
2. Tap **Scan QR code** and scan the computer's invitation. **Paste code**
   accepts the same `coder-pair:` string. Camera permission is requested only
   after choosing to scan; declining it leaves the paste path available.
   Rust validates the invitation and acquires a device-bound read-only grant.
3. Select a saved chat. Use the Rust-provided earlier-page and full-text
   controls to reach original content; a partial page must remain labeled.
4. Keep **Follow new messages** enabled to move to the latest visible record.
   Dragging the transcript pauses following and tells Rust to pin the exact
   cached page that was visible. Enable the toggle to resume. The opaque page
   identity is captured when the interaction occurs, so a refresh that is
   already in flight cannot silently select a different page to pin.

The native shell requests reader refreshes every five seconds while its
computer panel is open and the app is active, and coalesces them while a Rust
call is in progress. Closing the panel or backgrounding stops refreshes.
Rust owns freshness and connection status; cached
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

## Explore Verse

The app launches into the same Rust world used by the desktop app. It starts
offline, facing the computer. Drag in the left half of the surface to move and in the right
half to look around. **Jump**, **Sprint**, and the zoom controls forward typed
requests to Rust; they do not implement a second controller in Swift.

The Rust Native projection names a generic surface resource. The app registers
`verse.world` with a native `CAMetalLayer` mount. wgpu renders the Rust-provided
city, player, and agent into that layer. Native display callbacks run at up to
30 frames per second while the app is active. Opening the world computer
clears held input and pauses player movement. Backgrounding pauses the
render loop, clears held input, and resets the frame clock. Dismantling the mount destroys its Rust surface before
releasing the native layer. Chat synchronization uses a different Rust handle
and worker queue.

Use the computer's world connection controls to join a compatible world relay. This
is an explicit network action that publishes the device's world presence and
movement. **Leave relay** ends that session. The Verse key uses its own
`com.openagents.coder.verse` Keychain service; it never reuses the reader's
identity. Synthetic mode uses a separate fixture account and starts offline.

A renderer failure remains visible with **Retry world renderer**; it does not
leave an unexplained empty surface. The world currently requests three shader
varyings and downlevel device limits instead of assuming desktop defaults.

Native labels, the camera preview, and controls stay outside the GPU glyph
atlas. The computer model and its projected interaction anchor are shared
Rust geometry; no application-specific component enters Rust Native. The
mobile slice provides world rendering, movement, presence, and retained chat
access at the computer; it does
not port the desktop's chat composer, model-backed agent conversations, quest
board, or retained-run replay picker. Those desktop paths remain available
in the desktop app.

## Verification boundaries

The [world-first pairing receipt](verification/2026-09-26-world-pairing/README.md)
covers the current camera and paste paths, world computer, retained reader,
protocol checks, synthetic production-relay exchange, and Rust-to-Apple QR
image decoding. The prior [build 39](verification/2026-09-26-verse/README.md)
and [build 38](verification/2026-09-26/README.md) receipts remain unchanged.

The implementation must retain code/build results separately from simulator
and physical-device behavior. Native controls do not automatically establish
VoiceOver, all keyboard/input methods, very large record navigation, or every
supported iOS version. No real transcript publication, model call, benchmark
run, or physical-device release result is implied by a synthetic smoke check.

See [issue #9696](https://github.com/OpenAgentsInc/openagents/issues/9696), the
[Rust Native contract](../../docs/coder/rust-native/architecture.md), and the
[suite tracker](../../docs/coder/migration-status.md) for scope and acceptance.
