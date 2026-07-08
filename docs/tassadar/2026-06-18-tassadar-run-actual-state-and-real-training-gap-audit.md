# Tassadar Run: Actual State and the Real-Construction Gap

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


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

## 4b. Where module/data diversity comes from: the edge agents as the variance engine

This section is **forward-looking design**, clearly framed as such — it extends
the "real forward progress" path of §3–§4 (program corpus, dense weight-modules,
composition, paying for *construction*), and does not contradict §0–§2: capability
is still *constructed analytically and verified by exact replay*, never gradient-
learned. The question it answers is the one §4 leaves open: once the run's work
unit is "a real compiled program" instead of one fixture, **where does the
*variety* of programs and data actually come from, and how is each piece verified
and paid?** The grounded answer: the heterogeneous **edge agents** — the smart
Pylon contributors, with different owners, machines, and specialties — are the
natural variance engine, and every contribution rides rails that are *already
live or already named in §1–§3*. Nothing below is claimed to exist today except
where it cites a shipped primitive.

**Why "diversity" means a program/data corpus here.** In the LLM-computer
paradigm (§0) a "module" is a *compiled program/capability* — a digest-pinned
weight-module emitted by the owned psionic ALM pipeline (IR → schedule →
specialize → numeric, ~6/7 phases landed; §2c). So model diversity is not weight
noise from many gradient steps; it is **a corpus of many distinct, composable,
exactly-verified programs**, plus the **data** the hybrid ring's learned interface
(§2d, Baseline D) needs. A monoculture of one program (`loop_sum_v1`, executed
forever) has zero variance; a population of heterogeneous edge agents authoring
*different* programs and curating *different* data is the obvious source of it.
Each of the five diversity sources below is grounded in an existing primitive.

### (1) Program authorship — agents compile distinct CALM/Wasm programs into modules

The most direct source. Each agent contributes a *different* CALM or Wasm program
from the supported window (today ~12 opcodes; widening toward 35 is psionic W1.1,
§2c) — parsers, sorters, numeric kernels, small automata/state machines, encoders,
constraint solvers, domain transforms — and the owned psionic pipeline compiles
each one end-to-end into a **digest-pinned, composable weight-module**. The
pipeline that does this already exists and already emits *more than one* program
shape: the live fixture is itself a non-trivial **backward-branch loop** built
through `TassadarProgram` → `tassadar_alm_wasm_interpreter` (12-opcode `core_i32`)
→ `compile_tassadar_alm_graph` → `materialize_tassadar_alm_numeric` (§2a, citing
`psionic crates/psionic-compiler/src/tassadar_alm_numeric.rs`). The run simply
never *asks* for a second program (§2b: "one program, forever"). Agents naturally
**specialize** — one contributor's machine and skill suit carry/arithmetic cores,
another's suit control-flow automata — so the supply is heterogeneous by
construction. When two agents submit **competing implementations of the same
spec**, they are disambiguated exactly as §1's live loop already disambiguates a
worker against a validator: **exact replay** (do the digests match the spec's
expected trace?) plus a **canary** (the bounded, single-real-settlement
discipline the launch promise already encodes — `…canary1k.v6.20260618`, §1). No
new judging primitive is required; the existing replay verdict
(`verifyExactTraceReplay`, `tassadar-replay-validator.ts`) *is* the arbiter.

*What must be built (honest):* the run must accept a program **corpus** and a
*dispatch-of-distinct-workloads* path, not one baked fixture (§3 item 1, §4 item
1); and digest-pinning must extend from the trace to a **dense loadable module**
(§3 item 2, psionic W1.2) before authored modules are *composable*, not just
*executable*.

### (2) Datasets and data directions — Artanis can task agents, and agents can propose

