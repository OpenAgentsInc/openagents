# coderos-camera

One owner of the camera on a CoderOS desktop. The daemon opens the camera
node once and serves every consumer at the same time: a `v4l2loopback`
node for the programs that read a camera node, a recording of the camera to
a file, and hand landmarks on a socket. Before it, the camera circle's
`mpv` held `/dev/video0` and nothing else could read it. Carried from the
private Coder repository on 2026-09-28 by the owner's decision to publish
it; [Camera and hands](../../docs/os/camera-and-hands.md) describes how it
fits a CoderOS desktop.

## What it does

`coderos-camera serve` reads the grant the host wrote, opens the camera
with V4L2 at the size and rate the grant names, MJPEG first and YUYV when
the camera has no MJPEG, and decodes each frame once. Every frame then goes
to three outputs, each on a thread of its own behind a mailbox that holds
one frame, so an output that falls behind drops frames rather than holding
the camera and the other outputs back:

- **The loopback.** The frame, as YUYV, onto the `v4l2loopback` node the
  grant names, `/dev/video10` by default. `mpv` for the camera circle, Zoom,
  and a browser in a sandbox open that node when they ask for a camera, so
  `camera-overlay` and the sandboxes change nothing but the device name. A
  node that is absent, because the module is not loaded, is reported in
  `status` and tried again every five seconds.
- **The recording.** While a recording runs, the frame goes over a pipe to
  an `ffmpeg` child as raw RGB, which writes an H.264 MP4 at a constant
  frame rate against the wall clock. The recorder reads the daemon's frames
  rather than the loopback node because a pipe works on a host with no
  `v4l2loopback` module, never competes with the circle and the sandboxes
  for the node, and carries the same frames every other output got.
- **The tracker.** While tracking is on, the frame goes through the ONNX
  landmarker in [`coder-hands`](../../crates/coder-hands/README.md), and one
  JSON line a frame goes to every reader of the hands socket.

The daemon holds the node for the life of the session. `SIGTERM`,
`SIGINT`, and the `stop` verb end it the same way: the outputs finish the
frames they hold and close, a running recording gets the ending `record
stop` gives one, the socket files go with the process, and the exit is 0.
It does not stream to a network; that is a later issue. It reads one
camera, and it does not answer the desk protocol.

## The sockets

Both sockets sit under `$XDG_RUNTIME_DIR/coderos-camera/`, the directory a
sandboxed client can reach, the way the desk socket does. A host with no
runtime directory keeps them under `~/.openagents/camera/`, and
`CODEROS_CAMERA_DIR` names another directory for a test.

### `control.sock`

One JSON object a line in each direction, the shape the desk protocol uses
([`crates/coder-desk`](../coder-desk/README.md)): a caller connects, writes one
request with its `generation` and its `type`, reads one answer, and
closes. `src/protocol.rs` holds the types.

| Verb | Asks or answers |
| --- | --- |
| `status` | The camera node, the format it streams, its size, the frames read, the rate over the last second, the tracker's status line, the file a recording writes, and every output. |
| `outputs` | Every output: its name, its target, its state, the frames it took, and the frames it dropped. |
| `record_start` | Starts a recording at `path`, or under `~/Videos/` when the request names none. Refused with `already_recording` while one runs and with `cannot_record` when `ffmpeg` does not start. |
| `record_stop` | Stops the recording and answers its receipt. Refused with `no_recording` when none runs. |
| `hands_on` | Starts the tracker. The first start fetches the model when the cache is empty; `status` says `fetching the hand model` while it does. |
| `hands_off` | Stops publishing landmarks. The model stays loaded. |
| `stop` | Shuts the daemon down the way `SIGTERM` does: the outputs close, a recording gets its trailer and tag, and the sockets are removed. |

A change verb answers `done`, `record_stop` answers `recorded` with the
receipt, and any verb can answer `refused` with a code and a message. A
request at another generation is refused with `unsupported_generation`,
and a verb this daemon does not know with `unknown_verb`.

### `hands.sock`

One JSON object a line for every frame the tracker read, to every reader
that is connected, and nothing while tracking is off. The line carries
`timestamp`, seconds since the Unix epoch when the frame was read; `hands`,
an array with one entry per hand, each holding `landmarks`, 21 `{x, y, z}`
points normalized to the frame in MediaPipe order, and `pose`, the label
`coder_hands::HandPose::label` gives, such as `pinch`; and `status`, the
tracker's line, such as `1 hand(s) detected`; and `dropped`, the frames
the daemon's hands lane dropped between the last line and this one, so a
reader knows when it fell behind. A reader whose socket is full is
dropped, because a reader that stopped reading would otherwise hold the
tracker's thread. `coder_hands::wire::Line` parses and renders the line,
so a reader and the daemon share one shape.

