# Aethyrforge for Mac (experimental)

An AppKit shell around the website's Everglade 5e WebGPU engine in WKWebView,
with its own local room, page server and local Parakeet voice service. Requires an Apple Silicon Mac running
macOS 26 or later, Xcode and the existing Coax web build toolchain.

```sh
coax/build.sh
apps/aethyrforge-desktop/build.sh --open
```

The build tool is Coax. It builds the local stack when its source closure is newer,
then bundles the room, static server, engine, browser host, shaders and assets.
It ad-hoc signs the app and, with `--open`, copies it to
`~/.openagents/aethyrforge/Aethyrforge.app` and opens that copy. It installs the
shared CLI and its dictation/Parakeet helpers in `~/.openagents/bin/`, retaining
older CLI versions under `~/.openagents/versions/`. The app works
independently of the checkout. No installation to `/Applications` or web deploy.

The app opens `/worlds/everglade?forge&room=pair-aethyrforge&relay=ws://localhost:PORT#gate`
with EDIT on. The existing named playtest-room profile takes a default character
and skips the first-visit character picker that would otherwise consume EDIT's
keys while hidden behind its panels. Its servers bind only `127.0.0.1`, on two free ports, and stop on app quit or
when either server fails. A lock prevents two instances writing the same world.
View → Reload Everglade reloads the page. Closing the window quits the app.
The relay uses `localhost`, the existing room client's required local override,
so the WebSocket connects straight to the room and leaves the HTTP page proxy
free to serve saves.

Every launch shows setup over the running Everglade scene. It checks the shared
CLI, lists running local agent engines and lets you keep a preferred engine,
shows the installed voice/edit models without hashing their contents. Running
processes are observations, not attached agents;
selecting one saves a preference. View → Setup… (Cmd+,) opens the pane again.
Refresh repeats discovery; only pressing Verify voice model hashes the pinned
complete weight/tokenizer files. No model is loaded
and no microphone or agent session starts during setup.

The app's files live in `~/.openagents/aethyrforge/`. `setup.json` keeps the
preferred engine and audio-model selection. The local world is `everglade/`:

- `world-everglade.json`: edits saved by the room; retained across launches.
- `room-control.txt`: the local room's existing development controls.
- `room.log`, `serve.log`: the current launch's server output.
- `local-url.txt`: the current local page URL, for headless inspection.
- `voice-host.log`, `voice-access.log`: local voice service output and permission/
  phase diagnostics. The access log contains no transcripts or audio.

On the first launch only, the app copies the repository's development world
snapshot and controls there. Existing files are preserved. It does not fetch the
published world. The existing editor's `1`–`5` keys place things, clicking selects
them, the arrows move, `R` rotates, `+`/`-` resize, Delete removes, and `Z`/`Y`
undo/redo during the session. Saving settles through the local room; this path's
word “publish” means the local document only. The undo stack is not durable yet.

Audio output is muted. Leave EDIT with DONE/B, then hold TALK/V while speaking.
The first request asks for macOS microphone permission and starts the local
Parakeet service on a reserved loopback port. If the permission dialog interrupted
the hold, hold TALK again afterward. Recognition uses the shared checkpoint on
Metal; the model stays resident until app quit. No model loads during setup.
The bundled browser voice engine receives real microphone samples and returns
the final words to Everglade's existing voice UI, without the public voice host.
Release closes capture; setup, focus loss, hiding and quit cancel it. A permission
request that finishes after cancellation cannot reopen capture or a voice socket.

Local gpt-oss interpretation, full voice-edit acceptance, typed instructions,
remote publish/revert, bots and raid controls are future milestones in the
[desktop spec](../../docs/verse/2026-11-04-aethyrforge-desktop.md).

On this Mac, the original world directory is moved from Application Support on
the first upgraded launch, without replacing an existing destination. Models are
shared with the CLI, Coder and the voice host under `~/.openagents/models/`:

```sh
openagents coder agents --json
coder/tui/coder.sh agents --json
openagents models ls --json
openagents models verify parakeet --json
openagents models migrate --json
```

Migration moves legacy Parakeet, Whisper and original gpt-oss directories and
leaves compatibility symlinks, without duplicating weights. Distinct existing
copies refuse migration. `OPENAGENTS_MODEL_CACHE` selects another shared model
root; the existing model-specific overrides still work. The inventory's
`present`, `verified` and `loaded` fields describe separate facts. gpt-oss is
listed as present only; its verification/loading is future work.

The native smoke check creates a hidden WKWebView and an isolated world next to
the report. It waits for a real WebGPU scene, places a thing through the editor's
keyboard path, checks the saved document, captures a frame, and quits both servers.
Run it again in the same directory to check the saved world survives reopening:

```sh
mkdir -p /tmp/aethyrforge-smoke
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --smoke-test /tmp/aethyrforge-smoke/first.json
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --smoke-test /tmp/aethyrforge-smoke/reopened.json
```

The setup smoke check exercises the real bundled CLI, process inventory and
shared-model presence without hash verification, then enters the world, saves settings and reopens
setup over WebGPU. It uses an isolated world beside its report:

```sh
mkdir -p /tmp/aethyrforge-setup
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --setup-smoke /tmp/aethyrforge-setup/setup.json
```

Reports include the first-frame time and child PIDs; matching PNGs show the frame.
Use a fresh directory for a new edit test. This is milestone one's shell/save
check, not the future voice acceptance suite.

The hidden voice check plays the committed fixture through a synthetic media
stream, the actual WKWebView AudioWorklet and voice Wasm, the loopback voice
socket and resident Parakeet on Metal. It requires the shared model, opens no
physical microphone, keeps output muted, checks cancellation of a delayed capture
request, and stops all three servers:

```sh
mkdir -p /tmp/aethyrforge-voice
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --voice-smoke /tmp/aethyrforge-voice/voice.json
```

The Coax `voice-build.coax` packages the existing voice engine and a Parakeet-only
native voice host using serial compilation. It adds no inference or protocol logic.
