# OpenAI Codex `/goal` Implementation Audit

Date: 2026-06-04

Source baseline: `/Users/christopherdavid/work/projects/repos/codex` was pulled
with `git pull --ff-only` before this audit. The audited commit is
`d46a98d31a9c32949580e6e3ce65b85772f3470d` (`Bridge host-loaded skills into the
skills extension (#26172)`).

Scope: how current OpenAI Codex implements persisted thread goals, the `/goal`
TUI command, model-visible goal tools, app-server control APIs, persistence,
runtime accounting, automatic continuation, and goal update notifications.

## Source Inventory

Primary implementation files:

- `codex-rs/core/src/goals.rs`
- `codex-rs/core/src/tools/handlers/goal.rs`
- `codex-rs/core/src/tools/handlers/goal/get_goal.rs`
- `codex-rs/core/src/tools/handlers/goal/create_goal.rs`
- `codex-rs/core/src/tools/handlers/goal/update_goal.rs`
- `codex-rs/core/src/tools/handlers/goal_spec.rs`
- `codex-rs/core/src/tools/spec_plan.rs`
- `codex-rs/core/src/tools/registry.rs`
- `codex-rs/core/src/tasks/mod.rs`
- `codex-rs/core/src/codex_thread.rs`
- `codex-rs/core/src/session/turn_context.rs`
- `codex-rs/state/src/model/thread_goal.rs`
- `codex-rs/state/src/runtime/goals.rs`
- `codex-rs/state/goals_migrations/0001_thread_goals.sql`
- `codex-rs/tui/src/slash_command.rs`
- `codex-rs/tui/src/chatwidget/slash_dispatch.rs`
- `codex-rs/tui/src/app_event.rs`
- `codex-rs/tui/src/app/thread_goal_actions.rs`
- `codex-rs/tui/src/chatwidget/goal_menu.rs`
- `codex-rs/app-server-protocol/src/protocol/v2/thread.rs`
- `codex-rs/app-server/src/request_processors/thread_goal_processor.rs`
- `codex-rs/app-server/src/request_processors/thread_lifecycle.rs`
- `codex-rs/app-server/src/extensions.rs`
- `codex-rs/app-server/src/bespoke_event_handling.rs`
- `codex-rs/prompts/src/goals.rs`
- `codex-rs/prompts/templates/goals/continuation.md`
- `codex-rs/prompts/templates/goals/budget_limit.md`
- `codex-rs/prompts/templates/goals/objective_updated.md`

Secondary implementation files:

- `codex-rs/ext/goal/*`

The `ext/goal` crate mirrors much of the goal design as an extension-shaped
backend sketch, but its `lib.rs` explicitly says it is not wired into the host
yet. The shipped path is the in-core runtime under `codex-rs/core/src/goals.rs`
plus app-server and TUI clients.

## Executive Summary

Codex implements `/goal` as a persisted per-thread objective, not as a prompt
macro. The user-facing TUI command sets, edits, pauses, resumes, clears, and
summarizes a thread goal through app-server JSON-RPC. The model-facing surface
is separate: three function tools named `get_goal`, `create_goal`, and
`update_goal` are added to the tool plan only when the goals feature and host
support are both enabled.

The authoritative state is a single row per thread in a separate SQLite goals
database (`goals_1.sqlite`). Each row has a generated `goal_id`, objective,
status, optional token budget, accumulated tokens, accumulated elapsed seconds,
and timestamps. Runtime state only tracks the active turn's token baseline,
wall-clock baseline, budget-limit steering suppression, and continuation locks.

The most important product behavior is automatic continuation. When a goal is
active and a turn completes, Codex can start another turn with hidden internal
goal context. That hidden context tells the model to keep pursuing the original
objective, verify completion rigorously, and only call `update_goal` when the
goal is complete or genuinely blocked. Goal persistence, token accounting, and
continuation are therefore one system.

## Feature Gating

The feature flag is `Feature::Goals` in `codex-rs/features/src/lib.rs`. It is
stable and `default_enabled: true`. The feature description is persisted thread
goals plus automatic goal continuation.

Goal tools are only included when `TurnContext::goal_tools_enabled()` returns
true. That requires:

- `goal_tools_supported` to be true for the host/thread.
- `Feature::Goals` to be enabled.

The tool planner in `codex-rs/core/src/tools/spec_plan.rs` adds
`GetGoalHandler`, `CreateGoalHandler`, and `UpdateGoalHandler` to the core
utility tools only when that condition passes. Tests in
`codex-rs/core/src/tools/spec_plan_tests.rs` verify that tools are absent when
the feature is disabled, when host support is disabled, and in review subagent
threads.

Review threads explicitly disable goals in `codex-rs/core/src/session/review.rs`.
The parent turn may support goal tools, but the review-specific feature set
turns `Feature::Goals` off so review subagents cannot own or mutate the parent
goal.

Plan mode is also special. `should_ignore_goal_for_mode()` returns true for
`ModeKind::Plan`, so plan-mode turns do not count token usage and do not drive
automatic goal continuation.

## Persistence Model

The active persistence model is a dedicated goals SQLite database, not a table
inside the main state database. `codex-rs/state/src/runtime.rs` defines the
goals database spec as `goals_1.sqlite`, and opens/migrates it separately as
the "goals DB".

`codex-rs/state/goals_migrations/0001_thread_goals.sql` creates:

- `thread_id TEXT PRIMARY KEY NOT NULL`
- `goal_id TEXT NOT NULL`
- `objective TEXT NOT NULL`
- `status TEXT NOT NULL`
- `token_budget INTEGER`
- `tokens_used INTEGER NOT NULL DEFAULT 0`
- `time_used_seconds INTEGER NOT NULL DEFAULT 0`
- `created_at_ms INTEGER NOT NULL`
- `updated_at_ms INTEGER NOT NULL`

Allowed statuses are:

- `active`
- `paused`
- `blocked`
- `usage_limited`
- `budget_limited`
- `complete`

`codex-rs/state/src/model/thread_goal.rs` exposes these as
`ThreadGoalStatus::{Active, Paused, Blocked, UsageLimited, BudgetLimited,
Complete}`. `BudgetLimited` and `Complete` are considered terminal by
`is_terminal()`, but the UI still lets the user edit or clear them, and editing
either one reactivates the goal.

The store intentionally has one logical goal row per thread. New objectives
created through `replace_thread_goal()` generate a fresh `goal_id`, reset token
and time usage to zero, and upsert over the previous thread row. Model-created
goals use `insert_thread_goal()`, which does `ON CONFLICT(thread_id) DO NOTHING`
and returns `None` if a goal already exists.

The `goal_id` is used as a stale-update guard. `GoalUpdate.expected_goal_id`
can require the row's current `goal_id` to match before updating. Runtime
accounting also passes an expected goal ID so an old turn cannot accidentally
charge tokens to a newer replacement goal.

## Store Semantics

`GoalStore` in `codex-rs/state/src/runtime/goals.rs` owns all direct SQL
mutations:

- `get_thread_goal(thread_id)`
- `replace_thread_goal(thread_id, objective, status, token_budget)`
- `insert_thread_goal(thread_id, objective, status, token_budget)`
- `update_thread_goal(thread_id, GoalUpdate)`
- `pause_active_thread_goal(thread_id)`
- `usage_limit_active_thread_goal(thread_id)`
- `delete_thread_goal(thread_id)`
- `account_thread_goal_usage(...)`

