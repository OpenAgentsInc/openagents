# CS336 A1 Multi-Device Real-Gradient Training With Paid Closeouts (issue #4678)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4678` (CS336 distributed-homework epic,
plan step 6).

Registry version during the live run: `2026-06-11.7` (live worker
deployment `974b9fc2`). This commit ships `2026-06-11.8` with the
`pylon.first_real_model_training_run.v1` red -> yellow transition recorded
receipt-first as
`transition:promise_transition_7e5325b3-c06b-484b-9724-1e4fb41421c0`
(result `passed`, all four checks passed, recorded before the registry
edit).

Operator approval: `approval.operator.20260611.focus_cs336_issue4678`
(spend cap 300 sats total).

## Result

A bounded CS336 A1 real-gradient training run executed across two real
contributor devices on two distinct physical machines and device
classes, end to end on production `openagents.com`:

- Production training run `run.cs336.a1.real_gradient.demo` is public at
  `/api/training/runs/run.cs336.a1.real_gradient.demo` with
  `realGradient.externalAsk.status: "observed"` and `blockerRefs: []`.
- Devices: `pylon.24819249b4634a4c9d5e`
  (`device_class.apple_silicon_macos`, shard 0, lease
  `training.lease.58efbfb9-6794-4382-8911-d834fc1202f4`, window
  `training.window.cs336_a1_rg.demo.20260611.w1`) and
  `pylon.user-1e8fe007-c956-475f-adbe-429e5d9e4639`
  (`device_class.x86_64_linux`, a separate physical Linux box, shard 1,
  lease `training.lease.c715bd71-2faf-427d-b6e2-12b7a0b1a4ba`, window
  `training.window.cs336_a1_rg.demo.20260611.w2`).
- `verifiedWorkCount=3` (two cross-device `deterministic_recompute`
  verdicts, one `freivalds_merkle` verdict), `reconciledWindowCount=2`,
  `assignedContributorCount=2`.
- Two public settled receipts, both HTTP 200 with `state: settled`,
  `amountSats: 30`, `movementMode: real_bitcoin`,
  `realBitcoinMoved: true`:
  - `receipt.nexus_pylon.settlement.assignment_cs336_a1_real_grad_shard_0_20260611T1030`
  - `receipt.nexus_pylon.settlement.assignment_cs336_a1_real_grad_shard_1_20260611T1030`
- `/api/training/leaderboards/a1` ranks both device rows for this run
  with `bestValidationLoss: 1.3835386088779047` and
  `verifiedWindowCount: 2` each, under the standing scope boundaries
  (`scope.cs336_a1.bounded_multi_device_training_evidence_only`,
  `scope.cs336_a1.does_not_replace_qwen_finetune_gate_4670`,
  `scope.cs336_a1.no_first_real_training_run_green_copy_from_this_issue_alone`).

## What actually computed (real gradients, not finite differences)

