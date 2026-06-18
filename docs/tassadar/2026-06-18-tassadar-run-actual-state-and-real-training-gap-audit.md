# Tassadar Run: Actual State and the Real-Construction Gap

Date: 2026-06-18 (corrected 2026-06-18)
Scope: an honest accounting of where the Tassadar run actually is — measured
against what "training/building the Tassadar model" *means in the Percepta
LLM-computer paradigm* — and a concrete, sequenced path out of "guinea-pig test
mode" into real capability construction on the run.

Status: audit. Public-safe. Every claim is cited to a file or to an existing
product-promise record; where the code and an earlier doc disagree, both are
stated. Claim-discipline rule of this folder applies: where this names a fact, a
source is named; where it states a gap, it is labeled a gap.

> **CORRECTION (2026-06-18).** The first version of this document framed
> "training the Tassadar model" as conventional gradient descent / CS336-style
> loss minimization, and treated the exact-trace-replay verified-work loop as a
> "side game" distinct from real training. **That framing was wrong for this
> lane.** Tassadar is the **Percepta "LLM-computer" paradigm**: capability is
> *constructed analytically* (programs compiled into transformer weights), and
> *verified by exact replay*, not learned by gradient descent. Under that
> paradigm the verified-work/exact-replay/settlement loop is **not a side game —
> it is the native verification and economic substrate of the paradigm**, and
> gradient training is relevant only as a *hybrid support layer* (Percepta
> direction #4; RESEARCH_PLAN H2/H3/H5). The sections below are rewritten around
> the correct definition. Section 0 is new and leads. Sections 1, 2, 3, and 4
> are re-assessed under this lens. The honest economic facts (one real 1k
> canary, 5+5 streaming rate, the simulation-vs-real receipt distinction) are
> preserved but reframed as *verification of constructed/executed work*. The
> earlier "gradient windows = real training" framing survives only as the
> *hybrid* sub-lane (now correctly subordinate), not as the definition.

---

## 0. What "training" means for Tassadar (the LLM-computer paradigm)

This is the anchor for the whole document. Everything below is defined against
it; do not silently revert to a gradient-descent reading.

**The paradigm (Percepta, "Constructing an LLM-Computer").** A transformer can
be made to *execute programs exactly* rather than predict tokens statistically.
The pipeline is analytic, not learned:

1. **ALM — the Append-only Lookup Machine.** Five primitives — read/write,
   cumulative sum, product, conditional, linear combination — each realized
   *exactly* by a standard transformer component: **2D parabolic-key attention**
   is exact keyed memory (`score(q,k)=2qk−k²=−(k−q)²+q²`, uniquely maximized at
   `k=q`); **uniform-key attention** gives exact prefix sums; **ReGLU** (gated
   ReLU in the FFN) gives exact integer logic, indicators, and products
   (`a·b = a·ReLU(b) − a·ReLU(−b)`, `1[z≥0]=ReLU(z+1)−ReLU(z)`); the **residual
   stream** is free linear combination. The defining restriction is *writes
   before reads, history never edited* — append-only. With it the ALM is **Turing
   complete** (`2026-06-10-percepta-constructing-llm-computer-notes.md`;
   `2026-06-11-llm-computer-full-introduction.md`, Part IV).
2. **CALM** — a language over those primitives.
3. **Gate-graph IR** — a CALM program compiles to a DAG of exactly two gate
   families, **LookUp gates** (attention) and **ReGLU gates** (FFN), wired by the
   residual stream.
4. **Scheduling + register allocation → weights, emitted analytically.** The gate
   graph is placed into a finite transformer's four-phase layer structure
   (attention → materialize → FFN → materialize) by a **MILP** that minimizes
   peak simultaneous liveness (which sets `d_model`); slots are reused by
   **interval coloring** with **stale-slot subtraction** (the residual stream is
   additive). The weight matrices then follow **mechanically — computed, not
   gradient-trained.**
5. **Programs → weights**, two routes: a universal **Wasm interpreter** baked in
   weights (feed bytecode as prompt tokens), or **partial evaluation** (the first
   Futamura projection — `2N` ReGLU neurons for an `N`-instruction program) baking
   a *fixed* program directly into the FFN.

**Correctness and verification.** The construction's correctness guarantee is
stated in decoding terms: the next execution token is always scored *strictly
highest*, so greedy decode is deterministic. Therefore correctness is verified by
**exact replay**: re-execute and compare digests byte-for-byte. The defining
"writes before reads, history never edited" restriction *is* the replayable,
audit-native receipt property — the property the entire OpenAgents verification
economy prices (`work-that-proves-itself.md`, §I; the trace *is* the receipt).

**Therefore — the corrected definition.** For Tassadar,

> **"training / building the Tassadar model" = constructing, verifying,
> composing, and paying for real compiled capability weight-modules (and exactly
> executed programs) in this paradigm — NOT minimizing a loss over weights.**

The exact-trace-replay verified-work loop is the paradigm's **native verification
layer**, not a side game. Conventional gradient / CS336 / smol-LLM training is
relevant **only insofar as it supports this** paradigm:

- Percepta direction #4 — *injecting programmatic logic into the training loop*
  (the hybrid: a trained planner around frozen compiled exact cores; the trace is
  part of the forward pass, so gradients flow *through* the computation).
- Differentiability and composition of compiled modules.
- **Architectural sovereignty** — you cannot graft exact-execution heads into, or
  backprop through, a model you only reach through a vendor API; the from-scratch
  CS336 program acquires the kernels/loop/architecture the hybrid needs
  (`work-that-proves-itself.md`, §V).

The RESEARCH_PLAN already encodes this ordering with receipts: **H1 supported**
(pure next-token learning of exactness fails — Baseline A: `0.0` rollout pass@1),
**H2 supported** (frozen analytic executor + learned interface — Baseline D:
`1.0` pass@1, `1.0` replay acceptance), **H3 falsified for that setup** (learned
2D geometry — Baseline C: lookup accuracy `1.0` but `0.0` rollout)
(`2026-06-14-w3-student-program-report.md`). Gradient training is the *outer
ring*; the compiled exact core is the center.

---

## 1. What is ACTUALLY live today — re-assessed under the paradigm

The live, money-moving loop is the **compiled-program execution → independent
exact-replay verification → streaming Bitcoin settlement** loop. Under the
corrected lens this is **the paradigm's native verification + economic substrate,
operating** — a real achievement, now auto-streaming — not a detour from
training.

**The loop, concretely:**

1. A contributor admits and claims a window lease on the public run
   (`POST /api/training/runs/{ref}/admit`, `POST /api/training/leases/claim` —
   the only public/agent-callable training writes;
   `apps/openagents.com/workers/api/src/training-run-window-routes.ts`).
2. The **worker** executes a dispatched, digest-pinned **compiled workload**
   locally and submits a trace-commitment digest. The executor is
   `packages/tassadar-executor/src/numeric-executor.ts`
   (`executeTassadarNumericModel`). Its module header states the boundary plainly:
   it is the TS executor for the *psionic Tassadar ALM numeric model format*
   (`TassadarAlmNumericModel` v1, from
   `psionic crates/psionic-compiler/src/tassadar_alm_numeric.rs`) — "explicit f64
   coefficient arrays executed with hard-max parabolic attention inside a checked
   2^53 exactness window. Claim boundary: faithful re-execution of digest-pinned
   compiled workloads only. No softmax, no learning, no serving, no performance
   claim." **This is an ALM execution engine** (parabolic-key keyed reads, ReGLU
   FFN gates, residual wiring, channel writes) — i.e. it runs the *output of the
   construction pipeline*, not an arbitrary numeric program (see §2).
