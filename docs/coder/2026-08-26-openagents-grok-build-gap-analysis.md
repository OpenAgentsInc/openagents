# OpenAgents CLI and grok-build gap analysis

## Executive summary

OpenAgents has harvested the hardest low-level parts of grok-build that directly
affect answer quality in a terminal: its composer model, streaming Markdown
engine, Unicode-aware wrapping, hyperlink metadata, mouse scrolling, text
selection behavior, and several motion patterns. OpenAgents also has capabilities
that grok-build does not try to provide. One `openagents` binary exposes Coder,
forge operations, issues, projects, computers, traces, providers, plugins, and
other OpenAgents services.

The remaining gap is not one missing renderer or one missing command. grok-build
has a mature interaction shell around its agent. It provides searchable and
selectable scrollback, layered overlays, a command registry with contextual
discovery, session branching and rewind, worktrees, queues, plans, tasks,
notifications, settings, diagnostics, media, and several render modes. Coder has
a deliberately smaller frame around a capable runtime. Its ten local commands,
single transcript view, and mostly compile-time presentation choices expose that
difference.

The comparison supports five conclusions:

1. Preserve the OpenAgents runtime and CLI architecture. Do not replace it with
   grok-build's xAI-specific session, account, or model layers.
2. Build a reusable interaction layer around Coder instead of copying individual
   screens into `tui.rs`. The current frame has reached the point where each new
   overlay or navigation feature otherwise increases coupling.
3. Prioritize interruption, recovery, transcript navigation, permissions, and
   session lifecycle ahead of decorative parity. Those gaps can lose work or
   leave you unable to understand a running turn.
4. Port concepts, tests, and behavior contracts. Do not copy grok-build's
   492,583-line pager wholesale or create OpenAgents versions of xAI product
   surfaces that have no OpenAgents equivalent.
5. Treat current strengths as constraints. Any redesign must retain the unified
   binary, provider-neutral model catalog, OpenResponses transport, ACP tools,
   ATIF export, OpenAgents goals, credit accounting, and the amber interface.

## Scope and method

This audit compares these pinned trees and includes the local persistence work
prepared for OpenAgents CLI `0.0.17`:

- OpenAgents based on `ae16348639`, principally `crates/openagents-cli`, with
  the `0.0.17` persistence changes reviewed as part of this audit.
- grok-build at `07b2f7144fd5c5c9d3dd1966937a87852d2dbdb8`, principally
  `crates/codegen/xai-grok-pager` and its supporting crates.

The audit reads implementation and tests. Screenshots and prior parity claims are
supporting context, not evidence. Counts include Rust source under each primary
crate's `src` directory:

| Measure | OpenAgents CLI | grok-build pager |
| --- | ---: | ---: |
| Source lines | 68,644 | 492,583 |
| Rust test attributes in source and tests | 1,439 | 9,324 |
| Local slash commands | 10 | 71 registered commands, plus aliases and dynamic ACP commands |

The line counts show relative implementation depth, not desired size. The Grok
pager's largest areas are `app` (214,588 lines), `views` (141,015), `scrollback`
(55,829), `slash` (16,613), `acp` (10,371), and `diagnostics` (9,204).
OpenAgents should not reproduce all of them.

This audit does not compare unrelated Phoenix pages or server internals. grok-build
has no homologous forge, Issues API, Projects API, computer service, or Phoenix
application. It compares root CLI commands only where they affect what a Coder
user can reach from the unified binary.

### Status definitions

- **Equivalent**: OpenAgents provides the same user outcome, even if its
  architecture differs.
- **Partial**: OpenAgents provides the core behavior but lacks meaningful parts
  of the Grok experience or safety contract.
- **Missing**: OpenAgents has no corresponding behavior.
- **OpenAgents advantage**: OpenAgents provides a broader or stronger behavior.
- **Intentional difference**: The Grok behavior is xAI-specific, conflicts with
  OpenAgents product decisions, or has insufficient value to port.

## Architectural comparison

### OpenAgents

`crates/openagents-cli/src/cli.rs` defines a single command tree. The Coder front
door enters `coder::interactive`, which owns terminal setup and the event loop.
`coder::runtime` owns the OpenResponses turn, retry, metering, and control events.
`coder::tui` owns most visible state and rendering. `coder::transcript` and the
vendored `coder::markdown` stack render messages. The shared `composer` directory
owns editing, completion, history, and key classification. `coder::commands`
contains the complete local slash-command table and dispatch.

This architecture is compact and understandable. It also concentrates unrelated
concerns in the interactive loop and `CoderUi`: session state, transcript state,
tool animation, selection mode, composer state, scroll position, banners, status,
and command results. The static command table has no abstraction for availability,
aliases, argument completion, modal results, or commands supplied by an ACP agent.

