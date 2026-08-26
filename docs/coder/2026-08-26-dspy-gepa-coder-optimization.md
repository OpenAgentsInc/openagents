# DSPy, GEPA, and the coder: from DSE to an optimizer over the Gym

Date: 2026-08-26. Status: analysis, feeding the autoimprovement plan
(`docs/coder/autoimprove.md`) and the tracker. Companions:
`docs/coder/runbook.md`, `docs/coder/best-practices.md`. Prior art reviewed:
the DSPy-in-Effect history audit
(`docs/dspy/2026-07-20-dspy-in-effect-git-history-audit.md`), the
Python-versus-Effect decision audit
(`docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`,
historical), the evolve-the-harness audit
(`docs/research/2026-07-04-harness-optimization-evolve-the-harness-audit.md`,
historical), and in the openagents.com repo the plugin model assessment
(`docs/2026-08-24-triage-and-plugin-model-assessment.md`), the Harbor plan
(`docs/2026-08-24-harbor-terminal-bench-plan.md`), and the registry network
strategy (`docs/2026-08-24-registry-network-strategy.md`). Upstream clones:
`../projects/repos/dspy`, `../projects/repos/gepa`, `../projects/repos/rlm`.

## 1. What we already built, twice

The history is longer than it looks from the current tree, and it settles
several questions the coder lane would otherwise re-litigate.

**DSE (February 2026).** OpenAgents built a real DSPy-in-Effect:
`@openagentsinc/dse`, "Declarative Self-Improving Effect," 53 files and
9,678 lines in the terminal package plus production wiring in the web app —
typed signatures on Effect Schema, a structured Prompt IR under stable
hashes, an evaluation system with datasets, metrics, and caching, a bounded
compiler, immutable policy artifacts, receipts, budgets, a canary gate, and
one recorded production promotion. Two findings from the history audit
matter now:

- **It never contained a real optimizer.** The compiler was deterministic
  grid search, greedy few-shot selection, and rule-based refinement — not
  MIPROv2, not GEPA. The hard part of DSPy was the part DSE did not have.
- **It was not removed on merit.** A whole-app deletion took the serving
  side, then a repository-wide Rust-only mandate deleted the package
  (pruned in `d7f53fccc`, archived in the backroom repo). The audit's words:
  the removal sequence shows unstable architecture ownership, not a
  comparative rejection of DSE quality.

**The hybrid decision (June 2026, historical but load-bearing).** The
2026-06-28 audit answered "reimplement or adopt" with a tier rule this
document keeps: **use upstream Python DSPy and GEPA as the offline
optimization tier that produces candidate artifacts and evidence; keep the
online serving and governance path native, as the authority that selects,
gates, and admits.** Reimplementing GEPA's Pareto-evolutionary search in
our own stack was judged large, low-leverage duplication of a co-developed,
paper-backed upstream (`dspy` depends on `gepa[dspy]`) — and DSE's missing
optimizer is the empirical support for that judgement.

**The external validation (July 2026).** The evolve-the-harness audit
recorded a published result: a frozen open model moved 63.4% → 80.1% on a
1,251-task legal benchmark purely by letting an automated loop rewrite the
harness. The loop mechanics it recommended adopting are exactly the shape
the coder loop needs: one mechanism per candidate with copy-and-adapt
diffs, a token-cost term inside the objective, a noise-floor acceptance
gate stated with its trial count, code mechanisms treated as first-class
candidates (they transferred across model families; tuned prompts did
not), and a judge-gaming caution — a harness evolved against a grader can
overfit the grader.

**The lineage in current product docs.** The openagents.com plugin model
assessment names the plugin manifest's typed interface "a signature in the
DSE sense" and adopts the old laws wholesale: sandboxed modules, typed and
validated boundaries, content-addressed immutable artifacts, no keyword
routing, and *an optimizer output is a candidate, never a deployment*. The
Harbor plan reserves RewardKit rollouts for "GEPA-style" optimization under
the same law, and the registry strategy places the eventual selection loop
"bounded and offline in the DSE/GEPA style." The direction was decided
three times; what is missing is the running loop.

