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
openagents lease RESOURCE [--amount N] [--no-wait] [--receipt PATH] -- CMD [ARGS...]
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

A request that can't be admitted waits, first in, first out per resource,
and prints why it waits and who holds the resource. `--no-wait` fails at
once with exit code `1` instead. Under `--json`, the receipt is printed as
one JSON document after the command's own output. `--json` after `--`
belongs to the command.

## Leased builds

```sh
openagents lease build [--keep-target-dir] [--no-wait] [--receipt PATH] -- CMD [ARGS...]
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
- **The floor.** Below the free-space floor (10 GB, `OPENAGENTS_SLOT_FREE_GB`,
  `coder.slot_free_gb`), the build first reclaims idle caches in the slots.
  If that isn't enough, it refuses, giving the free space and the floor and
  naming the reclaim command, `openagents background run disk`.
- **`--keep-target-dir`** keeps a `CARGO_TARGET_DIR` the caller already set
  and takes no slot, for an agent that keeps its own long-lived target
  directory. Without it, the slot replaces the caller's directory.
- Outside a Git repository, the build takes the lease and no slot. A build
  inside another `build` lease takes neither and keeps the outer lease's
  target directory.

### Coder's own builds

A Coder task run that builds (full access or this computer's toolchains)
holds a target slot for the whole run and, beside it, one counted `build`
lease, so Coder's builds and every other agent's share one build count. The
run waits up to 10 minutes in the broker's queue; a run not admitted in time
runs without the lease and says so, as a run with no free slot builds
outside the slots. The run's processes inherit the lease's variables, so a
nested `openagents lease build` or `cargo` shim passes through. The checks
of the GitHub issue flow do the same. The table is `leases` beside the task
store, which for the default store is `~/.openagents/leases`;
`OPENAGENTS_LEASE_ROOT` overrides it.

### The cargo shim

Coder puts a `cargo` shim first on the `PATH` of every agent it delegates
to: Claude Code, Codex, and OpenCode delegations, the ACP agents (Devin,
OpenCode, and Grok Build), and Microcoder's local commands. Studio seats and
the workshop agent run as Coder tasks, so they get the task's build lease.
The delegate takes leases without knowing about them, and its briefing says
that heavy `cargo` commands may wait their turn.

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
| `build` | Counted, slots | `max(1, cores / 8)`: 2 on an 18-core Mac | A lease takes one slot unless `--amount` says more. |
| `memory` | Counted, GiB | 75 percent of physical memory | A lease must declare `--amount`. |
| `disk` | Counted, GB | The free space above the floor | A lease must declare `--amount`. |
| `quiet` | Exclusive | One holder | Waits for builds; holds new builds. |
| `screen` | Exclusive | One holder, with an owner grant | Offscreen capture is the default. |
| `browser`, `gpu`, `unreal`, `blender` | Exclusive | One holder | Headless Chrome with its own profile needs no lease. |
| `artifact/NAME`, `issue/N` | Exclusive | One holder | For the single-digest artifact queue and issue claims. |

A counted lease is admitted while the amounts held plus its own fit the
capacity. A `disk` lease is admitted when the free space on the lease
root's volume is at least the floor, plus the budgets other disk leases
hold, plus its own budget. Held budgets count in full, even when part of
them is already written, so the check errs toward waiting.

An amount larger than the capacity is refused at once, because it could
never be admitted.

### Defaults and overrides

| Setting | Variable | Setting key | Default |
| --- | --- | --- | --- |
| Build slots | `OPENAGENTS_BUILD_LEASES` | `coder.build_leases` | `max(1, cores / 8)` |
| Memory budget, GiB | `OPENAGENTS_MEMORY_LEASE_GIB` | None | 75 percent of physical memory |
| Disk floor, GB | `OPENAGENTS_SLOT_FREE_GB` | `coder.slot_free_gb` | 10 |
| Lease root | `OPENAGENTS_LEASE_ROOT` | None | `~/.openagents/leases` |

A variable wins over the setting, and the setting wins over the default.
Set a key with `openagents settings set coder.build_leases 3`. The disk
floor is the same one Coder's build slots keep.

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
- **Priority and times:** the priority (`normal` today; the priority queue
  orders by it later), when the lease was requested, and when it was
  admitted.

`openagents lease list` shows every lease held and waiting with these
fields, and `--json` prints the table, the limits, and the screen grant.

## The wrapped command's environment

| Variable | Value |
| --- | --- |
| `OPENAGENTS_LEASE_ID` | The lease's identifier. |
| `OPENAGENTS_LEASES` | The resources the command runs under, comma-separated, such as `quiet` or `build,gpu`. |
| `OPENAGENTS_SESSION` | The holder's session, so the command's own leases name the same session. |
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
disk floor, first-in-first-out waiters, a waiter that times out, the quiet
rule with a real process that is never signaled, the screen grant, receipts,
and nesting. `tests/dead_holder.rs` kills a holder and a waiter with
`SIGKILL` and shows the lease is free on the next request.
`tests/shim.rs` runs the `cargo` shim with stand-in programs.
`cargo test -p openagents-cli --test lease` runs the command end to end,
including `lease build` in a slot, a second build that waits under a count
of one, the refusal below the floor, and the shim over the real command.
`cargo test -p coder-delegate --test lease_shims` shows a delegation gets
the shim first on its `PATH`.
Every test uses a temporary lease root; under `cfg(test)` the crate panics
when a root resolves into the real home.
