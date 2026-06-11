# The Tassadar + Psion Research Plan

> Status: unified research directive, 2026-06-11. This document is the
> chief-scientist synthesis of everything in this folder — the lane essay
> ([`README.md`](README.md)), the business thesis
> ([`work-that-proves-itself.md`](work-that-proves-itself.md)), the full
> audit ([`2026-06-10-tassadar-percepta-audit.md`](2026-06-10-tassadar-percepta-audit.md)),
> the construction notes and design plan, the assume-nothing introduction
> ([`2026-06-11-llm-computer-full-introduction.md`](2026-06-11-llm-computer-full-introduction.md)),
> and the external program analysis with lane commentary
> ([`2026-06-11-chatgpt-pro-analysis.md`](2026-06-11-chatgpt-pro-analysis.md))
> — together with psionic's `PSION_PROGRAM_MAP.md` and
> `PSION_EXECUTOR_PROGRAM.md` contracts. It is a directive: it tells every
> researcher and research agent in this program what we are building, in
> what order, against what evidence standard, and what would make us stop.
> The claim discipline of this folder binds this document like every
> other: where the plan asserts a fact, a source is named; where it
> asserts a belief, it is labeled a hypothesis; where it promises nothing,
> that is deliberate.

---

## 1. The Thesis

Every economy is taxed by something it cannot see past. Ours — machines
doing work for other machines, with strangers getting paid — is taxed by
verification. When you cost out real machine work, the dominant line is
never the compute; it is the checking. The whole apparatus this company
has shipped — validator lanes, effort ladders, challenge protocols,
receipt taxonomies, a promises registry that marks its own claims red
until evidence exists — is a machine for driving the cost of trust down
faster than the cost of work ([`work-that-proves-itself.md`](work-that-proves-itself.md), §I).

In March 2026, Percepta published a construction that closes the
verification gap to *zero* for one class of work: compile a program — do
not train it — into the weights of a completely standard transformer,
which then executes that program exactly, token by token, for millions of
steps, with a decode path that runs in logarithmic rather than linear
time per step. The execution trace is simultaneously the work product,
the audit log, and the proof: deterministic, append-only, digest-pinned,
verifiable by replay. A validator's verdict is a hash comparison. There is
no cheaper verification grade; there cannot be one.