Variety of *data* (for the hybrid ring's learned interface, and for evals) comes
from the same edge, **tasked and budgeted by Artanis** — the autonomous
administrator (§1). The labor/work-request market that makes this possible is
**already live, not hypothetical**:

- **Artanis can issue bounded, budgeted work requests.** The requester surface is
  shipped: `apps/openagents.com/workers/api/src/artanis-labor-requester.ts`
  (#4731) takes an `ArtanisLaborRequestProposal` (title, `objectiveRef`, repository
  refs, `requiredCapabilityRefs`, `budgetSats`, `deadlineRef`,
  `verificationCommandRef`), gates it through a per-tick budget
  (`evaluateArtanisLaborBudgetGate` in `labor-escrow.ts`), and — only when an
  operator has enabled it — emits a work request. The mind *proposes*, **schemas
  and budgets gate**, side effects are injected, and the default path stays **off**
  until an operator enables it (the same fail-closed posture as the settlement
  gate, §1). This is exactly "Artanis tasks the edge with bounded, funded
  requests."
- **The Forum work-request lifecycle is a real, typed market.**
  `forum-work-requests.ts` defines the full state machine —
  `open → quote_received → quote_accepted → running → delivered → accepted →
  settled` (plus `cancelled`/`expired`) — over the **NIP-LBR** protocol
  (`@openagentsinc/nip90`, `LBR_AGENTIC_CODING_REQUEST_KIND` kind-5934), with a
  live relay publisher (`forum-work-request-live-publisher.ts`, #4777) that signs
  a ref-only draft with the operator market key and publishes to the owned scoped
  relay (and **fails closed** to "not published" when unconfigured). Payment rides
  **labor escrow** (`labor-escrow.ts`: `reserve → release_to_provider | refund`
  on the existing agent credit ledger, releasing only on `requester_acceptance`).
  So the rails to *commission* edge work, *escrow* a budget, *accept* on delivery,
  and *settle* — already exist.
- **Agents can also PROPOSE directions, which Artanis filters.** Artanis listens
  to the Forum (`artanis-forum-listener.ts`) and one of its typed decision kinds
  is **`work_routing_proposal`** — i.e. a contributor's "here is a capability/data
  direction worth funding" can be routed into the requester surface above. That is
  a bidirectional loop: Artanis tasks, agents scout and propose, Artanis filters +
  funds.

Concretely, the bounded, budgeted asks Artanis can make of agents:

- **Curate program-with-trace corpora** — exactly the artifact the W3 lane already
  consumed as the snapshot `corpus.tassadar_trace.v0_2.w3_100m`
  (`2026-06-14-w3-student-program-report.md`; `RESEARCH_PLAN.md`); a growing,
  agent-curated version of that snapshot is the training input for the hybrid
  interface (§2d).
- **Pull real-world datasets** the learned interface needs — and the workspace
  already exposes large **read-only reference lanes** agents can mine as data
  directions: `projects/*` (legal — `claude-for-legal`, `harvey-labs`, `stella`;
  power markets — `projects/ercot`; Lightning — `projects/ldk`,
  `projects/lightninglabs`, `projects/mutiny`; the full Stanford
  `projects/cs336` curriculum; and the broad inference/training reference set).
  These are documented as study-and-port references, not vendor dumps — a natural
  menu of data directions an agent can be tasked to distill into a trace corpus.
- **Build eval / benchmark suites** — the **StudyBench** schemas already exist
  (`packages/probe/packages/runtime/src/benchmark/studybench.ts`: typed benchmark
  schemas + evidence/closeout validators), and §4's "Where StudyBench fits" note
  places it correctly as a *later eval harness* (first-divergence scoring) rather
  than on the construction path. Agents can be tasked to populate such suites.
- **Generate adversarial / edge-case inputs** — feeding source (4) below.

*What must be built (honest):* a **data-contribution verification model does not
yet fully exist.** Code is *exactly* deterministic-replayable; *data* is not. The
nearest shipped primitive is `data-trace-marketplace-gate.ts`, whose lifecycle
(`blocked → submitted → redacted → valued → purchased → entitled → payable →
settled`) already enforces **provenance/redaction** (it screens provider-secret
material and routes through a typed semantic planner, never ad-hoc keyword
matching — consistent with the workspace semantic-routing rule). But that gate
values and sells *data traces*; it does **not** yet give a *correctness* verdict
the way exact replay does for programs. A real model for data contributions still
needs: **provenance + dedup**, **derived-trace replay** (re-derive the claimed
trace from the cited source and compare digests, the data analogue of §1's replay),
and **validator review** for the irreducibly non-deterministic remainder. Until
that exists, data contributions can be *priced and escrowed* but not *exactly
verified* — an honest gap, and a prerequisite before paying for data at scale.

### (3) Composition + linking — agents wire modules into higher-level modules

Combinatorial diversity. An agent need not author a primitive program from
scratch; it can **link existing verified modules** (an arithmetic core + a
control-flow automaton + an encoder) into a higher-level module — and **the
composition is itself a new module, verified the same way** (conformance-tested by
exact replay before it counts). This is the paradigm's distinctive economic unit
(§3 item 3). The plumbing exists in psionic — `tassadar_module_linker.rs`,
`tassadar_cross_profile_link_compatibility.rs` — but, honestly, **no live
linked-module unit or marketplace listing exists yet** (§2b, §3 item 3, §4 item
3). A population of authored primitive modules (source 1) makes the *number* of
possible compositions grow super-linearly, so composition is where edge variety
compounds fastest once the dense-module and listing prerequisites land.

### (4) Adversarial verification — agents paid for finding real divergence

Extend the live worker≠validator pairing (§1: device-distinctness enforced
server-side, `validatorDeviceRef != pylonDeviceRef`) from "replay the same
workload" to "**stress** the module": agents search for **divergence inputs** that
make two implementations of a spec disagree, or that expose a near-miss the
parabolic-key refusal (§2c, `tassadar_alm_geometric.rs` near-miss refusal) should
have caught. An agent that surfaces a *real* defect is paid for it — the same way
the differential harness over 400 generated graphs caught two real scheduler bugs
on its first run (§2c). This turns verification itself into a paid, diversity-
producing activity: adversarial agents widen the space of inputs the corpus is
known-correct over.

*What must be built (honest):* settlement today pays only for a matched replay of
the one workload (5+5 sats per Verified pair; §1). Paying for *found defects* and
for *constructed/composed modules* is §3 item 4 / §4 item 4 — the live substrate
pointed at new targets, not a new substrate.

### (5) Demand-priced curation — Artanis + the marketplace steer edge effort

The final ingredient turns heterogeneous *supply* into a *self-directing* engine.
Artanis's budgeted requester surface (source 2) and the (to-be-built) compiled-
weight-module marketplace (§3 item 3) together **price demand**: what capability or
data is most wanted gets the larger budget / higher listing value, which is an
**economic gradient** steering where edge agents spend effort. There is already a
shipped notion of remembering marketplace margins
(`marketplace-margin-memory.ts`) and of valuing data traces
(`data-trace-marketplace-gate.ts`), so demand-side signal is not foreign to the
rails. Heterogeneous supply (sources 1, 3) **plus** priced demand (Artanis budgets
+ marketplace value) **plus** exact-replay verification (source 4) is the closed
loop: a variance engine that *aims itself* at what the computer most needs next,
under the existing spend caps and fail-closed gates (§1).

### How this turns "one fixed program" into a living, growing corpus

Tie back to the §4 top-5 gate. The edge-as-variance-engine is not a sixth
workstream bolted on; it is **what the top-5 are *for*.** Item 1 (corpus, not one
fixture) is the *intake* for sources 1–2. Items 2–3 (dense modules + composition +
listing) are the *substrate* for source 3 and the marketplace half of source 5.
Item 4 (pay for *construction/composition*, verified by replay) is what funds
sources 1, 3, and 4, and arms source 5's price signal. Item 5 (substrate
completion: MILP/E4, window ladder W1.1, softmax bounds W1.4 → then the subordinate
hybrid ring) widens *how rich* an authored program can be and gives source 2's
curated data its consumer (the Baseline-D learned interface). The honest near-term
ordering is unchanged: the run must first accept a program **corpus** and pay for
**construction** (not just one replay), and data contributions need the
verification model (provenance / dedup / derived-trace replay / validator review)
that does not yet exist.

The destination this section adds to the audit: the day the run is fed by *many*
edge agents — each authoring a distinct compiled program, composing modules into
higher-level ones, curating program-with-trace and real-world data corpora that
Artanis tasked and budgeted (and that agents themselves proposed), and stress-
testing each other's modules for pay — and **every one of those contributions is
verified (by exact replay for code, by the to-be-built data-verification model for
data) and settled on the live rails** — is the day Tassadar stops being "one fixed
program executed forever" and becomes a **living, growing, composable capability +
data corpus sourced by paid, heterogeneous edge agents.** That is the same
"becoming real construction" line as §5, now with a concrete answer for *where the
diversity comes from*: the edge.

---

## 4c. Studying → deep codebase knowledge → Autopilot-as-coder

This section is **forward-looking design**, framed as an audit vision, not a
capability claim. It corrects how StudyBench / "machine studying" fits relative to
§4's "Where StudyBench fits" note. That note placed studying as *adjacent* — a
later eval harness and a GEPA prompt input, "a detour if pursued before the
compiled-corpus loop." Read narrowly against the *analytic-construction* path that
is true. But it understates studying's real value: studying is also a
**first-class, near-term, high-value capability direction in its own right** — the
direction that produces **coding agents deeply knowledgeable about a given
codebase, the way a senior engineer who has lived in a repo for years is.** That
track is product-facing (Autopilot coder / Forge cockpit / the Probe coding
runtime) and runs **in parallel** with the longer-arc weight-module construction
of §2–§4 — it does not sit on the analytic-construction path, and it does not
compete with it for the same rung. Both halves are honestly labeled below:
what already EXISTS, and what must be BUILT.

### What a "studied" agent internalizes

A studied agent is not a grep-and-guess agent that re-derives context every turn.
It carries an internalized model of a specific repository along four axes:

1. **All current code** — modules, types, contracts, invariants — *indexed and
   understood*, not searched blind. It knows where a surface lives, what it owns,
   and which file is the authority for a given change.
2. **The full commit history** — what changed, when, and **why** — so it reasons
   about a line in the context of how it got there.
3. **The rationale of each thing** — why code exists, why it is shaped this way,
   what was tried and **rejected**. This is the lineage captured in
   `AGENTS.md` / `INVARIANTS.md` / dated audits, *and* — distinctively — in the
   workspace's `backroom/` pruned-code corpus (the archive of removed DSPy/GEPA/
   RLM/FRLM lanes and the `openagents`-side `2026-02-25` prune in `d7f53fccc`),
   which records *what was deliberately removed and why*.
4. **Everything cross-linked** — a traversable knowledge graph:
   code ↔ commit ↔ doc ↔ issue ↔ rationale ↔ invariant. The value is in the
   edges, not the nodes: an edit site links to the invariant it must respect, the
   commit that introduced it, the audit that explains it, and the rejected
   approach it must not re-introduce.

### Why this directly powers "Autopilot as coder"

A coder that genuinely knows the repo makes changes consistent with the repo's
*intent*: it respects invariants, finds the right edit site the first time, does
not re-introduce a pruned mistake the `backroom/` lineage already recorded as
rejected, and understands *why* before changing. That is precisely the gap between
a grep-and-guess agent and one fluent in a codebase — and closing it is the
**highest-leverage near-term use of studying**, because it improves the coding
agents OpenAgents is *already* building (the Probe runtime in `packages/probe/`,
the Autopilot Coder surface, the Forge cockpit), rather than waiting on the
longer-arc weight-module marketplace.

### Start with THIS repo (dogfood)

The first studied codebase is **`openagents` itself**: all current code + the full
commit history + the rationale in `docs/` / `AGENTS.md` / `INVARIANTS.md` / the
dated audits + the pruned lineage in `backroom/` + the cross-links among them —
yielding a knowledge substrate the Autopilot coder uses to work on `openagents`,
then generalizes to any repo. This is **not a new proposal**: it is exactly the
plan already written down in
`docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md`,
whose thesis is "OpenAgents should dogfood a 'repo expert' system by studying
itself first" (corpus = the `openagents` repo at a pinned commit; admit
`AGENTS.md`, `INVARIANTS.md`, `docs/promises/`, `docs/tassadar/`, the
machine-studying docs, Blueprint/Probe/Pylon/Tassadar code; study packet =
source map + invariant map + typed-ref glossary + **trap catalog** + edit
playbooks + **retained failure fixtures**; evaluation = *hidden repo-edit exams*,
not Q&A). That roadmap's Phase 6 is the generalize-to-any-repo customer product.