Budget handling is enforced in the store. If a goal is `active` and its
`tokens_used` is already greater than or equal to a non-null budget, status
normalizes to `budget_limited`. If accounting pushes an active goal over its
budget, the accounting update changes status to `budget_limited` in the same
SQL update.

`GoalAccountingMode` controls which statuses can be charged:

- `ActiveStatusOnly`: active only.
- `ActiveOnly`: active and budget_limited.
- `ActiveOrComplete`: active, budget_limited, and complete.
- `ActiveOrStopped`: active, paused, blocked, usage_limited, and budget_limited.

The active core path mostly uses `ActiveOnly`; `update_goal complete` first
accounts the current tool/turn progress before changing status, so completed
goals get final usage in the tool result.

Negative time and token deltas are clamped to zero. No-op accounting returns the
current row as `Unchanged`, while successful updates return the updated row.

## Model Tool Contract

The model gets three function tools:

- `get_goal`
- `create_goal`
- `update_goal`

`get_goal` has no parameters. It returns the current thread goal, remaining
tokens when there is a token budget, and no completion report.

`create_goal` has:

- `objective`: required string.
- `token_budget`: optional integer, only when explicitly requested.

The tool spec says the model must only create a goal when the user or higher
priority instructions explicitly ask for one. Ordinary tasks must not be
silently turned into goals. If a goal already exists, creation fails with a
model-facing error telling the model that a new goal cannot be created and that
`update_goal` is only for status.

`update_goal` has:

- `status`: required enum, only `complete` or `blocked`.

The tool spec and handler reject pause, resume, budget-limited, and usage-limited
mutations from the model. Those transitions are controlled by the user, system,
or runtime. Before applying the requested status, the handler emits a
`GoalRuntimeEvent::ToolCompletedGoal`, which accounts current progress while
suppressing budget-limit steering. Then it calls `Session::set_thread_goal()`
with no objective and status `complete` or `blocked`.

The `update_goal` spec includes the strict blocked audit: the model should only
mark a goal blocked after the same blocking condition repeats for at least
three consecutive goal turns and the agent is genuinely unable to make progress
without user input or external state. Completion also has a strict proof
standard in the continuation prompt: the model should inspect current evidence
and verify every requirement before calling `update_goal complete`.

Tool responses are JSON with:

- `goal`
- `remainingTokens`
- `completionBudgetReport`

When a budgeted or elapsed-time-bearing goal is marked complete,
`completionBudgetReport` tells the model to report final usage from the
structured fields. Tests verify that a completed budgeted goal returns final
`tokensUsed`, `tokenBudget`, and `remainingTokens`.

## TUI `/goal` Command

`codex-rs/tui/src/slash_command.rs` defines `SlashCommand::Goal` with the
description "set or view the goal for a long-running task." It supports inline
arguments and remains available during active tasks.

The live dispatch behavior is in `codex-rs/tui/src/chatwidget/slash_dispatch.rs`:

- Bare `/goal` opens the goal menu if a thread ID exists.
- Bare `/goal` before a thread starts shows usage text.
- `/goal clear` sends `AppEvent::ClearThreadGoal`.
- `/goal pause` sends `AppEvent::SetThreadGoalStatus { status: Paused }`.
- `/goal resume` sends `AppEvent::SetThreadGoalStatus { status: Active }`.
- `/goal edit` sends `AppEvent::OpenThreadGoalEditor`.
- `/goal <anything else>` sends `AppEvent::SetThreadGoalObjective` with
  `ThreadGoalSetMode::ConfirmIfExists`.

There is no special parsing for token-budget syntax in the slash command. The
test for `/goal --tokens 98.5K improve benchmark coverage` asserts that the
objective is exactly `--tokens 98.5K improve benchmark coverage`. Token budgets
exist in the model tool and app-server protocol, but the current slash parser
does not expose a `/goal --tokens` UI.

Goal slash commands intentionally strip attachments. The tests show that image
attachments are dropped, while textual placeholders such as `[Image #2]` remain
in the objective string. Mention bindings are also converted to plain text, so
`$figma` remains literal objective text rather than carrying an app binding.

If `/goal <objective>` is entered before a thread exists, the TUI queues the
slash command and parses it after the thread starts. The tests verify that the
event emitted after thread startup contains the original objective.

## TUI App Actions

`codex-rs/tui/src/app/thread_goal_actions.rs` turns TUI app events into
app-server requests:

- `open_thread_goal_menu()` calls `thread/goal/get`.
- `open_thread_goal_editor()` calls `thread/goal/get`, then opens a prompt.
- `set_thread_goal_objective()` may read existing goal state, confirm
  replacement, clear the old goal, then call `thread/goal/set`.
- `set_thread_goal_status()` calls `thread/goal/set` with only status.
- `clear_thread_goal()` calls `thread/goal/clear`.

Replacement confirmation is status-aware. If the existing goal is complete,
`/goal <objective>` can replace it without confirmation. Active, paused,
blocked, usage-limited, and budget-limited goals require explicit confirmation
before replacement.

`codex-rs/tui/src/chatwidget/goal_menu.rs` renders the bare `/goal` summary:

- Status.
- Objective.
- Time used.
- Tokens used.
- Token budget if present.
- Available follow-up commands.

The command hints are:

- Active: `/goal edit, /goal pause, /goal clear`.
- Paused, blocked, usage-limited: `/goal edit, /goal resume, /goal clear`.
- Budget-limited, complete: `/goal edit, /goal clear`.

The edit prompt preserves active, paused, blocked, and usage-limited status. If
the user edits a budget-limited or complete goal, the edited goal becomes active
again.

Ephemeral sessions get a product-friendly error. The TUI translates raw
"ephemeral thread does not support goals" or "thread goals require a persisted
thread" errors into a message explaining that goals need a saved session.

## App-Server Protocol

`codex-rs/app-server-protocol/src/protocol/v2/thread.rs` exposes:

- `thread/goal/set`
- `thread/goal/get`
- `thread/goal/clear`

`ThreadGoal` DTO fields are:

- `threadId`
- `objective`
- `status`
- `tokenBudget`
- `tokensUsed`
- `timeUsedSeconds`
- `createdAt`
- `updatedAt`

`ThreadGoalSetParams` includes:

- `threadId`
- optional nullable `objective`
- optional nullable `status`
- double-option `tokenBudget`

The double-option token budget distinguishes "leave unchanged" from "set to
null" from "set to a number."

The server emits:

- `thread/goal/updated` with `threadId`, optional `turnId`, and `goal`.
- `thread/goal/cleared` with `threadId`.

`codex-rs/app-server/src/request_processors/thread_goal_processor.rs` handles
these requests. It rejects requests when the goals feature is disabled, parses
thread IDs, requires a materialized persistent thread, reconciles rollout state
for unloaded threads, validates objectives and positive budgets, accounts live
runtime progress before external mutation, persists the mutation, sends the
JSON-RPC response, emits ordered notifications, and then applies runtime effects
to any running thread.

Notification ordering matters. When a running thread has a listener command
channel, app-server enqueues `EmitThreadGoalUpdated` or `EmitThreadGoalCleared`
so notifications remain ordered with resume responses and replay. If no listener
is available, it sends the notification directly.

On resume, `emit_resume_goal_snapshot_and_continue()` sends a goal snapshot
first, then asks the core thread to continue the active goal if idle. That
prevents clients from seeing an automatic continuation before they know the
current goal state.

