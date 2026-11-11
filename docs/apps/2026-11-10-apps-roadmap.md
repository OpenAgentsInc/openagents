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

Status, 2026-11-10: it opens. `apps/openagents-desktop/linux/build.sh --open`
builds and opens the window on `coderos-4080`. Slices 1 to 3 below are done,
and from slice 4 saved sessions and settings. What was checked: the demo
conversation and a new chat draw (text and icons from the Coax rasterizers,
`components/raster/`); a turn with the stand-in Claude Code and one real turn
ran offscreen; a session is kept and continued across a quit; the window
holds 60 frames a second through a resize; and
`coax/web/tool.sh apps/openagents-desktop/linux/check.coax` passes its eight
cases against the goldens in `goldens/linux/`. Not checked: typing into the
window with a real keyboard. Not built on Linux yet: kerning and colour
emoji, the folder and file pickers, attachments, the room, dictation, and a
terminal for a notice's Log in or Install button.

The host is `apps/shared/host/linux/` (shared, 300 lines of C over the
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
| `apps/openagents-desktop/zeron/claude.coax`, `codex.coax`, `psionic.coax`, `lab.coax`, `microcoder.coax` (about 4,700 lines) | Sessions, parts, tool chips, approvals, routing | Its own engine layer over the SDKs |
| `coder/tui/` (`coder.coax`, `codex.coax`, `session.coax`) | Coder's terminal: the same engines, its own session log | A second engine layer over the SDKs |
| `apps/aethyrforge-desktop/chat.coax` | The Forge's chat | Runs Coder as a child and reads its JSONL |
| `apps/everglade-desktop/next.coax`, `tiers.coax` | Next issue, and Golemancy's issue rating | Calls the Claude SDK directly |
| `apps/everglade-desktop/golems.coax`, `os/verse-agents.coax`, `coax/lib/agent/local-agents.coax` | Which agents are working on this machine | Three readers of the same transcripts |
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
- **Not built yet:** moves 2, 3, 4 and 6.

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