### grok-build

The Grok pager separates its concerns into explicit layers:

- `app` contains the state machine, event loop, actions, agent events, consent,
  session loading, turn completion, mouse routing, and overlay coordination.
- `views` contains independent surfaces for permissions, plans, tasks, timelines,
  session picking, settings, history, questions, status, usage, and other views.
- `scrollback` owns block layout, sticky regions, search, selection, links, tables,
  and viewport behavior.
- `input` normalizes terminal keys, modifiers, mouse events, and line editing.
- `slash` provides a command trait, registry, aliases, fuzzy matching, recent-use
  ranking, availability rules, argument suggestions, and ACP-supplied commands.
- `notifications`, `settings`, `diagnostics`, `headless`, and `minimal` isolate
  cross-cutting behavior that OpenAgents currently handles locally or does not
  provide.

The important architectural gap is therefore a missing interaction domain, not
a missing widget library. OpenAgents needs boundaries for actions, views,
scrollback, commands, and settings before it adds the highest-value parity items.

## Capability matrix

| Area | OpenAgents status | Evidence and consequence |
| --- | --- | --- |
| Unified product CLI | **OpenAgents advantage** | `cli.rs` exposes Coder and OpenAgents service namespaces from one binary. |
| Provider-neutral runtime | **OpenAgents advantage** | Coder resolves lanes against the server model catalog and uses OpenResponses rather than an xAI-only session. |
| ACP tool execution | **Equivalent** | `coder/acp.rs`, `acp_tool.rs`, and `acp_harness.rs` provide the agent/tool bridge. Grok has a larger ACP presentation layer. |
| Streaming response order | **Equivalent** | Coder holds final synthesis until the tool loop completes and renders tool activity before the answer. |
| Markdown | **Equivalent** | OpenAgents vendored nearly the complete Grok Markdown and pager-render core, including streaming checkpoints, tables, syntax, LaTeX, Mermaid text, CJK wrapping, and links. |
| Composer editing | **Equivalent** | Multi-line input, word and line movement, kill operations, persistent history, and path and command completion live under `composer`. |
| Paste correctness | **Equivalent for text** | Bracketed multi-line paste is retained as one prompt. Grok additionally has placeholders and image handling. |
| Trackpad scrolling | **Equivalent** | Coder handles mouse-wheel and trackpad scroll without sacrificing terminal selection. |
| Text selection | **Partial** | Shift-drag delegates selection to the terminal. Grok also owns internal selection, copy actions, link maps, and selection across rendered blocks. |
| Scrollback | **Partial** | Coder supports following, line scroll, and page scroll. It lacks search, jump targets, sticky regions, block navigation, and a scrollbar model. |
| Tool lifecycle | **Partial** | Coder shows active rails, Markdown output, bounded animated previews, and final state. It lacks expandable block inspection and rich per-tool controls. |
| Shell presentation | **Equivalent** | Shell calls use `>` and tool output has distinct, restrained styling. |
| Turn interruption | **Missing or unsafe** | Exit keys leave the session. There is no clearly advertised cancel-current-turn action with a tested settlement contract. |
| Retry visibility | **Partial** | Coder shows waiting and retry states and retries a stalled request. Grok has a broader progress and turn-status model. |
| Permission prompts | **Partial** | The runtime can refuse unsafe actions, but Coder lacks Grok's dedicated consent and permission views, scoped approvals, and remembered policy controls. |
| Agent questions | **Missing** | No structured choice/input overlay matches Grok's question view. Questions must degrade into transcript text. |
| Plan approval | **Missing** | No plan view, approval action, or `/view-plan` equivalent exists. |
| Goals | **OpenAgents-specific, partial UI** | `/goal` manages OpenAgents goal state. Grok instead has plans, tasks, todos, and workflows with richer views. |
| Tasks and todos | **Missing as first-class views** | Tool output may describe tasks, but Coder has no task list, detail view, or todo progress surface. |
| Prompt queue | **Missing** | Grok can enqueue, inspect, edit, and remove prompts while a turn runs. Coder has no durable visible queue. |
| Background prompt | **Missing** | Grok's `/btw` supports a side request without replacing the main interaction. |
| Subagent roster | **Partial** | OpenAgents delegation exists outside and through tools, but Coder lacks Grok's live agent dashboard, roster, status, and catalog views. |
| Workflows | **Partial outside Coder** | OpenAgents has broader automation and delegation concepts, but Coder has no workflow picker, run view, or local slash surface comparable to Grok. |
| Session creation | **Partial** | Coder starts a session and can log out. It lacks `/new` as an in-frame lifecycle operation. |
| Session resume | **Partial** | `--continue` reopens the newest local session for the current directory and `--resume [id]` reopens a selected session. Coder still lacks Grok's picker, title search, previews, forks, and relocation workflow. The in-frame `/resume` command continues to discover foreign coding-agent sessions. |
| Session history | **Partial** | Coder now keeps complete local event logs and restores its transcript. It does not yet provide an in-frame history browser, search index, rename, or delete workflow. |
| Local persistence | **Equivalent for the core record** | Both products use a per-session directory, an atomic summary, and append-only JSONL as the source of truth. OpenAgents additionally materializes an ATIF snapshot. Grok has a persistence actor, locks, caches, and more auxiliary state. |
| Cloud transcript storage | **OpenAgents privacy advantage** | Coder stores transcripts locally by default. `--cloud-history` explicitly opts in to server event and report text. Remote inference still sends prompts to the selected provider; this setting governs durable transcript storage, not inference transport. |
| Session metadata | **Partial** | `/info` shows spend, model, lane, and thread. Rename, delete, share, recap, and transcript commands are absent. |
| Fork and rewind | **Missing** | Coder cannot branch a session, rewind turns, restore repository state, or edit a previous prompt as one transaction. |
| Worktree integration | **Missing in frame** | Grok can create and select worktrees during session workflows. OpenAgents has repository and workspace code but no comparable Coder view. |
| Diff review | **Partial** | `/diff` reports repository changes. It lacks an interactive changed-file picker and review navigation. |
| Model selection | **Partial by design** | Coder selects a lane at launch and shows resolved model information. It lacks an in-session searchable model picker. |
| Reasoning selection | **Partial** | Reasoning is a launch option. Grok exposes in-session effort selection and valid-level discovery. |
| Mode switching | **Intentional difference** | Coder uses named server lanes. Grok exposes auto, approval, compact, multiline, Vim, full-screen, and minimal modes. Some are presentation settings, not model lanes. |
| Command discovery | **Partial** | `/help` and prefix completion share a static table. Grok has a fuzzy, contextual, MRU-ranked dropdown with aliases, tags, arguments, and dynamic commands. |
| Command extensibility | **Missing** | Coder cannot merge built-ins, plugins, ACP commands, and server capabilities into one registry. |
| File mentions | **Partial** | Composer completion supports paths and `@` mentions. Grok adds richer match ranking and visual suggestion rows. |
| Search | **Missing** | No `/find` or incremental transcript search exists. |
| Jump and timeline | **Missing** | No message/block jump view or chronological tool/turn timeline exists. |
| Copy | **Partial** | Terminal selection copies text. There is no semantic copy command for the last answer, code block, message, or transcript. |
| Links | **Partial** | Markdown retains OSC 8 links. There is no keyboard link picker or link-number navigation. |
| Image input | **Missing** | No clipboard image, dropped image, or image attachment flow exists. |
| Image and video generation | **Intentional difference until supported** | Grok's `/imagine` commands are tied to its product APIs. Add them only through an OpenAgents media service contract. |
| Inline media | **Missing** | Coder does not negotiate Kitty/iTerm graphics or provide a fallback media block. |
| Voice | **Missing** | Grok has voice controls and a voice module. OpenAgents has voice/cloud work elsewhere, but not a Coder interaction. |
| Mermaid text diagrams | **Equivalent** | The vendored Markdown renderer produces terminal diagrams. Raster rendering is intentionally absent. |
| Notifications | **Missing** | Grok supports terminal title, focus, hooks, tmux, sleep handling, and progress notifications. Coder gives no out-of-focus completion signal. |
| Terminal title | **Missing** | Coder does not maintain a useful session/turn title. |
| Theme choice | **Intentional difference** | Coder has one amber identity. Grok's theme picker should not be copied unless OpenAgents decides to support themes. |
| Color degradation | **Partial** | The amber renderer can override lower-color and `NO_COLOR` behavior. Accessibility and terminal-capability behavior need explicit contracts. |
| Settings | **Missing as a system** | Coder has flags and constants, not a typed persistent settings registry or settings view. |
| Keybinding discovery | **Partial** | `/help` lists keys. There is no searchable shortcuts view or contextual key hints. |
| Key rebinding | **Missing** | Neither current Coder architecture nor its old input package provides a suitable terminal action map. |
| Vim input | **Missing, optional** | Grok supports a Vim mode. This should follow an action/keymap abstraction, not precede it. |
| Compact and full-screen modes | **Missing** | Coder has one full-frame layout. Grok supports inline, full-screen, compact, and minimal surfaces. |
| Headless execution | **OpenAgents advantage** | The unified CLI, delegation, and ACP tooling support noninteractive work beyond the Coder frame. |
| Export | **Equivalent with different contract** | Coder exports ATIF, a structured agent transcript. Grok also provides human-oriented transcript/export/copy/share paths. |
| Import | **Missing, low priority** | Grok imports Claude sessions. OpenAgents resumes foreign sessions but does not present a migration/import workflow. |
| Account lifecycle | **Partial** | `/login` and `/logout` exist. Grok additionally has account/home/privacy/announcements surfaces tied to its product. |
| Credit reporting | **OpenAgents-specific** | Coder reports spendable credits only, by product decision. Grok usage and subscription views are not equivalent contracts. |
| Diagnostics | **Partial** | OpenAgents has root diagnostic and trace commands. Coder lacks an in-session doctor, terminal inspection, debug HUD, event log, and support bundle. |
| Logging | **Partial** | Dev server startup and runtime errors are more visible than before, but there is no unified, user-addressable session log view. |
| Telemetry and traces | **OpenAgents advantage outside frame** | The root CLI exposes traces and OpenAgents receipts. Coder should link to these rather than reimplement them. |
| Terminal compatibility | **Partial** | Coder handles raw mode, bracketed paste, Kitty keys, mouse, and PTYs. Grok has a larger terminal-support and doctor matrix. |
| macOS modifiers | **Partial** | Grok isolates macOS modifier normalization. Coder handles required chords but has less platform-specific coverage. |
| Windows support | **Partial and release-dependent** | The release matrix includes Windows, but interaction behavior needs PTY-backed verification per target terminal. |
| Real-terminal tests | **Partial** | OpenAgents has PTY integration coverage, but many rendering assertions still use test backends. Grok's much larger harness covers more resize, mouse, overlay, and terminal cases. |
| Accessibility | **Missing as an explicit contract** | Selection and opacity have been tuned visually, but reduced motion, `NO_COLOR`, contrast, screen-reader-friendly output, and keyboard-only flows lack acceptance tests. |
| Tutorial and onboarding | **Partial** | The centered welcome card explains startup context and disappears after the first message. Grok has an interactive tutorial and contextual tips. |
| Release notes and announcements | **Intentional difference** | These belong to OpenAgents' update or web surfaces unless evidence shows in-session value. |
| Easter eggs | **Do not port** | Grok's hidden game is not a product gap. |

