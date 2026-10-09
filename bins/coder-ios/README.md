# Coder for iOS

Coder opens into **Verse**. Walk up to the computer in the world to pair by
QR code and open the read-only **Chats** reader. Tap the physical monitor:
Rust renders the **COMPUTER** and **TAP TO OPEN** text in the 3D scene and
hit-tests its visible surface. There is no native Computer button. These are public Rust-owned
surfaces under the existing Coder app identity. Rust owns
pairing, protocol verification, synchronization, encrypted reader cache, world
state, movement, rendering, and application projection. The thin SwiftUI host
renders Rust Native views, keeps separate device identities in Keychain, and
forwards bounded UI events. The [Gym building](../../docs/verse/gym.md)
observes separately granted host records and requests configured run recipes
after explicit confirmation. The phone itself runs no model or benchmark;
the chat reader remains read-only.

**Latest internal TestFlight: 0.5.0 (53).** App Store Connect confirms `VALID`
and `IN_BETA_TESTING`. The [build 53 record](verification/2026-09-27-plaza-neon-build53/README.md)
binds it to clean source commit `f3bb5b36ab`. The plaza draws as a neon
stage in its own amber palette with glowing lines. (Build 53's floor
reflection was removed afterward.)

Build 52 ([record](verification/2026-09-27-lagrange-realism-build52/README.md))
renders Lagrange 1 with physical light: real-unit sunlight and shadows,
bounce light, measured materials, the textured Earth and Moon and catalogue
stars, and an **Art** camera preset.

Build 51 adds whole-screen touch look and a bottom movement stick
([record](verification/2026-09-27-touch-look-stick-build51/README.md)).

Build 50 binds to clean source commit `c7451dfd71`: the Atlantis zone is
renamed Ruins, and a second plaza portal opens [Lagrange 1](../../docs/verse/lagrange-1.md)
([record](verification/2026-09-27-ruins-lagrange-build50/README.md)).

Build 49: App Store Connect confirms `VALID`
and `IN_BETA_TESTING`. The [distribution receipt](verification/2026-09-27-atlantis-realtime-build49/testflight-build49.json)
binds it to clean source commit `07bf4c2188`. Atlantis now runs the original
Wizard Woods real-time combat and player controller. The GPU hotbar casts
Firebolt, Magic Missile, and Fireball while monsters pursue and NPC wizards
cast. Mana, cooldowns, projectiles, damage, statuses, and destructible ruins use
the retained simulation. The turn-based encounter is removed.

[Native acceptance and recording](verification/2026-09-27-atlantis-realtime-build49/native-verification.md)
show production startup, autonomous combat, Fireball launch/flight/burst during
movement, cooldown recovery, and return to the plaza. The
[source audit](../../docs/verse/ruins-source-parity.md) records fidelity and
inherited limits. Physical-device acceptance remains separate.

Build 48's default public relay and world-rendered Gym entry remain. New or
unconfigured installs join `wss://relay.openagents.com` automatically; custom
relays and explicit **Leave** choices survive relaunch. Synthetic previews stay
offline. Gym records require their separate host grant. The
[connection/Gym acceptance](verification/2026-09-27-world-gym-build48/native-verification.md)
and [signed relay evidence](verification/2026-09-27-relay-presence/README.md)
remain retained. Notification wakes remain unconfigured.

Build 47 adds the [Atlantis forest portal](../../docs/verse/zones.md), original
runtime-loaded artwork, separate forest palette, local SRD 5.1 encounter, and
return to the saved plaza position. Its [acceptance](verification/2026-09-27-forest-zones/README.md)
retains normal optimized launch, an empty-cache HTTPS download, native
portal/encounter/return controls, cached reentry, and signed packaging checks.
Build 49 replaces that historical turn-based demo with the original Wizard
Woods real-time simulation; see
[the source audit](../../docs/verse/ruins-source-parity.md).

Build 46 adds the [expandable map, companion reactions, and
Spark/Halo gates](../../docs/verse/world-interactions.md). Select a
map landmark or a clear position to walk there; manual movement cancels the
route. Tap the companion for a brief wiggle and hop. Near a gate, choose a
demo item, tap to inspect its destination, then tap again to walk there. Item
and gate memory survive relaunch; they provide no service or payment authority.

The same update adds a one-command computer pairing, automatic chat loading after
pairing, concise chat panels, a remembered world relay, and composed left-side
movement with right-side look or double-tap jump. Camera mode and **Recenter**
use labeled icons. [Connection evidence](../../docs/coder/verification/2026-09-26-mobile-connections.md)
and [world-interaction evidence](verification/2026-09-26-world-interactions/README.md)
separate Rust checks, native acceptance, and distribution. The archive pins
source commit `a0426ded9d`; later main-branch integrations have separate checks
and are not implicitly included in that binary.