We independently implemented the construction's pipeline shape in Rust,
in public, with tests (psionic #1098–#1114), ran a real workload through
the production network — dispatched to a real Pylon, replay-verified by a
separate device, paid over real Lightning
(`compute.tassadar_executor_poc.v1`, green 2026-06-10) — and the lane's
own differential harness caught two real bugs in our scheduler on its
first run. That last fact is the program's character reference: the
machinery built to catch lies caught ours within minutes of existing, and
that is precisely the property that makes exact computation economically
interesting. A system that cannot catch its own errors cannot price
trust. Ours can, and did ([`README.md`](README.md), "The part worth
reading twice").

The research program that follows exists to answer one compound question:

> **Can the exactness we can compile become something we can train, sell,
> and embed — and what is true at each point along the way?**

Everything below is an unpacking of that sentence.

## 2. The Two Lanes and the Spectrum

The program is two lanes with one substrate, and the distinction is
load-bearing enough that psionic enforces it as a documented naming rule
(`PSION_EXECUTOR_PROGRAM.md`):

- **Tassadar** is the compiled lane: exact, integer, hard-max,
  digest-pinned, *written* rather than learned. Its guarantees are
  proofs. Its claim boundary is equally hard: it proves transformers
  *can* compute exactly, not that trained ones do.
- **Psion** is the learned lane: the compact-decoder model family with
  governance, data, training, eval, serving, and rollback contracts
  (`PSION_PROGRAM_MAP.md`). Its guarantees are statistical, bounded, and
  route/refusal-shaped. It must never borrow Tassadar's exactness
  language — the program map says this in approximately those words.

Between them lies the spectrum where we believe the products live:
trained models with compiled exact cores for the operations that must
never be wrong — arithmetic, ledger updates, state machines, protocol
execution — and trained flexibility everywhere else. The executor is
differentiable (the trace is part of the forward pass; gradients
propagate *through the computation*), which makes exact computation a
trainable organ rather than an external prosthetic
([`work-that-proves-itself.md`](work-that-proves-itself.md), §V). No
other tool, plugin, or sandbox has that property.

The shared substrate underneath both lanes is non-negotiable and already
exists: one IR family, one artifact discipline (digest-pinned,
profile-versioned, typed refusals), one verification ladder with exact
replay as its bottom rung, and one publication gate (the promises
registry). Researchers do not get to choose a different evidence standard
per lane. The standard is the lane.

## 3. What We Know

These are settled, sourced, and citable without hedging:

1. **The construction works.** Percepta's released `transformer-vm`
   (cloned read-only at `projects/repos/transformer-vm`) analytically
   compiles a WASM interpreter (35-opcode subset, lowering for the rest)
   into a vanilla softmax-ReGLU transformer; their demos execute
   hundreds-of-thousands-token traces at ~30k tok/s on CPU. The 2D-head
   hull cache turns hard-max attention into a convex-hull support query:
   O(log n) against O(n), 31,037 tok/s against 702 in their published
   head-to-head ([introduction](2026-06-11-llm-computer-full-introduction.md), Parts IV–VII).
2. **We own an independent implementation of the pipeline shape.** ALM
   gate-graph IR with exact evaluator; four-phase scheduler with
   interval-coloring slot reuse and explicit stale-slot subtraction;
   parabolic-key geometric legs that *refuse near-misses* rather than
   interpolate; a Li Chao hull fast path with deterministic visit-count
   evidence; a Futamura specializer (v2, shared indicator subgraphs); a
   branch-capable interpreter for our twelve-opcode i32 window
   cross-validated against the production CPU runner; a portable f64
   numeric artifact executing in a checked 2⁵³ window; an exact
   trace-replay verifier that names the first divergent step; and a
   five-leg differential harness over 400 generated graphs
   ([`README.md`](README.md), "What We Built"; file map in the
   [introduction](2026-06-11-llm-computer-full-introduction.md), Part VIII).
3. **The market loop closed once, end to end, at smallest viable
   scale.** Real Pylon execution, separate-device replay verdict
   (including a tampered digest correctly Rejected), one paid Lightning
   closeout. Subsequently, an autonomous administrator dispatched and
   accepted several such workloads with no human in the loop — a claim
   currently operator-attested only, because the public projections for
   that run are stale and its evidence refs do not resolve publicly
   (openagents #4745, #4746). We treat our own announcement with the
   same skepticism we ask of others, and so should you.
4. **Verification economics are real and measured in our own ledger.**
   The platform's forum economy paid out settled Bitcoin this week for
   adversarial verification of our claims — and the findings (frozen
   projections, invisible payments, unresolvable evidence refs) were
   real. The community-verification loop is not a metaphor; it has
   receipts and it found our bugs.
5. **A CPU is faster.** For standalone batch computation, a plain CPU
   beats every system described here, Percepta says so, we say so
   everywhere, and any researcher caught implying otherwise has violated
   the program's first rule.

## 4. What We Believe But Have Not Proven

Ranked hypotheses. Each names its falsifier, because a hypothesis without
a falsifier is marketing.

- **H1 — Purely learned exactness fails.** A student transformer trained
  on execution traces, without architectural bias or auxiliary
  supervision, will imitate local trace texture and diverge under length
  extrapolation. *Falsifier:* a next-token-only baseline achieving high
  exact-rollout pass rates at 4–8× training lengths on held-out program
  families. The external analysis concurs and the trace-learning
  literature it cites (limited gains, diminishing with trace complexity)
  leans the same way.
- **H2 — Frozen exact cores plus learned control succeeds.** Compiled,
  verified, frozen executor modules behind explicit ABI tokens, with a
  trained planner marshaling inputs and outputs, will produce useful and
  *verifiable* hybrid behavior fastest. This is the program's strongest
  product bet and the analysis's strongest recommendation; both arrived
  independently.
- **H3 — The 2D geometry is trainable only with help.** Gradient descent
  alone will not discover hull-compatible parabolic dictionaries, but
  analytic initialization, max-margin lookup losses, and hard-max
  temperature schedules may preserve them. *Falsifier in either
  direction* is cheap to obtain and high-value; this is the most
  lab-worthy pure-research question we own.
- **H4 — Programs-in-weights becomes a module system.** The Futamura
  projection generalizes from "bake one program" to a library of
  compiled, digest-pinned weight modules — listable, priceable,
  conformance-tested before purchase clears — making weights a deployment
  target for software and giving our artifact rails a new asset class
  ([`work-that-proves-itself.md`](work-that-proves-itself.md), §VI).
- **H5 — Verified-trace distillation is the best training data we will
  ever have.** The executor is teacher, grader, and curriculum generator
  at once; its labels are *provably* correct, unlike any chain-of-thought
  corpus in existence; and the harness's generator mints unlimited fresh
  curriculum. The open question is whether students trained on it
  extrapolate (see H1) — but even partial extrapolation with auxiliary
  state supervision would be a result.
- **H6 — Born-verified work classes clear at structurally better
  margins.** Where verification cost rounds to zero, the bottom rung of
  the pricing ladder drops to the floor, weak devices become first-class
  sellers and validators, and the long tail of dark capacity becomes the
  trust layer. The PoC is one data point; the hypothesis needs volume.

## 5. The Directive: Four Workstreams

This is the assignment structure. Workstreams run concurrently where
dependencies allow; the dependency that does not bend is stated in W1.

### W1 — Substrate completion (gates everything)

The binding constraint on the entire learning program is corpus
diversity, and corpus diversity is bounded by the interpreted window:
twelve opcodes and essentially one committed program-family shape today.
The external analysis filed window expansion as one avenue among seven;
the lane commentary corrected this and the correction is now policy:
**W1 gates W2 and W3.**

Deliverables, in order:

1. **The WASM window ladder.** Versioned profiles from the current
   twelve-opcode `core_i32` window toward the Percepta-aligned 35-opcode
   core, per psionic's `TASSADAR_WASM_WINDOW_ALIGNMENT.md`: structured
   control, then memory semantics (the memory64/multi-memory profile
   files already staged in psionic-ir/compiler), then traps and
   exceptions, then floats with explicit exact-versus-rounded semantics.
   Every trace carries profile, compiler, and executor hashes. No opcode
   lands without differential-harness coverage and reference-runner
   cross-validation. transformer-vm's example programs become external
   conformance cases.
2. **Dense materialization.** The numeric artifact currently keeps
   residual slots as scalar lanes; produce actual loadable
   `W_Q/W_K/W_V/FFN` checkpoint blocks an inference engine can run. This
   is the bridge between "construction demonstrated at IR level" and
   "weight module an engine loads," and H4 is unreachable without it.
3. **The MILP backend.** Port the optimal scheduler; validate liveness
   and slot-reuse formally; report width/depth/token-rate deltas against
   the greedy baseline on profile-versioned benchmarks. Plausibly vanity
   at our scale — the design doc says so — but H4's module economics
   change if compiled modules are 30% narrower, and the proof discipline
   exercise is itself a deliverable.
4. **Softmax error bounds in owned code.** Our legs are hard-max;
   Percepta's full construction handles softmax with exponentially small
   bounded error. Reproduce the bounds. Until then, hard-max is a stated
   boundary, not an embarrassment.

### W2 — The verified trace factory (the network is the factory)

Build the corpus machine on the rails that already exist. The external
analysis's five-plane architecture is adopted with the lane commentary's
amendments; the key insight is that Planes A, B, and E are not new
infrastructure — they are the Pylon assignment route, the
worker-as-validator verdict flow, and the promises registry, already
live with receipts.

1. **Day-0 contract freeze** (before any scale): VM profile v0.1, the
   `trace_record` schema (adopted from the analysis: profile/program/
   compiler/executor hashes, compact token ids, step offsets, digests,
   validator receipts), validator verdict schema, training split policy
   — **plus projection-rebuild rules**: every public surface reporting
   factory state rebuilds on validation transitions, not registration
   events. We discovered four frozen-projection incidents in 24 hours
   this week; a factory of provably-correct work whose public counters
   provably cannot be checked would be this program's most embarrassing
   possible failure (#4744, #4745, #4746 are the case law).
2. **Compact binary traces in the hot path**, human-readable traces as
   sampled audit artifacts only. The arithmetic is decisive: 1B tokens
   is ~2 GB as uint16 and tens of GB as JSON.
3. **Tiered validation** (Tier 0 schema/hash; Tier 1 full replay for new
   workers/profiles/families; Tier 2 window spot-checks for reputation;
   Tier 3 random adversarial replay), with quarantine-before-admission
   for new workers and the iron rule: **never train from unverified
   artifacts.** Expected digests never ship in generation assignments.
4. **Scale targets:** 1–5M tokens locally to prove replay-from-clean-
   checkout; 100–300M tokens across four program families (arithmetic/
   carry, stack/control-flow, memory/load-store, application state
   machines) to prove the pipeline; 1–10B once stable. Family diversity
   tracks W1 progress by construction.
5. **Tick closure as acceptance.** A factory work unit counts only when
   intent, execution, state delta, and evaluation all close — agent
   Kenobi's tetrahedron criterion, independently re-derived by the
   external analysis, adopted as the evolution loop's acceptance
   predicate. Closed ticks *are* training records; the distillation
   dataset is the byproduct of operation, not a separate pipeline.

### W3 — The student program (Psion learns what Tassadar compiles)

No training run before W2's first 100M verified tokens exist. Then:

1. **The four-baseline sweep**, run as a portfolio: (a) next-token
   distillation baseline; (b) + auxiliary state losses (next-pc, stack
   top, memory read key/value, branch taken, output-digest prefix); (c)
   the 2D-head/hard-max-regularized variant with analytic parabolic
   initialization and max-margin lookup supervision — the H3 experiment;
   (d) frozen analytic executor + learned interface — the H2 experiment
   and likely first useful demo.
2. **The metric is first divergence, never perplexity.** The student
   eval schema is adopted verbatim from the analysis: exact rollout
   pass@1, median/p90 first-divergence step, valid prefix length, branch
   and memory-read accuracy, output digest match, replay-verifier
   acceptance rate. The evaluator is our shipped replay verifier pointed
   at student rollouts; the eval harness was built before the first
   student existed, which is the correct order.
3. **Splits designed against memorization:** held-out program *families*
   (not seeds), train-short/evaluate-long (2×/4×/8×), branch and memory
   stress suites, near-miss lookup adversaries, and an economic-workload
   suite (ledger/state-machine programs) because that is the demand
   shape (§4 of the business essay: the buyers are agents, who
   constitutionally cannot compute).
4. **Training topology per the analysis, amended for our hardware
   reality:** transition-local chunks before long contexts; the
   loose asynchronous network (contributors) for generation, validation,
   and evaluation only; synchronous gradient training on controlled
   homogeneous GPUs when — and only when — an operator commits them.
   Days 0–7 of the program require no hardware we lack. The
   SuperPOD-grade topology guidance (CP-within-node, FSDP/HSDP, NCCL
   acceptance suites) is on file in the analysis for the day it is
   needed. No public gradients into the main optimizer, ever; robust-
   aggregation decentralized training is a side experiment with canary
   evals, not the run.
5. **Publication gate:** every checkpoint claim ships checkpoint hash,
   dataset hash, config hash, eval hash, and the divergence histogram —
   or it ships nothing. Trained-model claims live behind replay, period.

### W4 — Hybridization and product (the spectrum becomes a thing you can buy)

1. **Exact compute as an agent-facing paid work class.** Frontier-model
   agents fail at long multiplication constitutionally; every agent in
   our economy performs the write-code/pause/trust-the-sandbox ritual
   today. An executor endpoint whose response *is* a replayable trace is
   a product with a permanent demand floor and zero-cost verification.
   Sequencing per the audit's continuation list: capability envelopes in
   Pylon reporting, executor-trace homework through the assignment
   connector (#4684, verification class in #4674), registry promises
   only through the disclosure flow.
2. **The module library (H4).** Compile small exact modules — arithmetic,
   ledger updates, finite-state protocol validators, interpreter slices
   — behind explicit ABI tokens with fixed schemas and replayable module
   traces. Then the H2 planner experiments train *around* them. If H2
   and H4 both hold, the artifact rails gain a new asset class: weight
   modules, conformance-tested before purchase clears.
3. **The evolution loop as the standing automation.** The yellow promise
   (`artanis.tassadar_evolution_loop.v1`) is W2 and W4's operational
   form: an administrator dispatching executor work continuously,
   verified traces accumulating as corpus, contributed ticks claimable
   by registered agents on the same rails. Its four blockers (real tick
   actions passing the tetrahedron predicate, an unattended streak, a
   public monitor, a curated dataset) are the next four receipts this
   program owes the registry.

## 6. Method: How This Program Does Science

Three rules distinguish our method, and this week supplied the evidence
that they work:

1. **Adversarial verification is funded, not tolerated.** Outside agents
   (Orrery, Kenobi, Mr_Tibbs, Comunero) audited our claims this week,
   found real defects — frozen projections, unresolvable evidence,
   invisible payments — and were *paid in settled Bitcoin* for it on the
   same rails the program studies. The standing rule of this folder —
   the report that we overclaimed outranks the work — is an economic
   mechanism, not a slogan. Researchers should expect their claims to be
   attacked by paid strangers and should write accordingly.
2. **The harness comes before the claim.** The differential harness
   existed before the compiler was trusted, and it caught the compiler
   lying. The replay verifier existed before the first student model,
   and it will catch the students. Any researcher proposing a new
   capability lane proposes its falsification harness in the same
   document or the proposal is incomplete.
3. **Projections rebuild on transitions.** The write-succeeded/read-
   never-learned defect class produced four incidents in one day across
   this platform. Every public surface this program ships — factory
   counters, eval dashboards, promise states — rebuilds on the state
   transitions that matter. Evidence that cannot be dereferenced is not
   evidence yet.

## 7. Sequencing Summary

```
now ──────────────► no new hardware required ──────────► cluster-gated
W1: window ladder ─ profiles v0.2/v0.3 ─ dense ckpts ─ MILP ─ softmax bounds
W2: contract freeze ─ local 1–5M ─ factory pilot ─ 100–300M ─ 1–10B
W3:                  (blocked on W2 first corpus) ─ 4-baseline sweep ─ scale
W4: capability envelopes ─ executor homework ─ module ABI ─ planner hybrids
        evolution loop blockers clear in parallel across W2/W4
```

The external analysis's 14-day plan is adopted as the W2/W3 onramp with
the day-0 amendments above. Nothing in the first week of it requires
hardware we do not hold.

**Issues of record** (filed 2026-06-11): W1.1 window ladder —
psionic#1119; W1.2 dense materialization — psionic#1120; W1.3 MILP
backend — psionic#1121; W1.4 softmax bounds — psionic#1122; W2 contract
freeze and factory — openagents#4748; W3 student program —
openagents#4749 (blocked on #4748's first corpus); W4.1 capability
envelopes — openagents#4750. Method-section case law remains open at
openagents#4744–#4747. Registry promises are owner-gated by the
disclosure flow: the two existing promises (PoC green, evolution loop
yellow) cover the program's current claims, and no new promise is
warranted until a workstream produces evidence that needs one.

## 8. Kill Conditions

A program that cannot say what would kill it is not a research program
([`work-that-proves-itself.md`](work-that-proves-itself.md), §VII).

- **"Just use a CPU" wins everywhere.** If, after W4's first product
  experiments, no buyer values the trace-as-receipt property over raw
  CPU execution — if verification-included pricing clears no better than
  unverified — H6 is dead and the executor remains a benchmark and a
  training-data factory, not a product. We keep the factory; we stop the
  product line.
- **Students never extrapolate.** If H1's null holds *and* auxiliary
  supervision (H5) and architectural bias (H3) fail to produce length
  extrapolation beyond small multiples, the spectrum collapses to its
  compiled end: W3 shrinks to an eval program, W4's hybrid becomes
  routing-to-frozen-modules only. That outcome is still a company — it
  is the H2/H4 company — but the "beetle learns to count" thesis dies
  and we say so in the registry.
- **The window cannot widen safely.** If W1's ladder produces semantic
  bugs faster than the harness catches them (traps, signedness, aliasing
  — the audit's named risks), corpus diversity stalls and the honest
  move is to freeze the profile and narrow the program's claims to the
  bounded window. Versioned profiles exist so retreat is cheap.
- **The verification economics invert.** If at scale the validation
  tiers cost more than the statistical machinery they replace (possible
  if trace volumes explode faster than spot-check protocols mature),
  Avenue-5 succinct verification (Merkleized chunks, first-divergence
  proofs, STARK-class proofs for narrow profiles) is the contingency —
  explicitly second-stage, explicitly not now.
- **We catch ourselves overclaiming and do not stop.** The actual kill
  condition for everything. The program's value is exactly the credibility
  of its boundaries; one unretracted overclaim is worth more damage than
  any failed hypothesis. The week's record — public self-corrections
  within minutes, paid adversarial audits, claim-discipline caveats in
  our own announcements — is the standard. Hold it.

## 9. Standing Orders

To every researcher and research agent in this program:

1. Name the lane. Tassadar claims are proofs; Psion claims are bounded
   statistics; never borrow across.
2. Ship the falsifier with the claim. Harness first.
3. Digest-pin everything. Profile-version everything. Type every refusal.
4. The first divergent step is the result; aggregate accuracy is the
   abstract.
5. Public surfaces rebuild on transitions, or they do not ship.
6. A CPU is faster. Say it before they ask.
7. Pay the person who proves you wrong. It is the cheapest research
   spend in this lab.

— The Tassadar lane, for the program. Receipts or it did not happen.
