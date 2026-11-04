# Aethyrforge for Mac: local editing by voice (experimental spec)

Status: experimental desktop shell, local setup, routed Coder HUD, object/NPC scene tools and cultist event controls implemented, 2026-11-04.
The local Everglade editor, shared storage, CLI inventory, microphone capture
and local Parakeet transcription work; the local edit model below remains planned. Nothing
here ships to customers or TestFlight until the owner asks.

Aethyrforge for Mac is a desktop app that opens your Everglade on your own Mac and
lets you change it by talking. Speech recognition, the model that turns words into
edits, the room and the world document all run locally. Optional Coder chat uses
the selected agent's normal provider connection and existing CLI login. World
publication remains a separate explicit action.

It builds on the web Aethyrforge (docs/verse/aethyrforge.md), the local room
(`coax/web/room/local.sh`, `room-dev.coax`), local Parakeet (#12667, #12669), and
gpt-oss-20b on Metal (docs/verification/psionic-gptoss-metal-2026-10-29.md).

Where it goes next: [Aethyrforge as the conductor](2026-11-04-aethyrforge-conductor.md)
makes this app the one place that schedules builds and tests, keeps the owner's
world current without restarting it, owns agents' workspaces and the lane that
lands changes, and routes world edits, tuning and code changes.

## Current build

`apps/aethyrforge-desktop/` now contains the AppKit/WKWebView shell, its Coax
entry-point rules and Coax build tool. It bundles the website's existing
`everglade5e.wasm`, shaders, pack and browser host, the native room, a static
page server, the voice Wasm and a local Parakeet voice host. This is the same
`/worlds/everglade` engine players use.

- Opens at the South Gate (`#gate`) with EDIT on (`?forge`). The native window
  identifies this as local editing. The existing editor's placement, selection,
  movement, rotation, resizing, deletion and in-session undo/redo are available.
  The prototype uses the existing named playtest-room profile
  (`room=pair-aethyrforge`) to enter with a default character: the normal
  first-visit character picker otherwise consumes keys behind EDIT's panels.
  Normal character selection remains future integration work.
- Starts its own room in `--dev` mode and a static page server, on two free
  loopback ports. The page's `/api/` requests go to that room. It stops both
  servers on normal quit, window close or a child-server failure; a file lock
  prevents two app instances writing the same local world.
  The relay is `ws://localhost:PORT`, the room client's existing local override;
  it connects directly to the room so a WebSocket cannot occupy the HTTP-only
  page proxy and block subsequent saves.
- Seeds the local world and controls from `coax/web/room/dev/` **only when absent**.
  Hand edits save through the local room into
  `~/.openagents/aethyrforge/everglade/world-everglade.json` and
  survive reopening. It does not yet fetch the current published world. The
  editor's existing “publish” request only writes this local room's document.
- Shows setup in a front pane over Everglade until setup has been saved. Later
  launches open directly into Everglade; existing valid saved setup counts too.
  Tool/model inventory refreshes in the background. The pane shows the shared OpenAgents CLI,
  Parakeet's installed status and location, gpt-oss's installed status,
  and the running local agent engines with a preferred-engine choice. Open
  Everglade saves the choices to `~/.openagents/aethyrforge/setup.json`; View →
  Setup… reopens the pane. Launch and Refresh only list installed files; they
  never hash model contents. The explicit Verify voice model button checks the
  pinned complete Parakeet weights and tokenizer SHA-256. Missing voice weights
  do not prevent manual editing. Models remain unloaded during setup.
- Uses `~/.openagents/` for app-owned files across OpenAgents apps. The app bundle
  and world/settings/logs live in `aethyrforge/`, shared executables in `bin/`
  with retained versions in `versions/`, and reusable model checkpoints in
  `models/`. This is now a repository rule for future apps. Upgrading moves the
  original Application Support world when the new destination is absent.
- `openagents coder agents --json` and standalone `coder agents --json` expose
  the same bounded, read-only inventory of local agent processes. They read
  executable names/PIDs, never prompts or command arguments. Running processes
  are explicitly `attached:false`; selecting an engine is a saved preference,
  not an attachment to the world. Session attachment remains future work.
- `openagents models ls --json` reports shared audio/text models; `models verify
  parakeet --json` verifies Parakeet; `models migrate --json` moves the legacy
  caches and leaves compatibility symlinks, refusing conflicting copies. The CLI,
  Coder dictation, Mac dictation and voice host use the shared model defaults.
- Microphone capture works through the page's TALK/V gesture after leaving
  EDIT with DONE/B. The native shell declares its microphone usage and requests
  macOS consent for audio from its own loopback main frame. First use starts a
  reserved-loopback voice host with Parakeet on Metal and local-only providers;
  the browser's voice runtime is bundled. Models stay unloaded during setup and
  resident after first use. Capture closes on release, setup, leaving the app, hiding
  or quit. A canceled permission request cannot later reopen capture. Speech
  stays on this Mac; the public voice service is not used.
  Microphone permission is separate from model preparation: a permission sheet
  taking key focus no longer produces a WebKit denial, and model startup failure
  cannot poison the site's grant. Every hold obtains a fresh native ticket even
  when WebKit caches permission. The real-device `--microphone-check` passed
  three captures with macOS already authorized and the window not key; tracks
  were stopped without recording/sending audio. It also passed repeated-ticket,
  model-failure recovery and canceled-preparation checks. The permission policy
  killed all four mutants; 137 desktop checks passed.
- Audio output remains muted through a live zero-gain graph so the microphone's
  AudioWorklet can process samples. A final Parakeet transcript now starts the
  same automatically routed Coder run/fork as typed input, while the Enter bar
  stays closed and typed drafts remain intact. Recognized words and live reply
  chunks float on the left and remain for five seconds after the turn finishes;
  the words are consumed by the native bridge and
  cannot also trigger an Everglade edit. A native microphone-grant ticket,
  local top-frame origin and current foreground/world readiness gate one final
  dispatch. Partials, canceled or duplicate finals and blank input do not run
  Coder; busy turns are retained and show that the new voice input was not sent.
  Coding-provider selection applies to finalized text; audio remains local.
  Typed and voice turns use Coder's unattended launch profile: Claude's
  `bypassPermissions`, Codex's `approvalPolicy=never` / `danger-full-access`
  and Microcoder's ungated tools. The desktop no longer forces `--ask`.
  New opt-in `--stream-json` emits `coder.reply-stream.v1` deltas as Coder receives
  them, with credential-prefix withholding and full-value redaction. Legacy JSON
  output and durable whole-reply histories are unchanged. The
  [separate stream/IPC contract](../protocol/coder-reply-stream-v1.md) records the
  new schemas and progress-versus-durable semantics.
  Verification: the actual muted WKWebView fixture traversed AudioWorklet,
  loopback Parakeet on Metal, Elephant routing and Microcoder, delivering live
  reply output with the bar closed, preserving a draft and ignoring a duplicate
  final. Its unnecessary tool request was denied by the fixture; production
  that earlier checkpoint used manual approvals. 435 Coder and 135 desktop checks passed; all 74 voice
  admission mutants and all 31 credential-prefix mutants were killed. The
  repository tools check reports 28 existing unrelated missing-header files.
  The dedicated voice transcript bar, Space binding, model download, typed model
  edits, durable undo log, remote PUBLISH/REVERT, bots and raid controls are not
  wired into this shell yet.
- Basic coding-agent chat uses the same `openagents coder` as the TUI. The
  compact Aethyr composer is 88 points high with a faint tint; there is no
  full-window frame or label. Coder is hidden until Enter or Cmd+L opens it.
  Native Return/keypad Enter replaces region chat and consumes the complete
  press, including held-key repeats and release. Automatic routing status and
  the working folder sit above the bottom input. The setup agent dropdown and
  composer agent/model/effort dropdowns have been removed. Enter sends and closes
  the composer, Shift-Enter inserts a line and Escape closes while
  retaining the draft and running agent; Stop cancels. With the composer closed,
  only reply text floats on the left while a run is active, without a background,
  heading, controls or mouse interception. Completed replies stay for five
  seconds, then hide; Enter restores the full conversation. The floating feed
  is an Aethyr component (`aethyr/ui/feed.coax`, `feed-draw.coax`): Coax owns
  its buffer, UTF-8 retention, text wrapping and lifetime, and emits shared
  font/stroke vertices for Metal. Swift observes time and submits buffers;
  floating text has no AppKit text view or Swift deadline state. Input/IME
  and selectable full history remain platform text effects. The unattended
  launch options are Coax too. Real Claude and Codex checks wrote the requested
  fixture file with zero approval prompts and verified context and Stop.
  Native input smoke checks both Enter keys over the real
  packaged realm, no leaked region-chat events, presentation/approval transitions
  and a real CLI round trip. Verification: 142 Coax checks passed; all 72 feed,
  33 presentation and four launch-option mutants were killed. Native input passed
  with zero realm Enter keydowns/keyups and no region-chat text field. The
  repository tools check still reports its 28 pre-existing unrelated files
  without Firm reason headers. Follow-ups use Coder's
  durable fork/context path. New chat and folder changes start fresh.
  Complete histories stay in `~/.openagents/coder/sessions/`; UI settings and
  workspace are under `~/.openagents/aethyrforge/`. Prebuilt Coder/Microcoder are bundled,
  so messages do not compile tools or need a checkout. This creates an owned
  Coder conversation; discovered PIDs remain unattached. Slash commands and
  turning chat into typed world edits remain planned.
- Automatic agent routing reuses Coder's typed System One decision router.
  Every `run` and automatic `fork` asks for an eligible agent, task difficulty
  and reasoning effort. Local Elephant at `127.0.0.1:8793` answered actual probes:
  a typo chose Codex/low; a complex multi-service refactor chose Claude/high/Opus.
  Claude's model profile is Haiku for trivial, Sonnet for easy/moderate and Opus
  for hard/very hard; Codex uses its configured model and routed effort. These
  finite probes are not a new routing-accuracy benchmark. Shared config lives in
  `~/.openagents/coder/router.json` (`coder.router.v1`), created only if absent.
  The resident local server is reused; no weights are downloaded, rehashed or
  loaded by the desktop. A configured Pylon System One fleet/proxy uses the same
  request format, marked remote and subject to Coder's existing session-consent
  gate. CLI discovery currently advertised no Pylons; the enabled route is local.
  Calls expire after eight seconds. The classifier sees a UTF-8-safe 768-byte
  task projection and 256 bytes of recent conversation; the worker retains full
  prompt/history. Failure/low confidence uses only an eligible default and logs
  the reason, without a hosted-model substitution. New `coder.route.v2` records
  have a [separate schema](../../proto/coder/route-v2.schema.json); legacy route
  records and the System One network protocol remain unchanged. Native automatic
  chat checks exercise two routed turns and retained context, both with Codex
  candidates and with the full shared eligibility (which selected Microcoder
  for brief chat). The focused router
  policy killed all 88 mutants; 432 Coder and 131 desktop checks passed.
  Router-disconnected, empty-config and missing-URL probes refused cleanly;
  tools-check reports 27 existing unrelated missing-header files at this base.
- Startup waits on real rendered-scene readiness, using a Coax budget that
  counts visible rendering time. WebKit's normal occlusion/background suspension
  no longer consumes the timeout. The native Metal view occupies only the
  composer, and both overlays explicitly report transparency. Startup/server
  errors stop the spinner, show their cause and offer Retry startup; stale
  process-exit callbacks cannot stop a restarted stack. The visible launch
  check uses real animation callbacks, separately from the hidden frame-pump
  smoke, including a 65-second hidden interval and the actual Retry control.
  Verification on 2026-11-04: visible launch reached a ready scene in 1.11 s;
  the background/retry check retained 64.96 s of actual hidden time and then
  reached the new room's rendered scene with entry enabled and setup dismissed.
  The startup unit killed all 13 mutants; all 80 session checks passed.
- Requires Apple Silicon and macOS 26+ for this initial WKWebView/WebGPU build.
  The app is ad-hoc signed, experimental and outside `/Applications`.

- Typed and finalized voice requests can now change the live local scene through
  the same routed Coder conversation. The bundled Coax `aethyrforge-scene` tool
  reads the catalogue and current document, then atomically places rows or
  supported stacks, moves groups or removes groups. “Add three crates” and
  “add a stack of five crates” were exercised through the real local Elephant
  router and Codex; the composer stayed closed. There are no placement decisions
  in Swift. The tool reuses Everglade's saved document, footprint admission,
  room update events, player walls and NPC fences. Rows leave walkable gaps;
  the shared NPC detour now rejects a corner whose final leg crosses the same
  obstacle. Regression checks make an NPC walk around both generated layouts.
  Concurrent edits use the existing base-version check, with readback before
  success. See [the command contract](../protocol/aethyrforge-scene-v1.md).
  Verification: the policy's 157-check baseline passes and all 354 root mutants
  are killed, with zero survivors. The shared controller/scene suite passed
  210 checks. The native room startup, WebGPU bundle and real routed Coder
  row/stack, group edits, atomic refusal and reopen persistence checks pass.
  The tools audit retains 28 unrelated missing-reason headers.

- NPC authoring uses the same automatically routed Coder turns, through the
  Coax scene tool's `npcs`/`npc` commands. It can spawn cultists, villagers and
  guards, direct one actor or a group to attack a named townsperson, player,
  NPC/creature or building, move them, stop them or remove them. The local room
  owns combat turns and real HP/deaths; melee strikes damage the existing
  breakable building pieces and the page hears its normal break events.
  Explicit NPCs keep their orders independently of the automatic siege.
  Humanoid models reuse the existing character parts. Spawn floors and motion
  use the actual world and current crate fences; an object placed under a
  stopped NPC pushes it clear. Definitions/orders persist in the separate
  `forge-npcs.json`, with atomic save before application and optimistic versions.
  Reopening starts a fresh playtest, without preserving transient HP/movement.
  Batches are 1–8, with 32 authored NPCs. Admission and saved-document units
  kill all 311 and 109 mutants respectively, with zero survivors. See
  [the NPC command contract](../protocol/aethyrforge-npcs-v1.md).
  The hidden native check exercised six real routed Coder turns: order a
  cultist group to attack a villager, spawn three villagers, then stop the first
  group and spawn/order another against the stables. It observed real NPC HP
  loss, four replicated building breaks and seven live rendered creatures; all
  eight authored definitions survived reopening. Fountain/crate spawn refusal,
  dynamic crate detours, stopped-NPC eviction and human villager parts pass.
  The tools audit reports 29 unrelated missing-reason headers.
- The planned cultist event is available through Coder and `aethyrforge-scene
  raid`: immediate start, double the cult army, add a wave, set its fighter
  target and stop. It uses the existing siege simulation, leader, wards, watch
  and replicated HUD. The default cult target is 30; doubling makes it 60,
  while the watch target stays 24. Complete batches must fit the shared 96
  creature slots, preserving authored NPCs and summons, including reserved dead
  NPC definitions. The local control poll cannot overwrite a requested army.
  Event state is transient on reopening. The 106 local event admission mutants
  and all 12 existing event regression checks pass. The same native check
  routed event start/double/set/wave through Coder, verified a cult target of
  60 and 86 replicated creatures on the page, then checked atomic overflow
  refusal and stopping without deleting the eight authored NPC definitions.

The remaining sections describe the target behavior. They do not imply every voice
acceptance criteria already pass. Build and native smoke-check instructions are
in [the app README](../../apps/aethyrforge-desktop/README.md).

Shell/save checkpoint (`d1f14ec560`): both native smoke launches drew a WebGPU scene in 4.92 s
and 5.07 s. A keyboard placement followed by DONE saved local version 23; reopening
kept the exact saved bytes. Both child servers exited after each check. The signed
standalone app was opened and its EDIT view inspected. The session's two Coax
checks, native room startup check, bundle signature and plist checks passed.
`coax/web/tool.sh tools-check` was run and reported 20 pre-existing missing-reason
headers outside this change; the new build wrapper has its required reason.

Setup checkpoint: the hidden native check drew WebGPU in 8.67 s, discovered the
real CLI and 9 local agent processes, verified shared Parakeet files, saved the
preferred engine, entered Everglade and reopened setup. Both child servers exited.
The visible native pane and its Claude/Codex/Coder/Cursor choices were inspected;
Open Everglade and Cmd+, worked. The migrated world retained the exact SHA-256
(`f33e9c80…63ff43`). The setup/session suite passed 78 checks; shared paths and
inventory passed 79. Model identity mutation killed all 4 mutants, with zero
survivors. The installed shared CLI transcribed the recorded `hello-foreman.wav`
through Metal using the shared checkpoint and helper, without a checkout or
microphone: “Hello Foreman. What are we building today?” New inventory schema
generation and protocol compatibility/conformance checks passed. The final
tools audit retained the same 20 unrelated baseline findings.

Startup follow-up (2026-11-04): removed automatic model hashing from launch.
Only the explicit Verify button runs the full-file check. The rebuilt native
shell's setup smoke passed with installed voice weights, `voiceVerified:false`,
9 local agents, saved settings and a WebGPU frame in 0.93 s. It reused the
installed engine/room assets. The tools audit reported 21 existing unrelated
missing-reason headers at this later main revision.

Microphone follow-up (2026-11-04): fixed the unconditional WebKit capture denial,
missing usage description/voice Wasm, suspended audio graph and remote voice
default. A hidden WKWebView fixture check passed through synthetic microphone,
AudioWorklet, voice Wasm, loopback socket and Parakeet on Metal, returning “Hello
Foreman, what are we building today?” It also proved that canceling a pending
capture stops late tracks and opens no voice socket. All three child servers
exited. The owner approved the microphone prompt; a subsequent real TALK hold
reached `listening` and connected to the local Parakeet host, then closed on
release. Permission/phase diagnostics contain no transcripts or audio. A separate
hidden hardware-capture attempt could not open WebKit's physical microphone in
an unordered window; the visible Talk path verified that platform boundary.
Native compilation, voice packaging, signature/plist and diff checks passed.
The tools audit retained 21 unrelated baseline findings.

## What you do

1. Open **Aethyrforge.app**, review setup and press **Open Everglade**. The world
   is at the South Gate, with EDIT on.
2. Hold **Space** (or the mic button) and say what you want:
   - "Put three barrels by the inn door."
   - "Make that crate twice as big."
   - "Move the well five meters toward the gate."
   - "Turn the lanterns off on Inn Street."
   - "Make it night."
   - "Undo that."
3. Let go. Your words appear in the transcript bar within about 0.3 s. The edit
   appears in the world within about 2 s, with a short line saying what changed:
   "Placed 3 barrels by The Bent Nail's door."
4. Walk, fly or click to look around. Click a thing to select it, so "that" means
   it.
5. Press **PUBLISH** to send your local version to openagents.com, or **REVERT**
   to go back to the published one.

Nothing else is on screen: the world, the transcript bar, the edit line, EDIT's
panes, PUBLISH and REVERT. No mode switches.

## Parts

| Part | What it does | Where it lives |
|---|---|---|
| App shell | An AppKit window, the menu, the mic and Space key, and the processes below. | `apps/aethyrforge-desktop/` (Swift host, Coax logic, like `apps/openagents-desktop/`) |
| World view | The same Everglade build as the site, drawn with WebGPU in a WKWebView. | The page from `apps/openagents-web/build.sh`, loaded from the local server |
| Local room | The room server in `--dev` mode: the local player is the admin, it saves the world document, and every page reloads it. | `coax/web/room/bin/room --dev DIR` (a38754d290) |
| Local page server | It serves the built page and passes `/api/` to the room. | `coax/web/serve.sh --static --api PORT` |
| Recognition | Resident Parakeet TDT v3, hold to talk, final transcripts only. | The voice host's `--recognizer parakeet` and `openagents dictate` path |
| Edit model | Turns a transcript and the world's context into one typed edit. | gpt-oss-20b on Psionic Metal |
| Edits | Typed, invertible edits with undo and redo. | `aethyr/forge/edit.coax` |
| World document | Your local copy of Everglade's world. | `~/.openagents/aethyrforge/everglade/world-everglade.json` |

### Why the world view is a web view

Firm reason: the world view must be the exact engine build that players get. A
WKWebView with WebGPU runs that build unchanged. A separate native Metal renderer
would be a second renderer to keep in step, which is the two-copies problem behind
today's room bugs. Everything around the view (voice, the model, edits, files) is
native Coax and C.

## Voice

- The app records 16 kHz mono while Space or the mic button is held, and sends the
  hold's audio to one resident Parakeet model when you let go. The model loads once
  on first explicit voice use and stays loaded until quit.
- `aethyr/voice.coax` admits each hold's final transcript once, in its turn,
  through the replay gate, as the voice host does.
- The transcript shows in the bar. You can click it to fix a word, then press
  Return to resend it.
- Typing works too: press Return with nothing held to type an instruction.

## The edit model

The model runs locally: **gpt-oss-20b** on Psionic Metal, measured at about 35
tokens a second decode on this Mac (M5 Max, 128 GB). Its 13.8 GB of weights are
resident in one GPU buffer.

- **Input:** the transcript, the selected thing, what is under the cursor, the
  camera's place and heading, the district names near you, the last 3 edits, and
  the edit schema.
- **Output:** exactly one typed verb from the schema, filled in, or a short
  question back when something is ambiguous ("Which crate? There are four on Inn
  Street.").
- **Constrained:** the output is decoded under a grammar built from the edit
  schema, so it is always a valid edit. Parsing it is bounded field parsing after
  the verb is chosen (the workspace's semantic routing rule).
- **Verbs:** `place`, `move`, `scale`, `rotate`, `delete`, `duplicate`, `switch`
  (a feature on or off), `set` (time of day, weather, light), `name`, `ask` (a
  thing the catalogue cannot draw goes on the list), `undo`, `redo`, `look`
  (answer a question about the screen), and `say` (a short reply).
- **Places and things:** the model refers to things by the world document's ids.
  The app resolves "the inn door", "that", "here" and "five meters toward the gate"
  to ids and coordinates from the world's district, building and door tables
  before the edit is applied. The model never invents coordinates.
- **Time to edit:** under 2 s from release to the edit on screen for a one-verb
  edit, measured with the transcript bar and the room's save in the path.

### The model download

- The app checks for the model at start. When it isn't present, it downloads the
  pinned `openai/gpt-oss-20b` revision
  (`6cee5e81ee83917806bbde320786a8fb61efebee`) to `~/.openagents/models/gpt-oss/20b/REVISION/original/`,
  with a progress bar and size shown ("13.8 GB"). It checks every file's hash
  before use.
- Parakeet downloads the same way to `~/.openagents/models/parakeet/`.
- This Mac already has both, migrated without copying the weights. Parakeet
  verification is manual only; full gpt-oss verification and automatic downloads
  remain planned.
- Weights never go in Git.

## Saving, undo and publish

- **Each edit saves** through the local room's `POST /api/playtest/world/everglade`,
  as the next local version. The room reloads it and the view updates.
- **Undo and redo** are the edit log's inverses (Cmd+Z, Cmd+Shift+Z, or saying
  it). The log lives next to the world document and survives restarts.
- **PUBLISH** sends the local document to openagents.com through the same world
  publish the web EDIT uses, signed in as you. It shows what changes first ("12
  edits: 5 placed, 4 moved, 3 resized") and needs one click to confirm. It never
  publishes on its own.
- **REVERT** fetches the published document and replaces the local one, after one
  click to confirm.
- **At start**, the app fetches the published Everglade into the local document
  when there are no local changes. If there are local changes, it keeps them and
  shows "Local changes: 12 edits not published".

## Play-testing your edits

Because it is a real local room, you can test edits as players do:

- **ADD BOTS** starts N local bot players (`coax/web/room/raid-check.coax`) in the
  same room.
- **START RAID (implemented through Coder and the scene CLI)** starts the planned
  standing siege immediately: cult fighters, 24 watch, Darian and three wards.
  “Double the cultist army” doubles the cult target; “add a wave of five” raises
  it by five and musters immediately. Set and stop are also available.
  Cult targets are 1–60, within the room's 96 shared creature places; overflow
  refuses atomically. Public room limits remain unchanged. The event runs real
  combat and the existing replicated siege HUD. Event state resets on reopening,
  while authored NPC definitions/orders persist. See the
  [local event command contract](../protocol/aethyrforge-raid-v1.md).
- The app never touches openagents.com's rooms.

## Build and run

```sh
apps/aethyrforge-desktop/build.sh --open
```

It builds the Everglade page and room when their source trees are newer, builds
the app at `apps/aethyrforge-desktop/build/Aethyrforge.app`, and opens a complete
copy at `~/.openagents/aethyrforge/Aethyrforge.app`. That copy runs without the
checkout. The app picks free local ports for the room and page server on each
start, and stops both when it quits. The local world directory also contains
`room.log`, `serve.log` and the current launch's `local-url.txt`.

The app is experimental:

- It's signed for this Mac only.
- It isn't in releases, DMGs or TestFlight.
- It isn't installed to /Applications.

## Acceptance

1. Open the app and see Everglade within 10 s, with EDIT on.
2. Each of the six example sentences in "What you do" produces the right edit, and
   the edit is on screen within 2 s of release.
3. "Undo that" reverts the last edit. Quitting and reopening keeps every edit.
4. An ambiguous sentence gets a question back, not a guess.
5. With Wi-Fi off, steps 1 to 4 still pass.
6. PUBLISH, confirmed, changes the live Everglade's document. A new browser
   session at openagents.com/worlds/everglade shows the edit.
7. A headless test drives the app's local stack end to end. It sends 20 recorded
   WAV instructions through Parakeet and the model, applies the edits through the
   room, and checks the world document and a frame for each.

## Build order

Each step lands on `main` working end to end.

1. **The shell and the local stack (implemented):** the app window, the local room
   and page server, and Everglade with EDIT on. Saving by hand uses the existing
   editor and local room. Native smoke verification opens a hidden WKWebView,
   waits for a WebGPU scene, places a thing by keyboard and checks its saved
   document; a second launch verifies the saved bytes survive reopening.
   **Setup (implemented):** the front pane, shared `~/.openagents/` storage,
   CLI/agent/model inventories, pinned Parakeet verification and saved choices.
   **Coder HUD (implemented):** basic chat, live Codex model selection, CLI
   follow-up context, tool consent and Stop. Native checks exercised real Codex,
   Claude Code and Microcoder two-turn conversations, a Codex approved file
   write, cancellation and the visible Enter/send/reply path. Cursor discovery
   now rejects the full installer shim; this Mac's installed agent executables
   exit 137 even for `--version`, so chat reports their runtime failure.
   Gates: 107 HUD/parser checks, 54 admission mutants killed with zero survivors,
   417 Coder checks, 36 Cursor checks and a native long-shim regression. Unchanged
   world/voice assets reuse their earlier verification. Tools-check still reports
   the same 21 unrelated missing-reason files. Agent attachment and the conductor's
   worktree/integration lane remain planned; compiler build-load work is owned
   separately.
2. **Voice to text (implemented):** hold to talk, resident Parakeet, and the
   finalized transcript sent directly to routed Coder with its replies in the HUD.
3. **Text to edits (basic catalogue path implemented):** the routed coding agent
   calls a typed Coax scene tool. Rows, supported stacks, group moves/removals,
   local persistence and immediate player/NPC obstacles work. A batch is 1–16
   items, within the existing 64-object document; stacks have at most 10 m of
   base elevation. Named-place resolution, arbitrary assets and undo/redo remain
   planned. No dedicated gpt-oss interpreter is needed for this initial path.
4. **Publish and revert,** with the change summary.
5. **Raid controls (implemented),** bots for play-testing, and the broader
   recorded-voice end-to-end acceptance. Local event command admission kills
   all 106 root mutants. Native NPC/event acceptance also exercises real routed
   Coder, the local room and the WKWebView scene.

## Not in this version

- Windows or Linux.
- Multiple people editing one local world. Use the web EDIT on a room for that.
- Building new kinds of things the catalogue can't draw. Those go on the list
  (`ask`) for an agent to build.
- Speaking replies aloud. Replies are text in the edit line.