## The command

The same binary is the command. Every word but `serve` is a verb sent to
the running daemon over the control socket, and the answer is printed one
line a fact; `--json` prints the answer as it came, for a script.

```sh
coderos-camera status
coderos-camera outputs
coderos-camera record start ~/Videos/clip.mp4
coderos-camera record stop
coderos-camera hands on
coderos-camera hands off
coderos-camera stop
coderos-camera --json outputs | jq '.outputs[] | select(.name == "loopback") | .state'
```

The exit code is 0 for an answer, 1 for a refusal or a daemon that did not
answer, and 2 for words that are not a verb.

## The grant and the receipt

The host writes `/etc/coderos/camera.json` from `coderos.desktop.camera`
([`os/modules/coderos/camera.nix`](../../os/modules/coderos/camera.nix)):
the node, the size, the frame rate, and the loopback node or `null`.
`CODEROS_CAMERA_GRANT` names another file, and a checkout with no file runs
on the defaults, `/dev/video0` at 1280x720 and 30 frames a second with no
loopback. The session's own variables win over the file, so a run by hand
can point the daemon elsewhere: `CODEROS_CAMERA_DEVICE`,
`CODEROS_CAMERA_CAPTURE` as `WxH`, `CODEROS_CAMERA_FPS`, and
`CODEROS_CAMERA_LOOPBACK`, where an empty value means no loopback.

A recording leaves the receipt `screen-record` leaves. `record stop`
closes the pipe, waits for `ffmpeg` to finish the container, reads the
length and the frame count back with `ffprobe`, writes them into the file's
`comment` tag through a copy remux (`coderos: camera video 12.3s, 370
frames at 30 fps constant`), and answers the same facts on the socket. The
command prints `Recording saved: <path> (12.3s, 370 frames at 30 fps)`.

## The model

The landmark model is data, so it stays out of the repository and out of
the Nix store: [`crates/coder-hands/model.toml`](../../crates/coder-hands/model.toml)
pins its URL and its SHA-256, the first `hands on` fetches it with `curl`
to `~/.openagents/quest/models/hand_landmarker.onnx`, and the tracker
checks the digest before it loads the file. `CODER_QUEST_HAND_MODEL` names
a model file of your own, which skips the pin. The directory and the
variable keep the names they had when a second tracker shared the cache.

## On a CoderOS host

`os/pkgs/coderos-camera.nix` builds the daemon with the static ONNX
Runtime fetched by digest and the V4L2 bindings generated under
`bindgenHook`; `nix build ./os#coderos-camera`
from the repository root builds the same package by hand. With
`coderos.desktop.camera.enable` on, the module puts the daemon on the
session's `PATH`, writes the grant, loads `v4l2loopback` at boot with
`exclusive_caps=1` and a fixed `video_nr`, exports `CODEROS_CAMERA_DEVICE`,
`CODEROS_CAMERA_CAPTURE`, and `CODEROS_CAMERA_LOOPBACK` to the session,
and adds `coderos-camera serve` to `coderos.desktop.start` ahead of
`camera-overlay`, so the daemon is up before the circle opens the loopback.
`camera-overlay` waits for `outputs` to report the loopback up before it
opens the player, and `camera-toggle` starts a daemon when `status` gets no
answer.

Super+C toggles the circle's window: closing the circle hands nothing
over, because the daemon keeps the camera.

## Tests

```sh
cargo test -p coder-hands -p coderos-camera
cargo clippy -p coder-hands -p coderos-camera --all-targets -- -D warnings
os/tests/coderos-camera.sh
```

The crate tests cover the fan-out with a fake source into fake sinks,
including a slow sink that drops frames while a fast one sees every frame;
the control verbs, parsed from words and answered over a socket; the
manifest and the digest check; the pixel conversions; the encoder and tag
arguments; and the JSON line. The shell test covers `camera-overlay`
reading the loopback as YUYV and waiting for the daemon, `camera-toggle`
asking the daemon, and the option rendering, which it evaluates with
`nix-instantiate` on a host that has one.