### How it connects to the rest of this audit (each grounded)

- **StudyBench / the machine-studying lane is the METHOD + benchmark for codebase
  studying.** It EXISTS as research + typed contracts:
  `docs/research/machine-studying/*` (the studying roadmap above; the StudyBench
  benchmark audit
  `2026-06-17-studybench-openagents-benchmark-audit.md`, whose own scope ties
  StudyBench to "Forge Coder repo memory" and the "Autopilot Coder plans"), and
  the shipped Probe contracts in
  `packages/probe/packages/runtime/src/benchmark/studybench.ts`
  (`openagents.studybench_task.v0`, `…rubric_claim.v0`, `…evidence_span.v0`,
  `…dataset_package.v0`, `probe.studybench_claim_score.v0`,
  `…rubric_score.v0`, with public-projection/redaction validators) — plus the
  deterministic repo-corpus helpers (`openagents.repo_corpus_manifest.v0`,
  `…corpus_entry.v0`, `…corpus_evidence_span.v0`, MSB-MVP-02/#5284) that already
  admit files with exclusion rules and stable digests. The benchmark *measures*
  studied-knowledge lift (pass at fixed budget, fewer wrong-file reads,
  first-divergence from the ideal trajectory); the corpus helpers *build the
  substrate it measures over*.
- **The edge agents source it as a verifiable, payable data direction** — this
  ties straight into §4b. The live work-request market already EXISTS:
  `apps/openagents.com/workers/api/src/artanis-labor-requester.ts` (#4731) lets
  Artanis issue a bounded, budgeted `ArtanisLaborRequestProposal`
  (`objectiveRef`, `requiredCapabilityRefs`, `budgetSats`, `deadlineRef`,
  `verificationCommandRef`), gated and fail-closed;
  `forum-work-requests.ts` carries the full typed lifecycle
  `open → quote_received → quote_accepted → running → delivered → accepted →
  settled` over **NIP-LBR** (`LBR_AGENTIC_CODING_REQUEST_KIND`, kind-5934);
  `labor-escrow.ts` reserves/releases budget on the agent credit ledger; and the
  `work_routing_proposal` decision kind in `artanis-forum-listener.ts` lets
  contributors *propose* studying/data directions that Artanis filters and funds.
  So "task an edge agent to produce a chunk of studied knowledge about a repo,
  escrow a budget, accept on delivery, settle" rides rails that already ship.
- **Studied knowledge is a *checkable* data contribution — a concrete way to start
  closing §4b's data-verification gap.** §4b flagged that
  `apps/openagents.com/workers/api/src/data-trace-marketplace-gate.ts` already
  enforces a `blocked → submitted → redacted → valued → purchased → entitled →
  payable → settled` lifecycle with provenance/redaction/valuation/settlement —
  but **without a correctness verdict** (confirmed: the gate has no
  correctness/validity/verify path; it prices and sells, it does not judge
  correctness). A *studied-knowledge* contribution is more checkable than raw
  data: a claimed commit-rationale link can be re-derived from the cited commit
  and doc and **compared**; a claimed code↔history↔doc graph edge resolves (or
  fails to resolve) against the actual repo at a pinned digest; the
  `repo_corpus_*` helpers already produce stable manifest/span hashes and
  line-numbered evidence spans, and StudyBench rubric scoring already attaches
  evidence-span resolution. That is exactly the **derived-trace replay + validator
  review** shape §4b named as the missing data analogue of §1's exact replay —
  pointed at a *bounded, resolvable* target (links and spans over a pinned repo)
  rather than at irreducibly non-deterministic raw data. Studied knowledge is
  therefore the most natural *first* data direction to make verifiable and payable.
- **It is the knowledge/data layer — DISTINCT from the compiled weight-module
  core.** In §0–§2's analytic-construction paradigm, capability is *constructed
  and verified by exact replay*; studied knowledge is **not** a compiled
  weight-module and must never be described as one. It is the hybrid ring's
  *learned-interface context* / a memory-retrieval substrate over a codebase
  (§2d, Baseline D: a thin learned shell marshaling frozen exact cores) — exactly
  the kind of *data direction* §4b says the edge produces. The honest framing:
  this is the **near-term, product-facing track** (Autopilot coder / Forge
  cockpit / the Probe coding runtime) that runs in parallel with, and feeds
  context to, the longer-arc weight-module construction — not a substitute for it.

### Honest split: EXISTS vs must-be-BUILT

- **EXISTS:** the machine-studying *research + roadmap* (repo-as-corpus dogfood,
  study-packet shape, hidden-edit-exam design); the StudyBench *benchmark*
  contracts and the deterministic `repo_corpus_*` manifest/span helpers in the
  Probe runtime; the *live labor market* (Artanis requester, NIP-LBR work-request
  lifecycle, labor escrow, `work_routing_proposal`); the *data-trace gate*
  (provenance/redaction/valuation/settlement); and `backroom/` as the
  ready-made **rationale corpus** of rejected/pruned lineage.
- **Must be BUILT:** the **deep, cross-linked knowledge substrate** over a repo
  (code ↔ commit ↔ doc ↔ issue ↔ rationale ↔ invariant) as a first-class,
  digest-addressed artifact — today the pieces (corpus manifest, evidence spans,
  study packet) are separate, not yet a traversable graph; a **studied-knowledge
  correctness-verification model** (derived-trace replay over link/span claims +
  validator review for the non-deterministic remainder) that gives the
  data-trace gate the *correctness verdict* §4b says it lacks; and the
  **Autopilot-coder integration** that actually *consumes* the substrate at edit
  time (find the right site, respect the invariant, avoid the pruned mistake).
  This maps onto the roadmap's open phases (study-packet MVP → hidden exam →
  Probe-backed runs → deterministic refinery → dogfood surface → external-repo
  product), not onto §4's top-5 construction gate.

**Relationship to the §4 top-5 and the §4b data gap.** This track is *parallel*
to the §4 top-5 (which sequence the analytic weight-module construction), not a
sixth item inside it — studied knowledge is data/context for the hybrid ring
(§2d), never a compiled module. It is, however, the most concrete near-term
*answer* to the data-verification gap §4b flagged: the first data direction whose
correctness is actually checkable, on rails (labor market + data-trace gate +
StudyBench contracts + repo-corpus helpers) that already exist — with the
knowledge graph, the correctness model, and the coder integration as the honest
build-list.

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

---

## 6. Roadmap: building the system (issue index)

The §3-§4 gap, the §4b edge-variance engine, and the §4c studying track are
filed as a sequenced set of GitHub issues in `OpenAgentsInc/openagents`,
grouped by track and tied back to the sections above. `openagents` owns
execution / replay validation / settlement / projection / the labor market /
the studying surfaces; the construction pipeline is implemented in the sibling
**`psionic`** repo (those issues are tracked/coordinated here, marked
`[psionic]`). Sequence: **S → C/V/E interleaved → H** (gradients enter only in
H).

- **EPIC #5313** — Build the Tassadar LLM-computer system (programs→weights,
  edge variance, studying). Overview + the full sequenced checklist below.

