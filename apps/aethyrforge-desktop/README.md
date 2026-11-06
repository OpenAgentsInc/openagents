# Aethyrforge desktop (experimental)

The four-host Everglade performance matrix, immutable sixteen-bot siege,
measurement scopes and one-command rerun are in
[`docs/verification/everglade-hosts-2026-11-05/`](../../docs/verification/everglade-hosts-2026-11-05/README.md).
The visible muted SDK check uses `--frame-profile REPORT.json --frame-benchmark`;
`--bench-no-timestamps` selects the control, and `--bench-probe` is a short
transport/counter diagnostic, not a qualified performance sample.

## Linux: native Wayland and Vulkan

On Linux the same build command produces a native desktop executable with
Bitcoin Lab loaded. Nix supplies the C SDKs; it does not rebuild the system.
The window runs the existing Bitcoin Lab Coax producer, its Coax-generated
SPIR-V, Aethyr's native Vulkan pass host and the shared desktop Coder HUD.
There is no browser, page server or JavaScript in the Linux runtime.

```sh
apps/aethyrforge-desktop/build.sh --open
# Later, use the Aethyrforge desktop launcher, or:
~/.openagents/bin/aethyrforge
```

The build installs native images in `~/.openagents/bin/`, renderer files in
`~/.openagents/aethyrforge/native-linux/`, and a desktop launcher backed by
`openagents-aethyrforge.service`. Builds replace executable images atomically;
an existing window can finish using its current image. The installed app runs
independently of its source checkout. Existing general CLI installations,
provider logins, models and Mac settings/worlds are preserved.

Enter opens Coder and releases held walking keys. Enter sends; Shift-Enter
adds a line; Escape closes the composer. Stop cancels this window's own Coder
child. New starts a new conversation, Reload resets Bitcoin Lab, and Quit
closes the app and its child. Subsequent turns fork the window's own durable
Coder session. The default workspace is
`~/.openagents/aethyrforge/workspace/`; `AETHYRFORGE_WORKSPACE` overrides it.
Coder uses the existing automatic router/provider logins. No model or
microphone starts when the window opens.

This initial port uses the native host's existing US keyboard mapping and
pointer input. Native clipboard/IME, microphone setup and Everglade's editing
service have not been ported. Bitcoin Lab remains the recorded, validated
regtest replay with selectable block slabs and transaction/output navigation;
it does not connect to a live Bitcoin node. Its native renderer uses the
existing legacy pass host, without claiming canonical renderer adoption.

Focused checks:

```sh
coax/bin/coax test apps/aethyrforge-desktop/linux-native.coax --only 'native Aethyrforge'
# A fresh directory isolates Coder's logs/router/memory from the owner's:
CODER_HOME=/tmp/aethyrforge-coder-check AETHYRFORGE_ENGINE=echo \
  coax/web/tool.sh apps/aethyrforge-desktop/linux-native-check.coax
# Actual offscreen Vulkan; --keys opens the real Coax composer:
~/.openagents/bin/aethyrforge-native ~/.openagents/aethyrforge/native-linux - \
  --headless 90 --size 1440x900 --keys 2:-2:1,3:-2:0
```

The admission checks and zero-survivor mutation gate are in
`linux-policy.coax`. The request envelope stays inside this process's Coax
state; it is not a command endpoint. The native bridge does scalar ABI and
window/GPU effects only. Application layout, draft state, input ownership,
process launch/stop decisions and Coder event interpretation stay in Coax.

## Mac: AppKit and Metal

An AppKit shell around the shared Everglade 5e Coax engine, with a native Metal
world, web preview, local room, page server and local Parakeet voice service. Requires an Apple Silicon Mac running
macOS 26 or later, Xcode and the existing Coax web build toolchain.

The default (or `--world metal`) selects the native AppKit/CAMetalLayer realm host with the
same Coax Everglade producer and renderer. It uses the shared canonical assets,
Coax audio mixer and native room. Its [Mac qualification](../../docs/verification/metal-host-2026-11-05/current-85c-final.md)
records actual 120 Hz gameplay, frame parity, input, session reload and GPU
recovery. The [current three-host qualification](../../docs/verification/host-contract-2026-11-06/current/README.md)
records the common v1 migration and current admitted compiler/source inputs.
`--world web` selects WebKit. **View → Check as Players See It** opens the
same local world in the web view; **View → Return to Native World** returns.
Each action closes the previous realm before loading the replacement. The
[actual menu round-trip record](../../docs/verification/metal-host-2026-11-05/web-preview.md)
retains frame, room-seat and resource-release observations.

