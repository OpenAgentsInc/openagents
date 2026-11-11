# Apps roadmap: Linux first, one agent stack, shared code for Verse apps

Written 2026-11-10. Status: Step 0 is done, Step 1 opens (see its status), Steps 2 and 3 are in progress.
Tracked in GitHub #13153.

The order is the owner's (2026-11-10): work on Linux first because the Mac is
busy, get OpenAgents for desktop opening on `coderos-4080`, then make every
app use the same agent code.

## Where things stand

| App | Runs on | State on 2026-11-10 |
| --- | --- | --- |
| `apps/openagents-web` | Browser (WebGPU) | The main product. `/chat` is drawn with the shared Zeron components |
| `apps/openagents-desktop` | Mac only | Chat with Claude Code, Codex, Psionic and the Lab. Swift host on AppKit and Metal |
| `apps/aethyrforge-desktop` | Mac, Linux | The only app with a native Linux window (Wayland and Vulkan) |
| `apps/everglade-desktop` | Mac only | Runs the web's realms natively through Aethyr's Metal page host |
| `apps/everglade-mobile` | iOS, Android | Experimental |
| `apps/openagents-ios` | iOS | Home, wallet, voice |
| `apps/openagents-cli` | Mac, Linux | Active |

Shared today, in `apps/shared/`: the Zeron chat components (`zeron/`) and
dictation. Everything else is per app.

## Step 0. Pages that need WebGPU must say so

Found on 2026-11-10: on `coderos-4080` the Coder Browser has `navigator.gpu`
but no adapter, so openagents.com and `/chat` showed only a page title and a
link to the page's Markdown.

- **The site.** When a page can't get a WebGPU adapter, `coax/web/host.js`
  shows one plain message on every page: what is missing and how to turn it
  on. The Markdown link stays for agents and readers, below that message.
- **The Coder Browser** (`~/coder/os/bin/coder-browser`, not this
  repository). Measured: it gets the RTX 4080 as its WebGPU adapter when
  started with `--enable-unsafe-webgpu`, `--disable-gpu-sandbox`,
  `VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json` and
  `LD_LIBRARY_PATH=/run/opengl-driver/lib`. Without the driver file it gets
  SwiftShader (software). `--disable-features=Vulkan` stays, since Chromium's
  Vulkan compositor crashes on this card under Wayland. Running the GPU
  process without its sandbox is the owner's decision.

Done when: openagents.com and `/chat` draw in the Coder Browser, and a browser
with no adapter shows the message.

## Step 1. OpenAgents for desktop opens on Linux

Status, 2026-11-10: it opens, and slices 1 to 4 below are done except IME.
`apps/openagents-desktop/linux/build.sh --install --open`
builds it, installs `~/.openagents/bin/openagents-desktop` and opens the
window on `coderos-4080`.

What was checked: `coax/web/tool.sh apps/openagents-desktop/linux/check.coax`
passes 23 cases offscreen, 21 on a machine without the Parakeet model. Seven
are pictures against `goldens/linux/` (the demo conversation at two sizes, a
table, the sidebar closed, no Claude Code installed, an attachment,
dictation listening). Seven are what the app prints: a turn, an
approval allowed and one denied, Claude asking Codex and Codex asking Claude
through the room, a session kept at quit and continued after reopening. They
use the stand-in Claude Code and Codex the Mac's checks use. Nine more are
what dictation prints (below). One real Claude
Code turn also ran offscreen, and the window held 60 frames a second
through a resize. Text is drawn and kerned by Coax (`components/raster/`,
`coax/lib/text/ttf.coax`); the kerning agrees with fontTools on all 2,891
kerned ASCII pairs of Geist.

Dictation is the Mac's. Hold Ctrl+D, or the microphone under the composer,
and speak; the words go into the draft when you let go and are never sent.
It is off until you press Ctrl+Shift+D or start the app with `--dictation
on`, and `settings.json` keeps the choice. The same Coax decides everything
(`zr_dictation_*`, `apps/shared/dictation-policy.coax`). The Linux host
(`linux/dictation-host.c`) runs the recorder `openagents dictate` runs
(`pw-record`) and Psionic's Parakeet on CUDA, and looks for the model in the
shared model home (`~/.openagents/models/`, or `OPENAGENTS_MODEL_CACHE`).
Ten of the 23 cases are dictation's, and none opens the microphone. Eight
feed a recorded clip on the app's fixture clock: the picture, the key with
Undo, the microphone button, a tap, dictation off, and the setting saved and
restored. Two run in real time, with a stand-in
(`tests/recorder-stand-in.coax`) started in `pw-record`'s place and the key
held while it plays the clip. One of each kind recognizes the clip with the
pinned Parakeet model on the GPU and is skipped where the model home has no
model; the other eight use a stand-in recognizer that answers with the
clip's words.

