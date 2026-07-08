# "Don't Train the Model, Evolve the Harness" — Audit and Adoption Map

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-07-04
Status: research audit in the standard dated-audit lane. No promise state
flips, no public copy changes. Source: Joel Niklaus, "Don't Train the Model,
Evolve the Harness" (Hugging Face Space `joelniklaus/harness-optimization`,
read 2026-07-04). Companion external quotes and our own lanes referenced
throughout.

## 1. What the post shows (summary, exact numbers as published)

A frozen open model (DeepSeek-V4-Pro, zero weight changes) on **Harvey's
Legal Agent Benchmark (LAB)** — 1,251 realistic legal tasks graded by an
LLM judge (Sonnet 4.6) against detailed rubrics — improved from **63.4% →
80.1%** pooled criterion pass rate (all-pass 0% → 5.0%) purely by letting an
automated loop rewrite the **harness**: the runtime wrapper that feeds
context, runs tool calls, lands deliverables, and decides when a run ends.
The optimized open model landed between Sonnet 4.6 and Opus 4.6 on the
headline metric at roughly 7× lower cost per task.

The loop ("how the loop works"):

1. **Proposer** (Opus 4.8): reads execution histories + the current best
   harness, hypothesizes **exactly one mechanism**, writes it into code,
   documents it in a `pending_eval.json` manifest.
2. **Evaluation**: candidates run on a 24-task dev set, three trials each;
   blended score `pooled_criterion_rate + 0.5×all_pass_rate −
   0.005×tokens_per_million` (a token-cost penalty is in the objective).
3. **Acceptance**: promote only if the candidate beats the incumbent by
   ≥1 point — just above the three-trial noise floor. Strict
   **copy-and-adapt**: each candidate copies the best harness byte-for-byte
   plus one change, so accepted mechanisms compound.

What the loop discovered:

- **Deliverable landing was the single biggest gain, not intelligence.**
  The model did the legal analysis correctly, then saved it under the wrong
  filename, into a scratch folder, or not at all — the judge only grades
  files at the exact requested path. A deterministic post-solve step that
  reassembles chunked writes and lands the file where the judge expects it
  beat every prompt change, at zero extra model tokens. The headline "0%"
  was measuring the harness, not the legal reasoning.
- **Code transferred; prompts did not.** The same harness lifted a smaller
  same-family model ~14.4 points; a different family (Nemotron-3 Ultra)
  gained only +0.4 — robustness code (e.g. `toolcall_json_repair`)
  transferred, tuned prompt playbooks backfired cross-family. Five of the
  top six frontier harnesses were deterministic code, not prompt changes.
- **The harness dominates the score.** Same frozen model, same judge, same
  tasks: five harnesses scored **3.5% (mini-swe-agent) to 80.1%
  (optimized)**, with general-purpose agents far below the domain baseline
  (Goose 23.2%, Pi 45.4%).
- **Gains flatten at real capability.** Dev plateaued ~83.3%; the remaining
  misses (quantitative depth, citation precision on a tax §382 task) look
  like genuine model limits. The wrapper runs out of tricks; then and only
  then do weights have to carry the work.

## 2. Why this matters here (the one-line version)

**This is external, benchmarked validation of three bets we already made**
— Mutalisk (offline optimizer producing evidence-gated candidates), the QA
Swarm/chill-evals thesis (a score measures model + harness together), and
the Reactor improvement ladder (cheapest honest lever first) — plus one
correction: harness evolution belongs *ahead of* fine-tuning on every
improvement ladder we sell.

## 3. Adoption map, lane by lane

### 3.1 Mutalisk (the DSPy/GEPA offline-optimization lane) — this IS the loop

The post's architecture is field-for-field the Mutalisk contract: an
offline proposer/evaluator emitting **candidate artifacts + evidence**,
with promotion decided elsewhere. Their loop details worth adopting into
Mutalisk's runner as configuration, not new architecture:

- **One-mechanism-per-iteration, copy-and-adapt** candidate generation
  (GEPA-style compounding with a strict diff discipline — makes every
  accepted gain attributable and revertible).
- **A cost term in the objective** (`−0.005×tokens_per_million`): our
  blended metrics should always carry the token/cost penalty so the
  optimizer cannot buy score with spend — consistent with exact-accounting
  culture.
- **Noise-floor acceptance** (≥1 point over incumbent after N-trial
  averaging) as the default promotion gate shape in candidate manifests
  (`metricValueBps` deltas should state the trial count and floor).
- **Optimize code mechanisms, not just prompts.** Mutalisk today frames
  candidates as prompts/modules/policies; the post's evidence says the
  highest-value candidate class is **deterministic harness code**
  (deliverable landing, tool-call repair, loop-break). The candidate
  artifact schema already carries generic module refs; the runner should
  treat harness-code mechanisms as first-class candidates with the same
  evidence gates. Admission authority stays in the Effect/product side
  (Khala/Artanis), unchanged.
