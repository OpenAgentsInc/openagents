# Delegation

Coder hands one bounded task to one executor, runs several at once under a
stated bound, and records each as an ATIF `Call` named `delegate`.

Status: implemented in `crates/coder` (`delegate.rs`, recorded by
`trace.rs`). The executor is `devin-local`, the Devin CLI on the operator's
computer. A `delegate` step of a program reaches this through
`coder::runtime`, and an operator's sentence reaches the program —
[How a sentence reaches it](../programs.md#how-a-sentence-reaches-it)
covers the path.

Read [`../programs.md`](../programs.md) for why delegation is reached
through a program rather than offered to the model, and
[NIP-CAP](../../nips/openagents/NIP-CAP.md) for what a capability manifest
says about an executor.

## Why this exists

The `devin-fan-out-six` golden in `crates/coderbench` records six
delegations, and a shell script made them. Coder could not. That is the
step the episode spends nearly all of its time in, so until Coder could
take it, the golden was a specification rather than a path Coder walks.

## What one delegation is

```rust
use coder::delegate::{Delegator, Task};
use coder::survey::Survey;

// The executor comes from the probed manifest: the binary is the absolute
// path the probe resolved, and the arguments are the manifest's `invoke`.
let repository = std::path::Path::new("/Users/someone/work/openagents");
let executor = Survey::read(Some(repository), repository)
    .executor("devin-local")
    .expect("devin-local is present here");

let delegator = Delegator::new(executor)
    .in_repository(repository)
    .bounded_to(6);

let answers = delegator
    .fan_out(vec![
        Task::reading(
            "How many `pub struct` declarations are in crates/atif/src/document.rs? \
             Answer with a single integer and nothing else.",
            "crates/atif/src/document.rs",
        )
        .expecting("5"),
    ])
    .await;
```

Each one records as the golden's calls do:

```jsonc
{"agent": "devin-local", "isolation": "directory",
 "prompt": "…", "bounds": {"minutes": 5}}
```

with the capability, the resolved binary, the fan-out's width, what the
delegate read, what it was expected to answer, whether it did, and whether
it wrote in the call's `extra`. `crates/coderbench`'s `observe` counts a
delegation by the call's name, so a recorded fan-out reads back the way a
golden does.

## Parallelism is the feature

Six sequential delegations took 80.7 seconds in the first recording and six
parallel ones took 46.6. So `fan_out` is concurrent from its first version
rather than sequential with concurrency added later, and the width is a
bound rather than an ambition: `bounded_to` sets it, the default is six, and
every recorded call carries the width it ran under.

Results come back in the order the tasks were given, whatever order they
finished in, so a caller can pair an answer with the question it asked.

A live run of the golden's six questions from this repository, at a width of
six:

| | |
| --- | --- |
| Wall clock | 95.9 s |
| Summed agent time | 406.3 s |
| Correct | 6 of 6 |

## Resolve the binary by absolute path

`devin` is on the operator's interactive `PATH` and not on the one a spawned
subshell inherits. The first recorded attempt at this fan-out failed six
times out of six with `command not found` on a machine that had the binary.

The capability probe searches `PATH` and then the directories a spawned
process usually does not have — `~/.local/bin`, `~/.bun/bin`,
`~/.cargo/bin`, `~/bin`, `/opt/homebrew/bin`, `/usr/local/bin`,
`/usr/bin`, `/bin` — and returns an absolute path, which the trace records.
`CODER_CAPABILITY_PATH` adds directories ahead of all of them.

A probe that shells out to a bare name reports the capability absent on a
machine that has it.

## Probing needs an operator's approval

Reading `capabilities/` never runs an argv. A probe — the version argv,
then the manifest's `workspace_probe` — runs only after the operator
approves the exact manifest with `capability-trust approve <slug>`, which
pins the manifest's digest, the adapter's canonical path and contents,
and any repository-controlled script an argv names. The store lives
outside the checkout, and a record copied into one authorizes nothing: a
manifest cannot approve itself. A manifest nobody approved is `unprobed`,
and a probe that times out, exits wrong, or answers past the output cap
is `unknown` — neither state is a route. `crates/capability` owns the
contract; CoderBench's preflight runs the same one.

## The manifest says how to drive the executor

Nothing in `delegate.rs` names an executor. `survey::executor` builds an
`Executor` from a probed manifest: the binary is the path the probe
resolved, the arguments are the manifest's `invoke`, and the refusals are
the ones its `refuses` list declares. A binary name and an argv written
into the source would be a second source of truth beside the manifest that
exists to be the first, and the two would drift.

A task that writes runs the manifest's `invoke_writing` argv when it
declares one, and `invoke` otherwise. `devin-local` declares
`--permission-mode dangerous` there: the CLI's default non-interactive mode
rejects every tool call that would edit a file or run a command, so three
writing delegations under it came back "answered" with three unchanged
worktrees. The mode turns off the executor's own confirmation, not the
host's: the delegate still runs inside the boundary, which lets it write its
own worktree and its granted adapter state and nothing else. The approval
pins `invoke_writing` beside `invoke`, so a manifest that changes either one
loses its approval until the operator approves it again.

The executor also carries a filesystem `Policy`, built from the proof the
probe ran under: the adapter-state directories the operator's approval
granted, and the approval's own material — the manifest, the resolved
adapter, and the directory the approval store lives in — sealed against
the delegate it approves. A manifest's `enforces` list is a claim about
bounds and never becomes a grant; a caller-built executor carries
`Policy::empty` and grants nothing.

## The filesystem boundary is enforced, not declared

`writes: false` on a task is a statement, not a wall. Every delegated
command runs inside a `coder-boundary` write boundary, and the record
carries the resolved profile — backend, checkout, writable, protected,
and sealed paths — in the call's `extra`.

- A read-only task denies `file-write*` everywhere except a private
  scratch directory and the adapter state the policy grants. The
  boundary exports its scratch as `TMPDIR` and repoints nothing else:
  `HOME` and the `XDG` variables pass through from the caller, so where
  an adapter keeps its state is the approval's word — a granted writable
  path — never a variable the boundary redirected. The rest of the
  environment, the declared argv, and the working directory are the
  manifest's and the caller's, preserved.
- A writing task adds its own worktree to the writable set. The main
  checkout stays protected — the worktree beneath it is the profile's
  one exception — and the common Git directory stays sealed against
  every exception, so a delegate cannot commit, stage, or corrupt the
  shared object store. Its edits stay in the worktree for the reviewer.
- A platform with no enforced backend, a grant that overlaps a protected
  or sealed path, or a path that does not resolve refuses the delegation
  as `boundary_unavailable` before anything spawns. There is no
  unrestricted fallback.

The boundary and the checkout are both held through
`supervise::Job::run_holding`, so neither the profile file, the owned
scratch, nor the worktree is released until the process group is
terminated, reaped, and drained — including when the caller walks away.

## The approval is decided again at dispatch

A survey's proof is a cache: it says the probe ran under an approval, not
that the approval still holds when the delegation runs. The policy an
executor carries names the approval's identity — the manifest's digest
and `invoke` argv as surveyed, the adapter's canonical path, and the
store the record lives in — and `Delegator::run` re-decides it before
anything spawns. The manifest is re-read and compared byte-for-byte: a
manifest that changed since the survey refuses the cached executor with
`unapproved` even when the operator approved the change, because the new
approval names a manifest the old executor's argv never read — a fresh
survey builds the executor that carries it. When the manifest matches,
the store is re-read once and the record's adapter path, adapter bytes,
and pinned argv files are verified again in the directory the argv will
run in, through `Trust::decide_verified`: the grants and pins the
boundary is built from come from the same store snapshot the decision
checked, not a second read. A manifest rewritten since approval, an
adapter replaced, a retargeted script, or a revoked record refuses the
delegation with `unapproved`, and a grant the operator withdrew is not
handed out anyway.

One refinement the pins alone do not cover: sealing protects what a
pinned word *points at*, but the argv spells the word — a script path
inside a directory the grant makes writable can be retargeted by the
delegate after verification, swapping the pinned script for one the
approval never read. The policy keeps each pinned word's spelled path,
and a writable grant — including the writing checkout itself — that
covers that path or any directory above it refuses the delegation as
`boundary_unavailable` before anything spawns. The check follows the
word's real ancestry, so a symlinked directory inside a grant counts the
same as a plain one. The same check covers the lexical adapter, manifest,
and approval-store paths. Sealing their canonical targets alone would leave
a writable alias outside those targets available for retargeting.

What this does not promise: the seal and the ancestry check are the
boundary's word, enforced at spawn and held while the child runs. A
process outside the boundary — another shell, another agent — can still
rewrite an approved file between the check and the read. The approval
store is the host's guarantee about its own writes; this implementation
does not claim to close races against writers the boundary does not
hold.

## Four outcomes, not one

| Outcome | What it means | ATIF outcome |
| --- | --- | --- |
| `Answered` | The executor ran the task and answered. | `completed` |
| `Refused` | The executor declined, in a way its manifest declares. | `cancelled` |
| `TimedOut` | The bound expired and the host killed it. | `failed` |
| `Failed` | The executor ran and exited non-zero. | `failed` |
| `Harness` | No answer reached the host: nothing spawned. | `failed` |

This is the line `gym::eval::classify` draws for a decision door, for the
same reason: **a typed refusal is the executor's own answer and a failure
carrying no code is the harness.** A present executor that declines a
directory has told you something about itself. A binary that would not spawn
has told you something about the machine. The trace's `status` field keeps
all of them apart, because the ATIF outcome alone cannot.

The one refusal `devin-local` declares today is `untrusted_workspace`: six
of six delegations from a git worktree under `/private/tmp` came back with
`Refusing to run in an untrusted workspace`. The capability was installed,
detected, and unavailable for that directory — a state a present-or-absent
probe cannot represent.

The executor publishes no typed code for that; it exits 1 and says why on
stderr. So an `Executor` carries the phrases it declares and matches them.
That is a bounded field of a named executor's description rather than intent
routing: the route is already chosen and the executor is already named.

A bound that expires kills the delegate **and everything it started**.
Without that the host stops waiting while the executor keeps running against
the repository, which makes the bound a timer rather than a bound; killing
only the direct delegate leaves its background children doing the same
thing. `crates/supervise` runs each delegation in a process group of its
own, terminates that group on the bound or on a cancelled caller, and reaps
the direct child before the delegation reports — which is also why a
read-only worktree is removed after the executor is gone rather than while
it is still writing to it. A timed-out delegation keeps the bounded output
the executor managed to print. Read [`subprocesses.md`](subprocesses.md).

## Isolation is provided or refused, never pretended

A delegate that writes needs a checkout of its own, or six of them collide.
A `Delegator` told which checkout it is working in makes one worktree per
delegation and runs the executor in it. A read-only delegation's checkout
is removed when the delegation ends; a writing delegation's stays where
the executor left it — its edits are owed to a reviewer, not silently
merged or discarded — and the recorded call names the retained path.
The retained worktree stays in `git worktree list` for the reviewer to
inspect, merge, or remove: the program runs the delegation, and the
operator finishes it.

A delegator that was **not** told refuses a task asking for a worktree with
`isolation_unavailable`, and any delegator refuses a task that says it
writes into the directory it shares with five siblings, with
`isolation_required`. Both refusals land before anything spawns. Nothing
records `isolation: worktree` and runs in the shared directory.

The worktrees go under the repository, in `.coder/worktrees`, rather than
under the system temporary directory. That is not tidiness: `devin`
declines a directory nobody has trusted interactively, and six of six
delegations from a worktree under `/private/tmp` came back with `Refusing
to run in an untrusted workspace`. A worktree inside a checkout the
operator already trusts is accepted.

### The worktree is trusted for the run, and no wider

The Devin CLI keeps one trusted-workspace list per data directory, at
`$XDG_DATA_HOME/devin/cli/trusted_workspaces.json`. A worker that runs
under its own `XDG_DATA_HOME` and an operator who trusted the checkout
under the default `~/.local/share` hold two lists that disagree, and the
first burn-down episode met that split as `untrusted_workspace` from a
worktree the boundary already confined the delegate to
([#9413](https://github.com/OpenAgentsInc/openagents/issues/9413)).

A worktree Coder made a moment ago is a directory no list can already
trust and no operator can be asked about, so the `Delegator` answers the
question itself for a `devin-local` delegation in a checkout of its own:

- It enters the worktree's canonical path in the list under the
  `XDG_DATA_HOME` the executor inherits, falling back to
  `$HOME/.local/share`, after the boundary is built and before the
  executor spawns. The entry is that path and nothing above it. The
  repository, `.coder/worktrees`, and the shared directory are never
  entered — a delegation in the shared directory runs where the operator
  chose, and that directory is the operator's to trust.
- It withdraws the entry when the run ends, whether the delegation
  answered, failed, or timed out, and a withdrawal that fails is reported
  as `Harness`. A retained worktree is the reviewer's to read and fetch
  from, not to run the executor in. A caller that walks away
  mid-delegation leaves the entry behind with the worktree; the path it
  names is a checkout under `.coder/worktrees`, and nothing wider.
- A list it cannot read or write is the harness, not the executor: the
  delegation reports `Harness`, because the refusal the delegate would
  have given was this host's doing.

Other keys in the file are kept as they are, and writers take a lock
beside it, so six delegations starting together each see the other five.
The `Delegator::trusting_under` builder points a test or a worker at a
different data directory.

Worktrees separate edits; the filesystem boundary is what prohibits
writes. A recorded call still says `wrote: null` rather than claiming a
check nobody runs — what a delegate actually changed is the workspace
snapshot's answer, taken independently of the run.

### Coordinate checkout creation and cleanup

Creating and removing worktrees changes shared Git metadata. These operations
must not overlap: a parallel program test reproduced `git worktree add` reading
a sibling's `commondir` while that sibling was removed, leaving only five of six
delegations answered. This is tracked in #9442.

Coder now serializes these operations with an exclusive `flock` on the common
Git directory itself. Linked checkouts and separate Coder processes use the
same lock. The lock is advisory; external tools must cooperate with it for the
same guarantee. Delegated work stays concurrent because it runs outside the
lock. Locking the directory rather than a file in it, and removing
`.coder/worktrees` and `.coder` when the last checkout leaves them empty, is
what lets a read-only fan-out leave the workspace exactly as it found it: the
CoderBench workspace comparison reads every path, and a lock file or an empty
metadata directory would count as a write.

Lock acquisition and each Git subprocess have a 30-second bound. Git output is
capped at 64 KiB per stream. Coder reserves a new directory atomically, so a
recycled process identifier cannot make cleanup remove an earlier checkout.
Normal completion awaits cleanup and reports a cleanup failure with its path.
On cancellation the supervisor holds the checkout until the child is reaped.
What happens then depends on what the task declared: a writing delegation's
checkout was retained before the executor spawned, so it — and whatever the
delegate already wrote into it — stays on disk and in `git worktree list` for
the reviewer even when no result came back; a read-only delegation's is
removed by the cleanup transaction. Abrupt process termination can still leave
a checkout for the operator to inspect and remove.

## Running the live check

The delegation tests run against a stub executor, so `cargo test -p coder`
needs no Devin CLI and no network. Two tests are ignored by default and run
against the real one:

```sh
CODER_DELEGATE_DIR=/path/to/openagents \
  cargo +1.97.1 test -p coder --test delegation -- --ignored --nocapture
```

Run them from a checkout the executor trusts. `CODER_DELEGATE_DIR` names the
repository the delegates run in, which matters because a git worktree under
`/private/tmp` is one this executor refuses.

## Delegating over the relay

A capability whose manifest says `"transport": "relay"` has no adapter on
this machine. Its adapter is a `coder-worker` somewhere on the relay, and
each delegation to it is one [NIP-CJ](../../nips/openagents/NIP-CJ.md) job.
The checked-in one is [`capabilities/devin-relay.json`](../../capabilities/devin-relay.json).

Select it for a turn with `CODER_DELEGATE=devin-relay`, and name the far
end with the same two variables the relay door already uses:

```sh
CODER_RELAY=wss://relay.openagents.com \
CODER_WORKER=<worker pubkey, hex> \
CODER_DELEGATE=devin-relay \
  coder -p "…"
```

What this host needs: a Nostr identity (`~/.openagents/nostr-secret`,
created on first use), the relay URL, and the worker's public key. What it
does not need: a Devin CLI, a `capability-trust` approval, a trusted
workspace, or the worker's credentials. Those stay on the worker's host,
where the approval, the filesystem boundary, and `CODER_WORKER_ALLOW`
decide what runs. A terminal that has none of them still runs the fan-out;
the measurement in [`relay-transport.md`](relay-transport.md#delegations-over-the-relay)
was taken from one.

**The probe is a job.** Before the program runs, `Survey::probe_relays`
sends the worker a `{"v":2,"type":"probe"}` request and records the answer
as a `capability_probe` check in the trace, the same check a local
executable probe records:

| The worker… | Presence |
| --- | --- |
| answers, and its door delegates (`CODER_EXECUTOR` set) | `present` |
| answers without an executor, refuses admission, or errors | `present_unavailable`, with the typed cause |
| cannot be reached, or `CODER_RELAY`/`CODER_WORKER` is unset | `absent` |

A relay capability that is not `present` is not delegated to; the turn
takes the same refusal path a missing local executor takes.

**One task, one request.** Each task in the fan-out becomes one kind
`25900` event, NIP-44 encrypted to the worker and tagged `p` with its key,
on a connection of its own. The worker's kind `27000` feedback and kind
`26900` result come back encrypted to the Coder identity and tagged `e`
with the request. `fan_out` bounds the terminal side with the manifest's
`concurrent_max`; the worker bounds its own side with `CODER_WORKER_JOBS`
and refuses the overflow with the typed code `busy`, which the trace
records as `refused: busy` on that delegation and nothing else.

**What the trace records.** A relayed delegation's `delegate` call carries
`"capability": "devin-relay"`, the status and the answer as any delegation
does, and under `relayed`: the relay URL, the worker's key, the request
event ID, the model the worker reported (`devin-local` for the executor
door), and the count of feedback events. The request ID is the join key
between the trace, the worker's log, and the relay's `debug` log.

## What is not built

- **Grading what a delegate changed.** The boundary confines writes and
  the retained checkout keeps them, and `crates/coder-boundary` snapshots
  observe a tree before and after a run; the CoderBench side that grades
  a run against that observation is separate work.
