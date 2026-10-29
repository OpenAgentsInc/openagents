# OpenAgents for Mac, in Coax (spec, draft)

Written 2026-10-28; updated 2026-10-29. Status: `apps/openagents-desktop/`
builds and opens on Zeron's new chat screen, which talks to your own Claude
Code (below). The rest of this spec is still a proposal.

A native Mac app where Claude Code and Codex work side by side in one window,
and can talk to each other. The UI follows the old OpenAgents desktop app (the
Rust/GPUI fork of Zeron that `0b401561a0` removed) closely in layout and
behaviour, and the app itself (UI and engine) is Coax.

## The UI is forked from Zeron (2026-10-29)

We are moving off the HUD look. The UI is rebuilt from
[Zeron](https://github.com/zeronsh/zeron) (Rust/GPUI, by Wing, MIT; a local
clone is at `~/work/zeron`), one component at a time, each matched pixel for
pixel against a render of Zeron itself. Based on Zeron by Wing, MIT.

How it works:

- `apps/openagents-desktop/zeron/gpui.coax` is GPUI's drawing model in Coax:
  quads (rounded rect, fill, border), drop shadows (blurred rounded rect) and
  monochrome sprites (glyphs and SVG icons from an atlas), snapped to device
  pixels the way GPUI's `window.rs` snaps them, text placed on quarter-pixel
  x positions with CoreText's advances and kerning. `Zeron.metal` ports GPUI's
  `quad_fragment`, `shadow_fragment` and `monochrome_sprite_fragment`.