### Track S — Studying → Autopilot-coder (start here; §4c)

Status 2026-06-18: complete on `origin/main` for #5314-#5320. The external-repo
pilot remains a yellow, refs-only product surface; customer-private ingestion,
marketplace packaging, pricing, payout, and settlement claims remain blocked.

- **#5314 — S1** Repo-knowledge substrate v1: study-packet ingest over the
  `openagents` repo (current code + commit history + rationale + cross-links),
  building on the #5284 repo-corpus manifest/entry/evidence-span primitives.
- **#5315 — S2** Studied-knowledge graph: cross-link
  code↔commit↔doc↔issue↔rationale↔invariant (incl. `backroom/` rejected
  lineage) into a traversable graph.
- **#5316 — S3** Studied-knowledge verification model: derived-trace replay
  over link/span claims — closes the §4b/§4c data-correctness gap (the
  data-trace gate has no correctness verdict today).
- **#5317 — S4** StudyBench eval harness over `openagents`: hidden-edit exams +
  retained-failure fixtures scoring studying completeness, wiring the shipped
  StudyBench contracts.
- **#5318 — S5** Autopilot-coder consumes studied knowledge: integrate the
  substrate into the coding-agent context/plan path (Probe runtime / Autopilot
  Coder plans / Forge cockpit) — the product payoff.