## Core Runtime Events

`codex-rs/core/src/goals.rs` owns `GoalRuntimeEvent`, the main integration
surface between sessions/tasks/tools and goal runtime behavior:

- `TurnStarted`
- `ToolCompleted`
- `ToolCompletedGoal`
- `TurnFinished`
- `MaybeContinueIfIdle`
- `TaskAborted`
- `UsageLimitReached`
- `ExternalMutationStarting`
- `ExternalSet`
- `ExternalClear`
- `ThreadResumed`

`Session::goal_runtime_apply()` is the dispatcher. This centralizes runtime
policy instead of scattering goal side effects throughout tools, task loops, and
app-server code.

`codex-rs/core/src/tasks/mod.rs` emits `TurnStarted` with the token usage
baseline before a task runs. It emits `TurnFinished` when a turn completes and
then emits `MaybeContinueIfIdle` after clearing the active turn. Task abort
paths emit `TaskAborted`.

`codex-rs/core/src/tools/registry.rs` emits `ToolCompleted` after a tool call
has reached a terminal lifecycle outcome. It skips duplicate lifecycle claims,
then calls goal accounting once per completed tool. The update-goal handler
uses the separate `ToolCompletedGoal` event so completion/blocking can account
current usage without injecting budget-limit steering into the same tool result.

`codex-rs/core/src/session/turn.rs` emits `UsageLimitReached` when model/account
usage limits stop the turn. The runtime accounts progress and transitions the
active or budget-limited goal to `usage_limited`, then clears active accounting
so no continuation starts.

`codex-rs/core/src/codex_thread.rs` exposes the app-server-facing runtime hooks:

- `apply_goal_resume_runtime_effects()`
- `continue_active_goal_if_idle()`
- `prepare_external_goal_mutation()`
- `apply_external_goal_set()`
- `apply_external_goal_clear()`

Those methods are thin wrappers around `GoalRuntimeEvent`.

## Accounting Details

`GoalRuntimeState` keeps:

- an optional `StateDbHandle` cache
- `budget_limit_reported_goal_id`
- `accounting_lock`
- `GoalAccountingSnapshot`
- `continuation_lock`

The accounting snapshot has two parts:

- Turn accounting: current turn ID, last accounted token usage, and active goal
  ID for that turn.
- Wall-clock accounting: last accounted instant and active goal ID while idle.

Token deltas use non-cached input plus output tokens:

`goal_token_delta_for_usage = usage.non_cached_input() + max(output_tokens, 0)`

Reasoning output tokens are not added separately; they are only included if
they are already represented in output token accounting. Cached input tokens are
subtracted out.

On turn start, the runtime stores the current total token usage as a baseline.
If the persisted goal is active or budget-limited, the turn and wall-clock
snapshots are marked with that `goal_id`. If the goal is stopped or absent,
wall-clock active accounting is cleared.

On ordinary tool completion, `account_thread_goal_progress()`:

1. Checks feature and mode gates.
2. Gets the current state DB.
3. Acquires the accounting semaphore.
4. Computes token delta from total usage since the last accounting point.
5. Computes elapsed wall-clock seconds since the last accounting point.
6. Calls `GoalStore::account_thread_goal_usage()` with `ActiveOnly` and expected
   goal ID.
7. Updates local baselines if the store row changed.
8. Emits terminal metrics if status changed to blocked, usage-limited,
   budget-limited, or complete.
9. Emits `ThreadGoalUpdated`.
10. Injects budget-limit steering only once for the same goal ID when steering
    is allowed and the goal first becomes budget-limited.

Budget-limited goals keep accruing progress until turn stop or a suppressing
accounting path clears active accounting. Tests verify that a goal can cross
the budget at a tool boundary, receive steering, and still accumulate later
tokens before the turn ends.

On external mutation, the runtime first accounts the active turn if one exists.
If there is no active turn, it accounts idle wall-clock time. This keeps usage
from disappearing when a user edits, pauses, resumes, completes, or clears a
goal from outside the model.

On interrupt or shutdown without an active turn, active goals remain active.
Interrupt accounts progress but does not pause the goal.

## Automatic Continuation

Automatic continuation is implemented in
`maybe_continue_goal_if_idle_runtime()` and
`maybe_start_goal_continuation_turn()`.

The runtime will not start a continuation when:

- Goals are disabled.
- Collaboration mode is Plan.
- Another turn is already active.
- Trigger-turn mailbox input is pending.
- The thread is ephemeral or has no state DB.
- No persisted goal exists.
- The persisted goal is not active.
- The goal changes between candidate creation and turn launch.

When continuation is allowed, the runtime reserves an active turn slot, appends
a hidden goal context `ResponseItem` to that turn's pending input, creates a new
default turn with a fresh UUID sub-ID, and starts a regular task. The hidden
context is built from `codex-rs/prompts/templates/goals/continuation.md`.

The continuation prompt tells the model:

- Continue the active thread goal.
- Treat the objective as user-provided task context, not higher-priority
  instructions.
- Preserve the full objective instead of redefining success to fit one turn.
- Use current worktree and external state as authoritative.
- Use `update_plan` when useful for multi-step progress.
- Verify every requirement before marking complete.
- Only call `update_goal blocked` after the strict repeated-blocker audit.

Budget-limit steering uses a different hidden prompt. It tells the model that
the goal reached its token budget, should not start new substantive work, and
should wrap up with progress, remaining work, blockers, and a next step.

Objective-update steering is injected when an externally active goal changes
objective during a running turn. It tells the model the new objective supersedes
the old one and that work serving only the prior objective should stop.

## Event And Notification Flow

Core goal updates are emitted as `EventMsg::ThreadGoalUpdated` with the current
thread ID, optional turn ID, and protocol goal. App-server has two paths for
forwarding these:

- `app-server/src/extensions.rs` forwards extension event sink goal updates.
- `app-server/src/bespoke_event_handling.rs` forwards core session goal update
  events as global server notifications.

App-server request-initiated changes use ordered listener commands where
possible so clients see goal changes in a predictable sequence relative to
thread resume and replay.

Clears are app-server notifications only; the core session goal event type is
an update event, while `thread/goal/cleared` is a protocol notification used by
clients after clear/delete operations or snapshot reads with no row.

## Metrics

Goal metrics are emitted through session telemetry:

- `GOAL_CREATED_METRIC`
- `GOAL_RESUMED_METRIC`
- `GOAL_BLOCKED_METRIC`
- `GOAL_USAGE_LIMITED_METRIC`
- `GOAL_BUDGET_LIMITED_METRIC`
- `GOAL_COMPLETED_METRIC`
- `GOAL_TOKEN_COUNT_METRIC`
- `GOAL_DURATION_SECONDS_METRIC`

Created metrics fire for new logical goals. Resumed metrics fire when status
changes from paused, blocked, or usage-limited back to active. Terminal metrics
fire only when status changes into a terminal/stopped status that should be
reported.

## Important Edge Cases

Ephemeral sessions do not support goals. Core returns "thread goals require a
persisted thread" and app-server returns "ephemeral thread does not support
goals"; TUI maps those into a saved-session explanation.

The state DB is lazily materialized. `state_db_for_thread_goals()` calls
`try_ensure_rollout_materialized()`, reconciles rollout state if thread metadata
is missing, and caches the resulting `StateDbHandle` in `GoalRuntimeState`.

