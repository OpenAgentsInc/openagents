# Verse in Coder for iOS and Android

Coder opens directly into **Verse**, which mounts the same seeded city, player controller,
collision rules, camera, avatar animation, following spade agent, meshes, and
wgpu scene renderer as the desktop application. Rust Native supplies a generic
native drawing-surface contract. Verse supplies the world; `coder-ui` supplies
the application palette. No OpenAgents identity, theme, world, or network
implementation belongs to the reusable `rust-native` crate.

## Walk the world

The world fills the entire display behind the system clock and home indicator.
Camera icons sit at the bottom right inside the safe area. The hand/gyroscope
icon switches touch and motion look; the crosshair recenters the camera. The world has no
title banner or idle-status labels. It starts offline and requires no chat
pairing or model account. A computer sits directly ahead of the starting position.
Walk closer and tap the computer's monitor to open its controls. The monitor
shows **WALK CLOSER** until you are within reach, then **TAP TO OPEN**.

- In **Touch look**, drag on the left half to move and on the right half to
  turn and look up or down. Releasing the left side stops movement.
- Switch to **Motion look** to look around by turning your body with the phone
  or pointing the phone left, right, up, or down. Screen roll does not tilt the
  horizon. The shared Rust camera interpolates toward the latest orientation
  on each rendered frame. Hold the left half to walk forward; move that touch
  to adjust direction. **Recenter** uses
  your current phone position as the new reference and returns the camera behind
  your character at its default pitch. Zoom stays unchanged.
  Switch back to **Touch look** whenever you prefer finger controls.
- Motion look pauses while the app is in the background or an in-world panel
  is open. It starts from a fresh reference when you return. If motion is
  unavailable, use touch look.
- Double-tap open world space with your right thumb to jump, including while
  your left thumb keeps moving. You can also move on the left and drag the
  camera on the right at the same time. The computer keeps its single-tap action.
- Place two fingers together, then spread them to zoom in or pinch inward to
  zoom out. A deliberate pinch owns those touches until you lift them. Adding
  a right-hand control after movement starts keeps independent controls active.
  The HUD has no walk/sprint toggle, jump button, or zoom buttons.
- Use the world computer to pair by QR code and open **Chats**, the read-only
  Codex and Claude viewer. Selecting a transcript expands its reading area;
  **All chats** returns to the chat list. Close the panel to
  continue walking.
- Walk east to the **GYM** building. Enter, approach the boards, and tap
  **Use Gym board** to inspect Microcoder and Terminal-Bench runs.
  [Gym setup and controls](gym.md) cover its separate host grant, recorded
  charts, and explicitly confirmed run recipes. Leaving pauses Gym updates.
- Follow the [pairing guide](../coder/guides/mobile-readonly.md). The computer
  command displays an expiring QR invitation and stays running to serve chats;
  the phone also accepts its complete pairing string. Camera permission is
  requested only when you choose to scan.

The iOS world requests a 60 Hz display callback (30 Hz minimum); Android
follows its native display callback. Motion samples are requested at 60 Hz.
Both use a single-sample render target: a Metal layer on iOS and an Android
native window on Android. Desktop retains its mouse and keyboard controls and
supported 4× MSAA.
Both surfaces run `verse::runtime::WorldRuntime`; mobile does not approximate
the city with a separate scene or image.

Native motion adapters supply device-to-world quaternions and separate sample
and receipt timestamps. Freshness is checked at receipt, so a slow previous
render cannot make a fresh sensor reading look stale. Recenter, app suspension,
and panel transitions discard the previous motion target. The camera keeps
its eye above the ground while preserving the requested view direction,
including upward look; the ground clamp does not force it to keep looking
at the avatar. See the
[motion-camera verification](../coder/verification/2026-09-26-motion-camera.md).

## Map navigation