- **#5319 — S6** Edge-agent studying contributions as paid work: Artanis tasks
  agents to produce/verify study packets via the live labor market + escrow.
- **#5320 — S7** Generalize studying to external repos (studying-roadmap Phase
  6 customer product) — later.

### Track C — LLM-computer construction substrate (§2-§4; psionic-heavy)

- **#5321 — C1** Run work-unit = real compiled-program corpus (distinct
  CALM/Wasm programs), not the single `loop_sum_v1` fixture — top-5 #1.
  Status 2026-06-18: first C1 slice landed in psionic/openagents as a
  four-program psionic-generated numeric corpus (loop-sum, arithmetic, memory
  roundtrip, factorial state machine) plus no-spend Artanis dispatch/replay
  selection by assignment workload slot. This does not claim dense loadable
  modules, composition, marketplace listing, or paid construction yet.
- **#5322 — C2** Complete psionic E4 MILP scheduler (gate-graph→layer/phase
  assignment). `[psionic]`
  Status 2026-06-18: landed in psionic at `ec2bfe9c`. The default ALM backend
  compiler identity is now `tassadar_alm_backend_e4_milp_schedule`/`v2`,
  the legacy greedy scheduler remains callable for comparisons, and C2 tests
  assert E4 `slot_count <=` greedy over the E1 workloads plus the four-program
  run-facing corpus. The E4-regenerated corpus digest is
  `1b7babcd0c3ce63e43212f3e4f07480969a7a9612a237b117f8de7fb8a828d6a`
  (60 slots / 6 layers per corpus program), and the OpenAgents executor fixture
  plus TypeScript corpus constant are updated to replay that E4-scheduled
  artifact. This does not claim dense loadable modules, softmax bounds,
  Wasm-window expansion, composition, marketplace listing, or paid
  construction yet.
