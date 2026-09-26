# Durable local tasks

`coder task` records requests on local disk so you can inspect or cancel them
after the submitting process exits. It is the first implementation slice of
the [suite migration](../migration-status.md), tracked by
[#9672](https://github.com/OpenAgentsInc/openagents/issues/9672).

Submission runs no agent, shell command, network request, or paid operation.
An explicit execution grant can run the `bounded-command` adapter through a
detached owner. See [task ownership and evidence](../runtime/task-owner.md) for
`start`, `execute`, `recover`, and paged `view` reads, admission requirements,
cancellation, and recovery limits. The terminal and `coder -p` retain their
existing shared turn; they do not automatically consume the inbox.

## Try the synthetic fixture

From the repository root, use an isolated store outside your checkout:

```sh
cargo run -p coder --bin coder -- task submit \
  --file docs/coder/fixtures/tasks/submit.json \
  --store "$HOME/.openagents/task-demo"
cargo run -p coder --bin coder -- task list \
  --store "$HOME/.openagents/task-demo"
cargo run -p coder --bin coder -- task show example-task-1 \
  --store "$HOME/.openagents/task-demo"
cargo run -p coder --bin coder -- task cancel \
  --file docs/coder/fixtures/tasks/cancel.json \
  --store "$HOME/.openagents/task-demo"
```

The [submit fixture](../fixtures/tasks/submit.json) names a synthetic
`/workspace/example` directory. No directory is inspected or created there.
The [cancel fixture](../fixtures/tasks/cancel.json) targets revision `1`, the
revision of a newly queued task. Cancellation advances it to revision `2`.

Without `--store`, the inbox uses `~/.openagents/tasks`. All successful
commands print JSON to standard output. Store and command refusals print
JSON to standard error and exit `1`; invalid CLI syntax exits `64`.
`coder task --help` prints the supported operations. `--file -` reads the
command from standard input with the same size limit.

## Record a request

Copy the submit fixture into a private file and change its command ID, task
ID, title, prompt, workspace path, and requested adapter/model identifiers.
Use a new task ID for a new request. Keep the exact file bytes for retries.
Do not put credentials or private service configuration in the request.
Task IDs and command IDs are distinct. A supplied source revision is a full
40- or 64-character hexadecimal commit ID, not a moving branch name.

The workspace path, optional source revision, and requested configuration
are **inert intent**. Queue admission does not validate that a repository
exists, the requested adapter is installed, a model is supported, the source
is current, or the caller has permission or budget to execute it. Those
checks belong to execution admission. A stored request cannot
grant authority to itself.

Tasks retain separate queue and execution status. A request cancelled before admission has not started execution and has not been checked. It has no
successful result, verified acceptance, integration result, token cost, or
trace to report. Do not interpret missing cost as zero-cost completed work.

## Retry and cancel

Command IDs cover submission and cancellation together within one store.
After a command is accepted, reusing its ID with different bytes refuses,
including whitespace changes
or a different action. Replaying the identical bytes returns the original
receipt without advancing the task or changing the state file. The receipt
describes that command's original transition; use `show` for the task's
current state.

Read `show` before making a new cancellation and copy its revision into
`expected_revision`. Cancellation requires a queued or running task and its current
revision. A new command with a stale revision refuses; retrying the exact
successful cancellation still returns its original receipt. Cancellation
of running work records a request that the owner observes. It does not assert
that a process has stopped; inspect the supervisor result.

The CLI checks that the file's action matches the selected subcommand before
opening the store. A cancel command passed to `submit` never cancels a task.
Unknown fields, duplicate JSON keys, invalid identities, and unsupported
schemas refuse rather than being ignored.

## Persistence and recovery

The store is private to the local OS user. On supported Unix hosts, its
directory uses mode `0700` and its files use mode `0600`. An existing
directory with broader permissions refuses without changing its permissions.
The store uses an OS-held
lock to serialize readers and writers, and atomically persists requests,
task transitions, and original command receipts together before returning
acceptance. A bounded wait on the lock can return a busy refusal; retry the
same command file. Read operations do not update an existing task document.

Corrupt, truncated, unsupported, or inconsistent state refuses. A missing
state file in an already initialized store does not become an empty inbox.
Do not delete a lock file or edit the state document to bypass a refusal.
Keep the complete store for diagnosis and restore from a consistent private
backup. Do not back up an in-flight partial file as authoritative state.

If a write succeeds but the process loses its output before you receive the
receipt, retry the identical command. If persistence cannot establish an
acknowledged result, the CLI returns an error; reconcile by reopening the
store and retrying those same bytes. It never invents another task identity
to resolve uncertainty.

The store accepts commands up to 64 KiB and retains at most 1,024 tasks,
2,048 accepted command receipts, and a 16 MiB state document. Capacity
exhaustion refuses new mutations; it does not discard idempotency history.
This is a local filesystem contract. It supplies neither distributed
fencing nor protection from a malicious process running as the same OS user.
Do not place the store on a shared filesystem and assume these local
guarantees establish multi-host ownership.

## Relationship to Nostr and execution

This local command schema is groundwork for the runtime contract. It is not
the signed NIP-SESS or NIP-CTRL wire format: those protocols add authenticated
actors, grants, epochs, canonical command fingerprints, expiry, disclosure,
and owner-side admission. A future transport must map those identities
explicitly. Exact-byte local retry behavior is deliberately stated apart
from NIP-SESS canonical JSON fingerprints.

The [local owner](../runtime/task-owner.md) now retains execution admission,
effects, ATIF views, artifacts, corrections, and independent checks. The
[Microcoder repository adapter](../runtime/microcoder-repository.md) runs its
existing loop through that host with an explicit foreground model grant.
The [migration tracker](../migration-status.md) separates each gate from
mobile, desktop, CoderOS, and labor delivery.