Walk east into the [Gym](../../docs/verse/gym.md) for Microcoder and
Terminal-Bench boards. Its observations and run recipes require a separate
host grant. Leaving or backgrounding pauses Gym updates.

The motion-camera changes introduced in build `44` and included in working
build `45` correct left and right body turns, enable upward look, and interpolate
camera movement. Native updates request 60 Hz; fresh
samples remain valid after a slow frame. See the
[motion-camera verification](../../docs/coder/verification/2026-09-26-motion-camera.md).

The world-computer changes introduced in build `43` and included in working
build `45` replace the native computer button with the shared world-space
monitor interaction. See [verification](../../docs/coder/verification/2026-09-26-world-computer.md).

Build `42` fills the display behind the system clock and home indicator.
Switch between whole-screen touch look and phone-motion look; use **Recenter**
to establish a comfortable reference. In motion mode, hold the bottom stick to
walk forward. See [controls](../../docs/verse/mobile.md) and
[verification](../../docs/coder/verification/2026-09-26-fullscreen-motion.md).

## App identity and source boundary

The user-authorized app metadata and icon preserve the existing Xcode setup:

| Setting | Value |
| --- | --- |
| Project, scheme, target, product | `Coder` |
| Bundle identifier | `com.openagents.coder` |
| Development team | `HQWSG26L43` |
| Marketing version and source build | `0.5.0` / `53` |
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

Build `41` adds the shared Gym building, recorded native charts, and separately
authorized recipe requests. Build `40` makes Verse the home screen and adds a world computer with QR
pairing. Build `39` introduced the shared world; build `38` introduced the
reader. The app
identity, marketing version, signing team, and distribution profile stay the
same; the build number advances for this replacement implementation.

## Build and install

Prerequisites: the pinned Rust toolchain, `aarch64-apple-ios` and
`aarch64-apple-ios-sim` targets, Xcode with the iOS SDK, and `xcodegen`.
The helper links the exact `libcoder_mobile.a` static archive, generates the existing
`Coder.xcodeproj`, and invokes `xcodebuild`. Build products stay outside the
checkout in the worktree's target directory. Compiler environments exclude
provider and service credentials. Every built app passes
`scripts/verify-coder-ios-bundle.py`, which verifies signatures and native
library dependencies. A reference to a Cargo output directory cannot pass;
Android's dynamic-library output does not change the iOS link choice.

```sh
# Build only; do not install or launch.
scripts/build-coder-mobile.sh sim-build

# Install and launch synthetic Chats and an offline Verse world on a booted simulator.
scripts/build-coder-mobile.sh sim --synthetic

# Run normal-startup and synthetic UI checks on the selected simulator.
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
optimized release profile. Simulator commands default to development Rust;
set `CODER_IOS_RUST_PROFILE=release` for release acceptance. Before a TestFlight
upload, launch that optimized app in a dedicated simulator without synthetic
arguments, run `ProductionLaunchUITests`, and inspect the rendered world.
Keep this mobile acceptance check separate from the full workspace release gate.
Commit the release source before archiving. The helper records the commit,
tracked-diff digest, workspace status, Cargo lock digest, compiler versions,
and archived executable digest beside the archive. A nonempty workspace status
must be reviewed rather than described as an exact committed-source build.
Set `CODER_IOS_BUILD_NUMBER` to a new
positive build number after checking the existing App Store Connect builds;
the checked-in and confirmed internal TestFlight build is `53`.
The `export` command produces a local
distribution package using the existing export settings with destination
changed to `export`. Upload is a separate operator action using the retained
upload configuration and protected App Store Connect credentials. Never put
API key material in a command argument, log, or repository file.

`sim-test` uses the selected simulator and retains an `.xcresult` bundle under
`CODER_IOS_OUTPUT`. `ProductionLaunchUITests` uses normal app startup without
test arguments; feature tests use their explicit synthetic fixtures. Use a
dedicated simulator when another agent is testing on the same machine.
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

1. Walk toward the computer in Verse, tap its monitor, and choose **CHATS**.
   (The computer opens on **COMPUTERS**, your linked hosts, drawn in the
   world HUD; see [the world computer](../../docs/verse/mobile.md#the-computers-screen)
   and [its verification](../../docs/coder/verification/2026-09-27-verse-computer-hud.md).)
   On your physical
   computer, run `openagents pair`. Keep the command running.
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
headings, tables, and fenced blocks retain their source syntax. Only `https`
links open, in the system browser. Tool
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

Arches on the plaza lead to local [zones](../../docs/verse/zones.md). The
Ruins zone was removed on 2026-10-05. **L1 portal** leads to
[Lagrange 1](../../docs/verse/lagrange-1.md), a construction station at the
Sun–Earth L1 point: fly a maneuvering pack, **Grab** parts at the depot, and
**Latch** them into the keel jig. **Plaza** returns to your saved position at
any time. Plaza presence and Gym observation pause while you are in a zone.
Rules and artwork notices are in **Computer > Settings > About Verse**. Build 47
acceptance and its release receipt are retained in the
[forest verification record](verification/2026-09-27-forest-zones/README.md).


The app launches into the same Rust world used by the desktop app. It starts
in the public plaza unless you saved an offline choice, facing the computer, with no title banner or idle-status labels over
the world. Drag anywhere on the surface to look around, and push the
translucent stick above the bottom-left safe area to walk, back up, or strafe.
**Motion look** uses the phone's orientation instead; hold the stick to walk
and use the crosshair to return the camera behind your character.

Double-tap open world space to jump, including while holding the stick to move.
The stick and camera dragging also work together. Spread two fingers to
zoom in, or pinch them together to zoom out. The computer monitor keeps its
single-tap interaction. Pinching cancels held movement and pending taps. The
HUD has no walk/sprint toggle, jump button, or zoom buttons. Rust owns gesture
interpretation and camera bounds; Swift forwards native touch and pinch
updates. See the [startup and gesture correction](../../docs/coder/verification/2026-09-26-ios-static-link.md)
for verification and delivery status.

The Rust Native projection names a generic surface resource. The app registers
`verse.world` with a native `CAMetalLayer` mount. wgpu renders the Rust-provided
city, player, and agent into that layer. Native display callbacks request
60 frames per second, with a 30 Hz minimum, while the app is active. Opening the world computer
clears held input and pauses player movement. Backgrounding pauses the
render loop, clears held input, and resets the frame clock. Dismantling the mount destroys its Rust surface before
releasing the native layer. Chat synchronization uses a different Rust handle
and worker queue.

New or unconfigured installs join `wss://relay.openagents.com` by default and
publish the device's world presence and movement. Use the computer to change
relays or choose **Leave**. The app remembers a custom relay or an explicit
offline choice across relaunch. The Verse key uses its own
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

