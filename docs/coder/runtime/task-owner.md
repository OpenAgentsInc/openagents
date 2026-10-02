# Local task ownership and evidence

The opt-in task host runs an explicitly admitted `bounded-command` request.
It reuses `coder-boundary` for filesystem enforcement, `supervise` for process
ownership and resource limits, and ATIF for the transcript. It does not add an
agent loop. [#9673](https://github.com/OpenAgentsInc/openagents/issues/9673)
tracks ownership and [#9675](https://github.com/OpenAgentsInc/openagents/issues/9675)
tracks reconstructed evidence views. The terminal and `coder -p` keep their
existing shared turn. The [repository Microcoder adapter](microcoder-repository.md)
uses this same owner through an explicit foreground model grant.

## Admission and authority

Submission remains inert. The local operator supplies a separate closed grant
with schema `openagents.coder.task-execution-grant.v1`, task ID, intent digest,
expected revision, canonical executable path, arguments, workspace write
permission, wall seconds, per-stream bytes, and process memory bytes. The host
records the exact grant bytes and digest, executable digest, canonical
workspace, observed Git revision, and a complete filesystem snapshot before
recording an effect intent. Missing or mismatched pins refuse execution.

The task must request adapter `bounded-command` and a null model. This adapter
cannot claim model identity, effort support, a dollar ceiling, or known billing.
Limits are 1–3,600 seconds, 1 KiB–1 MiB per captured stream, and 64 MiB–8 GiB
process memory. The supervisor records cleanup; an exit code is an executor
outcome, not independent correctness. Cost remains unknown.

The store must be outside the workspace. Writes are denied outside the granted
workspace; the store and common Git directory remain protected. Writing requires
an isolated Git worktree whose common Git directory is outside it. Read-only
commands can use a normal checkout. Reads are confined to the workspace,
executable, and the boundary's documented system directories. Environment
variables are cleared. On macOS the existing offline boundary denies external
IP traffic but permits localhost; Linux uses an isolated network namespace.
Those different capabilities are recorded, not described as equivalent network
isolation. Do not use this fixture adapter to promise a paid execution budget.

The immutable intent, grant, and observed source pin are distinct: knowing a
prompt or editing a task file does not supply an execution grant through the
public command API. Local OS-user access is the trust boundary. This is neither
remote attestation nor a multi-host lease service.

## Owner lifecycle

Each task has a stable `owner-TASK_ID.lock`. The execution owner holds its OS
lock until it stops. Each task's short write lock serializes its command and
host-event journal mutations. A new client can read or request cancellation without taking
the execution lock. A second owner cannot execute the same task. Within one
store, an unresolved admitted task also blocks admission of another task on the
same canonical workspace or an overlapping subtree, until it is resolved or
its owner is gone (see [A run whose owner ended](#a-run-whose-owner-ended)).

`coder task start --grant GRANT.json` retains the exact grant, creates a private
launch diagnostic file, and starts a separate session with detached standard
input and output. Its response means the owner process started; `admission:
"pending"` is not an admission receipt. Inspect `show` and the returned diagnostic
path for admission or refusal. `execute` is the foreground host entry point.
The detached host continues after the starting client exits.

A durable effect intent precedes dispatch. The host records the spawned process
identity when it learns it; the gap between spawn and recording remains
ambiguous after a crash. Cancellation returns `cancel_requested` with execution
still running. Only the supervisor's result can report confirmed process-group
cleanup. A request accepted before dispatch starts no command. Output is captured
into ATIF as it arrives, with byte offsets, raw bytes, and explicit dropped spans.
The host stops after retaining at least 2 MiB of stdout; the current delivery and
cleanup remainder can add up to two stream caps. stderr retains its declared cap.
A stopped or capped run never becomes independently verified.

`coder task recover TASK_ID` must acquire the abandoned owner's lock. When the
run recorded a process whose process group is still there, it advances the
ownership epoch and records execution as `unknown`; it does not rerun the
command, kill a recycled PID, or assert that descendants have stopped. A process
killed with SIGKILL can leave descendants behind. Automatic retry of an
uncertain writing effect is unsupported. A finished task is also not rerun by
repeating `start` or `execute`.

### A run whose owner ended

An unknown run is no longer sticky
([#10124](https://github.com/OpenAgentsInc/openagents/issues/10124), owner
direction: things just work, with no decision prompt). `Store::settle` ends a
running, stopping, or unknown run with no result once nothing of it is left:
its owner's OS lock is free (a live owner holds it for as long as it runs, and
a recycled process ID cannot) and no process group the run recorded exists.
It records the run's result with ending `owner_process_ended`, no exit code,
`group_clear` meaning the owner's lock was free and no recorded group was
left, unknown cost, and the digest of the trace as the owner left it. The
task becomes finished, its execution failed (stopped when a stop was
asked). Checks whose owner is gone become `unavailable`. It kills nothing,
reruns nothing, and keeps every earlier record; the result is the only
event it adds. A run whose own result already said its process group was not
confirmed clear stays `unknown`.

The store settles runs automatically: before any admission, every run
overlapping the new task's workspace; on each host auto-start sweep, every
run; and on a device's command or cancel for that task. `recover` settles
before falling back to `unknown`. An admission that a live run still blocks
refuses with `workspace_busy`, which a device reads as
`Couldn't start: another Coder task is still running in this project`.

## Durable journal and compatibility

The task store v3 ([#10231](https://github.com/OpenAgentsInc/openagents/issues/10231),
design in [2026-10-02-task-store-v2.md](2026-10-02-task-store-v2.md)) keeps each
task in its own file, `task/<id>.json`: the task, its accepted command bytes and
receipts, and its owner events, in one ordered journal. Reading a task replays
that journal and compares the task and every receipt. A write takes only its
task's lock (`task/<id>.lock`) and replaces the file atomically; it holds the
stable `tasks.lock` only for the moment it reserves a command identity in the
append-only `identities.log`. An admission or check intent also holds
`workspaces.lock` while it checks every other task. Readers take no lock.
Receipt and event sequences count per task.

A v2 (or v1) single-document store migrates once, on the first open of a
binary that knows v3, under the stable lock: the document is validated as
before, each task's file is written, then `store.json` marks the store as v3
and `tasks.json` is kept as `tasks.v2.json`. Older binaries then refuse the
store instead of starting an empty one. Private files, atomic replacement,
fsync, bounded storage, and refusal after ambiguous writes retain the inbox's
guarantees. Do not delete a stable lock or recreate a missing `store.json`.

The local filesystem and OS lock are the enforcement domain. Separate stores,
network filesystems, distributed ownership, hostile same-user rewriting, and
uncooperative external workspace writers are outside this guarantee. External
changes detected between admission and dispatch refuse the attempt; the snapshot
is an observation rather than a global lock on every writer.

## Reconstruct a view

```sh
coder task show TASK_ID
coder task view TASK_ID --limit 100
coder task view TASK_ID --limit 100 --cursor '{"schema":"..."}'
```

Use the complete `evidence.next` object from the previous page as the cursor;
the abbreviated example is not a valid cursor. The library entry point is
`coder::task::view::read`. A view reads the validated task journal and the original
ATIF bytes in a fresh process. It includes full recorded steps, run and source
identity, the candidate snapshot, executor result, transcript faults, and known
or unknown cost. A cursor binds task, intent, trace filename, and the delivered
step prefix; changing that prefix refuses reconnect. Appending steps does not
invalidate it. A page limit is 1–200 steps. A read is bounded to 64 MiB of trace
bytes and labels an oversized trace explicitly.

Evidence states distinguish not started, missing, unsafe/unavailable, unreadable,
malformed, damaged, incomplete, unsealed, digest mismatch, and sealed. Damaged
logs retain recoverable steps and report every parser fault. A sealed transcript
must match the digest in the owner result. The reader never substitutes an
empty success for a missing transcript or treats an intact transcript as an
independent check. An owner failure can leave an intact but unsealed transcript;
that is still an unknown execution disposition.

Views grant no execution or disclosure authority. This API currently serves
local OS-user reads. Recipient admission, private remote artifact delivery,
multiple devices, and Nostr session/control transport retain their separate
migration acceptance gates.

## Retained candidate artifacts

The owner retains changed file bytes outside the writable workspace. Each file
is bound to the exact completed snapshot before it enters a content-addressed
blob. The manifest records additions, changes, removals, renames, directories,
and symlink targets. Reads use descriptor-relative paths and refuse parent
symlinks, leaf symlinks, and hard-linked regular files. The reader never follows
a retained symlink to retrieve another file.

Retention permits 4,096 changes, 1 MiB per file, and 8 MiB of file content per
candidate. Oversized, missing, unsafe, or changed files make the manifest
incomplete; they are not silently omitted from a complete delivery claim.
`coder task view` includes the manifest and current blob faults.
`coder task artifact TASK_ID --path RELATIVE_PATH` returns the exact retained
bytes and digest for a manifest entry, even after the workspace file disappears.
It refuses arbitrary paths and missing or altered blobs. Artifact availability,
execution success, and independent correctness are separate observations.

## Frozen requirements and independent checks

The optional grant field `requirements` uses schema
`openagents.coder.task-requirements.v2`. It names a positive version, a nonempty
list of requirement IDs and statements, a check ID list for each requirement,
a protected `openagents.verification.v1` plan, instruction targets, and source
exclusions, current task sources, and independent check-source lineage. Every check must cover a declared requirement. Each check requires
typed suite evidence; a bare successful exit cannot satisfy this contract.
The plan's input and each suite input must be the literal `{candidate_digest}`.
Only the host replaces that placeholder after independently observing the
completed candidate. Suite and capability manifest digests are frozen before
execution. Each suite digest is the SHA-256 of its canonical external executable,
not an arbitrary suite label. Both the program and its manifest must live outside
the executor's writable workspace. The only accepted argument is the candidate
digest; shell command strings and executor-written test scripts are not admitted
as independent suites. The operator remains responsible for the protected
program's coverage and dependencies.

The grant also accepts `expected_source_snapshot`, the raw SHA-256 digest from
`coder_boundary::Snapshot::digest`. If supplied, it must match before admission.
The host rechecks source and executable identity immediately before dispatch.
A Git commit pin alone does not identify uncommitted files.

At admission, the host retains the effective user prompt and the root and scoped
`AGENTS.md` files for declared file or directory targets, with their exact bytes,
paths, scopes, and digests. Ancestor instructions precede nested instructions.
These texts are context, not execution authority. The original prompt and every
accepted correction remain in the journal. Optional exact Markdown knowledge
pins are opened within the workspace, validated against their ID, version,
digest and declared provenance, and retained in the context. Source overlap
with the current task or exclusions refuses admission. The repository adapter
requires explicit `frozen-context` configuration to deliver this text; the
bounded-command adapter makes no model-delivery claim. No ambient retrieval
runs. [Frozen task context](frozen-task-context.md) specifies the bounds,
declaration limits, and compatibility with retained v1 history.

Run the independent plan explicitly:

```sh
coder task check TASK_ID
```

This loads the operator's existing capability trust store and reuses
[`coder::verification`](../../../crates/coder/src/verification.rs); it does not
use executor-written acceptance tests or introduce a second verification engine.
Checks run against a read-only candidate with supervisor bounds and exact typed
suite identities. Journal replay validates the full typed evidence and derives
the verdict again, rather than trusting an outer `passed` label. The existing
checker boundary requires explicitly declared
unrestricted reads and networking; those permissions do not extend the executor's
grant. Capability approval remains required. Missing manifests, stale candidate
bytes, incomplete observations, absent suite output, and mismatched identities
produce unavailable evidence, never a pass. A failed suite remains failed.

The host reserves overlapping workspace trees while execution is unresolved or
checks are running. A second admitted owner or checker in the same store cannot
write that tree concurrently. An external writer is not fenced by this local
reservation; changed observations make the result unavailable.

Check intent and its result are separate durable events. A lost checker owner
recovers to `unavailable` without silently rerunning it. A successful process
remains execution `finished` even when independent checks fail. The task view
always reports integration as `not_attempted`; this path never commits or merges
a candidate automatically. Model judgments, retained transcripts, and the older
#9584 studies cannot supply a mechanical pass.

## Corrections

`coder task correct --file COMMAND.json` accepts the existing closed command
format with action `{"type":"correct","prompt":"...","reason":"..."}`
and the current `expected_revision`. It appends the replacement instructions
without changing the original intent, grant, effects, cost state, or transcript.
A queued task can be admitted only at its new revision. A running task requests
cancellation; it cannot reuse the old context for further admitted work. A
completed result becomes disputed. If independent checking is already running,
its evidence is retained, but its final disposition becomes disputed rather
than verifying the corrected request. A replacement execution requires a new
explicitly admitted task; automatic replanning is not implemented by this adapter.

### Steering and acknowledgment

A correction is a steer at the task level. Each adapter states how its engine
takes one, as a `coder_one::steering::Steering` row: its native mode, the
emulation it offers, and the evidence that confirms consumption. Both
task-owner adapters, `microcoder-repository` (`coder::task::adapter::STEERING`,
also exported as `microcoder::STEERING`) and `bounded-command`, are
`turn_boundary` engines without emulation: a run reads its instructions once,
at admission. Their acknowledgment is `next_turn_start`. The admission's
context records the task revision it read, and the run's trace records each
correction it consumed as its own System step with a `steer_consumed`
extension (the correction's revision and reason, the adapter, and the
acknowledgment), after the user step and before the admission step.
The repository adapter also lists its row under `capabilities.steering`. A
grant whose route is the Devin CLI lists `coder_delegate::steering::DEVIN_ACP`
instead: also `turn_boundary` with `next_turn_start`, with cancel and continue
as its emulation (see [the Devin route](devin.md#steering)). An OpenCode
route's grant lists `coder_delegate::steering::OPENCODE_ACP`, the same row
(see [the OpenCode route](opencode.md#steering)). A Grok Build route's grant
lists `coder_delegate::steering::GROK_ACP`, the same row (see [the Grok Build
route](grok.md#steering)).

Accepting a correction is not consuming it. `Task::unconsumed_steers` is the
ledger of routed corrections that no admitted run has read yet: those newer
than the revision the current run's admission records. `Steering::admit`
refuses native mid-turn steering to an engine without it unless the caller
chose the engine's emulation, and never picks emulation for the caller.

## Turns and device commands

A task can take more than one turn. `{"type":"continue","prompt":"..."}` at
the current revision starts the next turn of a finished or cancelled task:
the task is queued again with the message as its effective prompt, the
earlier run moves to `earlier`, and the message is kept in `follow_ups`.
A running, queued, or unknown turn does not continue, a task takes at most
64 turns, and each turn needs its own execution grant at the new revision,
like a new task. Turn `N` writes `TASK.N.atif.jsonl` and records the effect
`TASK:N:command`. A later turn of the repository adapter carries the earlier
turns into its trace and its engine prompt; a bounded command does not.

`coder::task::commands` keeps the device command journal behind NIP-HOST
`task.command` in `commands.json`, beside the task document and under its
lock: one entry per device and command ID, its sender's grant and epoch, and
its state (`received`, `held`, `dispatching` with the exact task-owner
command, or a final outcome). `commands::decide` is the pure evaluation; its
rules are in [NIP-HOST](../../../nips/openagents/NIP-HOST.md#operations).
The resident host evaluates held commands every few seconds and passes each
continued turn to the auto-start policy. `commands::edit_queue` lists and
edits the held messages behind NIP-HOST `task.queue`: a 60-second edit lease
per task, kept in the same journal, holds queued messages while a device
edits them, and an edit keeps the original request beside the edited text
so a device's replay still matches.

A repository turn can end by asking. With `ask` on (the repository profile
turns it on), a Microcoder step that sets `ask` to `question` or `approval`,
runs no commands, and puts the question in `reply` ends the turn; the run's
result ending is `asked_question` or `asked_approval`
(`coder::task::interaction`), and the execution counts as finished. The
task then waits: its summary reports `waiting`, and an `answer` command
continues it with the answer as the next turn's message. In a run nobody
answers, such as a benchmark, a step that asks is told so and the loop goes
on.
