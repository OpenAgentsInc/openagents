# The determinism thesis

Status: Coder's working thesis, 2026-09-24. It explains why earlier coding
agents, including our own versions, plateaued, and why the approach we're
building now should do better. It also states the predictions that our
benchmark runs will confirm or invalidate. The strategy that follows from
it is in [the Luna pivot](luna-pivot.md).

## The thesis in one paragraph

A coding agent should make every decision it can with deterministic code.
It should use Jev only for narrow, typed, calibrated judgments, and use a
generative model only for the part that's irreducibly generative: writing
code and tests. At the start of every issue-to-PR run, a mostly
deterministic process, with Jev's help, turns the issue into an exhaustive
executable acceptance suite, proven to fail on the untouched code. The
agent then loops until that frozen suite passes. When "done" is a program
state rather than a model's opinion, a cheap model iterating many times
can reach it. So pass rate rises and cost falls together.

## Why the usual approach plateaus

Most coding agents are a while loop around one model with a handful of
tools. The model decides everything:
- what to read
- what the task requires
- what to change
- whether the change works
- when to stop

Each decision is a sample from a distribution. Three things follow.

1. **Errors compound.** A run is a chain of decisions, and one bad
   decision (a misread requirement, a skipped test, a premature "done")
   can sink the whole run. The only way to keep the per-step error low
   enough is a bigger, slower, more expensive model. That's why the
   frontier harnesses cost dollars per task.
2. **The judge is the defendant.** When the model that wrote the fix also
   decides whether the fix works, its blind spots are the same in both
   roles. The failure we keep seeing is "claimed success it didn't earn."
3. **The KV cache shapes the design.** A conversation that only grows is
   cheap to extend and expensive to rebuild, so harnesses keep stale
   context instead of rebuilding the right context for each step. That
   design constraint is the subject of
   [episode 287](../../transcripts/287.md).

## Why our own versions plateaued

The [version arc](../../terminal-bench/2026-09-24-version-arc.md) and the
matched experiments show the same pattern inside Coder One.

- **Hints to a model aren't determinism.** The first Coder One fed Jev's
  judgments to a Gemini loop as hints. The model was free to ignore them,
  and removing them passed one more task.
- **Model judgment stacked on model work doesn't add signal.** From v2 to
  v10 the controller gained checks, a second executor, repair, persistence
  rounds, effort routing, and escalation. Every one of these keyed on a
  model-produced or template-produced signal, checked after the fact.
  - None of those signals separated passes from failures: checks that "all
    passed" meant a pass 50% of the time
    ([truthful checks](../../terminal-bench/2026-09-24-truthful-checks.md)).
  - Escalation rescued 0 of 12 trials
    ([escalation](../../terminal-bench/2026-09-24-escalation-on-failed-check.md)).
  - No pass was credited to persistence
    ([persistence v10](../../terminal-bench/2026-09-24-persist-v10.md)).
  - With the executor held fixed, the whole controller cost 68% more for no
    significant gain
    ([matched test](../../terminal-bench/2026-09-23-matched-controller-targeted.md)).
- **Routing moves work between models without removing it.** Choosing the
  executor or the effort per task helped at the margin. It has the same
  ceiling as the loop it routes into.

## What worked, and the pattern behind it

Every clear win so far came from code deciding something a model used to
decide:

| Win | What code took over | Evidence |
| --- | --- | --- |
| Jev probes instead of model exploration | What to read first: a fixed battery of read-only commands, with Jev picking among the outputs | Probe v2 into lean Opus passed 24/24 on the development panel at 61% less than Claude Code |
| The coverage packer | What evidence reaches the model: every requirement's records, delivered by code | Luna went from 0/3 to 3/3 on log summaries with no other change ([tunable results](../../terminal-bench/2026-09-23-tunable-results.md)) |
| Lean executor settings | Which tools exist, and how long context lives | Most of the measured savings |
| The combined verdict | Which signals count: fitted to the verifier and tested on held-out tasks | Nearly 4× the failure recall of today's checks ([truthful checks](../../terminal-bench/2026-09-24-truthful-checks.md)) |

The winning public trajectories point the same way. Fable's winners read
longer before they edit, test before their first edit, and test after
every edit
([strategy fingerprints](../../terminal-bench/2026-09-24-strategy-fingerprints.md)).
Those are disciplined, repeatable procedures, which is what code can
enforce on a cheaper model.

## The mechanism: an acceptance contract, then a loop to green

