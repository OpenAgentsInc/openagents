# Autopilot Roadmap

This roadmap replaces ROADMAP-old and defines the work to evolve Autopilot into a
first-class Codex app-server client while keeping the Adjutant autopilot loop and
local-first UX. The focus is on app-server parity with the CLI so the UI can drive
threads, turns, items, approvals, and reviews directly through the JSONL protocol.

## Goals

Autopilot should treat the Codex app-server as its primary backend and deliver full
coverage of approvals, tool results, and review flows while preserving local
session continuity and the existing Adjutant-driven autopilot mode. The roadmap
prioritizes robust transport, event mapping, and session persistence first, then
builds toward config, auth, MCP, and skills integration that matches the app-server
API surface. Codex runs as a sidecar runtime controlled over JSONL, wrapped by an
AgentRuntime adapter surface (start thread, start turn, stream events, respond to
approvals, stop) so Autopilot can orchestrate other backends later without
rewriting the UI. The event stream doubles as the HUD spine and telemetry bus, with
raw JSONL plus a normalized event log stored as a flight recorder for replay,
evaluation, and dataset capture, and approvals treated as the autonomy control
surface rather than a one-off dialog. The sidecar boundary is intentional for crash
isolation, upgrades, and embedding into other products while keeping an optional
single-binary mode on the table for the future.

## Guiding principles

Codex app-server should be treated as a sidecar runtime that Autopilot supervises over JSONL. That means a clean AgentRuntime
adapter (start thread, start turn, stream events, respond to approvals, stop) and a default posture of spawning a separate
process rather than embedding a library, so crashes are isolated, upgrades are simple, and multi-backend orchestration stays
possible.

The v2 event stream is both the HUD spine and the telemetry bus. Autopilot should preserve item and turn boundaries, capture
raw JSONL plus normalized trace events, and emit enough detail to compute APM-style metrics like tool latency, approval wait
time, and token usage without reparsing raw output. The same flight recorder enables replay, resume, and offline evaluation.

Approvals are the autonomy control surface. Treat approval requests as policy inputs (read-only / propose / auto / escalate),
and make capability discovery first-class by enumerating models, skills, and MCP tools at the start of each run so the UI and
policy layer both see the same live surface area.

## Current State (v0.1 snapshot)

Autopilot already ships as a Winit/WGPU desktop app with streaming Markdown, tool
call cards, a command palette, and a large suite of telemetry panels. The Codex
backend currently streams through codex-agent-sdk rather than the app-server, and
Autopilot mode runs the Adjutant loop with OANIX and DSPy, capped at 10 iterations
with verification enabled. Prompt expansion supports @file and !command, and
session data is stored locally with list/fork/export and checkpoint restore UI.

The remaining gaps are concentrated in app-server parity: Codex thread resume is
not wired, the tool list is empty for the Codex path, and approval requests are not
surfaced to the user. Catalog discovery for agents, skills, hooks, and MCP exists
in the UI but does not yet drive Codex runs, and local session metadata is not
linked to Codex rollouts. There is no runtime adapter boundary yet, approvals lack
policy-driven auto decisions, and app-server runs do not capture a raw + normalized
trace for replay or evaluation.

## Milestones

### M1: App-server transport and lifecycle

Scope: implement a managed process runner that spawns `codex app-server` (or the
`codex-app-server` binary), sets up JSONL read/write loops, and handles request ID
routing without the JSON-RPC version field. The client must perform the initialize
handshake (`initialize` then `initialized`), detect double-init errors, and expose a
typed wrapper for core requests and notifications so higher-level systems are not
stringly typed. This milestone should also define the first concrete Codex runtime
adapter with the minimal lifecycle surface (start thread, start turn, stream events,
respond to approvals, stop), even if other runtimes are not yet wired in.

Acceptance: Autopilot can start the app-server, issue `thread/start`, and receive
`thread/started` and `turn/started` notifications without deadlock. The process
exits cleanly, and the client can shut down without leaving background tasks or
broken pipes.

### M2: Thread, turn, and event mapping to UI

Scope: wire `turn/start` and `turn/interrupt` into the existing query flow and map
app-server events into `ResponseEvent` so the UI renders the same way it does for
codex-agent-sdk. This includes items such as `userMessage`, `agentMessage`,
`reasoning`, `commandExecution`, `fileChange`, `mcpToolCall`, `enteredReviewMode`,
`exitedReviewMode`, `webSearch`, `imageView`, and `compacted`, along with deltas
(`item/agentMessage/delta`, `item/reasoning/*`, `item/commandExecution/outputDelta`,
and `item/fileChange/outputDelta`). `turn/diff/updated`, `turn/plan/updated`, and
`thread/tokenUsage/updated` should feed the existing diff, plan, and usage displays,
and the normalized event stream should be ready to drive HUD panes and telemetry
metrics without additional parsing.