- The Swift host (`Sources/Zeron.swift`) is GPUI's platform layer: it
  rasterizes Geist (Zeron's font, bundled under `fonts/` with its OFL) exactly
  as `gpui_macos/text_system.rs` does (four subpixel offsets, the
  font-smoothing dilation picked from the text color's luminance) and Zeron's
  SVG icons at twice their device size, into one atlas, and hands Coax the
  metrics (`zeron-face.c`).
- `zeron/composer.coax` is Zeron's composer (`crates/ui/src/composer.rs` and
  the chips of `pickers.rs`) in its expanded new-chat form, in Zeron Dark on
  the opaque surface, and docked under a chat (one row with the chat's model
  chip until the text needs more). `zeron/screen.coax` is the chat screen:
  the canvas, the transcript and the docked composer, connected to your own
  Claude Code (below).
- `zeron/claude.coax` runs your `claude` CLI through `coax/lib/agent/claude`
  (stream-json both ways, the control protocol), polled without blocking from
  the frame loop: one long-lived session per conversation, cwd `$HOME` or
  `--cwd PATH`. Like the old Backend it injects no key (ANTHROPIC_API_KEY is
  removed), takes PATH from your login shell plus `~/.local/bin` and the
  Homebrew dirs, and shows a chip with the install line
  (`curl -fsSL https://claude.ai/install.sh | bash`, Install runs it in
  Terminal) when claude is missing, or Log in (`claude /login` in Terminal)
  when the run errors as signed out. Events fold into parts and parts into
  transcript rows the way Zeron does (`harness/src/claude/normalize.rs`
  decode_tool_use, `doc/src/parts.rs`, `proto/src/view.rs` chip labels and
  group summaries, `ui/src/transcript.rs` grouping and the working trailer
  with its gradient spinner, flavour word and elapsed time): streaming
  Markdown and thinking, consecutive thinking and tools folded into a group
  whose trailing one stays open while the turn runs, chips for Bash,
  Read/Edit/Write (Edit and Write with a line diff), Grep/Glob, WebFetch and
  WebSearch, Task/Agent, MCP and TodoWrite, results attached, failures in
  red, errors and limits as Zeron's danger chips; rebuilds coalesce at 120
  ms. Sub-agent frames (`parent_tool_use_id`) are not drawn. The composer's
  chips cycle on click (Zeron opens menus): model Haiku, Sonnet, Opus 5.5
  (default Opus, `set_model` mid-session); effort Low to Max (default High,
  applied by restarting with `--resume` before the next message); access
  Full access (allow every tool), Auto-accept edits (`acceptEdits`, ask for
  the rest) or Supervised (ask for every tool), asks shown as an Allow/Deny
  chip. Enter sends; the send button becomes Stop while a turn runs, and Stop
  or Escape sends the interrupt and ends the turn with "Stopped". `--demo`
  (or the shot step `fixture open`) shows Zeron's demo conversation instead.
  Shot steps `send TEXT`, `stop`, `model/effort/access NAME`, `wait S` and
  `waitidle S` drive a live run offscreen. Tests: `coax test --library
  zeron/claude.coax` runs a scrubbed recorded stream
  (`zeron/fixtures/claude-tools.jsonl`, regenerated with `fixture.py`)
  through the mapping.
- Claude | Psionic: a chat runs on Claude (above) or on Psionic, our own
  gpt-oss-20b through the Psionic Responses API (`zeron/psionic.coax`). As in
  Zeron the harness rides on the model chip: its mark (the OpenAgents power
  symbol for Psionic) and model; on an empty chat the model chip cycles Opus,
  Sonnet, Haiku, then gpt-oss-20b; a conversation keeps its mode. Psionic's
  effort chip is Low, Medium, High (the request's `reasoning.effort`); the
  access chip gates its three tools (read_file, list_folder, run_command):
  Full access runs all, Auto-accept edits runs the two that read and asks for
  run_command, Supervised asks for each. The modes and each chip's choices
  are tables in `zeron/modes.coax` (exported as `coaxial_zeron_option_*`,
  `coaxial_zeron_mode*` for a picker; a new mode is a new row). Requests go
  through `coax/lib/gateway/` (`sse.coax`, the event-stream reader;
  `transport.coax`, streaming curl polled each frame, the key passed only
  through curl's environment, never an argument), the first two pieces of
  microcoder's gateway row; the rest of that row is not done. Each turn
  replays the whole conversation (prior reasoning dropped; tool results
  capped at 1200 bytes); past the server's context the oldest turn is
  dropped and the turn retried. 503 `worker_busy` shows "Psionic is busy";
  nothing listening shows the tunnel command; Stop closes the connection.
  Config: `PSIONIC_URL` and `PSIONIC_API_KEY` from the environment or
  `~/work/.secrets/psionic-responses.env` (gitignored, chmod 600), default
  `http://127.0.0.1:8080`. The server: `ssh coderos-4080 'tmux new -d -s
  psionic-responses ~/psionic-responses/serve.sh'` (wraps
  `psionic/serve-gptoss.sh` on port 8080 with the key from
  `~/.config/psionic-responses.env`), then `ssh -N -L 8080:127.0.0.1:8080
  coderos-4080`. gpt-oss-20b needs most of the 4080's 16 GB, so it cannot
  run beside another resident model there. Flags `--mode claude|psionic`,
  shot step `mode NAME`. Tests: `coax test --library zeron/psionic.coax`
  runs recorded streams (`zeron/fixtures/psionic-turns.sse`, recorded and
  turned into `psionic_fixture.coax` by `fixtures/psionic.py`).
- `zeron/transcript.coax` (with `markdown.coax` and `tools.coax`) is Zeron's
  chat transcript (`crates/ui/src/transcript.rs`, `render.rs`'s Markdown and
  code blocks, `changes.rs`'s diff rows) showing a fixed demo conversation,
  Zeron's own (`crates/client/src/demo/transcripts.rs`): user bubbles,
  Markdown (paragraphs, bold, inline code, lists, a fenced code block with
  Zeron Dark's Rust highlighting and its wrap and Copy buttons), and tool
  groups that open to the activity rail and chips (thought, search, read and
  edit with file badges, a failed run, a run still going, a fetch), each chip
  opening to its invocation and its output, thought or diff. Wheel and
  trackpad scrolling (momentum as AppKit delivers it), pinned to the bottom
  until you scroll up; clicks open and close groups and chips; Zeron's hover
  tones and chip chevrons; drag, double- and triple-click selection across
  rows, copied with its line breaks (Copy, Cmd+C, Select All); the code
  block's Copy puts its code on the pasteboard; the content fades under the
  titlebar and the composer as Zeron's EdgeFade does.
- Text editing is Zeron's (`ComposerInput` in `composer.rs`), in Coax: a
  selection with an anchor and a caret; Zeron's keymap (Shift, Alt and Cmd
  arrows, Home/End and Cmd+Left/Right to the logical line, Cmd+Up/Down to the
  document, Up/Down keeping the column, Alt/Cmd+Backspace and Delete, the
  same chords on Ctrl for Z, A, C, X, V); typing or pasting over a selection
  replaces it; undo and redo in typed bursts (700 ms, 200 steps, a paste or
  cut its own step); click, Shift+click, drag, double-click word and
  triple-click line, a drag keeping its unit; caret and selection by
  grapheme, words by UAX #29; scrolling once the text passes 260px; the
  selection drawn as Zeron's (the accent at 0.35, row by row to the box's
  right edge, no caret over it). The host adds Zeron's menu bar (app menu;
  Edit with Undo, Redo, Cut, Copy, Paste, Select All, to which AppKit adds
  Dictation and Emoji & Symbols; Window), the pasteboard, and
  `NSTextInputClient` for IME composition (underlined as GPUI underlines
  marked text), dead keys, the accent popup and the emoji picker. Text the
  atlas lacks (other scripts, accents, emoji) goes through a glyph cache as
  GPUI's does: Coax asks for each new grapheme, the host shapes it with
  CoreText and rasterizes its cells (emoji into a color atlas). Zeron's
  composer has no context menu, so neither does this one.
- The Lab (`zeron/lab.coax`, 2026-10-29) is the third mode beside Claude
  and Psionic: on the model chip after Psionic in an empty chat (or
  `--mode lab`, shot step `mode lab`).
  Each message first goes to our own Elephant, `elephant-1` served by
  `psionic-serve --elephant-gpu` (Psionic chapter 16, `/v1/systemone`) on
  the RTX 4080 worker `coderos-4080`, through the Coax System One client
  (`coax/lib/systemone`, its keyless Psionic door, polled with `so_step`
  from the frame loop). It asks the old router's five questions (agent,
  difficulty, effort, Claude model, local work) with the agents this app
  can run as choices: Claude Code, Psionic (gpt-oss-20b), and Codex when it
  is on PATH. The decision is a group in the transcript, "Routed to Claude
  Code · Sonnet 5.5 · Medium", opening to a "Routed by elephant-1 · N ms"
  chip with every distribution, the round trip and server latency, usage
  and the served identity. Claude Code then runs the message with the
  chosen model and effort, or the Psionic engine (gpt-oss-20b) runs it when
  Elephant picks Psionic; if Psionic is unreachable Claude Code (Sonnet,
  medium) answers after Psionic's error. Codex is not an engine here yet,
  so its messages run on Claude Code and the decision says so. Refusals
  (503 busy, 524 timeout, 413 too long, any other code) and an unreachable
  server are error chips; the unreachable one names the tunnel
  (`ssh -N -L 8790:127.0.0.1:8790 coderos-4080`) and the start command.
  `PSIONIC_SYSTEMONE_URL` names another door, `PSIONIC_SYSTEMONE_KEY` a key
  if one is ever needed (psionic-serve has none). The wire is HTTP through
  the tunnel, not a Nostr decision job: the body is the same NIP-DEC body,
  but the server is loopback-only and one decision at a time, and a relay
  round trip adds latency and a relay to a call the app makes per message.
  A five-question decision takes about 1.8 s on the server (the device
  plan was raised to 512 tokens per question for it).
- The HUD chat, its LAB and the Mammoth router (`chat.coax`, `router.coax`,
  `Router.swift`) stay in the tree and build, unshown.
- Licenses and attribution: `apps/openagents-desktop/zeron/NOTICE`.

Component one, the composer, against Zeron's own render (1320x880 at scale 2,
sidebar collapsed, one project): every pixel of the text, caret, pill, border,
shadow and chips is identical; Zeron's three SVG icons, rasterized here by
CoreSVG rather than resvg, differ at their antialiased edges (0.2% of the
composer region's pixels, at most 24 of 255 in a channel). Not yet drawn: the
project and device selectors above the composer, the titlebar, the
microphone (Zeron hides it while dictation is off), the pickers' menus, and
hover states. Selections against Zeron's render of the same keystrokes (one
row, Shift+Alt+Left twice; three rows, Shift+Up twice): text and selection
identical, pixel for pixel. Not ported from Zeron's input: mentions and
chips, Markdown list continuation on Enter and Tab, dictation, attachments
from pasted files or images.

