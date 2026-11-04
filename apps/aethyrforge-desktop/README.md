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

The Metal HUD view covers only the bottom composer; both native overlay views
are explicitly transparent, so WebKit can keep rendering the world. Startup's
60-second budget counts visible rendering time, pausing while the window or
page is hidden. A genuine startup/server failure shows its error, stops the
spinner and provides Retry startup. Retrying replaces only this app's local
processes and preserves the saved world.

The Coder HUD uses `aethyr/ui/draw.coax` for a compact, lightly tinted 88-point
composer and shared layout, a native Metal overlay and AppKit text/IME. There
is no frame or label around the whole window. Coder is hidden at rest. Enter
or Cmd+L opens its composer and conversation; native Return/keypad Enter never
opens the realm's region chat. Enter sends and closes the composer; Shift-Enter
inserts a line. Escape closes it while retaining the draft and any running
agent. Automatic routing status and the working folder sit above the bottom
input; there are no agent/model/effort dropdowns, including in setup. Stop
cancels a run. When closed,
only received reply text appears on the left during an active run, with no
background, heading, controls or mouse interception. Replies remain visible
for five seconds after the run ends, then hide; Enter restores the
conversation, including tools, results, paths and errors.
Pending tool requests show a message to press Enter for Allow once / Deny;
closing the composer never grants approval. New chat and folder changes start fresh.
Slash commands, history browsing and attachment to a running PID are future work.

The UI calls shared `~/.openagents/bin/openagents coder run|fork --engine auto --stream-json --ask`,
the same Coder as the TUI. Every submitted turn routes, including follow-ups,
which carry the previous durable session's
context. Prebuilt Coder/Microcoder are bundled; messages do not build tools or
need a checkout. Provider CLIs use their existing logins. Coder's existing typed
System One router asks agent, difficulty and effort. Coder's own eligibility
selects installed Claude Code/Codex, bundled Microcoder, and Psionic when a local
text endpoint is configured; cooldowns and `CODER_ROUTE_ENGINES` constrain dispatch.
Claude uses Haiku for trivial work, Sonnet for easy/moderate
work and Opus for hard work; Codex retains its configured model with routed
reasoning effort. The resolved model is reported by the agent. This is an initial
profile, checked with finite probes rather than a new accuracy benchmark.

The app creates shared `~/.openagents/coder/router.json` only if absent:

```json
{"schema":"coder.router.v1","url":"http://127.0.0.1:8793/v1/systemone","remote":false}
```

This reuses the resident local Elephant on this Mac; the app neither downloads
weights nor starts another inference process. `PSIONIC_SYSTEMONE_URL` overrides
the door. A Pylon-backed System One fleet/proxy can be configured with `remote:true`
(including a loopback relay proxy); remote HTTPS doors may use
`PSIONIC_SYSTEMONE_KEY` from the environment. Remote requests retain Coder's
`system-one` session-consent gate. The raw public NIP-98 endpoint is not a bearer
HTTP substitute. No live Pylon was advertised in this session's CLI discovery;
the shipped default uses the verified local server.

Router calls have an eight-second deadline and receive at most 768 UTF-8 bytes
of the current task plus 256 bytes of recent conversation. Workers receive the
full prompt/history. Missing, malformed or uncertain decisions use Coder's
eligible default, with the reason recorded visibly in the conversation; they
never switch to hosted Mammoth behind the user's back. Selected agent/profile,
complexity and effort are recorded as `coder.route.v2` JSONL, separately from
the unchanged legacy route format and System One network protocol.

`chat.json` stores automatic routing and the folder under `~/.openagents/aethyrforge/`;
the initial folder is `workspace/` there. Complete histories stay under shared
`~/.openagents/coder/sessions/`. The view is bounded; reopening starts fresh.

The app's files live in `~/.openagents/aethyrforge/`. `setup.json` keeps the
audio-model setup (old preferred-agent fields are ignored). The local world is `everglade/`:

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
Parakeet service on a reserved loopback port. macOS permission is checked
separately from model startup; system dialogs taking key focus do not turn an
existing grant into a cached WebKit denial. Every hold prepares its own native
capture ticket, including when WebKit reuses its site grant. If the permission
dialog interrupted the hold, hold TALK again afterward. Recognition uses the shared checkpoint on
Metal; the model stays resident until app quit. No model loads during setup.
The bundled browser voice engine receives real microphone samples and returns
the final words to Everglade's existing voice UI, without the public voice host.
Release closes capture; setup, leaving the app, hiding and quit cancel it. A permission
request that finishes after cancellation cannot reopen capture or a voice socket.