Acceptance: streaming responses and tool outputs render correctly for app-server
runs, with plan updates and unified diffs appearing live without recompute. The UI
should behave consistently across both the Codex SDK stream and app-server stream.

### M3: Approvals and sandbox/permission parity

Scope: connect server-initiated approval requests
(`item/commandExecution/requestApproval` and `item/fileChange/requestApproval`) to
the existing permission dialogs and rule persistence, and apply allow/deny rules to
auto-decide when possible. Coder modes should map to `approvalPolicy` and
`sandboxPolicy`, and the client must respond with accept or decline (including
`acceptSettings` when relevant). This milestone should frame approvals as autonomy
levels (read-only / propose / auto) so policy-driven decisions can be layered in
without reworking the UI. Declined or failed approvals must render final item status
so the user can see what happened.

Acceptance: a dangerous command or file edit triggers a permission dialog, user
choice gates execution, and allow/deny updates persist to disk. Subsequent requests
honor the new rules without restarting the app, and "allow once" vs "allow always"
map to the correct approval response behavior.

### M4: Sessions, history, and review

Scope: adopt `thread/list`, `thread/resume`, and `thread/archive` and connect them
to the session list, resume, fork, and export UI. Store Codex `threadId` alongside
local session metadata so rollouts can be resumed and archived from the UI, and
capture raw JSONL wire logs plus normalized event traces for replay and eval,
including per-run `wire.jsonl` plus an event log that maps into the UI timeline.
This milestone also introduces `review/start` support for inline and detached
reviews and renders entered/exited review items in the chat timeline or a
dedicated view so review output is first-class alongside regular turns. The
rollout artifacts should be usable for deterministic-ish replay and evaluation
harnesses, not just local history recovery.

Acceptance: session list shows Codex rollouts, resume uses `thread/resume`, and
review results appear in the UI in a way that mirrors CLI behavior. Session metadata
should remain consistent between local files and Codex rollouts, and each run has
trace artifacts that can be replayed without re-running the model.

### M5: Config, auth, models, MCP, and skills

Scope: integrate `model/list` to back the model picker and reasoning effort
controls, wire `config/read` and write endpoints for persistent settings, and add
auth flows for `account/read`, `account/login/start`, `account/login/cancel`,
`account/logout`, and rate limit notifications. MCP endpoints
(`mcpServerStatus/list`, `mcpServer/oauth/login`) should drive MCP status and tool
lists, and `skills/list` should feed skill discovery with a refresh path. Map
Autopilot skills to Codex skills or MCP tools so capability discovery happens at
the start of each run and tools feel first-class regardless of where they live.

Acceptance: changing models or settings in the UI updates Codex configuration on
disk, auth state changes are reflected in the UI, and MCP/skill lists populate with
live data from the app-server.

### M6: Autopilot loop and reliability

Scope: route Adjutant autopilot runs through the app-server when Codex is selected,
use `command/exec` for safe one-off validation or prompt expansion, and add
integration tests that cover initialize, turn streaming, and approval flows. Error
handling and telemetry should be hardened so retries and failure states are visible
and recoverable.

Acceptance: Autopilot mode can complete a turn via app-server end-to-end, and test
coverage exists for thread start, turn stream, and approval accept/decline paths.

### M7: Runtime adapters and orchestration

Scope: introduce a runtime adapter interface so Codex is one sidecar runtime behind
a stable boundary, add runtime pooling with per-job isolation, and normalize event
streams into a persistent trace format that feeds HUD metrics and offline evals.
Define an autonomy policy layer (read-only / propose / auto) with a small DSL so
approval decisions can be automated or escalated based on command patterns, path
rules, and network settings, and add runtime pooling plus per-job isolation (e.g.,
dedicated `CODEX_HOME`) to safely run multi-backend workflows that schedule work at
item boundaries (planner/executor/tester style). This is also where multi-backend
orchestration becomes first-class, with Codex acting as planner/executor while
specialized runtimes handle tests or refactors and report back through diffs and
approvals.