## Detailed findings

### Turn control and correctness

Coder's most important remaining gap is explicit turn control. A long or looping
turn needs separate actions for canceling the current request, clearing queued
prompts, and leaving the application. The action must cancel the transport,
terminate or detach child tools according to policy, settle the thread and credit
receipt, retain completed transcript entries, and return focus to the composer.
The current exit-oriented key handling does not make that contract visible.

Grok models turn completion, progress, agent events, and cancellation as app
actions. OpenAgents should adopt that separation. It should not embed more
special cases in the terminal event loop.

Required acceptance cases:

- Cancel before the first model event.
- Cancel while assistant text streams.
- Cancel while one tool runs and while multiple tools run.
- Cancel during the three-second waiting state and during retry backoff.
- Receive a late event after cancellation without resurrecting the turn.
- Settle credits and server thread state exactly once.
- Exit while a turn runs with an explicit, tested policy.

### Transcript and scrollback

OpenAgents has strong text rendering but a weak document model. `CoderUi.entries`
and wrapped transcript output are sufficient for sequential reading. They are not
enough for search, block selection, folding, sticky headers, link navigation, or
stable positions across resize.

Grok's `scrollback` package demonstrates the appropriate boundary: semantic
blocks own identity and layout; the viewport owns positions; selection and search
refer to semantic content rather than terminal cells. OpenAgents should create a
smaller equivalent around existing `Entry`, `ToolCall`, and Markdown output.