Component two, the transcript, against Zeron's render of the same demo
(1320x880 at scale 2, `crates/ui/examples/transcript-fixture.rs` in a
throwaway Zeron worktree): collapsed, and expanded (every group open, the
thought, failed run, edit and first run of the second group open) at scroll
0, 500 and the bottom. In the transcript column at most 0.4% of pixels differ
at all and at most 0.01% by more than 32 of 255 (the Rust file icon,
rasterized by CoreSVG, and one middle dot); bubbles, paragraphs, lists, code
block, diff and chips are otherwise identical. Not yet drawn: the titlebar
and the collapsed sidebar's handle, the timestamp and copy actions an entry
shows on hover, the code block's wrap toggle (drawn, not wired), Copy's
checkmark, and approvals; hover states follow Zeron's code but were not
compared against a Zeron render.

Rebuild and look: `apps/openagents-desktop/build.sh --open`; an offscreen PNG:
`build/OpenAgents.app/Contents/MacOS/OpenAgents --shot out.png [--click X,Y] [--type TEXT] [--do STEPS]`,
where STEPS drive the keyboard, mouse, menus and IME (`Sources/Zeron.swift`
`shot`), e.g. `--do "key shift+alt+left ; menu Copy ; click 900,792,2 ; menu Paste"`;
`--do "click 20,20 ; fixture open ; scroll 500"` opens the transcript as the
Zeron capture does.

