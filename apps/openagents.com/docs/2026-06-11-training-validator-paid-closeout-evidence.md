# Training Validator Paid Closeout Evidence (issue #4676)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4676` (CS336 distributed-homework epic,
plan step 4 — validator work as paid Pylon assignments, weak-device lane).

Registry version during run: `2026-06-11.5` (live worker). This commit
ships the `2026-06-11.6` registry edit; the
`training.verification_classes.v1` planned → yellow transition receipt
`promise_transition_0bfce0c5-e4dd-4d19-9221-4bc9504f2055` was recorded
live against `2026-06-11.5` (all checks passed) before the edit.

Operator approval: `approval.operator.20260611.focus_cs336_issue4676`
(spend cap 300 sats total).

## Result

A real, freshly registered validator Pylon claimed a training
verification challenge over another Pylon's verified CS336 A1
contribution, independently re-executed the bound `freivalds_merkle`
verification class on its own device, submitted the verdict with
evidence refs through the paid Pylon assignment rail, and received a
settled 30-sat Lightning payout with a public receipt:

- Validator Pylon `pylon.4f4ef3d029e57674be98` (`pylon-4676-validator`)
  was registered tonight from a fresh `PYLON_HOME` with
  `capability.public.training_verification` at registration time, plus a
  brand-new MDK wallet in its original wallet home.
- Worker Pylon under validation: `pylon.24819249b4634a4c9d5e`, whose
  `contribution.cs336_a1.assignment.cs336_a1.homework.20260611050100.homework`
  was produced in the #4675 paid homework run.
- Public settled receipt
  `receipt.nexus_pylon.settlement.assignment_public_training_validator_recheck_20260611053500`
  reports `settled`, `amountSats: 30`, `movementMode: real_bitcoin`,
  `realBitcoinMoved: true`.
- The public run page for `run.cs336.a1.demo` now reports
  `verifiedWorkCount=3` and links the validator's verdict ref.

## What the validator actually computed

The validator work is real bounded re-execution, not a rubber stamp.
`scripts/training-validator-live-verify.ts` ran on the validator device
with no network and no secrets:

- It independently recomputed the bounded CS336 A1 workload from
  `src/cs336-a1-homework-workload.ts` (BPE merge shard: 24 merges, 127
  shard tokens; training-step matmul with per-row SHA-256 commitments
  and Merkle root — the recomputed root matches the worker's live
  commitment `commitment.cs336_a1.merkle_root.sha256_14cd1fdf7d013cfe`).
- It re-ran the `freivalds_merkle` class locally through the same
  registry code the production Worker executes at finalize time
  (Freivalds round A(Br) == Cr mod p plus exact-product recompute), and
  produced an independent `Verified` verdict with
  `consensus_verified` single-validator projection (Freivalds quorum
  escalation applies to rejections only).

## Live route chain (all production)

