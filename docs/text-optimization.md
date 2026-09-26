# Text optimization here: nine programs, one experiment, no wins

Status: historical audit through 2026-09-19. The negative findings below apply
to the reviewed attempts and retained evidence; this document is not a live
scoreboard for later experiments. Use the [optimization index](optimization/README.md)
and [master roadmap](roadmap.md) for current contracts and delivery decisions.

A history of DSPy, GEPA, and prompt optimization across `openagents`,
`psionic`, `coder`, and `backroom`, what it means for the Gym, and the one
run that finally happened.

Read this before proposing an eleventh attempt.

## The finding

**Nine separate programs for optimizing text or program parameters have been
built in this workspace since December 2025. Not one has produced a text
change that beat a baseline on a measurement anyone would accept today.**

The tenth attempt is not a program. It is the experiment the ninth was built
for, run on 2026-09-19 against hosted Jev, and it is negative:
[The tenth attempt, which ran](#the-tenth-attempt-which-ran), below.

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
| The routing reword, Sep 2026, 400 hosted calls | +0.125 where it was written, **+0.025 where it was not**, against a 0.056 floor, with ECE, Brier, and log loss all worse |

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

**We now have the floor.**
[`lev/measurements/2026-09-19-seed-variance.md`](lev/measurements/2026-09-19-seed-variance.md)
records 0.056 accuracy at two sigma for a two-door comparison on our suite. That is
the part nobody here has ever had, and it is what turns the tenth attempt
from a tenth attempt into an experiment.

**The seam is no longer the obstacle.** See
[One structural obstacle](#one-structural-obstacle-invisible-until-you-try--now-closed),
below: a reworded question is a candidate against the same items as of
openagents#9386, and no existing digest moved to make it one.

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

**That is what happened, and the scoring partition turned out to be tighter
still.** The routing calibration partition scores 0.975 before anything is
reworded, so its whole headroom is one item — 0.025, inside the floor before
a candidate is written. A suite can be too easy to measure a lever on, and
nothing about that is visible until somebody scores the baseline there.

## One structural obstacle, invisible until you try — now closed

In `crates/gym/src/suite.rs`, `question` was a field of `Item`, and
`compute_digest` hashes `self.items`. **Rewording a question changed the
suite digest.** Every row pins `suite_digest`, by design, so that changing a
rule produces new rules rather than new history.

Under that contract, a question-text variant was **not a candidate door
against a pinned suite — it was a different suite**, and the store correctly
refused to compare across them. That is right, and it blocked the one
experiment nine optimizer programs were built for.

openagents#9386 closed the seam the first of the two ways this section
described: the question text moved out of the digested item into a
separately-digested question set, and a run pins both.

**A run now pins three digests.** The suite says what was asked about and
what the answer is. The question set says how it was asked. The gate says
what bar judged it. They are three files with three digests, and each row
carries all three.

- `crates/gym/src/questions.rs` holds a `QuestionSet`: one question per
  family, with a digest over the questions and nothing else. Renaming a set
  leaves its digest alone, for the same reason renaming a suite leaves the
  suite's alone.
- The sets live in `crates/gym/questions/`, as the gates live in
  `crates/gym/gates/`. `support-v2-three-way-v1.json` is the text
  `support-v2-three-way` already served, under a name a row can pin.
- A suite manifest names its set in a `questions` field, outside the digest,
  exactly as it names its gate. A row carries `question_set` and
  `question_digest` beside `gate_id` and `gate_digest`.
- `gym::store::admit_comparison` says what two sets of rows are. Same items
  and same text is a door comparison. Same items and different text is a
  **question-text comparison**. Different items is not a comparison, and it
  is still refused — a test pins that, because separating the text out must
  not buy a changed label past the rule the record exists to hold.

**No digest moved.** `support-v2-three-way` is still
`54fbf4137c3de538f2dea07d47ca1ee835c09eb25aa26a320441679129f618f9` and
`support-v2` is still
`6877c24bf261d5bdcb0550824c20f017c7bb5095aac22dbf5f4c6ef47b789368`. The 196
items keep their inline question text, which is where their digest already
covers it; the committed set is the same text lifted out, and a test asserts
the two agree. A migration that silently reissued digests would have
invalidated the chain it was built to protect.

**What the experiment does now.** Fork
`crates/gym/questions/support-v2-three-way-v1.json` to a new id, reword the
`routing` entry, and run `gym eval --questions <id>`. The rows land in the
same store beside the baseline's, because the question digest is part of what
identifies a trial, and `gym compare` reads the two as a question-text
comparison over unchanged items.

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

## The disposition, which was carried out

The disposition was: do not start a tenth optimizer, and **run the experiment
the ninth one was built for and never performed** — the routing family's
question text, optimized on the development partition, scored on calibration,
against 0.056, on the full metric panel. It said that if the reword cleared
the floor it would be the first text-optimization win in this workspace's
history, and that if it did not, the tenth consecutive negative result should
be written here so the eleventh proposal has to argue with it.

It did not clear the floor. Here it is.

## The tenth attempt, which ran

2026-09-19, hosted Jev, 400 calls, a few minutes. The full record is
[`gym/measurements/2026-09-19-question-text-routing.md`](gym/measurements/2026-09-19-question-text-routing.md);
the rows are in `crates/gym/results/routing-question-text-*.jsonl`.

No optimizer was built. Three rewordings of the `routing` question were
written by hand against the development partition — one changing the three
criteria strings, one changing the instructions, one changing both — each
freezing the option names and the question type. The best of the three was
selected on development and committed before any calibration row existed, so
the choice is on the record rather than in a paragraph.

| Question set | Development, where it was written | Calibration, where it was not |
| --- | --- | --- |
| baseline | 0.875 | 0.975 |
| criteria reworded, **selected** | **1.000** | 1.000 |
| instructions reworded | 0.900 | 0.950 |
| both reworded | 0.925 | 0.875 |

**The selected candidate gains 0.025 accuracy on the held-out partition — one
item in forty, against a 0.056 floor.** `decision-v1` reads it
`unverifiable`. The other two candidates read `failed`. On development the
same text fixed every error the baseline made, a gain of 0.125; the
optimization set overstated the effect five times over.

**Accuracy was not the whole panel, and the rest of it got worse.** ECE rose
from 0.061 to 0.083, Brier from 0.019 to 0.031, log loss from 0.074 to 0.105,
and the mean probability on the true class fell from 0.939 to 0.917. The
Choice adapter's warning, repeating on a different lever: a change bought
accuracy by making the door less sure.

Five things the eleventh proposal has to argue with:

1. **A hand-written reword that fixes every error where it was written gains
   one item in forty where it was not.**
2. **The criteria carry the effect, not the instructions** — and rewording
   both is worse than rewording the criteria alone. That is the one finding
   here worth reusing, and three hand-written candidates found it for 160
   calls.
3. **The candidate that would have shipped under a development-only report
   loses 0.10 on calibration.** It gains 0.05 on development. That is
   `dataset=examples, valset=examples` measured on our own suite.
4. **A partition at 0.975 cannot express a measurable gain.** Its whole
   headroom is 0.025, inside the floor before the experiment begins, and the
   gate refuses to compute a standard error on one expected error. Measure a
   baseline's headroom before writing a candidate.
5. **Cost was never the obstacle.** 400 calls and a few minutes, on the tenth
   try, for the first text result this workspace has ever finished.

The two remaining doors are unmeasured. Kev and Lev were not asked, because
the machine was busy driving the on-device model and a contended number is
worth less than no number. So the transfer question — whether optimized
question text is a property of the task or of the model — is still open, and
it is now one command away rather than nine programs away.