Budget updates can immediately stop an active goal if existing usage already
exceeds the new budget.

`usage_limited` can be applied to either active or budget-limited goals. That
allows account-level usage failure after budget-limit steering to preserve final
progress while preventing continuation.

Parallel tool completions are guarded by the accounting semaphore and expected
goal ID. Tests verify only one parallel completion accounts the same token
delta.

Plan-mode turns do not count usage and do not stop the goal on usage-limit
events.

If a user edits an active goal while a turn is running, runtime injects hidden
objective-updated steering into that turn. If there is no active turn, the
runtime can immediately try to continue the active goal if idle.

If app-server updates an unloaded thread, it reconciles rollout state and
updates persistence without live runtime effects. When the thread resumes, the
resume path snapshots goal state and restores active goal wall-clock accounting.

## Tests Covering `/goal`

Useful test coverage lives in several layers:

- `codex-rs/tui/src/chatwidget/tests/slash_commands.rs` covers slash parsing,
  queued goals, dropped images, control commands, and literal `--tokens` text.
- `codex-rs/tui/src/app/thread_goal_actions.rs` unit tests cover temporary
  session error rendering and replacement confirmation rules.
- `codex-rs/core/src/tools/spec_plan_tests.rs` covers tool gating by feature,
  host support, and review subagent source.
- `codex-rs/core/src/session/tests.rs` covers interrupt accounting,
  continuation after no-tool turns, request-user-input continuation suppression,
  budget-limit steering, usage-limit stopping, external mutation accounting,
  objective-update steering, active external set accounting, and final complete
  tool output.
- `codex-rs/state/src/runtime/goals.rs` tests cover goal insert/update/delete,
  status transitions, stale expected goal IDs, usage accounting, budget updates,
  and mode-specific accounting.

## Design Lessons For OpenAgents product surface

The strongest pattern is the separation between durable goal state, live runtime
accounting, model tools, and user controls. The model can only create a goal
when explicitly told and can only finish or block it. Users and app-server
controls can pause, resume, replace, edit, budget, and clear. Runtime alone
owns usage-limit and budget-limit transitions.

The second strong pattern is using the persisted `goal_id` as an identity guard.
Any long-running worker, external UI update, or tool-completion hook can race
with a replacement goal. Expected goal IDs keep old accounting events from
mutating the new goal.

The third strong pattern is hidden internal context rather than rewriting the
system prompt. Continuation, budget-limit, and objective-update prompts are
ordinary hidden user-context fragments with source `goal`. That makes them
auditable, testable, injectable into current turns, and removable from normal UI
copy.

The fourth strong pattern is centralized runtime events. Tool loops, task loops,
usage-limit paths, app-server mutations, and resume paths all report a small
enum into one policy module. This is a cleaner boundary than sprinkling goal
side effects across every caller.

The most obvious gap is slash-command budget UX. Codex has a budget in the
model tool and app-server protocol, but the TUI slash command currently treats
`--tokens` as literal objective text. If OpenAgents product surface copies the design, it should make
an explicit choice: either no slash budget syntax, or typed parsing that is
covered by tests and backed by the same double-option API.

## Artanis Plan Integration

The existing Artanis work already has most of the durable substrate that Codex
uses for `/goal`, but it is split across run records, team/project chat rows,
sync scopes, and SHC/OpenCode event ingestion. The right next step is to make a
goal the stable product object that Artanis lives around, then treat each
`agent_runs` row as one execution attempt, continuation turn, or worker session
for that goal.

Relevant existing OpenAgents product surface pieces:

- `docs/2026-06-03-team-project-rooms.md` records the seeded Artanis project:
  `project_artanis | team_openagents_core | artanis | Artanis`.
- `workers/api/migrations/0023_team_projects.sql` adds `team_projects`,
  `team_chat_messages.project_id`, and `agent_runs.project_id`.
- `workers/api/migrations/0024_project_agent_metadata.sql` stores the compact
  Artanis project-agent projection: active project-scoped Autopilot agent,
  SHC backend, `openagents` repository, and Pylon focus.
- `workers/api/src/omni-runs.ts` persists `agent_runs` with `teamId`,
  `projectId`, runtime/backend, repository, `goal`, assignment JSON, status,
  event cursor, and lifecycle timestamps.
- `workers/api/src/omni-runs.ts` persists append-only `agent_run_events` with
  sequence, type, summary, status, source, sanitized payload JSON, artifact
  refs, external event id, and created timestamp.
- `packages/sync-worker` and Worker sync routes already project workspace,
  thread, team, and `agent-run:{runId}` scopes.
- `workers/api/src/team-chat.ts` already projects team/project messages,
  `autopilot_intent` rows, `agentRunId`, and strict compact `runSummary`
  metadata for parent-room cards.
- The Artanis answer-back work now prefers completed `result.md` artifact
  reads before falling back to assistant progress text.

Codex's goal model should not replace this system. It should add the missing
long-lived objective layer above it:

```text
Artanis project agent
  -> durable goal
  -> one or more agent runs / continuation attempts
  -> append-only private event ledger
  -> sanitized team/project chat projection
  -> optional sanitized public stream
```

That keeps Artanis from being only "a run that happened" or "a chat message
that launched a run." Artanis becomes a durable agent identity with a current
objective, status, history, and public progress trail.

### Pylon Campaign Priority

The first public Artanis campaign should be the Pylon release plan. The
shareable route is `https://openagents.com/artanis`, with
`/agents/artanis` kept as the canonical public-agent route shape.

That page should lead with the public Pylon proof surface before the longer
activity stream:

- campaign objective: release the next Pylon version, connect it more deeply
  to OpenAgents product surface, route more inference and fine-tuning work to the live Pylon wave,
  and use the new Bitcoin infrastructure as the work settlement layer;
- current durable public Artanis goal when one has been published, plus the
  fallback campaign objective while the durable goal is not yet public;
- Nexus connection state, source URL, hosted relay URL, last refresh timestamp,
  and explicit unavailable/stale-copy state when the recovery proxy is in use;
- the old Laravel `openagents.com/stats` Pylon counters: Pylons online now,
  sellable Pylons online now, sessions online now, Pylons seen in the last
  24 hours, accepted-work sats total and 24-hour total, contributors to
  training, and the compact recent-Pylon table;
- sanitized public Artanis activity, artifacts, receipts, and links that prove
  what the agent is building without exposing the runner control plane.

This page is both the marketing link for the livestream/referral campaign and
the proof page for the new public Autopilot pattern. It must never expose SHC
callback tokens, provider refs, hidden steering prompts, raw runner payloads,
private repository contents, raw shell output, `payloadJson`, or unredacted
credentials. Public viewers should see the work and the network state, not the
private substrate.

### Durable Goal Authority

OpenAgents product surface should add an `agent_goals` or `thread_goals` authority rather than keep
only `agent_runs.goal` as a free text field. The current run-level `goal` should
remain as a denormalized snapshot for existing APIs and timeline rendering, but
the authoritative goal object should carry:

- `id`
- `agent_id`, such as `agent_artanis`
- `team_id`
- `project_id`
- `objective`
- `status`: active, paused, blocked, usage_limited, budget_limited, complete
- `visibility`: private, team, public
- `current_run_id`
- optional `token_budget`
- `tokens_used`
- `time_used_seconds`
- created, updated, completed, archived timestamps