The workload is the committed
`src/cs336-a1-real-gradient-workload.ts` (new in this commit), the
dispatchable monorepo mirror of the Psionic
`psion_cs336_a1_real_gradient_reference_v1` lane (psionic#1114): a tiny
A1-shaped LM (embedding -> RMSNorm -> single-head causal softmax
attention -> RMSNorm -> SwiGLU -> RMSNorm -> unembedding ->
cross-entropy; 2,244 f64 parameters; vocabulary 32; sequence length 16)
trained by synchronous data-parallel SGD with hand-derived analytic
backprop. The committed test pins every parameter tensor's analytic
gradient against central finite differences (worst relative error
< 1e-5), mirroring the Psionic lane's gradient-correctness bar.

Per step, each contributor device computed the full-batch analytic
gradient of its own 48-sequence data shard (768 data units per shard
per step) at the synchronized parameter state and committed to it with
a deterministic SHA-256 digest; the operator-side aggregation averaged
the two shard gradients, applied the SGD step, and measured held-out
validation loss (32 sequences). All transcendentals use
engine-independent IEEE-754 `detExp`/`detLn`, so digests match
bit-for-bit across architectures:

- Initial parameter state `p0` was generated independently on both
  machines and was byte-identical
  (`digest.sha256.4c3de6ca350bebea744a1acee4e91095fdef421edbe21891ca3a65fbe12f2943`,
  seeded from the committed A1 tokenizer shard digest
  `digest.sha256.097226577c582f296c8c3d050278007d68340963e1a24e7723e769c98a0e009c`).
- Cross-device recomputes matched in all four checks: the Linux box
  recomputed the Mac's shard-0 gradients (steps 0 and 5) and the Mac
  recomputed the Linux box's shard-1 gradients (steps 0 and 5), digest
  for digest.
- The shard-1 step-0 gradient file produced during this run is
  byte-identical to the partial-run state left by the prior lane on the
  contributor box (md5 `6766df878d6ad20bc20f4f6224171b8e`).

### Loss curve (validation, public on the run page)

| Step | Validation loss |
| --- | --- |
| 0 | 3.5197062668735226 |
| 1 | 2.951128363282113 |
| 2 | 2.5675990018969443 |
| 3 | 2.2080697872990354 |
| 4 | 1.8804644102382486 |
| 5 | 1.589493254125287 |
| 6 | 1.3835386088779047 |

Budget: `budget.cs336_a1.real_grad.uniform_baseline_ln32_6_steps`
(`maxValidationLoss = ln(32) = 3.4657359027997265`); final loss
1.3835 <= 3.4657, strictly decreasing at every step. Merged final state
`merge.cs336_a1.real_grad.aggregated_state.step_6.sha256_bda8a9edba1c18da`
(`digest.sha256.bda8a9edba1c18da74e4c138d66da65888d3fe51f9ae4538b9046f7d701bdbb3`).

Final-step shard losses: shard 0 `1.5579864299674817`
(`commitment.cs336_a1.real_grad.step_5_shard_0.sha256_db529d372ab556df`),
shard 1 `1.4213293251989427`
(`commitment.cs336_a1.real_grad.step_5_shard_1.sha256_6a7c02b2fda63968`).

## Live route chain (all production)

| Stage | Evidence |
| --- | --- |
| run + windows + leases (prior lane, kept) | `run.cs336.a1.real_gradient.demo`; windows w1/w2 (`admin_dispatched_homework`, dataset `dataset.cs336_a1.real_grad.seeded_structured_stream.v1`); leases above |
| paid assignment dispatch (admin) | `assignment.cs336_a1.real_grad.shard_0_20260611T1030` and `assignment.cs336_a1.real_grad.shard_1_20260611T1030`, jobKind `cs336_a1_homework` (first-class, post-#4675), paymentMode `payable_pending_settlement`, both gates `dispatchAllowed: true` |
| worker chains (agent bearers; the Linux box's bearer never left that box) | accept/progress/artifacts/closeout events per device, e.g. `pylon_event.assignment_acceptance.c74b2fc1-...` (Mac), `pylon_event.assignment_acceptance.baf9a960-...` (Linux); artifacts carry all six per-step gradient commitment refs per shard |
| verification challenges (admin create, open validator claim, admin finalize) | `training.verification.challenge.43a8dcca-f10f-45bb-b6c6-bb63f0c39efd` (`deterministic_recompute`, w1, Mac shard-0 step-5 digest recomputed by the Linux device, Verified), `training.verification.challenge.1b4d5279-b1f6-4aa6-a146-3102e3ae5e62` (`deterministic_recompute`, w2, Linux shard-1 step-5 digest recomputed by the Mac, Verified), `training.verification.challenge.09dfde91-4b58-4646-913c-24354823d41e` (`freivalds_merkle`, training-step matrix at the merged final state, `commitment.cs336_a1.real_grad.merkle_root.sha256_1687ac99274fb888`, Verified); validator `validator.cs336_a1.real_gradient_recheck_issue4678` |
| verdicts | `verdict.training.deterministic_recompute.verified.training.verification.challenge.43a8dcca-f10f-45bb-b6c6-bb63f0c3`, `verdict.training.deterministic_recompute.verified.training.verification.challenge.1b4d5279-b1f6-4aa6-a146-3102e3ae`, `verdict.training.freivalds_merkle.verified.training.verification.challenge.09dfde91-4b58-4646-913c-24354823` |
| operator closeouts (admin) | `closeout.cs336_a1.real_grad.operator_accepted_shard_{0,1}_4678`, both assignments `accepted_work` |
| payments | 30 sats per device from the operator edge payer over Lightning; edge balance 1455 -> 1395 sats; Mac contributor wallet 343 -> 373 sats; Linux contributor wallet logged a provider-confirmed `PaymentReceived` (29 sats net after a 1-sat receive-side LSP fee); redacted refs `payment.redacted.mdk_agent_wallet.551a9bfe0f49865fab8e444c` (shard 0), `payment.redacted.mdk_agent_wallet.3658b797dbbb59080d0927be` (shard 1) |
| pylon settlement events (agent bearers) | payment-receipt + settlement-status `settled` per device, e.g. `pylon_event.payment_receipt.b6396ed8-...`, `pylon_event.settlement_status.549e9a7a-...` (Linux) |
| settlement bridges (admin) | both receipts above, adapter `mdk_agent_wallet`, public receipt routes 200 |
| window seal + reconcile (admin) | both windows `reconciled` (see seam note below) |
| evidence admission | the full `Cs336A1RealGradientEvidenceRequest` validated locally through the committed `admitCs336A1RealGradientEvidence` seam against the exact remote run row, then applied as one operator-staged D1 `UPDATE` of route-equivalent validated JSON (the deployed worker predates this commit's admission route — same precedent as the #4679/#4681/#4682 passes) |
| public 200s | `route:/api/training/runs/run.cs336.a1.real_gradient.demo` (`realGradient` observed, 7-point loss curve, both closeout/device/loss requirements satisfied), `route:/api/training/leaderboards/a1` (both device rows ranked with real loss), both public settlement receipts |

## Spend accounting

| Movement | Sats |
| --- | --- |
| Paid closeout, shard 0 (Mac contributor wallet, Lightning) | 30 |
| Paid closeout, shard 1 (remote Linux contributor wallet, Lightning) | 30 |
| Total operator spend (cap 300) | 60 |
| Hosted MDK treasury spend | 0 |

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths appear in this document or in any public ref.

## Transition and registry

- Transition receipt (recorded first, at registry `2026-06-11.7`):
  `promise_transition_7e5325b3-c06b-484b-9724-1e4fb41421c0`, red ->
  yellow for `pylon.first_real_model_training_run.v1`, result `passed`.
- This commit's registry edit (`2026-06-11.8`): state `yellow`, blocker
  `remote_multi_device_training_missing` cleared, replaced by the honest
  remainder `blocker.product_promises.model_ladder_network_rungs_not_run`
  per the promise's own verification text (the green path runs through
  Tassadar executor training and the model ladder's network rungs on
  real contributor devices). The launch dashboard mirror row moved to
  `yellow` with the same remainder.

## Seams found

- The window seal/reconcile transitions for w1 were submitted with the
  shard-1 receipt refs and w2 with the shard-0 receipt refs (an
  operator-side shell array indexing slip). All four refs are real,
  settled, and scoped to this run; the precise per-device receipt
  mapping is carried by `realGradient.shardContributions[].receiptRefs`
  in the admitted public projection and by this document. Window
  transitions are single-shot, so the cross-attachment stands as
  recorded.
- Re-registering the Mac worker pylon to advertise
  `capability.cs336_a1.real_gradient_shard_executor` stripped its
  unreceipted `capability.tassadar_poc.numeric_model_executor` claim (the
  W4.1 admission working as designed) and reset heartbeat/wallet/version
  gate state, which had to be re-posted before dispatch.
- The receive-side LSP fee makes a 30-sat payment land as 29 sats on a
  fresh-channel recipient; the sender-side debit and the settlement
  receipt both record 30 sats.

## Honest remainder (named gaps)

- `blocker.product_promises.model_ladder_network_rungs_not_run`: this is
  one bounded two-device A1-scale run (2,244 parameters, 6 steps,
  seeded synthetic structured streams — not TinyStories/OWT corpora).
  The promise's green path (network-scale `training.model_ladder.v1`
  rungs on real contributor devices) has not run. No "first real
  training run" green copy ships from this issue.
- `remainder.cs336_a1.deploy_then_live_admission`: the
  `real-gradient-evidence` admission route and its OpenAPI entry ship in
  this commit but are not on the deployed worker; tonight's admission was
  an operator-staged D1 write of route-equivalent JSON validated through
  the exact committed seam. After the next deploy, admissions go through
  `POST /api/training/runs/{trainingRunRef}/real-gradient-evidence`.
- `blocker.cs336_a1.real_gradient_psionic_lane_external` (#4669): the
  devices executed the committed monorepo mirror of the Psionic
  analytic-backprop lane (psionic#1114), not the packaged Psionic
  sidecar. The packaged execution boundary remains the external
  dependency.
- Training runs still have no state-transition route, so the run row
  stays `planned` with two reconciled windows (known #4675 seam).
- Operator-staged lane: dispatch, challenge create/finalize, closeout,
  payment execution, and bridge were operator actions; a standing
  real-gradient training market needs self-serve admission and automated
  settlement.