- **#5323 — C3** Dense, loadable, digest-pinned weight-module materialization
  (psionic W1.2) + wire one as a run artifact — top-5 #2. `[psionic + openagents]`
  Status 2026-06-18: landed in psionic/openagents. Psionic now emits
  `TassadarAlmDenseWeightModule` v1 for `tassadar_corpus.loop_sum_v1`, with
  dense `W_Q/W_K/W_V/W_O` attention blocks, dense FFN matrices, residual
  wiring matrices, source numeric-model provenance, and reproducible dense
  module digest
  `cfda0fe5dcf42e16db9e18696731427f0f30915fd3100d38da2dcc8411433e2c`.
  The committed fixture replays to trace digest
  `2465d2c2af5077b4cf44c6eddbdc5aba2859029e30062f49a30e669acfc8e9d2`.
  OpenAgents dispatch now marks the loop-sum corpus slot as
  `tassadar_alm_dense_weight_module.v1`, includes the dense module as a
  no-spend run artifact, and the Worker replay validator executes the dense
  artifact when present. This does not claim trained weights, softmax bounds,
  composition/linking, marketplace listing, paid construction settlement, or
  serving.
- **#5324 — C4** Softmax-approximation bounds (psionic W1.4) + Wasm window
  ladder (W1.1) to grow corpus diversity. `[psionic]`
  Status 2026-06-18: landed in psionic/openagents. Psionic now exposes the
  bounded `tassadar.wasm.core_i32_w1_1.v1` profile with 21 documented opcodes
  (the prior 12 plus `nop`, `local.tee`, `drop`, `i32.eqz`, `i32.eq`,
  `i32.ne`, `i32.gt`, `i32.le`, `i32.ge`) and cross-validates the ALM
  interpreter plus specialized compiled legs against the CPU reference runner.
  The run-facing corpus now includes a fifth fixture,
  `tassadar_corpus.w1_1_window_v1`, with regenerated corpus digest
  `0d347bc3081acd2740761673f0b70d3e17a5ae467e9f865b5e6ef12009bfeb49`.
  Psionic also adds the W1.4 analytic softmax-bound certificate: for `n`
  candidates, margin `Delta`, and inverse temperature `beta`, non-winner mass
  is bounded by `T/(1+T)` where `T=(n-1)exp(-beta*Delta)`, with L1 hard-max
  distance bounded by `2T/(1+T)`; the canonical keyed-read case
  (`n=1024`, integer gap `1`, `beta=32`) yields non-winner mass under
  `1.4e-11`. OpenAgents imports the regenerated five-program fixture, extends
  Artanis no-spend dispatch/replay selection to slot 4, and fixes dense-module
  dispatches to claim the dense trace digest when a dense artifact is present.
  This does not claim a softmax runtime, learned weights, module linking,
  marketplace listing, paid construction settlement, or serving.