Offline `local:` sockets use that native room process through the same text
messages as a network room. The loopback-only `--native-local` room option admits
one ticketless `everglade/local-1` seat; ordinary rooms still require tickets.
Launch the app executable with `--offline` to select this one-seat mode.
The browser still runs the same simulation in its Web Worker. For a visible,
muted comparison without microphone/model preparation, run the app executable
with `--offline-check /tmp/forge-native.json`, then
`--offline-check /tmp/forge-worker.json --offline-worker`. Each check uses an
isolated world, verifies seat zero and room delivery, counts actual Workers,
and records 15 seconds of tick observations. Native vitals are 10-second
histograms with 10 microsecond bins; Worker samples include frame delivery
and are quantiles of individual ticks over each second.

Run both with one command after building the app:

```sh
coax/web/tool.sh apps/aethyrforge-desktop/offline-check.coax \
  "$PWD/apps/aethyrforge-desktop/build/Aethyrforge.app" /tmp/forge-offline
```

The simulation comparison uses the existing drift harness and tolerances:
`coax/web/tool.sh everglade-drift --offline native --profiles local --no-raid`.

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

The app opens `/worlds/everglade?forge=play&room=pair-aethyrforge&relay=ws://localhost:PORT#gate`
in play mode with the editor closed. B/EDIT opens the builder when requested;
launch and reload leave it closed. The existing named playtest-room profile takes a default character
and skips the first-visit character picker that would otherwise consume EDIT's
keys while hidden behind its panels. Its servers bind only `127.0.0.1`, on two free ports, and stop on app quit or
when either server fails. A lock prevents two instances writing the same world.
View → Reload Everglade reloads the page. Closing the window quits the app.
After leaving EDIT, Shift-drag one hotbar spell onto another to swap their
positions and number keys. This uses Everglade's shared bar code, including
level-based room kits. The saved arrangement describes the full bar; the current
kit applies those swaps among its available spells, with Repair fixed.
The relay uses `localhost`, the existing room client's required local override,
so the WebSocket connects straight to the room and leaves the HTTP page proxy
free to serve saves.

The first launch shows setup over the Everglade scene. After Open Everglade
saves setup, later launches go directly into the world, including installations
with an existing valid setup file. Tool/model discovery runs in the background.
Setup checks the shared
CLI, lists running local agent engines and lets you keep a preferred engine,
shows the installed voice/edit models without hashing their contents. Running
processes are observations, not attached agents;
selecting one saves a preference. View → Setup… (Cmd+,) opens the pane again.
Refresh repeats discovery; only pressing Verify voice model hashes the pinned
complete weight/tokenizer files. After microphone approval, startup prepares
Parakeet and the local voice connection in the background. No microphone stream
or coding-agent session starts during setup.

Setup automatically requests macOS microphone permission while its window is
visible and the app is active. Its microphone row shows Allowed, a pending
request, or Open Settings after denial. Permission is read from macOS each time;
it is not inferred from saved setup. Existing completed installations still
skip the setup screen and preflight missing permission at foreground launch.
The permission request opens no microphone stream. Once approved, the separate
Coax voice-readiness gate admits resident preparation: the local Parakeet host,
voice Wasm, a suspended AudioWorklet graph and an idle local HELLO/WELCOME socket.
Setup shows Preparing local voice, then Voice ready. The same preparation runs
at startup when setup is already saved. The microphone opens only on V/TALK.
V/TALK rechecks the grant and shares an in-flight setup request if necessary.
Automated setup tests use a fake permission transport and do not show a system
sheet; the explicit microphone acceptance check exercises the real WebKit path.

The setup permission checks passed on 2026-11-04: 78 setup/JSON checks, three
permission checks and all 12 permission-policy mutants killed. The hidden native
setup check simulated first approval plus a concurrent capture permission check:
one system request, ready status before voice, and Settings after denial, without
opening a microphone or model. The real WebKit acceptance also recorded
`authorizedBeforeCapture:true`, three closed captures, fresh tickets and successful
cancellation/startup-failure recovery. Its passing capture was run under LLDB;
two earlier direct runs ended without reports and a separate Launch Services run
reported a JavaScript exception. These diagnostic artifacts are retained under
`/tmp/forge-setup-microphone/`; the passing report is `microphone-debug/result.json`.

