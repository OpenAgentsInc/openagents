# Plan: the Task tool — Coder Mini as the first delegation target

Date: 2026-08-27
Status: proposed → implementation-ready
Owner direction: Claude-Code-shaped delegation, streaming inline in the chat. One 4-row box per agent; the whole subagent conversation streams through that box until the agent finishes. Fullscreen subagent transcripts are a later surface. Delegation work comes **first** — ahead of the Claude SDK parity ladder except where a step is a strict prerequisite (call out explicitly, Part 5).

Related: #232 (SDK parity ladder, stays open), #228 (acp/delegate consolidation — superseded by this plan), `docs/analysis/2026-08-27-claude-sdk-rust-parity-and-coder-harness-imports.md` (the harness-import list this plan implements first).

Sources: `~/work/projects/repos/cc/tools/AgentTool/` (AgentTool.tsx, UI.tsx, runAgent.ts, prompt.ts, agentToolUtils.ts, built-in/), our `crates/openagents-cli/src/{tools.rs,delegate.rs,runtime.rs}`, `src/coder/{tui.rs,runtime.rs,interactive.rs}`.

---

## Part 1 — what Claude Code's Agent tool actually does

From `~/work/projects/repos/cc/tools/AgentTool`:

**Input schema** (`AgentTool.tsx:83`):
```
description    short 3–5 word task label (the tool-call header text)
prompt         the full self-contained brief
subagent_type  which agent (general-purpose default; Explore; Plan; user-defined)
model          optional override (sonnet/opus/haiku)
run_in_background  optional; auto-backgrounds after 120s by default
isolation      'worktree' → temp git worktree, auto-removed if unchanged
cwd            optional override (mutually exclusive with worktree)
```

**Streaming**: the child's message stream is forwarded through the tool's `onProgress` callback as `agent_progress` events, each wrapping the child's actual `Message` (assistant text, `tool_use`, `tool_result`). The parent UI therefore sees the subagent's *real* tool calls — not summaries — as they happen.

**Inline rendering** (`UI.tsx`): while running, the tool box shows only the **last 3 progress messages** (`MAX_PROGRESS_MESSAGES_TO_SHOW = 3`), collapsing consecutive search/read ops into one dim summary line, and prints `+N more tool uses (ctrl+o to expand)` for the hidden ones. Ctrl+O toggles the full transcript. When the agent settles, the box shows `Done · N tool uses · total duration` plus the final report.

**Result payload** (`agentToolUtils.ts:227`): `{agentId, content:[{type:'text',...}], totalToolUseCount, totalDurationMs, totalTokens, usage{...}}` — what the parent model reads back.

**Built-in agents** (`built-in/`): `general-purpose` (all tools, report-back system prompt), `Explore` (read-only: disallows Agent/Edit/Write, fast model, no CLAUDE.md), `Plan` (read-only planning). Each is `{agentType, whenToUse, tools|disallowedTools, model, systemPrompt}` — a tiny definition record, and the tool description lists them with their tool access.

**Lifecycle**: agent gets an id; worktree isolation optional with change-detection cleanup (unchanged worktrees are removed); async path registers a task with progress tracking and a completion notification.

## Part 2 — what we already have

Everything needed exists as parts; this plan is mostly composition.

- **In-process harness runner**: `CoderRuntimeSession` (`coder/runtime.rs`) — a full tool registry, `execute_turn_with_id_and_images()`, and three observer seams: `observing_tools` (ToolEvent::Started/Finished), `observing_progress`, `observing_stream` (content/reasoning deltas). This **is** Coder Mini's engine; it runs on any lane including our own proxy grant.
- **Box streaming**: `Control::Tool {call_id,…}`, `Control::ToolOutput {call_id, chunk}`, `Control::ToolDone {call_id, is_error, duration_ms}` flow through `TurnRouter` into `tui.rs` `Entry`s that render as the 4-row boxes with scroll/fade animation (`tool_output_scroll_start`, `settle_tool`). The box is identified by `call_id` — a nested agent is just another call_id whose output keeps growing.
- **Delegation plumbing**: `DelegationGate`, `ChildEvent::{Started,Output,Activity,Finished}` from `dispatch_streaming`, ACP harness path (`run_devin_child` pattern) for external agents, one-external-agent-per-turn cap (`acp_spent`).
- **Tool declarations**: `list_tools()` with the `delegate` arm as the pattern for dynamic descriptions; `BUILTIN_TOOL_NAMES` gate `every_declared_tool_has_an_arm_that_answers_it`.

