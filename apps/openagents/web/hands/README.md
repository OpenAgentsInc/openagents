# openagents-web-hands

Hand input for [openagents.com/lander9](../../../../crates/openagents-web/src/pages/lander9.rs):
the page's hand landmarker hands this crate 21 points per hand, and the crate
turns them into smoothed hands, gestures, and pointer input. The demo
(`../graph-card`, `hands` feature) draws the hands and runs the interactions.

| Module | Holds |
| --- | --- |
| `filter` | The One Euro filter (Casiez, Roussel, and Vogel, 2012), one per coordinate. |
| `track` | Each hand's pose with `crates/coder-hands`' rules and constants: pinch closes under 0.35 of a palm and opens past 0.55 (`PINCH_RELEASE`), a fist after 60 ms, an open palm held for a second, the pointing rule. The first hand moves the pointer and presses (where the cursor was 120 ms before the pinch closed); two hands pinching spread the parts. The packed frame format the page sends. |
| `synth` | Landmarks for a pose at a place on the page, for the tests and the page's test hook. |

```sh
cargo test -p openagents-web-hands
cargo test -p openagents-web-graph-card --features hands   # hit testing, dragging, drop-to-start
```

## The landmarker: a third-party runtime exception

The model runs in the browser through Google's MediaPipe Tasks Vision
HandLandmarker 0.10.35 (JS API, wasm runtime, and the float16 hand landmarker
model, all Apache-2.0). The owner approved running MediaPipe in the browser
for `/lander9` on 2026-10-20, in place of a Rust inference path. The exception
is narrow:

- `scripts/fetch-hand-landmarker.sh` fetches the files pinned by SHA-256 and
  writes a `NOTICE`; the site serves them from its own origin under
  `/lander9/hands/`. Nothing is loaded from a CDN.
- The page loads none of it until the hand button is clicked.
- The page's script only moves frames and points: it runs the landmarker on
  the camera's `<video>` element and passes the points to the demo's
  `hand_frame`. The smoothing, gestures, hit testing, and interactions are in
  Rust (this crate and `../graph-card`).
- Camera frames never leave the page. Only the points reach the wasm module,
  and nothing is sent to the server.

The rules are the ones `crates/coder-hands` measured for the CoderOS camera.
OpenAgentsInc/commander's earlier MediaPipe Hands demo (AGPL-3.0) was read for
reference only; nothing here is copied from it.
