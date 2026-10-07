# Many agents on one machine

Status: reflection and proposal, 2026-10-06. Nothing here is built unless it
cites a path. The evidence comes from the owner's Mac (18-core Apple silicon,
128 GB of memory, a 1.8 TiB disk) on 2026-10-04 and 2026-10-05, while 8 to 14
Claude Code subagents and one Codex agent worked on this repository at once.

## The prompt

The owner shared a thread and asked for reflections, because "we bump into
this problem constantly." The thread says that dozens of agents on one
machine create resource contention, computer-use contention, and browser
contention that nobody has solved, and that the author built a custom setup
for it. The replies propose two answers:

- **Isolation.** A lightweight microVM per agent, so each acts like a cloud
  machine and nobody fights over the browser.
- **A queue.** "All you need is some sort of queue for resources," like a
  GitHub merge queue, or decent timesharing, because agents mostly wait on
  model responses.

A related post asks for GPU orchestration for local model owners: NVMe temp
space, core availability, storage and memory allocation, and running safely
without damaging expensive hardware.

Both answers are right about different things. The rest of this page
separates the contention into kinds, says which answer fits each, maps what
this repository already has, and proposes what to add.

## What contended, by kind

### Compute

Load averages reached 100 to 176 on 18 cores, 5 to 10 runnable threads per
core. Builds slowed: `docs/verification.md` records a `cargo test -p verse
--lib` build at 581 s in a shared slot against 280 s in a fresh one, with the
load between 35 and 80. Timing tests flaked, for example
`a_flood_of_output_is_applied_within_the_frame_budget`. Nothing scheduled the
builds; every agent ran `cargo` when it wanted to.

### Memory

Memory never became the failure on this Mac, but it is the next limit.
Fourteen concurrent `rustc` trees at a few gigabytes each fit in 128 GB only
because most agents were waiting on a model at any moment.

### Disk and I/O

Free space fell to 26 GB once and to 10 GB another time.

- Per-agent Cargo target directories grew to 35 to 150 GB each. One slot
  reached 212 GB when several agents shared it, because checkouts at
  different commits rebuild each other's crates in one directory.
- Worktrees ran about 2.8 GB each, and dozens accumulated.
- The kache compile cache grew to 320 GiB against its 30 GiB cap. Its
  garbage collection fell behind or stuck ("Another GC is already running").
  Only 99 GiB was private; 219 GiB was APFS clones shared with target
  directories. Deleting the cache frees far less than its size suggests,
  and deleting a target directory frees less than `du` says.
- Cleanup was manual and reactive: delete finished slots' target
  directories, check worktrees for unmerged commits, then remove the clean
  ones.

### GPU

One GPU served offscreen Verse captures, the owner's game window, and soaks.
Nothing accounted for it.

### Exclusive interactive devices

These are resources that exactly one agent can use at a time, whatever the
CPU count:

- **The desktop session.** Agents opened Verse windows on the owner's screen
  and took the screen with `screencapture`.
- **The browser.** Headless Chrome ran on a fixed debugging port (9333) with
  one profile directory, so two agents couldn't verify in a browser at once.
  That setup lives in agents' habits, not in this repository.
- **Editors and tools.** One Unreal editor and one Blender binary.

### Shared mutable state

- **The stash stack.** Git shares it across worktrees. A helper dropped
  another session's stash entry and restored it at a different position.
- **Pinned artifacts.** The Everglade asset pack has one digest, so every
  asset change conflicts with every other. The working rule became "rebase
  right before pushing and repin once", a manual serialization point.
- **Issues.** Agents duplicated work on the same issues. Claims were GitHub
  comments, and every agent used the same account, so an assignee says
  nothing about which agent holds a claim.

### External quotas

The Claude weekly usage limit stopped all 8 Claude agents at once, mid-task.
They resumed hours later. Nothing knew the limit was shared and near.

### Credentials and home

The rules say tests never touch the real home or Keychain: a scratch `HOME`,
`--state`, `--root`, and `--tasks`. The first live run of the workshop agent
still installed a Rust toolchain into the real `~/.rustup`. A rule that a test
enforces doesn't cover a live run.

