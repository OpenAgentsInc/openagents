# Codex Sub-Agents: Data Model, Protocol, and the TUI-vs-Desktop Rendering Gap

Date: 2026-07-10
Author: Fable (agent), commissioned by Chris.
Subject: How OpenAI Codex models and renders sub-agents, why the terminal TUI
under-renders them relative to the (closed) ChatGPT/Codex desktop app, and the
GPT-5.6-era model-capability changes that landed in the last few days.

Method: read-only static analysis of the reference clone at
`projects/repos/codex/` (fresh 2026-07-10; `HEAD = dffe1f02a3` "Respect model
support for reasoning summaries (#32290)"). Rust paths are relative to
`codex-rs/`. Cross-referenced with the companion
`docs/teardowns/2026-07-10-chatgpt-desktop-app-teardown.md`, which established
that the desktop app ships the same Rust `codex` binary and drives it as a local
`codex app-server` over JSON-RPC (`stdio://`).

Evidence tags: `[code path]` = read directly in this tree; `[commit SHA]` = git
history in this clone; `[inferred]` = reasoned conclusion, not directly asserted
by a source.

---

## 1. Executive summary

- **Codex has a real, first-class multi-agent system.** A parent thread can
  `spawn_agent`, `send_input`/`send_message`, `wait`, `resume_agent`, and
  `close_agent` on child threads. Two generations coexist behind a
  `MultiAgentVersion { Disabled, V1, V2 }` flag `[protocol/src/protocol.rs]`. V1
  models collaboration as per-thread *interaction events*; V2 adds a canonical
  *path-based* `SubAgentActivity` stream and a persisted parent/child topology
  store (`agent-graph-store`).

- **The runtime emits a rich event vocabulary.** The core `EventMsg` enum carries
  ten collaboration-specific variants — `CollabAgentSpawnBegin/End`,
  `CollabAgentInteractionBegin/End`, `CollabWaitingBegin/End`,
  `CollabCloseBegin/End`, `CollabResumeBegin/End` — plus `SubAgentActivity`
  `[protocol/src/protocol.rs:1457-1479]`. A reduction layer
  (`app-server-protocol`) folds those low-level events into a smaller,
  UI-oriented `ThreadItem` model: `ThreadItem::CollabAgentToolCall { tool,
  status, sender_thread_id, receiver_thread_ids, prompt, model, reasoning_effort,
  agents_states }` and `ThreadItem::SubAgentActivity { kind, agent_thread_id,
  agent_path }` `[app-server-protocol/src/protocol/v2/item.rs:335-363]`.

- **Both surfaces consume the same protocol.** The terminal TUI is *itself an
  app-server client* — it does not embed core directly; it drives an
  `AppServerClient` over JSON-RPC exactly like the desktop app
  `[tui/src/app_server_session.rs:1-4,17-20,175-176]`. So the "gap" is **not** a
  protocol-access gap. Both clients receive the identical `ThreadItem` /
  `ServerNotification` stream.

- **The gap is a rendering/topology gap, structural to a terminal.** A terminal
  is a single linear scrollback showing **one thread's transcript at a time**.
  The TUI's own module docs make this explicit: the app "decid[es] which thread
  is currently displayed" and you cycle between agents with a `/agent` picker and
  `Alt+Left/Right` `[tui/src/app/agent_navigation.rs:11-19,21-25]`. Sub-agent
  structure is therefore flattened into three degraded forms: (a) one-line
  interleaved history rows ("Started \`path\`", spawn/close summaries)
  `[tui/src/multi_agents.rs:311-327]`; (b) a **bounded, best-effort** `/agent`
  status feed capped at 6 items × 3 lines × 240 graphemes
  `[tui/src/app/agent_status_feed.rs:15-18]`; (c) a spawn-order picker for
  switching the single active view. The parent↔child *graph* that
  `agent-graph-store` and the `rollout-trace` reducer maintain
  (`InteractionEdge`, breadth-first descendant listing) is never rendered as a
  graph in the TUI. A GUI can render the same `ThreadItem` stream as concurrent
  panels / a live tree — hence "you kind of have to use a desktop app for some
  things." `[inferred, from the single-active-thread TUI model vs. the graph
  data available in the protocol]`

- **GPT-5.6 landed 2026-07-09; the last few days of commits are mostly
  model-capability plumbing, and some of it touches spawned agents directly.**
  `#32290` (HEAD) makes reasoning-summary emission a per-model capability and
  **applies the capability of the *final selected model* when a spawned agent
  uses a different model** `[commit dffe1f02a3]`. `#32288` makes "GPT-5.6 Sol"
  the default Bedrock model `[commit 610f09abcf]`. `#32206` "Always send reasoning
  parameters" `[commit d2d00b6632]`, `#32277` "Honor `personality = none`"
  `[commit 09ccae2c07]`, and `#32274` "Remove the personality migration"
  `[commit c0ea3c4d0a]` reshape how instructions/reasoning are built — and
  because a sub-agent's `agent_role` can override model + reasoning effort, these
  capability flags flow through the spawn path per child.

---

## 2. The sub-agent data & lifecycle model

### 2.1 A sub-agent is a child *thread*, spawned by a tool call

Codex does not have a separate "agent" object distinct from a conversation. A
sub-agent **is a thread** with a recorded spawn relationship to its parent. The
five collaboration tools are enumerated identically in core and in the
app-server projection:

```rust
// app-server-protocol/src/protocol/v2/item.rs:1037
pub enum CollabAgentTool {
    SpawnAgent,
    SendInput,
    ResumeAgent,
    Wait,
    CloseAgent,
}
```

Handlers live under `core/src/tools/handlers/multi_agents/` (`spawn.rs`,
`send_input.rs`, `wait.rs`, `resume_agent.rs`, `close_agent.rs`) for V1 and
`core/src/tools/handlers/multi_agents_v2.rs` (+ its `spawn`, `send_message`,
`wait`, `list_agents`, `interrupt_agent`, `followup_task` submodules) for V2
`[code paths]`.

`spawn_agent` is a normal model tool. Its search blurb is the clearest statement
of intent: `"spawn_agent spawn agent subagent sub-agent delegate delegation
parallel work worker explorer no-apps fork model reasoning"`
`[core/src/tools/handlers/multi_agents/spawn.rs:33]`. Spawn enforces a depth
limit — `next_thread_spawn_depth` / `exceeds_thread_spawn_depth_limit` against
`turn.config.agent_max_depth`; on overflow the model gets *"Agent depth limit
reached. Solve the task yourself."* `[spawn.rs:62-70]`. A child may inherit or
override the parent's model and reasoning effort, and an `agent_type` (role)
maps to `apply_role_to_config`, which rewrites the child's config
`[spawn.rs:1-9,53-58]`.

### 2.2 Lifecycle status

Each agent carries a lifecycle status derived from its emitted events:

```rust
// protocol/src/protocol.rs:1708
pub enum AgentStatus {
    PendingInit,
    Running,
    Interrupted,
    Completed(Option<String>),  // final assistant message
    Errored(String),
    Shutdown,
    NotFound,
}
```

The app-server projects this to a UI-facing `CollabAgentState { status:
CollabAgentStatus, message: Option<String> }` where `CollabAgentStatus` is the
same set minus the payloads (`PendingInit … NotFound`)
`[app-server-protocol/src/protocol/v2/item.rs:1181-1197]`. The `Completed(msg)` /
`Errored(msg)` payloads are unwrapped into the separate `message` field, so
downstream UIs get both a discrete status enum and an optional human string.

### 2.3 Parent↔child tracking: `agent-graph-store`

Parent/child topology is a persisted, storage-neutral concern owned by the
`agent-graph-store` crate — deliberately separate from rendering:

```rust
// agent-graph-store/src/types.rs:6
pub enum ThreadSpawnEdgeStatus { Open, Closed }
```

The `AgentGraphStore` trait exposes a full graph API: `upsert_thread_spawn_edge`,
`set_thread_spawn_edge_status`, `list_thread_spawn_children`, and
`list_thread_spawn_descendants` (documented "breadth-first by depth, then by
thread id") `[agent-graph-store/src/store.rs:18-58]`. Each child has *at most one*
persisted parent. Consumers include `thread_manager.rs`, `agent/control.rs`, and
the multi-agent tool handlers `[grep across core/src]`. **This is a real tree
with depth and BFS traversal** — the data model to render an agent org chart
already exists; it is simply not surfaced as one in the terminal.

### 2.4 V1 vs V2

`MultiAgentVersion { Disabled, V1, V2 }` `[protocol/src/protocol.rs:2998]`
selects the collaboration surface:

- **V1** records collaboration as directional *interaction events* keyed by
  sender/receiver thread ids (`CollabAgentInteractionBegin/End`, spawn/close/
  resume begin/end). The `rollout-trace` reducer turns these into `InteractionEdge`
  values with `InteractionEdgeKind::{AssignAgentTask, SendMessage, …}`
  `[rollout-trace/src/reducer/tool/agents.rs]`.
- **V2** adds a canonical **path-based** identity (`AgentPath`) and a dedicated
  `SubAgentActivity` stream (`Started / Interacted / Interrupted`) emitted via
  `emit_sub_agent_activity` → `TurnItem::SubAgentActivity`
  `[multi_agents_v2.rs:43-51]`. V2 is what the `agent_path` fields throughout the
  UI reference. The reducer comments note V2's ordering subtlety: a sender tool is
  recorded "before the target thread necessarily includes the delivered mailbox
  message in a model-visible request," so it keeps a `PendingAgentInteractionEdge`
  until the recipient item appears `[rollout-trace/src/reducer/tool/agents.rs:23-45]`.

---

## 3. The event / protocol surface

### 3.1 Low-level: core `EventMsg`

The runtime's `EventMsg` enum is the wide surface. Collaboration variants
`[protocol/src/protocol.rs:1457-1479]`:

```rust
CollabAgentSpawnBegin(CollabAgentSpawnBeginEvent),
CollabAgentSpawnEnd(CollabAgentSpawnEndEvent),
CollabAgentInteractionBegin(CollabAgentInteractionBeginEvent),
CollabAgentInteractionEnd(CollabAgentInteractionEndEvent),
CollabWaitingBegin(CollabWaitingBeginEvent),
CollabWaitingEnd(CollabWaitingEndEvent),
CollabCloseBegin(CollabCloseBeginEvent),
CollabCloseEnd(CollabCloseEndEvent),
CollabResumeBegin(CollabResumeBeginEvent),
CollabResumeEnd(CollabResumeEndEvent),
SubAgentActivity(SubAgentActivityEvent),   // "Path-based v2 sub-agent activity."
```

The spawn-end event is the richest — it carries the resolved child identity and
its *effective* runtime config after inheritance/role overrides:

```rust
// protocol/src/protocol.rs:4181 CollabAgentSpawnEndEvent (excerpt)
new_thread_id: Option<ThreadId>,
new_agent_nickname: Option<String>,
new_agent_role: Option<String>,       // alias: agent_type
prompt: String,                        // may be empty to avoid CoT leakage
model: String,                         // effective model after inheritance/role
reasoning_effort: ReasoningEffortConfig,
status: AgentStatus,
```

`SubAgentActivityEvent` is the compact v2 signal: `{ event_id, occurred_at_ms,
agent_thread_id, agent_path, kind: SubAgentActivityKind }`
`[protocol/src/protocol.rs:4255]`. `SubAgentSource::ThreadSpawn { parent_thread_id,
depth, agent_path, agent_nickname, agent_role }` records provenance on the child
side `[protocol/src/protocol.rs:2797]`.

### 3.2 UI-level: `ThreadItem` (the reduction layer)

`app-server-protocol` collapses those ~11 low-level events into **two** UI item
variants via `thread_history.rs` (`handle_sub_agent_activity`, plus one
`CollabAgentToolCall` upsert per tool per begin/end)
`[app-server-protocol/src/protocol/thread_history.rs:364,874-1109]`:

```rust
// app-server-protocol/src/protocol/v2/item.rs:335
CollabAgentToolCall {
    id, tool: CollabAgentTool, status: CollabAgentToolCallStatus,
    sender_thread_id: String,
    receiver_thread_ids: Vec<String>,
    prompt: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    agents_states: HashMap<String, CollabAgentState>,   // last-known per target
},
SubAgentActivity { id, kind: SubAgentActivityKind, agent_thread_id, agent_path },
```

`CollabAgentToolCallStatus` collapses the ten begin/end events into a
three-state lifecycle `{ InProgress, Completed, Failed }`
`[item.rs:1131]`. **This is the shared contract both the TUI and the desktop app
consume.** The desktop app, per the companion teardown, receives this same
`ServerNotification::{ItemStarted, ItemCompleted}` stream over the app-server
JSON-RPC transport `[docs/teardowns/2026-07-10-chatgpt-desktop-app-teardown.md]`.

---

## 4. TUI rendering — what renders, what degrades, and why

### 4.1 The TUI is an app-server client, not an embedded engine

`tui/src/app_server_session.rs` is the whole story: it holds an
`AppServerClient` and issues typed JSON-RPC (`ClientRequest`, `ModelListParams`,
`ReviewStartParams`, …) `[app_server_session.rs:17-49,175-176]`. The file's own
doc: *"App-server session facade used by the TUI event loop … owns the typed
JSON-RPC calls needed by the TUI"* `[app_server_session.rs:1-4]`. So the TUI has
**full access to the rich sub-agent protocol**. Whatever it fails to show is a
rendering choice, not missing data.

### 4.2 What the TUI *does* render (three degraded projections)

**(a) Interleaved one-line history rows.** `multi_agents::tool_call_history_cell`
matches `ThreadItem::CollabAgentToolCall` and, per `CollabAgentTool` arm, emits a
single `PlainHistoryCell` — `spawn_end`, `interaction_end`, `resume_begin/end`,
`waiting_begin/end`, `close_end` `[tui/src/multi_agents.rs:206-279]`. Notably,
`SpawnAgent`, `SendInput`, and `CloseAgent` **return `None` while
`InProgress`** — i.e. an in-flight spawn renders *nothing* until it resolves
`[multi_agents.rs:226-228,239-241,271-273]`. `SubAgentActivity` renders as a
terse titled line:

```rust
// tui/src/multi_agents.rs:311
SubAgentActivityKind::Started    => format!("Started `{agent_path}`"),
SubAgentActivityKind::Interacted => format!("Interacted with `{agent_path}`"),
SubAgentActivityKind::Interrupted => format!("Interrupted `{agent_path}`"),
```

Child *content* (the sub-agent's own reasoning/messages/tool calls) does not
appear inline in the parent transcript — only the edge events do.

**(b) The `/agent` status feed — explicitly bounded.** `agent_status_feed.rs` is
headed *"Bounded, best-effort previews for the v2 `/agent` status output"* and
renders a "Sub-agents running" block. It is hard-capped:

```rust
// tui/src/app/agent_status_feed.rs:15
const AGENT_STATUS_PREVIEW_LINES: usize = 3;
const AGENT_STATUS_PREVIEW_ITEMS: usize = 6;
const AGENT_STATUS_PREVIEW_GRAPHEMES: usize = 240;
```

Empty state is literally `"  • No sub-agents running."`
`[agent_status_feed.rs:40]`. Each thread shows its `agent_path` and up to 6
recent activity summaries truncated to 240 graphemes over 3 lines. This is a
*peek*, not the child's transcript.

**(c) Spawn-order picker + single active view.** `agent_navigation.rs` owns a
first-seen spawn-order list and `Alt+Left/Right` cycling; per-status the picker
renders a green/dim dot and a `nickname [role]` label
`[tui/src/multi_agents.rs:73-101]`, mapping `CollabAgentStatus` to colored spans
(`Running` → bold cyan, `Errored` → red, etc.) `[multi_agents.rs:621-646]`. The
module's stated division of labor is the crux: **`App` decides "which thread is
currently displayed"** `[agent_navigation.rs:14]`. There is exactly one displayed
transcript.

### 4.3 What degrades and why

The degradation is not any single dropped `match` arm — the TUI actually handles
every `CollabAgentTool` and `SubAgentActivityKind` variant. The degradation is
**dimensional**: a terminal transcript is 1-D (a single vertical scrollback for a
single active thread), but the sub-agent model is a 2-D+ tree of concurrently
live threads, each with its own full transcript. The TUI therefore must project
the tree onto a line:

- **No concurrent transcripts.** You watch one agent; the others become one-line
  edge summaries and a 6-item preview. To read a child's actual work you *switch*
  to it (losing the parent view), because there is nowhere to put a second
  scrollback.
- **In-progress spawns are invisible** until they complete (the `InProgress →
  None` arms above), so a slow child shows no live output in the parent stream.
- **The graph is never drawn.** `agent-graph-store` has depth + BFS descendant
  traversal and `rollout-trace` builds `InteractionEdge`s, but the TUI has no
  tree/graph widget — only a flat spawn-order list. Parent→child→grandchild
  structure (depth is right there in `SubAgentSource::ThreadSpawn.depth`) is not
  visualized.
- **Bounded previews truncate.** 6×3×240 is a deliberate ceiling to protect the
  terminal from flooding; a GUI has no equivalent pressure.

These are rational terminal-UI compromises, not bugs — but they are exactly the
"sub-agent stuff doesn't render well in the CLI" symptom.

---

## 5. The TUI-vs-desktop gap and its cause

**Cause: identical protocol, different rendering dimensionality — not a
capability/permission split.** `[inferred, strongly supported]`

1. Both clients speak the same app-server JSON-RPC and receive the same
   `ThreadItem` stream. The TUI proves the "thin client over `codex app-server`"
   pattern in-tree `[tui/src/app_server_session.rs]`; the desktop app runs `codex
   app-server --listen stdio://` per the companion teardown.
2. The protocol already carries everything a rich UI needs: per-target
   `agents_states: HashMap<thread_id, CollabAgentState>`, effective `model` /
   `reasoning_effort` per spawn, canonical `agent_path`, nicknames, roles, depth,
   and a persisted parent/child graph with BFS descendant queries.
3. The **terminal** can only foreground one thread's scrollback, so it *must*
   flatten: interleaved edge rows + a capped status peek + switch-to-view
   navigation. A **windowed GUI** can bind the same data to concurrent surfaces
   — side-by-side agent panels, a live spawn tree, per-child streaming
   transcripts, always-visible status chips — with none of the truncation.

So the desktop app isn't privileged; it's *dimensionally unconstrained*. The
same `CollabAgentToolCall` + `SubAgentActivity` + `agents_states` payload that
becomes a 6-line "Sub-agents running" peek in the terminal can become a live
multi-pane agent dashboard in a webview. That is why some multi-agent workflows
feel first-class only in the desktop app. `[inferred]`

**Corollary for us:** the fix is not "make the protocol richer." The protocol is
already rich. The fix is a rendering surface that can hold the topology. (See the
companion design frame.)

---

## 6. GPT-5.6 & model-capability changes (last few days)

GPT-5.6 shipped 2026-07-09; this clone's `HEAD` is 2026-07-10. The recent history
is dominated by model-capability plumbing. The load-bearing ones for reasoning /
personality / sub-agents:

### 6.1 `#32290` — "Respect model support for reasoning summaries" (HEAD, `dffe1f02a3`)

Adds `supports_reasoning_summary_parameter` to model metadata (defaults `true`).
`reasoning.summary` and the summary-delivery `StreamOptions` are now omitted when
the model lacks the capability `[core/src/client.rs]`:

```rust
summary: (model_info.supports_reasoning_summary_parameter
    && summary != ReasoningSummaryConfig::None)
    .then_some(summary),
```

**Sub-agent relevance is explicit in the PR body:** *"Apply the capability of the
**final selected model** when a spawned agent uses a different model."* The added
test `spawned_agent_uses_summary_support_for_final_model` toggles
`model.supports_reasoning_summary_parameter` on a custom spawned child and asserts
the child request honors the child model's capability `[commit dffe1f02a3, tests
in core/src/client_tests.rs & core/tests/suite/spawn_agent_description.rs]`. So a
child spawned with a different model gets summaries gated by *its* model, not the
parent's — directly shaping the streamed reasoning events a sub-agent produces.

### 6.2 `#32206` — "Always send reasoning parameters" (`d2d00b6632`)

Builds a reasoning payload for *every* Responses request and always includes
`reasoning.encrypted_content`; **removes** the older
`supports_reasoning_summaries` model flag and the
`model_supports_reasoning_summaries` config override; drops the capability gate on
reasoning effort (including for guardian reviews and tracing)
`[commit d2d00b6632; touches core/src/client.rs, config/src/config_toml.rs,
core/config.schema.json, guardian/review_session.rs, session/turn_context.rs]`.
Net: reasoning params are now unconditional; **summary** emission is the
remaining per-model toggle (re-introduced more precisely by #32290 above). This is
a two-step refactor landing the same day: #32206 removed a coarse flag, #32290
added a finer one scoped to the summary parameter.

### 6.3 `#32277` — "Honor `personality = none`" (`09ccae2c07`)

Model-catalog base instructions can bake in a `# Personality` section. When
personality support is on and the setting is explicitly `none`, that section is
stripped through the next level-one heading (CRLF-aware), while explicit
`base_instructions` overrides are preserved. Personality is now passed into the
models manager `[commit 09ccae2c07]`. Because a sub-agent's `agent_role` runs
through `apply_role_to_config`, personality resolution is part of the per-child
instruction build.

### 6.4 `#32274` — "Remove the personality migration" (`c0ea3c4d0a`)

Deletes the startup migration that inspected existing sessions to auto-set
`personality = "pragmatic"`; removes `core/src/personality_migration.rs`, the
marker, helpers, and ~849 lines of tests; touches both the TUI and app-server
startup paths `[commit c0ea3c4d0a]`. Personality is now an explicit setting with
no implicit backfill.

### 6.5 `#32288` — "Make GPT-5.6 Sol the default Bedrock model" (`610f09abcf`)

Reorders the static Amazon Bedrock catalog so the GPT-5.6 **Sol / Terra / Luna**
variants rank ahead of GPT-5.5 and GPT-5.4, making **Sol the default**; each
variant uses its bundled description and default reasoning level while retaining
Bedrock's `max` reasoning support `[commit 610f09abcf;
model-provider/src/amazon_bedrock/catalog.rs, model-provider/src/provider.rs]`.
(The companion desktop teardown independently observed this machine's `~/.codex`
pinned to `gpt-5.6-sol`.) Model **defaults** live in the `model-provider` catalogs
and `models.json` (updated repeatedly this window: `#31684`, `#21818`).

### 6.6 Net effect on sub-agent orchestration/rendering

- **Reasoning-summary shape is now per-model and resolved at the child.** A
  spawned agent on a summary-less model emits no summary stream option; the
  parent's `CollabAgentSpawnEnd.model` / `reasoning_effort` already record the
  child's *effective* config, so the projection stays honest. `[inferred from
  #32290 + CollabAgentSpawnEndEvent fields]`
- **Personality/instructions are now explicit and role-scoped**, so per-child
  role overrides (`apply_role_to_config`) determine each sub-agent's instructions
  and personality without a hidden migration.
- **None of these changes touch the `EventMsg`/`ThreadItem` collaboration
  variants** — the sub-agent event *shape* is stable across the 5.6 changes; what
  moved is *which reasoning/personality/model config each thread resolves to*.
  The rendering gap in §4-5 is orthogonal to and unaffected by the 5.6 work.

---

## 7. Implications & observations

1. **The gap is architectural, not a missing feature.** Codex already models
   sub-agents as a persisted tree with rich per-agent state and a clean
   reduced-item protocol. The terminal simply cannot hold a tree of concurrent
   transcripts, so it flattens. Anyone who wants full-fidelity multi-agent
   visibility must use a GUI over the same protocol.

2. **The reduction layer is the leverage point.** `thread_history.rs` collapsing
   ~11 `EventMsg` variants into 2 `ThreadItem` variants + a per-target
   `agents_states` map is a good, stable UI contract. A GUI binds richly to it; a
   TUI truncates it. Any client we build should target the *item* layer, not the
   raw event layer.

3. **`agent-graph-store` is the org chart nobody draws (yet).** Depth + BFS
   descendant traversal already exist. A tree/graph widget is a pure
   presentation add; the data is there.

4. **In-progress invisibility is a real UX cost.** Spawn/send/close render
   nothing while `InProgress` in the TUI; a live GUI would show a spinning child
   panel. This is the most user-visible "doesn't render well" symptom.

5. **For OpenAgents:** our typed-catalog + one-renderer-per-surface premise says
   the *same* sub-agent projection should render honestly on desktop, mobile, and
   web with no capability tier. Codex's split (terminal under-renders, desktop
   over-renders the identical protocol) is the anti-pattern to design against.
   See `docs/teardowns/2026-07-10-openagents-subagents-design.md`.

---

## 8. Appendix — key files

| Concern | Path |
|---|---|
| Collab event variants | `codex-rs/protocol/src/protocol.rs:1457-1479` |
| `AgentStatus`, `SubAgentSource`, `MultiAgentVersion` | `codex-rs/protocol/src/protocol.rs:1708,2797,2998` |
| Spawn/interaction event structs | `codex-rs/protocol/src/protocol.rs:4140-4270` |
| Reduced UI items (`ThreadItem`) | `codex-rs/app-server-protocol/src/protocol/v2/item.rs:335-363,1037-1197` |
| Event→item reduction | `codex-rs/app-server-protocol/src/protocol/thread_history.rs:364,874-1109` |
| Spawn handler + depth limit | `codex-rs/core/src/tools/handlers/multi_agents/spawn.rs` |
| V2 tool surface | `codex-rs/core/src/tools/handlers/multi_agents_v2.rs` |
| Parent/child topology store | `codex-rs/agent-graph-store/src/{store,types}.rs` |
| Interaction-edge reducer | `codex-rs/rollout-trace/src/reducer/tool/agents.rs` |
| TUI app-server client | `codex-rs/tui/src/app_server_session.rs` |
| TUI multi-agent rendering | `codex-rs/tui/src/multi_agents.rs` |
| TUI `/agent` status feed (bounded) | `codex-rs/tui/src/app/agent_status_feed.rs` |
| TUI agent navigation (single active) | `codex-rs/tui/src/app/agent_navigation.rs` |
| 5.6 reasoning-summary capability | commit `dffe1f02a3` (#32290) |
| Always-send reasoning params | commit `d2d00b6632` (#32206) |
| Personality = none | commit `09ccae2c07` (#32277) |
| Remove personality migration | commit `c0ea3c4d0a` (#32274) |
| GPT-5.6 Sol Bedrock default | commit `610f09abcf` (#32288) |
