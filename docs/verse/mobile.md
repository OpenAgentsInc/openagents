# Verse in Coder for iOS and Android

Coder opens directly into **Verse**, which mounts the same seeded city, player controller,
collision rules, camera, avatar animation, following spade agent, meshes, and
wgpu scene renderer as the desktop application. Rust Native supplies a generic
native drawing-surface contract. Verse supplies the world; `coder-ui` supplies
the application palette. No OpenAgents identity, theme, world, or network
implementation belongs to the reusable `rust-native` crate.

## Walk the world

The world fills the entire display behind the system clock and home indicator.
Controls stay inside the safe area over the canvas. It starts offline and requires no chat pairing
or model account. A computer sits directly ahead of the starting position.
Walk closer and tap **Use computer** to open its controls inside the world.

- In **Touch look**, drag on the left half to move and on the right half to
  turn and look up or down. Releasing the left side stops movement.
- Switch to **Motion look** to look around by turning your phone. Hold the left
  half to walk forward; move that touch to adjust direction. **Recenter** uses
  your current phone position as the new reference without changing the view.
  Switch back to **Touch look** whenever you prefer finger controls.
- Motion look pauses while the app is in the background or an in-world panel
  is open. It starts from a fresh reference when you return. If motion is
  unavailable, use touch look.
- Tap **Jump**, toggle **Sprint**, or use the zoom buttons.
- Use the world computer to pair by QR code and open **Chats**, the read-only
  Codex and Claude viewer. Selecting a transcript expands its reading area;
  **All chats** returns to the smaller computer panel. Close the panel to
  continue walking.
- Walk east to the **GYM** building. Enter, approach the boards, and tap
  **Use Gym board** to inspect Microcoder and Terminal-Bench runs.
  [Gym setup and controls](gym.md) cover its separate host grant, recorded
  charts, and explicitly confirmed run recipes. Leaving pauses Gym updates.
- Follow the [pairing guide](../coder/guides/mobile-readonly.md). The computer
  command displays an expiring QR invitation and stays running to serve chats;
  the phone also accepts its complete pairing string. Camera permission is
  requested only when you choose to scan.

The world uses a 30 Hz native display callback and a single-sample render
target: a Metal layer on iOS and an Android native window on Android. The desktop retains its mouse/keyboard controls and supported 4× MSAA.
Both surfaces run `verse::runtime::WorldRuntime`; mobile does not approximate
the city with a separate scene or image.

## Join another player

The computer controls can join a compatible Nostr world relay. This explicitly
publishes the phone's Verse profile, presence, positions, and gestures. It uses
a separate protected device identity from the encrypted chat reader
(Keychain on iOS; Keystore-encrypted storage on Android). It
neither copies desktop account credentials nor reads the computer's chat grant.

Use the same reachable `wss://` relay URL on desktop and phone. Mobile refuses
credentials, queries, fragments, and plaintext remote connections. On join, it
attempts to recover only its own signed avatar state before publishing its first
position. Recovery has a 1.5-second deadline, including connection setup; a slow
relay falls back to a clear local spawn rather than blocking the render thread. A computer's loopback
address such as `ws://127.0.0.1:7447` refers to the phone itself when entered
there. Use a reachable secure WebSocket deployment with NIP-MV support and
appropriate event limits. The existing [local relay helper](README.md#multiplayer)
serves the desktop world; exposing it to a phone is an operator deployment task.

Mobile publishes moving poses every three seconds, idle poses every ten
seconds, and durable movement state every thirty seconds. Rendering stays at
30 Hz. This deliberately reduces mobile bandwidth and relay pressure; other
players see less frequent position samples than with the desktop's 10 Hz
moving-pose profile. A compatible wire format does not bypass a relay's
admission, authentication, or rate limits.

Backgrounding pauses rendering, clears held input, and cancels the world
connection. Returning starts a fresh motion session if a relay was selected.
Opening the computer stops player movement while keeping the world visible. Cancellation does not prove the relay received an offline state;
other clients must age out stale presence. Leaving the relay clears the local
connection choice. A new native mount starts a new local world.

## Shared code and platform boundaries

| Component | Ownership |
| --- | --- |
| Validated `Surface` element, viewport, active/disposed lifecycle, frame timing | `crates/rust-native`; generic and independent of product crates |
| Seeded geometry, movement/collision, camera, gait, follower, render pipelines | `crates/verse`; shared desktop/iOS/Android implementation |
| Device touch interpretation, world state, connection choices, C bridge | `crates/coder-mobile` |
| GPU surface, display callback, native controls, protected identities, scene lifecycle | Thin SwiftUI/UIKit host in `bins/coder-ios`; Android framework host in `bins/coder-android` |
| Palette | `crates/coder-ui`; outside Rust Native |
| Desktop model chat, retained benchmark-file discovery, verified XP ledger | Desktop feature dependencies; not loaded by the mobile world |

Desktop chat, public-feed panels, XP/quest inspection, and the local Microcoder
versus Fable replay picker remain desktop UI features. The portable replay
clock/track/landmark types remain shared, but this mobile delivery has no replay
artifact importer, XP trust configuration, or world-chat composer. The follower
moves and emotes without a model. Walking through Verse does not start a model or a benchmark. The Gym can
request an explicitly confirmed, host-configured recipe through its separate
execution grant; the chat reader remains read-only.

The `desktop` feature is enabled by default for the Verse executable. Both mobile
applications depend on `verse` with default features disabled: no desktop
harnesses, local Gym result store, knowledge store, or window event loop enter
that targets. The portable `gym-bridge` client observes a separately configured
host; its filesystem readers and execution host are feature-gated out of both mobile targets. Native
text and controls remain native platform widgets; the desktop glyph atlas is not used
as the phone's text renderer.

## Verification and distribution

See the [iOS build guide](../../bins/coder-ios/README.md),
[Android build guide](../../bins/coder-android/README.md),
[Android emulator verification](../coder/verification/2026-09-26-android-mobile.md),
[the Verse verification record](../coder/verification/2026-09-26-verse-mobile.md),
[QR pairing verification](../coder/verification/2026-09-26-world-pairing.md),
[Gym verification](../coder/verification/2026-09-26-verse-gym.md),
[full-screen and motion verification](../coder/verification/2026-09-26-fullscreen-motion.md),
and [issue #9698](https://github.com/OpenAgentsInc/openagents/issues/9698), and
[Verse-first pairing #9699](https://github.com/OpenAgentsInc/openagents/issues/9699).
Simulator rendering and lifecycle evidence are separate from physical-device
frame rate, thermals, and a two-device relay session. Only checks recorded in
that verification document have been performed.