### Durability

The Mac rebooted mid-run and cleared `/private/tmp`, which held captures,
scripts, and agent scratch. Evidence that lived there is gone.

### Measurement integrity

The Codex agent ran a 20-player battle soak (#10559) that measures movement
acknowledgment delay and memory growth. It ran while our builds held the CPU,
so its numbers are possibly contaminated. A store-lock test flaked because
child processes inherited a lock (#10751), a test that measured the machine
instead of the code. The fix was to tell agents to check `pgrep -fl soak`
and run one heavy build at a time: an honor-system lock.

## Isolation, queues, and which fits where

Isolation and scheduling solve different problems.

**Isolation solves conflict.** A microVM, container, or sandbox gives each
agent its own home, browser profile, display, ports, stash, and `/tmp`. Two
agents can no longer corrupt each other's state or fight over a device that
can be multiplied. It does nothing for scarcity: eight VMs on 18 cores still
share 18 cores, the disk is still 1.8 TiB, the GPU is still one GPU, and the
weekly model limit is still one limit. Isolation can make scarcity worse,
because each VM duplicates a toolchain, a target directory, and a cache that
the host could have shared.

**A queue solves scarcity.** Leases on counted and exclusive resources stop
oversubscription and make "one heavy build at a time" a fact instead of a
request. A queue needs each job to declare what it needs before it runs, and
it can't help with state that two jobs both assume they own, such as the
stash or `~/.rustup`.

**The timesharing reply is the important observation.** An agent spends most
of its wall time waiting on a model. Its expensive local phases are bursts:
a build, a test run, a capture, a soak. Those bursts are short, recognizable
commands, so a scheduler only has to gate the bursts, not the agents. Eight
agents can think concurrently and still build two at a time.

**The merge-queue analogy fits shared artifacts exactly.** A merge queue
serializes changes to one shared thing and rebases each onto the last.
"Rebase right before pushing and repin once" for the Everglade pack is a
merge queue run by hand. Any single-digest artifact, such as a pack, a
generated fixture, or a lockfile, needs the same treatment.

The answer is both, layered:

1. Isolate everything that can be multiplied cheaply: home, scratch, browser
   profile, port, display, and stash.
2. Lease everything that can't: CPU bursts, disk budget, GPU, the real screen,
   licensed editors, the quiet machine, and model quota.
3. Serialize writes to single-digest artifacts through a queue.
4. Send work elsewhere when the local machine is full.

## What we have, mapped to each kind

"Enforced" means code refuses or waits. "Honor system" means a rule in
`AGENTS.md` or a message that agents follow when they remember.

| Kind | What exists | Status |
| --- | --- | --- |
| Compute | `crates/supervise` runs every subprocess in its own process group with deadlines and output caps. `coder::task::autostart` (`docs/coder/runtime/host-autostart.md`) bounds how many auto-started tasks run at once (`max_running`). | Enforced for Coder tasks. Nothing bounds concurrent builds, and Claude Code subagents don't run through either. |
| Memory | `crates/supervise/src/memory.rs`: a cgroup `MemoryMax` scope on Linux; on macOS a sampler that kills the process group past the cap. Auto-start passes `--memory-mib` (default 4096). | Enforced per Coder command. No machine-wide budget. |
| Disk | `crates/coder/src/task/targets.rs` leases one of four build slots per repository (`background::SLOTS`), caps a slot at 25 GB, and refuses to build below 10 GB free after reclaiming idle caches. `crates/background` is the disk cleanup monitor (`docs/background/2026-10-02-background-processes.md`), shipped as the Disk cleanup plugin, which is turned on on the owner's Mac. `crates/coder/src/task/owner.rs` waits out a full disk for `STORAGE_FULL_WAIT` while keeping evidence. | Enforced for Coder tasks. The agent slots under `~/work/openagents-target-agentN` are an `AGENTS.md` convention with no lease or cap. See the gaps below. |
| GPU | The eval runner runs on `coderos-4080` with a per-trainer daily quota (`docs/deployment/eval-runner.md`). | Quota on runs, not on the device. No local GPU lease. |
| Exclusive devices | The NIP-TERM typist role (`nips/openagents/NIP-TERM.md`, `term-typist` in `crates/coder-pty/src/ext.rs`): one attachment types into a terminal at a time, with watch and drive shares (`crates/coder-pty/src/share.rs`). On CoderOS, the Coder compositor and the desk protocol (`crates/coder-compositor`, `crates/coder-desk`) own windows on a dedicated machine. | The typist role is a real resource lock, for terminals only. The screen, browser, Unreal, and Blender have none. |
| Shared state | Worktrees per agent. Issue claims (`openagents issue claim`, `crates/openagents-cli/src/issue.rs`) and the project board (`scripts/project-status.sh`). | Worktrees isolate files, not the stash. A claim is a comment marker under one shared account, so it's advisory. Pack repins are honor system. |
| External quotas | The provider capacity book (`crates/microcoder-loop/src/capacity.rs`, `capacity.json`) records each usage or rate-limit refusal until its reset, and failover moves a turn to the next provider. `tenancy::quota` is a durable, retry-safe reservation ledger with crash recovery. The gateway bounds each door's declared capacity (`crates/gateway`). | Enforced for Coder and the serving half. Claude Code subagents don't read or write the book. |
| Credentials and home | `crates/coder-boundary` wraps a command in a write boundary (`sandbox-exec` on macOS, `bwrap` on Linux) and snapshots what it left. The scratch-host rules (temporary `HOME`, `--state`, `--root`, `--tasks`). `coder_service::adopt::Paths::under` panics under `cfg(test)` when a test reaches the real home (`crates/coder-service/src/adopt.rs`). | Enforced in tests and in bounded Coder commands. A live agent run outside a boundary isn't covered, which is how `~/.rustup` changed. |
| Durability | Coder task evidence lives in the task store, outside `/tmp`. | Agent scratch uses `/private/tmp` by default. |
| Measurement integrity | Nothing. | Honor system (`pgrep -fl soak`). |
| Elsewhere | `crates/coder-ssh` installs and reaches remote hosts. NIP-HOST grants and delegation (`crates/coder-access`) and NIP-REACH placement (`crates/coder-reach`) say which computer may run what. Boat sandboxes per task and the GCE pool (`docs/cloud/README.md`), with holds, metering, and a price book in `crates/retail-cloud` and `docs/cloud/retail-prices.md`. | Real, and underused for builds and soaks. |

### The central observation

Most of the scheduling pieces exist, but they serve Coder's own tasks, and
the agents that write this repository don't run as Coder tasks. Coder tasks
lease build slots with a cap and a free-space floor; Claude Code subagents
pick `~/work/openagents-target-agentN` by convention. Coder reads the capacity
book; the subagents found the weekly limit by hitting it. The typist role is a
lease; the screen and the browser have no equivalent.

The disk monitor shows the same gap from the other side. It was on, and the
disk still fell to 10 GB, for reasons its own policy explains:

- An agent target directory qualifies only when untouched for three days. A
  150 GB slot that finished an hour ago is not stale by that rule.
- Its classes don't include Claude Code worktrees, which live under the
  checkout at `.claude/worktrees/`, or the kache store.
- Its start level on this disk is 90 GB free, so it acts only once a burst is
  already underway, and a burst of several fresh slots outruns a five-minute
  check.

This page doesn't establish whether the host's runner was live during each
episode; check its audit log before changing the policy.

## What to add

The natural owner is the Coder host (`crates/coder-host`). It already holds
grants, tasks, terminals, the typist role, the capacity book, build-slot
leases, and the disk monitor. A resource broker there is a generalization of
what it does, not a new service. Estimates are agent-hours at this
repository's recent pace and include tests.

### 1. Make our own agents first-class tenants (about 3 agent-hours)

Give a conversation agent the same leases a Coder task gets: a command,
for example `openagents lease build -- cargo test -p verse`, that takes a
slot from `targets.rs`, sets `CARGO_TARGET_DIR`, runs the command under
`supervise`, and releases the slot on exit. Add it to `AGENTS.md` in place of
the `openagents-target-agentN` convention. This alone puts subagent builds
under the 25 GB cap and the free-space floor.

### 2. A local resource broker (about 8 agent-hours)

A host-owned lease table, durable in the host root, with two resource
shapes:

- **Exclusive leases** on named resources: `browser`, `screen`, `gpu`,
  `unreal`, `blender`, and `quiet`. A lease names its holder, its task or
  agent, and an expiry, and renews while its process group lives, so a
  crashed agent can't hold a lease forever.
- **Counted leases** on capacities: build slots sized from the core count
  (for example, cores divided by 6, which is 3 on this Mac), a disk budget
  per holder, and a memory budget.

The `quiet` lease is the measurement-integrity fix. A soak takes it; while it
is held, new build leases wait, and running builds get `SIGSTOP` or a lower
priority (`taskpolicy -b` on macOS, `cpu.weight` on Linux). The soak's receipt
records that it ran under `quiet`, so a reader knows whether to trust its
latency numbers.

Expose the table through `openagents lease` and NIP-HOST, so a phone can see
who holds the screen. The typist role stays as it is; the broker is the same
idea for devices that aren't terminals.

### 3. A build queue with priorities (about 3 agent-hours, after 2)

Counted build leases wait in a queue instead of failing. Priority comes from
the request: a check before a push outranks a speculative build, and a build
that blocks an owner request outranks both. Waiting costs little, because the
agent is usually waiting on a model anyway.

### 4. Disk accounting and reclaim on pressure (about 4 agent-hours)

- Account by holder, not by directory: each lease records the bytes its
  target directory and worktree hold, measured as allocated blocks, with
  APFS clones counted once.
- Reclaim at lease end, not after three days: when a build lease ends and its
  holder's task or conversation has ended, its slot becomes a class 1
  candidate at once.
- Add the missing classes to the disk monitor: `.claude/worktrees/*` (clean,
  pushed, and no live process inside, using the class 3 rules) and the kache
  store (through kache's own collector, never by deleting files under it).
- Make reclaim proactive: before granting a counted build lease, require the
  free space the lease's budget needs, reclaiming first, as `targets.rs`
  already does for its floor.

### 5. Per-agent browser profiles and ports (about 1 agent-hour)

A verification helper that starts headless Chrome with a fresh
`--user-data-dir` under the agent's scratch and `--remote-debugging-port=0`,
reads the chosen port from `DevToolsActivePort`, and removes the profile on
exit. Two agents then verify at once. This needs no broker.

### 6. Per-agent virtual displays (about 6 agent-hours)

Default every capture to offscreen; `verse --capture` already renders
without a window. For work that needs a real window, give each agent a
display of its own: on CoderOS, a nested Coder compositor seat per agent
(`crates/coder-compositor` already has a nested backend); on macOS, a virtual
display, which needs the private `CGVirtualDisplay` API or a VM. The real
screen becomes the `screen` exclusive lease, and an agent takes it only when
the owner is meant to watch.

### 7. A serialized queue for single-digest artifacts (about 4 agent-hours)

A host-side queue for the Everglade pack and anything like it: an agent
submits an asset change, and the queue applies changes one at a time onto
current `origin/main`, repins once per batch, runs the pack's check, and
pushes. This replaces "rebase right before pushing and repin once" with the
merge-queue behavior the thread describes.

### 8. Claims and leases enforced by the host (about 3 agent-hours)

Keep GitHub comments as the public record, but make the host the authority:
an issue claim is a lease in the broker table, held by a named agent session
rather than the shared account, and `openagents issue claim` refuses a claim
another live session holds. Expiry frees claims of crashed sessions.

### 9. Model-quota awareness across all agents (about 3 agent-hours)

Have every agent read and write the capacity book, not only Coder. Before a
new subagent starts, the orchestrator checks the book; a usage-limit refusal
writes the reset time; and agents that hit the limit record a resume point,
so a restart after the reset continues the task instead of re-deriving it.
Usage that the provider reports as it approaches a limit can lower the
number of new agents started before the wall.

### 10. Durable scratch (under 1 agent-hour)

Point agent scratch at a per-session directory under `~/.openagents/scratch/`
instead of `/private/tmp`, and have the disk monitor age it out after the
session ends. Evidence a check needs belongs in the task store or the
repository, not in scratch.

### 11. Lightweight isolation, and when to leave the machine

- **macOS.** Apple's Virtualization framework runs Linux guests with fast
  startup and shared directories (VirtioFS), and Apple's container tooling
  builds on it. A guest is the right home for untrusted live runs, such as
  the workshop agent, where a write boundary isn't enough because the run
  installs toolchains. It's the wrong home for builds: each guest needs its
  own toolchain, target directory, and cache, which multiplies the disk
  problem.
- **Linux.** On CoderOS, use cgroups v2 for CPU, memory, and I/O weights per
  agent; `supervise` already creates scopes for memory. `bwrap` already
  isolates the filesystem. Firecracker or Cloud Hypervisor microVMs fit the
  untrusted-run case, with the same caveat about duplicated caches.
- **Leave the machine** when a job is long, heavy, and not interactive:
  release-gate builds, soaks, and Terminal-Bench runs. `coderos-4080` (28
  cores, `bwrap`), Boat sandboxes, and the GCE pool already exist, and a soak
  on a machine nobody builds on needs no `quiet` lease. The broker can make
  this a placement decision: when the counted lease would wait longer than a
  remote run takes, offer the remote host.

## The GPU orchestration ask

The related post maps onto the broker with GPU-specific resources:

- **GPU leases** with VRAM accounting: a job declares its peak VRAM, and the
  broker admits jobs whose sum fits, as the counted leases do for memory.
- **Thermal and power limits** as admission inputs: on `coderos-4080`, read
  temperature and power draw (`nvidia-smi`), and stop admitting new GPU jobs
  above a set limit instead of letting the driver throttle them mid-run.
- **NVMe temp budgets** as a per-lease disk budget on the scratch volume,
  reclaimed at lease end.
- **Core and memory allocation** through cgroups v2 on the Linux host.

Is there a product here? Possibly. The pieces that make a broker more than a
script are identity, grants, durable state, and a remote view, and the Coder
host already has all four: NIP-HOST grants, the task store, NIP-TERM, and a
phone that can see the host. A broker that a person runs on their own
machines, and that their agents, Coder runs, and soaks all respect, is a
natural extension of "the host owns the computer." The honest test is
whether it fixes our own Mac first.

## What to change tomorrow

The cheapest fixes, each under about two agent-hours:

1. **A reclaim script** that lists finished subagent target directories and
   clean, pushed `.claude/worktrees`, shows allocated sizes, and deletes on
   confirmation, using the disk monitor's safety rules.
2. **Ephemeral Chrome profiles and ports** in our verification helpers
   (`--user-data-dir` under scratch, `--remote-debugging-port=0`).
3. **Enforce kache's cap**: find why its collector sticks on "Another GC is
   already running", clear the stale lock, and run collection on a schedule,
   with the 30 GiB cap measured against private bytes.
4. **Durable scratch** under `~/.openagents/scratch/<session>` instead of
   `/private/tmp`.
5. **A quiet-machine flag for soaks**: a lock file that soaks create and that
   the build helper checks before it starts, plus a field in the soak receipt
   that says whether the flag held for the whole run. It's still an honor
   system, but a checked one, and it becomes the broker's `quiet` lease
   later.
6. **Lower the disk monitor's staleness** for agent target directories from
   three days to a few hours on this Mac, and raise its start level, through
   the rule's own settings.

## Open questions for the owner

1. Should Claude Code subagents run as Coder tasks, so they inherit every
   lease, or should the broker serve both kinds of agent?
2. Which jobs must always leave the Mac: soaks, release gates, Terminal-Bench?
3. How many concurrent heavy builds do you want on this Mac? The proposal
   suggests 3 on 18 cores.
4. May the broker pause running builds (`SIGSTOP`) for a `quiet` lease, or
   only delay new ones?
5. Should the real screen be off limits to agents unless you grant the
   `screen` lease, with offscreen capture the default?
6. Is a separate agent identity per session acceptable for issue claims, or
   should claims stay under the one account with a session field?
7. Is a resource broker something to offer other people, or only
   infrastructure for this repository?