## What came before

The old app is at `0b401561a0^:apps/openagents/` (read it with
`git show '0b401561a0^:apps/openagents/<path>'`):

- `README.md`, `ARCHITECTURE.md`: engine plus viewport, one binary, headed or headless.
- `docs/research/feature-inventory.md`: the full UI checklist (window, sidebar,
  composer, transcript, pickers, shortcuts). This spec takes its basics.
- `crates/harness/src/{claude,codex}/`: how each CLI was driven and its events
  normalized; `crates/engine`: sessions, logins, model catalogs.

What we keep from it: the shape (an engine that runs the agents and a UI that
views it), the layout, the harness picker, and the habit of normalizing every
agent's events into one transcript. What we drop: Loro, Durable Objects,
WorkOS, sync and Postgres. The first version is local only.

What we already have in Coax:

- `coax/lib/agent/claude/`: the Claude Agent SDK port (the `claude` CLI's
  stream-json protocol, the control protocol, in-process MCP servers), with
  `coax/platform/process-native.c` for child processes.
- `apps/openagents-web/ui.coax` and `font.coax`: the HUD components and the
  vector font. `chat.coax` already draws a conversation, a composer and model
  chips.
- The iOS app's Metal host (`apps/openagents-ios/OpenAgents/Platform/`), which
  draws Coax output natively.

## The app

### Shape

```
AppKit window + Metal  ──  Coax UI (compiled to C)  ──  Coax engine (C)
                                                         ├── claude  (stream-json, stdio)
                                                         └── codex app-server (JSON-RPC, stdio)
```

- **Host:** a thin Swift/AppKit shell, `apps/openagents-desktop/`. It owns the
  window, the Metal layer, input, the clipboard, file dialogs and the menu bar,
  and it calls into Coax compiled to C. It contains no product logic, just as
  `host.js` contains none on the web.
- **UI:** Coax files that write vertex buffers each frame, like the web pages
  do. Compiled to C rather than wasm, linked into the app, and drawn by a Metal
  port of the HUD shader (`ui.wgsl`). One component set serves the web and the
  Mac.
- **Engine:** Coax that runs on its own thread in the same process. It owns
  sessions, child processes, the transcript store and the agent room (below).
  The UI talks to it only through messages, so it can move into a separate
  daemon later without changing that boundary.

### Harnesses

| Harness | How | Coax library |
| --- | --- | --- |
| Claude Code | `claude` with `--output-format stream-json --input-format stream-json` | `coax/lib/agent/claude/` (exists) |
| Codex | `codex app-server`, JSON-RPC 2.0 over stdio (threads, turns, items, approvals) | `coax/lib/agent/codex/` (new) |

Both feed one normalized event model, `coax/lib/agent/events.coax` (new):

- message started, text delta, message done;
- thinking;
- tool call (name, input) and tool result;
- approval asked and approval answered;
- turn done (with cost when the harness reports it) and error.

The transcript only ever draws these events. A third harness later (Cursor,
OpenCode) is a new library that writes the same events.

**Logins are the CLIs' own.** The app never stores an Anthropic or OpenAI
credential. It runs the installed `claude` and `codex` as the user, so their
existing logins work. When a CLI is missing, the app shows the one-line install
command, and when it isn't logged in, the login command, each with a button
that runs it.

### Agents that talk to each other

All sessions in a window share a **room**: an ordered log of messages between
agents, kept by the engine and drawn in the UI. Each agent gets the room as
MCP tools:

| Tool | What it does |
| --- | --- |
| `agents_list` | The other agents in the room: name, harness, model, folder, busy or idle |
| `ask_agent(name, message)` | Sends a message to another agent as a new turn in its session and returns its final answer as the tool result |
| `tell_agent(name, message)` | Queues a message for the other agent's next turn and returns at once |
| `room_read(since)` | The room's messages since a point |

- **Claude Code** gets the tools through an in-process SDK MCP server.
  `mcp.coax` already speaks that protocol.
- **Codex** gets them through a stdio MCP server, the app's own binary run as
  `OpenAgents --mcp-room <socket>`, registered in the thread's config when the
  session starts.

Rules:

- **Visible.** Every message between agents shows in both transcripts and in
  the room, labelled from → to.
