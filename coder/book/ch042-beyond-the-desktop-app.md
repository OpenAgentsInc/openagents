# 42. Beyond the desktop app

> **Status: unimplemented — design draft.** Chapters 40 and 41 document what
> `apps/coder-desktop/` does today: a native macOS GPU app driving
> Microcoder and Claude Code workers with parallel sub-agents, saved chats,
> background runs and installed skills. This chapter connects that desktop
> app to the rest of the book—and lays out what comes next so Coder grows
> from a single-machine desktop client into the full OpenAgents coding
> surface.

The desktop app proved the core split: a Coax state machine (`page.coax`)
compiled to a Metal carrier, talking over one JSON-lines protocol
(`mcworker-wire.coax`) to isolated worker processes. Everything that
follows builds on that same boundary rather than replacing it.

## 1. One session log across CLI, TUI and desktop

Today, the desktop app stores chats in `index.json` and `<ID>.json` and
replays prior turns into `mcworker` via `MWW_RECALL`, while `openagents
coder` and `coder/tui` write the append-only, hash-chained session log from
[Chapter 2](ch002-model-visible-means-logged.md).

**Next:** unify desktop persistence on the Chapter 2 session log.

- Every desktop turn writes to the same append-only journal (`MWW_JOURNAL`)
  that `coder status`, `coder log`, `coder replay` and `coder resume` read.
- A session started in `coder/tui` or `openagents coder run` appears in the
  desktop sidebar immediately, and a desktop chat can be resumed or forked
  from the terminal.
- `coder.observability.v2` attempt records (`MWW_ATTEMPT`) render directly
  in the desktop UI as per-turn model, token, latency and cost summaries.

## 2. Every engine behind the same worker wire

Today, `App.swift` spawns `mcworker` (Gemini/OpenRouter) and `claudeworker`
(Claude Code). The rest of Part II and Part VIII already implement three
more engines:

- **Codex** ([Chapter 11](ch011-drive-codex.md)) over `codex app-server`,
  with model and reasoning-effort selection.
- **Psionic-served local and Pylon models** ([Chapter 12](ch012-a-psionic-served-model.md))
  for offline, zero-API-cost or LAN-hosted inference.
- **Cursor and Antigravity** ([Chapters 38](ch038-drive-cursor.md) and
  [39](ch039-drive-antigravity.md)) over ACP.

**Next:** wrap each engine behind a thin `mcworker-wire.coax` worker (or
let `mcworker` route to them), and wire [Chapter 13](ch013-choose-an-engine.md)'s
System One router into the desktop composer so "Auto" picks the engine,
model and effort per task while still letting the person override it.

## 3. Sandboxes, approvals, consent and memory in the UI

The wire protocol already carries `MWW_ASK` / `MWW_ANSWER` for command
approvals and `MWW_EMBED` / `MWW_EMBEDDING` for semantic recall, but the
desktop app currently runs workers with permissive defaults and no memory
or consent inspector.

**Next:** surface the safety and memory layers from Parts I and III:

- **Interactive approval cards** ([Chapter 5](ch005-a-sandbox-and-approvals.md)):
  render `MWW_ASK` as an inline diff/command card with Allow Once, Allow in
  Folder, and Deny, backed by OS confinement.
- **Consent ledger** ([Chapter 18](ch018-consent-before-sharing.md)):
  per-chat opt-in before any session content, memory or telemetry leaves
  the machine.
- **Source-linked memory** ([Chapter 19](ch019-remember-a-fact-then-revise-it.md)):
  use `MWW_EMBED` to recall, revise or forget project facts in the sidebar,
  each linked back to the session line that produced it.

## 4. Embedded terminal and multimodal inputs

Coding tasks frequently need to inspect a failing test runner, a TUI, or a
UI screenshot.

- **Embedded VT terminal** ([Chapter 37](ch037-a-terminal-inside-the-agent.md)):
  embed `coax/lib/tui/vt.coax` inside desktop tool cards so long-running
  commands and interactive programs render live inside the conversation
  instead of dumping flat text after exit.
- **Images and attachments:** pass screenshots and design references into
  Microcoder's image support (`coax/lib/microcoder/image.coax`) and
  multimodal gateways directly from drag-and-drop or clipboard paste.

## 5. Always-on daemons, remote machines and OpenAgents Router

Right now, closing `Coder.app` stops its child workers. [Chapter 17](ch017-one-agent-many-tasks.md)
and [Chapter 20](ch020-an-agent-that-stays-online.md) separate the UI from
the long-lived daemon:

- **Attach to `coder daemon`:** `App.swift` connects to a local or remote
  (`--on ssh:DEST` or Pylon) daemon instead of owning every worker process
  directly. Closing the window leaves background jobs running; reopening it
  reattaches to live streams.
- **OpenAgents Router integration:** GitHub issues, CI failures and chat
  threads routed by OpenAgents appear in the desktop sidebar as Worker
  tasks you can watch, steer, approve or take over.
- **Multi-device continuity:** the same session log and daemon protocol
  serve the desktop app, `coder/tui`, web and mobile clients.

## 6. Cross-platform native carrier and built-in measurement

`page.coax` already compiles to C with zero macOS-specific code; only
`App.swift`, `KitEditorMac.swift` and `ui-host.m` are Apple-specific.

- **Linux and Windows hosts:** pair the existing `components/gallery/linux`
  carrier with a native text editor bridge so Linux and Windows get the
  exact same Coax UI and worker binaries.
- **Built-in harness benchmarks** ([Chapter 21](ch021-measure-the-harness.md)):
  run the repository pilot and regression suites from the desktop app,
  comparing engines, models and prompt changes side by side.

## Done means

- [ ] Desktop chats read and write Chapter 2's append-only session log and
  interoperate with `coder resume` and `coder replay`.
- [ ] Codex, Psionic, Cursor and Antigravity run behind the desktop worker
  protocol alongside Microcoder and Claude Code.
- [ ] `MWW_ASK` approvals, Chapter 18 consent gates and Chapter 19 memory
  recall are wired into `page.coax`.
- [ ] `Coder.app` can attach to a local or remote `coder daemon` so runs
  survive closing the window.
- [ ] A Linux desktop build runs the same `page.coax` and worker binaries.