`agent_runs` should then gain `goal_id`, and every runner event ingestion should
carry either the expected `goal_id` or a run-to-goal lookup. That gives OpenAgents product surface
the same stale-event protection Codex gets from `goal_id`: if Artanis starts a
new objective, old SHC callbacks cannot mutate the new goal's status or budget.

The model-tool/user-control split should also carry over. Public viewers and
the model should not be able to pause, resume, replace, or budget a goal.
Owners/operators can set, edit, pause, resume, clear, and choose visibility.
The agent/model can only explicitly create a requested goal and mark it
complete or genuinely blocked under the same strict blocked/completion audit
standard.

### Long-Running Artanis Runtime

For Artanis, a "turn" is not only a ChatGPT/Codex model turn. It can be a full
SHC/OpenCode run, a restarted worker session, a follow-up continuation, or a
checkpointed Workflow step. OpenAgents product surface should map SHC/OpenCode callbacks into a
small goal runtime event enum:

- `GoalCreated`
- `RunAccepted`
- `RunStarted`
- `ToolCompleted`
- `ArtifactPublished`
- `CheckpointPersisted`
- `UsageAccounted`
- `RunCompleted`
- `RunFailed`
- `UsageLimitReached`
- `ExternalSet`
- `ExternalClear`
- `WorkerResumed`

The policy interpreter should own status transitions, token/time accounting,
continuation scheduling, public projection, and notification ordering. The SHC
runner should only report facts; it should not decide whether a goal is public,
complete, blocked, budget-limited, or eligible for continuation.

The continuation rule should be stricter than "start another run whenever the
last run ended." A new Artanis continuation should start only when:

- the goal is still active;
- no owner or operator pause is set;
- no unanswered approval/user-input blocker is pending;
- the previous run's public/private snapshots have been durably written;
- the expected goal id still matches;
- budget and usage limits still permit work;
- there is no higher-priority mailbox or steering input waiting.

If those checks pass, a Queue or Workflow can enqueue the next Artanis run with
hidden goal context in the assignment. This mirrors Codex's hidden continuation
prompt while fitting OpenAgents product surface's process-heavy SHC runtime.

### Public Visibility And Public Chat

The public surface should be an explicit projection of the goal/run event
ledger, not a browser tail of raw SHC, OpenCode, Codex, or callback payloads.
The safe product shape is:

```text
/agents/artanis
  public agent profile
  current public goal
  compact status and run card
  live sanitized activity stream
  published artifacts / receipts
  public chat-style transcript generated from projection records
```

The same projection could also be embedded under the project route when the
viewer is authenticated:

```text
/teams/openagents-core-team/projects/artanis/chat
  team/private project chat
  owner controls and full run diagnostics
  compact run cards
  answer-back messages
  optional "make public" / "public view" affordance
```

Public chat should look alive while Artanis works, but it must only stream safe
records. Good public records are:

- goal status changes;
- run accepted/started/completed summaries;
- safe tool/action summaries;
- file or artifact publication events with public artifact refs;
- token/time totals when public;
- blocker summaries;
- final answer-back messages;
- receipts, commit links, issue links, and deployment links after sanitization.

Bad public records are:

- raw provider-account refs or grant material;
- callback token refs;
- raw `auth.json`, OpenCode credentials, or OAuth material;
- full runner payload blobs;
- private repository contents unless explicitly published;
- unredacted shell output that may contain secrets;
- hidden dispatch prompts;
- chain-of-thought or internal reasoning traces;
- high-volume token/text deltas that make the UI noisy without adding public
  understanding.

OpenAgents product surface already has this projection instinct. `publicAgentRunBundle()` redacts the
runner callback token ref, and the sync projections carry sanitized
`payloadJson` for diagnostics instead of dumping raw callback bodies into the
primary chat. The public Artanis stream should make that separation formal:

- Private ledger: full authorized run state for operators and owners.
- Team projection: sanitized but richer project-room timeline and diagnostics.
- Public projection: safest compact records for anonymous or public viewers.

### Sync Scope Shape

The current implemented scopes cover authenticated work:

- `workspace:{userId}`
- `team:{teamId}`
- `thread:{threadId}`
- `agent-run:{runId}`

For public Artanis visibility, add or activate public scopes such as:

- `public-agent:agent_artanis`
- `public-goal:{goalId}`
- `public-agent-run:{runId}`

The public route should load a snapshot first, then subscribe to a Durable
Object/WebSocket stream exactly like authenticated scopes. Cursor gaps should
reload the public snapshot. Public subscribers should never need access to the
team scope, private thread scope, or raw `agent-run:{runId}` scope.

This lets the same D1 outbox and sync reducer architecture support both private
operator views and public observer views, while keeping authorization and
redaction at the scope boundary.

### Effect Services To Add

Implement this as Effect services rather than page-level glue:

- `AgentGoalRepository`: D1 persistence for goals, status, visibility, expected
  goal ids, budgets, and usage totals.
- `AgentGoalRuntimeService`: maps runner facts into goal runtime events and
  applies the policy interpreter.
- `AgentGoalContinuationService`: enqueues the next Artanis run through Queues
  or Workflows when the goal remains active and continuation is allowed.
- `AgentPublicProjectionService`: converts private goal/run records into public
  records, rejects unsafe fields, and writes public sync changes.
- `AgentGoalAccessService`: decides owner, team, operator, and anonymous public
  read/write capabilities.

The browser should only see the projected model. It should not know SHC control
URLs, callback credentials, runner-internal IDs beyond public refs, or hidden
goal steering text.

### Recommended Implementation Sequence

1. Ship the public Pylon campaign surface at `/artanis`, backed by
   `/api/public/pylon-stats` and the existing public Artanis goal projection.
2. Add a docs-backed `agent_goals` contract and D1 migration, with `goal_id` on
   `agent_runs`.
3. Add strict public/team/private projection schemas for goal and goal event
   records.
4. Update Artanis project chat launch to create or attach an active goal before
   creating the run.
5. Map SHC/OpenCode ingest events into goal runtime events and account usage
   with expected goal IDs.
6. Publish team/project sync patches for goal status and run summary updates.
7. Add public scopes and public snapshots for `agent_artanis`.
8. Render `/agents/artanis` and `/artanis` as the public observer page with the
   live sanitized chat-style stream.
9. Add owner/operator controls for public visibility, pause/resume, and
   goal replacement.
10. Gate automatic continuation behind durable snapshot publication and
    explicit active-goal checks.
11. Add regression tests that prove private callback data, auth refs, hidden
    dispatch prompts, and raw runner payloads cannot appear in public
    projections.

The key product principle is that public Artanis visibility should increase
trust without exposing the wrong substrate. People should see Artanis moving
toward a goal through readable events and artifacts. They should not see the
private runner control plane.

## OpenAgents product surface Implementation Notes

### 2026-06-04 Durable Goal Foundation

Issue #45 added the first concrete OpenAgents product surface substrate for this audit:

- `workers/api/migrations/0027_agent_goals.sql` creates the durable
  `agent_goals` table with status, visibility, current run, token budget,
  usage totals, timestamps, and an archived-current-goal model for one current
  goal per agent/user/team/project scope.
- The same migration adds `agent_runs.goal_id`, preserving
  `agent_runs.goal` as the denormalized objective snapshot while giving future
  runner events a stable durable-goal identity to attach to.
- `workers/api/src/agent-goals.ts` defines Schema-backed goal status,
  visibility, private record, public projection record, typed Effect errors,
  `AgentGoalRepository`, and `AgentGoalAccessService`.