The first useful slice should provide:

1. Stable IDs for user, assistant, system, and tool blocks.
2. A viewport anchor that survives width changes and streaming updates.
3. Search with next and previous results.
4. Block expand and collapse for tool output.
5. Semantic copy for a block, code fence, last answer, or complete transcript.
6. Keyboard link traversal based on the existing hyperlink metadata.

Do not replace terminal-native selection. Semantic selection should complement

### Commands and discovery

`coder/commands.rs` is a good single source of truth for ten commands. It cannot
scale to the command surface already implied by OpenAgents. Grok's command system
supports a trait, aliases, availability, mode support, required tools, argument
suggestions, dynamic ACP commands, fuzzy matching, and MRU ranking.

OpenAgents should introduce a registry before adding many commands. A command
descriptor should declare:

- Canonical name and aliases.
- Short description and usage.
- Availability predicate based on authentication, tools, server capabilities,
  repository state, and active turn state.
- Whether the command is local, an OpenAgents CLI adapter, an ACP command, or a
  plugin command.
- Argument parser and completion source.
- Result type: transcript output, action, overlay, asynchronous job, or error.
- Supported render modes.

The registry should adapt useful root commands instead of duplicating their API
clients. For example, Coder can open issue, project, trace, computer, and plugin
views by calling existing OpenAgents modules.

