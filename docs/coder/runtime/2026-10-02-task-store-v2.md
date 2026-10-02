# Task store: one file per task (#10231)

Status: implemented 2026-10-02 (store schema `openagents.coder.task-store.v3`).
The filename keeps the issue's working title, "task store v2"; the on-disk
schema it replaces was already called v2.

## Problem

`~/.openagents/tasks/tasks.json` held every task, every accepted command and
every owner event in one JSON document (4.5 MB on CoderOS). Every open took one
exclusive lock (`tasks.lock`) for the store handle's whole life and replayed
the whole journal; every save rewrote and fsynced the whole document. The host,
issue flows, followers and `chat work` all shared that lock, so callers waited
seconds and device-facing opens (5 s) refused with `store_busy` (#10213 was
patched by waiting 120 s).

## Layout

```
~/.openagents/tasks/            0700, a real directory
  tasks.lock                    the stable index lock (same file as before)
  store.json                    marker: {"schema":"openagents.coder.task-store.v3"}
  identities.log                append-only: one line per accepted command identity
  task/                         0700, a real directory
    <task_id>.json              one task: state + its own commands and owner events
    <task_id>.lock              the task's write lock (never removed)
  tasks.v2.json                 the migrated v2 document, kept as a backup
```

A task file is `{"schema":"openagents.coder.task-file.v1","task":…,"commands":[…],
"host_events":[…]}`: the same accepted-command bytes and receipts and the same
owner records the v2 document kept, but only this task's. Reading it replays its
own journal and compares the task and every receipt, exactly as v2 did for the
whole document, so a forged or truncated fact is still refused.

## Writes

Every file is written atomically: write a private temp file in the same
directory, fsync it, rename over the target, fsync the directory. A temp file
left by a crash starts with `.` and is never read; the next writer of that task
removes it.

- **A command** (`Store::apply`) takes only its task's lock. Under it: read the
  task file; an exact retry returns the original receipt (after an fsync
  barrier on the file and directory, so a visible-but-unsynced write never
  acknowledges); different bytes under a known identity are a conflict. The
  transition is computed against the current task (expected revisions are
  enforced here). Then the *index lock* is taken for milliseconds to reserve
  the command identity: `identities.log` is read, an identity already reserved
  for another task is a conflict, the global limits are checked
  (`MAX_COMMANDS` identities, `MAX_TASKS` submitted tasks), and one line is
  appended and fsynced. Finally the task file is replaced. Reservation precedes
  the task write, so a crash between them leaves a reservation without a
  receipt: a retry of the same command for the same task proceeds, and nothing
  else can take that identity. A refused command reserves nothing.
- **An owner event** (`Store::record`) takes its task's lock. An event that
  reserves a workspace (admission, check intent) also takes `workspaces.lock`
  and reads every other task file, so the #10124 rule (no two unresolved runs on
  overlapping trees) holds across processes. Settling dead runs happens before
  any lock is taken.
- Lock order is always task lock, then at most one of the index or workspace
  lock. Nothing holds two task locks.

## Reads

`show`, `list` and every follower read the atomically replaced task files with
no lock at all. `Store::open` takes the index lock only to initialise or
migrate; it no longer holds anything for the handle's life. A `Store` is now a
handle on the directory, and each call sees the latest committed state.

## Sequences

A receipt's and an owner record's `sequence` is now per task (strictly
increasing within the task's file) instead of global. Nothing outside the store
read it as global.

## Invariants kept

- No silent pruning: tasks, commands and events are never dropped; limits
  refuse new work (`limit_exceeded`). `MAX_TASKS` and `MAX_COMMANDS` stay
  global (counted in `identities.log`); `MAX_HOST_EVENTS` (8192) now bounds each
  task, and each task file is bounded by `MAX_STORE_BYTES` (16 MiB).
- A command identity names the same bytes forever, across tasks and actions.
- Expected revisions are enforced under the task lock.
- Private files (`0600`, no symlinks, one link) in private real directories.
- A refused or ambiguous write leaves the handle refusing until reopened.
- A missing marker in an initialised store is never recreated.

## Migration

`Store::open`, under the index lock, finds `tasks.json` and no `store.json`:
it validates the v2 (or v1) document exactly as before, writes each task's file
from that task's commands and events (temp files fsynced, then renamed), writes
`identities.log` with every legacy command identity, writes `store.json`, and
only then renames `tasks.json` to `tasks.v2.json` (never overwriting an earlier
backup). A crash at any point before the marker re-runs the migration from the
untouched `tasks.json`; after the marker, the leftover `tasks.json` is moved to
the backup. An older binary then finds no `tasks.json` beside an existing lock
and refuses (`corrupt_store`) instead of starting an empty store.

## Measurement

`task::tests::measure_create_beside_a_thousand_tasks` (`--ignored --nocapture`),
a debug build on a Boat `large` sandbox: open the store and create one task
beside 1,000 existing tasks of about 4 KB each (a 9.5 MB legacy document).

| | open + create one task |
| --- | --- |
| v2, one document | 513-538 ms (five runs) |
| v3, one file per task | 8.1-13.0 ms (five runs) |

The one-time migration of that document took 1.6 s; listing all 1,005 tasks
afterwards took 0.41 s (a reader, holding no lock). Eight processes each
creating a task and steering it ten times at once
(`processes_writing_different_tasks_never_wait_on_each_other`) saw a slowest
single open + apply of 42-58 ms.