- The repository supports the API/runtime operations needed by later issues:
  current-goal lookup, goal replacement, objective edit, pause/resume and other
  status transitions, archive, visibility changes, budget changes, usage
  accounting with expected-goal-id checks, and current-run attachment.
- `AgentGoalAccessService` keeps owner/team/operator/public read/write rules
  separate from route handlers and exposes a public projection that omits
  private scope fields.
- `workers/api/src/omni-runs.ts` now carries nullable `goalId` on
  `AgentRunRecord` and authenticated run projections, while keeping dispatch
  behavior unchanged until the SHC/OpenCode wiring issue lands.

This foundation intentionally does not yet expose HTTP routes, browser UI,
automatic continuation, public sync scopes, or runner callback goal accounting.
Those are separate follow-on implementation issues so each boundary can be
tested and closed independently.

### 2026-06-04 Goal API Routes

Issue #46 added the Worker API boundary on top of the durable goal service:

- Authenticated browser routes:
  - `GET /api/autopilot/goals/current`
  - `POST /api/autopilot/goals`
  - `GET /api/autopilot/goals/:goalId`
  - `PATCH /api/autopilot/goals/:goalId`
  - `POST /api/autopilot/goals/:goalId/pause`
  - `POST /api/autopilot/goals/:goalId/resume`
  - `POST /api/autopilot/goals/:goalId/clear`
  - `POST /api/autopilot/goals/:goalId/visibility`
- Operator routes with admin bearer auth:
  - `GET /api/operator/autopilot/goals/current`
  - `POST /api/operator/autopilot/goals`
  - `GET /api/operator/autopilot/goals/:goalId`
  - `PATCH /api/operator/autopilot/goals/:goalId`
  - `POST /api/operator/autopilot/goals/:goalId/pause`
  - `POST /api/operator/autopilot/goals/:goalId/resume`
  - `POST /api/operator/autopilot/goals/:goalId/clear`
  - `POST /api/operator/autopilot/goals/:goalId/visibility`
- Programmatic agent routes:
  - `GET /api/agents/goals/current`
  - `POST /api/agents/goals`
  - `GET /api/agents/goals/:goalId`
  - `POST /api/agents/goals/:goalId/complete`
  - `POST /api/agents/goals/:goalId/blocked`
- Public read-only routes:
  - `GET /api/public/agents/:agentId/goal`
  - `GET /api/public/goals/:goalId`

All request bodies are decoded through Effect Schema classes in
`workers/api/src/agent-goal-routes.ts`. Route handlers provide
`AgentGoalRepository` and `AgentGoalAccessService` as Effect services, map
domain errors once at the route boundary, and return DTOs rather than raw D1
rows. Browser and operator DTOs include private scope fields only for
authorized callers. Public DTOs are produced through the service-owned public
projection and omit `userId`, `teamId`, `projectId`, and `visibility`.

### 2026-06-04 Agent-Facing Goal Tool Contract

Issue #47 added the first model/runner-facing goal contract:

- `packages/sync-schema/src/index.ts` now includes
  `AgentGoalAssignmentContext`, `AgentGoalToolContract`,
  `AgentGoalToolSpec`, and `AgentGoalHiddenSteering`.
- `AgentRunAssignment` can carry optional `goalContext` with the exact
  `get_goal`, `create_goal`, and `update_goal` tool contract, current goal
  identity/status/budget fields, and hidden continuation/budget/objective/public
  steering templates.
- `workers/api/src/agent-goal-runtime.ts` defines `AgentGoalRuntimeService`,
  `AgentGoalEventRepository`, a D1 event repository, model-facing result
  schemas, terminal-status schemas, and steering builders.
- `workers/api/migrations/0028_agent_goal_events.sql` adds the private
  `agent_goal_events` ledger. Terminal model claims record goal id, expected
  goal id, run id, caller type, status, usage deltas, payload, and timestamp.
- Programmatic agent creation now requires an explicit goal request and fails
  when a current goal already exists in the same scope. The model-facing create
  path cannot set visibility.
- Programmatic agent updates are terminal-only: `complete` and `blocked` are
  accepted through `POST /api/agents/goals/:goalId/update` or the existing
  compatibility paths. Pause, resume, budget-limited, usage-limited, visibility,
  and objective changes remain user/operator/runtime controls.
- `update_goal` accounts final token/time usage before applying the terminal
  status and returns a structured completion budget report.
- SHC/OpenCode dispatch payloads include `goalContext` only when the queued run
  is attached to a durable goal. Public run bundles strip hidden steering text
  while preserving safe goal metadata and the tool contract shape.

The hidden continuation template treats objective text as untrusted user data by
embedding it as a JSON literal, preserves the full objective across runs, points
the model at current worktree/external state, requires verification before
`complete`, and repeats the strict three-attempt blocked audit before `blocked`.
The public-visibility template forbids secrets, raw callback payloads, hidden
steering text, private substrate details, and chain-of-thought in public
artifacts or answer-back text.

### 2026-06-04 Runtime Accounting And Continuation Policy

Issue #48 added the first centralized runtime policy layer for goals:

- `workers/api/migrations/0029_agent_goal_event_idempotency.sql` adds
  `external_event_id` to `agent_goal_events` and a unique
  `(goal_id, external_event_id)` index so runner callback retries can be
  detected and skipped.
- `AgentGoalEventRepository` now supports `recordOnce`, `hasExternalEvent`,
  and event-type counting. Runtime accounting uses this ledger before applying
  usage so one external event is counted once.
- `AgentGoalRuntimeEventType` models the runtime fact vocabulary:
  `GoalCreated`, `RunAccepted`, `RunStarted`, `ToolCompleted`,
  `ArtifactPublished`, `CheckpointPersisted`, `UsageAccounted`,
  `RunCompleted`, `RunFailed`, `UsageLimitReached`, `BudgetLimitReached`,
  `ExternalSet`, `ExternalClear`, and `WorkerResumed`.
- `AgentGoalAccountingService` maps those facts into policy-owned transitions:
  run attachment for accepted/started/resumed facts, token/time accounting for
  usage-bearing facts, idempotent retry handling by external event id,
  `budget_limited` when the explicit or default token budget is exhausted, and
  `usage_limited` when runtime/capacity reports usage exhaustion.
- `AgentGoalCapacityPolicyService` centralizes first-pass continuation checks:
  status must be `active`, token budget must remain, continuation attempts must
  stay under the configured cap, durable snapshots must be written, approval and
  mailbox blockers must be absent, provider health must be acceptable, and
  account capacity must be available.
- `AgentGoalContinuationService` evaluates the capacity policy, transitions
  exhausted goals to `budget_limited` or `usage_limited`, and enqueues the next
  run only through an injected queue service. It records `WorkerResumed` with a
  deterministic external event id and attaches the returned run id to the goal.

The default policy is deliberately bounded: a goal without an explicit budget
uses `DEFAULT_AGENT_GOAL_TOKEN_BUDGET`, and continuation attempts default to
`DEFAULT_AGENT_GOAL_MAX_CONTINUATIONS`. The continuation implementation is not
an in-process recursive loop; it hands off to the queue boundary and records a
durable event before the next run can be observed.

### 2026-06-04 SHC/OpenCode Goal Wiring

Issue #49 connected the durable goal system to the existing OpenAgents product surface run pipeline:

- Operator and logged-in user Autopilot launches now resolve a goal scope before
  queuing the run. Project launches use the project agent id when available,
  which makes `agent_artanis` project work attach to the Artanis project goal
  rather than a generic personal Autopilot scope.
- Launches reuse the current active goal in that scope. If the current goal is
  complete, blocked, paused, usage-limited, or budget-limited, the new launch
  replaces it through `AgentGoalRepository.setGoal`, which archives the old
  current row and creates a fresh active goal.
- `createQueuedAgentRun` receives the durable `goalId`, current goal status,
  visibility, budget, and usage totals. `agent_runs.goal` remains the objective
  snapshot, while the assignment JSON carries safe `goalContext` and hidden
  goal steering for the runner.
- Accepted launch events, dispatch events, callback events, and body-level
  callback statuses are mapped into `AgentGoalRuntimeEvent` values and applied
  through `AgentGoalAccountingService`. Callback token usage uses the existing
  OpenAgents product surface token-usage extractor and the same external source refs that back
  `autopilot_token_usage`.
- Callback body status is a single synthetic goal runtime event. A completed
  callback no longer makes every log record look like `RunCompleted`; ordinary
  log records only become usage/progress events when their normalized event
  payload warrants it.
- Completed and failed body statuses include elapsed run seconds from the
  updated `agent_runs` row, so goal time accounting moves with the run lifecycle.
- The callback path ignores stale or archived goals through the existing
  expected-goal-id and archived-row checks. Retried callback events are
  idempotent through `agent_goal_events.external_event_id`.
- Completed callbacks keep the existing team/project answer-back behavior and
  then request a policy-gated continuation. Continuations issue fresh provider
  and GitHub-write grants, preserve the original repository/work-order context,
  enqueue a new goal-linked run, publish the normal run sync scopes, and dispatch
  through the same SHC/OpenCode handoff.

The important remaining gap after #49 is projection/UI breadth, not backend goal
authority. Private run records and runner callbacks can now drive durable goal
state, but issue #50 still needs to expose owner controls in the authenticated
UI, issue #51 still needs public sanitized streams, issue #52 still needs full
goal sync projections/reducers, and issue #53 still needs broad end-to-end
guardrails.

### 2026-06-04 Logged-In Goal Controls

Issue #50 added the authenticated owner-facing goal surface without reopening
the old monolithic logged-in update file:

- The browser model now carries a typed `AgentGoalPanelModel` with the current
  goal, objective draft, token-budget draft, edit state, pending action, and
  optional error. The API DTO is schema-backed by `AgentGoalApiGoal` and
  `AgentGoalResponse`, matching the existing `/api/autopilot/goals` private
  route contract.
- `apps/web/src/page/loggedIn/goals/scope.ts` centralizes route-to-goal-scope
  mapping. Personal chats and thread routes use the personal `autopilot` scope,
  team rooms add `teamId`, and enabled project rooms use the project agent id
  plus `teamId` and `projectId`.
- `goals/commands.ts` owns the Effect-backed Foldkit commands:
  `LoadAgentGoal`, `SaveAgentGoal`, and `UpdateAgentGoalAction`. They all use
  the shared JSON request/decode helper and return typed success/failure
  messages instead of raw fetch logic in the parent update.
- `goals/transitions.ts` owns the pure reducer for hydration, draft editing,
  budget validation, save submission, pause/resume/clear, and public visibility
  requests. Stale scope responses are ignored, clear removes the current goal
  from the panel, and invalid budgets fail locally before issuing an API call.
- `goals/view.ts` renders the compact right-side workroom control: set a goal
  when no goal exists, view status and token usage when one exists, edit the
  objective/budget, pause or resume, clear, and request public visibility.
  The chat page only imports the dock and remains below the architecture line
  budget.
- `initialCommands` loads the current goal for chat workrooms alongside the
  workspace snapshot, files, team messages, and composer focus command. Product
  policy now catalogs the three goal commands so browser command intent remains
  centrally named.
- Regression coverage includes initial command loading, reducer save/action
  payloads, and a Foldkit scene assertion for the goal panel in the workroom
  side rail.

This is intentionally not the final public/realtime projection layer. Issue #52
still needs the goal sync collection and reducers so usage and status can update
from live snapshots/streams instead of explicit command responses.

### 2026-06-04 Public Goal Projection And Artanis Route

Issue #51 added the first anonymous public projection layer for public agent
goals:

- `workers/api/src/agent-goal-public-projection.ts` defines
  `AgentPublicProjectionService`, `PublicAgentGoalSnapshot`, and
  `PublicAgentGoalStreamEvent`. The service reads only public current goals or
  public goal IDs, projects private ledger events into compact public event
  summaries, and rejects unsafe projected records with
  `AgentGoalPublicProjectionUnsafe` if provider-secret material appears at the
  service boundary.
- `AgentGoalEventRepository.listByGoal()` now provides the bounded event feed
  needed for public snapshots. It reads the private event ledger in chronological
  order but returns those records only to the projection service, never directly
  to anonymous routes or browser UI.
- Public goal APIs now return `{ agentId, goal, events }` for both goal and
  agent entrypoints. The supported anonymous snapshot routes are:
  - `GET /api/public/agents/:agentId/goal`
  - `GET /api/public/agents/:agentId/current-goal`
  - `GET /api/public/goals/:goalId`
  - `GET /api/public/goals/:goalId/snapshot`
- Public API events intentionally include only `id`, `goalId`, `runId`, public
  event `type`, public `status`, safe `summary`, token/time deltas, and
  `createdAt`. They do not include `payloadJson`, external event IDs, callback
  refs, auth grant refs, hidden steering text, expected-goal IDs, raw runner
  output, or private scope fields.
- `apps/web/src/route.ts` now supports `/agents/:agentRef` as `PublicAgent`.
  Startup policy keeps that route public for logged-out visitors, complete
  authenticated users, and incomplete-onboarding authenticated users. Auth
  bootstrap is skipped for the route so anonymous viewers do not enter the
  product shell.
- The logged-out Foldkit model now has a typed public-agent state machine
  (`idle`, `loading`, `loaded`, `failed`) plus `LoadPublicAgentGoal`, which maps
  `/agents/artanis` to the durable `agent_artanis` identity and decodes the
  public snapshot DTO through Effect Schema.
- `apps/web/src/page/loggedOut/page/publicAgent.ts` renders the current public
  Artanis goal and sanitized activity list as the first public observer page.
  The page reads only the public DTO, so hiding sensitive fields is not a UI
  convention; those fields never reach the model.
- Regression coverage now proves public snapshots omit private scope fields,
  `payloadJson`, external event IDs, callback-token-like refs, auth grant refs,
  hidden steering text, and raw `auth.json` references. Browser tests prove
  `/agents/artanis` skips auth bootstrap, loads `LoadPublicAgentGoal`, stays
  outside the authenticated shell, and renders the sanitized public event
  projection.

This closes the public projection/page foundation. The live Durable
Object/WebSocket cursor stream remains intentionally in issue #52, because it
requires changing the sync scope parser, outbox publication, browser reducers,
and cursor-gap reload behavior as one coherent sync-system change. The #51
route already exposes public stream state as snapshot records; #52 should make
those same projected records live through public sync scopes such as
`public-agent:agent_artanis` and `public-goal:{goalId}`.

### 2026-06-04 Goal Sync Projections And Reducers

Issue #52 connected goal state to the OpenAgents Sync substrate:

- `packages/sync-worker/src/index.ts` now names public sync scopes:
  `public-agent:{agentId}`, `public-goal:{goalId}`, and
  `public-agent-run:{runId}`. Existing authenticated scopes remain
  `workspace:{userId}`, `team:{teamId}`, `thread:{threadId}`, and
  `agent-run:{runId}`.
- `workers/api/src/sync-routes.ts` now parses the public scope kinds. Public
  snapshot and stream requests bypass browser-session auth, while public
  mutations still return `404` and never enter the mutation path.
- `workers/api/src/sync-notifier.ts` defines the goal sync collections:
  `agent_goals`, `agent_goal_events`, `public_agent_goals`, and
  `public_agent_goal_events`. Private goal records publish to workspace, team,
  agent-run, and thread scopes as appropriate. Public records publish only when
  visibility is `public`, and archived/private transitions emit deletes into
  public scopes.
- Runtime goal accounting in `workers/api/src/omni-handlers.ts` now publishes
  the updated goal and goal event after each committed goal runtime event. This
  covers launch acceptance, dispatch, callback status, usage accounting, and
  continuation events. Route-level browser/operator/agent mutations also
  publish the changed goal through the same notifier helper.
- Public event sync values reuse `publicAgentGoalEventFromRecord()` from the
  public projection service. That keeps public sync records on the same
  sanitized boundary as the public snapshot APIs and avoids pushing
  `payloadJson`, external event IDs, callback refs, hidden steering, or raw
  runner material into public collections.
- Browser sync projection now parses `agent_goals` records into the existing
  `AgentGoalApiGoal` shape. The logged-in sync reducer hydrates the current
  goal panel from snapshots and live patches when the active route scope matches
  the route's goal scope.
- Cursor gaps continue to use the existing sync reload path:
  `ReceivedSyncCursorGap` marks the scope failed and issues `LoadSyncSnapshot`
  with `syncSnapshotHref(gap.scope)`. Because goal records now live in the
  scope collections, the reloaded snapshot repopulates the goal dock through the
  same reducer.

Regression coverage now includes public anonymous sync snapshots, public
streams, rejection of public mutations, public route projection safety from
#51, browser goal projection from sync collections, and reducer-level live goal
patch application.

Local verification for the tracked #52 work passed `bun run typecheck` and
`bun run test`. The architecture script was blocked in this working tree by
unrelated local pylon-route work (`workers/api/src/index.ts` plus untracked
`workers/api/src/public-pylon-stats.ts`), which the script scans even though it
is not part of this issue commit. On a clean tracked checkout, the #52 changes
do not add new architecture-budget findings.

### 2026-06-04 Goal Hardening Guardrails

Issue #53 added explicit regression guardrails around the completed goal
system:

- `workers/api/src/agent-goal-hardening.test.ts` statically checks that goal
  route handlers do not own SQL directly. Goal behavior must keep flowing
  through `AgentGoalRepository`, `AgentGoalRuntimeService`, and their Effect
  layers rather than route-local D1 statements.
- The same hardening test verifies the private/public projection split for sync
  records. Private `agent_goals` values may include owner/team/project
  authority fields, while `public_agent_goals` and `public_agent_goal_events`
  omit private scope fields, raw payload JSON, external event IDs, callback
  token refs, auth grant refs, `auth.json` references, and hidden steering.
- Agent-facing `update_goal` remains terminal-only. Schema decoding accepts
  `complete` and rejects statuses such as `paused` and `budget_limited`, so the
  model-facing API cannot pause, resume, edit, clear, or force runtime-owned
  limited states.
- Browser goal/sync/public-agent sources are checked for direct SHC/OpenCode
  control-plane strings. The browser continues to call OpenAgents API/sync
  surfaces, not runner substrate endpoints or callback/grant material.

This supplements the existing repository, route, runtime, UI, public
projection, and sync tests added across issues #45 through #52. Together those
tests cover durable D1 goal lifecycle, status transitions, stale goal IDs,
`agent_runs.goal_id` linkage, agent tool constraints, runtime accounting,
bounded continuation decisions, idempotent callback accounting, API validation,
logged-in goal controls, public projection safety, public sync scopes, and live
goal-panel hydration from sync patches.

Local verification for #53 passed:

- `cd workers/api && bun run test src/agent-goal-hardening.test.ts`
- `bun run typecheck`

The full architecture script remains blocked in this working tree by the same
unrelated pylon-route files noted under #52. They are intentionally not part of
the #53 commit.

### 2026-06-04 Pylon Stats And `/artanis` Campaign Surface

The Pylon campaign surface now connects the public Artanis route to the same
Nexus telemetry family that the previous Laravel site showed at `/stats`:

- `workers/api/src/public-pylon-stats.ts` normalizes the public Nexus payload
  from `https://nexus.openagents.com/api/stats` into a compact DTO for
  anonymous browsers. It keeps the old Pylon counters: online identities,
  sellable identities, online sessions, identities seen in the last 24 hours,
  accepted-work sat totals, 24-hour accepted-work sats, contributors to
  training, hosted relay URL, refresh timestamp, and recent-Pylon rows.
- `GET /api/public/pylon-stats` returns that DTO with
  `Cache-Control: no-store`. If the Nexus fetch fails or the response is marked
  as a recovery-proxy cached copy, the route returns an explicit unavailable
  snapshot instead of pretending stale telemetry is live.
- The logged-out browser model adds a typed `PublicPylonStats` state machine
  and `LoadPublicPylonStats` command. Public Artanis startup now loads both the
  public durable goal snapshot and the public Pylon stats snapshot.
- `apps/web/src/page/loggedOut/page/publicAgent.ts` renders `/artanis` as the
  campaign/proof page: campaign objective, current public goal when available,
  Nexus connection, Pylon counters, recent Pylon rows, and sanitized public
  Artanis activity.
- `apps/web/src/route.ts` keeps `/agents/artanis` as the canonical public-agent
  shape and adds `/artanis` as the livestream/referral-friendly short route.
  Both routes skip auth bootstrap and remain outside the private product shell.
- Regression coverage checks the Nexus normalizer, no-store response behavior,
  stale recovery-proxy handling, public route alias, public command loading,
  rendered Pylon stats, and the absence of private fields in the public page.

This makes `https://openagents.com/artanis` a usable public proof link before
the durable live sync stream is finished. The public route can honestly show
what Artanis is trying to build, whether a durable public goal has already been
published, and how many Pylons are connected to Nexus right now without leaking
operator-only run data.

## Speculation: Combining With OpenAgents product surface Long-Running Agents

OpenAgents product surface could combine this pattern with long-running agents by making a persisted `AgentGoal` or `ThreadGoal` Effect service the durable authority for agent objectives, then letting worker sessions report typed runtime events such as `TurnStarted`, `ToolCompleted`, `CheckpointPersisted`, `UsageLimitReached`, `ExternalSet`, and `WorkerResumed` into one policy interpreter that owns accounting, continuation scheduling, and hidden steering. The useful adaptation is not to copy Codex's SQLite details directly, but to preserve the contract: user/API controls own objective replacement and pause/resume, model tools own only explicit create and terminal complete/blocked claims, Cloudflare D1/Durable Object state stores one current goal identity with expected-ID guards, Queues/Workflows resume active goals only after notification snapshots are durable, and all long-running agent restarts replay from persisted goal state rather than from chat memory.
