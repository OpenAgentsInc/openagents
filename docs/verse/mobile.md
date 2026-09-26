# Verse in Coder for iOS

The **Verse** tab in Coder mounts the same seeded city, player controller,
collision rules, camera, avatar animation, following spade agent, meshes, and
wgpu scene renderer as the desktop application. Rust Native supplies a generic
native drawing-surface contract. Verse supplies the world; `coder-ui` supplies
the application palette. No OpenAgents identity, theme, world, or network
implementation belongs to the reusable `rust-native` crate.

## Walk the world

Open **Verse** beside **Chats**. The world starts offline and requires no chat
pairing or model account.

- Drag on the left half of the world to move. Releasing stops movement.
- Drag on the right half to turn and look up or down.
- Tap **Jump**, toggle **Sprint**, or use the zoom buttons.
- Switch to **Chats** to use the existing read-only Codex and Claude viewer.

The world uses a 30 Hz native display callback and a single-sample Metal render
target. The desktop retains its mouse/keyboard controls and supported 4× MSAA.
Both surfaces run `verse::runtime::WorldRuntime`; mobile does not approximate
the city with a separate scene or image.

## Join another player

The connection menu can join a compatible Nostr world relay. This explicitly
publishes the phone's Verse profile, presence, positions, and gestures. It uses
a separate device-only Keychain identity from the encrypted chat reader. It
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

Leaving the tab or backgrounding pauses rendering, clears held input, and
cancels the world connection. Returning starts a fresh motion session if a relay
was selected. Cancellation does not prove the relay received an offline state;
other clients must age out stale presence. Leaving the relay clears the local
connection choice. A new native mount starts a new local world.

## Shared code and platform boundaries

| Component | Ownership |
| --- | --- |
| Validated `Surface` element, viewport, active/disposed lifecycle, frame timing | `crates/rust-native`; generic and independent of product crates |
| Seeded geometry, movement/collision, camera, gait, follower, render pipelines | `crates/verse`; shared desktop/iOS implementation |
| Device touch interpretation, world state, connection choices, C bridge | `crates/coder-mobile` |
| Metal layer, display callback, native controls, Keychain, scene lifecycle | Thin SwiftUI/UIKit host in `bins/coder-ios` |
| Palette | `crates/coder-ui`; outside Rust Native |
| Desktop model chat, retained benchmark-file discovery, verified XP ledger | Desktop feature dependencies; not loaded by the mobile world |

Desktop chat, public-feed panels, XP/quest inspection, and the local Microcoder
versus Fable replay picker remain desktop UI features. The portable replay
clock/track/landmark types remain shared, but this mobile delivery has no replay
artifact importer, XP trust configuration, or world-chat composer. The follower
moves and emotes without a model. Mobile Verse does not start a model or a
benchmark, and the chat reader remains read-only.

The `desktop` feature is enabled by default for the Verse executable. The iOS
application depends on `verse` with default features disabled: no desktop
harnesses, Gym, knowledge store, or window event loop enter that target. Native
text and controls remain SwiftUI controls; the desktop glyph atlas is not used
as the phone's text renderer.

## Verification and distribution

See [the native app build guide](../../bins/coder-ios/README.md),
[the Verse verification record](../coder/verification/2026-09-26-verse-mobile.md),
and [issue #9698](https://github.com/OpenAgentsInc/openagents/issues/9698).
Simulator rendering and lifecycle evidence are separate from physical-device
frame rate, thermals, and a two-device relay session. Only checks recorded in
that verification document have been performed.
