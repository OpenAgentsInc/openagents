# Testing coder against terminal-bench, through the Gym

An audit: what `coder` would need to be measured on real coding tasks, what
the Gym contributes that the existing bench does not, and what a decision
model can and cannot do to make a task come out right.

`~/work/coder` is read here as reference material. Nothing is copied; the
mechanisms below are described at the level needed to rebuild them.

## The finding that comes before any strategy

**`crates/coder` cannot currently run a terminal-bench task.** Not "scores
badly" — cannot run. Its shell loop is bounded well below what the tasks
require, and no decision model changes that.

| Bound | `crates/coder/src/shell.rs` | What tb2 needs |
| --- | --- | --- |
| Command timeout | **15 s** | `sqlite-with-gcov` builds SQLite; `build-cython-ext` compiles three Cython extensions |
| Rounds per turn | **3** (`ROUNDS_MAX`) | 18 to 68 tool calls per task |
| Commands per round | 10 | — |
| Output kept per command | 16 KB | configure/make logs run far past it |
| **Output the judge sees** | **2,048 bytes** (`HEAD_MAX`) | the compiler error is often past byte 2,048 |
| Agent wall clock | unbounded per turn, but 3 rounds | 900 s upstream, 2,400 s for the long task |

A `make` that takes ninety seconds is killed at fifteen. A compile-fix loop
that needs twenty iterations gets three. And the judge — the decision model
this whole directory is about — is shown the **first 2 KB** of a build log,
which is where the banner lives, not the error.

