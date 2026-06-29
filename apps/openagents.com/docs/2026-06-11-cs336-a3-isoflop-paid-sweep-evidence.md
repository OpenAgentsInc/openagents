# CS336 A3 Scaling Sweep: First Crowd-Sourced IsoFLOP Curve With Paid Cells (issue #4679)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4679` (CS336 distributed-homework epic,
plan step 7 — the crowd-sourced IsoFLOP scaling sweep).

Registry version during run: `2026-06-11.7` (live worker). This commit
ships no registry edit and records no promise transition: the issue's
named substrate promise `pylon.compute_revenue_modes.v1` stays gated by
`blocker.product_promises.live_gepa_network_missing` (a GEPA-specific
blocker this sweep does not touch), and no `training.*` promise names an
A3/scaling-sweep blocker this run clears. The evidence lands on the run
page and the public IsoFLOP feed instead.

Operator approval: `approval.operator.20260611.focus_cs336_issue4679`
(spend cap 300 sats total).

## Result

The public CS336 A3 IsoFLOP dashboard at `GET /api/training/isoflop/a3`
is live with its first crowd-sourced curve, leaving the honest empty
state (`cells: 0`, three blockers) for the first time:

- 24 sweep cells (4 compute budgets x 6 geometric N points) dispatched
  as 24 individual paid Pylon assignments — one planner-chosen `(N, D)`
  cell per assignment, per the issue contract — split 12/12 across the
  two registered live Pylons `pylon.24819249b4634a4c9d5e` and
  `pylon.4f4ef3d029e57674be98`.
- Every cell ran real bounded training compute on the contributor
  device and produced a deterministic output digest commitment that was
  pre-registered in the dispatch `resultExpectationRefs`.
- Six sampled cells (at least one per budget, three per Pylon) were
  independently re-executed by the opposite Pylon and verified by the
  production Worker's `deterministic_recompute` class: six challenges,
  all `Verified`, zero failure codes.
- All 24 closeouts paid 10 sats each over real Lightning with public
  settled receipts (`settled`, `amountSats: 10`,
  `movementMode: real_bitcoin`).
- The Psionic `psion_cs336_a3_scaling_reference_v1` fit (the exact
  committed Rust source, compiled and run over the returned losses)
  produced the published IsoFLOP artifact: parameter exponent
  `a = 0.7881` (`N_opt = k·C^a`), data exponent `b = 0.2119`, four
  interior budget optima, fit digest
  `233fb590dea48e2b681f27e13f16107ecbbe5853022895904c84903af62494ec`.
- Live feed state: `cells: 24`, all `verified: true`, `fitArtifacts: 1`,
  `status: fit_published`, `blockerRefs: []`.

## What actually computed

`scripts/cs336-a3-scaling-sweep.ts` executed
`src/cs336-a3-sweep-workload.ts` on the contributor device per cell:

- Workload `workload.cs336_a3.seeded_factored_bigram_lm.v1`: a seeded
  factorized bigram language model (vocabulary 256, parameter count
  `N = 2·256·rank`) trained by single-pass SGD over `D = C/(6N)` data
  units of a deterministic synthetic next-token stream, then measured
  on a held-out 2,048-unit stream (cross-entropy validation loss).
- Grid: budgets `{3e8, 6e8, 1.2e9, 2.4e9}` planned FLOPs, six geometric
  N points per budget between 1,024 and 65,536 (the planner mirrors the
  Psionic `cs336_a3_plan_isoflop_sweep` contract; continuous planned N
  realizes to the nearest integer factor rank).
- Stream seeds derive from the A1 tokenizer shard digest
  (`cs336-a1-homework-workload.ts`), binding A3 cells to the same
  committed corpus pipeline as the #4675 run.
- Every budget shows a real interior loss minimum, and the optimal N
  grows with budget (7,313 → 10,938 → 24,635 → 34,461) — the IsoFLOP
  shape the assignment is about. Heterogeneous-hardware tolerance is by
  construction: cells are budget-normalized and digests are
  deterministic commitments.
- Total sweep compute ~4.5e9 planned FLOPs per the `C = 6ND`
  accounting convention; the realized JS arithmetic is approximately
  4N multiply-adds per data unit, recorded as the convention-vs-measured
  caveat, ~4 s of wall training time per 12-cell half.

