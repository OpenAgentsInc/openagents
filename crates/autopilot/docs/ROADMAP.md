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
API surface. Codex should run as a sidecar runtime controlled over JSONL, with an
adapter boundary that keeps Autopilot free to orchestrate other runtimes later. The
event stream should double as the telemetry spine, with raw JSONL and normalized
events persisted for replay and evaluation, and approvals treated as the autonomy
control surface rather than a one-off dialog.

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
stringly typed.

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
`thread/tokenUsage/updated` should feed the existing diff, plan, and usage displays.

Acceptance: streaming responses and tool outputs render correctly for app-server
runs, with plan updates and unified diffs appearing live without recompute. The UI
should behave consistently across both the Codex SDK stream and app-server stream.

### M3: Approvals and sandbox/permission parity

Scope: connect server-initiated approval requests
(`item/commandExecution/requestApproval` and `item/fileChange/requestApproval`) to
the existing permission dialogs and rule persistence, and apply allow/deny rules to
auto-decide when possible. Coder modes should map to `approvalPolicy` and
`sandboxPolicy`, and the client must respond with accept or decline (including
`acceptSettings` when relevant). Declined or failed approvals must render final
item status so the user can see what happened.

Acceptance: a dangerous command or file edit triggers a permission dialog, user
choice gates execution, and allow/deny updates persist to disk. Subsequent requests
honor the new rules without restarting the app, and "allow once" vs "allow always"
map to the correct approval response behavior.

### M4: Sessions, history, and review

Scope: adopt `thread/list`, `thread/resume`, and `thread/archive` and connect them
to the session list, resume, fork, and export UI. Store Codex `threadId` alongside
local session metadata so rollouts can be resumed and archived from the UI, and
capture raw JSONL wire logs plus normalized event traces for replay and eval. Add
`review/start` support for inline and detached reviews and render entered/exited
review items in the chat timeline or a dedicated view.

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
the start of each run.

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
streams into a persistent trace format that feeds HUD metrics. Define a policy
layer that can auto-approve or defer approvals based on rules, and allow multi-step
workflows to schedule tasks at item boundaries (planner/executor/tester style).

Acceptance: Autopilot can select a runtime via the adapter, spin up isolated Codex
instances, persist raw and normalized traces, and apply a policy-driven approval
decision without requiring a UI prompt.

## Dependencies and risks

The app-server protocol may evolve and require schema regeneration, so the client
must track versions carefully. Codex binary availability and auth requirements are
prerequisites for most flows, and mapping local session metadata to Codex rollouts
must preserve user-visible history without losing data. Codex should remain a
sidecar process rather than an embedded library to keep lifecycle control and
multi-runtime swaps safe.

## Out of scope (for now)

This roadmap does not cover multi-user sync or cloud session sharing.
