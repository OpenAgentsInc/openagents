# Text optimization here: nine attempts, no wins

A history of DSPy, GEPA, and prompt optimization across `openagents`,
`psionic`, `coder`, and `backroom`, and what it means for the Gym.

Read this before proposing a tenth attempt.

## The finding

**Nine separate programs for optimizing text or program parameters have been
built in this workspace since December 2025. Not one has produced a text
change that beat a baseline on a measurement anyone would accept today.**

The ratio is lopsided in a specific way: we have built a great deal of
machinery *around* optimizers and almost never *run* one. Of the nine, one
ran at scale and lost, two produced deltas at n=2 and n=4, one produced a
number whose arm has no producing code anywhere in the repository, and the
other five died as infrastructure before they ever ran.

Where evidence exists, it points against the technique:

| Experiment | Result |
| --- | --- |
| HillClimber, Dec 2025, 202 runs | 0 passes. *"Best configs are the original ones (no hints)."* |
| HillClimber validation, 317 runs | 1 pass, 0.3% |
| `env_facts` prelude, coder rounds 13–15 | kept once, then **reverted twice** on confirmation |
| Router Power learned playbook | lost ~18 points to plain retrieval |
| An external GEPA tool's own benchmark | Jev 80% zero-shot → **77%** optimized |

Across roughly 34 rounds of the coder optimization loop, the only three that
varied instruction text are rounds 13 to 15, and all three ended reverted.

## The nine

1. **HillClimber / TestGen** (Rust, Dec 2025). LLM-generated one-line hints
   for Terminal-Bench tasks. The only text optimizer here that ran at scale.
   Verdict above. Archived in `backroom`.
2. **PHP DSPy** (Laravel, Jan 2026). `App\DSPy\Predict`, a signature, a
   controller. Superseded, no results.
3. **`crates/dsrs`** (Rust, Jan–Feb 2026). The largest: 42,408 lines, with
   four real optimizers including a faithful 534-line GEPA port. **None was
   reachable.** The single production entry point read
   `bail!("Unknown optimizer: {}. Currently supported: mipro")`, and the one
   reachable optimizer wrote `Scorecard::new(0.0)` — a successful run
   persisted no measured gain by construction. Deleted twice.
4. **DSE v1** (TypeScript/Effect, Feb 2026). Shipped to production. See
   below.
5. **`psionic-optimize`** (Rust, Mar–Apr 2026). 4,622 lines of GEPA
   re-derived: reflective dataset, mutation proposer, Pareto frontier,
   lineage-aware merge. **There is no LLM in it.** Its two shipped mutation
   strategies are an operator-supplied string map and a labelled test double.
   Still present, never given a proposer.
6. **Probe GEPA campaign** (Jun 2026, four repositories at once). ~60 files,
   payment modes, settlement gates, lease proofs. Its own closeout audit:
   *"It does not yet prove live distributed benchmark execution."* Psionic's
   side records **zero successful rollouts** — both live imports land as
   `agent_failed` at score zero.
7. **`mutalisk`** (Python, Jun–Jul 2026). The only lane that ever called
   `gepa.optimize`. Retired after nine commits.
8. **DSE v2** (Effect v4, Jul 2026). Rebuilt for Apple FM, extracted as a
   package, and its only consumer deleted two weeks later.
9. **`bench/optimize/`** (Aug–Sep 2026). The Gym's GEPA lane. See below.

## DSE, which got furthest

"Declarative Self-Improving Effect" — described in its own code as *"DSPy,
but Effect TS"*. It was real and it reached production: typed signatures, a
structured prompt intermediate representation under canonical-JSON hashes,
immutable compiled artifacts, receipts, runtime budgets, canary bucketing,
promotion and rollback, and an overnight compile-canary-promote loop.

**What it did not have was the optimizer.** Its own audit:

> | Did it implement MIPROv2? | No. The code did not contain that algorithm. |
> | Did it implement GEPA? | No. The code did not contain that algorithm. |

