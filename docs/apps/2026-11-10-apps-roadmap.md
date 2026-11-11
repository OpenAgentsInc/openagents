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

Status, 2026-11-10: it opens, and slices 1 to 4 below are done except IME
and dictation. `apps/openagents-desktop/linux/build.sh --install --open`
builds it, installs `~/.openagents/bin/openagents-desktop` and opens the
window on `coderos-4080`.

What was checked: `coax/web/tool.sh apps/openagents-desktop/linux/check.coax`
passes 13 cases offscreen. Six are pictures against `goldens/linux/` (the
demo conversation at two sizes, a table, the sidebar closed, no Claude Code
installed, an attachment). Seven are what the app prints: a turn, an
approval allowed and one denied, Claude asking Codex and Codex asking Claude
through the room, a session kept at quit and continued after reopening. They
use the stand-in Claude Code and Codex the Mac's checks use. One real Claude
Code turn also ran offscreen, and the window held 60 frames a second
through a resize. Text is drawn and kerned by Coax (`components/raster/`,
`coax/lib/text/ttf.coax`); the kerning agrees with fontTools on all 2,891
kerned ASCII pairs of Geist.

Not checked: typing into the window with a real keyboard, an input method,
and choosing a file in the desktop's file chooser (the dialog was opened
and closed by a test, never answered). Not built on Linux: dictation,
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
| Dictation audio | AVFoundation | PipeWire; the CLI's Linux dictation already records |

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
serves Linux, Android and any shot check.

Done when: `apps/openagents-desktop/build.sh --open` on `coderos-4080` opens
the window, a message to Claude Code streams an answer, and `check.sh` passes
on Linux.

## Step 2. One agent stack

Today the same job is done several ways:

| Where | What it does | How |
| --- | --- | --- |
| `coax/lib/agent/claude`, `coax/lib/agent/codex` | The SDKs: transport, control protocol, messages | The shared base. Keep |
| `coax/lib/agent/engine/` (moved 2026-11-10 from `apps/openagents-desktop/zeron/`, about 4,700 lines) | Sessions, parts, tool chips, approvals, routing | The shared engine layer over the SDKs; the desktop app is its first user |
| `coder/tui/` (`coder.coax`, `codex.coax`, `session.coax`) | Coder's terminal: the same engines, its own session log | A second engine layer over the SDKs |
| `apps/aethyrforge-desktop/chat.coax` | The Forge's chat | Runs Coder as a child and reads its JSONL |
| `apps/everglade-desktop/next.coax`, `tiers.coax` | Next issue, and Golemancy's issue rating | Calls the Claude SDK directly |
| `apps/everglade-desktop/golems.coax`, `os/verse-agents.coax`, `coax/lib/agent/observe/processes.coax` | Which agents are working on this machine | One walker, `coax/lib/agent/observe/transcripts.coax` (2026-11-10); the third lists processes |
| `coax/web/agent/chat.coax`, `turn.coax` | `/chat` and the project agent on the server | Calls the Claude SDK directly |

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
- **Not built: moves 2, 3, 4 and 6.** Each one changes how a program starts
  and reads its engine, and none can be checked here without a real model
  turn or a stand-in that does not exist yet. What each needs:
  - **The shared Claude engine takes its owner's options.** `e_launch` builds
    the desktop app's options itself (its access levels, the room's tools).
    Coder (bypass or `--ask` with "yes, from now on", the PreToolUse hook for
    its policy and plugins, its own MCP tools, realm servers, an API key,
    fork and resume), Everglade (`next.coax` and `tiers.coax`: their own
    system prompt, tool lists and one in-process tool each) and the server
    (`coax/web/agent/`: confinement hooks, budgets) each need their own.
    The engine has to accept a prepared `ClOptions`, hand hook events and
    permission requests with their suggestions to its owner, and answer any
    in-process tool, not only the room's.
  - **The record keeps what those programs print.** A tool part keeps its
    kind but not the tool's own name (Coder labels a call by it), and text
    is folded only from stream deltas (the server engines run without
    partial messages and read whole assistant frames).
  - **Move 2, Coder's terminal.** Its Claude and Codex paths
    (`coder/tui/coder.coax` `cd_claude_*`, `coder/tui/codex.coax`) print to
    the scrollback straight from SDK messages. On the shared engines they
    would print from the record's new parts. Its checks are 428 unit checks
    that do not run an engine, and the smoke test's engine runs are live.
    Needs: the two engine changes above, then a stand-in run of `coder run
    --json` before and after (the desktop app's stand-ins speak enough of the
    protocol for a plain turn and an approval; `coder/tui/claude_standin.coax`
    covers hooks and tools).
  - **Move 3, Everglade.** `next.coax` and `tiers.coax` call the SDK with
    their own tools. Everglade for desktop is Mac only, so its app cannot be
    run here; its Coax checks can.
  - **Move 4, Aethyrforge.** Its chat runs `coder run --engine auto
    --stream-json` and so uses Coder's router and all six of Coder's engines.
    It can drop the child only after move 2.
  - **Move 6, the server.** `chat.coax` and `turn.coax` each run one turn
    and write the runner's wire events straight from SDK messages. They need
    the whole-frame text fold above, and a deploy to check.
- **Also left.** On the Mac the desktop app's session files are still beside
  `settings.json` in Application Support (the Swift host picks the folder);
  on Linux they are under `~/.openagents/desktop/`. Coder writes the log's
  base lines only, so a Coder session loads into a record with each call's
  label but not its kind or id. The engines call host functions named
  `zr_*` (the clock, random bytes, the Lab's signer), which any new host
  must supply as `apps/openagents-desktop/zeron-face.c` does.

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

## Housekeeping

- `AGENTS.md`'s table doesn't list `aethyrforge-desktop`, `everglade-desktop`,
  `everglade-mobile` or `shared`.