Not checked: typing into the window with a real keyboard, an input method,
choosing a file in the desktop's file chooser (the dialog was opened
and closed by a test, never answered), and dictation with a real
microphone and a real keyboard: `pw-record` itself has never been run by
the app. Not built on Linux:
colour emoji, the five font dilation levels, and a terminal for a notice's
Log in or Install button (the command is printed instead).

The host is `apps/shared/host/linux/` (shared: the window and pipeline in
`ui-host.c`, the desktop's file chooser in `portal.c`, over the
realms' Vulkan and Wayland helpers in `verse/gpu/`). The audit in
[docs/research/exact/04-apps-audit-2026-11-10.md](../research/exact/04-apps-audit-2026-11-10.md)
recommends its `--do` steps become `coax drive` operations.

The app's product code is already Coax compiled to C behind `face.h`. Only
the host is Mac-only. A Linux host has to supply what `Sources/Zeron.swift`
and `Sources/main.swift` supply:

| The host's job | On the Mac | On Linux |
| --- | --- | --- |
| Window, keys, pointer | AppKit | Wayland, from `verse/gpu/vk-window.c` (Aethyrforge's Linux build uses it) |
| Drawing | `Zeron.metal` | Vulkan, with the shaders from `apps/openagents-web/gpu/chat.coax` (the Coax port `/chat` already uses) emitted as SPIR-V by `coax spirv` |
| Glyph and icon atlas | CoreText and AppKit SVG | To decide: see the open question below |
| Clipboard, IME, file picker | AppKit | Wayland data device and text input; `xdg-desktop-portal` for files |
| Offscreen shots | `--shot`, `--do`, `--compare` | The same flags, so `check.sh` and its goldens run on Linux |
| Dictation audio | AVFoundation | PipeWire, through the CLI's recorder (`pw-record`) |

Slices, each one opens and is checked before the next:

1. **A window with the demo conversation** (`--demo`): window, Vulkan, atlas,
   the frame loop. No engine, no input beyond scroll and quit.
2. **Typing and sending** to your own Claude Code and Codex: keys, pointer,
   clipboard, the cursor. The engines are Coax and already run on Linux in
   Coder's terminal.
3. **The checks**: `--shot` and `--compare` on Linux, and Linux goldens beside
   the Mac ones (text rasterizes differently, so they are separate files).
4. **The rest**: IME, attachments, saved sessions and settings, the room,
   dictation.

The host is written once as shared code (`apps/shared/host/linux/`), not
inside `openagents-desktop/`. Aethyrforge's Linux build moves onto it in
Step 3.

Open question: the glyph atlas. The Mac host matches GPUI's CoreText
rasterization exactly. Linux has no CoreText, and the rule is no toolchain
beyond Coax and the host C compiler. Options: rasterize Geist in Coax (a
TrueType outline reader and scanline fill, also usable on Android), or link
FreeType through Nix as a host library. Recommended: Coax, so one rasterizer
serves Linux, Android and any shot check. Taken: Linux builds the atlas in
Coax (`components/raster/`); the Mac keeps CoreText until that rasterizer
has font fallback and colour glyphs (components/spec.md, decision 3).

Done when: `apps/openagents-desktop/build.sh --open` on `coderos-4080` opens
the window, a message to Claude Code streams an answer, and `check.sh` passes
on Linux.

## Step 2. One agent stack

Today the same job is done several ways:

| Where | What it does | How |
| --- | --- | --- |
| `coax/lib/agent/claude`, `coax/lib/agent/codex` | The SDKs: transport, control protocol, messages | The shared base. Keep |
| `coax/lib/agent/engine/` (moved 2026-11-10 from `apps/openagents-desktop/zeron/`, about 4,700 lines) | Sessions, parts, tool chips, approvals, routing | The shared engine layer over the SDKs; the desktop app is its first user |
| `coder/tui/` (`coder.coax`, `codex.coax`, `session.coax`) | Coder's terminal: the same engines, its own session log | On the shared engines (2026-11-10, Claude Code; 2026-11-11, Codex, Cursor and Microcoder); its own session log |
| `apps/aethyrforge-desktop/chat.coax` | The Forge's chat | Runs Coder as a child and reads its JSONL |
| `apps/everglade-desktop/next.coax`, `tiers.coax` | Next issue, and Golemancy's issue rating | On the shared Claude engine with their own options (2026-11-10) |
| `apps/everglade-desktop/golems.coax`, `os/verse-agents.coax`, `coax/lib/agent/observe/processes.coax` | Which agents are working on this machine | One walker, `coax/lib/agent/observe/transcripts.coax` (2026-11-10); the third lists processes |
| `coax/web/agent/chat.coax`, `turn.coax` | `/chat` and the project agent on the server | On the shared Claude engine; the wire is written from the record (2026-11-10) |

The target is three shared layers under `coax/lib/agent/`, and nothing above
them reimplements them:

1. **Engines** (`agent/engine/`): one interface (start, send, poll, answer an
   approval, stop) with Claude, Codex, Psionic and the Lab's router behind
   it. Taken from the desktop app's `zeron/` engines, which are the most
   complete.
2. **Sessions** (`agent/session/`): one record of a conversation (parts,
   tools, results, status) and one log format on disk under
   `~/.openagents/`. Coder's JSONL is the starting point because three
   programs already read it.
3. **Observation** (`agent/observe/`): one reader of Claude Code, Codex and
   Coder transcripts, for the Golems board, Verse OS and anything else that
   shows who is working.

Order of moves, one consumer at a time, each with its checks kept green:

1. Extract the engines from `apps/openagents-desktop/zeron/` with no
   behaviour change; the desktop app is the first consumer.
2. Coder's terminal moves onto them.
3. Everglade's Next issue and Golemancy's rating move onto them.
4. Aethyrforge's chat drops its Coder child for the shared engines.
5. The three transcript readers become one.
6. The server engines (`coax/web/agent/`) take the shared session record.

Status (2026-11-10, GitHub #13153):

- **Built: the three layers exist.** `coax/lib/agent/engine/` holds the
  engines moved from the desktop app (`claude.coax`, `codex.coax`,
  `psionic.coax`, `lab.coax`, `gpu.coax`, `microcoder.coax`) and
  `engine.coax`, the one interface (`ag_start`, `ag_send`, `ag_poll`,
  `ag_answer`, `ag_stop`). `coax/lib/agent/session/` holds the record
  (`session.coax`, with the part kinds in `kinds.coax`) and the log format
  (`log.coax`: Coder's JSONL plus the keys a record needs to come back
  whole). `coax/lib/agent/observe/` holds the transcript walker
  (`transcripts.coax`), the process inventory (`processes.coax`, was
  `local-agents.coax`) and the read-only Claude history reader
  (`claude-history.coax`, was `session-reader.coax`).
- **Built: move 1.** The desktop app's screen calls only `engine.coax`; its
  `zeron/claude.coax` keeps only how a session becomes rows; its store
  writes the shared log format and still reads its old files.
- **Built: move 5.** The Golems board's reader and Verse OS find transcripts
  through `observe/transcripts.coax`. `local-agents` never read transcripts
  (it lists processes); it moved beside the walker.
- **Checked end to end on Linux** (`coderos-4080`, 2026-11-10): the Linux
  build of OpenAgents for desktop, run offscreen (`--shot`, `--do`) against
  the stand-in Claude Code in `zeron/fixtures/stream/`, sends a message
  through `engine.coax`, writes
  `~/.openagents/desktop/sessions/<id>.jsonl` in the shared log format,
  and on the next launch restores the conversation from it and resumes the
  harness session. `coder ls`, `coder status` and `coder log` read that same
  file when it is put in Coder's sessions folder.
- **Built: what the other consumers needed from the shared layers.** The
  Claude engine takes its owner's options as they are (`e_attach`, `e_own`),
  hands over its events one at a time after its own part of each (`e_poll`,
  `e_next`; `ag_next` in `engine.coax`), runs one turn or a conversation
  (`once`), and leaves hooks, permission requests and in-process tool calls
  to the owner (`e_allow`, `e_deny`, `e_tool_text`). The record keeps a
  tool's own name, folds text from whole frames when partial messages are
  off (`frames`), keeps text as it came (`tabs`), and serves a surface that
  prints as it goes (`s_next`, `s_trim`).
- **Built: move 2 for Claude Code.** Coder's terminal starts, sends, polls,
  answers and stops its claude engine through the shared engine, and prints
  the reply and each tool call from the record. Checked against the Coder
  from before the move, with stand-in Claude Codes: `coder run --engine
  claude` prints the same lines and writes the same session JSONL for a plain
  turn, an approval allowed and denied, no `claude` on PATH, and a turn with
  an Edit's diff, a failed Write, a Grep and text with a tab and a table.
- **Built: move 2 for Codex, Cursor and Microcoder** (2026-11-11). The
  shared Codex engine takes an owner as the Claude one does (`cxe_own`: the
  owner's launch, its `CxSettings` as they are, with its dynamic tools at
  thread start, and its client name; `cxe_poll` hands each event over after
  the engine's part; approvals and tool calls are the owner's, `cxe_approve`
  and `cxe_tool_answer`); the desktop app's path is unchanged.
  `engine/cursor.coax` is the Cursor engine (`cue_*`: start, log in, open or
  resume the session, hold the first message until its settings are
  answered, answer a resumed session's replay), moved out of
  `coder/tui/cursor.coax`. `engine/microcoder.coax` gains the worker client
  (`McWorker`, `mcw_*`) beside the Lab's in-process run of the same loop,
  moved out of `coder/tui/microcoder.coax`. Coder loads `engine.coax`; a Coax
  tool gets weak definitions of the Lab's signer and clock from
  `coax/platform/agent-signer-none.c`, which a host's own (`zeron-face.c`)
  override. Checked against the Coder from before each move with stand-ins
  (`coder/tui/codex_standin.coax`, `cursor_standin.coax`) and Microcoder's
  recorded replay (`GW_REPLAY`, the real worker and loop): `coder run` prints
  the same stdout, writes the same session JSONL and sends the same protocol
  lines for a plain turn, a command allowed, refused and run unasked, a file
  change allowed and refused, Codex's dynamic tool, a failed turn, Cursor's
  `--effort` applied and not offered, and no CLI on PATH. Cursor and
  Microcoder are not modes of `engine.coax`'s `Agent` (the desktop app has
  neither); Coder drives them with `cue_*` and `mcw_*` as it drives Claude
  with `e_*` and Codex with `cxe_*`. Not run: Coder's interactive prompt
  (y/a/n answers, Esc) against a stand-in, and any real Codex, Cursor or
  model.
- **Built: move 3.** Everglade's Next issue panel (`next.coax`) and
  Golemancy's rating (`tiers.coax`) start and drive their agents through the
  shared engine with their own options and tools. They still print from the
  messages. Checked by their Coax checks, `coax c` of `desktop.coax`, and a
  run of `tiers.coax` against a stand-in (same table and ledger line before
  and after). Everglade for desktop is Mac only: not built or run here.
- **Built: move 6.** `coax/web/agent/chat.coax` and `turn.coax` run their
  turn on the shared engine and write the wire's message and tool events
  from the record. Checked by building both as the Dockerfile does and
  running a turn against stand-ins before and after: the same wire lines
  for a plain answer and for a refused tool call. Not deployed.
- **Move 4, first part built: what Aethyrforge's child does, as shared
  operations.** `coax/lib/agent/engine/route.coax` is the System One router
  (`agr_begin`, `agr_needs_grant`, `agr_ask`, `agr_pick`, `agr_json`; two
  steps so an owner keeps its consent ledger), `availability.coax` the
  allowlist, installed CLIs and provider cooldowns (`ca_eligible`,
  `ca_record`), with `route-policy.coax` and `availability-policy.coax`
  (`#authority`) moved as they were; `echo.coax` is the echo engine
  (`ech_*`); `coax/lib/agent/session/fork.coax` is a fork of a session log
  (its turns as text, or Claude Code's resume or Cursor's session/load) and
  a context file ahead of the first turn (`sf_*`). Coder runs on all of it:
  32 `coder run`, `fork`, `route` and `replay` runs against stand-ins print
  and write the same before and after.
- **Not built: move 4, the child itself.** Aethyrforge's chat still starts
  `coder run|fork ... --engine auto --stream-json` (`linux-native.coax`
  `fgn_launch` and `fgn_tick`'s line reader; `Sources/Chat.swift` on the
  Mac). What is left is one headless turn runner in `agent/engine/` (say
  `run.coax`) that does per turn what `coder run` does in its plain
  `--stream-json` mode, and that both Coder's plain runs and Aethyrforge
  call: route (a remote or hosted door needs the consent gate, which is
  Coder's `gate.coax` `gt_session_to_so` today and moves with it), start the
  picked engine with the options Coder gives it (Claude `cd_claude_options`
  through `e_own`; Codex Coder's `CxSettings` through `cxe_own`; Cursor
  `cue_launch`, `cue_resume`, `cue_send`; Microcoder and Psionic `mcw_start`
  and Coder's `McTask`), answer approvals as an unattended run does (deny,
  or an owner's answer), write each engine's events as the lines `fc_read`
  reads (`session`, `route`, `model`, `user`, `reply_delta`/`reply_end`,
  `reply`, `tool`, `result`, `ask`, `diff`, `end`) and the session log with
  its `engine_session` lines (so `sf_fork` resumes). That glue is App-bound
  in Coder today: `coder.coax` `cd_claude_*`, `codex.coax`, `cursor.coax`,
  `microcoder.coax` and `session.coax` `cs_*`. Then `fgn_launch` and the
  reader become runner calls, `chat-launch.coax` (`#authority`) names the
  runner's options instead of argv and is mutated, `linux-native-check.coax`
  reads the fork's parent from the shared log, and `Chat.swift` calls the
  runner through the face (Mac build owed, #13156). Check: Aethyrforge's
  checks and `linux-native-check` with echo and the stand-ins, before and
  after.
- **Also left.** On the Mac the desktop app's session files are still beside
  `settings.json` in Application Support (the Swift host picks the folder);
  on Linux they are under `~/.openagents/desktop/`. Coder writes the log's
  base lines only, so a Coder session loads into a record with each call's
  label but not its kind or id. The Lab's Microcoder route calls host
  functions named `zr_*` (random bytes, the signer), which a host must
  supply as `apps/openagents-desktop/zeron-face.c` does. No live model turn
  was run for any of this: every end-to-end check used a stand-in.

Done when: a grep for the Claude or Codex SDK outside `coax/lib/agent/` finds
only tests.

## Step 3. Shared code for Verse apps

1. **One host per platform.** Linux from Step 1; then Mac, iOS and Android
   moved onto the same contract, replacing the three Mac `main.swift` files
   and the Android C host. A Verse app such as Everglade is then its Coax
   plus a short list of what it needs from the host.
2. **One component system.** Specified in
   [components/spec.md](../../components/spec.md): one set of components in
   `components/`, skins over them, and a library page at
   openagents.com/components. It replaces the three drawing stacks (the
   Zeron components, Aethyr's HUD and Everglade's HUD) and the four Markdown
   implementations.
3. **`/chat` on the shared stack.** `/chat` already draws with the Zeron
   components. What it lacks next to the desktop app: sessions grouped by
   folder and tabs, the shared session record, and access for people who are
   not admins.

Status (2026-11-11, GitHub #13156, on the owner's Mac):

- **Built: the hosts.** `apps/shared/host/apple/ui-host.m` is the Mac and
  iOS host on `ui-host.h` (AppKit or a UIKit view, Metal, the input
  method through NSTextInputClient, pictures through ImageIO);
  `apps/shared/host/android/ui-host.c` is the Android one (Vulkan through
  `verse/gpu/`, offscreen only: no window yet). The component gallery's own
  `main.c` and Coax build on all four (`components/gallery/{linux,apple,android}/build.sh`).
  Its picture at 780 by 1688, 2x, is the same to the pixel in the iPhone
  simulator and an Android 35 emulator.
- **Built: one shader.** The Mac apps draw with
  `components/backend/gpu-shader.coax` emitted as Metal; `Zeron.metal` is
  deleted and the 21 Mac goldens still pass.
- **Kept: CoreText's atlas on the Mac** (components/spec.md, decision 3).
- **Not built:** OpenAgents, Everglade and Aethyrforge still run their own
  `main.swift` hosts. Moving them is the next part: OpenAgents first (its
  Linux `main.c` already runs on the contract and needs a Mac file chooser
  and dictation host in place of the portal and the Linux ones, and a
  CoreText atlas filler in place of the Coax one), then Everglade's
  Workshop (which then gets the component pass Phase 4 needs), with the
  page host for the realms kept as it is, then Aethyrforge. The Android
  window, on the activity's `ANativeWindow`, is not written. Who and when:
  components/spec.md, "Phases".
- **Staged: /chat's server engines.** `coax/web/agent/deploy.sh --staging`
  put the runner with #13153's engines on `oa-agent-staging`. Its first turn
  failed: the checkout of the public repository (1.7 GB) ran the container
  out of memory. The runner now leaves packed data out (about 480 MB), and
  on staging a Haiku turn answered from the checkout and a second turn
  resumed its session. Production is not deployed (NEEDS_OWNER.md).
- **Not built: /chat's folders and tabs and the shared record.** They need
  a change to the `chat` schema (a migration, which is a production deploy)
  and the page. Who may use /chat beyond admins is the owner's decision
  (NEEDS_OWNER.md).

## Housekeeping

- `AGENTS.md`'s table doesn't list `aethyrforge-desktop`, `everglade-desktop`,
  `everglade-mobile` or `shared`.
