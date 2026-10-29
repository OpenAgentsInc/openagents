# OpenAgents for Mac, in Coax (spec, draft)

Written 2026-10-28. Status: proposal. Nothing is built yet.

A native Mac app where Claude Code and Codex work side by side in one window,
and can talk to each other. The UI follows the old OpenAgents desktop app (the
Rust/GPUI fork of Zeron that `0b401561a0` removed) closely in layout and
behaviour, but it is drawn with our HUD components, and the app itself (UI and
engine) is Coax.

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

- **Host:** a thin Swift/AppKit shell, `apps/openagents-mac/`. It owns the
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
and drawn with HUD components (thin orange frames, the vector font, black
background).

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

1. **Name and bundle:** is it "OpenAgents" with `com.openagents.mac`, or does
   it keep the old desktop bundle id?
2. **Where Codex sees the room:** a stdio MCP server per session (as above), or
   one long-lived local MCP server shared by all sessions?
3. **Default models:** Opus 5.5 for Claude Code and GPT 6.1 Sol for Codex,
   unless you say otherwise.
4. **Distribution:** a notarized `.dmg` from this Mac only, or the same release
   path as the iOS app?
5. **The ROOM's first use:** is "Claude writes, Codex reviews" the case to make
   smooth first, or something else?