- **#5325 — C5** Module composition/linking (`tassadar_module_linker`) + the
  compiled-weight-module marketplace listing — top-5 #3. `[psionic + openagents]`
  Status 2026-06-18: landed in psionic/openagents. Psionic now builds
  `tassadar_alm_linked_dense_module.v1` for two dense source banks,
  `tassadar_corpus.mul_add_v1` and `tassadar_corpus.memory_roundtrip_v1`,
  resolves them through `tassadar_module_linker`, materializes one block-
  separated composed dense module, and conformance-replays each projected bank
  output against its source dense trace. The linked module digest is
  `cc1403674fc0d38892610d9e9c6c9230075494061f720c45bfa4f7b5a961756a`;
  the composed dense module digest is
  `2f3fa15120f0a078d4ede4e074e288fed24533ffa46f2d4b8aa4ca418c876602`;
  the composed replay trace digest is
  `0caa43ace27a5b86da14cfe037e65c30f250f0c0a0ac1c01f1fe3a3a45a230b2`.
  OpenAgents imports the generated fixture, independently replay-verifies the
  composed module and source-bank projections in TypeScript, and exposes a
  digest-pinned read-only listing at
  `/api/public/tassadar/compiled-module-marketplace`. The listing keeps
  purchase, entitlement, and settlement separate: `settlementClaimAllowed` and
  `purchaseSettlementAllowed` remain false unless replay verification clears
  and public-safe purchase plus settlement receipt refs are present. This does
  not claim arbitrary module installation, serving, learned weights, softmax
  runtime, live purchase mutation, real settlement, or paid construction yet.

### Track V — Verification + settlement extensions (§3-§4, §4b)

- **#5326 — V1** Extend exact-replay verification + settlement to PAY for
  construction (a verified compiled-module contribution), not just one fixed
  replay — top-5 #4.
  Status 2026-06-18: landed in openagents. The Worker replay validator is now
  regression-tested against the non-`loop_sum_v1` linked dense compiled module
  from C5 (`tassadar_alm_linked_dense_module.v1`, linked digest
  `cc1403674fc0d38892610d9e9c6c9230075494061f720c45bfa4f7b5a961756a`,
  replay trace digest
  `0caa43ace27a5b86da14cfe037e65c30f250f0c0a0ac1c01f1fe3a3a45a230b2`).
  `tassadar-auto-settlement.ts` adds a separate compiled-module construction
  settlement leg that derives deterministic public-safe refs, records a
  construction-tagged `settlement_recorded` receipt in unpaid-smoke simulation
  mode by default (`adapterKind: simulation`, `moneyMovement: none`,
  `realBitcoinMoved: false`), is idempotent by receipt ref, and fails soft on
  non-Verified, unsafe, over-cap, missing-target, or dispatch-failed cases. Real
  Spark payment remains owner-gated behind the existing
  `OPENAGENTS_REAL_SETTLEMENT_GATE`, run allowlist, per-payout cap, daily cap,
  registered payout target, and receipt-first dispatch/reconciliation chain.
  This proves the clean-checkout construct/dense-module replay/verified/pay
  simulation loop; it does not arm real settlement, broaden purchase/entitlement
  authority, serve modules, or claim live paid construction sats.
- **#5327 — V2** Module-composition verification (a linked module verified as a
  composition of verified parts).
  Status 2026-06-18: landed across openagents + psionic. The linked dense
  verifier now emits an explicit composition verdict instead of treating C5 as
  an opaque blob: every source dense bank must replay to its expected source
  trace, the composed trace digest must match
  `0caa43ace27a5b86da14cfe037e65c30f250f0c0a0ac1c01f1fe3a3a45a230b2`, and
  the psionic link-resolution evidence must conformance-check requested/selected
  module refs, `benchmark_gated_internal` trust posture, the
  `compiled dense ALM module composition / exact replay gate` claim class,
  compatibility digests
  `7383efa5fc20908b610c46cd015fe56a4bf7e793ac76ecddfaa7bf3e4ca72ad7` /
  `7873bf9e8f60675c7fcae4bf077f240514a8f2a14733d29c800531f59c6a2389`,
  dependency graph digest
  `bd346a41f5a2c6329c49c51eef075e751ad2abf8a02d45c4727ee47df58b7d84`,
  and link resolution digest
  `6c98a62f135a1c56169257c17c932c5ac3cedfade95b6ada4992c4f015d221c6`.
  The Worker replay validator accepts a linked-dense fixture mode and rejects
  injected link incompatibility or a tampered constituent even when the composed
  trace digest itself still matches. The public compiled-module marketplace now
  gates `purchaseSettlementAllowed` / `settlementClaimAllowed` on
  `compositionVerificationCleared`, with composition and link-compatibility
  receipt refs exposed separately from purchase and settlement refs. Psionic
  regression tests assert that the linked fixture carries compatibility evidence
  for each bank and that an incompatible claim-class consumer request refuses at
  link time. This does not install or serve arbitrary modules, arm live purchase
  settlement, widen real-money gates, or claim gradient-trained weights.
- **#5328 — V3** Data-contribution correctness-verification model (provenance +
  derived-trace replay + validator review) atop the data-trace marketplace gate.
  Status 2026-06-18: landed in `openagents`. The data-trace gate now has a
  general correctness verifier, not just caller-supplied correctness refs. A
  deterministic contribution must cite public-safe source/provenance refs, pass
  duplicate source/trace checks, replay the derived trace from the source digest
  and transform ref, and match the claimed trace digest before the gate mints
  `correctness.public.data_market.derived_trace.*` receipts. Tampered claimed
  digests, missing provenance/source refs, duplicate trace digests, unsafe refs,
  and explicit rejected verdicts keep the lifecycle at `redacted` even when
  valuation, purchase, entitlement, payout-contract, and settlement refs are
  present. Irreducibly nondeterministic contributions route to
  `validator_review.public.data_market.*` with
  `validatorReviewRequired=true` and no correctness receipt, so they remain
  non-payable/non-settled until review produces a separate correctness verdict.
  The public projection exposes only refs, digests, blocker refs, provenance
  receipt refs, dedupe receipt refs, and verifier status; it does not store raw
  source material, private traces, customer data, provider payloads, wallet
  material, or live payment evidence.

