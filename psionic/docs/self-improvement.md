# Measure, then improve: the Gym, self-improvement and prompt optimization

Planning note, 2026-10-29, for Part XXIV of the [book plan](../book/PLAN.md).
It condenses pre-reset work, readable with `git show 0b401561a0^:<path>`:

- **The Gym:** `crates/gym`, `docs/gym/README.md`, `docs/text-optimization.md`.
- **Self-improvement:** `docs/product/self-improving-codebases.md`, the
  self-improving-codebases audit and the training-system audit under
  `docs/audits/2026-10-10-*`.
- **Coder One's studies:** `crates/coder-one/src/study/`, plus
  `docs/optimization/` and `nips/openagents/NIP-OPT.md`.
- **Transcripts:** 237, 242, 243, 271 and 287 in `docs/transcripts/`.

The numbers are historical, not Psionic results.

## The Gym: the part that held up

About 110,000 lines of Rust with gates, suites and result files. It came out of
three record-keeping faults:

- items silently dropped from a denominator;
- calibration maps that could not say which model they were fitted for;
- a gate and a suite changed in one commit.

Its answers to those faults:

- **Three digests per run.** Items with partitions, the question set, and the
  gate each carry a digest, so rewording a question is a candidate against the
  same items, not a new suite.
- **A locked partition read once.**
- **An append-only store** with a receipt chain.
- **Gates that carry their own digest and can say "unverifiable".**
  - `decision-v1`: a gain of at least two standard errors.
  - `probability-v2`: ranks log loss and confident errors above calibration
    error and accuracy.
  - `ext-eval-v2`: cost and time can block a result but never earn one.

Measuring the noise floor between two identical doors (0.056 accuracy at 2σ)
retired two published findings. A partition already at 0.975 cannot show a
gain, so headroom is measured first. That becomes chapters 178–180.

## Self-improvement: designed, partly built, never closed

The product spec defined "recursive" plainly: the system's outputs become
training data for its own decision layer, and an agent never rewrites its
goals. Its design law, taken from the exact-computation work: **train only the
interface** (rankers, decision heads, calibration maps), and keep indexes,
compilers, tests and replay exact.

What it produced:

- **A pre-registered briefed-agent study** cut cost per accepted change by
  53.9% (95% CI 44.3–60.4%) at equal success. Equal-or-better quality was not
  shown.
- **A nightly refit** adopted a threshold only with at least 40 fit rows,
  20 held-out rows and a gain of 0.02 or more.
- **A per-task oracle** passed 24/24 tasks for $0.07 against $1.09 for a
  frontier agent alone. Choosing a configuration per task was worth more than
  any single change.

What the audits found:

- **"OpenAgents does not have a working training system."** About 1–3% of the
  training crate computed gradients.
- **A retraining script wrote the production model with no gate,** training on
  its evaluation cases.
- **All 41 replayed traces came from groups reserved for calibration or
  development.** Hence the exposure ledger in chapter 186.
- **A LoRA adapter raised accuracy from 0.77 to 0.90 but worsened log loss from
  1.952 to 2.323,** and did not transfer to another suite.

Coder One added about 48 hours of policy versions and oracles that won on one
in-sample task, and the owner called it overcomplicated. The default stays the
simplest loop, and every addition needs a measured gain on tasks it was not
built from.

## DSPy and GEPA: ten attempts, no measured win

| Attempt | Language | What happened |
| --- | --- | --- |
| A hint hill-climber | Rust | 202 runs, no passes |
| A Laravel DSPy | PHP | Superseded, no results |
| `crates/dsrs` | Rust, 42,408 lines, including a faithful 534-line GEPA port | Never reachable from its entry point |
| A TypeScript optimizer | TypeScript | Reached production with only grid and greedy search |
| `psionic-optimize` | Rust | Had no language model in it |
| A distributed GEPA campaign | Four repositories | Zero successful rollouts |
| A Python GEPA lane | Python | Retired |
| A second Python GEPA lane | Python | Would have re-scored the unchanged tree |

The one run that executed (400 hosted Jev calls, three hand-written rewordings
of a routing question) gained +0.025 against the 0.056 noise floor. That is
unverifiable, and calibration got worse; the development split overstated the
effect five times, and the option strings carried the effect, not the
instructions.

What worked was fitting settings on recorded answers (ReAnchor, ported to
Rust): zero model calls, acceptance only when strictly better on the fit set
and on grouped cross-validation. It raised stall recall from 35% to 53%,
although no policy changed. Another lesson: code transfers between models and
prompts do not (a deterministic harness gained 63.4 → 80.1% on one family and
+0.4 on another).

GEPA itself, as written: candidates are a fixed set of text fields; a
reflection reply is extracted between fences and, if unmatched, installed
verbatim; "Pareto" means sampling candidates by how many validation items they
are tied-best on. Its known risks are overfitting without a separate
validation set and drift of the task definition. NIP-OPT already states the
discipline: student, reflector and grader are separate; the proposer sees
development data only; candidates may not read answers, change the runner,
widen tool grants or lower thresholds.

## How it fits Psionic

- **The Gym is library code.** Chapters 12, 13, 85, 86, 154, 156 and 174 report
  through one comparison rule (178–180), whose gates are `#authority` Coax with
  zero surviving mutants.
- **Optimization runs on Psionic's own models.** With Elephant at about 237 ms a
  decision on the GPU, GEPA's typical 15–30 times the validation set (600–1,200
  calls for 40 items) costs minutes. The reflector is a separately pinned
  Psionic model, gpt-oss with constrained output, never the grader.
- **Settings fitting comes first** (182). It needs no model calls, and it is the
  only kind of optimization that produced a result before.
- **Model judges are components** (185), measured against real outcomes and
  never allowed to grade their own work.
- **Outcomes close the loop** (186). Part XXII's outcome joins feed calibration
  refits, settings fitting and post-training rows, each through its own gate.
  Verified traces from Part XXIII are admitted only with replay verdicts.
- **Weights change last.** Improve settings, prompts, routers and harness
  configuration first; update weights only when both decision and probability
  gates pass.

## What the owner said

- 271: continual learning "doesn't need to take place so much at the level of
  the weights … the systems around it iterating … will be enough."
- 242: independently optimizable components, each like a DSPy signature, that
  "you can run GEPA" on.
- 243: "this thing's gonna all start improving itself."
- 287: "We need better measurements. We need a more composable process."
- And on record: port useful algorithms natively rather than adopt Python
  libraries such as DSPy.