The native `--microphone-check REPORT.json` acceptance mode opens the real
microphone through WebKit, immediately stops each track without recording or
sending audio, and exits. It uses an isolated world beside the report and the
existing macOS grant. It checks a visible foreground window without key focus,
repeated captures with fresh tickets, recovery after a model startup failure
and cancellation during preparation. On 2026-11-04 these checks passed with
`authorization=3`, `key=false` and one WebKit permission callback across three
captures. Both Coax permission checks passed and all four policy mutants were
killed. The complete desktop suite now has 137 passing checks.
The recorded-audio voice check also passed through the new per-hold preparation,
Parakeet, Elephant routing and two live Microcoder reply chunks with the bar
closed. The [capture IPC contract](../../docs/protocol/aethyrforge-microphone-capture-v1.md)
records the permission/preparation boundary.

Local gpt-oss interpretation, full voice-edit acceptance, typed world edits,
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

The chat smoke uses the native composer and real packaged/shared CLI without
opening a window or starting the world. It checks two replies and context.
`FORGE_CHAT_SMOKE_ENGINE` selects the agent (default Echo). Optional actions
check approval of a file write in the isolated workspace and cancellation:

```sh
mkdir -p /tmp/aethyrforge-chat
FORGE_CHAT_SMOKE_ENGINE=codex FORGE_CHAT_SMOKE_ACTIONS=1 \
  ~/.openagents/aethyrforge/Aethyrforge.app/Contents/MacOS/Aethyrforge \
  --chat-smoke /tmp/aethyrforge-chat/result.json
```

Use `FORGE_CHAT_SMOKE_ENGINE=auto` for two actual routed turns; copy the shared
`router.json` into `<report-directory>/.openagents/coder/` first. The check
requires a successful Elephant route record on both turns as well as the reply
and retained context. Native input smoke also checks that setup/composer contain
no agent/model dropdowns. Verification: 131 desktop checks, 432 Coder checks,
88/88 router-policy mutants killed, native input and automatic two-turn chats.
The repository tools check reports 27 pre-existing missing-header files outside
this change.

The input smoke sends AppKit Return/keypad Enter through the actual window over
the packaged WebKit realm. It checks held-key repeats, Shift-Enter, draft retention,
approval review, active-message visibility, zero Enter events in region chat and
an Enter-submitted offline Coder CLI round trip. It also checks that completed
replies stay visible for five seconds with the composer closed (measured 5.03
seconds on 2026-11-04). All 27 presentation mutants were killed:

```sh
mkdir -p /tmp/aethyrforge-input
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge \
  --input-smoke /tmp/aethyrforge-input/result.json
```

To check the real visible launch, without the hidden smoke's frame pump:

```sh
~/.openagents/aethyrforge/Aethyrforge.app/Contents/MacOS/Aethyrforge \
  --launch-check /tmp/aethyrforge-launch.json
```

This opens the normal local world and records actual animation callbacks,
WebKit visibility/readiness and whether setup enabled entry. It temporarily
keeps the test window visible, enters the world on success and restores its
normal window level. Add `--launch-check-background` to minimize it for 65 seconds
before showing it, and `--launch-check-retry` to terminate its own room once
and exercise the real Retry button. These checks preserve the world document;
they do not synthesize animation frames or override page visibility.

Finalized Parakeet speech starts the same routed Coder turn directly. It keeps
the Enter bar closed, preserves any typed draft and shows the recognized words
and live reply chunks on the left. It does not also send those words to the
realm's editor. Canceled captures, partials and duplicate finals never dispatch;
if Coder is busy, a message says the new voice input was not sent. Tool consent
still waits for Enter and Allow once / Deny. Speech recognition stays local;
the finalized text goes to the coding provider selected by the router.

`--stream-json` adds opt-in live stdout deltas; ordinary `--json` and durable
whole-reply logs stay unchanged. Known credentials are scrubbed before live
encoding, including values split across chunks. See the
[stream and voice IPC contract](../../docs/protocol/coder-reply-stream-v1.md).

The hidden voice check plays the committed fixture through a synthetic media
stream, the actual WKWebView AudioWorklet and voice Wasm, the loopback voice
socket and resident Parakeet on Metal. It requires the shared model, opens no
physical microphone, keeps output muted, checks cancellation of a delayed capture,
then requires an actual routed Coder reply streamed with the bar closed, a
preserved typed draft, canonical durable history and duplicate-final rejection.
It stops all three servers:

```sh
mkdir -p /tmp/aethyrforge-voice
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --voice-smoke /tmp/aethyrforge-voice/voice.json
```

Verified on 2026-11-04: the recorded “Hello Foreman…” final reached Elephant
routing and Microcoder, then delivered live reply output with the bar closed.
The fixture denied one unnecessary tool request; production tool consent stays
manual. The typed draft survived and the duplicate final was ignored. 435 Coder
checks and 135 desktop checks passed; voice admission killed all 74 mutants,
and live credential-prefix handling killed all 31. Repository tools-check
reports 28 unrelated existing missing-header files at this base.

The Coax `voice-build.coax` packages the existing voice engine and a Parakeet-only
native voice host using serial compilation. It adds no inference or protocol logic.