### Sessions, recovery, and repository state

Grok does not use SQLite as its canonical conversation store. Its default
`StorageMode::Local` writes a directory of JSON and JSONL files. `Writeback` is
an explicit remote mode, and the background remote queue treats local files as
authoritative even when a remote flush fails. Grok uses SQLite for derived
search and memory indexes, which it can rebuild from the session record.

The principal Grok files are:

- `xai-grok-shell/src/config/mod.rs`: defines `StorageMode`, defaults to local,
  and resolves the CLI, environment, remote-settings, and default precedence.
- `xai-grok-shared/src/session/mod.rs`: computes a session directory from the
  working directory and session identity.
- `xai-grok-shell/src/session/storage/mod.rs`: owns `summary.json`, canonical
  `updates.jsonl`, derived `chat_history.jsonl`, plans, signals, goals, and
  atomic replacement.
- `xai-grok-shell/src/session/storage/jsonl/mod.rs`: lays out
  `<root>/sessions/<encoded-cwd>/<session-id>/`, enumerates sessions, appends
  auxiliary journals, and uses file locks.
- `xai-grok-shell/src/session/persistence.rs`: serializes all writes through a
  persistence actor. Messages cover ordinary and acknowledged appends, history
  replacement, model and reasoning settings, plans, rewind, goals, workflows,
  feedback, flush, and copy.
- `xai-grok-shell/src/session/chat_persistence.rs`: adapts chat operations to
  the persistence actor instead of allowing callers to write files directly.
- `xai-grok-shell/src/remote/sync.rs`: mirrors local state in writeback mode as
  a best-effort background operation.
- `xai-grok-pager/src/app/dispatch/session/load.rs`: creates a loading state,
  replays persisted history in batches, and then attaches the live session.
- `xai-grok-pager/src/app/session_load_barrier.rs`: prevents live events from
  appearing before already queued replay events.
- `xai-grok-pager/src/app/cli.rs`: defines explicit `--continue` and optional
  `--resume [id]` startup behavior. An ordinary launch starts a new session.

Grok persists more than visible chat. Its directory can contain
`rewind_points.jsonl`, `feedback.jsonl`, `btw_history.jsonl`, workflow state,
compaction state, images, plan state, goal state, and announcement state. Its
`chat_history.jsonl` is a rebuildable acceleration structure; `updates.jsonl`
is the complete event record. This distinction matters because a cache may be
replaced while the source journal remains append-only.

OpenAgents `0.0.17` adopts the smallest complete version of that contract:

```text
~/.openagents/sessions/<percent-encoded-working-directory>/<session-id>/
├── summary.json
├── updates.jsonl
└── trajectory.atif.json
```

- `summary.json` is an atomic catalog row with the session ID, original working
  directory, timestamps, lane, reasoning setting, last model, format version,
  and cloud-history choice.
- `updates.jsonl` is the authoritative append-only record. Each envelope has a
  format version, monotonic sequence, timestamp, event type, and payload.
- `trajectory.atif.json` is an atomic, complete ATIF v1.7 projection. Coder
  refreshes it after completed turns and at exit. `/export` uses the same ATIF
  document builder.

ATIF should not replace the append log. A complete ATIF document requires a
whole-file rewrite after each event. That creates write amplification and makes
the interchange file a poor crash journal. Keeping ATIF as an always-current,
atomic projection provides portable tooling without making a torn JSON document
the only copy of a conversation.

Coder creates a new local session on an ordinary launch. `--continue` loads the
most recently updated session under the current working directory. Bare
`--resume` has the same selection rule, and `--resume <id>` finds a session by
ID even if the repository moved. Replay restores user messages, reasoning,
tools, failures, assistant messages, and the model recorded on each answer. The
same reconstructed wire history becomes the next model request context.

Local persistence and server authority have separate responsibilities. The
local record owns conversation continuity. The server thread still owns grants,
metering, credit receipts, and settlement. By default, Coder does not append
transcript events to the server and replaces conversation-derived settlement
reports with a generic outcome. `--cloud-history` opts in to the previous event
and report-text behavior. It does not make remote inference local: prompts still
travel to the selected remote model unless you choose a local lane.

