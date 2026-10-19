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
  variable, and so do the agents Coder delegates to and the `cargo` shim
  when it is installed.
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
holds a target slot for the whole run and no `build` lease. Its `cargo`
commands run directly: queued builds cost more time than the contention
the queue saved, so the `cargo` shim is off unless the machine opts back
in ([The cargo shim](#the-cargo-shim)). What the run's commands and the
coding agent it runs always get is the `screencapture` shim
([The screen](#the-screen)) first on `PATH`, plus the variables a shim or
a manual `openagents lease` reads: `OPENAGENTS_LEASE_BIN`,
`OPENAGENTS_LEASE_ROOT`, `OPENAGENTS_BUILD_LEASES`, and
`OPENAGENTS_LEASE_PRIORITY`.

- Under this computer's toolchains, the run's boundary also lets it read the
  shim directory and the `openagents` binary and write the lease root, and
  nothing more. When the lease root lies inside the workspace, the task
  store, or the Git directory, the run gets no shims.
- Every task in the inbox answers a request from the owner or a device the
  owner enrolled, so a lease the run takes waits at `owner`, unless the
  process that runs the task sets `OPENAGENTS_LEASE_PRIORITY`.
- The GitHub issue flow's checks run before it pushes and hold a target
  slot while they build, waiting up to 10 minutes for a free slot; checks
  that get none build outside the slots.
- The shims are on only in a process that turned them on: `coder`,
  `microcoder`, the desktop app, and the `openagents` commands that run
  Coder. Elsewhere, such as in a library's tests, a run's builds get no
  shims.

The table is `leases` beside the task store, which for the default store is
`~/.openagents/leases`; `OPENAGENTS_LEASE_ROOT` overrides it.

### The cargo shim

The `cargo` shim is off by default: delegates' `cargo` commands run
directly, and `openagents lease build` and `openagents n --class build`
stay the manual forms. Set `OPENAGENTS_LEASE_CARGO=1` in the environment of
`coder`, `microcoder`, the desktop app, or the `openagents` commands that
run Coder before they start, and their delegates get the shim again while
the machine is under extreme contention. Enabling the shims without it
removes a `cargo` shim an earlier version wrote, so an old install cannot
keep leasing builds.

When it is installed, Coder puts the `cargo` shim first on the `PATH` of
every agent it delegates
to: Claude Code, Codex, and OpenCode delegations, the ACP agents (Devin,
OpenCode, and Grok Build), and Microcoder's local commands. Studio seats and
the workshop agent run as Coder tasks, so their commands get the shim the
same way. The delegate takes leases without knowing about them. The
delegate's builds wait at the delegation's priority: Coder passes its own
`OPENAGENTS_LEASE_PRIORITY` on, and the shim's `openagents lease build`
reads it.

- The shim is a POSIX `sh` script that `coder_lease::shim` writes into
  `~/.openagents/bin/lease-shims/cargo` (`OPENAGENTS_LEASE_SHIMS` moves the
  directory). `coder`, `microcoder`, the desktop app, and the `openagents`
  commands that run Coder (`coder`, `host`, `task`, `chat`, `terminal`, and
  `studio`) write it when they start with `OPENAGENTS_LEASE_CARGO` set. The
  directory is never put on your
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

### Agent chats in the desktop app

The OpenAgents desktop app gives every agent chat it starts a build gate
([`coder_lease::agent_builds`](../../../crates/coder-lease/src/agent_builds.rs),
#11971), so a coordinator such as Foreman can't be overrun by its own chats.
The app writes a `cargo` shim into `~/.openagents/bin/agent-build-shims`
(`OPENAGENTS_AGENT_SHIMS`) and puts that folder first on each agent's `PATH`.
For `build`, `test`, `check`, `clippy`, `run`, `bench`, and `nextest`, the
shim prepares the build before it runs the real `cargo`:

- **Shared target folders.** It builds in one target folder per Cargo
  workspace under `~/work/agent-targets` (`OPENAGENTS_AGENT_TARGETS`). The
  workspace comes from `cargo locate-project --workspace`, honoring
  `--manifest-path`, with `Cargo.lock` discovery for older shims. Every worktree
  of a repository shares the folder for the same workspace, so the root
  workspace and `apps/openagents` each get one. The directory name uses the
  Git common directory plus the workspace's relative path, never the agent's
  ID or the worktree's name. Cargo locks each build directory while compiling.
  An explicit shared `CARGO_TARGET_DIR` stays: `~/work/openagents-target-1`,
  `~/work/openagents-target-1-app`, a folder directly under the gate targets,
  or a path in `agent-target-grants`. Other targets are replaced and the shim
  says so; `OPENAGENTS_KEEP_TARGET_DIR=1` keeps any explicit target. On the
  owner's Mac, use the existing shared targets rather than creating another
  set under `agent-targets` (#12104).
- **Source-path identity.** The shim loads
  [`cargo-build-env.sh`](../../../scripts/dev/cargo-build-env.sh). The helper
  resolves a rustup proxy to its selected compiler, exports its absolute path
  as `RUSTC`, and hashes that path, full `rustc -vV` output, and physical
  workspace manifest path with SHA-256. It sets
  `CARGO_BUILD_BUILD_DIR` to `$CARGO_TARGET_DIR/worktrees/<first two hex digits>/<remaining hex digits>`.
  Fingerprints, dependency libraries, and build-script outputs belong to that
  source path and compiler; kache still restores compiled artifacts by source content.
  This covers workspace members, excluded path dependencies, and nested
  workspaces. Final binaries stay in the shared target's normal `debug` or
  `release` directory. A `.openagents-workspace` file records the manifest
  and its modification time records each build, including all-fresh builds.
  `.openagents-compiler` records the compiler path and version. Old path-only
  state stays unused until the sweep removes it, under the same disk cap.
  Pinning Cargo alone is insufficient: Cargo probes `rustc` in the workspace,
  then runs it in each crate's directory. A rustup proxy can choose a different
  compiler there while Cargo records the probed version in its fingerprints,
  leaving incompatible dependencies `Fresh` (#12113). Reload the helper after
  changing `RUSTC`. To switch rustup toolchains, unset `RUSTC`, set
  `RUSTUP_TOOLCHAIN`, and reload the helper.
- **No incremental state.** Heavy Cargo subcommands set
  `CARGO_INCREMENTAL=0`, overriding an inherited `1`. Both this and source-path
  identity apply with `OPENAGENTS_KEEP_TARGET_DIR=1`, inside a build lease,
  and when `openagents` is unavailable. An inherited build directory is replaced
  so it cannot point at another worktree's compiler state.
- **The disk floor.** It refuses to start when less than 30 GiB is free
  (`OPENAGENTS_AGENT_FLOOR_GIB`), and says how much is free.
- **The build limit.** It runs `openagents lease build --keep-target-dir`
  with `OPENAGENTS_BUILD_LEASES` set to the coordinator's limit: the number
  in `agent-build-limit` in the lease root, else
  `OPENAGENTS_AGENT_BUILD_LIMIT`, else 5. Each build reserves 1 GB above the
  floor (`OPENAGENTS_BUILD_DISK_GB`), because shared folders grow little. A
  build over the limit waits in the `build` queue and prints why.

Every other subcommand runs the real `cargo` directly. A build inside a
`build` lease skips admission after selecting its build directory and
disabling incremental compilation. Without an installed `openagents`, a build
runs without waiting and says so. Start the app with `OPENAGENTS_AGENT_BUILD_GATE=0`
to give its agents no gate.

Issue #12071 reproduced a wrong-code link with kache both enabled and
removed. Cargo uses workspace-relative source paths and modification times
for freshness: when two worktrees' different sources predate a build in one
shared target, the second can be called `Fresh` without running rustc or
kache. A cache-key salt inside kache cannot correct that skipped invocation.
A [separate Cargo build directory](https://doc.rust-lang.org/cargo/reference/config.html#buildbuild-dir)
keys all intermediate state by the source manifest's path before the freshness
check, including excluded path dependencies inside the workspace.

For direct agent builds, choose the shared target and load the same settings,
then run Cargo directly:

```sh
export OPENAGENTS_KEEP_TARGET_DIR=1
export CARGO_TARGET_DIR="$HOME/work/openagents-target-1"
. "$(git rev-parse --show-toplevel)/scripts/dev/cargo-build-env.sh"
cargo test -p coder-lease
```

For `apps/openagents`, use `openagents-target-1-app` instead and load the helper
from that directory. Keep one shared target per workspace; a source hash
belongs to its manifest path and compiler, never an agent or session.

The offline regression uses real Cargo and kache, two Git worktrees with
different same-version library sources older than every artifact, alternating
root and app builds, and one target per workspace. It checks workspace members,
an added module whose entry file also predates the build, an excluded
dependency, an external domain dependency, `--manifest-path`, an
explicit target, a nested lease, unchanged dependency bytes, and empty
incremental state. Its worktrees, targets, home, and kache store live under
durable scratch and are removed when it finishes:

```sh
OPENAGENTS_SCRATCH="$(openagents scratch)" \
OPENAGENTS_TEST_KACHE="$(command -v kache)" \
cargo test -p coder-lease --test shared_target -- --ignored
```

The compiler regression reproduces E0514 without a cache wrapper, including
the `Fresh` retry, then checks the helper with two installed toolchains and
separate cache keys and hits in a disposable kache store:

```sh
OPENAGENTS_SCRATCH="$(openagents scratch)" \
OPENAGENTS_TEST_RUSTUP="$(command -v rustup)" \
OPENAGENTS_TEST_KACHE="$(command -v kache)" \
OPENAGENTS_TEST_OLD_TOOLCHAIN=stable \
OPENAGENTS_TEST_NEW_TOOLCHAIN=1.98.1 \
cargo test -p coder-lease --test compiler_state -- --ignored
```

The app's MCP server offers `build_status`: the builds running and waiting,
the limit, the free disk, and the floor. Called with `limit`, it sets the
limit for builds that start from then on.

### The build sweep

Shared target folders still grow: every worktree and feature set compiles
new artifacts into them, and on 2026-10-17 two of them reached 328 and 208
GiB in four hours while free disk fell to 32 GiB. The build sweep
([`coder_lease::agent_builds::sweep`](../../../crates/coder-lease/src/agent_builds/sweep.rs),
#12023) keeps this Mac's floor of 100 GiB free with no agent session
running. launchd runs `~/.openagents/bin/openagents-build-sweep` every 5
minutes (`com.openagents.build-sweep`). Each run, in order:

1. **Worktree state.** Compiler artifacts under `worktrees/<hash>` go when
   the recorded manifest no longer exists, or when the state has been unused
   for 6 hours (`OPENAGENTS_BUILD_SWEEP_WORKTREE_IDLE_HOURS`), or 30 minutes
   when free disk is below the floor. The sweep skips
   state held by a process or Cargo lock, and skips this cleanup when it can't
   read processes. It takes every affected profile's Cargo locks and refreshes
   the process and last-use checks before removal. Empty directories, source
   markers, and lock files stay so a waiting Cargo keeps the same lock inode.
   The states all count toward their parent target's cap; they have no separate
   cap or disk allowance.
2. **Copies.** A target folder in `~/work` other than
   `openagents-target-1` (root crates), `openagents-target-1-app` (app
   crates), and the owner's `openagents-target-app` is a copy. The sweep
   flags it in the log, and removes it once no build holds it and nothing
   in it changed for an hour. A path in `agent-target-grants` in the lease
   root (one a line) is shared, not a copy. The gate's own folders under
   `~/work/agent-targets` are shared.
3. **The floor.** Below 100 GiB free, it removes whole target folders no
   build holds, copies first and then the least recently used, until 100 GiB
   is free again.
4. **The cap.** A target over 120 GiB loses its least recently used
   artifacts in `deps` (a crate's files with its fingerprint), `build`, and
   `incremental` that nothing read or wrote for 2 hours, down to 90 GiB.
   Below the disk floor, this pass also uses the 30-minute idle window.
5. **Still below the floor**, it does the same across every target for
   artifacts untouched 30 minutes, until 100 GiB is free.

A `cargo`, `rustc`, `kache`, `clippy-driver`, `rustdoc`, or build-script
process holds a target when its `CARGO_TARGET_DIR` (from `ps -E`), its
working directory (`lsof -a -d cwd -p PIDS`), or an argument lies inside
it, and so does a build holding one of a profile's cargo locks. If the
process table can't be read, every target counts as held. The sweep removes
artifacts only while it holds the profile's Cargo locks itself. It visits unlocked targets and profiles
first. Each target shares a 20-second total lock-wait budget across all its
profiles, then busy profiles wait for the next scheduled pass. No background
lock waiter survives the pass, and each pass logs a summary. After obtaining
a lock, the sweep refreshes file ages before removal.

When every Cargo process on a target uses `worktrees/<hash>` as its build
directory, the target's old top-level `deps`, `build`, `.fingerprint`, and
`incremental` entries are eligible after the same idle window. The sweep
refreshes that process check while holding the intermediate `.cargo-build-lock`;
a held final-output lock does not block this cleanup. Final executables in
`debug` or `release` stay. Any Cargo using the old layout, or an unreadable
process table, requires all profile locks. Source-keyed states always require
all their own locks and stay while a process uses them (#12106).

Below the floor, the sweep also runs `kache gc` before measuring targets. It
uses `~/.cargo/bin/kache` even under launchd's limited `PATH`, bounds the
command to 30 seconds, and records success or failure. Dry runs never run GC.

macOS records a read in a file's access time only when that time is older
than the file's change time, so a fresh artifact never shows a read. After
each run the sweep sets each artifact's access time a second before its
change time, which makes the next read show, and keeps every artifact's last
use in `~/.openagents/build-sweep.state.json`. It counts nothing as untouched
from before it began watching, so a fresh install removes no artifact for 2
hours.

Never touched: `~/work/openagents-target-app` (the owner's dev app) unless
less than 40 GiB is free, agent scratch and research artifacts under
`~/.openagents/scratch`, the paths in `~/.openagents/disk-janitor.keep`, and
anything reached through a link.

Every removal, flag, and skip is a line in
`~/.openagents/logs/build-sweep.log` with the Chicago time, the size, the
path, and the reason, after a line for the run with the free space and each
target's size and holder. `build-sweep.last.json` beside it is the last run
as JSON.

| Setting | Default | Variable |
| --- | --- | --- |
| Per-target cap, including worktree state | 120 GiB | `OPENAGENTS_BUILD_SWEEP_CAP_GIB` |
| Unused worktree state | 6 hours | `OPENAGENTS_BUILD_SWEEP_WORKTREE_IDLE_HOURS` |
| Free-disk floor | 100 GiB | `OPENAGENTS_BUILD_SWEEP_FLOOR_GIB` |
| Critical level (the dev app may go) | 40 GiB | `OPENAGENTS_BUILD_SWEEP_CRITICAL_GIB` |
| Untouched before a capped target loses it | 120 minutes | `OPENAGENTS_BUILD_SWEEP_IDLE_MINUTES` |

- Install or update: `scripts/dev/install-build-sweep.sh` builds the release
  binary in the shared target, outside the busy debug-profile queue.
- See what a run would remove, removing nothing:
  `~/.openagents/bin/openagents-build-sweep --dry-run`.
- Pause: `touch ~/.openagents/build-sweep.off`; resume: `rm` it.
- Turn it off for good: `scripts/dev/install-build-sweep.sh --uninstall`.

The disk janitor (`scripts/dev/disk-janitor.sh`, every 10 minutes) still
cleans worktrees, scratch, and old caches; the sweep is the part that keeps
up with builds.

### Machine actions

The sweep runs on its own schedule. When an agent needs space sooner, or
needs to stop a hung process, it uses a **machine action**
([`coder_lease::machine`](../../../crates/coder-lease/src/machine.rs),
#12033). On 2026-10-17 a coordinating agent deleted about 640 GiB of build
caches and stopped a production deploy script on its own judgment, with no
scope and no record. Machine actions are scoped and logged instead:

| Scope | Allows |
| --- | --- |
| `build.prune` | Removing a build cache: a target folder the sweep knows (in `~/work` or `~/work/agent-targets`), or a folder inside one |
| `process.stop` | Sending `SIGTERM` to a process |
| `host.upgrade` | Upgrading the host app from `main` (#12056) |

Since 2026-10-18 (#12135) no scope is required: agents prune, stop and
upgrade without a grant, and the log and the safety refusals still apply. The
records below stay in case grants come back.

The owner grants scopes to an **agent ID**: the chat's ID for an app chat,
or its session (`claude-code:ID`, `codex:ID`) for a terminal agent. A grant
never goes by a role name such as "Foreman": a fork of an agent holds
nothing until the owner grants its own ID.

```sh
openagents lease grant machine CHAT_ID --scope prune,stop [--for 8h]
openagents lease revoke machine CHAT_ID
openagents lease actions [--limit 50]
```

`grant machine` asks the owner to type `yes` and refuses in an agent
environment, like `grant screen`. Without `--for` it lasts until revoked.
Granting an agent again replaces its earlier scopes. Grants live in
`grants/machine.json` in the lease root.

An app chat acts through its `machine_action` or `upgrade_host` tool,
which passes its own chat ID: the tools have no argument for naming another
agent. From its own shell, a chat can also run `openagents lease prune PATH
--reason TEXT`, `openagents lease stop PID --reason TEXT`, or `openagents
host-upgrade --agent CHAT_ID`. Each action needs a reason. No grant is needed
(#12135); the owner's `openagents lease grant machine` records are kept but
no longer gate anything.

**Proof of the chat (#12067).** A chat ID is no secret, so naming one
proves nothing. The engine keeps a key in `grants/machine.key` in the lease
root (mode 600) and gives each chat its proof, a keyed hash of its chat ID,
in `OPENAGENTS_CHAT_PROOF` beside `OPENAGENTS_CHAT_ID`: in the environment
of the chat's `openagents` MCP server, and in the environment its shell
commands get (Claude Code's process; Codex's `shell_environment_policy`).
Every machine-action path
([`machine::proof`](../../../crates/coder-lease/src/machine/proof.rs))
accepts a chat ID only with that chat's proof:

- A command that names a chat (`--agent`, or `OPENAGENTS_CHAT_ID`) with no
  proof, or with another chat's, is refused and logged, and the refusal
  says the proof is missing or belongs to another chat. Copying a granted
  chat's ID into another process, or into another chat, isn't enough.
- An agent that names no chat acts as the session the broker detects from
  its own process ancestry (`claude-code:ID`, `codex:ID`).
- The owner at an interactive terminal needs no proof and acts as the
  owner, logged as `owner`, even when naming a chat. `host-upgrade
  --detach` hands its detached copy a proof bound to that one upgrade.

A chat started before this change gets its proof on its next turn, when
the engine starts its harness and MCP server again. Harnesses other than
Claude Code and Codex get the proof only in their MCP server, so they use
the tools.

With the grant, these are still refused:

- **Prune:** a path that isn't a build cache (worktrees and checkouts
  included), the owner's dev-app target (left to the sweep below 40 GiB),
  agent scratch and kept paths, links, a relative path or one with `..`, a
  target a build holds (process or cargo lock, as the sweep reads them),
  and every target when the process table can't be read.
- **Stop:** init, the caller and its parents, another user's process, and a
  process running a deploy (a word of its command line starts with
  `deploy`, such as `scripts/deploy-production.sh` or `wrangler deploy`).
  Deploys stay with the owner.

Every action and every refusal is a line in `machine-actions.jsonl` in the
lease root. Each line records the agent ID, the scope, the outcome (`done`,
`refused`, or `failed`), the reason given, the path and bytes or the pid and
command name (never its arguments), the detail, and the Chicago time.
`openagents lease actions` prints the grants and recent lines, and the
`machine_action` tool's `log` and `grants` actions return the same record
to the agent. On one Unix account, as with the screen grant, this stops
accidents and borrowed IDs, not a process set on reading the key file to
forge a proof.

**Delegated grants (#12163).** The owner grants one coordinator `delegate`,
once, on any computer:

```sh
openagents lease grant machine CHAT_ID --scope delegate
```

The grant is written where the command runs and sent to the computer the
coordinator's chat runs on. That chat can then grant `build.prune`,
`process.stop` and `host.upgrade` to other chats with the `grant_machine` and
`revoke_machine` tools, or from its shell with `openagents lease subgrant CHAT
--scope prune --reason TEXT [--for 2h]` and `openagents lease subrevoke CHAT
--reason TEXT`. The coordinator's computer checks its proof and its
`delegate` grant. The subgrant goes through the device channel
(`GrantMachine`/`RevokeMachine` with `targetDeviceId`) to the target chat's
own computer, which writes it with `issuedBy` and the reason. Both computers
log it (scope `delegate`, the coordinator as agent, the chat as target).
`delegate` itself isn't delegable, and money, keys and secrets are never
machine scopes. The prune and stop refusals above, including deploys and kept
paths, still apply. A computer whose app predates these methods is named with
its version and the subgrant fails; upgrade that computer first. `openagents
lease revoke machine CHAT_ID` ends the coordinator's grant and every grant it
issued, here and on every other computer that answers, and lists any it
couldn't reach.

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

Each working chat holds one issue. To work a small batch, name all its issues
with `openagents issue claim N --batch N,M[,P]` (at most three); later claims
must belong to that batch. `--force` does not expand it. App claims use
`OPENAGENTS_CHAT_ID` when present, so chats do not share an inherited harness
session's limit. Closing an issue frees its slot at the next claim. Coordinators
leave claims to the chats doing the work; their planning uses recent chat
progress and reports claims with no live chat separately.

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
that is never signaled, the screen grant, receipts, nesting, and machine
actions (grants keyed by agent ID, refusals outside them and around builds,
dev-app, scratch, deploys, and the caller, and the log, all on fixtures that
never signal a real process). `tests/dead_holder.rs` kills a holder and a waiter with
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

## Machine facts in app check-ins

The read-only `machine_status {"agent_id":"ID"}` app tool reads the machine's
free disk against a 100 GiB floor, exact Cargo process count, build slots and
cap, sweep report and recent evictions, provider capacity, machine grants and
actions for that ID, landing queue, GCE pool, and timed `GET` health checks.
Without `agent_id`, it uses the calling chat's ID. Errors are reported per
source; an unavailable reading is not zero. This tool grants no scope and
performs no machine action.

A check-in host calls `openagents_mcp::machine_status::collect(agent_id)` on
its five-minute tick (`REFRESH_EVERY`), caches the result by agent definition
ID, and appends `Status::prompt_block()` to each prompt. Keep a single
`coder_lease::machine_status::Alerts` in the host and pass each new result's
`machine` to `observe`. Each returned `DiskLow` or `BuildsOverCap` becomes a
wake event for that agent. A continuing shortage raises no second event;
recovery followed by another shortage raises a new one. An unavailable
reading preserves the previous alert state. Automatic prompt and wake wiring
belongs in the check-in host; the tool and these APIs do not change it.


`owner_wait` in the same block lists `NEEDS_OWNER.md` headings and pending
owner or device/store steps in the After-close table. Agent-run checks alone
are excluded. Start owner-only After-close steps with `Owner:`; device and
store steps are also recognized by their names. Each entry has its source path and line, the Git commit time
of that heading or row, and `age_seconds`. An uncommitted or unreadable line
has no known age; changing another entry does not reset it. Missing files in an available checkout
mean no entries; an unavailable checkout or a read failure stays in `errors`.

The deploy fact uses the host's explicit `CLOUDSDK_CONFIG`, the pool's pinned
configuration, or its automation configuration. It never falls back to a
person's default login. The only IAM request is
[`testIamPermissions`](https://docs.cloud.google.com/iam/docs/reference/rest/v1/projects.serviceAccounts/testIamPermissions)
for `iam.serviceAccounts.actAs`: a read-only POST, with a 10-second deadline,
no redirects, and no token in the facts. A successful refusal adds
"production deploy: owner only". A credential, network, or response error
leaves permission unknown. This one permission does not certify every other
permission a deploy might need. The duration of an existing permission block
is unknown; no first-seen file is created.

Any agent can select `repository`, `after_close`, `deploy_project`, and
`deploy_service_account` in the tool call. Defaults use the current checkout
(or `OPENAGENTS_FACTS_REPOSITORY`), this repository's After-close document, and
OpenAgents' production runtime account. The deploy defaults can also be set
with `OPENAGENTS_DEPLOY_PROJECT` and `OPENAGENTS_DEPLOY_SERVICE_ACCOUNT`.
Hosts use `collect_with(agent_id, OwnerOptions)` for these same inputs and
append `Status::prompt_block()` on every check-in. Keep a `NewEntries` value
in the host, call `observe(agent_id, &status.owner_wait)`, and send its new
entries through the granted owner-notification primitive (#12040). Failed
sources preserve notification state. Neither collection nor observation sends
a notification or grants permission to deploy.