What's missing: a tool arm that streams *through* its box (current `delegate` arm returns one `ToolOutput` at the end, and children drain events into a discard sink); tool-pool restriction for children (Coder Mini's "few tools"); the agent-definition registry; the CC-style result trailer.

## Part 3 — design

### 3.1 The tool: keep the name `delegate`, adopt CC's schema

Naming debate resolved: `delegate` is already on the wire in sessions, transcripts, surfaces, and issue #228; renaming to `task`/`agent` buys nothing and breaks replays. The *shape* becomes CC's:

```json
delegate({
  "prompt": "...",                       // required — the full brief
  "description": "Audit auth module",    // optional 3–5 word label for the box header
  "count": 2,                            // fan-out (our extension, stays)
  "agent": "explore" | "coder-mini" | "<acp-agent-id>",  // replaces bare lane naming
  "tools": "read-only" | "read-write" | "all",           // Coder Mini tool pool (default read-only)
  "model": "...",                        // optional, Coder Mini only
})
```

Older callers' `{"prompt", "count"}` keep working unchanged (count+missing agent = today's fan-out semantics on the session lane).

### 3.2 Coder Mini — the built-in agent type

`agent: "coder-mini"` (and aliases `explore` = read-only, `coder` = read-write) run **in-process** on `CoderRuntimeSession`:

- **Read-only pool** (`tools:"read-only"`, the default): `read`, `bash` (read-only refusal filter — reuse `check_shell_refusal`), `grep`/`glob` if present, `skill`. No `write`/`edit`/`delegate`/`openagents`/`capability`.
- **Read-write pool** (`tools:"read-write"`): adds `write`, `edit`, unrestricted `shell`.
- **All** (`tools:"all"`): the session's own pool minus `delegate` (no recursive fan-out; matches `HarnessToolRegistry::child`).
- System prompt: CC's general-purpose shape — "You are an agent for Coder… complete the task fully… respond with a concise report covering what was done and any key findings" — plus the tool-pool statement.
- The mini session runs its turn with `execute_turn_with_id_and_images` on a **fresh session** (no parent message replay — the prompt is self-contained, exactly like a CC subagent).
- Worktree isolation: reuse `DelegationGate`/`ChildOptions` isolation semantics for read-write runs; read-only runs in place.

### 3.3 ACP agents stay first-class

`agent: "<registry-id>"` (devin, cursor, opencode, …) routes through the existing ACP harness path — own credentials, own bill, one per turn cap unchanged. This is what makes the same tool work for "our mini version" and "any ACP agent" without special cases in the schema.

### 3.4 Inline streaming — the box is the transcript

New `Control` variant (the one structural change):

```rust
/// A delegated agent's activity, streamed into that agent's own box.
SubagentOutput { call_id: String, line: String },
```

Flow for `agent:"coder-mini"`:

1. Tool arm starts the mini session inside the turn's task (as today's delegate does), with the parent's `call_id` in hand.
2. The mini session's `observing_tools` observer forwards each child tool event as `Control::Tool`/`ToolOutput`/`ToolDone` **with the parent's call_id** — the child's tool calls are already 4-row boxes keyed by call_id, so they render inline inside the delegate box exactly like native ones. Child assistant text/reasoning streams as `SubagentOutput` lines (dim, prefixed `· `, like today's `[child 1] · …` TUI form).
3. The box shows the last N events with a `+N earlier` dim counter (CC's `MAX_PROGRESS_MESSAGES_TO_SHOW` idea; N=6 lines of box history). Ctrl+O-style full expansion is the later fullscreen surface — note it in the export, don't build it now.
4. On finish, the box settles (`settle_tool`) with CC's trailer as the output text: `Done · 14 tool uses · 96s · 41.2k tokens` followed by the child's final report.

For ACP children: `ChildEvent` maps onto the same variants — `Started`→`SubagentOutput("started on devin in <ws>")`, `Tool`→`SubagentOutput`, `Output`→`SubagentOutput`, and the token activity line on finish.

Mechanism: `execute_tool_cancellable` currently returns one `ToolOutput`. The delegate arm needs the *turn's* `Sink` — it already has the seam: `Session::open_at` builds registries with `observing_tools`; add `with_event_sink(Sink)` on `DelegationGate`/registry so tool arms can emit mid-run `Control`s. TurnRouter already scopes every Control by TurnId, so cancellation fencing works unchanged.

### 3.5 Export and transcript

`Control::SubagentOutput` exports as an indented line under the tool call in `export.rs` (same treatment as `Notice` today). The ATIF tool record already captures final output; the streamed lines are presentation-only.

## Part 4 — implementation slices

Each slice lands green and independently revertible.

**S1 — Coder Mini exists** (no UI change): `AgentDefinition` registry (`coder-mini`, `explore`, plus discovered ACP ids) in a new `coder/agents.rs`; `HarnessToolRegistry::with_tool_pool(pool)` restriction; `delegate` arm gains `agent`/`description`/`tools` params; `agent:"coder-mini"` runs one in-process mini turn and returns CC-trailer-form output; unknown agent refused by name with the installed list (port of #228's tests). Old `{"prompt","count"}` behavior untouched. Surfaces regenerate (`check:coder-surfaces`).

**S2 — the box streams**: `Control::SubagentOutput`; registry `with_event_sink`; delegate arm (mini + ACP paths) forwards child events into the parent's box; TUI renders `SubagentOutput` lines inside the tool box with last-N clipping and the settle trailer. Tests: reducer accepts SubagentOutput only for the active turn's call; box clips to N; ACP ChildEvent mapping.

**S3 — model override + worktree for mini**: `model` param on mini runs; `isolation:"worktree"` on read-write mini runs with unchanged-worktree cleanup (steal CC's semantics, not code).

**S4 — result plumbing**: `totalToolUseCount`/`totalTokens` collected from the mini session's usage; trailer becomes data, not string.

Out of scope, deliberately: background agents, nested subagents (depth cap 1 — CC's own default), fullscreen transcript viewer, agent-definition files on disk (`.openagents/agents/` later).

## Part 5 — prerequisites and order vs the parity ladder

Strict prerequisites for this plan: none from #232. Coder Mini runs on the existing `CoderRuntimeSession` (OpenResponses lanes); the parity ladder (message variants, initialize handshake, control timeouts, dead options) is orthogonal — it upgrades the *Claude spawn* path, while this plan upgrades *delegation*, which today already works over lanes/ACP.

Order: **this plan first** (S1 → S2 → S3 → S4), parity ladder resumes after. Only overlap: if/when a Claude lane child is added to Coder Mini's targets, it needs #232 slices 1–2 (message variants + initialize) first — noted on #232, not blocking here.

## Verification

- S1: `cargo test -p openagents-cli` — new tests: `a_coder_mini_run_reports_done_with_tool_use_count`, `an_unknown_agent_is_refused_by_name_with_the_installed_list`, `the_read_only_pool_refuses_write_and_edit`, `old_callers_without_agent_keep_the_fan_out`; `pnpm run check:coder-surfaces` after description changes.
- S2: a live `oa coder` session — `delegate {"agent":"coder-mini","prompt":"read src/main.rs and summarize"}` shows one box whose lines stream the child's `read` call box, text lines, and settles with `Done · N tool uses · …`; cancel mid-run stops the child and settles the box as failed; #228's regression tests (plan-upsell, one-external-per-turn) stay green.
- S3/S4: worktree left in place when changes exist, removed when clean; trailer numbers match `Control::Usage` for the same run.