- **No courtesy loops.** A reply comes back as the asker's tool result, never
  as a new message, so "thanks" and "got it" exchanges can't bounce back and
  forth. An agent that is mid-`ask_agent` can't be asked by the agent it is
  waiting on (no deadlock), and that call gets an error saying so.
- **The person is in charge.** STOP on any session stops that session and
  cancels every ask it is waiting on. There is no hop or turn limit by
  default. Settings may let the user set one for themselves.
- **Approvals stay with the person.** A tool call that needs approval in one
  harness is asked of the person, even when another agent started the turn.

### The UI, first version

The layout follows the old app's feature inventory §1.1–1.8, cut to the basics
and drawn with the Zeron-fork components (above), not the HUD's. Labels below
in capitals were written for the HUD; they take Zeron's wording as each
component is ported.

- **Window:** 1320×880 (minimum 900×600), inset title bar, traffic lights at
  {14, 15}.
- **Sidebar** (resizable 208–400 px, default 256, collapsible with ⌘S):
  - NEW SESSION;
  - the sessions, newest activity first, grouped by folder;
  - each row shows the title, the harness mark (Claude or Codex), a status dot
    (working, waiting for you, idle, error) and the time.
- **Tabs** across the top for open sessions, plus a **ROOM** tab that shows the
  messages between agents.
- **Transcript:**
  - your messages, and each agent's text as it streams;
  - tool calls folded to one line, opening to show the input and result;
  - approval prompts inline, with ALLOW and DENY;
  - turn cost and time at the end of each turn.
- **Composer:**
  - Enter sends, Shift+Enter adds a new line, and STOP replaces SEND while a
    turn runs;
  - for a new session: the harness (CLAUDE CODE or CODEX), model, folder and
    reasoning effort;
  - an @ mention of another session in the message (for example `@codex-1`)
    sends that part of the message as a `tell_agent`.
- **Status strip** under the composer: working, waiting for approval, or
  idle, so the composer never moves.
- **Shortcuts:**
  - ⌘N a new session, ⌘W closes the tab, ⌘1–9 picks a tab;
  - Shift+↑/↓ moves through the sessions;
  - ⌘. stops the session.

**Not in the first version:** the Changes pane, the terminal, Git and pull
requests, browser previews, voice, sync between devices, the phone, plugins,
and harnesses other than Claude Code and Codex.

### Storage

Data is local, under `~/Library/Application Support/OpenAgents/`:

- `sessions/<id>.jsonl`: the normalized events of one session, appended as they
  arrive. Each harness's own session id is kept, so a restart resumes the
  session (`--resume` for Claude, `thread/resume` for Codex).
- `room.jsonl`: the room's messages.
- `settings.json`: window, sidebar width, shortcuts and defaults.

No account is needed. A Nostr identity (the same one the phone uses for
`/login`) comes later, for sync and for openagents.com projects.

## Milestones

Each milestone ends with a check that someone else can run.

| # | Milestone | Done when |
| --- | --- | --- |
| M0 | Codex app-server client in Coax (`coax/lib/agent/codex/`) | Its tests replay a recorded app-server session byte for byte, and a live smoke runs one turn against the installed `codex` |
| M1 | One event model for both harnesses (`events.coax`) | Recorded Claude and Codex turns produce the same event shapes; tests in Coax |
| M2 | Mac host: window, Metal, HUD shader, input, Coax UI compiled to C | An empty HUD window at 60 fps; a headless capture matches a golden image |
| M3 | Sessions: sidebar, tabs, transcript, composer, one harness per session | A person runs a Claude Code session and a Codex session side by side in one window, with streaming, STOP and approvals |
| M4 | The room: `ask_agent`, `tell_agent`, `agents_list`, `room_read` for both harnesses | Scripted check: Claude asks Codex to review a diff, the answer comes back as its tool result, and the room shows both messages |
| M5 | Resume, settings, missing-CLI and login help, a signed `.app` | Quit and reopen: sessions come back and continue; a fresh Mac without `codex` shows the install line |

Two build rules carry over from the web: no comments ship, and everything
visible is drawn by Coax.

## Open questions for the owner

1. **Name and bundle:** is it "OpenAgents" with `com.openagents.desktop`, or does
   it keep the old desktop bundle id?
2. **Where Codex sees the room:** a stdio MCP server per session (as above), or
   one long-lived local MCP server shared by all sessions?
3. **Default models:** Opus 5.5 for Claude Code and GPT 6.1 Sol for Codex,
   unless you say otherwise.
4. **Distribution:** a notarized `.dmg` from this Mac only, or the same release
   path as the iOS app?
5. **The ROOM's first use:** is "Claude writes, Codex reviews" the case to make
   smooth first, or something else?