The candidate intentionally omits SQLite. A search database becomes useful when
Coder has a session picker, title and content search, and enough sessions for
directory scans to be slow. Add it then as a disposable index built from
`summary.json` and `updates.jsonl`, never as a second authority.

The remaining Grok persistence gaps are meaningful but separable:

- Serialize concurrent writers through an actor and add cross-process locks.
- Add a replay barrier before live events can race session restoration.
- Add session picker, title generation, rename, delete, fork, and relocation
  workflows.
- Persist goal, prompt queue, plan, compaction, permission, and repository
  checkpoints when those domains have durable schemas.
- Add rewind semantics only after conversation, credit receipt, tool side
  effects, and Git state can move together without double settlement.

### Permissions, questions, and plans

Transcript prose is not an adequate substitute for structured interaction.
Permission requests need scope, consequences, and an unambiguous decision.
Questions need selectable options and free-form input. Plans need a stable view
and approve, revise, or reject actions.

Build one overlay host and use typed overlay results for all three. The host must
preserve streaming state behind it, handle resize, constrain focus, expose a
keyboard-only path, and render correctly in low-color terminals. Avoid separate
ad hoc loops for each modal.

Approval memory should be narrow. Support one operation, the current turn, or a
named policy scope. Do not copy an unbounded “always approve” switch without an
OpenAgents safety decision.

### Goals, tasks, queues, and delegation

OpenAgents `/goal` is not equivalent to Grok plans and tasks. A goal records the
outcome and lifecycle of a long-running task. A plan describes intended steps. A
todo reports local progress. A queue holds future user prompts. A subagent roster
reports delegated execution. Collapsing these concepts into one list would make
the interface less clear.

Coder should expose each only when the runtime has structured data for it:

- Keep `/goal` backed by OpenAgents goal state.
- Render plan events through a plan view and approval contract.
- Render task or todo events as progress, without pretending they are durable
  OpenAgents issues.
- Add an editable prompt queue for input submitted during an active turn.
- Build the delegation view from actual child-agent events and existing delegate
  APIs, not by parsing assistant prose.

### Input, completion, and media

The shared composer is already one of OpenAgents' strongest harvested parts. Its
next gaps are presentation and attachment semantics, not basic editing.

Add a suggestion view that can show command, file, mention, model, and session
matches with one navigation contract. Preserve Tab for completion. Do not restore
mode cycling on Tab.

For large pastes, placeholdering can reduce visual noise, but the exact submitted
content must remain inspectable and exportable. Image paste requires a typed
attachment in OpenResponses or ACP; converting a file path into unexplained
prompt text would be a false implementation. Voice and generated media likewise
need service contracts before terminal chrome.

### Notifications and background behavior

Coder gives little feedback when its terminal is not focused. A long coding turn
should update the terminal title and notify on completion, refusal, question, or
permission request. Grok's notification package covers focus detection, terminal
protocols, hooks, tmux, and sleep behavior.

OpenAgents can start smaller:

1. Set a concise title containing `Coder`, repository or directory, and state.
2. Emit an OSC notification only when the terminal lacks focus, where supported.
3. Provide a configurable command hook with structured environment variables.
4. Suppress duplicate notifications and never include prompt or answer content by
   default.

### Settings, modes, and accessibility

Coder currently encodes many product choices as constants. Some should remain
fixed: amber is the identity, tool output is compact, and the footer reports
credits and model context. User preferences are different: reduced motion,
notification behavior, mouse reporting, timestamps, multiline send behavior,
history retention, and possibly Vim keys.

Create a versioned settings schema with defaults, validation, migration, and
`openagents config` integration. A settings overlay should edit that schema; it
must not become a second source of truth.

Before adding themes, make the current theme work under `NO_COLOR`, 16-color, and
256-color terminals. Add reduced-motion behavior for active rails, tool scrolling,
and settle fades. Opacity represented by dim terminal color must retain readable
contrast across supported palettes.

### Diagnostics and supportability

The root CLI already has diagnostics and traces. Coder should make them reachable
and attach session context. A `/doctor` view should report, without leaking
secrets:

- Version, target triple, terminal identity, color depth, keyboard protocol, and
  mouse mode.
- API origin, authentication state, catalog reachability, selected lane, and
  resolved model.
- Current directory, repository, worktree, and writable state.
- ACP agents and tools with availability or refusal reason.
- Session and trace identifiers.
- Log and ATIF export locations.

A support export should redact credentials and sensitive environment variables.
The existing trace namespace should remain the canonical deep diagnostic path.