| Stage | Evidence |
| --- | --- |
| validator agent + Pylon registration | `registration.pylon.4f4ef3d029e57674be98`, capabilities `capability.public.training_verification` + `capability.tassadar_poc.numeric_model_executor`, heartbeat `online` |
| wallet readiness + payout target | `pylon_event.wallet_readiness.ecb9adcf-67fe-4909-a89e-f5b54e4c55f5` (ready), `pylon_event.payout_target_admission.ef5a0110-bc42-4360-afa8-517c20676492` (`payout.bolt11.training_validator_4676_redacted`) |
| challenge create (admin) | `training.verification.challenge.8a74a531-8b0d-4392-a49d-ede5179f23f7` (`freivalds_merkle`, `validator_recheck`, run `run.cs336.a1.demo`, window `training.window.cs336_a1.demo.20260611.w1`) |
| self-validation block (live, dispatcher path) | dispatch attempt with validator == worker (`pylon.24819249b4634a4c9d5e` both sides) returned `kind: blocked`, `blocker.training_validator.self_validation`, nothing sent to the Worker (exit 1) |
| paid assignment dispatch (admin, bridge-guarded) | `assignment.public.training_validator.recheck_20260611053500`, jobKind `validation`, paymentMode `payable_pending_settlement`, dispatch gate `ready` with zero blockers (registration-time capability check passed) |
| challenge claim (validator) | `POST /api/training/verification/challenges/claim` leased to `pylon.4f4ef3d029e57674be98`, state `Leased` |
| validator chain (agent bearer) | accept, progress, `pylon_event.artifact_proof_metadata.cd417db7-7c3e-4c75-b997-1545d1b743b4` (verdict evidence refs incl. `verdict_evidence.training_validator.freivalds_merkle.verified`), `pylon_event.worker_closeout.c9c73e38-b074-41d9-8f25-6d36dcd9d79e` |
| finalize (admin) | challenge state `Verified`, `verdict.training.freivalds_merkle.verified.training.verification.challenge.8a74a531-8b0d-4392-a49d-ede5179f`, zero failure codes |
| operator closeout (admin) | `accepted_work.training_validator.freivalds_recheck_4676`, `closeout.training_validator.operator_accepted_4676`, state `accepted_work` |
| payment | 30 sats over Lightning from the operator edge payer wallet to the validator wallet (warm channel after the 100-sat hydration payment); payer 2025 -> 1995 sats, validator wallet 98 -> 127 sats (provider-confirmed both sides); `payment.redacted.mdk_agent_wallet.4a96a86d016b36c9fde9c578` |
| pylon events | `pylon_event.payment_receipt.12341bea-d300-440c-9d21-dcc102d7beaa`, `pylon_event.settlement_status.19df230b-1d86-497b-a235-690e08b7f745` |
| settlement bridge (admin) | `receipt.nexus_pylon.settlement.assignment_public_training_validator_recheck_20260611053500`, adapter `mdk_agent_wallet`, 6 trace events |
| public 200s | `route:/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_training_validator_recheck_20260611053500`, `route:/api/training/verification/challenges/training.verification.challenge.8a74a531-8b0d-4392-a49d-ede5179f23f7`, `route:/api/pylons/pylon.4f4ef3d029e57674be98`, `route:/api/training/runs/run.cs336.a1.demo` (verdict ref linked, `verifiedWorkCount=3`) |

## Spend accounting

| Movement | Sats |
| --- | --- |
| Operator hydration of the fresh validator wallet (Lightning, JIT channel fee absorbed by receiver) | 100 |
| Validator payout for the accepted Freivalds recheck | 30 |
| Total operator spend (cap 300) | 130 |
| Hosted MDK treasury spend | 0 |

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths appear in this document or in any public ref.

## Transition receipts

- New: `promise_transition_0bfce0c5-e4dd-4d19-9221-4bc9504f2055`
  (`training.verification_classes.v1`, planned -> yellow, result
  `passed`, recorded against registry `2026-06-11.5` before the registry
  edit in this commit). The paid weak-device validator closeout was the
  named in-flight item this receipt evidences; the per-class
  aggregate-only sampling re-decision (#4674) remains the open blocker
  keeping the promise off green.

## Honest remainder (named gaps)

- `blocker.training_validator.operator_staged_lane`: the validator lane
  ran operator-staged (operator-funded validator wallet, operator
  challenge creation/finalize, operator dispatch/closeout, operator
  payout execution because hosted-MDK programmatic payouts remain
  disabled on the production account). A standing validator market needs
  self-serve claim and automated settlement.
- `blocker.training_validator.window_receipt_link_terminal`: the
  validator settlement receipt is publicly retrievable and the verdict
  ref is linked on the run page, but the #4675 window
  `training.window.cs336_a1.demo.20260611.w1` was already `reconciled`
  (a terminal state), so the validator receipt could not be appended to
  the window `receiptRefs`.
- `blocker.training_validator.challenge_payload_distribution`: the
  public challenge projection intentionally omits the payload; tonight's
  validator recomputed it deterministically from the committed workload
  module. Non-deterministic workloads need a payload-distribution seam
  (artifact storage refs) before validators can re-execute them.
- Rejection-quorum behavior (two distinct validator Pylon refs before a
  Freivalds rejection blocks a worker payout) is enforced and tested in
  `src/training-validator-assignments.test.ts`; no live rejection was
  manufactured against a real worker's payout tonight.
- `seeded_replication` and `statistical_cross_check` have not yet run on
  real dispatched work; the promise stays yellow with #4674's written
  per-class sampling decision outstanding.
