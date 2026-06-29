# CS336 A2 Device-Capability Dataset: First Receipted Rows With Paid Closeouts (issue #4681)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4681` (CS336 distributed-homework epic,
plan step 9 — the public device-capability dataset).

Registry version during run: `2026-06-11.6` (live worker). This commit
ships the `2026-06-11.7` registry edit; the
`training.device_capability_dataset.v1` planned → yellow transition
receipt `promise_transition_af246d4d-f37e-456a-b5ee-fba2d5ba3017` was
recorded live against `2026-06-11.6` (all four checks passed) before the
edit.

Operator approval: `approval.operator.20260611.focus_cs336_issue4681`
(spend cap 300 sats total).

## Result

The public CS336 A2 device-capability dataset at
`GET /api/training/device-capabilities/a2` is live with its first
receipted rows, leaving the honest empty state for the first time:

- Two paid benchmark assignments ran the real bounded benchmark suite
  (`benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1`) on two
  registered live Pylons: `pylon.24819249b4634a4c9d5e` (the #4675 worker
  device) and `pylon.4f4ef3d029e57674be98` (the #4676 validator device).
- The production Worker verified all four measurement kinds with
  `statistical_cross_check` on real cross-device agreement scores — the
  first time this verification class ran on real dispatched work.
- Both 30-sat closeouts settled over real Lightning with public
  receipts reporting `settled`, `amountSats: 30`,
  `movementMode: real_bitcoin`.
- The live dataset now serves `observedDeviceClassCount=1`,
  `observedMeasurementCount=4`, all rows `cross_checked`/`verified`, with
  `blockerRefs: []`, and `GET /api/training/leaderboards/a2_throughput`
  ranks the four class rows with their settlement receipt refs.

## What actually computed

`scripts/cs336-a2-device-benchmark.ts` executed
`src/cs336-a2-benchmark-workload.ts` on the contributor device, three
repetitions per Pylon context, with wall-clock timing and deterministic
output digests per kernel:

- `attention_throughput`: bounded softmax(QK^T)V attention block
  (seq 64 x dim 32), measured megaflops.
- `memory_bandwidth`: STREAM-triad style typed-array passes
  (2M float64 x 8 passes), measured GB/s.
- `tokens_per_second`: bounded greedy reference-row decode loop
  (vocab 96 x hidden 48 projection matmul per step), measured rows/sec.
- `step_time_ms`: one bounded forward/backward/SGD training step
  (32x64x32), measured wall milliseconds.

Output digests are deterministic commitments (identical across runs and
devices); timings are the measured distribution. Cross-check scores per
metric were the real min-over-max same-class p50 ratios across the two
Pylons: 0.9297 (attention), 0.9836 (bandwidth), 0.9924 (decode), 0.9609
(step time) — all above the 0.5 class threshold, so all four
`statistical_cross_check` challenges finalized `Verified`.

## Live route chain (all production)

| Stage | Evidence |
| --- | --- |
| run plan (admin) | `run.cs336.a2.device_capability.demo`, promiseRef `training.device_capability_dataset.v1` |
| window plan + activate (admin) | `training.window.cs336_a2.demo.20260611.w1` (worker), `training.window.cs336_a2.demo.20260611.w2` (validator), homeworkKind `admin_dispatched_homework` |
| window leases (contributors) | `training.lease.9f42c598-3b94-4c4f-b4df-996d3a9029d3` (w1, worker Pylon), `training.lease.45dd6df3-67a5-4290-89fa-651decacb7d3` (w2, validator Pylon) |
| paid assignment dispatch (admin) | `assignment.cs336_a2.benchmark.worker_20260611060805`, `assignment.cs336_a2.benchmark.validator_20260611060805`, paymentMode `payable_pending_settlement`, both gates `dispatchAllowed: true` with zero blockers |
| worker chains (agent bearer) | acceptance, progress, `pylon_event.artifact_proof_metadata.1d2854de-501f-46f6-a446-0de1b0022e05` / `.54745a42-ac18-441e-a6e5-725dadc84b9e` (digest commitments + per-metric result refs), worker closeouts |
| verification (admin create, open claim, admin finalize) | four `statistical_cross_check` challenges, all `Verified`: `training.verification.challenge.c80ba722-0d0e-4e06-9e33-599ad78479f4` (attention), `.bea7519b-03ef-4d9c-973e-0f55b87071c9` (bandwidth), `.ca462e7a-a5ad-4728-b7c2-46d8aeb3e379` (decode), `.dc9397ee-a1c6-44ed-91ec-81e851523a1e` (step time); validator `validator.cs336_a2.statistical_cross_check_issue4681` |
| operator closeouts (admin) | `accepted_work.cs336_a2.device_benchmark_4681_worker` / `_validator`, both assignments `accepted_work` |
| payments | 30 sats each over Lightning from the operator edge payer wallet (warm channels); payer 1995 -> 1935 sats, worker wallet 147 -> 177, validator wallet 127 -> 156 (provider-confirmed on both sides; 1-sat receive-side fee visible in the wallet ledgers) |
| pylon events | payout-target admissions, `pylon_event.payment_receipt.02b54b00-734a-43f1-b450-008e4ee04cf9` / `.abd45c12-b33d-4ca0-8b63-b3a822d5b033`, settlement-status events |
| settlement bridges (admin) | `receipt.nexus_pylon.settlement.assignment_cs336_a2_benchmark_worker_20260611060805`, `receipt.nexus_pylon.settlement.assignment_cs336_a2_benchmark_validator_20260611060805`, adapter `mdk_agent_wallet`, 6 trace events each, public route 200s |
| window seal + reconcile (admin) | both windows `reconciled` with operator closeout + settlement receipts in `receiptRefs` |
| dataset admission | measurement rows (sampleCount 6 per metric across both Pylons, nearest-rank p50/p90/min/max, settlement receipt refs, verdict refs, modeled-from-measured earning estimate) validated through `admitCs336A2DeviceBenchmarkEvidence` + `publicDeviceCapabilityProjection` locally, then applied as one operator-staged D1 `UPDATE` (the deployed worker predates this commit's admission route) |
| public 200s | `route:/api/training/device-capabilities/a2` (`blockerRefs: []`, 4 cross-checked rows), `route:/api/training/leaderboards/a2_throughput` (4 ranked rows), `route:/api/training/runs/run.cs336.a2.device_capability.demo` (`verifiedWorkCount=4`, `reconciledWindowCount=2`, `assignedContributorCount=2`), both public settlement receipts |

## Earning estimates

Each row carries `basisLabel: modeled_from_measured_benchmark_distribution`
with `estimate_basis.cs336_a2.paid_sats_over_assignment_wall_time`:
30 paid sats over the measured benchmark-start-to-provider-confirmed-
settlement wall time per assignment (p50 285.8 sats/hour, p90 301.2
sats/hour across the two assignments). These are modeled prices of the
receipted closeouts, not earning guarantees.

## Spend accounting

| Movement | Sats |
| --- | --- |
| Worker-Pylon benchmark closeout (Lightning, warm channel) | 30 |
| Validator-Pylon benchmark closeout (Lightning, warm channel) | 30 |
| Total operator spend (cap 300) | 60 |
| Hosted MDK treasury spend | 0 |

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths appear in this document or in any public ref. Redacted
payment refs are sha256 derivations, not hash prefixes.

## Transition receipts

- New: `promise_transition_af246d4d-f37e-456a-b5ee-fba2d5ba3017`
  (`training.device_capability_dataset.v1`, planned -> yellow, result
  `passed`, checks `promise_exists`/`from_state_differs`/
  `evidence_refs_present`/`verification_named`, recorded against registry
  `2026-06-11.6` before the registry edit in this commit).

## What this commit adds (integrated surfaces)

- `cs336_a2_device_benchmark` is now a first-class
  `PylonApiAssignmentJobKind` literal; tonight's live assignments rode
  jobKind `claude_agent_task` with the A2 payload and
  `rail.job_kind.claude_agent_task_until_cs336_a2_device_benchmark_deploys`,
  the same documented rail the #4675 A1 run used pre-deploy.
- `src/cs336-a2-benchmark-workload.ts` (+ tests): the real measured
  benchmark suite with deterministic digests and injectable clock.
- `POST /api/training/runs/{trainingRunRef}/device-benchmark-evidence`
  (admin): the previously missing admission seam. Nothing could write
  `a2DeviceBenchmark.measurements` into a run projection before this
  route; it validates ordering/sample-count/receipt requirements and
  runs the privacy guard at admission time. OpenAPI operation
  `admitTrainingA2DeviceBenchmarkEvidence`.
- `attachRunEvidence` on `TrainingAuthorityStore` (single-statement D1
  `UPDATE`).

## Honest remainder (named gaps)

- `blocker.product_promises.same_host_replication_caveat`: both Pylons
  run on one physical machine. The same-class cross-check is real
  cross-process replication, not cross-machine replication; every
  dataset row carries `caveat.cs336_a2.single_physical_host_two_pylons`.
- `blocker.product_promises.second_device_class_missing`: one device
  class observed (`device_class.apple_silicon_macos.arm64`). Promise
  green requires at least two distinct device classes.
- `blocker.product_promises.thermal_throttle_detection_missing`:
  sustained-versus-burst thermal behavior and the mobile-GPU quirks
  taxonomy from the QVAC review are not yet measured or typed.
- `blocker.cs336_a2.job_kind_first_class_after_deploy`: the live worker
  predates the `cs336_a2_device_benchmark` job-kind literal and the
  evidence-admission route in this commit; tonight's dataset admission
  was an operator-staged D1 write of route-equivalent validated JSON.
  After the next deploy, admissions go through
  `POST /api/training/runs/{ref}/device-benchmark-evidence` and A2
  assignments dispatch under their own job kind.
- Operator-staged lane: dispatch, challenge create/finalize, closeout,
  and payout execution were operator actions (hosted-MDK programmatic
  payouts remain disabled). A standing benchmark market needs self-serve
  admission and automated settlement.
- Psionic external ask unchanged: owned Metal/CUDA attention kernels and
  real-transport DDP/FSDP graduate this from measurement to the real A2
  port (`scope.cs336_a2.psionic_kernel_and_transport_parity_external`).