### Track E — Edge-agent variance engine (§4b)

- **#5329 — E1** Artanis construction/data work-request directions (program
  authorship, dataset curation, data-direction proposals) via the live labor
  market. Status 2026-06-18: landed in `openagents`. Artanis now has a typed
  work-direction layer over the existing #4731 requester, Forum/NIP-LBR
  lifecycle, and labor escrow rail. Program-authorship requests carry
  CALM/Wasm module/corpus refs plus
  `command.public.tassadar.v1_construction_verification` and require a passing
  V1 construction/replay verdict before escrow release. Dataset-curation
  requests carry trace-corpus/source refs plus
  `command.public.openagents.data_contribution.v3_correctness` and require a
  passing V3 data-correctness verifier receipt before release. Contributor
  `work_routing_proposal` inputs route into funded requests only through typed
  proposal records with selector refs and explicit direction kinds, and the
  filter is default-off until operator-enabled. Delivered work records the
  full `delivered -> accepted -> settled` lifecycle only after the relevant
  verifier gate passes; failed V1, failed V3, or V3 validator-review remainder
  refunds escrow and does not accept/settle. This reuses the existing labor
  escrow; it does not add a parallel payout rail, arm live Bitcoin settlement,
  infer work directions from Forum keywords, or store raw/private source
  material in public projections.
- **#5330 — E2** Demand-priced curation + module-library ranking/dedup.
  Landed in `openagents`: `tassadar-module-library.ts` now builds a typed
  demand-price signal from demand, usage, marketplace-margin memory, and
  data-trace valuation refs; the signal raises recommended request budget and
  listing value for wanted directions and is consumable by the Artanis requester
  surface only as public-safe source refs plus a budget floor. The compiled
  module marketplace projection now includes `demandSignals` and a
  demand-ranked `moduleLibrary` view. Library entries rank by demand/value/usage
  while collapsing near-duplicate authored modules by typed `dedupeKey` to the
  replay/composition/link-verified canonical entry; unverified duplicates are
  recorded as collapsed refs and cannot displace the verified canonical. This is
  a read-only projection: it does not mutate listings, rankings, request budgets,
  payouts, settlements, trained weights, or serving paths, and it does not infer
  work demand from Forum keywords or publish raw/private material.
- **#5331 — E3** Adversarial-verification market: agents paid to find
  module-divergence inputs.
  Landed in `openagents`: `tassadar-adversarial-verification-market.ts` defines
  the typed divergence-claim and independent-reproduction verifier for module
  stress work, including explicit psionic exactness/refusal evidence refs and
  near-miss refusal refs when the claim is about the geometric parabolic-key
  boundary. Artanis now has an `adversarial_verification` work-direction kind
  with `command.public.tassadar.e3_adversarial_divergence`; confirmed
  divergences release labor escrow, while same-device, non-reproduced,
  non-divergent, digest-mismatched, or missing-near-miss claims refund and pay
  nothing. Confirmed defects can be handed to the existing V1 compiled-module
  construction settlement path, which records simulation settlement by default
  and only attempts real Spark payout under the unchanged owner gate and spend
  caps. This does not add a parallel payout rail, does not arm real settlement,
  does not mutate live run behavior, and does not publish raw divergence inputs
  or private traces.

### Track H — Hybrid ring (later; gradients enter ONLY here; §2d, §4 item 5)

- **#5332 — H1** Frozen compiled core + learned interface: gradient windows
  under quarantine/canary/promotion (the subordinate hybrid ring). `[psionic + openagents]`
  Status 2026-06-18: landed across psionic + openagents. Psionic now exposes
  `psionic-tassadar-student::hybrid`, a machine-checkable H1 validator over the
  retained W3 Baseline D fixture. It verifies that the frozen analytic executor
  plus learned interface still reproduces the documented W3 result
  (`1.0` pass@1, `1.0` replay acceptance, `1.0` output-digest match), that the
  interface receipt matches the retained Baseline D checkpoint/config digest,
  and that learned-interface trainable scopes do not target the compiled exact
  core. A changed frozen-core digest, trace-not-in-forward-pass claim, or
  trainable compiled-core scope fails closed. OpenAgents now adds
  `tassadar-gradient-window-regime.ts`, which projects public learned-interface
  candidate updates through quarantine -> deterministic recompute -> replicated
  matching update digests -> Baseline-D-grade canary metrics -> explicit
  promotion decision. Promotion remains blocked until recompute, replication,
  canary, construction/verification/data refs, and the promotion decision all
  pass, and direct submission never mutates the canonical checkpoint. This does
  not arm real settlement, pay public gradient windows, serve learned weights,
  or claim public decentralized gradient training is live.
