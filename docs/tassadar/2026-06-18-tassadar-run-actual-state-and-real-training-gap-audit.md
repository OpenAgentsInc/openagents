# Tassadar Run: Actual State and the Real-Training Gap

Date: 2026-06-18
Scope: an honest accounting of where the Tassadar run actually is — whether we
are training the actual Tassadar model or running the verified-work /
executor-trace verification + settlement loop — and a concrete, sequenced path
out of "guinea-pig test mode" into real model training.

Status: audit. Public-safe. Every claim is cited to a file or to an existing
product-promise record; where the code and an earlier doc disagree, both are
stated. Claim-discipline rule of this folder applies: where this names a fact, a
source is named; where it states a gap, it is labeled a gap.

---

## TL;DR (the bottom line first)

**No. We are not training the actual Tassadar model on the live run.** The live
run `run.tassadar.executor.20260615` is the **verified-work loop**: a worker
executes a fixed, digest-pinned computation (a pinned ~80-step numeric fixture),
an independent validator device re-executes the same fixed computation, the two
trace digests are compared, and on a match a tiny Bitcoin settlement streams to
both parties. That is **exact-trace-replay verification of a fixed computation
with sats settlement on top** — the economic and verification rails — not a
training loop, not gradients, not a model that improves.

This is a real and valuable achievement (the rails work, end to end, in the
open, with one real-Bitcoin canary settlement). It is also, accurately, a
verification proof-of-concept and **not** model training. The single biggest
blocker between here and training the real Tassadar model is that **there is no
canonical model + checkpoint lineage + training loop wired into the run**: the
run's "work" is a frozen fixture, settlement gates on a digest match (not on any
training metric), and the one piece of genuine training code in the repo
(analytic CS336-A1 backprop) is **script/test-only and unwired** from the run,
settlement, projection, and routes.

The shortest credible path is **RESEARCH_PLAN W5** (`training.public_gradient_windows.v1`,
state `planned`): define a tiny real student model + checkpoint C0, publish a
trace/data shard, make the run's "work unit" an *accepted training window*
(gradient/loss claim) instead of a fixed-fixture trace, add quarantine →
canary-eval → promotion, and adapt verification (recompute/replicate) to the
new work unit. The rails we already have carry most of the verification and
settlement machinery; what is missing is the model, the training loop, the
window unit, and the merge/promotion gates.

---

## 1. What is ACTUALLY live today

The live, money-moving loop is the **executor-trace contribution → independent
replay verification → streaming Bitcoin settlement** loop. State it as the real
achievement it is: the economic and verification rails are built and proven
end-to-end in the open.

**The loop, concretely:**

1. A contributor admits and claims a window lease on the public run
   (`POST /api/training/runs/{ref}/admit`, `POST /api/training/leases/claim` —
   the only public/agent-callable training writes;
   `apps/openagents.com/workers/api/src/training-run-window-routes.ts`).
2. The **worker** runs a dispatched, digest-pinned workload locally and submits
   a trace-commitment digest. The executor is
   `packages/tassadar-executor/src/numeric-executor.ts`
   (`executeTassadarNumericModel`), whose module header states the boundary
   plainly: "faithful re-execution of digest-pinned compiled workloads only. No
   softmax, no learning, no serving, no performance claim."
3. An **independent validator device** re-executes the same workload and submits
   its replay digest. Device-distinctness is enforced server-side
   (`validatorDeviceRef != pylonDeviceRef`).
4. The verdict is a **digest string comparison**, not recomputed judgment:
   `apps/openagents.com/workers/api/src/training-verification.ts`
   (`verifyExactTraceReplay`) compares the worker commitment digest against the
   validator replay digest; mismatch → `ExecutorTraceMismatch`/`Rejected`,
   match → `Verified`. The worker side of validation is
   `apps/openagents.com/workers/api/src/tassadar-replay-validator.ts`
   (`runTassadarReplayValidation`), which re-runs `executeTassadarNumericModel`
   and sets `matches = trace.traceDigest === request.claimedTraceDigest`.