## 2. The plugin system is the DSPy layer we kept

The claim "our plugin system lends itself to DSPy" is precise, not
analogical. The correspondence:

| DSPy concept | Coder counterpart | State |
| --- | --- | --- |
| Signature (typed I/O contract) | Plugin manifest input/output schemas, host-validated | Shipped |
| Module (a callable unit) | WASM plugin, content-addressed by digest | Shipped, twelve in-tree |
| Program (composed modules) | The coder harness: system prompt + tools + catalog + plugins per turn | Shipped |
| Demos / instructions (optimizable text) | System prompt, tool descriptions, the twelve catalog lines, knowledge-base stances | Shipped, but **compiled into code** |
| Retrieval rail | `knowledge-base` plugin: corpus queried every turn, attached as a bracketed note | Shipped, with a governed promotion path |
| Metric | Harbor verifier + ATIF token metrics per trial | Shipped (the Gym) |
| Trace | ATIF trajectory per trial | Shipped |
| Optimizer / teleprompter | — | **Missing** (as it was in DSE) |

Two entries deserve emphasis:

- **The knowledge base is an optimizable parameter surface that reaches
  every turn without a code change.** A stance edit plus a rebuild changes
  what the harness attaches to matching turns; the corpus is already
  reviewed-like-content with stable ids and a promotion path from system
  memory. This is the cleanest channel an optimizer could write
  candidates into — and the one with governance already in place.
- **The catalog lines are the selection policy DSPy would tune.** Twelve
  slots, one sentence each, contested (best practice P3). Which plugin a
  model reaches for on which task class is a function of that text today;
  the registry strategy's "selection loop" is this surface under
  optimization.

What we do not have — and did not have in DSE either — is the optimizer.
The rest of the machine is more complete than DSE's ever was, because the
metric is now an external verifier over a receipted store rather than an
in-house eval harness.

## 3. GEPA fits the Gym

