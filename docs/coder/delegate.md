# Delegation

Coder hands one bounded task to one executor, runs several at once under a
stated bound, and records each as an ATIF `Call` named `delegate`.

Status: implemented in `crates/coder` (`delegate.rs`, recorded by
`trace.rs`). The executor is `devin-local`, the Devin CLI on the operator's
computer. A `delegate` step of a program reaches this through
`coder::runtime`; what still runs from a caller in Rust is the program,
not the fan-out.

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
the direct child before the delegation reports — which is also why the
worktree is removed after the executor is gone rather than while it is still
writing to it. A timed-out delegation keeps the bounded output the executor
managed to print. Read [`subprocesses.md`](subprocesses.md).

## Isolation is provided or refused, never pretended

A delegate that writes needs a checkout of its own, or six of them collide.
A `Delegator` told which checkout it is working in makes one worktree per
delegation, runs the executor in it, and removes it when the delegation
ends — whatever it ended as, because a delegation that timed out leaves a
checkout behind just as surely as one that answered.

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

Worktrees separate edits. They do not prohibit writes, and a recorded call
still says `wrote: null` rather than claiming a check nobody runs.
Observing what a delegate actually changed is separate work.

### Coordinate checkout creation and cleanup

Creating and removing worktrees changes shared Git metadata. These operations
must not overlap: a parallel program test reproduced `git worktree add` reading
a sibling's `commondir` while that sibling was removed, leaving only five of six
delegations answered. This is tracked in #9442.

Coder now serializes these operations with `coder-worktrees.lock` in the common
Git directory. Linked checkouts and separate Coder processes use the same lock.
The lock is advisory; external tools must cooperate with it for the same
guarantee. Delegated work stays concurrent because it runs outside the lock.

Lock acquisition and each Git subprocess have a 30-second bound. Git output is
capped at 64 KiB per stream. Coder reserves a new directory atomically, so a
recycled process identifier cannot make cleanup remove an earlier checkout.
Normal completion awaits cleanup and reports a cleanup failure with its path.
On cancellation, the supervisor retains the checkout until the child is reaped;
a background cleanup transaction then removes it. Abrupt process termination
can still leave a checkout for the operator to inspect and remove.

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

## What is not built

- **Evidence of what a delegate changed.** A worktree keeps six delegates
  from colliding; it does not stop one from writing, and nothing yet reads
  back what a delegation left behind.
- **Reaching a fan-out from an operator's sentence.** `coder::runtime` runs
  the program, and the program still runs from a caller in Rust.
