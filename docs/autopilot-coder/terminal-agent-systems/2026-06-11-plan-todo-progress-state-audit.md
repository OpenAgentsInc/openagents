# Plan, Todo, And Progress State Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #10 from the Bun/Effect terminal-agent systems list. It defines
how plans, todos, task lists, progress updates, blockers, ownership, and final
closeout should be represented in a terminal coding agent.

## Target

Build one progress model that separates planning intent from executable tasks
and user-visible status. The model should help the user understand what the
agent is doing without allowing the model to fake completion through prose.

## User-Visible Capability

The user should see:

- A concise current plan when the work is non-trivial.
- A checklist of concrete tasks when useful.
- Exactly which item is in progress.
- Which items are blocked and why.
- Which tasks have owners or delegated agents.
- A final closeout that matches the task state.
- Verification status, not just implementation status.

The UI can compress completed work, but the durable state should retain the
full history.

## Concepts

Use distinct concepts:

- `Plan`: a model-authored strategy for the current work.
- `Todo`: a lightweight user-visible checklist item.
- `Task`: an executable or delegated unit tracked by the task supervisor.
- `ProgressEvent`: append-only status update.
- `Blocker`: a typed dependency or unavailable authority.
- `Closeout`: final summary with evidence refs.

Plans are not receipts. Todos are not proof of execution. Tasks and artifacts
carry evidence.

## Core Design

Define a `ProgressService` that owns plans, todos, blockers, and projections.

Suggested service boundary:

```ts
interface ProgressService {
  setPlan(request: PlanSetRequest): Effect.Effect<PlanRecord, ProgressError>
  updateTodo(request: TodoUpdateRequest): Effect.Effect<TodoList, ProgressError>
  recordProgress(event: ProgressEvent): Effect.Effect<void, ProgressError>
  linkTask(link: TodoTaskLink): Effect.Effect<void, ProgressError>
  closeout(request: CloseoutRequest): Effect.Effect<CloseoutRecord, ProgressError>
}
```

The task supervisor should own execution state. The progress service should own
the user-facing projection and integrity checks that connect todos, blockers,
tasks, and receipts.

## Durable State

Durable records:

- Plan versions.
- Todo list versions.
- Todo status transitions.
- Task links.
- Blocker graph.
- Owner changes.
- Verification markers.
- Closeout records.
- Progress summaries emitted to the user.

Ephemeral records:

- Spinner text.
- Temporary streaming status.
- UI expansion state.
- Draft plan text before acceptance.
- In-progress model reasoning.

The durable record should be replayable into the same user-visible progress
projection.

## Status Model

Todo status should be narrow:

- `pending`
- `in_progress`
- `blocked`
- `completed`
- `cancelled`

Task status can be richer, but the user-facing checklist should remain simple.

Rules:

- Prefer at most one active todo per agent unless a real parallel task system
  is linked.
- A todo cannot become completed solely because text says it is complete.
- A todo linked to a failed task needs explicit override or retry.
- A blocked todo must name the blocking condition.
- A final closeout must not claim completion for pending or blocked todos.
- Verification should be tracked separately from code changes.

## Event Shape

Progress events should include:

- `plan.created`
- `plan.revised`
- `todo.created`
- `todo.started`
- `todo.blocked`
- `todo.unblocked`
- `todo.completed`
- `todo.cancelled`
- `todo.linked_to_task`
- `owner.changed`
- `verification.started`
- `verification.completed`
- `verification.failed`
- `closeout.created`

Each event should include run id, progress id, sequence, generatedAt,
visibility, and evidence refs when available.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for progress state.
- `Schema` for plan, todo, blocker, and closeout records.
- `Ref` for active in-memory projection.
- `Queue` for task-to-progress events.
- `Stream` for UI progress updates.
- `Layer` for persistence and fixture substitution.
- `Schedule` for delayed progress reminders or stale-task checks.

The service should accept events from the conversation engine and task
supervisor, then project a stable progress view for the terminal UI and any
remote control surface.

## Blockers

Blockers should be typed:

- Waiting for user input.
- Waiting for approval.
- Missing credential.
- Workspace unavailable.
- Dependency task running.
- Dependency task failed.
- Network or provider outage.
- Budget or quota exhausted.
- Policy disallows action.
- External system status unknown.

Typed blockers let the runtime route recovery: ask the user, retry, continue
elsewhere, or close honestly.

## Safety Rules

- Do not let the model delete inconvenient progress state silently.
- Record plan revisions rather than overwriting history.
- Public projections should not expose private task output or raw internal
  notes.
- Closeouts must include evidence refs for implementation and verification
  claims.
- Blocked states should not be hidden as completed states.
- Progress updates from delegated work must pass through task events or signed
  adapter closeouts.
- If state storage fails, the conversation should surface degraded progress
  tracking instead of pretending persistence succeeded.

## Tests

Minimum regression coverage:

- Create and revise a plan.
- Create todo items and transition through pending, in progress, and completed.
- Reject invalid status transitions.
- Link a todo to a task and reflect task success.
- Keep a linked todo incomplete when the task fails.
- Record a blocker and require a blocker reason.
- Generate a closeout that refuses to mark pending work as complete.
- Replay progress events into the same projection.
- Redact private refs in public progress output.
- Resume after restart with the same active plan and todo state.

## OpenAgents Translation Notes

When promoted, map this system to OpenAgents assignment progress, public-safe
closeouts, artifact refs, and projection freshness. Verify the live issue and
roadmap state before saying a progress path is shipped, planned, or blocked.

## Decision

Planning should be useful, but it should not be confused with execution.
Represent plans, todos, tasks, blockers, verification, and closeouts as separate
typed records and derive the UI from those records.