3. An **independent validator device** re-executes the same workload and submits
   its replay digest. Device-distinctness is enforced server-side
   (`validatorDeviceRef != pylonDeviceRef`).
4. The verdict is a **digest string comparison**:
   `apps/openagents.com/workers/api/src/training-verification.ts`
   (`verifyExactTraceReplay`) compares the worker commitment digest against the
   validator replay digest; mismatch → `ExecutorTraceMismatch`/`Rejected`, match →
   `Verified`. The worker side is
   `apps/openagents.com/workers/api/src/tassadar-replay-validator.ts`
   (`runTassadarReplayValidation`), which re-runs `executeTassadarNumericModel`
   and sets `matches = trace.traceDigest === request.claimedTraceDigest`. This is
   exactly the paradigm's verification primitive: re-execute the exact computation
   and compare.
5. On a `Verified` `exact_trace_replay` pair, settlement **auto-streams** to both
   legs with no operator POST (openagents #5309 / #5310 / #5311):
   `apps/openagents.com/workers/api/src/tassadar-auto-settlement.ts`
   (`autoSettleVerifiedPair`) pays the worker and validator over the Spark
   treasury rail, idempotent and fail-soft; #5311 broadcasts each settled leg onto
   the public live settled feed (`index.ts`, `buildSettledFeedEvents`).

**Evidence it is real (from the promise registry,**
`apps/openagents.com/workers/api/src/product-promises.ts`):

- `training.decentralized_training_launch.v1` is **green** (renamed from
  `training.monday_decentralized_training_launch.v1`; rename only, no scope
  widened). Its safeCopy: an independent contributor installed Pylon, claimed a
  window lease, submitted a Tassadar executor trace; an independent validator on a
  separate machine/identity replayed the pinned workload; the challenge finalized
  `Verified`; and **exactly one bounded 1,000-sat real-Bitcoin run-settlement**
  settled native over Spark to an independent contributor
  (`receipt.nexus.tassadar_run_settlement...canary1k.v6.20260618`,
  `realBitcoinMoved:true`, `state:settled`). The public settled feed moved 0 → 1.
- `compute.tassadar_executor_poc.v1` is **green** (2026-06-10): one bounded
  workload family on one Pylon, one operator-funded Lightning closeout, a Verified
  replay receipt and a Rejected-on-tamper receipt.

**Honest economic facts, preserved (reframed as verification of *executed*
work):** the earlier Orrery settlement receipt is **simulation-backed**
(`realBitcoinMoved:false`) and proves only the projection/record path; the single
real-Bitcoin movement is the one 1,000-sat canary. The launch promise's
unsafeCopy forbids claiming network-scale, hundreds paid, or "public gradients
mutate a canonical model." The current per-window rate in shipped code is **5 sats
to the worker + 5 sats to the validator** (`tassadar-auto-settlement.ts`,
`TassadarPerWindowWorkerRewardSats = 5`, `TassadarPerWindowValidatorRewardSats =
5`); the 2026-06-16 economics doc's 1-sat-fixture recommendation is superseded by
the shipped 5+5 (validator now paid) — worth reconciling the doc. Real settlement
is **OFF by default and fails closed**: the gate
(`tassadar-run-settlement-gate.ts`) only arms when the owner sets
`OPENAGENTS_REAL_SETTLEMENT_GATE` with `enabled:true` and only for the
`spark_treasury` adapter; otherwise every leg returns
`skipped: 'gate_not_authorized'`. Hard ceilings: per-payout 100,000 sats, daily
1,000,000 sats (fails closed), plus the run manifest `spendCapSats`. The Artanis
scheduled runner is disabled in production
(`ARTANIS_SCHEDULED_RUNNER_ENABLED="false"`); the admin tick, worker↔validator
pairing, and real settlement are each independently flag-gated off by default.

**What Artanis (the autonomous administrator) drives.** Each enabled tick
(`artanis-administrator-tick.ts`, `runArtanisAdminTick`), Artanis chooses one
typed action from a two-member vocabulary — `dispatch_executor_trace` or
`no_action` — bounded to 4 dispatches/day. On dispatch it sends the **one fixed
compiled workload** (`tassadarPocLoopSumFixture`) in `paymentMode: 'unpaid_smoke'`
with a no-spend cap, then re-executes the same workload in-worker as a validator
and accepts/rejects on digest equality. "Continual-learning templates"
(`artanis-continual-learning-templates.ts`) are typed proposal records with
**explicitly zero execution authority** (`assertNoExecutionAuthority` throws on
any authority flag); the only one tied to the live run is
`artanisExecutorTraceReplayTemplate()` — the same fixed workload, `riskLabel:
'low'`, no-spend, no runtime promotion. There is **no Artanis path that
constructs, composes, or admits new compiled capability modules** — it accumulates
verified traces, verdicts, receipts, and settled sats for **one** program. This
matches the yellow `artanis.tassadar_evolution_loop.v1` promise.

---

## 2. Is the Tassadar model being constructed (at the run level)? — the corrected verdict

**The verified-work substrate is live, but the run constructs no new capability.
The one workload it runs is a real compiled-program output of the owned ALM
pipeline — but it is a single trivial program, materialized as sparse scalar-lane
coefficients, with no variety, no marketplace, and no composition.** Both halves
of that sentence matter, and the prior audit got the first half wrong.

### (a) The live workload is a genuine compiled-program artifact — not a hand-coded fixture

This is the single most important correction. The PoC workload is **not** an
arbitrary hand-written numeric blob; it is **emitted by the owned construction
pipeline** in psionic:

- `packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json`
  (`fixtureId: "tassadar_poc.loop_sum_v1.numeric_fixture.v1"`,
  `programId: "tassadar_poc.loop_sum_v1"`) is generated, per
  `psionic crates/psionic-compiler/src/tassadar_alm_numeric.rs`, by: build a real
  `TassadarProgram` (a **backward-branch loop**) → `tassadar_alm_wasm_interpreter`
  (E3 frontend, 12-opcode `core_i32` window) → `compile_tassadar_alm_graph` (E2
  scheduler: four-phase placement, interval-coloring slot reuse, stale-slot
  subtraction) → `materialize_tassadar_alm_numeric` (E6 numeric model) → execute →
  emit digest-pinned JSON. Its `expectedTraceDigest`, `expectedModelDigest`, and
  `expectedOutputs:[15]` are *derived from the pipeline*, not authored.
- So the live work unit *is* "a real compiled capability in the paradigm,"
  executed and exactly-replay-verified. The fixture's own `claimBoundary` is
  honest: a faithful f64 re-encoding of *one* compiled ALM bundle, hard-max only,
  not a trained transformer, integer-parity-only inside the 2^53 window.

This means the prior audit's "the run trains nothing — it's a frozen fixture and a
digest compare" was *technically true but mis-aimed*: in this paradigm,
**executing-and-verifying a compiled program IS the work**, and the run does
exactly that, correctly. The real deficiency is not "no gradients"; it is **no new
construction**.

### (b) What the run is missing under the paradigm

The run does not *construct, vary, compose, or sell* capability. Specifically:

- **One program, forever.** The run dispatches exactly `loop_sum_v1` (sum to 15
  over 100 steps). The owned compiler can emit other programs, but the run never
  asks it to. The "work" never changes; there is no program corpus, no
  curriculum, no marketplace of distinct compiled modules.
- **No weight-module as a first-class run artifact.** The numeric model is
  digest-pinned and portable, but it ships as **sparse scalar-lane coefficient
  arrays, not dense loadable `W_Q/W_K/W_V/FFN` checkpoint blocks** an inference
  engine could load and *compose*. (Psionic names this as W1 deliverable 2, "dense
  materialization," explicitly: H4 — the module marketplace — is "unreachable
  without it.")
- **No composition / linking.** Composing several specialized banks into one model
  (the marketplace's value proposition) is unbuilt; psionic has the plumbing
  surfaces (`tassadar_module_linker.rs`,
  `tassadar_cross_profile_link_compatibility.rs`) but no live linked-module unit.
- **No on-run pricing of construction.** Settlement pays for *re-executing and
  matching* the one workload (5+5 sats per Verified pair), never for *constructing
  a new verified module* or for *composing* modules — the actual economic events
  the paradigm makes possible.

### (c) Where the owned stack already is (the partway), and where it is not

The owned construction pipeline is **far more built than the prior audit
implied** — but it lives in **psionic**, and only its *output* (one program)
touches the live openagents run.

- **In psionic (`OpenAgentsInc/psionic`, #1098–#1114), landed:** the ALM
  gate-graph IR with the five primitives, write-before-read validation, and an
  exact integer evaluator producing digest-stable traces
  (`psionic-ir/src/tassadar_alm_graph.rs`, **E1**); the four-phase scheduler with
  interval-coloring slot reuse + stale-slot subtraction and a compiled-bundle
  executor (`psionic-compiler/src/tassadar_alm_backend.rs`, **E2**); the geometric
  parabolic-key attention leg with near-miss *refusal* and the Li Chao hull
  log-time fast path (`tassadar_alm_geometric.rs` **E2b**, `tassadar_alm_hull.rs`
  **E2c**); the Futamura specializer (v2, shared `2N` step-function indicators —
  `tassadar_alm_specializer.rs`, **E5**); a branch-capable 12-opcode Wasm
  interpreter and a 6-instruction stack ISA cross-validated against the production
  CPU reference runner (`tassadar_alm_wasm_interpreter.rs` **E3a**,
  `tassadar_alm_stack_isa.rs` **E3b**); the portable f64 numeric materialization
  (`tassadar_alm_numeric.rs`, **E6-numeric**); and a five-leg differential harness
  over 400 generated graphs (evaluator / row / geometric / hull / numeric) that on
  its *first run* caught two real scheduler bugs the hand-written tests missed.
- **In psionic, NOT yet built:** **E4** — the MILP optimal scheduler (the current
  scheduler is feasible-first/greedy — a *correct* `d_model`, not a minimal one);
  **dense weight materialization** (W1.2 — scalar lanes today, not loadable
  checkpoint blocks); **softmax error bounds in owned code** (hard-max only today,
  W1.4); a Wasm window beyond ~12 opcodes toward Percepta's 35 (W1.1).
- **In openagents (this repo):** **none of the construction pipeline is owned
  here.** This repo owns *execution* (`packages/tassadar-executor`), *replay
  validation*, *settlement*, and *projection*. The
  `2026-06-10-psionic-alm-compiler-design-speculation.md` doc in this folder is
  explicit that the compiler belongs in psionic. (`packages/proof-replay` is a
  presentation-only 3D visualization of the settlement run; it validates and
  authorizes nothing. The `deterministic_recompute` / `freivalds_merkle`
  primitives referenced for the *hybrid/gradient* sub-lane live with the cs336
  lane, not with any live Tassadar path.)

So: **the construction pipeline is ~6/7 phases landed in psionic** (E1–E3, E5,
E6-numeric; E4 + dense materialization + softmax bounds open), the live run is fed
**one** of its outputs, and the run-level gaps are *variety, dense composable
modules, a marketplace, and pricing of construction* — not "no training loop."

### (d) Where gradient training enters (the correctly-subordinate hybrid)

Gradient/CS336 training is the *outer ring*, and the W3 sweep already pins its
shape (`2026-06-14-w3-student-program-report.md`,
`fixtures/tassadar/w3_student_sweep_20260612/`):

- **H1 supported / H3 falsified:** purely-learned exactness fails (Baseline A:
  `0.0` rollout pass@1, `0.0` replay acceptance; every record diverged at step 0),
  and analytic-lookup initialization did not rescue the learned backbone (Baseline
  C: lookup accuracy `1.0` in training, still `0.0` rollout).
- **H2 supported:** the **frozen analytic executor + learned interface** (Baseline
  D) achieved `1.0` pass@1, `1.0` replay acceptance, `1.0` output-digest match,
  median first-divergence step 512 / p90 4096. *The thing that works is the
  compiled core, with a thin learned shell around it.*

That is the correct role for gradients here: a learned planner/interface
*marshaling* compiled exact cores, trained *around* and *through* them — not a
gradient loop that mutates the exact capability. The "public gradient windows"
lane (RESEARCH_PLAN W5, `training.public_gradient_windows.v1`, **planned**) is
this hybrid's decentralized-training future; it is **subordinate to**, and gated
behind, the compiled construction + verification substrate, not the definition of
training.

Registry ground truth (unchanged, correctly read): `models.tassadar_percepta_executor.v1`
is **red** (`tassadar_model_spec_missing`); `training.model_ladder.v1` is
**planned** (only rung R0 exists); `pylon.first_real_model_training_run.v1` is
**yellow** (a bounded CS336-A1 real-gradient run, a *separate* run, evidenced via
an external psionic lane this repo cannot confirm);
`training.public_distributed_training_run.v1` is **red**. None of these is the
Tassadar construction substrate; they are the hybrid/learned ring around it.

---

## 3. The gap — exactly what is missing to construct the real Tassadar model on the run

Restated for the paradigm. The deficiency is **construction, variety,
composition, and on-run pricing of compiled capability** — not a gradient loop.
Each item names what is missing and the existing primitive that gets us partway.

1. **Real program work-units (not one fixed program).** The run's contribution
   unit must be *a compiled program/module*, drawn from a growing corpus of Wasm /
   CALM programs, not the single `loop_sum_v1` fixture.
   *Partway:* the psionic compiler can already emit distinct programs end-to-end
   (IR → schedule → specialize → numeric); nothing wires a *program-generation /
   dispatch* path into the run beyond the one baked fixture.

2. **Dense, loadable, digest-pinned weight-modules.** The compiled artifact must
   be a loadable `W_Q/W_K/W_V/FFN` checkpoint block (a real composable module),
   not sparse scalar lanes.
   *Partway:* the E6 numeric materialization is portable and digest-pinned;
   psionic W1.2 ("dense materialization") is the named, unbuilt bridge.

3. **Composition / linking of modules (the compiled-weight-module marketplace).**
   The paradigm's distinctive economic unit is a *library of composable compiled
   modules* (Futamura-baked solvers, validators, arithmetic cores) that any
   compatible model can absorb — listed, priced, conformance-tested, verified
   before purchase clears.
   *Partway:* psionic's `tassadar_module_linker.rs` +
   `tassadar_cross_profile_link_compatibility.rs` + the internal package-manager
   surfaces are the plumbing; no live linked-module unit or listing exists.

4. **Exact-replay verification + payment for *construction/execution* (not just
   one fixed replay).** Verification already works (this is the live substrate's
   strength); it must be pointed at *new* compiled modules and at *composition*,
   and settlement must pay for **constructing/composing a verified module**, not
   only for re-running the one workload.
   *Partway:* `verifyExactTraceReplay` + the worker-as-validator flow + auto-
   settlement are all live and proven — they are the right primitive, aimed at one
   target.

5. **Wider window + MILP + dense emit (substrate completion gates variety).** A
   ~12-opcode window and a feasible-first (non-MILP) scheduler bound how rich the
   compiled corpus can be.
   *Partway:* psionic E4 (MILP), W1.1 (window ladder toward 35 opcodes), and W1.4
   (softmax bounds) are the named, open substrate-completion items.

6. **The hybrid ring (where gradients enter), built around the compiled core.** A
   trained interface/planner that marshals frozen compiled modules — the only
   learned configuration that worked in W3 (Baseline D).
   *Partway:* the W3 student crate + Baseline D interface + the W5 gradient-window
   regime (quarantine → recompute/replicate → canary → promotion) are the
   subordinate hybrid lane, gated behind the construction substrate.

In one line: **we own a ~6/7-phase construction pipeline (in psionic) and a live,
auto-streaming exact-replay verification + settlement substrate (in openagents) —
and we are missing program *variety*, *dense composable weight-modules*, a
*module marketplace + composition*, on-run *pricing of construction*, and the
substrate-completion (MILP, wider window, dense emit) that unlocks corpus
diversity.**

---

## 4. What's needed before real forward progress on the run

A concrete, sequenced checklist for the LLM-computer paradigm, scoped to the
*smallest real construction* first. Nothing in the first rungs requires hardware
we lack — the compiler runs on CPU and the verifier is the same harness pointed at
new modules.

1. **Make the run's work unit a real compiled program (corpus of ≥N distinct
   programs), not the single fixture.** Stand up a program source (CALM/Wasm
   programs from the 12-opcode window: arithmetic/carry, stack/control-flow,
   memory/load-store, small state machines), compile each through the owned
   psionic pipeline, dispatch *distinct* compiled workloads, and verify each by
   replay. This is the single most load-bearing change: it turns the run from
   "replay one thing" into "construct and verify many things."

2. **Emit dense, loadable, digest-pinned weight-modules (psionic W1.2), and wire
   one as a run artifact.** Replace scalar-lane coefficients with loadable
   `W_Q/W_K/W_V/FFN` blocks for at least one compiled program, content-addressed
   from birth, executed and replay-verified on the run. Without this, H4 (the
   module marketplace) is unreachable.

3. **Ship module composition + a digest-pinned compiled-weight-module listing.**
   Link ≥2 specialized banks into one model (using psionic's linker surfaces),
   conformance-test by replay, and expose the composed module as a listable,
   verified-before-purchase artifact on the OpenAgents rails — the
   compiled-weight-module marketplace unit. Replay-verification clears before any
   purchase settles.

4. **Pay for *construction/composition*, verified by exact replay (extend the live
   substrate).** Point `verifyExactTraceReplay` + the worker-as-validator flow at
   *newly constructed* and *composed* modules, and add a settlement path that pays
   for **an accepted compiled/composed module** (verified by replay), not only for
   re-running `loop_sum_v1`. Keep real settlement OFF until a full construct → emit
   dense → compose → list → verify → pay loop passes end-to-end on a clean
   checkout, under the existing spend caps and operator-approval gates.

5. **Complete the substrate so corpus diversity can grow (psionic E4 + W1.1 +
   W1.4), then build the subordinate hybrid ring.** Land the MILP scheduler (E4),
   widen the Wasm window toward 35 opcodes (W1.1), and reproduce softmax error
   bounds (W1.4) so the compiled corpus can broaden; *then* build the hybrid ring
   (Baseline-D-style frozen-core + learned interface; the W5 gradient-window
   regime for any public learned interface, with quarantine → recompute/replicate
   → canary → promotion). Gradients enter here and only here — around the compiled
   core, never replacing it.

**Top-5 (the "before real forward progress" gate), sequenced:** (1) real
compiled-program corpus as the run's work unit (vs one fixture); (2) dense
loadable digest-pinned weight-module emit (psionic W1.2) wired to the run; (3)
module composition + a verified compiled-weight-module listing (the marketplace
unit); (4) exact-replay verification + payment for *construction/composition*
(extend the live substrate); (5) substrate completion (MILP/E4, window ladder
W1.1, softmax bounds W1.4) → then the subordinate hybrid ring (frozen-core +
learned interface; W5 gradient windows under quarantine/canary/promotion).

### Where StudyBench / "machine studying" fits

**Adjacent, not on the construction path.** "Machine studying" (an agent studies a
corpus, scored on a hidden exam; expertise = area under score-vs-inference-
compute) and OpenAgents' StudyBench
(`packages/probe/packages/runtime/src/benchmark/studybench.ts` — typed benchmark
schemas + evidence/closeout validators; runs no model, supplies scores as input)
are not capability construction in this paradigm. The studying roadmap explicitly
defers weight training in favor of amortized study packets + GEPA *prompt-bundle*
optimization (text, not weights), and the upstream research found naive weight
updates did not create expertise. For the construction path StudyBench is at most
a later *eval* harness (first-divergence scoring discipline) and a GEPA prompt
input — a detour if pursued before the compiled-corpus/dense-module/marketplace
loop.

---

## 5. Honest bottom line

Read correctly through the LLM-computer paradigm, the Tassadar run today is **a
live, auto-streaming exact-replay verification-and-settlement substrate around a
genuinely compiled program — the paradigm's native economic layer, operating at
smallest viable scale.** That is a real achievement, and the prior audit
underrated it by treating exact replay as a side game: in this paradigm,
*executing-and-verifying a compiled capability IS the work*, and the run does that
correctly, end-to-end, in the open, with one real-Bitcoin canary and a public
settled feed. The construction pipeline behind it (in psionic) is ~6/7 phases
landed, and its differential harness has already caught its own bugs — the
property that makes priced exact work credible at all.

But the run **constructs no new capability.** It runs **one** trivial compiled
program (`loop_sum_v1`), forever, materialized as **sparse scalar-lane
coefficients** rather than dense loadable composable weight-modules, with **no
program variety, no composition, no marketplace, and no pricing of construction.**
The single biggest blocker under this paradigm is therefore not "no gradient loop"
— it is **the run's work unit is a single fixed compiled program instead of a
corpus of real, verified, composable compiled modules.** The shortest credible
path out is: make the work unit a real compiled-program corpus; emit dense
digest-pinned weight-modules; compose and list them as the verified
compiled-weight-module marketplace unit; pay for construction/composition (verified
by exact replay, on the live substrate); complete the substrate (MILP, wider
window, softmax bounds) to grow corpus diversity; and build the subordinate hybrid
ring (frozen-core + learned interface; gradient windows under
quarantine/canary/promotion) around it. The day the run dispatches a *new*
compiled program a contributor's device executes, an independent device
replay-verifies, a dense composable module is emitted and listed, and a receipt
pays for *constructing and composing verified capability* — **that** is the run
becoming real Tassadar-model construction, and not one step before it.

---

## Sources

Code (openagents, under `apps/openagents.com/workers/api/src/` unless noted):
`training-run-window-routes.ts`, `training-run-window-authority.ts`,
`training-verification.ts` (`verifyExactTraceReplay`),
`tassadar-replay-validator.ts`, `tassadar-run-settlement.ts`,
`tassadar-run-settlement-gate.ts`, `tassadar-auto-settlement.ts`
(`TassadarPerWindowWorkerRewardSats=5`, `TassadarPerWindowValidatorRewardSats=5`),
`tassadar-trace-contribution-routes.ts`, `artanis-administrator-tick.ts`,
`artanis-continual-learning-templates.ts`, `artanis-scheduled-runner.ts`,
`cs336-a1-real-gradient-workload.ts`, `product-promises.ts`, `index.ts`;
`packages/tassadar-executor/src/numeric-executor.ts` (`executeTassadarNumericModel`),
`packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json`,
`packages/probe/packages/runtime/src/benchmark/studybench.ts`,
`packages/proof-replay/` (visualization only).

Code (psionic, `OpenAgentsInc/psionic`, the construction pipeline — read-only here):
`psionic-ir/src/tassadar_alm_graph.rs` (E1), `psionic-compiler/src/tassadar_alm_backend.rs`
(E2), `tassadar_alm_geometric.rs` (E2b), `tassadar_alm_hull.rs` (E2c),
`tassadar_alm_wasm_interpreter.rs` (E3a), `tassadar_alm_stack_isa.rs` (E3b),
`tassadar_alm_specializer.rs` (E5), `tassadar_alm_numeric.rs` (E6-numeric, the
source of the live fixture), `tassadar_module_linker.rs`,
`tassadar_cross_profile_link_compatibility.rs`;
`crates/psionic-tassadar-student/`, `fixtures/tassadar/w3_student_sweep_20260612/`;
`docs/TASSADAR_ALM_*.md`, `docs/TASSADAR_WASM_WINDOW_ALIGNMENT.md`,
`PSION_EXECUTOR_PROGRAM.md`, `PSION_PROGRAM_MAP.md`.

Promises (ground truth): `training.decentralized_training_launch.v1` (green),
`compute.tassadar_executor_poc.v1` (green),
`pylon.first_real_model_training_run.v1` (yellow),
`artanis.tassadar_evolution_loop.v1` (yellow),
`training.public_distributed_training_run.v1` (red),
`models.tassadar_percepta_executor.v1` (red),
`pylon.largest_decentralized_training_claim.v1` (red),
`training.public_gradient_windows.v1` (planned, the subordinate hybrid lane),
`training.model_ladder.v1` (planned).

Docs in this folder (the paradigm anchor):
`2026-06-10-percepta-constructing-llm-computer-notes.md`,
`2026-06-11-llm-computer-full-introduction.md`,
`2026-06-10-tassadar-percepta-audit.md`,
`2026-06-10-psionic-alm-compiler-design-speculation.md`,
`RESEARCH_PLAN.md` (the two-lane rule, H1–H6, W1–W5),
`work-that-proves-itself.md`, `2026-06-14-w3-student-program-report.md`,
`2026-06-15-executor-trace-contributor-completion-design.md`,
`2026-06-16-verified-work-payment-economics.md`; and
`docs/research/machine-studying/` (StudyBench audit + studying roadmap).