### Tests and verification

OpenAgents' 1,439 tests are substantial. The gap is coverage shape. Rendering
tests prove buffers; unit tests prove command helpers; some PTY tests prove real
process behavior. High-risk terminal interactions still need end-to-end cases.

Build a compact terminal compatibility matrix rather than pursuing Grok's raw
test count:

- macOS: iTerm2 and Terminal.
- Linux: one xterm-compatible terminal and tmux.
- Windows: Windows Terminal.
- Keyboard modes: legacy and Kitty enhanced keys.
- Display modes: true color, 256 color, 16 color, and `NO_COLOR`.
- Input: typing, bracketed multi-line paste, trackpad scroll, wheel scroll,
  terminal-native selection, resize, suspend/resume, and EOF.

Golden frames should cover the welcome card, waiting and retry states, streaming
assistant output, active and completed tools, overlays, wrapped links, Markdown
tables, narrow terminals, and reduced motion. Behavioral PTY tests should assert
what remains after the process exits, including restored raw mode and mouse state.

## What OpenAgents should not copy

The following Grok features are not current OpenAgents gaps:

- xAI subscription, billing, privacy, home, announcement, and release-note
  surfaces.
- xAI personas and exact Grok model names.
- Image and video commands without an OpenAgents media API.
- The hidden game and debug-only novelty commands.
- Multiple cosmetic themes before terminal accessibility works.
- Grok's internal implementation scale or module boundaries where a smaller
  OpenAgents abstraction meets the behavior contract.
- Any global approval mode without a scoped OpenAgents safety design.
- A cloud transcript store enabled without the reader's explicit choice.
- SQLite as canonical chat storage. Use it only for derived search indexes.
- ATIF as an in-place event journal. Keep it as an atomic projection of the
  append-only local record.

## Recommended target architecture

Keep `coder::runtime`, the shared composer, Markdown, OpenResponses, ACP, and the
root CLI service modules. Refactor the frame around them into these domains:

| Domain | Responsibility |
| --- | --- |
| `coder::app` | State reducer and typed actions for input, runtime events, resize, timers, and lifecycle. |
| `coder::views` | Transcript, composer, welcome, permission, question, plan, session, settings, task, and diagnostic views. |
| `coder::document` | Semantic blocks, layout cache, viewport anchors, search, selection, links, and copy targets. |
| `coder::commands` | Registry, built-ins, aliases, availability, argument completion, ACP commands, plugin commands, and root CLI adapters. |
| `coder::settings` | Versioned persistent settings and capability-derived defaults. |
| `coder::notifications` | Title, focus, notification protocol, and completion hooks. |
| `coder::terminal` | Raw-mode ownership, capability detection, input normalization, suspend/resume, and cleanup. |

Do not begin with a directory move. Add each boundary when implementing the
first behavior that needs it, then move existing behavior behind the boundary.
This sequence avoids a large refactor with no user outcome.

## Prioritized roadmap

### P0: Correctness and recovery

1. Add a distinct cancel-turn action with server, tool, receipt, and late-event
   tests.
2. Finish the native-session candidate by adding a session picker and a replay
   race barrier around the durable local record.
3. Add structured permission and question overlays through one overlay host.
4. Add stable transcript block IDs and viewport anchors.
5. Add an in-session doctor and redacted support export.

### P1: Daily navigation and control

1. Add transcript search, semantic copy, link traversal, and expandable tools.
2. Replace the static slash table with a contextual command registry and
   suggestion view.
3. Add an editable prompt queue and clear busy-state behavior.
4. Add session search, new, rename, and delete flows.
5. Add plan display and approval where the runtime emits structured plans.
6. Add terminal title and out-of-focus completion notifications.

### P2: Agent workflow depth

1. Add a live delegation roster and task detail built from typed events.
2. Add worktree creation and selection through existing repository modules.
3. Add model and reasoning pickers sourced from live server capabilities.
4. Add trace and issue adapters that reuse the root CLI modules.
5. Add typed settings, reduced motion, timestamps, and optional Vim input.

### P3: Optional expansion

1. Add session fork and rewind only after repository-state semantics are proven.
2. Add inline image input and output after OpenResponses or ACP carries typed
   attachments end to end.
3. Add voice after the OpenAgents voice service has a stable session contract.
4. Add compact, inline, or minimal render modes when a concrete embedding need
   exists.
5. Add importers for foreign transcripts when they preserve provenance and
   receipts.

## Deletion and simplification opportunities

First-principles parity also identifies code not to grow:

- Keep one Coder implementation in `openagents-cli`. Do not revive the standalone
  `coder-lite` crate or the retired TypeScript UI.