## Live route chain (all production)

| Stage | Evidence |
| --- | --- |
| run plan (admin) | `run.cs336.a3.scaling_sweep.demo`, promiseRef `pylon.compute_revenue_modes.v1` (substrate link only; no transition claimed) |
| window plan + activate (admin) | `training.window.cs336_a3.demo.20260611.w1` (worker Pylon), `training.window.cs336_a3.demo.20260611.w2` (validator Pylon), homeworkKind `admin_dispatched_homework` |
| window leases (contributors) | `training.lease.9ca01dcc-bd55-4177-ad3c-d57c02c7d18d` (w1), `training.lease.48337eb0-36e5-4844-bca5-21f911fd04d6` (w2) |
| paid assignment dispatch (admin) | `assignment.cs336_a3.sweep.cell_{1..24}_20260611015841`, paymentMode `payable_pending_settlement`, jobKind rail `claude_agent_task` with the `cs336A3` payload; the dispatch gate enforced one active assignment per Pylon, so each cell chained dispatch → worker chain → operator closeout before the next dispatch on that Pylon |
| worker chains (agent bearer) | per-cell acceptance, progress, artifact proof metadata (`commitment.cs336_a3.cell_{k}.sha256_*` digest commitments + loss/params/data-unit result refs), worker closeouts |
| sampled verification (admin create, open claim, admin finalize) | cells {2, 5, 7, 12, 18, 23} re-executed by the opposite Pylon; six `deterministic_recompute` challenges all `Verified`: `training.verification.challenge.8dfcb25c-3fff-42ee-8722-8e1ce2983ae6` (cell 2), `.a74ce915-34a9-42be-b92d-6aa98f5b4598` (5), `.98e2906f-e790-4e3a-82fe-28037d0ec01e` (7), `.dc79b1e4-c277-453b-a1ee-6ba3057f43da` (12), `.9c02a43f-11bf-4b53-a84d-2901412a487a` (18), `.5737fcb9-3753-4ad9-89aa-75eae4709226` (23); validator `validator.cs336_a3.deterministic_recompute_issue4679`, samplingPolicy `per_contribution` |
| operator closeouts (admin) | `accepted_work.cs336_a3.sweep_cell_{k}_4679_commitment_matched` x24, all assignments `accepted_work`; per-cell acceptance basis is the pre-registered deterministic commitment match, with the sampled re-run verdicts landing before any payment moved |
| payments | 24 x 10 sats over Lightning from the operator edge payer wallet (warm channels); payer 1935 -> 1695 sats, worker wallet 177 -> 294, validator wallet 156 -> 274 (provider-confirmed on both sides; receive-side fees visible in the wallet ledgers) |
| pylon events | payout-target admissions (`pylon_event.payout_target_admission.d29ad93b-c11c-4fa5-88b8-9eb49139ba55`, `.398e4c7a-a40f-4505-b48d-e524f9f5355e`), 24 payment-receipt + 24 settlement-status events with sha256-derived redacted payment refs |
| settlement bridges (admin) | `receipt.nexus_pylon.settlement.assignment_cs336_a3_sweep_cell_{1..24}_20260611015841`, adapter `mdk_agent_wallet`, public route 200s (`settled`, `amountSats: 10`, `movementMode: real_bitcoin`, `realBitcoinMoved: true`) |
| fit artifact | Psionic source `crates/psionic-train/src/cs336_a3_scaling_reference.rs` compiled verbatim and run over the 24 returned losses; `artifact.cs336_a3.isoflop_fit.sha256_233fb590dea48e2b`, predicted compute-optimal config at 4.8e9 FLOPs: 63,249 params / 12,648 data units |
| window seal + reconcile (admin) | both windows `reconciled` (seal receipt `closeout.cs336_a3.operator_accepted_cell_1_4679`, reconcile receipt `receipt.nexus_pylon.settlement.assignment_cs336_a3_sweep_cell_1_20260611015841`) |
| evidence admission | 24 cells + fit artifact validated through `admitCs336A3ScalingSweepEvidence` + `publicScalingSweepProjection` locally against the exact remote run row, then applied as one operator-staged D1 `UPDATE` (the deployed worker predates this commit's admission route) |
| public 200s | `route:/api/training/isoflop/a3` (`blockerRefs: []`, 24 verified cells, 1 fit artifact, `fit_published`), `route:/api/training/runs/run.cs336.a3.scaling_sweep.demo` (`verifiedWorkCount=6`, `reconciledWindowCount=2`, `assignedContributorCount=2`), public settlement receipts for all 24 cells |

## Spend accounting

| Movement | Sats |
| --- | --- |
| 24 sweep-cell closeouts x 10 sats (Lightning, warm channels) | 240 |
| Total operator spend (cap 300) | 240 |
| Hosted MDK treasury spend | 0 |

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths appear in this document or in any public ref. Redacted
payment refs are sha256 derivations, not hash prefixes.

## What this commit adds (integrated surfaces)

- `cs336_a3_scaling_sweep` is now a first-class
  `PylonApiAssignmentJobKind` literal; tonight's live assignments rode
  jobKind `claude_agent_task` with the A3 payload and
  `rail.job_kind.claude_agent_task_until_cs336_a3_scaling_sweep_deploys`,
  the same documented rail the #4675/#4681 runs used pre-deploy.
- `src/cs336-a3-sweep-workload.ts` (+ tests): the real bounded cell
  trainer with the Psionic-mirrored grid planner, deterministic digests,
  and the committed deterministic_recompute round-trip test (matching
  digests verify, tampered digests reject).
- `POST /api/training/runs/{trainingRunRef}/scaling-sweep-evidence`
  (admin): the previously missing admission seam. Nothing could write
  `a3ScalingSweep.cells` into a run projection before this route; it
  enforces receipted-cells-only, positive finite quantities, the
  20-cell minimum for fit artifacts, and the public-safety guard at
  admission time. OpenAPI operation `admitTrainingA3ScalingSweepEvidence`.
- `scripts/cs336-a3-scaling-sweep.ts`: the contributor-side executor
  (public-safe output only; no network, no secrets, no spend).

## Honest remainder (named gaps)

- `caveat.cs336_a3.single_physical_host_two_pylons`: both Pylons run on
  one physical machine. The sampled cross re-runs are real cross-process
  re-executions, not cross-machine replication; every cell carries the
  caveat ref.
- `caveat.cs336_a3.tiny_synthetic_grid_only`: the sweep is a bounded
  synthetic-task grid (factorized bigram LM, 4 tiny budgets, one device
  class). The fitted exponents are analysis artifacts about this grid
  only — never model-capability or general scaling claims
  (`copy.public.training.scaling_law_is_analysis_artifact`,
  `copy.public.training.scaling_law_not_capability_claim`).
- `blocker.cs336_a3.job_kind_first_class_after_deploy`: the live worker
  predates the `cs336_a3_scaling_sweep` job-kind literal and the
  evidence-admission route in this commit; tonight's evidence admission
  was an operator-staged D1 write of route-equivalent validated JSON.
  After the next deploy, admissions go through
  `POST /api/training/runs/{ref}/scaling-sweep-evidence` and A3
  assignments dispatch under their own job kind.
- Sequential-dispatch seam: the live dispatch gate's
  `duplicate_active_assignment` guard allows one active assignment per
  Pylon, so a 12-cell batch per Pylon runs as a serial
  dispatch→closeout chain. An embarrassingly-parallel sweep across many
  Pylons works today (one cell in flight per device), but per-device
  cell queues need either a batch job kind or a relaxed guard.
- Operator-staged lane: dispatch, challenge create/finalize, closeout,
  payment execution, and fit publication were operator actions
  (hosted-MDK programmatic payouts remain disabled). A standing sweep
  market needs self-serve admission and automated settlement.
- Psionic external ask honest state: the fit and planner contracts ran
  verbatim from the committed psionic source over the real returned
  losses, but as an operator-compiled local run; a packaged
  psionic-train invocation surface for the fit remains the external
  dependency. The issue's "A1 trainer unchanged" is realized as shared
  seed provenance (A1 tokenizer shard digest seeds every A3 stream),
  not as a literal reuse of the A1 matmul step, which produces no loss.
- `providerConfirmedSettledPayoutSats` on the public run page still
  hardcodes 0 (known #4675 seam); the 24 settled receipts are linked
  through window receipt refs and cell receipt refs instead.