What shipped was `instruction_grid.v1`, `fewshot_greedy_forward.v1`, and
`knobs_grid.v1` — deterministic grid search and greedy selection. DSE v2 says
so about itself in a code comment: *"There is no MIPRO, GEPA, COPRO, Pareto,
Bayesian scheduler, or generic module graph — those were never implemented
and stay out of scope."*

**And it did not die on merit.** From the 1,155-line git-history audit over
52,743 commits, the best artifact in this whole archaeology:

> - The first Rust line was removed while DSE remained.
> - The production applications were removed while DSE remained.
> - A Rust-only mandate then removed DSE.
> - The repository later removed the Rust alternative too.
> - **No removal commit cites a DSE test failure or quality regression.**
>
> Architecture ownership changed faster than either optimizer line could
> become durable.

That audit ends with a list of what a successor must fix. It is worth reading
as a specification, because `crates/gym` already enforces most of it — not by
coincidence, but because it is the same lesson arrived at twice:

> - The compiled ID must cover the complete artifact.
> - **A missing holdout must fail.**
> - **Train data must not silently become holdout data.**
> - **Optimizer names must match the actual algorithm.**
> - Each candidate must bind to an immutable dataset revision.
> - Cost and budget evidence must remain part of admission.

## The Gym's own GEPA lane, which never ran

Five commits, August to September 2026, deleted in the reshape. It staged
three text surfaces — system prompt, tool descriptions, catalog lines — as
content-digested artifacts extracted out of Rust string literals. That
staging commit's rationale is the sharpest sentence in the lane:

> That is fine for a human editing one sentence and wrong for everything the
> autoimprovement plan wants to do with it: **a lever cannot be diffed as an
> artifact, two cycles cannot be compared by digest, and an optimizer has
> nothing to mutate.**

Its objective put cost inside the score rather than beside it:
`acceptance − 0.005 × tokens/1e6 − 0.001 × wall_seconds/3600`. Its holdout
was structurally fenced — a holdout task appearing in a batch raised.

**It never ran, and as built it could not have worked.** The landing commit
says *"No live Harbor cycle ran"*, and the adapter's own docstring admits
that live evaluation of a mutated candidate would need an overlay or a
rebuild, so *"this packet scores the current tree (seed) live"*. Even with
`--live`, GEPA's proposed text would never have reached the agent under test.
**Every live trial would have re-scored the unmutated tree.**

The only result on record is a fixture dry-run asserting
`"disposition": "no_beat_incumbent"`.

## What GEPA actually is

From the upstream clone at 0.1.4. Worth stating plainly because three of the
nine attempts described something else by the same name.

A **candidate is `dict[str, str]`** — component name to text, nothing more —
and the component set is frozen by the seed, so a proposer cannot add or
rename a key. A **metric is a float per example**, higher better. Minibatch
acceptance sums; Pareto and reporting average.

The **reflective half** renders every key of a reflective dataset into an
`<side_info>` slot of a meta-prompt and asks a separate LM for new
instructions inside fenced blocks. Its extractor takes everything between the
first and last fence — **and if there is no matched pair, returns the reply
verbatim as your new component text.** A chatty reflection model silently
installs prose as your prompt.

The **Pareto half** is not a classical frontier. It keeps, per validation
instance, the candidates tied for best on that instance, then samples so that
selection probability is proportional to the number of instances a candidate
is tied-best on.

Its documented failure modes are the ones that matter here:

> Without a separate valset, GEPA will tend to overfit the training data.

> The optimizer over-indexes on surface patterns present in the training
> minibatch but not generalizable.

> GEPA's reflection LM can occasionally drift the task definition — for
> example, changing a 1–5 rating scale to 1–3.

And every published number in that repository is single-run: no variance, no
seeds, no error bars.

## Three lessons, in their own words

**Things died from architecture churn, not evidence.** No removal commit
anywhere cites a quality regression. The pattern repeats five times, and the
commit that took the last lane has an empty body.

**Naming honesty needed enforcing, repeatedly.** Three separate commits exist
purely to stop a projection being described as an optimizer:

> The honest label for this lane is `Pylon-distributed GEPA rollout
> optimization`. It is not distributed neural-network training.

> Live "GEPA" is a bounded status-projection loop, not real optimization.
> [...] we do not yet run a real GEPA optimization loop in production.

**Code transfers; prompts do not.** From the harness-optimization audit of an
external result that moved 63.4% to 80.1% on 1,251 tasks with zero weight
changes:

> Code transferred; prompts did not. The same harness lifted a smaller
> same-family model ~14.4 points; a different family gained only +0.4. Five
> of the top six frontier harnesses were deterministic code, not prompt
> changes.

A 2026-08-26 analysis listed the risk *"a third dead optimizer stack"* with
the control *"do not reimplement; consume upstream."* The lane it recommended
became the third dead optimizer stack seventeen days later.

## What this means for the Gym

**The machinery has never been what was missing.** Nine attempts built
signatures, artifacts, receipts, budgets, canaries, frontiers, merge
proposers, settlement gates, and lease proofs. What is missing, every time,
is a run that finishes and a number that survives a floor.

**We now have the floor.** `docs/decision-models/lev/measurements/` records
0.056 accuracy at two sigma for a two-door comparison on our suite. That is
the part nobody here has ever had, and it is what turns the tenth attempt
from a tenth attempt into an experiment.

**The budget is no longer the obstacle either.** Every previous lane died
partly on rollout cost — minutes to hours per trial in a container. Here a
rollout is one door call. The routing family's development partition is 40
items; GEPA's own guidance of 15 to 30 times the validation set puts a full
run at **600 to 1,200 calls**. That is minutes and pennies.

**But the floor cuts both ways, and this is the thing to face before
starting.** On a 40-item development partition, one item is 0.025. The
optimizer's own acceptance signal therefore moves in steps that are
themselves inside the noise — which is precisely the criticism
[`decision-models/research/2026-09-19-question-text-optimization.md`](decision-models/research/2026-09-19-question-text-optimization.md)
levels at an external tool's five-row minibatch. Having the floor does not
make the experiment easy. It makes it honest, and it may make it come back
negative, which on this evidence is the likely outcome.

## One structural obstacle, invisible until you try

In `crates/gym/src/suite.rs`, `question` is a field of `Item`, and
`compute_digest` hashes `self.items`. **Rewording a question changes the
suite digest.** Every row pins `suite_digest`, by design, so that changing a
rule produces new rules rather than new history.

Under the current contract, a question-text variant is therefore **not a
candidate door against a pinned suite — it is a different suite**, and the
store will correctly refuse to compare across them.

That seam has to be designed before any of this runs: either question text
moves out of the digested item into a separately-digested question set that a
run pins alongside the suite, or the store needs an explicit notion of
same-items-different-question-text. A day of work, not a rebuild, and worth
knowing before rather than after.

## What to reuse rather than rebuild

Nothing runs. `git grep -il 'gepa\|dspy'` on `main` returns four transcripts
and two documents. Zero code. Every implementation sits in a deleted tree
with a dead toolchain.

Three artifacts are worth lifting:

1. **The DSE audit's correction list** — a specification for acceptance
   discipline written by someone who had just read every mistake.
2. **The candidate schema** `openagents.coder_candidate.v1`, whose design
   principle is that *a reflection and a mutation are the same object*: a
   human reword and an optimizer mutation land as one artifact type, with
   lineage saying which.
3. **The staging pattern**: text as a digested artifact plus a build step
   plus a check that fails when artifact and build disagree, so a text change
   appears in a run row instead of reading as noise.

## The disposition

Do not start a tenth optimizer. **Run the experiment the ninth one was built
for and never performed**, at the smallest scale that can clear the floor:
the routing family's question text — currently one six-word sentence and
three criteria strings, never measured — optimized on the development
partition, scored on calibration, on all three doors, against 0.056, on the
full metric panel.

If it clears, it is the first text-optimization win in this workspace's
history. If it does not, that is the tenth consecutive negative result, and
it should be written here so the eleventh proposal has to argue with it.
