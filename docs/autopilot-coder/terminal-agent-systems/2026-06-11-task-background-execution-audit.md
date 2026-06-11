# Task And Background Execution Audit

Date: 2026-06-11

This is system #9 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should represent foreground work, background work,
subagents, remote jobs, output streams, cancellation, notifications, and
closeout receipts.

## Target

Build a task supervisor that makes concurrent work explicit. The user should be
able to see what is running, what completed, what failed, what was cancelled,
and where the evidence lives.

The task system should support native work, delegated local work, delegated
remote work, scheduled work, external adapters, and deterministic test jobs
without making each runner invent its own lifecycle model.

## User-Visible Capability

The user should see:

- A stable task id for each foreground or background unit of work.
- A short description and current status.
- Live or resumable output.
- Whether the task is blocking the current turn.
- Whether it can be stopped.
- Completion notifications that re-enter the main session.
- Failure summaries that include next action and evidence refs.
- Closeout receipts for artifacts, patches, verification, and usage.

Background work should not disappear into prose. If several lanes are running,
the task state should show that directly.

## Core Design

Define a `TaskSupervisor` service that owns task lifecycle and delegates actual
execution to typed runners.

Suggested service boundary:

```ts
interface TaskSupervisor {
  create(request: TaskCreateRequest): Effect.Effect<TaskRecord, TaskError>
  start(taskId: TaskId): Stream.Stream<TaskEvent, TaskError>
  stop(taskId: TaskId, reason: StopReason): Effect.Effect<TaskRecord, TaskError>
  list(filter: TaskFilter): Effect.Effect<ReadonlyArray<TaskRecord>, TaskError>
  output(taskId: TaskId, cursor?: OutputCursor): Stream.Stream<TaskOutputChunk, TaskError>
}
```

Runners should be plugins behind the supervisor. They emit events; the
supervisor persists state, handles cancellation, stores output, and produces
notifications.

## Task Model

Use a versioned task record:

- Task id.
- Kind.
- Parent run id.
- Optional parent task id.
- Description.
- Owner or adapter ref.
- Workspace ref.
- Permission snapshot ref.
- Status.
- Started and ended timestamps.
- Output ref.
- Artifact refs.
- Usage refs.
- Cancellation capability.
- Notification state.
- Public-safe summary.

Recommended statuses:

- `pending`
- `running`
- `waiting_for_approval`
- `waiting_for_dependency`
- `completed`
- `failed`
- `cancelled`
- `killed`
- `expired`

The state machine should reject impossible transitions such as completed back
to running unless represented as a new retry attempt.

## Task Kinds

The system should support task kinds without hard-coding policy into the enum:

- Foreground shell command.
- Background shell command.
- Native agent subtask.
- External agent subtask.
- Remote environment job.
- Scheduled job.
- Monitor or watcher.
- Verification job.
- Fixture job for tests.

Each kind should declare required authorities, output semantics, cancellation
support, and closeout requirements.

## Event Shape

Task events should be append-only:

- `task.created`
- `task.started`
- `task.output_appended`
- `task.progress_recorded`
- `task.waiting_for_approval`
- `task.approval_resolved`
- `task.artifact_recorded`
- `task.usage_recorded`
- `task.completed`
- `task.failed`
- `task.cancel_requested`
- `task.cancelled`
- `task.killed`
- `task.notification_enqueued`
- `task.notification_delivered`

Every event should include task id, run id, sequence, generatedAt, visibility,
and redaction class.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for the task supervisor.
- `Stream` for output and task events.
- `Queue` for notifications and task commands.
- `Fiber` for background execution.
- `Scope` for process handles, temporary directories, and log writers.
- `Deferred` for joins, approvals, and stop acknowledgements.
- `Schedule` for polling remote jobs and retrying transient reads.
- `Layer` for runner implementations and test fixtures.

Task runners should be interruptible Effect programs. Cancellation must close
scopes and settle state deterministically.

## Notification Model

Background task completion should not require polling by the model. The task
system should enqueue structured notifications that the conversation engine can
admit as normal user-visible events.

A notification should include:

- Task id.
- Status.
- Summary.
- Result or failure category.
- Output ref and cursor.
- Artifact refs.
- Workspace or branch refs when relevant.
- Usage refs.
- Recommended next action.

Notifications must be idempotent. A task that already delivered its completion
notice should not spam the conversation after resume.

## Output Storage

Task output should be streamed to the UI and written to an output store.

Output records should preserve:

- Source stream.
- Byte or line cursor.
- Redaction status.
- Truncation state.
- Start and end timestamps.
- Exit status when applicable.
- Artifact link when promoted to a receipt.

The model should receive summaries and bounded excerpts by default, not
unbounded raw logs.

## Safety Rules

- Background tasks inherit the permission and boundary snapshot from creation.
- Long-running tasks must have a stop path or explicit non-stoppable reason.
- Prompting for approval from a background task must route through the same
  permission service as foreground work.
- A failed task should preserve output and artifacts for diagnosis.
- Remote task polling must have timeout and backoff.
- Completion claims need evidence refs.
- Public task projections must not expose private paths, secrets, or raw
  provider payloads.
- Multiple task runners can execute concurrently, but state writes must be
  serialized or transactionally guarded.

## Tests

Minimum regression coverage:

- Create, start, complete, and list a foreground task.
- Run a background task and deliver exactly one completion notification.
- Stop a running task and settle as cancelled or killed.
- Persist output cursor and resume reading after process restart.
- Handle task failure with output and artifact refs preserved.
- Reject invalid lifecycle transitions.
- Poll a remote fixture until success, failure, and timeout.
- Route approval requests from a background task.
- Verify public projection redacts private paths and raw logs.
- Replay task events into the same final task record.

## OpenAgents Translation Notes

When promoted, map task records to OpenAgents assignment leases, artifact refs,
public-safe closeouts, and operator-visible receipts. Verify open and closed
issue state at the time of promotion before claiming implementation status.

## Decision

Background execution should be a first-class task system with typed events,
output refs, notifications, and receipts. Concurrency belongs in a supervisor,
not hidden inside terminal text or per-runner global state.