- **Transfer labeling:** candidate manifests should record the model family
  they were evolved against, because prompt-class candidates are now known
  to be family-specific while code-class candidates are portable — the
  admission gate can require re-evaluation when the target family differs.

### 3.2 Harvey LAB / legal lane — run this loop on our own runner

The benchmark in the post is the same Harvey LAB we hold as a reference
lane (`projects/repos/harvey-labs`), with owned execution belonging to
Psionic's Rust legal benchmark runner per the standing workspace contract.
Concrete follow-ons:

- Treat the published mechanism families (deliverable landing, matter
  fidelity, tool-call repair, loop-break) as the **seed mechanism set** for
  our own legal-lane harness, implemented natively rather than copied.
- Run the Mutalisk loop against our LAB runner with an open model served on
  the Hydralisk lane — the exact "frozen open model + evolved harness"
  recipe, on infrastructure we already own end-to-end.
- The deliverable-landing lesson generalizes to every fulfillment pipeline
  in ROADMAP_BIZ (BF-4 document products): **the deliverable lands at the
  contracted path/name as a deterministic post-step with a receipt**, never
  trusted to the model. That is a behavior-contract-shaped invariant and
  should be registered as one wherever a deliverable pipeline ships.

### 3.3 QA Swarm / chill-evals — the variance claim is our sales pitch

"Same model, same judge, same tasks, 3.5%–80.1% depending on harness" is
the strongest third-party articulation yet of what QS8 chill-evals sells:
variant comparison across harness/MCP/config axes with honest verdicts.
Use the post (cited, public) as category proof in QA Swarm material the
same way Friedberg/Mistral are used for Reactor. Corollary for our own
benchmark claims: never publish a model capability number without naming
the harness it was measured under — a score is a (model, harness) pair;
this belongs in the eval-receipt shape (RX-4 evalRefs should carry a
`harnessRef`).

### 3.4 Reactor — harness evolution becomes rung zero of the value ladder

The Reactor plan sold improvement as flywheel (fine-tune on interaction
data) + distill-to-fit. The post reorders that honestly: **evolve the
harness first** — zero weight changes, deterministic code, gains that
survive model swaps. Updated ladder (now reflected in the Reactor doc §5):

1. **Evolve the harness** (Mutalisk loop on the customer's tasks; code
   mechanisms; cheapest, fastest, transferable).
2. **Distill-to-fit** (shrink to the observed input distribution).
3. **Fine-tune/flywheel** (their data, their weights — only once the
   harness has flattened, per the post's own limit finding).

Two extra Reactor-specific consequences:

- **Harness assets are provenance-policy-robust.** Because code mechanisms
  transfer across model families, a customer who tightens their
  `reactor.model_policy.v1` (say, to US-origin-only) keeps most of the
  harness-evolution investment even as the model set changes — a selling
  point no weights-centric vendor can make. Prompt-class assets must be
  re-evaluated per family (the §3.1 transfer label).
- **The cost math is the pitch.** "Frozen open model + evolved harness ≈
  frontier-model quality on the customer's domain benchmark at a fraction
  of per-task cost" is the Reactor Assessment's quantitative story — always
  presented as the published external finding plus our own receipts once
  RX-4/RX-11 produce them, never as a pre-receipt promise.

## 4. Cautions (so we adopt the lesson, not the hype)

- The gains flatten; the post says so. Harness evolution is rung zero, not
  the whole ladder — never sell it as unbounded.
- Dev-set narrowness (24 tasks) constrained what their loop could find;
  our loops should state dev-set coverage in candidate evidence.
- Judge-gaming risk: a harness evolved against a judge can overfit the
  judge. Our acceptance evidence should include held-out checks and the
  chill-evals honest-verdict discipline (the deliberately-false candidate
  pattern from QS7).
- All §1 numbers are the post's, on their setup; none are our claims. Any
  OpenAgents copy citing them must attribute, and our own numbers wait for
  our own receipts.

## 5. Cross-references

- Mutalisk repo (`mutalisk/` — candidate contract, GD-3 admission seam).
- `docs/gepa/2026-06-30-gepa-usage-and-fleet-delegation-optimization-loop.md`.
- `docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md`
  (§5 ladder updated alongside this audit; RX-11 issue #8279).
- `docs/fable/2026-07-02-qa-swarm-product-plan.md` (QS8 chill-evals).
- `docs/fable/2026-07-03-behavior-contracts-and-customer-invariants.md`
  (deliverable-landing as a contract class).
- Harvey lane: `projects/repos/harvey-labs` (reference) + Psionic's Rust
  LAB runner (owned execution, per the workspace agent contract).
