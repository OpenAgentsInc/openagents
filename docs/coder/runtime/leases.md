# Leases

The host resource broker lets Coder, everything Coder delegates to, and any
other agent on a machine share scarce resources: build slots, memory, disk,
the quiet machine, the real screen, the GPU, and licensed tools. A command
takes a lease before it runs and gives it back when it ends. A request that
can't be admitted waits its turn.

The broker is the `coder-lease` crate
([`crates/coder-lease`](../../../crates/coder-lease/src/lib.rs)) and the
`openagents lease` command. It's a file-backed table, not a daemon, so it
works when the host isn't running and from any language through the command.
The design and its reasons are in
[Many agents on one machine](../design/many-agents-one-machine.md).

## Run a command under a lease

```sh
openagents lease RESOURCE [--amount N] [--priority P] [--no-wait] [--receipt PATH] -- CMD [ARGS...]
```

The command runs under `crates/supervise` in a process group of its own,
which holds the terminal while it runs, and `openagents lease` exits with its
status. For example:

```sh
openagents lease build -- cargo test -p coder-lease
openagents lease quiet --receipt bench/soak/lease.json -- scripts/grid-soak.sh
openagents lease memory --amount 24 -- ./train.sh
openagents lease gpu --no-wait -- verse --capture spawn.png
```

