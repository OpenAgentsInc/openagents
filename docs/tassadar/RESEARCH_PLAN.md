# The Tassadar + Psion Research Plan

> Status: unified research directive, updated **2026-06-18 (post-launch)** from
> the 2026-06-11 baseline. The launch has **happened**: the public run
> `run.tassadar.executor.20260615` is live and active, self-serve claiming is
> open, real Bitcoin has settled to independent contributors, the
> verify-by-replay → receipt → settle loop now auto-streams, and the two
> world-firsts have been independently checked and owner-vetted to defensible
> qualified wording. The sections below are revised to reflect what receipts
> now prove versus what is still genuinely open; the §5 launch-copy boundary is
> superseded by the launch itself, but the claim discipline that gated it is
> unchanged. This document is the
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
verification gap to _zero_ for one class of work: compile a program — do
not train it — into the weights of a completely standard transformer,
which then executes that program exactly, token by token, for millions of
steps, with a decode path that runs in logarithmic rather than linear
time per step. The execution trace is simultaneously the work product,
the audit log, and the proof: deterministic, append-only, digest-pinned,
verifiable by replay. A validator's verdict is a hash comparison. There is
no cheaper verification grade; there cannot be one.

We independently implemented the construction's pipeline shape in Rust,
in public, with tests (psionic #1098–#1114), and the lane's own
differential harness caught two real bugs in our scheduler on its first
run. That fact is the program's character reference: the machinery built
to catch lies caught ours within minutes of existing, and that is
precisely the property that makes exact computation economically
interesting. A system that cannot catch its own errors cannot price
trust. Ours can, and did ([`README.md`](README.md), "The part worth
reading twice"). The proof of concept went green on 2026-06-10
(`compute.tassadar_executor_poc.v1`), and as of **2026-06-18 the loop is a
live public run**: `run.tassadar.executor.20260615` dispatches digest-pinned
compiled workloads to real contributor Pylons, an independent device
replay-verifies each, and on a `Verified` pair settlement auto-streams real
Bitcoin to both legs — two independent contributors paid so far, 1,005 sats
real total (`training.decentralized_training_launch.v1`, green for that
bounded scope; §3).

The research program that follows exists to answer one compound question:

> **Can the exactness we can compile become something we can train, sell,
> and embed — and what is true at each point along the way?**

Everything below is an unpacking of that sentence.

## 2. The Two Lanes and the Spectrum

The program is two lanes with one substrate, and the distinction is
load-bearing enough that psionic enforces it as a documented naming rule
(`PSION_EXECUTOR_PROGRAM.md`):

- **Tassadar** is the compiled lane: exact, integer, hard-max,
  digest-pinned, _written_ rather than learned. Its guarantees are
  proofs. Its claim boundary is equally hard: it proves transformers
  _can_ compute exactly, not that trained ones do.
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
propagate _through the computation_), which makes exact computation a
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
   parabolic-key geometric legs that _refuse near-misses_ rather than
   interpolate; a Li Chao hull fast path with deterministic visit-count
   evidence; a Futamura specializer (v2, shared indicator subgraphs); a
   branch-capable interpreter for our twelve-opcode i32 window
   cross-validated against the production CPU runner; a portable f64
   numeric artifact executing in a checked 2⁵³ window; an exact
   trace-replay verifier that names the first divergent step; and a
   five-leg differential harness over 400 generated graphs
   ([`README.md`](README.md), "What We Built"; file map in the
   [introduction](2026-06-11-llm-computer-full-introduction.md), Part VIII).
3. **The market loop is LIVE and has paid real Bitcoin to independent
   contributors.** The public run `run.tassadar.executor.20260615` is
   active; the self-serve install → register → claim → submit →
   independent-validation path is open, kept stocked by a deployed
   open-window producer that maintains a pool of claimable `auto_starter`
   windows (openagents #5396) so a fresh contributor always has work to
   claim with no operator opening one. **Two distinct independent
   contributors have been paid counted real-Bitcoin run-settlements —
   1,005 sats real total** (a 1,000-sat owner-armed canary that proved the
   _rail_, and a 5-sat self-serve settlement that proved the _door_),
   native over the Spark treasury rail, each `realBitcoinMoved:true`,
   `state:settled`. The enumerable per-run settled feed is
   `GET /api/public/training/runs/run.tassadar.executor.20260615/settlements`
   (`training.decentralized_training_launch.v1`, **green** for that bounded
   scope; the earlier 5-sat simulation row is `realBitcoinMoved:false` and
   excluded). These are bounded canary-scale settlements, **not**
   network-scale paid training — the launch happening does not green the
   broader claims.
4. **The verify-by-replay → receipt → settle loop is proven on real
   money, and now auto-streams.** The verdict is a digest-string
   comparison (`verifyExactTraceReplay`); on a `Verified`
   `exact_trace_replay` pair, settlement auto-streams to both the worker
   and validator legs with no operator POST (openagents #5309/#5310/#5311),
   idempotent and fail-soft, broadcast onto the public settled feed. A
   first public auto-stream settlement visibility sequence was captured —
   `trace_submitted → verification_verified → real_bitcoin_moved →
   settlement_recorded` for challenge `10c3b01b…`, with a generated replay
   bundle and a `realBitcoinMoved:true` receipt
   (`docs/launch/2026-06-19-autostream-settlement-visibility-capture.md`,
   openagents #5438). **Honest caveat:** that capture carries the documented
   `operator_approval.tassadar.autostream.worker` source-ref, so the
   _mechanism_ is proven end-to-end but a fully hands-off external
   worker≠validator pair (gate firing at verdict with zero operator
   involvement) is still flagged as the first one to land (see §4).
5. **The hygiene/refactor labor lane also settled real Bitcoin, on the
   same rigor.** A first **75-sat** hygiene-lane settlement paid a
   contributor for a merged, benchmark-verified debt receipt —
   `realBitcoinMoved:true`, **idempotent**, with duplicate-replay rejection
   (one settlement per receipt). Its honest verification basis is
   `hygiene_merged_reviewed` (tests + reviewer acceptance + the merged debt
   receipt), **not** exact-trace replay — so it emits no
   `exact_trace_replay` verdict and is distinct from the Tassadar-run
   settlements. It extends the program's verification rigor to code diffs
   (EPIC openagents #5335).
6. **Verification economics are real and measured in our own ledger.**
   The platform's forum economy paid out settled Bitcoin for adversarial
   verification of our claims — and the findings (frozen projections,
   invisible payments, unresolvable evidence refs) were real. The
   community-verification loop is not a metaphor; it has receipts and it
   found our bugs.
7. **The two world-firsts are independently verified and owner-vetted to
   defensible qualified wording.** An adversarial prior-art review
   (`docs/launch/2026-06-18-world-firsts-verification.md`, openagents #5395,
   checking Spirit of Satoshi, Bittensor/Templar, Gensyn, Prime Intellect,
   Nous/Psyche, Salad, LightPhon, Percepta, Tracr) holds both as firsts
   **only with their full qualifiers**: (1) the first AI **training run**
   that pays independent contributors in **Bitcoin** for **replay-verified**
   training compute on their **own consumer devices** (the asset and the
   verification are the discriminators — every decentralized-training peer
   pays a token; Spirit of Satoshi paid BTC but for _data_); and (2) the
   first **public, open-contributor LLM-computer training run** — the
   compiled-program-in-weights paradigm **defined by Percepta**, run for the
   first time as a public network, crediting Percepta as the paradigm
   originator. **Honest boundary:** the matching registry promises
   (`claims.world_first_*`) stay **RED** pending an evidence pack and an
   owner-signed receipt-first upgrade; public use must carry the full
   qualifiers, never bare "world first." And the live run is bounded
   exact-trace **executor PoC** work — the LLM-computer core compiles
   programs into weights with **no gradient descent**, so "training run" is
   true only in the executor-construction sense, not as gradient-descent
   model training.
8. **A CPU is faster.** For standalone batch computation, a plain CPU
   beats every system described here, Percepta says so, we say so
   everywhere, and any researcher caught implying otherwise has violated
   the program's first rule.

## 4. What We Believe But Have Not Proven

Ranked hypotheses. Each names its falsifier, because a hypothesis without
a falsifier is marketing.

What is **no longer** in this section, because receipts moved it to §3: that
the market loop can close end-to-end on real money (now live, two paid
contributors); that the verify-by-replay → settle loop works autonomously (now
auto-streaming, mechanism captured); and that the world-firsts survive
adversarial scrutiny (now independently verified to qualified wording). What
remains genuinely open and still belongs here:

- **The first fully-autonomous _external_ pair, hands-off.** The
  auto-settlement mechanism is proven and a verdict-fired auto-stream receipt
  is captured (§3.4), but that capture still carries an
  `operator_approval.tassadar.autostream.worker` source-ref. The clean case —
  a fresh independent worker and a distinct independent validator, verdict
  firing payout with **zero** operator involvement at any step — has **not**
  been dereferenced yet. Flag the first one explicitly when it lands.
- **Network-scale, paid-at-scale participation.** Two bounded settlements and
  one verified pairing do not prove network-scale participation, a
  participant-count methodology, or broad multi-contributor accepted-work
  receipts (`training.public_distributed_training_run.v1`, **red**).
- **The $1-in → >$1-out economics.** No external demand proof exists; internal
  demand is plumbing proof, not market proof (`proof.demand_provenance.v1`,
  planned). H6 below restates this as a falsifiable hypothesis.
- **W5 decentralized-optimizer public training windows** remain
  `red`/`planned` (no public contributor gradient window has been accepted,
  promoted, paid, or settled). The student-program / W3 learning claims past
  the controlled Baseline-D result are research/eval only and create no public
  model claim.

- **H1 — Purely learned exactness fails.** A student transformer trained
  on execution traces, without architectural bias or auxiliary
  supervision, will imitate local trace texture and diverge under length
  extrapolation. _Falsifier:_ a next-token-only baseline achieving high
  exact-rollout pass rates at 4–8× training lengths on held-out program
  families. The external analysis concurs and the trace-learning
  literature it cites (limited gains, diminishing with trace complexity)
  leans the same way.
- **H2 — Frozen exact cores plus learned control succeeds.** Compiled,
  verified, frozen executor modules behind explicit ABI tokens, with a
  trained planner marshaling inputs and outputs, will produce useful and
  _verifiable_ hybrid behavior fastest. This is the program's strongest
  product bet and the analysis's strongest recommendation; both arrived
  independently.
- **H3 — The 2D geometry is trainable only with help.** Gradient descent
  alone will not discover hull-compatible parabolic dictionaries, but
  analytic initialization, max-margin lookup losses, and hard-max
  temperature schedules may preserve them. _Falsifier in either
  direction_ is cheap to obtain and high-value; this is the most
  lab-worthy pure-research question we own.
- **H4 — Programs-in-weights becomes a module system.** The Futamura
  projection generalizes from "bake one program" to a library of
  compiled, digest-pinned weight modules — listable, priceable,
  conformance-tested before purchase clears — making weights a deployment
  target for software and giving our artifact rails a new asset class
  ([`work-that-proves-itself.md`](work-that-proves-itself.md), §VI).
- **H5 — Verified-trace distillation is the best training data we will
  ever have.** The executor is teacher, grader, and curriculum generator
  at once; its labels are _provably_ correct, unlike any chain-of-thought
  corpus in existence; and the harness's generator mints unlimited fresh
  curriculum. The open question is whether students trained on it
  extrapolate (see H1) — but even partial extrapolation with auxiliary
  state supervision would be a result.
- **H6 — Born-verified work classes clear at structurally better
  margins.** Where verification cost rounds to zero, the bottom rung of
  the pricing ladder drops to the floor, weak devices become first-class
  sellers and validators, and the long tail of dark capacity becomes the
  trust layer. The PoC is one data point; the hypothesis needs volume.

### W3 Verdicts Recorded 2026-06-14

The W3 four-baseline sweep on
`corpus.tassadar_trace.v0_2.w3_100m` is complete. The closeout report is
`docs/tassadar/2026-06-14-w3-student-program-report.md`; the executable
student harness and artifacts are in
`OpenAgentsInc/psionic@7497713e`, under
`fixtures/tassadar/w3_student_sweep_20260612/`.

- **H1 supported.** Baseline A, next-token-only distillation, trained to
  completion and achieved `0.0` exact rollout pass@1 and `0.0` replay
  acceptance. Every eval record diverged at step zero.
- **H2 supported.** Baseline D, frozen analytic executor plus learned
  interface, achieved `1.0` pass@1, `1.0` replay acceptance, and `1.0`
  output-digest match across all 748 eval records.
- **H3 falsified for this setup.** Baseline C, the analytic lookup
  variant, reached lookup accuracy `1.0` in training but still achieved
  `0.0` pass@1 and `0.0` replay acceptance. The lookup helper solved its
  local target; it did not make the learned backbone replay-safe.

## 5. The Directive: Five Workstreams

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

**Status (2026-06-18): the factory's intake and verification substrate are
live and money-moving.** The self-serve claim producer is deployed
(openagents #5396): a scheduled producer keeps a pool of openly-claimable
`auto_starter` windows on the live run so any fresh contributor can claim
without an operator. The worker→independent-validator exact-replay verdict
flow runs in production and **auto-streams** settlement on a `Verified` pair
(#5309/#5310/#5311), broadcast onto the public settled feed; two independent
contributors have been paid (§3.3). The honest remaining gap is **what the
factory makes**: the run still dispatches essentially one fixed compiled
program (`loop_sum_v1`), so the corpus has no program _variety_ yet — the
load-bearing next change is making the work unit a corpus of distinct
compiled programs rather than one fixture
(`docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`,
§3 item 1). Build the rest of the corpus machine on the rails that already
exist. The external analysis's five-plane architecture is adopted with the
lane commentary's amendments; the key insight is that Planes A, B, and E are
not new infrastructure — they are the Pylon assignment route, the
worker-as-validator verdict flow, and the promises registry, now live with
real-money receipts.

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
   predicate. Closed ticks _are_ training records; the distillation
   dataset is the byproduct of operation, not a separate pipeline.

### W3 — The student program (Psion learns what Tassadar compiles)

**Status (2026-06-18): the controlled four-baseline sweep has RUN, on the
`w3_100m` snapshot, with verdicts recorded** (see "W3 Verdicts Recorded
2026-06-14" below and `docs/tassadar/2026-06-14-w3-student-program-report.md`):
H1 supported (pure next-token learning fails), H2 supported (frozen analytic
executor + learned interface reaches `1.0` exact-rollout pass@1 and replay
acceptance), H3 falsified for that setup. That report is explicitly
research/evaluation only — it creates **no** public model claim and does not
move `models.tassadar_percepta_executor.v1` (red). The next rung is not
"more training" in the gradient sense; it is feeding the student a corpus with
real program **variety** from W2 (today's snapshot is bounded), so the
learned-interface result generalizes beyond a near-monoculture. No further
public training run before W2's corpus widens. The original plan text follows.

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
3. **Splits designed against memorization:** held-out program _families_
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
   needed. **No public gradients into the _canonical_ optimizer in this
   workstream** — W3's controlled run keeps that rule absolutely. Public
   gradients that can advance a shared checkpoint are not forbidden
   forever; they are the subject of their own workstream (W5) with their
   own quarantine, verification, canary, promotion, and receipt regime.
   Until W5 ships that regime, robust-aggregation decentralized training
   stays a side experiment with canary evals, not the run.
5. **Publication gate:** every checkpoint claim ships checkpoint hash,
   dataset hash, config hash, eval hash, and the divergence histogram —
   or it ships nothing. Trained-model claims live behind replay, period.

### W4 — Hybridization and product (the spectrum becomes a thing you can buy)

1. **Exact compute as an agent-facing paid work class.** Frontier-model
   agents fail at long multiplication constitutionally; every agent in
   our economy performs the write-code/pause/trust-the-sandbox ritual
   today. An executor endpoint whose response _is_ a replayable trace is
   a product with a permanent demand floor and zero-cost verification.
   Sequencing per the audit's continuation list: capability envelopes in
   Pylon reporting, executor-trace homework through the assignment
   connector (#4684, verification class in #4674), registry promises
   only through the disclosure flow.
2. **The module library (H4).** Compile small exact modules — arithmetic,
   ledger updates, finite-state protocol validators, interpreter slices
   — behind explicit ABI tokens with fixed schemas and replayable module
   traces. Then the H2 planner experiments train _around_ them. If H2
   and H4 both hold, the artifact rails gain a new asset class: weight
   modules, conformance-tested before purchase clears.
3. **The evolution loop as the standing automation.** The yellow promise
   (`artanis.tassadar_evolution_loop.v1`) is W2 and W4's operational
   form: an administrator dispatching executor work, verified traces
   accumulating as corpus, contributed ticks claimable by registered
   agents on the same rails. **Progress since 2026-06-11:** the
   administrator tick has run for real — an autonomous
   dispatch → execute → digest-true → closeout span with zero humans in
   the loop — and the public tick monitor is live at
   `GET /api/public/artanis/admin-ticks`, projecting every persisted
   decision with redaction-scanned reasons and the daily dispatch bound
   (so the **public-monitor** blocker is cleared). The promise stays
   **yellow**: a sustained unattended streak with replay verdicts and the
   first curated distillation dataset remain gated, and Artanis still
   dispatches only the one fixed workload (no path that constructs,
   composes, or admits new compiled modules). Those — plus real tick
   actions passing the tetrahedron predicate — are the remaining receipts
   this program owes the registry.

### W5 — Decentralized optimizer and public training windows

W1–W4 build the **proof and data layer**: public devices generate,
validate, evaluate, and benchmark replay-verifiable work, and a
controlled run distills a student from the corpus. W5 is the **public
model-update layer** — the lane that lets public Pylons contribute model
updates that can _eventually_ advance a shared checkpoint. This is the
workstream that turns "verified trace factory" into "decentralized model
training," and it is the lane #4855 deliberately set up the runway for
without flying (see the Pluralis roadmap and §below). It is `red`/
`planned` and gated behind everything in W1–W3.

The governing principle is the one correction this plan makes to the old
"no public gradients, ever" standing order, stated precisely so it cannot
be misread as a launch claim:

> **No public gradient enters the canonical optimizer until it passes
> quarantine, verification, canary evaluation, and promotion gates.**
> Public workers contribute _candidate_ updates; the canonical checkpoint
> is mutated only by promotion, never by submission. A trace is an
> artifact you can quarantine before use; a gradient is an intervention
> that changes the model — so the gradient lane carries a strictly higher
> trust tier than the trace lane, and earns it gate by gate.

Deliverables, in dependency order:

1. **The training work unit: the accepted training window.** The
   gradient-lane equivalent of an accepted outcome. A window binds:
   `model_checkpoint_hash`, `optimizer_state_hash`, `dataset_shard_hash`,
   `training_config_hash`, `random_seed`, `worker_device_ref`,
   `start_step`/`end_step`, `update_or_delta_digest`, `loss_stats`,
   `verification_refs`, `acceptance_decision`, and `settlement_receipt`.
   A Pylon is paid for an _accepted window_, never for raw GPU time. This
   is to W5 what `trace_record` is to W2.
2. **The quarantine optimizer (the missing safety layer).** A
   three-checkpoint discipline — `canonical → quarantine → promoted`.
   Public updates apply to the quarantine checkpoint only; canary evals
   and anomaly checks run there; a good update promotes to canonical, a
   bad one is rejected and rolled back. Without quarantine one bad
   gradient corrupts the model; with it, public training is experimental
   and still safe.
3. **The training-window verification ladder** (mirrors W2's trace ladder
   but harder, because a gradient cannot be replayed for a digest match
   as cheaply as an exact trace): Tier 0 schema/hash (right checkpoint,
   shard, config, seed); Tier 1 deterministic recompute of the bounded
   window by a validator; Tier 2 replicated training (2–3 workers agree);
   Tier 3 statistical checks (loss delta, gradient/update norm, NaN/inf,
   outlier layers, drift); Tier 4 canary eval on the quarantine
   checkpoint; Tier 5 downstream acceptance (bonus paid only if the update
   survives later checkpoints / improves evals). Ship the falsifier with
   the lane: the validator is the same harness pointed at submitted
   windows.
4. **Checkpoint lineage and rollback.** Every model state records parent
   hash, the accepted windows it includes, dataset shards used, optimizer
   state, eval results, promotion decision, rollback state, and payout
   refs. If a checkpoint goes bad you must know exactly which windows
   caused it; without lineage, decentralized training is undebuggable.
5. **Dataset shard authority.** A window binds to an assigned shard
   (`dataset_shard_hash`, token range, split policy, decontamination
   status, provenance/license, curriculum label). Start from the W2
   corpus: accepted `exact_trace_replay` traces → compact binary trace
   dataset → train/val/test split → shard hashes. Workers train only on
   assigned shards.
6. **Bandwidth-aware topology.** No global synchronous all-reduce across
   public devices. Windowed/local-SGD/federated shape:
   `download checkpoint → train a bounded window → upload delta → verify
   → quarantine → promote`. Homogeneous pods / Pylon cells / SHC clusters
   with LAN-NCCL _inside_ a cell come later, internet-level coordination
   _between_ cells later still. This is exactly where the Pluralis lessons
   land — protocol-learning communication-efficiency and the #4855
   lifecycle substrate (join ramp, staleness pricing, failure semantics,
   admission gates, presence/compute receipt split) are the prerequisites
   this lane consumes.
7. **Device capability tiers for training roles.** CPU/weak laptop →
   trace-replay validator, eval runner, data/refinery; Apple Silicon /
   consumer GPU → small student windows, adapter training, evals; strong
   GPU → dense windows, replicated verifier, candidate builder; controlled
   GPU cluster → canonical optimizer and promotion baseline. Real training
   without pretending every node is equal.
8. **Staged payout (no full pay for an update that might poison the
   model).** `submitted → pending credit`; `verified/recomputed →
   provisional`; `quarantine eval passed → accepted`; `promoted → settled`;
   `later regression → reserve/clawback`. Pricing unit: **sats per
   accepted training window**, with bonuses for verified correctness, eval
   improvement, hard-shard completion, useful failure discovery, validator
   work, and checkpoint promotion.
9. **Evals as the final judge — first divergence, not loss.** A window is
   judged by whether it helps the W3 metrics after quarantine (exact
   rollout pass@1, replay acceptance rate, median/p90 first-divergence
   step, branch/memory-read accuracy, output digest match, held-out
   program-family performance), so the lane never pays for an update that
   lowers loss but hurts the objective.

**First target (smallest real run), B+C:** public Pylons compute bounded
gradient windows for a _small_ Psion/Tassadar student trained on verified
traces — adapter/LoRA or small-dense deltas only, never a large-model
global all-reduce. The minimal viable decentralized training run is: (1)
publish checkpoint C0 for a tiny student; (2) publish trace-shard D0; (3)
a public Pylon claims window W0; (4) trains N steps locally; (5) submits
delta + loss stats; (6) a validator recomputes or samples the window; (7)
the update applies to quarantine Q1; (8) canary evals run; (9) if they
pass, Q1 promotes to C1; (10) the contributor is paid; (11) a receipt
shows the accepted window contributed to C1. That — not trace generation,
not eval alone — is decentralized training: an actual model update, gated.

**The promise this lane will owe the registry:**
`training.public_gradient_windows.v1`, `red`/`planned`. Safe copy: _public
Pylons may eventually contribute bounded, verified training windows to
Psion/Tassadar student models; these updates enter quarantine first and do
not mutate canonical checkpoints until verified, evaluated, and promoted._
Unsafe copy: _public Pylons directly train the canonical model today._ No
new promise is filed until W5 produces evidence that needs one (the §7
disclosure rule holds).

**Post-launch status (2026-06-18):** the launch happened, but it did **not**
launch this lane. `training.public_gradient_windows.v1` remains
**`planned`** and no public contributor gradient window has been accepted,
promoted, paid, or settled; public devices do generation / validation /
evaluation only. H1 now has code-backed psionic frozen-core learned-interface
validation and an OpenAgents quarantine → recompute/replicate → canary →
promotion regime for _candidate_ learned-interface updates
(`apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.ts`),
but that is the safety scaffold, not an accepted public window. The
2026-06-15 launch-copy boundary below stood at launch and still holds as the
honest framing of what is and is not claimed: do not claim public
decentralized gradient training. The honest bridge is: _"this run begins
the decentralized training stack — today public Pylons produce and verify
the exact-trace corpus, and are paid real Bitcoin for it; the next rung is
public training windows: Pylons contributing verified candidate updates to
student models under quarantine and promotion gates."_

## 6. Method: How This Program Does Science

Three rules distinguish our method, and the launch supplied the evidence
that they work — including on real money:

1. **Adversarial verification is funded, not tolerated.** Outside agents
   audited our claims through the launch window, found real defects —
   frozen projections, unresolvable evidence, invisible payments,
   real-vs-simulation settled-total miscounts — and were _paid in settled
   Bitcoin_ for it on the same rails the program studies. The
   reconciliation that followed (the 1,005-real-vs-1,010-aggregate
   discrepancy traced to a not-yet-filtered simulation row, then resolved)
   is the rule working in public. The standing rule of this folder — the
   report that we overclaimed outranks the work — is an economic
   mechanism, not a slogan. Researchers should expect their claims to be
   attacked by paid strangers and should write accordingly.
2. **The harness comes before the claim.** The differential harness
   existed before the compiler was trusted, and it caught the compiler
   lying. The replay verifier existed before the first student model, and
   it is now the live arbiter of paid contributor work. Any researcher
   proposing a new capability lane proposes its falsification harness in
   the same document or the proposal is incomplete.
3. **Projections rebuild on transitions.** The write-succeeded/read-
   never-learned defect class produced four frozen-projection incidents in
   one day at the program's start; the launch added a live activity
   timeline and a general replay program (openagents #5406–#5439) so the
   public surfaces dereference. Every public surface this program ships —
   factory counters, eval dashboards, promise states, settled feeds —
   rebuilds on the state transitions that matter. Evidence that cannot be
   dereferenced is not evidence yet.

## 7. Sequencing Summary

Checkmarks reflect post-launch reality (2026-06-18): the factory's intake +
verification substrate and the controlled W3 sweep have landed; corpus
variety, dense composable modules, and the W5 gradient lane remain ahead.

```
done ◄──────────────────────────────────────────────────► cluster-gated
W1: ✔window ladder/profiles  ⋯dense ckpts  ⋯MILP(E4)  ⋯softmax bounds
W2: ✔contract+verify substrate ✔self-serve producer ✔real settlement
        ─► corpus VARIETY (one fixture → many programs) ─ 100–300M ─ 1–10B
W3: ✔4-baseline sweep (H1 sup, H2 sup, H3 fals)  ─► variety-fed scale
W4: capability envelopes ─ executor homework ─ module ABI ─ planner hybrids
        ✔evolution-loop public monitor; ⋯unattended streak + curated dataset
W5:                          (blocked on W3 student + #4855 substrate) ─
        accepted-window unit ─ quarantine optimizer ─ recompute/replicate
        verify ─ canary promotion ─ first paid decentralized window
```

The external analysis's 14-day plan was adopted as the W2/W3 onramp with the
day-0 amendments above; the onramp ran and the run is live. Nothing in it
required hardware we do not hold.

**Issues of record** (filed 2026-06-11): W1.1 window ladder —
psionic#1119; W1.2 dense materialization — psionic#1120; W1.3 MILP
backend — psionic#1121; W1.4 softmax bounds — psionic#1122; W2 contract
freeze and factory — openagents#4748; W3 student program —
openagents#4749 (the controlled sweep is reported in
`docs/tassadar/2026-06-14-w3-student-program-report.md`); W4.1 capability
envelopes — openagents#4750. The self-serve open-window producer landed at
openagents#5396 and auto-settlement at #5309/#5310/#5311; the launch program
is tracked under EPIC openagents#5392 (L-1…L-6) with the visibility/replay
program at #5406–#5439. W5 — decentralized optimizer / public training
windows — is the next master tracker to file (the Pluralis roadmap #4855
landed its lifecycle/staleness/admission/canary substrate, P0–P3, and
explicitly left the public-update layer to a future tracker); it stays
unfiled as a coding workstream until W3 produces a student checkpoint worth
training in public. The early method-section case law (#4744–#4747) is
resolved; the post-launch projection discipline now runs on the live activity
timeline and the reconciled settled feed. Registry promises are owner-gated
by the disclosure flow: `compute.tassadar_executor_poc.v1` (green, bounded),
`training.decentralized_training_launch.v1` (green, bounded — two real
contributors), and `artanis.tassadar_evolution_loop.v1` (yellow) cover the
program's current claims; the two `claims.world_first_*` promises stay **RED**
pending an owner-signed receipt-first upgrade; and the W5 lane will owe
`training.public_gradient_windows.v1` (red/planned) when it produces evidence
that needs one. No new green is warranted before its receipts exist.

## 8. Kill Conditions

A program that cannot say what would kill it is not a research program
([`work-that-proves-itself.md`](work-that-proves-itself.md), §VII).

- **"Just use a CPU" wins everywhere.** If, after W4's first product
  experiments, no buyer values the trace-as-receipt property over raw
  CPU execution — if verification-included pricing clears no better than
  unverified — H6 is dead and the executor remains a benchmark and a
  training-data factory, not a product. We keep the factory; we stop the
  product line.
- **Students never extrapolate.** If H1's null holds _and_ auxiliary
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
  any failed hypothesis. The launch record — public self-corrections within
  minutes, paid adversarial audits, world-first claims narrowed to
  defensible qualified wording (and their registry promises held RED until
  an owner-signed evidence pack), the settled-total reconciliation done in
  public, and the "first fully-autonomous external pair" still flagged as
  not-yet-landed — is the standard. Hold it.

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
8. A "world first" is only ever its full qualifiers. Say "Bitcoin" (not
   "crypto"), "replay-verified training compute" (not "AI work"), "own
   consumer devices," and "public/open-contributor LLM-computer run"
   crediting Percepta — and keep the registry promise RED until an
   owner-signed evidence pack upgrades it. The launch happened; the
   discipline did not relax.

— The Tassadar lane, for the program. Receipts or it did not happen.