## Push wakes

Push is off by default. A default build asks for no notification permission,
never calls `registerForRemoteNotifications`, and signs without the
`aps-environment` entitlement, so it keeps signing with the existing
`OpenAgents Coder App Store` profile, which doesn't include Push
Notifications yet.

A push build needs two switches. Both are build settings, and neither is in
`project.yml`:

- **Push settings.** `CODER_PUSH_RELAY_URL` (`wss://`),
  `CODER_PUSH_GATEWAY_URL` (`https://`), and `CODER_PUSH_APP_PROFILE` reach
  the app's `Info.plist` as `CoderPushRelayURL`, `CoderPushGatewayURL`, and
  `CoderPushAppProfile`. When all three are set, the app passes them to Rust,
  asks for notification permission, and registers for remote notifications.
  It passes the APNs token to Rust's `push_token` as lowercase hexadecimal at
  every launch. A declined permission or a failed registration shows as the
  wake status under the Computer panel's status line.
- **Entitlement.** `CODER_IOS_PUSH=development` or `production` signs with
  `host/Push/Coder-Push.entitlements`, which sets `aps-environment` to that
  value. An archive for TestFlight needs `production`.

Before you use the entitlement switch, the owner does the following:

1. In the Apple Developer portal, under **Certificates, Identifiers &
   Profiles** > **Identifiers**, open `com.openagents.coder` and turn on
   **Push Notifications**. Save.
2. Under **Profiles**, open **OpenAgents Coder App Store**, select **Edit**,
   then **Save**, and download it so the profile includes the push
   capability. Regenerate any development profile you use the same way.
3. Build with the switches:

   ```sh
   CODER_IOS_PUSH=production \
   CODER_PUSH_RELAY_URL=wss://relay.example.com \
   CODER_PUSH_GATEWAY_URL=https://push.example.com \
   CODER_PUSH_APP_PROFILE=coder-ios \
   scripts/build-coder-mobile.sh archive
   ```

Set the gateway's `PUSH_GATEWAY_APNS_ENVIRONMENT` to match: `production` for
TestFlight and App Store builds, `development` for development-signed builds.
See the [push gateway runbook](../../docs/deployment/push-gateway.md).

`PushRegistrationUITests` checks the handoff in a simulator. It runs only
with `TEST_RUNNER_CODER_PUSH_TEST=1` against a build made with
`CODER_IOS_PUSH=development` and loopback push settings
(`ws://127.0.0.1:9`, `http://127.0.0.1:9`), and is skipped otherwise.

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
