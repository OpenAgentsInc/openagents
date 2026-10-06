# Authoritative chamber clients

The [current capability table](status.md) covers the engine and world services.
The generated [runtime contract](runtime-contract.json) records current wire
and rules versions and declared client features. Historical receipt versions
identify their measured source rather than current protocol compatibility.

The chamber mounts one `verse-world` client, worker, prediction view, and
`verse-imported::chamber_session::Session`. Grid relay presence and the offline
Everglade and Grove are separate modes. A reachable host grant and an enrolled
world character are distinct permissions; a connection does not grant a game
role. The device key that proves the channel also signs the chamber challenge.

## Current feature matrix

| Capability | Native desktop | Rust mobile mount | Browser `?zone=chamber` |
| --- | --- | --- | --- |
| Authenticated chamber | TLS, REACH TCP, or REACH WebSocket | Same native connector with injected world key | REACH WebSocket, browser-managed TLS for `wss` |
| Authority, prediction, damage, death, respawn | Shared client and session | Shared client and session | Shared client and session |
| Movement and camera | Desktop controls | Touch controls, validated Rust remapping | Keyboard, touch buttons, pointer camera, and gamepad |
| Combat HUD and captions | Shared overlay, native audio | Shared overlay and captions | Shared overlay and labeled DOM controls and captions |
| Inventory and quest panels | Desktop panels | Combat subset | Combat subset |
| Output audio | Native callback mixer | Captions; output adapter pending | Captions; output adapter pending |
| Interruptions | Close/reconnect | Suspend/resume destroys worker and rejoins | Focus/visibility destroys worker and GPU; reconnect reloads route configuration |
| Verification in V24 | Native loopback, mounting tests, compile | Rust mounting/lifecycle tests; no physical phone run | Wasm link and headless software WebGL2: authority movement, DOM sizing, focus/reconnect, and grants; no hardware, gamepad, or screen-reader run |

This matrix describes code paths, not equal device performance or complete MMO
feature parity. The software WebGL2 smoke verifies authoritative movement and
stopped input at a 1000-by-800 CSS viewport. Full-resolution software rendering
initially ran at about four frames per second and retired control. The browser
now lowers its render scale after sustained frames over 40 ms, down to 1/16,
and slowly recovers resolution after 300 frames under 20 ms. The final probe
uses a 237-by-190 backing canvas; DOM controls and captions retain their CSS
size. The HUD reports reduced graphics quality. This reduces graphics work on
the shared session thread; it does not establish hardware or crowded-world
budgets. The browser caps unread channel data at four bounded messages plus one
partially read frame;
the DOM allocates incoming messages before delivering them to Rust. That cap
cannot establish a pre-delivery browser network allocation limit.

## Native configuration

The existing `ritual.json` retains its `address`, `instance`, `pack`, `scene`,
and `dir` fields. TLS routes require `trust_der` and use `server_name` (default
`localhost`). Optional `reach` adds `host`, `grant`, `epoch`, `generation`, and
`websocket`. Without `websocket`, the route uses authenticated REACH TCP. With
an exact `ws://` or `wss://` URL, it uses the existing framed WebSocket adapter;
`wss` also requires the trust certificate. Re-enrollment must update generation
and grant configuration. Stale grants fail rather than widening permissions.

Mobile requests use the existing Rust request bridge. To replace the active
visit's bindings, send `{"action":"chamber_bindings","bindings":[
{"control":"TouchForward","action":"strafe_left"}]}`. This replaces the
entire map. Invalid maps leave it intact. Bindings survive that visit's suspend
and reconnect; held controls require release before they activate again.
The native widget host has no new domain or transport implementation.

## Browser configuration

Serve the built `everglade-web` module and existing canvas page, then open
`?zone=chamber`. Serve `chamber.json` beside the page:

```json
{
  "websocket": "wss://game.example/chamber",
  "host": "HOST_PUBLIC_KEY_64_LOWERCASE_HEX",
  "grant": "GRANT_DIGEST_64_LOWERCASE_HEX",
  "epoch": 1,
  "generation": 1,
  "instance": 1,
  "content": "ADMITTED_CONTENT_DIGEST_64_LOWERCASE_HEX",
  "pack": "chamber/pack.json",
  "scene": "chamber/scene.json",
  "assets": "chamber/assets",
  "mips": false,
  "bindings": []
}
```

Replace placeholders with the host's enrollment and admitted content values.
The pack is the JSON pack from `openagents chamber pack`, not Everglade's VTP.
Asset paths are bounded relative paths on the same origin; downloads omit
credentials, refuse redirects, and have size and time limits. With `mips: true`,
serve the admitted `mips.json` and `mips.rgba` under `assets`. The client recomputes
the admitted pack, scene, texture, and mip identity before joining.

The page displays its persistent world public key. Enroll that key out of band,
and publish its grant configuration before reconnecting. The secret remains in
the existing Grid local-storage identity slot. This is browser-origin storage,
not hardware-backed key custody. A route refresh can update enrollment but
cannot switch instance or content during a visit; reload the page for that.

An empty `bindings` selects defaults; a nonempty list replaces them. Control
names include physical `KeyW`, `Digit1`, `Space`, `TouchForward`, and `PadForward`.
Actions include `forward`, `strafe_left`, `jump`, `target`, `respawn`, and
`{"cast":0}` through `{"cast":9}`. The shared map bounds names and counts,
rejects duplicates, combines independently held controls, and suppresses repeated
presses. Browser Tab remains available for DOM focus. Buttons have labels and
minimum 44-pixel targets; status changes use a polite live region.

## Verification limits

[V24 evidence](../../bench/verse/2026-10-05/platform-clients/README.md) records
source and targeted checks. [Owner checks](../../NEEDS_OWNER.md) retain the
physical phone, supported-browser, screen-reader, network interruption, frame,
thermal, and audio-output work. Use scratch hosts and keys; do not run acceptance
against the owner's resident services or retain test chats.