A request that can't be admitted waits in its resource's priority queue
([Priorities](#priorities)) and prints why it waits and who holds the
resource or goes first. `--no-wait` fails at once with exit code `1`
instead. Under `--json`, the receipt is printed as
one JSON document after the command's own output. `--json` after `--`
belongs to the command.

## Priorities

A waiting request has one of four priorities, most urgent first:

| Priority | For |
| --- | --- |
| `owner` | Work that blocks an owner request, such as a Coder task's builds. |
| `push` | A check before a push, such as the GitHub issue flow's checks. |
| `normal` | Everything else. It's the default. |
| `background` | Speculative work that can wait. |

- **Order.** Waiters are admitted most urgent first, then first in, first
  out within a priority. A held lease is never taken back: priority decides
  only who goes next.
- **Choosing one.** `--priority P` sets it for one command.
  `OPENAGENTS_LEASE_PRIORITY` sets the default for a session, and the
  flag wins over it. Commands that `openagents lease` wraps inherit the
  variable, and so do the `cargo` shim and the agents Coder delegates to.
- **Aging.** A waiter rises one level for each 20 minutes it waits, so a
  `background` request is `owner` after an hour and nothing starves behind
  a stream of urgent builds. `OPENAGENTS_LEASE_AGING_MINUTES` sets the step;
  `0` turns aging off. Within a level, the earlier arrival still goes first.
- **The quiet machine.** A held or queued `quiet` lease stops new `build`
  leases at every priority, `owner` included.
- **The list.** `openagents lease list` shows each waiter's place in its
  resource's queue (`waiting #1` goes next), its priority (with the level
  aging raised it to, as in `background (aged to push)`), and how long it
  has waited. `--json` adds `position`, `effective_priority`, and `wait_ms`
  to each lease, and `aging_minutes` to the table.

## Leased builds

```sh
openagents lease build [--keep-target-dir] [--priority P] [--no-wait] [--receipt PATH] -- CMD [ARGS...]
```

`lease build` takes one counted `build` lease and one of Coder's target
slots for the repository the working directory is in, sets
`CARGO_TARGET_DIR` to the slot, runs the command, and gives both back:

```sh
openagents lease build -- cargo test -p coder-lease
```

- **Slots.** The slot comes from the same pool Coder tasks use
  (`crates/coder/src/task/targets.rs`), beside the task store:
  `~/.openagents/targets/<project>-<digest>-slot-N`, four slots for each
  repository's common Git directory. `OPENAGENTS_TASKS` moves the task
  store and its slots with it. Coder tasks and conversation agents share
  one pool and one cap: a slot is pruned back toward 25 GB
  (`OPENAGENTS_SLOT_CAP_GB`, `coder.slot_cap_gb`) when its build ends, and
  the pool is trimmed to 64 GiB. When every slot of the repository is in use,
  the build waits for one.
- **Disk at admission.** A `build` lease is admitted only when the free
  space on the lease root's volume is at least the floor (10 GB,
  `OPENAGENTS_SLOT_FREE_GB`, `coder.slot_free_gb`) plus a disk budget for
  this build and for each build and `disk` lease already held (10 GB a
  build, `OPENAGENTS_BUILD_DISK_GB`). Short of that, `lease build` first
  runs class 1 of the disk cleanup on its own, which deletes the slots of
  ended sessions ([Slots of ended sessions](#slots-of-ended-sessions)) and
  the build caches of ended tasks with every check the cleanup makes, and
  looks again. If that isn't enough, it refuses, giving the free space,
  the space a build needs, and the floor, and naming the reclaim command,
  `openagents background run disk`.
- **The slot's floor.** Below the floor, taking a slot also reclaims idle
  caches in the slots first, and refuses the same way when that isn't
  enough.
- **`--keep-target-dir`** keeps a `CARGO_TARGET_DIR` the caller already set
  and takes no slot, for an agent that keeps its own long-lived target
  directory. Without it, the slot replaces the caller's directory.
- Outside a Git repository, the build takes the lease and no slot. A build
  inside another `build` lease takes neither and keeps the outer lease's
  target directory.

### Slots of ended sessions

When a `build` lease that took a slot ends, `lease build` measures the slot
and the holder's linked worktree (a checkout whose `.git` is a file; a main
checkout is shared, so it isn't counted) as allocated blocks, with each
hard link and each APFS clone family counted once across both. The bytes go
in the receipt ([Receipts](#receipts)). Then it writes `<slot>.lease.json`
beside the slot (schema `openagents.lease.slot-use.v1`): the slot, the
lease root, the lease, the holder's session, the agent process the session
runs in when one is among the holder's ancestors, and when the lease
ended.

The disk cleanup (`crates/background`, class 1) reads that record. A slot
whose session has ended is a class 1 candidate at once, with no idle time
to wait out. A session has ended when no lease in its table names it and
neither the process its identity names (`codex:4242`, `process:77`) nor its
recorded agent process still runs. A slot stays out of class 1 while:

- Its session still lives. The slot is then class 2's, which waits for the
  idle rule.
- Anything used the slot after the lease ended. Every slot lease writes the
  slot's lock when it starts, so a lock newer than the record means a
  later build, such as a Coder task run.
- A build holds it now. Its lock is held, and the cleanup's in-use checks
  keep it, as they keep every candidate.

The cleanup checks all of this while it plans and again right before it
deletes.

## Disk use per session

```sh
openagents lease du [--json]
```

`lease du` reads the receipts and reports, for each session, its agent,
whether it still runs, how many builds it ran, how long it held leases, and
the disk its builds' folders hold, largest first. A slot or worktree that
several receipts measured counts once, at its latest measurement, for the
session that built in it last, so the totals add up to what the folders
hold. `--json` prints each session with the folders behind its total.

### Coder's own builds

A Coder task run that builds (full access or this computer's toolchains)
holds a target slot for the whole run, as before, but no `build` lease of
its own. Instead, its commands get the `cargo` shim first on `PATH`, so
each heavy `cargo` command takes a counted `build` lease while it compiles
and gives it back when it ends. A run spends most of its time waiting on a
model, reading, and editing; a lease held for the whole run kept other
agents' builds waiting through all of that, so the lease now covers only
the compiling.

- The run's commands, and the coding agent it runs, get the shim's
  variables: `PATH` with the shims first, `OPENAGENTS_LEASE_BIN`,
  `OPENAGENTS_LEASE_ROOT`, `OPENAGENTS_BUILD_LEASES`, and
  `OPENAGENTS_LEASE_PRIORITY`. `CARGO_TARGET_DIR` stays the run's slot,
  because the shim keeps it (`--keep-target-dir`). A run that found every
  slot taken builds where the shim's `lease build` puts it: in a free slot
  when it can take one, else in its own target directory.
- Under this computer's toolchains, the run's boundary also lets it read the
  shim directory and the `openagents` binary and write the lease root, and
  nothing more. When the lease root lies inside the workspace, the task
  store, or the Git directory, the run gets no shim and builds unleased.
- Every task in the inbox answers a request from the owner or a device the
  owner enrolled, so its builds wait at `owner`, unless the process that
  runs the task sets `OPENAGENTS_LEASE_PRIORITY`.
- The GitHub issue flow's checks run before it pushes, so they hold one
  counted `build` lease at `push` (or `owner` when the flow runs at
  `owner`) while they build, beside their target slot. They wait up to 10
  minutes; checks not admitted in time run without the lease and say so.
- The shims are on only in a process that turned them on: `coder`,
  `microcoder`, the desktop app, and the `openagents` commands that run
  Coder. Elsewhere, such as in a library's tests, a run's builds are
  unleased.

The table is `leases` beside the task store, which for the default store is
`~/.openagents/leases`; `OPENAGENTS_LEASE_ROOT` overrides it.

### The cargo shim

Coder puts a `cargo` shim, and the `screencapture` shim that
[The screen](#the-screen) describes, first on the `PATH` of every agent it delegates
to: Claude Code, Codex, and OpenCode delegations, the ACP agents (Devin,
OpenCode, and Grok Build), and Microcoder's local commands. Studio seats and
the workshop agent run as Coder tasks, so their commands get the shim the
same way. The delegate takes leases without knowing about them, and its
briefing says that heavy `cargo` commands may wait their turn. The
delegate's builds wait at the delegation's priority: Coder passes its own
`OPENAGENTS_LEASE_PRIORITY` on, and the shim's `openagents lease build`
reads it.

- The shim is a POSIX `sh` script that `coder_lease::shim` writes into
  `~/.openagents/bin/lease-shims/cargo` (`OPENAGENTS_LEASE_SHIMS` moves the
  directory). `coder`, `microcoder`, the desktop app, and the `openagents`
  commands that run Coder (`coder`, `host`, `task`, `chat`, `terminal`, and
  `studio`) write it when they start. The directory is never put on your
  own interactive `PATH`.
- It runs `openagents lease build --keep-target-dir -- <real cargo> ARGS`
  for `build`, `test`, `check`, `clippy`, `run`, `bench`, and `nextest`, and
  their short forms, and runs the real `cargo` directly for every other
  subcommand, such as `fmt`, `metadata`, and `tree`.
- It finds the real `cargo` by walking `PATH` and skipping its own
  directory. It finds `openagents` through `OPENAGENTS_LEASE_BIN`, which
  Coder sets to the `openagents` beside it, else on `PATH`, else in
  `~/.openagents/bin`. Without one, it runs `cargo` unleased and says so.
- Under an existing `build` lease (`OPENAGENTS_LEASES` names `build`), it
  passes straight through.
- When the lease table can't be written, such as inside a sandbox that keeps
  the home unwritten, `lease build` from the shim runs the command without
  a lease and says so, rather than failing the build.

## Resources

| Resource | Shape | Capacity | Notes |
| --- | --- | --- | --- |
| `build` | Counted, slots | `clamp(cores / 4, 1, 4)`: 4 on an 18-core Mac | A lease takes one slot unless `--amount` says more, and reserves a disk budget for each. |
| `memory` | Counted, GiB | 75 percent of physical memory | A lease must declare `--amount`. |
| `disk` | Counted, GB | The free space above the floor | A lease must declare `--amount`. |
| `quiet` | Exclusive | One holder | Waits for builds; holds new builds. |
| `screen` | Exclusive | One holder, with an owner grant | Offscreen capture is the default. |
| `browser`, `gpu`, `unreal`, `blender` | Exclusive | One holder | Headless Chrome with its own profile needs no lease. |
| `pylon` | Counted, jobs | 64 | Each shared-compute pool job takes one at `background` priority; the pylon drains while a `quiet` lease or any `owner` priority lease is held or queued ([shared compute](../../compute/pylon.md#share-this-computer-from-the-host)). |
| `artifact/NAME`, `issue/N` | Exclusive | One holder | For [the single-digest artifact queue](artifact-queue.md) and issue claims. |

A counted lease is admitted while the amounts held plus its own fit the
capacity. A `disk` lease is admitted when the free space on the lease
root's volume is at least the floor, plus the budgets other disk and build
leases hold, plus its own budget. A `build` lease also needs its own disk
budget on top of the floor and the budgets held; the broker calls its
reclaim hook once before it refuses one for want of disk. Held budgets count in full, even when part of
them is already written, so the check errs toward waiting.

An amount larger than the capacity is refused at once, because it could
never be admitted.

### Defaults and overrides

| Setting | Variable | Setting key | Default |
| --- | --- | --- | --- |
| Build slots | `OPENAGENTS_BUILD_LEASES` | `coder.build_leases` | `clamp(cores / 4, 1, 4)` |
| Memory budget, GiB | `OPENAGENTS_MEMORY_LEASE_GIB` | None | 75 percent of physical memory |
| Disk floor, GB | `OPENAGENTS_SLOT_FREE_GB` | `coder.slot_free_gb` | 10 |
| Disk budget of a build, GB | `OPENAGENTS_BUILD_DISK_GB` | None | 10 |
| Lease root | `OPENAGENTS_LEASE_ROOT` | None | `~/.openagents/leases` |
| Default priority | `OPENAGENTS_LEASE_PRIORITY` | None | `normal` |
| Aging step, minutes | `OPENAGENTS_LEASE_AGING_MINUTES` | None | 20; `0` turns aging off |

A variable wins over the setting, and the setting wins over the default.
Set a key with `openagents settings set coder.build_leases 3`. The disk
floor is the same one Coder's build slots keep. A saved build count of `1`
keeps builds at one even on a larger machine. Raise it with
`openagents settings set coder.build_leases 4`, or use
`openagents settings unset coder.build_leases` to use the CPU-based default.
Restart existing Coder sessions after changing the count: their commands
can inherit the old count through `OPENAGENTS_BUILD_LEASES`.

## The quiet machine

A soak or a benchmark that measures the machine takes `quiet`:

- `quiet` is admitted only when no `build` lease is held.
- While `quiet` is held or queued, no new `build` lease is admitted. A queued
  `quiet` lease drains builds, so a soak can't starve behind a steady stream
  of short builds.
- The broker never stops, signals, or lowers the priority of a running
  build. Running builds finish on their own, and then the quiet job starts.

The wrapped command sees `OPENAGENTS_LEASES=quiet`, so its own receipt can
record that it ran on a quiet machine. `scripts/grid-soak.sh` writes the
variable into its `meta.json` as `leases`.

### Placement

A job can say what it is instead of which lease it needs: `openagents lease
run --class CLASS -- CMD`, or `openagents lease quiet --class soak -- CMD`.
The `coder.placement` setting then decides whether it runs here under its
lease or on another computer over SSH. By default release gates and
benchmarks go to the first configured computer that answers, else run here
under `quiet`; soaks stay here under `quiet`, because they measure this
machine's own client; and builds stay here, where the warm caches are. No
computer is configured until you set one.
[Placement](placement.md) covers the classes, the setting, remote runs, and
their receipts.

## The screen

The real screen is off limits to agents unless the owner grants it. A
`screen` lease is refused, without waiting, unless a grant admits the
requesting session:

```sh
openagents lease grant screen [--for DURATION] [--to SESSION]
openagents lease revoke screen
```

`grant screen` lasts one hour unless `--for` names a duration such as `30m`,
`2h`, or `1d`, and admits every session unless `--to` names one. It asks the
owner to type `yes` on an interactive terminal. It refuses when standard
input isn't a terminal, when an agent variable such as `CLAUDECODE`,
`CODEX_THREAD_ID`, or `OPENAGENTS_LEASE_ID` is set, and when an ancestor
process is an agent such as `claude` or `codex`. `revoke screen` ends the
grant; a screen lease already held runs until its command ends, and new ones
are refused.

Agents capture offscreen by default, such as with `verse --capture
FILE.png`. Two checks keep them off the real screen without a `screen`
lease (`OPENAGENTS_LEASES` names `screen`):

- A windowed program such as `verse` calls `coder_lease::screen_refusal`
  before it opens a window. In an agent environment, it refuses and points
  to `--capture` and `openagents lease screen -- CMD`. An agent environment
  is one where `OPENAGENTS_SESSION` names an agent's session or an agent
  variable such as `CLAUDECODE` or `CODEX_THREAD_ID` is set.
  `OPENAGENTS_LEASE_ID` alone, and a session named `process:PID`, which a
  lease records when no agent holds it, don't count, so a person running
  `verse` by hand or under a lease is unaffected.
- Coder puts a `screencapture` shim beside the `cargo` shim. It refuses
  without a `screen` lease, and under one it runs the real `screencapture`
  further along `PATH`.

`openagents lease list` shows who holds the screen.

On one Unix account, a grant stops accidents, not a process set on forging
one: such a process can write the grant file itself. A stronger guarantee
needs a separate account or a virtual machine.

## Holders and sessions

Each lease records its holder:

- **Session:** `OPENAGENTS_SESSION`, else the agent's own session variable
  (`CLAUDE_CODE_SESSION_ID` as `claude-code:ID`, `CODEX_THREAD_ID` as
  `codex:ID`), else the nearest ancestor process that is an agent, as
  `NAME:PID`, else `process:PPID`.
- **Agent kind:** such as `claude-code` or `codex`, or `none`.
- **Process:** the process that holds the lease.
- **Command:** the file name of the command's first word. Arguments are
  never recorded, because they can carry secrets.
- **Priority and times:** the priority it asked for, when the lease was
  requested, and when it was admitted.

`openagents lease list` shows every lease held and waiting with these
fields, by resource, the held leases first and then the waiters in the
order they'll be admitted, with each waiter's place, priority, and wait.
`--json` prints the table, the limits, the aging step, and the screen
grant.

## Issue claims

An issue claim must outlive the command that takes it, so it isn't a
holder lock. `openagents issue claim N` and Coder's issue flow write a claim
record, `claims/OWNER/NAME/issue-N.json` under the lease root
([`coder_lease::claims`](../../../crates/coder-lease/src/claims.rs)), with
the session, its agent process and that process's start time, and when it
was claimed. The claim holds while both of these are true:

- It's younger than the claim window: `claim_hours` in
  `.openagents/coder-issues.json`, 6 hours by default, the same window as
  `CLAIM_HOURS` in `scripts/project-sync.sh`.
- A lease in the table names its session, or its agent process still runs
  with the recorded start time.

Another session's claim is refused, naming the holder's session and the
claim's age. The same session claims again and renews it. A claim whose
session ended is taken over. `openagents issue release N` drops the record,
and `--force` on either command overrides another live session.

## The wrapped command's environment

| Variable | Value |
| --- | --- |
| `OPENAGENTS_LEASE_ID` | The lease's identifier. |
| `OPENAGENTS_LEASES` | The resources the command runs under, comma-separated, such as `quiet` or `build,gpu`. |
| `OPENAGENTS_SESSION` | The holder's session, so the command's own leases name the same session. |
| `OPENAGENTS_SCRATCH` | The session's durable scratch directory, created when missing: `~/.openagents/scratch/<session>/`, or `scratch` beside a moved lease root. [Durable scratch](../guides/scratch.md) covers it. |
| `CARGO_TARGET_DIR` | Under `lease build`, the target slot, unless `--keep-target-dir` kept the caller's. |

A command that already runs under a live lease on a resource passes through
a second request for that resource without taking another lease. A wrapped
command that calls `openagents lease build` again, directly or through a
shim, can't wait on itself.

## Receipts

When a lease ends, the broker writes `receipts/<id>.json` under the lease
root (schema `openagents.lease.receipt.v1`):

| Field | Meaning |
| --- | --- |
| `id`, `resource`, `amount`, `holder`, `priority` | The lease. |
| `requested_at_ms`, `acquired_at_ms`, `released_at_ms` | When it was asked for, admitted, and released, in Unix milliseconds. |
| `wait_ms`, `held_ms` | How long it waited in the queue and how long it was held. |
| `held_whole_run` | Whether the table and the holder lock still named this lease when it ended, so the resource stayed held for the whole run. |
| `nested` | Whether it passed through an outer lease of the same resource. A nested lease writes no file under `receipts/`. |
| `exit` | The command's exit code, when it had one. |
| `disk` | For a `build` lease from `lease build`: `slot` and `slot_bytes`, `worktree` and `worktree_bytes`, and `allocated_bytes`, the two together in allocated blocks with each hard link and APFS clone family counted once. |

`--receipt PATH` writes a copy to PATH.

## How the table works

Everything lives under the lease root, private to the user:

- `table.json` holds every lease, held or waiting. It's read and written only
  under an exclusive `flock` on `table.lock`.
- `held/<id>.lock` is each lease's holder lock. The holder keeps it locked
  with `flock` for the whole run, waiting included. A lease whose holder
  lock another process can take is dead, and the next reader drops it. The
  kernel releases the lock when the holder exits or crashes, even on
  `SIGKILL`, so a dead holder or waiter never keeps its place. The lock is
  opened close-on-exec, so the wrapped command doesn't inherit it.
- `receipts/` holds the receipts, and `grants/screen.json` the screen grant.

A table that can't be read is an error, never a reset.

## Tests

`cargo test -p coder-lease` covers exclusive and counted admission, the
disk floor, a build that reclaims before it refuses for want of disk,
receipts' disk use and the per-session totals, slot records and session
liveness, first-in-first-out waiters, priority order with first in, first
out within a priority, aging, a queued `quiet` lease that holds builds at
every priority, a waiter that times out, the quiet rule with a real process
that is never signaled, the screen grant, receipts, and nesting. `tests/dead_holder.rs` kills a holder and a waiter with
`SIGKILL` and shows the lease is free on the next request.
`tests/shim.rs` runs the `cargo` shim with stand-in programs.
`cargo test -p openagents-cli --test lease` runs the command end to end,
including `lease build` in a slot, a build's receipt with its slot and
linked worktree's bytes and `lease du` over it, a second build that waits under a count
of one, waiters lined up by `--priority` and `OPENAGENTS_LEASE_PRIORITY`
in `lease list`, the refusal below the floor, and the shim over the real
command. In `crates/background`, a test shows a slot whose lease ended with its
session is class 1 at once while a live session's, a reused one, and one
with no record are not, and that admission's reclaim deletes only that
slot. In `crates/coder`, the `targets` tests map a task run to `owner`
and the issue flow's checks to `push`, the issue flow's slot test checks
the checks' lease, and an adapter test runs `cargo` through the shim inside
a toolchains boundary.
`cargo test -p coder-delegate --test lease_shims` shows a delegation gets
the shim first on its `PATH`.
Every test uses a temporary lease root; under `cfg(test)` the crate panics
when a root resolves into the real home.