- Keep one composer and one Markdown engine. New views must consume them rather
  than fork them.
- Remove static command dispatch after the registry fully replaces it; do not
  maintain both.
- Reuse root CLI clients for issues, projects, traces, computers, providers, and
  plugins. Coder-specific API clients for the same endpoints would be redundant.
- Represent animations as shared motion primitives with reduced-motion fallbacks.
  Do not add feature-local timer formulas to `tui.rs`.
- Keep provider and product policy out of view modules. Views should consume
  capabilities and typed runtime state.
- Avoid copying Grok views until an OpenAgents runtime event or command requires
  them. Empty architecture prepared for hypothetical features is still waste.

## Completion criteria

This analysis is not a request for visual identity parity or equal source size.
The practical parity target is reached when you can:

- Start, interrupt, recover, resume, and finish a turn without losing state or
  obscuring settlement.
- Find, inspect, select, copy, and navigate any transcript content.
- Understand and control permissions, questions, plans, queued prompts, and
  delegated work through structured views.
- Discover commands and valid arguments based on current capabilities.
- Move among sessions and repository contexts without leaving Coder.
- Receive useful completion signals when the terminal is not focused.
- Diagnose terminal, server, model, tool, and session failures from one support
  path.
- Do all of the above on the supported release targets with accessible fallback
  behavior.

At that point, remaining Grok-only surfaces are product differences, not evidence
that OpenAgents has an incomplete terminal agent.

## Evidence map

Use these files as starting points when implementing or refreshing this audit:

### OpenAgents

- `crates/openagents-cli/src/cli.rs`: unified command tree.
- `crates/openagents-cli/src/coder/interactive.rs`: terminal event loop and
  lifecycle.
- `crates/openagents-cli/src/coder/runtime.rs`: OpenResponses turn and controls.
- `crates/openagents-cli/src/coder/tui.rs`: current frame and visible state.
- `crates/openagents-cli/src/coder/transcript.rs`: transcript wrapping and
  rendering.
- `crates/openagents-cli/src/coder/commands.rs`: local slash commands.
- `crates/openagents-cli/src/session_store.rs`: authoritative local session
  summary, event journal, lookup, and replay adapter.
- `crates/openagents-cli/src/coder/export.rs`: shared ATIF document builder,
  session projection, and `/export` output.
- `crates/openagents-cli/src/coder/goal.rs`: OpenAgents goal behavior.
- `crates/openagents-cli/src/composer`: editing, history, completion, and keys.
- `crates/openagents-cli/tests/coder_interactive_pty.rs`: real-terminal behavior.
- `docs/coder/2026-08-26-coder-lite-tui-inventory.md`: earlier port inventory.
- `docs/grok-cli/animation-inventory.md`: harvested motion inventory.

### grok-build

Pager paths below are relative to `crates/codegen/xai-grok-pager/src` in the
pinned grok-build checkout:

- `app`: state, actions, event loop, consent, sessions, agents, queue, and turn
  completion.
- `views`: overlays and dedicated user workflows.
- `scrollback`: semantic document, selection, search, layout, links, and viewport.
- `input`: terminal input normalization and editing.
- `slash/commands/mod.rs`: complete built-in command registry.
- `slash/command.rs`, `slash/registry.rs`, and `slash/matcher.rs`: command model,
  availability, aliases, dynamic commands, and matching.
- `notifications`: focus, title, protocol, hooks, tmux, and sleep behavior.
- `settings`: typed settings and persistence.
- `diagnostics` and `doctor_cmd`: runtime and terminal supportability.
- `headless` and `minimal`: alternate execution and presentation surfaces.
- `voice`: voice interaction.

Persistence also depends on these paths outside the pager crate:

- `xai-grok-shell/src/config/mod.rs`: local and writeback storage modes.
- `xai-grok-shell/src/session/persistence.rs`: serialized persistence actor.
- `xai-grok-shell/src/session/chat_persistence.rs`: chat persistence adapter.
- `xai-grok-shell/src/session/storage/mod.rs`: canonical and derived files.
- `xai-grok-shell/src/session/storage/jsonl/mod.rs`: directory layout, listing,
  locking, and auxiliary logs.
- `xai-grok-shell/src/remote/sync.rs`: optional remote writeback queue.
- `xai-grok-shared/src/session/mod.rs`: session path construction.
- `xai-grok-pager/src/app/dispatch/session/load.rs`: replay startup.
- `xai-grok-pager/src/app/session_load_barrier.rs`: replay and live ordering.

Re-pin both revisions and rerun the matrix before treating an individual status
as current. The categories and priorities should remain useful even as individual
features move.