Acceptance: Autopilot can select a runtime via the adapter, spin up isolated Codex
instances, persist raw and normalized traces, and apply a policy-driven approval
decision without requiring a UI prompt.

## CodexMonitor Parity Track (Autopilot UI)

This track adds a CodexMonitor-equivalent UI and workflow to Autopilot while preserving
the WGPUI shell and the app-server-first architecture described above. The goal is not
to fork a separate product, but to make Autopilot capable of running the same day-to-day
workspace orchestration pattern: add a repository, start or resume threads, inspect git
changes, approve tool actions, and track agent output with the same visual hierarchy and
interaction flow CodexMonitor established. This work runs alongside M7 because the
multi-runtime adapter and app-server event stream are the spine that will drive the
CodexMonitor layout.

### CM1: Workspace orchestration and session lifecycle

CodexMonitor centers on multi-workspace management, so parity requires a first-class
workspace list, persistent storage, and per-workspace app-server sessions. Autopilot
should implement the same behavior: add a workspace via folder picker, spawn and connect
one app-server per workspace, and restore the thread list on launch or window focus. This
is where we align the app-server transport with UI intent by treating workspace identity
as the top-level routing key for all thread and event data, rather than tying everything
to a single session. The result is a durable workspace index and a consistent resume flow
that mirrors CodexMonitor's "threads by repo" mental model.

### CM2: Layout parity in WGPUI

This section focuses on the visual contract: a left workspace sidebar, central chat or
diff view, right-hand git/approvals panel, top bar with branch and repo info, and bottom
composer. The rendering must translate CodexMonitor's glassy, compact layout into WGPUI
components without losing hierarchy or spacing, which means explicit layout geometry and
panel background treatment rather than repurposing the existing terminal-style layout.
By building a dedicated layout path, we can match CodexMonitor's spatial affordances while
still using our palette and rendering primitives, ensuring parity without abandoning the
Autopilot shell.

### CM3: Event-to-item mapping and timeline behavior

CodexMonitor renders the app-server's v2 event stream as a timeline of messages, reasoning
cards, tools, reviews, and file-change diffs. Parity requires a normalized item model in
Autopilot that mirrors this taxonomy and reacts to streaming deltas by updating items in
place, not just appending new messages. The key outcome is that app-server items drive the
timeline directly, with file-change output rendered as diff blocks and review events shown
as start/complete markers, so the UI faithfully reflects what the agent is doing at each
step. This mapping also becomes the basis for consistent telemetry and replay behavior.

### CM4: Git diff panel and diff viewer

The CodexMonitor right panel doubles as a change inspector. Autopilot should replicate
this by polling git status for the active workspace, summarizing additions/deletions, and
listing changed files with per-file counts. Selecting a file should switch the main view
to a diff viewer that renders unified patches with line numbers and syntax highlighting,
and auto-scrolls to the selected file. This is an explicit UX affordance for verification
and review, so it should remain visible and responsive during ongoing agent output.

### CM5: Composer controls, approvals, and review flow

The CodexMonitor composer provides inline selectors for model, reasoning effort, access
mode, and skills, while the approvals list is always visible in the right panel. Parity
means wiring the access mode selector directly into sandbox and approval policies on each
turn, and funneling approval requests into a visible queue with explicit accept/decline
actions. The `/review` command must behave the same way, including base-branch and commit
targets, with the composer disabled while a review is in progress and a timeline marker
reflecting the review state. Together these controls deliver the same autonomy and review
experience users already rely on in CodexMonitor.

### CM6: Debug panel and operational feedback

CodexMonitor includes a lightweight debug panel that surfaces stderr, warnings, and error
events without requiring a deep dive into logs. Autopilot should add the same collapsible
panel at the bottom of the layout, with copy and clear actions and a top-bar alert toggle
when new errors arrive. This is not a replacement for Autopilot's richer telemetry panels,
but a focused tool for troubleshooting protocol and runtime issues during multi-workspace
operation. Keeping the log capped and actionable preserves the CodexMonitor feel while
reducing friction during development and support.

## Dependencies and risks

The app-server protocol may evolve and require schema regeneration, so the client
must track versions carefully. Codex binary availability and auth requirements are
prerequisites for most flows, and mapping local session metadata to Codex rollouts
must preserve user-visible history without losing data. Codex should remain a
sidecar process rather than an embedded library to keep lifecycle control and
multi-runtime swaps safe, with embedding treated as an optional future distribution
mode rather than the default architecture.

## Out of scope (for now)

This roadmap does not cover multi-user sync or cloud session sharing.
