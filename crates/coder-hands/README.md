# coder-hands

What every reader and writer of hand landmarks on CoderOS shares. The
camera daemon [`coderos-camera`](../coderos-camera/README.md) runs the
landmarker and publishes the lines, the Coder compositor reads them and
turns them into desk input, and
[`coder-hands-measure`](../coder-hands-measure/README.md) scores the rules
against recorded runs. Carried from the private Coder repository on
2026-09-28 by the owner's decision to publish it.

| Module | Holds |
| --- | --- |
| `pose` | `Landmark`, the 21 MediaPipe Hands joint constants, `CONNECTIONS` for an overlay, `HandPose` with its labels, and `recognize`, the pose rules with a margin for each. |
| `model` | `Manifest`, the pin `model.toml` compiles in: the URL, the SHA-256, the file name, and the size. `model_path` says where the file lands, `~/.openagents/quest/models/`, and `ensure_model` fetches it with `curl` and refuses a file whose digest differs. `CODER_QUEST_HAND_MODEL` names a file of your own, which skips the pin. The directory and the variable keep the names they had when a second tracker shared the cache, so a host that already holds the file keeps it. |
| `landmarker` | `Landmarker`, the ONNX session over `ort` on Linux, behind the default `landmarker` feature: `load` opens the pinned model and `infer` reads one RGB frame. On another platform `load` says so. |
| `wire` | `Line`, the JSON object the daemon writes for every frame on its hands socket: the timestamp, every hand as 21 points and a pose label, and the tracker's status. |
| `gestures` | The rules a tracked hand becomes desk input by: point, pinch, swipe, and a held fist, with the constants each one turns on. |
| `socket` | Where the camera daemon keeps its sockets, and the verbs a reader asks it. |
| `judge`, `seam`, `watch` | The Jev seam beside the rules, behind the `judge` feature: the window, the three questions, the floors, the thread one request runs on, and the caller that feeds it. [Jev and hand tracking](../../docs/os/hands-judge.md) is the design. |

The crate reads no camera. A caller decodes its own frames and hands them
to `Landmarker::infer`.

The model is the MediaPipe hand landmark model as OpenCV Zoo exports it to
ONNX, at a fixed commit. Move the pin by changing the URL and the digest in
`model.toml` together.

## Tests

```sh
cargo test -p coder-hands
```

The tests run with the `judge` feature on and need no key and no camera.
The seam's test serves `fixtures/jev-hands-seam.json` from a loopback
server, and the gesture tests replay the landmark sequences under
`bench/golden/hands/`.