That last row is the interesting one, because it inverts the problem
[#9398](https://github.com/OpenAgentsInc/openagents/issues/9398) found on
real turns. There, a quarter of states were **too large** for the on-device
window. Here the state handed to the judge is **too small to contain the
evidence the decision depends on.** Both are the same defect: the state was
budgeted by a round number rather than by what the decision needs.

**So step one is not a suite.** It is to give the shell loop bounds derived
from the workload, and to decide what the judge should see of a long output
— almost certainly the tail, or a digest, rather than the head.

## What already exists, and what it is worth

`coder` has a real bench. The parts worth keeping:

**A pinned twelve-task cross-section** of `terminal-bench-2`, chosen against
written criteria and recorded with them — version control forensics, build
and compile-fix, a C extension, coverage instrumentation, a security fix,
dataset plumbing, file forensics, service configuration, and a long-horizon
interpreter. The selection document also records the **near-misses and the
exclusions with cause**, so the next round does not re-derive them. That is
the kind of artifact that is expensive to make and cheap to keep.

**A development and held-out split that was respected.** Two quick tasks for
screening, ten held out for promotion decisions, on the stated reasoning that
every verdict so far had flipped on one of the two development tasks. The
trace records which suite an attempt ran under, so a development attempt and
a held-out attempt on the same task never land in the same column.

That is a discipline we got **wrong** and they got right — see
[#9399](https://github.com/OpenAgentsInc/openagents/issues/9399), where half
our locked partition turned out to be adapter training data.

**One attempt per held-out task, all accepted**, on 2026-09-05, with calls,
agent seconds, verifier seconds and a trace per row.

### The number that is not there

**Those ten rows are n = 1 per task.** Ten accepted attempts is not a pass
rate; it is ten samples of a Bernoulli variable whose parameter nobody has
estimated. A task that passes 7 times in 10 and a task that passes 10 in 10
produce the same row here.

Everything below depends on knowing which, and it is the cheapest thing on
this page to find out.

### The instrument problem, which is the real inheritance

`coder`'s own records give the control-against-control spread on its bench:

- Two `--no-plugins` control passes of the same tasks, same night, differed
  by **+4.6% to +157.8%** on prompt tokens.
- One task, one binary, one selection, two control runs: **26 calls against
  16**, and **157,797 tokens against 63,679**.
- Three control attempts on one task cost **16, 33 and 94 calls**.

Against that, its admission rule is that a candidate must be **no worse on
the mean of any metric across every task** — point estimates, no interval,
no noise model at all.

The result is recorded in its own documentation: **the default plugin suite
has never held a member**, and one candidate that swept all three metrics on
all five held-out tasks was correctly refused because the control's own
spread covered it.

**That is the gap the Gym exists to fill,** and it is a better argument for
this migration than anything about receipts or digests.

## What the Gym contributes

| Gym property | What it fixes here |
| --- | --- |
| A **derived noise floor** (σ = 0.0197, two sigma = 0.056 on our suite) | The 63–158% spread above has no floor at all. Every tb2 comparison to date is unjudgeable. |
| **Three-valued verdicts** — failed > unverifiable > passed | A metric with no floor becomes `unverifiable`, not `passed`. This is the difference between an empty suite and an honest one. |
| **Receipt-chained rows** | The baseline cannot be quietly rewritten. `gym merge`, `gym fit` and `gym regress` are all views over the chain. |
| **Content-pinned suites and digested gates** | A changed task set is a different measurement rather than a moved number. |
| **Three-way partition with a locked set** | Already the split `coder` uses by hand; the Gym enforces it in the store. |
| **Label provenance per item** (`outcome` against `author`) | tb2 has something better than either — see below. |
| **`gym regress`** | Compares a door to *itself last week*. `coder` has no CI by policy, so this is its only guard. |
| **Per-family reporting** | Today the suite average moved by one item while 36 of 78 answers changed and two families regressed. The aggregate hides the move. |

## Three tasks, and what a decision would have to be worth

The user's question is what would make a task essentially impossible to fail.
Taking three tasks that answer it differently.

### `openssl-selfsigned-cert` — the completion gate

Described upstream as a fully specified checklist: a key with 0600
permissions, a self-signed certificate with exact subject fields, and
verification artifacts. Every command is known before the first one runs.

The bench selected it as *"the purest batching discriminator in the set;
round count is entirely a tool-habit measurement."* That is true of its
**cost**. It is not what determines its **outcome**.

A checklist task fails when the agent declares done with one requirement
unmet — wrong permission bits, a subject field spelled differently, an
artifact in the wrong path. Batching changes the round count; it does not
change whether the artifacts are right.

**The decision that makes failure nearly impossible is a completion gate:**
before `done`, one Noul per stated requirement — *the artifact at this path
satisfies this requirement* — and a refusal to finish while any is below
threshold.

This is the strongest fit for the contract in the whole suite:

- It is **Noul**, the primitive we can serve and calibrate, not a Choice
  among actions.
- It is **verification rather than generation**, and the verifier's question
  is strictly easier than the generator's.
- It is **checkable against ground truth**, because the task's own verifier
  says whether the artifact was right, so every Noul gets an outcome label
  for free.
- The threshold **gates an action**, which is exactly the case
  [`../decision-models/choosing.md`](../decision-models/choosing.md) says
  calibration stops being optional.

And it repairs a live defect. On real `coder` turns the `action` question is
39 `respond` to 1 `clarify` — near-constant, one of the six questions that
[#9395](https://github.com/OpenAgentsInc/openagents/issues/9395) found score
no better than a constant. *"Is the task actually complete?"* is the same
question asked where it has variance and consequence.

### `build-cython-ext` — the loop-exit decision

Fifty-three calls, 377 agent seconds, and upstream calls it *"the canonical
iterate-on-compiler-output loop."* The first recorded attempt ended
incomplete.

A compile-fix loop fails one way: the agent applies a fix, gets the same
error, and applies the same class of fix again until the clock runs out. The
decision that prevents it is not which fix to try. It is **whether this
failure is the same failure as the last one**, which is a Noul over a pair of
outputs, and it has a mechanical outcome label — did the next round's error
differ?

Two things make this the most promising family on the page.

**`shell_outcome` is the one question that beat its constant today.** On the
real-turn suite it scores +0.228 over a 0.527 majority class, the only one of
seven to beat its baseline at all. Terminal-bench is *entirely* shell rounds,
so the one question we have evidence for is the one this workload is made of.

**And its most common interesting answer currently routes nowhere.** `retry`
comes back on 13 of 22 real shell rounds and `Agent::turn` has no branch that
acts on it ([#9396](https://github.com/OpenAgentsInc/openagents/issues/9396)).
In a compile-fix loop, `retry` is not a footnote — it is the state the task
lives in.

### `fix-code-vulnerability` — where a decision model cannot help

Upstream: few rounds, low volume, reasoning-heavy, and selected deliberately
to *"discriminate on success, not cost — anchors the suite so efficiency wins
aren't confounded with capability losses."*

The agent reads a pinned checkout, identifies a CWE-classified
vulnerability, and reports it. It fails by being **confidently wrong about
which vulnerability it is.** There is no premature `done` to catch, no loop
to break, and no round to save.

**A completion gate does not help, because the agent's answer passes its own
check.** This is the shape of failure our own measurements keep producing:
the adapter that got more accurate and more confidently wrong; the specialist
that emitted a default answer on 36 of 41 out-of-catalogue items at mean
confidence 0.974; the compiled adapters wrong at 0.97 to 1.00.

There is one hypothesis worth stating and not believing yet: that a
*verifier* question — *does the reported CWE match the code path shown?* —
is easier than the generator's, and a separately-asked model might catch what
the generating model missed. That is the generator-verifier gap, it is
plausible, and **we have no evidence for it.** What evidence we do have
points the other way: on 38 real decisions, routing changed the agent's
behaviour twice and **both changes were wrong.**

Keeping this task in the suite is the point. Without a task the decision
engine cannot help, every efficiency gain reads as progress.

## The taxonomy

| Decision | Primitive | Tasks | Outcome label available? |
| --- | --- | --- | --- |
| Is every stated requirement met? | Noul per requirement | openssl, nginx, sanitize-git | **Yes** — the verifier says |
| Is this failure the same as the last? | Noul over a pair | build-cython, nginx, sqlite | **Yes** — did the next error differ |
| Does this need inspection, or can I answer? | Noul (`needs_code`) | regex-log, fix-code-vuln | Yes — did it run anything |
| Is this round pass, retry, or stop? | Choice | every shell round | **Yes**, and it is the one question that beats its constant |
| Which transcript turns still matter? | Score | schemelike, build-cython | Weakly — an ordered rubric, and see [#9394](https://github.com/OpenAgentsInc/openagents/issues/9394) |
| Which fix should I try? | — | fix-code-vuln | **No. Not a decision model's job.** |

The bottom row is the boundary. Everything above it is *process*: finishing
early, looping, wandering, running out of turns. Everything below is
*capability*.

## What "impossible not to solve" can mean

Honestly: **a decision engine cannot make a hard task solvable. It can make
a solvable task hard to fail.**

Those are different claims, and the second is worth a great deal, because the
recorded failures on this bench are mostly process failures — a run that
leaves its directive, a run that cannot be stopped, a loop that repeats, a
container killed mid-build, an attempt that ends incomplete after 514
seconds. The first recorded `build-cython-ext` attempt failed on wall clock,
not on reasoning.

For the tasks where process is the failure mode, the claim is defensible and
testable. For `fix-code-vulnerability`, it is false, and the suite should keep
it in order to keep saying so.

## The migration

**Phase 0 — make the loop capable, and measure what is there.** Derive the
shell bounds from the tasks instead of from round numbers; decide what the
judge sees of a long output (the tail or a digest, not the head). Then run
each task **n times, not once**, and publish the pass rate with its interval.
Nothing below is interpretable without it.

**Phase 1 — derive the floor on this bench.** Repeat the control-against-
control measurement the way
[`../lev/measurements/2026-09-19-seed-variance.md`](../lev/measurements/2026-09-19-seed-variance.md)
did: same door, same tasks, blocks, and report the spread for pass rate,
calls, tokens and seconds. Expect it to be large. **A floor that makes most
comparisons `unverifiable` is the correct outcome**, and it is what `coder`'s
own bench has been missing.

**Phase 2 — harvest decision points into a suite.** The same shape as
`coder-turns-v1`: reconstruct the state at each decision exactly as the
production code builds it, pin a digest, partition three ways, and carry
`label_source` per item.

tb2 offers something `coder-turns-v1` could not: **episode-level ground
truth.** The verifier says accepted or rejected. Use it carefully —
credit assignment across forty rounds is not solved by having an outcome at
the end. Label what is *locally* verifiable (did this command succeed, did
the error change, was this artifact right) and treat the episode verdict as a
coarse filter, not as a label on every decision inside it.

**Phase 3 — check headroom before concluding anything.** [#9392](https://github.com/OpenAgentsInc/openagents/issues/9392)
ran a full experiment against a partition scoring 0.975 at baseline, where
total headroom was 0.025 against a floor of 0.056: **no win was available at
any strength.** If the completion-gate items are 95% already-correct, that
family cannot host a comparison. Measure the incumbent first and publish the
headroom beside the digest.

**Phase 4 — gate, and keep the call site in our code.** `deployment-v1`
already judges latency, cost and refusal rate; a router in front of every
shell round is exactly the workload where a second of judge latency is the
product. And the decision must be *called by the loop at points we choose* —
`coder`'s own strongest null result is that model-called capabilities were
never adopted: **zero calls across 18 attempts on six task shapes**, while
the offer cost 2,307 bytes on every request of every turn.

## What to measure first

One task, one question, one week:

**`openssl-selfsigned-cert`, the completion gate, as a Noul per requirement,
scored against the upstream verifier.**

It is the fastest task in the suite, its requirements are enumerable, its
verifier gives a free outcome label for every Noul, and the primitive is the
one we can serve and calibrate on all three doors. If a completion gate
cannot be made to pay there, it will not pay anywhere else — and that is a
result worth a week.

## The trap

**Do not port `coder`'s admission rule.** A conjunctive sweep over point
estimates, against an instrument whose control-control spread reaches 158%,
produces exactly one outcome, and its own documentation records it: a suite
that has never admitted anything, and a genuine winner refused because the
noise covered it. Its own analysis prescribes the fix it has not applied —
*"a task-bootstrapped interval says whether the decision could have gone the
other way."*

We have the interval. That is the thing worth bringing.