GEPA's contract is one call: `optimize(seed_candidate, trainset, valset,
task_lm, reflection_lm, max_metric_calls)` over any textual parameter,
reflecting on full execution traces, keeping a Pareto pool of candidates.
Every argument has a concrete referent here:

| GEPA argument | Gym referent |
| --- | --- |
| `seed_candidate` | The current text surfaces: system prompt, tool descriptions, catalog lines, stances |
| `trainset` (dev) | `tb2-quick`, plus `owned-closed-issues` once its environments exist |
| `valset` (holdout) | `tb2-cross-section` — run sparingly, exactly as the runbook already directs |
| metric | Verifier acceptance minus a token-cost term, read from the trial's `result.json` and ATIF metrics |
| traces for reflection | ATIF trajectories — GEPA reflects over full execution traces, which every trial already leaves |
| `reflection_lm` | The cycle review (runbook §6); OpenAgentsInc/openagents#121 automates precisely this role |
| Pareto pool + lineage | `bench-results/` rows (hash-chained) + `docs/coder/reviews/` |

Read in this frame, **the autoimprovement loop just seeded is manual GEPA
with a population of one**: a cycle is a single mutation, the compare
against baseline is the acceptance test, a refutation is a discarded
candidate, and the ledger is the surviving pool. The runbook's disciplines
are the same ones the evolve-the-harness audit extracted from the
published loop — one lever per cycle is copy-and-adapt; M-series practices
are the acceptance gate. GEPA parallelizes the loop and adds the Pareto
memory; it does not change its laws.

The budget shapes the split. GEPA's own numbers are 100–500 metric calls
per optimization; a cross-section run is 12 tasks at minutes-to-hours each
under emulation. So the optimizer screens on the cheap sets (`tb2-quick`,
owned tasks — minutes per rollout) and only surviving candidates spend a
cross-section run, which doubles as the judge-gaming control: a candidate
that gamed the dev verifier fails the holdout it never trained against.
The dev set's narrowness is a real constraint (the audit's 24-task caveat
applies to our 2+12 harder), which is one more reason the
`owned-closed-issues` suite — real tracker issues with known accepted
outcomes — is worth standing up as trainset material.

## 4. The tier boundary, restated for the coder

The June rule survives contact with the current architecture cleanly,
because the offline tier already speaks Python: Harbor is Python, the
adapter is Python, and `bench/` is where both live. Concretely:

- **Offline (Python, `bench/optimize/`):** upstream `gepa` wrapped around
  the existing suite runner as its metric function. Output is a candidate
  artifact — a diff over the staged text surfaces, its evidence rows, its
  lineage, and a transfer label naming the model family it was evolved
  against (prompt-class candidates are family-specific; the audit's
  transfer finding). Nothing in this tier lands anything.
- **Online (the coder, unchanged):** a candidate becomes a change the way
  every other lever does — a fresh worktree, a landing commit stating the
  measured delta, the review, the ledger. The standing law holds: **an
  optimizer output is a candidate, never a deployment.**
- **The seam that makes it possible:** the text surfaces must be staged as
  data the optimizer can diff and the build can verify, rather than string
  literals inside `crates/coder-lite` and `packages/openagents-cli`. The
  knowledge base already has this shape (corpus file → build → digest);
  the system prompt, tool descriptions, and catalog lines need the same
  treatment. This staging is also what lets a human cycle (#119) and an
  optimizer cycle produce identical artifact shapes.

What we deliberately do not do, on the DSE evidence: reimplement the
optimizer in-house, in Rust or TypeScript or Effect. DSE died with a
grid-search stand-in where GEPA should have been; the lesson is to consume
the upstream that now exists, at the tier where Python is already native.
RLM stays out of scope for the coder lane entirely, and no DSPy program
layer enters the product — the coder harness is the program.

## 5. What to build

Two new tracker issues, sequenced against the existing loop set
(#116–#121):

1. **Stage the optimizable text surfaces as artifacts**
   (OpenAgentsInc/openagents#122). Extract system prompt, tool
   descriptions, and catalog lines into versioned data with digests,
   consumed by both CLIs; define the candidate-diff format. Prerequisite
   for the optimizer; makes #119's manual levers diffable artifacts too.
2. **A GEPA lane over the Gym** (OpenAgentsInc/openagents#123).
   `bench/optimize/` wrapping upstream `gepa` around the suite runner:
   dev-set screening, holdout confirmation, candidate artifacts with
   evidence, lineage, cost-term objective, and transfer labels. Waits on
   the baselines (#118) and the staging (#122); the review command (#121)
   is its reflection seam.

And three relations to the existing set: #119's manual cycles are the seed
candidates and the noise-floor calibration for the optimizer; #120's
plugin A/B rows are the per-module evidence the selection-policy surface
will eventually train on; #121's typed proposals are GEPA reflections and
should share the candidate schema from #122 so a review proposal and an
optimizer mutation are the same object.

## 6. Risks and their controls

| Risk | Control |
| --- | --- |
| Overfitting the verifier (judge-gaming) | Holdout suite the optimizer never screens on; suites pinned by content; the deliberately-false-candidate discipline when a judge is an LLM |
| Prompt candidates silently family-specific | Transfer label required on every candidate; re-evaluate when the target model differs from the evolved-against family |
| Optimizer buys score with spend | Token-cost term inside the objective, not alongside it (ledger M2 already refuses laundered cost) |
| Rollout cost explosion | Screen on cheap sets, spend cross-section runs only on survivors; `max_metric_calls` is a hard argument, treat it as a budget envelope |
| Candidate auto-landing by drift | The standing law is a ledger entry with detection (review of any change that lands optimizer output without its evidence rows) |
| A third dead optimizer stack | Do not reimplement; consume upstream at the Python tier where Harbor already lives |