1. **Build the contract** before any fix: probes, the requirement map,
   evidence per requirement, then an executable acceptance suite. Every
   requirement gets tests. Jev verifies that each test decides its
   requirement and asserts only what the task states. Code runs the suite
   on the untouched workspace, and every test must fail first.
2. **Freeze it.** The suite's digest is recorded, and the loop can't edit
   it.
3. **Loop until green.** Short model sessions, each with a context rebuilt
   from the contract and the current red tests, edit until every test
   passes or a bound is hit. Jev judges between sessions: next, retry,
   stuck. Code decides.

Split the pass rate into two factors:

- **The contract is faithful and complete**: if the suite passes, the task
  is solved. We raise this with determinism and Jev: probes, requirement
  extraction, red-first proof, and faithfulness checks. It doesn't need a
  big model at every step.
- **The loop reaches green, given a good contract.** We raise this with
  cheap iterations. When the target is fixed and checkable, each step
  needs less intelligence: the model only has to make the next red test
  pass. A model that costs cents per session can afford many sessions.

The usual harness multiplies many uncertain model decisions together. The
contract replaces most of them with checkable facts. That's why this
should raise pass rate while cutting cost, and why it favors a cheap model
like GPT-6 Luna run through [Microluna](microluna.md).

## Principles

1. **Code owns control.** Order, bounds, what runs, and when to stop are
   code.
2. **Jev judges narrowly.** A Jev question is typed, small, cached, and
   measured against ground truth. Jev informs a decision code makes. It
   never grants authority on its own.
3. **The model generates.** Code and tests are the model's job. Deciding
   whether they're right isn't.
4. **"Done" is an observed state:** the frozen suite is green, not a
   report that says so.
5. **Evidence beats instruction.** Put the right records in front of the
   model rather than telling it to look harder.
6. **Every signal earns its place.** A check, a trigger, or a judgment
   stays only if it measurably separates passes from failures on tasks it
   wasn't fitted on.
7. **Measure matched, and kill losers early.** Compare one change at a
   time against a matched baseline, and stop an arm as soon as it can't
   win.

## Where the thesis could be wrong

- **Hidden requirements.** A verifier can test what the task never states.
  A faithful suite written from the task text can be green on a solution
  that still fails. If this dominates, the first factor has a hard
  ceiling. Mitigations: the task's own examples, probes into existing
  tests, and the requirement kinds winners check that losers skip.
- **Correlated blind spots.** If the same model writes the tests and the
  fix, it can make the same mistake twice. Red-first and Jev faithfulness
  checks reduce this but don't remove it.
- **Tasks that don't test cheaply.** Long builds, interactive programs,
  performance targets, and nondeterminism make "run the suite" slow or
  noisy, and the loop pays for it every iteration.
- **Capability limits.** On some tasks a cheap model can't reach green
  however clear the target is. Those tasks are the honest remainder for a
  stronger executor, and every one is logged as a gap to close.
- **Setup cost.** Determinism isn't free: probing, building the contract,
  and running suites take wall time that the harness has to pay down.

## Predictions

The benchmark work in flight will confirm or invalidate each of these.

1. **A green suite predicts a pass.** Offline, on retained graded trials,
   a frozen acceptance suite passing predicts a verifier pass far better
   than today's checks or the combined verdict (#9588).
2. **Luna's pass rate rises with the contract.** On the TB4 subset where
   Luna direct is at 0 of 20 so far (#9583), Microluna with the contract
   and loop passes materially more (#9585, #9588).
3. **Cost per pass drops by an order of magnitude** against Opus 5.5 direct
   on the tasks where Luna plus the contract passes.
4. **Failures become honest.** The share of runs Jev flags as "claimed
   success it didn't earn" falls. What remains is mostly "didn't reach
   green within the budget," which is a failure the operator can see and
   act on.

**What would invalidate the thesis:**
- If green suites routinely fail the verifier, the contract isn't faithful
  enough, and the first factor is the ceiling.
- If loops with a good contract rarely converge on Luna, the second factor
  is a capability ceiling, and the cheap-model half of the thesis is wrong
  for that class of task.

Either result is useful: it tells us where to invest next.

## Related

- [The Luna pivot](luna-pivot.md): the strategy that follows from this
  thesis.
- [Microluna](microluna.md): the harness for many short sessions.
- [Thoughts on a TypeSafe coding agent](thoughts-on-a-typesafe-coding-agent.md):
  the founding notes.
- [Coder components](../../optimization/coder-components.md): the tunable
  components.