Tap the top-right map to expand it, then choose a clear position or landmark to
walk there. The character follows a collision-aware route; manual movement or
jumping stops it. Camera input can continue while walking. See
[maps, companions, and doors](world-interactions.md) for the ordered demo work.

## Use the world computer

The computer prompt is part of the 3D monitor. Its amber lettering and corner
marks are Rust-rendered geometry on the display surface. They change size and
perspective with the camera, and world geometry can obscure them. There is no
visible SwiftUI or Android **Computer** button floating above the object.

Tap the display itself. Rust traces that screen position into the world and
accepts it only when the nearby monitor is facing the camera and the path is
clear. Touch and motion camera modes use the same interaction. A drag, cancelled
touch, long hold, or second finger does not open the computer. Tapping the
monitor also avoids starting a movement or camera gesture.

VoiceOver and TalkBack expose a **Use computer** action on the world surface
when the computer is in reach. That action uses the same Rust proximity and
visibility checks without drawing another control over the scene.

Opening the computer still shows the existing native pairing, world-connection,
and chat-reader panels over Verse. This change moves the computer prompt and
its pointer interaction into the 3D world; it does not replace the reader,
keyboard, QR scanner, or transcript selection with a 3D UI. Closing the panel
returns to walking.

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
seconds, and durable movement state every thirty seconds. Those publication
intervals are independent of display callbacks. This reduces mobile bandwidth
and relay pressure; other
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
| Seeded geometry, movement/collision, camera, gait, follower, render pipelines, monitor lettering, and world picking | `crates/verse`; shared desktop/iOS/Android implementation |
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
those targets. The portable `gym-bridge` client observes a separately configured
host; its filesystem readers and execution host are feature-gated out of both mobile targets. Reader text, pairing controls, and other panels remain native platform widgets.
The computer prompt is scene geometry; it uses neither native text widgets nor
the desktop glyph atlas.

## Verification and distribution

See the [iOS build guide](../../bins/coder-ios/README.md),
[Android build guide](../../bins/coder-android/README.md),
[iOS startup and gesture correction](../coder/verification/2026-09-26-ios-static-link.md),
[Android emulator verification](../coder/verification/2026-09-26-android-mobile.md),
[the Verse verification record](../coder/verification/2026-09-26-verse-mobile.md),
[QR pairing verification](../coder/verification/2026-09-26-world-pairing.md),
[Gym verification](../coder/verification/2026-09-26-verse-gym.md),
[full-screen and motion verification](../coder/verification/2026-09-26-fullscreen-motion.md),
[world-computer interaction verification](../coder/verification/2026-09-26-world-computer.md),
[issue #9698](https://github.com/OpenAgentsInc/openagents/issues/9698), and
[Verse-first pairing #9699](https://github.com/OpenAgentsInc/openagents/issues/9699).
Simulator rendering and lifecycle evidence are separate from physical-device
frame rate, thermals, and a two-device relay session. Only checks recorded in
the linked verification records have been performed.

## World connection

At the computer, open **…** to view the world connection. **Join** saves the
selected relay on this device; the app reconnects when reopened. **Connected**
means the world subscriptions are accepted, including authentication when the
relay requests it. It does not mean other players are currently present.
Failures appear in the same panel. Joining or reconnecting keeps your current
position and leaves the panel open. A fresh app mount may restore your own
signed saved position from the relay.

**Leave** stops the connection and forgets the saved relay. World presence uses
its own identity and does not pair a chat reader. Synthetic acceptance tests
show **Preview** and publish no world events.

## Companion and demo gates

Tap your floating companion for a short wiggle and hop. Use the map's Spark
or Halo shortcut to approach a local demo gate, then turn to face it. Choose a
key in the nearby strip and tap the gate to see its effect and destination.
Tap again after the effect to walk there. Empty hands reuse that gate's last
compatible key; Reset clears its choice. Choices stay on this device, while
walks and effects stop when the app becomes inactive. See the
[interaction guide and portal specification](world-interactions.md).