The Metal HUD views cover only the bottom composer and the floating message
component; both are explicitly transparent, so WebKit keeps rendering the world. Startup's
60-second budget counts visible rendering time, pausing while the window or
page is hidden. A genuine startup/server failure shows its error, stops the
spinner and provides Retry startup. Retrying replaces only this app's local
processes and preserves the saved world.

On a ProMotion display the shell disables WebKit's preference for updates near
60 Hz, using its feature API only when that feature and its selectors exist.
Otherwise it keeps the 60 Hz preference. This changes this app's WebKit instance.
The native display rate reaches Coax's AUTO governor through page facts; desktop
AUTO targets that rate and has a 60 FPS minimum quality budget. It can reduce
rendering quality under load without adopting the web/phone governor's 30 FPS
budget. Actual throughput still depends on the scene and available GPU time.

`aethyr/frame-pacing.coax` admits at most two unfinished GPU frames in Everglade's
legacy WebGPU host. When both are outstanding, the host waits for a queue fence
before doing more simulation/encoding work; the next admitted frame receives the
elapsed time. Device recovery owns a fresh pending count. The HUD retains its
vertex array instead of allocating/zeroing its 8 MB feed capacity for each draw.

For a real, visible WKWebView timing capture, quit the app first, then run:

```sh
~/.openagents/aethyrforge/Aethyrforge.app/Contents/MacOS/Aethyrforge \
  --frame-profile /tmp/aethyrforge-frames.json
coax/web/tool.sh apps/aethyrforge-desktop/frame-profile.coax /tmp/aethyrforge-frames.json
```

After scene readiness and a three-second warmup, this writes six five-second
windows. Wait for all six before summarizing. Add `--frame-profile-walk` to
alternate real W/S key events every 2.5 seconds during capture. Ordinary launches
do not collect traces. The report separates animation callbacks from GPU
submissions and reports CPU work, queue completion round trips and pending work;
it does not claim to measure display presentation timestamps. Measurements and
checks are recorded in
[the performance verification](../../docs/verification/aethyrforge-frame-pacing-2026-11-04.md).

