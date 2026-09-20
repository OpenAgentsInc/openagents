# Delegation

Coder hands one bounded task to one executor, runs several at once under a
stated bound, and records each as an ATIF `Call` named `delegate`.

Status: implemented in `crates/coder` (`delegate.rs`, recorded by
`trace.rs`). The executor is `devin-local`, the Devin CLI on the operator's
computer. The program runtime that reaches this from an operator's sentence
is not built yet, so today the call site is a caller in Rust.

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
use coder::delegate::{Delegator, Executor, Task};

let delegator = Delegator::new(Executor::devin_local()?)
    .in_directory("/Users/someone/work/openagents")
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

`resolve` searches `PATH` and then the directories a spawned process usually
does not have — `~/.local/bin`, `~/.bun/bin`, `~/.cargo/bin`, `~/bin`,
`/opt/homebrew/bin`, `/usr/local/bin` — and returns an absolute path, which
the trace records. `CODER_DEVIN` points at a copy somewhere else.

A probe that shells out to a bare name reports the capability absent on a
machine that has it.

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

A bound that expires kills the delegate. Without that the host stops waiting
while the executor keeps running against the repository, which makes the
bound a timer rather than a bound.

## Isolation is not pretended

A delegate that writes needs a checkout of its own, or six of them collide.
That is not built. Rather than record `isolation: worktree` and run in the
shared directory anyway, a task that asks for a worktree is refused with
`isolation_unavailable`, and a task that says it writes without one is
refused with `isolation_required`. Both refusals happen before anything
spawns.

So every delegation today is read-only, which is why every recorded call
says `wrote: null` rather than claiming a check nobody runs. The first task
is read-only and needs no isolation; the write path waits for a real one.

## Running the live check

The delegation tests run against a stub executor, so `cargo test -p coder`
needs no Devin CLI and no network. Two tests are ignored by default and run
against the real one:

```sh
CODER_DELEGATE_DIR=/path/to/openagents \
  cargo +1.97.1 test -p coder --test delegation -- --ignored --nocapture
```

Run them from a checkout the executor trusts. `CODER_DELEGATE_DIR` names the
directory the delegates run in, which matters because a git worktree under
`/private/tmp` is one this executor refuses.

## What is not built

- **Worktree isolation**, and with it delegations that write.
- **The program runtime** that reaches a fan-out from an operator's
  sentence. Until it exists, the call site is Rust rather than a sentence.
- **A capability probe** that resolves the manifest and decides whether to
  offer the executor at all. `Executor::devin_local` resolves one binary;
  it does not read a manifest or admit a set of bounds.