5. On a `Verified` `exact_trace_replay` pair, settlement **auto-streams** to
   both legs with no operator POST (openagents #5309 / #5310 / #5311):
   `apps/openagents.com/workers/api/src/tassadar-auto-settlement.ts`
   (`autoSettleVerifiedPair`) pays the worker and validator over the Spark
   treasury rail, idempotent and fail-soft; #5311 broadcasts each settled leg
   onto the public live settled feed
   (`apps/openagents.com/workers/api/src/index.ts`, `buildSettledFeedEvents`).

**Evidence it is real (from the promise registry,**
`apps/openagents.com/workers/api/src/product-promises.ts`):

- `training.decentralized_training_launch.v1` is **green** (renamed from
  `training.monday_decentralized_training_launch.v1`; rename only, no scope
  widened). Its safeCopy: an independent contributor installed Pylon, claimed a
  window lease, submitted a Tassadar executor trace; an independent validator on
  a separate machine/identity replayed the pinned fixture; the challenge
  finalized `Verified`; and **exactly one bounded 1,000-sat real-Bitcoin
  run-settlement** settled native over Spark to an independent contributor
  (`receipt.nexus.tassadar_run_settlement...canary1k.v6.20260618`,
  `realBitcoinMoved:true`, `state:settled`). The public settled feed moved 0 → 1.
- `compute.tassadar_executor_poc.v1` is **green** (2026-06-10): one bounded
  workload family on one Pylon, one operator-funded Lightning closeout, a
  Verified replay receipt and a Rejected-on-tamper receipt.

**Honest caveats baked into the records themselves:** the earlier Orrery
settlement receipt is **simulation-backed** (`realBitcoinMoved:false`) and
proves only the projection/record path; the single real-Bitcoin movement is the
one 1,000-sat canary. The launch promise's unsafeCopy forbids claiming
network-scale, hundreds paid, or "public gradients mutate a canonical model."
The current per-window rate in shipped code is **5 sats to the worker + 5 sats
to the validator** (`tassadar-auto-settlement.ts`,
`TassadarPerWindowWorkerRewardSats = 5`, `TassadarPerWindowValidatorRewardSats = 5`).
Note: the 2026-06-16 economics doc recommended **1 sat** for the fixture class
and flagged that validators earned nothing; the shipped reality (5+5, validator
now paid) supersedes that recommendation — worth reconciling the doc.

**Gating is conservative.** Real settlement is OFF by default and fails closed:
the gate (`tassadar-run-settlement-gate.ts`) only arms when the owner sets
`OPENAGENTS_REAL_SETTLEMENT_GATE` with `enabled:true`; otherwise every leg
returns `skipped: 'gate_not_authorized'`. Hard ceilings apply: per-payout cap
100,000 sats, daily ceiling 1,000,000 sats (fails closed), plus the run
manifest `spendCapSats`. The Artanis scheduled runner is disabled in production
(`ARTANIS_SCHEDULED_RUNNER_ENABLED="false"` in `wrangler.jsonc`); the admin
tick, worker↔validator pairing, and real settlement are each independently
flag-gated off by default.

**What Artanis (the autonomous cloud-mind administrator) actually drives.** Each
enabled tick (`apps/openagents.com/workers/api/src/artanis-administrator-tick.ts`,
`runArtanisAdminTick`), Artanis chooses one typed action from a two-member
vocabulary — `dispatch_executor_trace` or `no_action` — bounded to 4 dispatches
per day. On dispatch it sends the **fixed PoC fixture** (`tassadarPocLoopSumFixture`)
in `paymentMode: 'unpaid_smoke'` with a no-spend cap, then re-executes the same
fixture in-worker as a validator and accepts/rejects purely on digest equality.
"Continual-learning templates"
(`artanis-continual-learning-templates.ts`) are **typed proposal records with
explicitly zero execution authority** (`assertNoExecutionAuthority` throws if any
authority flag is true); the only one tied to the live run is
`artanisExecutorTraceReplayTemplate()` — the same fixed trace-replay workload,
`riskLabel: 'low'`, no-spend, no runtime promotion. **There is no Artanis code
path that ingests verified contributions as gradients/data into an improving
model.** It accumulates verified traces, verdicts, receipts, and settled sats.
This matches the yellow `artanis.tassadar_evolution_loop.v1` promise (spine
deployed, one real autonomous dispatch span; sustained unattended streak and a
first curated distillation dataset still gated).

---

## 2. Is the actual Tassadar model being trained?

**No.** With proof, and with the two cases the question implies kept distinct.

### (a) The live run is not training a model

The run's "work" is a **pinned exact-program executor trace**, not forward/
backward passes:

- The workload is a committed fixture
  `packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json`
  (`fixtureId: "tassadar_poc.loop_sum_v1.numeric_fixture.v1"`,
  `programId: "tassadar_poc.loop_sum_v1"`), pinning `expectedTraceDigest`,
  `expectedModelDigest`, `expectedOutputs: [15]`, the full model coefficient
  arrays, and 80 identical input steps. Its own `claimBoundary` states it is
  "not a trained transformer ... makes no softmax, learning, or served-route
  claim."
- `executeTassadarNumericModel` (`numeric-executor.ts`) walks a fixed plan of
  wiring/attention/FFN rows and SHA-256-hashes step outputs into a
  `traceDigest`. **There is no loss, no gradient, no parameter update anywhere
  in this executor.**
- Settlement hard-gates on a `Verified` `exact_trace_replay` challenge:
  `apps/openagents.com/workers/api/src/tassadar-run-settlement.ts`
  (`buildTassadarRunSettlement`) rejects any other class with "Only
  exact_trace_replay executor-trace work is settleable on this run." Payment is
  for a **digest match**, never for loss reduction, gradient quality, or any
  training metric.
- The public `realGradient` projection
  (`apps/openagents.com/workers/api/src/training-run-window-authority.ts`,
  `publicRealGradientStatus`) is a **read-only status surface** over existing
  challenges/leases plus an optional blob a caller may stash in
  `publicProjectionJson.realGradient`. It computes no training; in the live flow
  it reports standing blockers (`requires_two_real_contributor_devices`,
  `blocker.cs336_a1.real_gradient_psionic_lane_external`) and `settledPayoutSats:
  0` in every leaderboard row. It is wired into the run summary but reports
  `blocked_external`.

### (b) The two training cases the registry actually distinguishes

The question's own framing — a one-off bounded A1-scale run that may have been
*evidenced* vs an ongoing real distributed model-training run — maps exactly to
two promise records:

- **`pylon.first_real_model_training_run.v1` — yellow.** SafeCopy: a *bounded*
  public remote two-device real-gradient training run (CS336 A1 scale,
  `run.cs336.a1.real_gradient.demo`) with digest-committed shard gradients on
  two physical machines, cross-device deterministic-recompute + Freivalds-Merkle
  verification, merge/eval refs, a loss-under-budget curve, and settled
  closeouts. This is the closest thing to "real training was done," and it is
  **a separate run from the live Tassadar executor run**, evidenced as bounded,
  with `model_ladder_network_rungs_not_run` still blocking. (The genuine
  analytic-backprop code exists at
  `apps/openagents.com/workers/api/src/cs336-a1-real-gradient-workload.ts` —
  real forward+backward, shard-gradient averaging, finite-difference gradient
  check — but in *this* repo it is imported only by its own test and
  `scripts/cs336-a1-real-gradient.ts`; the live Worker request path never calls
  it. The asserted execution lives in an external Psionic lane,
  `psion_cs336_a1_demo_v1`, which this repo cannot confirm.)
- **`training.public_distributed_training_run.v1` — red.** SafeCopy: a broad
  public distributed training run is *not* green — network-scale participation,
  broad accepted-work receipts, and a participant-count methodology remain
  unmet.

So: a bounded A1-scale real-gradient run has been **evidenced** (yellow); an
**ongoing real distributed model-training run is red**; and the **live Tassadar
run itself trains nothing** — it verifies and settles a fixed computation.

Supporting registry ground truth: `models.tassadar_percepta_executor.v1` is
**red** (`tassadar_model_spec_missing`, ...); the 2026-06-14 W3 sweep
(`docs/tassadar/2026-06-14-w3-student-program-report.md`) is explicitly
research/eval only and "creates no public model claim." `training.model_ladder.v1`
is **planned** — only rung R0 (a tri-host 12-step rehearsal, ~3,992 train
tokens) exists; no rung above R0 has started. No `models.psion_*` promise exists
at all.

---

## 3. The gap — exactly what is missing to train the real Tassadar model

The live run already supplies a surprising amount of the substrate. The gap is
specific. Each item below names what is missing and the existing primitive that
gets us partway.

1. **A real model definition + checkpoint lineage.** The run has no canonical
   model. `training-run-window-authority.ts` carries window-seal /
   `checkpointDigestRef` *bookkeeping* but no checkpoint that gradients merge
   into; `checkpointDigestRef` is optional and merely echoed.
   *Partway:* the W3 student crate
   (psionic `crates/psionic-tassadar-student/`, fixture bundle
   `fixtures/tassadar/w3_student_sweep_20260612/`) and the analytic A1 trainer
   (`cs336-a1-real-gradient-workload.ts`) are real model+gradient code; neither
   is wired to a published, versioned canonical checkpoint with parent-hash
   lineage.

2. **A training loop that ingests verified contributions as gradients/data.**
   Today the run ingests *trace digests*. Nothing turns a verified contribution
   into a gradient or a training step against a checkpoint.
   *Partway:* `cs336-a1-real-gradient-workload.ts` already does real shard
   gradient computation and `applyAggregatedSgdStep`; `training-real-gradient-
   evidence.ts` admits public-safe loss-curve/shard summaries into a projection.
   These are unwired from the run.

3. **Gradient aggregation/merge + admission into a canonical model.** No
   aggregation/merge of public contributions into a shared checkpoint exists.
   This is the W5 "accepted training window" unit
   (`model_checkpoint_hash`, `optimizer_state_hash`, `dataset_shard_hash`,
   `update_or_delta_digest`, `loss_stats`, `acceptance_decision`) and the
   quarantine optimizer (`canonical → quarantine → promoted`). Per
   `training.public_gradient_windows.v1` (**planned**), the substrate (#4855
   P0–P3: join lifecycle, staleness pricing, admission, canary) landed, but the
   **public model-update layer is not built**.

4. **A curriculum / corpus.** The CS336 ladder and the verified-trace corpus are
   the intended training data.
   *Partway:* a 100M-token verified-trace corpus snapshot exists
   (`corpus.tassadar_trace.v0_2.w3_100m`, 103,573,600 verified tokens, 6
   families) and the A4 data-refinery dashboard exists
   (`/api/training/refinery/a4`) — but the refinery in this repo is a
   verification-orchestration shim delegating the actual refining to an external
   Psionic lane, and the corpus is not wired as a *dataset-shard authority* the
   run trains against.

5. **Artanis orchestrating real training windows (not verification windows).**
   Artanis dispatches the fixed fixture in `unpaid_smoke`. To drive training it
   would need a new action (`dispatch_training_window`) that binds a checkpoint +
   shard + config and a settlement path keyed to an *accepted training window*,
   not a digest match. Today its continual-learning templates are zero-authority
   proposals; the high-risk training kinds (`lora_finetuning_training`) cannot
   even reach `running` without operator-approval + downstream executor-authority
   refs.

6. **Verification adapted to real training work.** Exact-trace replay does not
   transfer to gradient work (a gradient cannot be replayed for a byte-identical
   digest as cheaply as a deterministic trace). W5 specifies the harder ladder:
   Tier 0 schema/hash → Tier 1 deterministic recompute of a bounded window →
   Tier 2 replicated training (2–3 workers agree) → Tier 3 statistical checks
   (loss delta, update norm, NaN/inf, outliers) → Tier 4 canary eval on
   quarantine → Tier 5 downstream acceptance.
   *Partway:* the A1 lane already demonstrates `deterministic_recompute` and
   `freivalds_merkle` verification shapes against gradient/training-step digests
   (`cs336-a1-homework.ts`) — the right primitives, not yet pointed at an
   accepted-window unit on the live run.

In one line: **we have the verification ladder's bottom rung, the settlement
rails, the corpus snapshot, the autonomous dispatcher, and real (but unwired)
gradient code — and we are missing the canonical model, the window-as-training-
unit, the merge/quarantine/promotion gates, and Artanis driving training
windows.**

---

## 4. What's needed before real forward progress on the run

A concrete, sequenced checklist. This is the W5 lane
(`training.public_gradient_windows.v1`, planned) made operational, scoped to the
*smallest real run* first. Nothing here requires hardware we lack for the first
rungs (adapter/LoRA or small-dense deltas on a tiny student; CPU/Apple-Silicon
windows), per RESEARCH_PLAN §5 W5.

1. **Pick the smallest real model and publish checkpoint C0.** A tiny Psion/
   Tassadar student (reuse the W3 student crate or the A1 analytic trainer).
   Publish C0 with a checkpoint hash and an empty-parent lineage record. This is
   the thing the run trains. Decide: analytic-backprop A1-scale student first
   (fastest, already coded) vs the W3 distillation student.

2. **Define the accepted-training-window work unit + dataset-shard authority.**
   Replace the fixed-fixture "work" with a window binding
   `model_checkpoint_hash` + `dataset_shard_hash` + `training_config_hash` +
   `seed` + `start/end_step` + `update_or_delta_digest` + `loss_stats`. Seed the
   shard authority from the existing `corpus.tassadar_trace.v0_2.w3_100m`
   snapshot (split → shard hashes). This is the W2-`trace_record` analogue for
   training and is the single most load-bearing new contract.

3. **Build the quarantine optimizer + checkpoint lineage/rollback.** Three
   checkpoints (`canonical → quarantine → promoted`): public deltas apply to
   quarantine only; lineage records parent hash, included windows, shards,
   optimizer state, eval results, promotion/rollback decision, payout refs.
   Without this, one bad gradient corrupts the model and the run is undebuggable.

4. **Adapt verification to gradient windows (recompute/replicate + canary).**
   Point the existing `deterministic_recompute` / `freivalds_merkle` primitives
   (already proven in the A1 lane) at the new window unit; add Tier 2 replicated
   training and a Tier 4 canary eval on the quarantine checkpoint, judged by the
   W3 first-divergence/eval metrics — never by raw loss. Ship the falsifier with
   the lane (the validator is the same harness pointed at submitted windows).

5. **Wire Artanis to dispatch training windows + stage payout on promotion.**
   Add a bounded `dispatch_training_window` action and a staged-payout path
   (`submitted → pending`, `recomputed → provisional`, `quarantine-eval passed →
   accepted`, `promoted → settled`, `later regression → clawback`) — pay per
   *accepted window*, never raw GPU time, under the existing spend caps and
   operator-approval gates. Keep real settlement OFF until a full real run
   (publish C0 → shard D0 → claim W0 → train N steps → submit delta + loss →
   validator recompute → quarantine → canary → promote to C1 → pay → receipt
   shows the window contributed to C1) passes end-to-end on a clean checkout.

**Top-5 (the "before real forward progress" gate):** (1) publish C0 + lineage;
(2) define the accepted-training-window unit + dataset-shard authority from the
existing corpus; (3) quarantine optimizer + checkpoint lineage/rollback; (4)
gradient-window verification (recompute/replicate + canary on first-divergence
metrics); (5) Artanis training-window dispatch + staged-on-promotion payout. A
new master tracker (`training.public_gradient_windows.v1` becomes the registry
promise) is filed only when this lane produces evidence that needs one, per the
§7 disclosure rule.

### Where StudyBench / "machine studying" fits

**Adjacent infrastructure, not on the training path — at most a later eval/
optimization stepping stone.** "Machine studying" is an external research idea
(an agent studies an unlabeled corpus, then is scored on a hidden exam; headline
metric = expertise as area under score-vs-inference-compute). OpenAgents'
StudyBench (`packages/probe/packages/runtime/src/benchmark/studybench.ts`) is a
set of **typed benchmark schemas + evidence/closeout validators over the
OpenAgents repo** — it runs no model, no inference, no grading (scores are
supplied as input; the MVP-14 comparison numbers are authored, not computed),
and its only live footprint is the yellow, heavily-blocked
`autopilot.repo_study_packets.v1` promise. The studying roadmap is explicit:
*"The useful next move is not to train a model immediately"* — it favors
amortized study packets + GEPA *prompt-bundle* optimization (text, not weights),
and the upstream research found naive weight updates did **not** create
expertise. So for the real-training path, StudyBench is:

- **Not** training signal or corpus for the Tassadar/Psion model — a deliberate
  *deferral away from* weight training.
- **Possibly** a later *eval* harness (first-divergence-style scoring discipline,
  source-grounded rubrics) and a GEPA prompt-optimization input — useful once a
  student exists, but a **detour if pursued before W5's checkpoint-and-window
  loop.**

(For completeness: `packages/proof-replay` is unrelated to StudyBench — a live,
public-safe 3D visualization package that renders the real settlement run; it
validates nothing and authorizes nothing.)

---

## 5. Honest bottom line

The Tassadar run today is **a verification proof-of-concept with real settlement
rails — accurately described, it is not training.** That is not a criticism: the
rails are genuinely built and genuinely proven in the open. An independent
contributor installed Pylon, did a unit of dispatched work, an independent
validator on a separate machine confirmed it by replay, and a real-Bitcoin
canary settled with a public receipt — and the whole thing now auto-streams. The
hard, valuable parts of a machine-work economy — independent verification at
near-zero cost, hands-off settlement, public dereferenceable evidence,
conservative spend gates, an autonomous dispatcher under bounded authority —
exist and work. The registry states this honestly: green is scoped to exactly
one verified pairing and exactly one 1,000-sat canary, and the unsafeCopy
forbids extrapolating to network-scale or to "public gradients mutate a
canonical model."

But the "model" in "Tassadar run" is, on the live run, a **fixed 80-step
fixture**, and "training" is a **digest comparison**. We are in guinea-pig test
mode by design: the fixture is the cheapest possible unit that exercises the
rails end-to-end so that the rails can be proven before expensive work flows
through them. The single biggest blocker to real model training is the absence
of a **canonical model + checkpoint lineage and a training-window work unit
wired into the run** — settlement gates on `exact_trace_replay`, and the one
piece of real gradient code in the repo is unwired script/test code. The
shortest credible path out is RESEARCH_PLAN's W5, scoped to the smallest real
run: publish a tiny student checkpoint C0, make the run's work unit an accepted
*training window* (gradient + loss claim) drawn from the corpus we already have,
add quarantine → canary-eval → promotion, point the recompute/replicate
verification primitives (already proven in the A1 lane) at that window, and let
Artanis dispatch training windows under the existing spend caps. The day a
public Pylon trains N steps against a published checkpoint, a validator
recomputes the window, a canary eval passes, the update promotes C0 → C1, and a
receipt shows that accepted window contributed to C1 — **that** is the run
becoming real training, and not one step before it.

---

## Sources

Code (all under `apps/openagents.com/workers/api/src/` unless noted):
`training-run-window-routes.ts`, `training-run-window-authority.ts`
(`publicRealGradientStatus`), `training-verification.ts` (`verifyExactTraceReplay`),
`tassadar-replay-validator.ts`, `tassadar-run-settlement.ts`,
`tassadar-run-settlement-gate.ts`, `tassadar-auto-settlement.ts`,
`tassadar-trace-contribution-routes.ts`, `artanis-administrator-tick.ts`,
`artanis-continual-learning-templates.ts`, `artanis-scheduled-runner.ts`,
`cs336-a1-homework.ts`, `cs336-a1-real-gradient-workload.ts`,
`cs336-a4-data-refinery.ts`, `training-real-gradient-evidence.ts`,
`product-promises.ts`, `index.ts`; `packages/tassadar-executor/src/numeric-executor.ts`,
`packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json`,
`packages/probe/packages/runtime/src/benchmark/studybench.ts`,
`packages/proof-replay/`.

Promises (ground truth): `training.decentralized_training_launch.v1` (green),
`compute.tassadar_executor_poc.v1` (green),
`pylon.first_real_model_training_run.v1` (yellow),
`artanis.tassadar_evolution_loop.v1` (yellow),
`training.public_distributed_training_run.v1` (red),
`models.tassadar_percepta_executor.v1` (red),
`pylon.largest_decentralized_training_claim.v1` (red),
`training.public_gradient_windows.v1` (planned),
`training.model_ladder.v1` (planned).

Docs in this folder: `RESEARCH_PLAN.md` (W2 vs W5), `README.md`,
`work-that-proves-itself.md`, `2026-06-15-executor-trace-contributor-completion-design.md`,
`2026-06-16-verified-work-payment-economics.md`,
`2026-06-14-w3-student-program-report.md`; and
`docs/research/machine-studying/` (StudyBench audit + studying roadmap +
research note).