The Coder HUD uses `aethyr/ui/draw.coax` for a compact, lightly tinted 88-point
composer and shared layout. Floating messages use `aethyr/ui/feed.coax` and
`feed-draw.coax`: Coax owns the bounded UTF-8 ring, wrapping, newest visible rows
and five-second completion lifetime; Aethyr's font/stroke components emit their
Metal vertices. Swift supplies clock observations and submits those buffers.
The floating messages have no AppKit text view or Swift deadline/hide task.
AppKit still supplies editable input/IME and selectable conversation history. There
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
Desktop turns use Coder's unattended provider mode, without `--ask`: Claude's
`bypassPermissions` and Codex's `approvalPolicy=never` with `danger-full-access`.
The Codex adapter uses the app-server equivalent of its
[documented unattended mode](https://learn.chatgpt.com/docs/developer-commands?surface=cli).
Microcoder uses its existing ungated tool path. Native Allow once / Deny controls
remain available for an explicitly constrained backend, but normal desktop
turns do not request tool approval. New chat and folder changes start fresh.
Slash commands, history browsing and attachment to a running PID are future work.

The UI calls shared `~/.openagents/bin/openagents coder run|fork --engine auto --stream-json`,
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
- `voice-host.log`, `voice-host.previous.log`, `voice-access.log`: current/previous
  local voice service output and timestamped permission/capture-phase diagnostics,
  including release, cancellation reason and frame counts. The access log contains
  no transcripts or audio.

On the first launch only, the app copies the repository's development world
snapshot and controls there. Existing files are preserved. It does not fetch the
published world. The existing editor's `1`–`5` keys place things, clicking selects
them, the arrows move, `R` rotates, `+`/`-` resize, Delete removes, and `Z`/`Y`
undo/redo during the session. Saving settles through the local room; this path's
word “publish” means the local document only. The undo stack is not durable yet.

Setup and **View → Status…** use the shared native Zeron Metal renderer; their
layout, labels and readiness stay in Coax. After the first real scene frame,
the world view pauses while either panel covers it, or the app is hidden,
minimized or occluded. Its native room and socket delivery keep running.
The panels render only on changes. Returning to play resumes the same loop;
Reload obtains a fresh first frame before pausing a covered world again.

The visible, muted acceptance run uses an isolated world, no physical microphone
and no model load. It measures CPU and GPU effects, captures both panels and
checks setup/status, reload, minimize, hide and resume:

```sh
mkdir -p /tmp/aethyrforge-setup-profile
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge \
  --setup-profile /tmp/aethyrforge-setup-profile/result.json
```

Normal launches play Everglade's ambience, footsteps and effects through its
usual SOUND volume/mute control. Audio starts with the first click, tap or key
in the world. Automated smoke, launch, microphone and frame-profile checks keep
a live zero-gain output so they never play through the owner's speakers.
Footstep bank gain is 0.08 (previously 0.22); the synthesized fallback is 0.10
(previously 0.30), about 9 dB quieter. Other sound levels retain their settings.

The native audio regression check opens hidden, isolated WebKit views. It checks
that normal configuration leaves the realm's speaker route alone, then measures
the actual Everglade mix before the test's zero-gain sink:

```sh
mkdir -p /tmp/aethyrforge-audio
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge \
  --audio-smoke /tmp/aethyrforge-audio/result.json
```

This reproduced the unconditional mute before the fix. Afterward it observed a
running AudioContext, one Coax effect cue, the `ambient-e1` bed and a nonzero
peak of 0.312 with test output gain 0. No microphone was requested. Logs and
before/after reports are retained in `/tmp/forge-audio/` (2026-11-04).

Hold TALK/V while speaking in play mode. If you opened EDIT, close it with DONE/B first.
Permission and the local voice connection should already be ready from startup.
Each hold rechecks macOS permission and reuses the prepared worklet and connection;
if permission or preparation is still missing, the hold retries them. Consent is checked
separately from model startup; system dialogs taking key focus do not turn an
existing grant into a cached WebKit denial. Every hold prepares its own native
capture ticket, including when WebKit reuses its site grant. If the permission
dialog interrupted the hold, hold TALK again afterward. Recognition uses the shared checkpoint on
Metal; the model stays resident until app quit. On release/cancel the microphone
tracks stop and their source disconnects. After a completed/canceled capture,
the client establishes a fresh idle connection and suspends its graph, ready
for the next hold. Page exit closes the graph/socket. Ordinary web voice remains
on demand; automatic preparation is restricted to the desktop's local host.

Held capture uses the shared Coax engine's explicit push-to-talk mode. Silence
and quiet speech do not end it while V/TALK remains held. Release closes the mic
immediately, drains queued worklet PCM, then sends one end-of-turn. The configured
turn cap and cancellation still apply. Other clients retain automatic VAD turns
after engine init. Warmup primes one silent audio quantum before suspending;
it never opens the microphone.

The hidden `--voice-warm-smoke REPORT.json` check uses synthetic media and the
real resident Parakeet host. It verifies no microphone request during warmup,
an idle suspended graph, no startup audio frames, three prepared holds without
new sockets or CONNECTING (including the actual V-key path), two consecutive
recorded-fixture transcriptions, late-capture
cancellation, recovery without a duplicate reconnect and the unprepared path.
It never opens the physical microphone or calls a coding provider. Local check
artifacts live under `/tmp/forge-voice-warm/`.
The final clean-host check (`isolated2/result.json`, 2026-11-04) prepared in
1.248 seconds before capture; V reached listening in 26.86 ms and the two direct
repeat holds in 0.50/0.48 ms. No warmup microphone request or audio frames were
sent. Both speech turns returned “Hello Foreman, what are we building today?”;
recognition completed 392/198 ms after release. Cancellation, reconnect and the
ordinary unprepared client passed. Earlier exploratory runs against the previous
bundled host intermittently lacked a final transcript; the final acceptance uses
a freshly built host and checks both turns. Synthetic media exercises the real
worklet, wire and GPU recognizer; these timings exclude physical device acquisition.
The bundled browser voice engine receives real microphone samples and returns
the final words to Everglade's existing voice UI, without the public voice host.
Release closes capture; setup, leaving the app, hiding and quit cancel it. A permission
request that finishes after cancellation cannot reopen capture or a voice socket.

Held-capture regression (2026-11-04): `/tmp/forge-voice-hold/native6/result.json`
passed a one-second quiet onset, “Add two giant crates,” a 2.4-second pause and
the existing Foreman fixture in one 8.97-second buffer. No final arrived during
the pause. Release returned “Add two giant crates. Hello Foreman, what are we
building today?” and a second capture returned the complete Foreman sentence.
The crates WAV was synthesized locally with macOS `say` and converted with
`afconvert` to mono 16 kHz PCM; no owner microphone recording is committed.
Prepared V/direct holds measured 138.86/0.52/0.66 ms with no new sockets or
CONNECTING; the hidden harness pumps both Coax outbox frames and observes
capture-promise completion rather than a throttled polling timer. Cancellation,
reconnection and the cold path passed. All 45 voice and 36 mix checks passed;
the C bridge passed ASan/UBSan at 16/24/44.1/48 kHz, including a three-second
held pause and the existing automatic-turn regressions. The full Everglade Wasm
build passed its checks and size gate. Repository tools-check retains 29
unrelated missing-reason headers.
The rebuilt-world audio check (`audio/result.json` in the same directory)
observed ambience/effect PCM with normal speaker routing and a muted test sink.
The real routed voice-to-Coder check (`coder/result.json`) also passed: complete
Foreman transcript, live reply with the bar closed, preserved draft and duplicate
rejection. Those tests used isolated worlds and left the owner's document alone.

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
remote publish/revert and bots are future milestones in the
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
shared-model presence without hash verification, then enters the world, saves
settings and opens setup manually over WebGPU. A second launch in the same
directory checks direct entry (`setupSkippedOnLaunch:true`) and manual setup
refresh. It uses an isolated world beside its report:

```sh
mkdir -p /tmp/aethyrforge-setup
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --setup-smoke /tmp/aethyrforge-setup/first.json
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --setup-smoke /tmp/aethyrforge-setup/reopened.json
```

Reports include the first-frame time and child PIDs; matching PNGs show the frame.
`--hotbar-smoke REPORT.json` uses an isolated world and hidden WKWebView to
Shift-drag two actual room-kit slots, check their rendered bounds, press their
new number keys, cancel a drag and reload the page to check persistence. It uses
the normal DOM input listeners and read-only Coax hotbar probes, with audio muted.
It also requires the editor to be closed at launch and after reload, then checks
that B opens and closes it when requested.
The startup follow-up passed 85 session checks and the new `?forge=play` Coax
regression. Hidden desktop acceptance confirmed closed launch/reload, explicit
B control and the hotbar checks (`/tmp/forge-start-play/native/result.json`);
the rebuilt game is 2,033,959 bytes.
The 2026-11-04 check passed before and after reload: slots 21/22 exchanged
their rendered bounds, keys 1/2 selected 22/21, a cancelled drag changed nothing,
and the saved 47-slot permutation remained valid. Two scoped Coax checks cover
room-kit changes, hidden spells, offline layout, cancellation and fixed Repair;
the new room-kit regression failed against the previous implementation. Reports
and the frame are in `/tmp/forge-hotbar/native/`; the 2,033,168-byte game build
passed. `tools-check` reports the repository's 29 existing missing reason headers.
Use a fresh directory for a new edit test. This is milestone one's shell/save
check, not the future voice acceptance suite.

The routed conversation also controls the live local scene. Say or type
“add three crates” or “add a stack of five crates”; Coder uses the prebuilt Coax
scene tool to create saved catalogue objects, including player collision and NPC
avoidance. It can move or remove a whole group. Stacks include their base, and
rows leave walkable gaps. Placement admission and planning live in
`scene-policy.coax`; Swift observes player position and supplies the local
session address. See [the command contract](../../docs/protocol/aethyrforge-scene-v1.md).

The same tool works programmatically while the app is running:

```sh
~/.openagents/bin/aethyrforge-scene show
~/.openagents/bin/aethyrforge-scene apply '{"op":"place","item":"crate","count":3}'
~/.openagents/bin/aethyrforge-scene apply '{"op":"place","item":"crate","count":5,"arrangement":"stack"}'
```

NPCs use the same typed/voice Coder conversation. For example: “add three
cultists”, “add two villagers”, “have those cultists attack the stables”, or
“stop that group”. The tool supports cultists, villagers and guards, individual
or group attack/move/stop orders, and removal. Named townspeople, live players,
other NPCs/creatures and all 60 authored buildings are targetable. The room
simulates real movement, damage, deaths and building pieces breaking. Nearby
placement checks the actual world, including newly placed crates.

```sh
~/.openagents/bin/aethyrforge-scene npcs
~/.openagents/bin/aethyrforge-scene npcs people
~/.openagents/bin/aethyrforge-scene npcs buildings
~/.openagents/bin/aethyrforge-scene npc '{"op":"spawn","kind":"cultist","count":3}'
~/.openagents/bin/aethyrforge-scene npc '{"op":"order","group":1,"action":"attack","target_kind":"building","target_id":9}'
~/.openagents/bin/aethyrforge-scene npc '{"op":"order","group":1,"action":"stop"}'
```

NPC definitions and orders are saved separately in the local world's
`forge-npcs.json`; reopening starts a fresh playtest of that setup. Transient
HP and movement are not a saved combat session. A batch is 1–8 NPCs, with 32
authored NPCs total. See [NPC command v1](../../docs/protocol/aethyrforge-npcs-v1.md).
The hidden `--npc-smoke REPORT.json` check exercises real routed Coder spawning,
group attacks, damage, replicated building breaks, rendered creatures and
blocked spawns. It also routes event start/double/set/wave through real Coder,
checks the rendered army, atomic overflow refusal and event stop.
`--npc-reopen` checks NPC persistence and that transient events stay stopped.

The planned Cultist Raid is controllable through the same conversation: “start
the cultist raid”, “double the cultist army”, “add another wave of five”, or
“stop the raid”. It starts the real standing siege immediately, with Darian,
three ward stones and the watch. Doubling changes only the cult fighter target.

```sh
~/.openagents/bin/aethyrforge-scene raid
~/.openagents/bin/aethyrforge-scene raid '{"op":"start"}'
~/.openagents/bin/aethyrforge-scene raid '{"op":"double"}'
~/.openagents/bin/aethyrforge-scene raid '{"op":"set","count":10}'
~/.openagents/bin/aethyrforge-scene raid '{"op":"wave","count":5}'
~/.openagents/bin/aethyrforge-scene raid '{"op":"stop"}'
```

Cult fighter targets are 1–60, the watch stays at 24, and the complete target
must fit alongside existing NPCs/summons in the room's 96 slots. Overflow
refuses without changing the army. Army edits require the siege phase, before
the camp stand. Events are transient; reopening keeps authored NPC definitions
and starts with no event. See [event control v1](../../docs/protocol/aethyrforge-raid-v1.md).

The hidden scene check exercises programmatic placement, group move/removal,
individual resizing, atomic refusal, three real automatically routed Coder requests with the composer
closed, the page's world refresh, and persistence on a second launch. It uses
the installed provider login and local Elephant router, with an isolated world
and Coder history. The NPC regression walks around the exact planned footprints.

Existing objects can be resized individually: “make the middle crate 3× larger”
changes only its size. Read `show` for its current index and scene version, then:

```sh
~/.openagents/bin/aethyrforge-scene apply '{"op":"resize","index":13,"version":23,"scale_percent":300}'
```

Use the actual index/version from `show`. `scale_percent` multiplies the current
size; `size_percent` sets the percentage of catalogue default instead. Supply
one size field. The resulting size must be 25–400%. Stale selections, collisions
and bounds refuse unchanged; neighbours and group membership stay intact.
See [scene commands v2](../../docs/protocol/aethyrforge-scene-v2.md).

```sh
coax/bin/coax test apps/aethyrforge-desktop/scene-policy.coax
coax/bin/coax test apps/aethyrforge-desktop/scene-collision-checks.coax
mkdir -p /tmp/aethyrforge-scene
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --scene-smoke /tmp/aethyrforge-scene/result.json
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --scene-smoke /tmp/aethyrforge-scene/reopened.json --scene-reopen
```

The chat smoke uses the native composer and real packaged/shared CLI without
opening a window or starting the world. It checks two replies and context.
`FORGE_CHAT_SMOKE_ENGINE` selects the agent (default Echo). Optional actions
check an unattended file write in the isolated workspace and cancellation:

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
if Coder is busy, a message says the new voice input was not sent. Voice turns
use the same unattended provider mode as typed turns. Speech recognition stays local;
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
That earlier fixture denied one unnecessary tool request; the current desktop
uses unattended provider launches. The typed draft survived and the duplicate final was ignored. 435 Coder
checks and 135 desktop checks passed; voice admission killed all 74 mutants,
and live credential-prefix handling killed all 31. Repository tools-check
reports 28 unrelated existing missing-header files at this base.

HUD/unattended checkpoint, 2026-11-04: 142 desktop checks passed; the shared feed
killed all 72 mutants, presentation killed all 33 and launch options killed all
four. Native input exercised the Coax feed over the real realm, its five-second
lifetime and a Metal offscreen frame. Real Claude and Codex each retained
follow-up context, wrote the requested fixture file with zero approval requests
and stopped an active turn. The repository tools check still reports the same
28 unrelated missing headers.

The Coax `voice-build.coax` packages the existing voice engine and a Parakeet-only
native voice host using serial compilation. It adds no inference or protocol logic.
