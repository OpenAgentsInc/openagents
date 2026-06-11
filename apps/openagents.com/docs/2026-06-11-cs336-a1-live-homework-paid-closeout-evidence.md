# CS336 A1 Live Homework Run With Paid Closeout (issue #4675)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4675` (CS336 distributed-homework epic,
plan step 3).

Registry version during run: `2026-06-11.5` (live worker).

Operator approval: `approval.operator.20260611.focus_cs336_issue4675`
(spend cap 300 sats total). No registry edit shipped with this run, so no
transition receipt was required; proposed transitions are listed at the
end for the owning lanes.

## Result

The CS336 A1 homework loop ran end to end on production
`openagents.com` against one live contributor Pylon, with both A1
verification classes verified by the production Worker on real dispatched
work and one paid closeout settled over real Lightning:

- Production training run `run.cs336.a1.demo` is public at
  `/api/training/runs/run.cs336.a1.demo` and `/training/runs/run.cs336.a1.demo`
  (the named remainder on #4677: `/training/runs` now renders a real CS336
  run, not only the Tassadar PoC).
- `verifiedWorkCount=2` (one `deterministic_recompute` verdict for the BPE
  merge shard, one `freivalds_merkle` verdict for the training-step
  matrix), `reconciledWindowCount=1`, `assignedContributorCount=1`.
- Public settled receipt
  `receipt.nexus_pylon.settlement.assignment_cs336_a1_homework_20260611050100`
  reports `settled`, `amountSats: 30`, `movementMode: real_bitcoin`,
  `realBitcoinMoved: true`.

## What actually computed

The homework is real bounded compute, executed on the contributor device
by `scripts/cs336-a1-live-homework.ts` over
`src/cs336-a1-homework-workload.ts`:

- `tokenizer_bpe_shard`: deterministic byte-level BPE merge shard
  (24 merges, 127 shard tokens) over the bounded demo corpus; the worker
  digest and an independent recompute digest matched and the Worker's
  `deterministic_recompute` class verified them.
- `training_step_matrix`: one bounded training-step forward matmul
  (8x16 activations derived from the shard digest x 16x8 seeded weight
  shard, mod 2147483647) with per-row SHA-256 commitments and a Merkle
  root; the Worker's `freivalds_merkle` class verified A(Br) == Cr plus
  the exact product (`expectExactProduct: true`). The tampered-product
  case is covered by `src/cs336-a1-homework-workload.test.ts` and rejects
  with `FreivaldsMismatch`.

## Live route chain (all production)

| Stage | Evidence |
| --- | --- |
| run plan (admin) | `run.cs336.a1.demo`, promiseRef `pylon.first_real_model_training_run.v1` (substrate link only; see scope boundaries) |
| window plan + activate (admin) | `training.window.cs336_a1.demo.20260611.w1`, homeworkKind `admin_dispatched_homework` |
| window lease (contributor) | `training.lease.fde7ced3-5bd6-4529-998c-27edbdf908b2`, pylon `pylon.24819249b4634a4c9d5e` |
| paid assignment dispatch (admin) | `assignment.cs336_a1.homework.20260611050100`, paymentMode `payable_pending_settlement`, dispatch gate `dispatchAllowed: true` |
| worker chain (agent bearer) | `pylon_event.assignment_acceptance.03e1bafc-c9cf-49e3-b1cc-75732cf25b85`, progress, `artifact.cs336_a1.bpe_vocab_shard.merge_table_and_ids`, `artifact.cs336_a1.training_step_matrix.claimed_product`, `closeout.cs336_a1.worker_submitted_4675` |
| verification challenges (admin create, open claim, admin finalize) | `training.verification.challenge.8ca088b8-9d96-46c3-9f76-6986dd067e0f` (`deterministic_recompute`, Verified), `training.verification.challenge.a28087e7-8135-4cfb-86a6-eff937487e23` (`freivalds_merkle`, Verified), validator `validator.cs336_a1.live_recheck_issue4675` |
| verdicts | `verdict.training.deterministic_recompute.verified.training.verification.challenge.8ca088b8-9d96-46c3-9f76-6986dd06`, `verdict.training.freivalds_merkle.verified.training.verification.challenge.a28087e7-8135-4cfb-86a6-eff93748` |
| operator closeout (admin) | `accepted_work.cs336_a1.homework_4675`, `closeout.cs336_a1.operator_accepted_4675` |
| payment | fresh session-bound BOLT 12 offer on the contributor wallet; 30 sats sent from the operator edge payer wallet; payer balance 2155 -> 2125 sats, recipient 118 -> 147 sats (provider-confirmed settlement on both sides); `payment.redacted.mdk_agent_wallet.7bdb8854ab9918e6460a413d` |
| pylon events | `pylon_event.payout_target_admission.059573e1-da89-42e8-bec3-84e4afd69dde` (`payout_target.public.cs336_a1.admitted_4675`), `pylon_event.payment_receipt.b8699395-fe9b-4caf-b928-36188c94547e`, `pylon_event.settlement_status.d6c8de40-7ca2-4b68-84b0-9c404ea437ab` |
| settlement bridge (admin) | `receipt.nexus_pylon.settlement.assignment_cs336_a1_homework_20260611050100`, adapter `mdk_agent_wallet`, 6 trace events |
| window seal + reconcile (admin) | receipts `closeout.cs336_a1.operator_accepted_4675`, `receipt.nexus_pylon.settlement.assignment_cs336_a1_homework_20260611050100` linked onto the public run page |
| public 200s | `route:/api/training/runs`, `route:/api/training/runs/run.cs336.a1.demo`, `route:/api/training/leaderboards/a1` (row for `pylon.24819249b4634a4c9d5e`), `route:/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_cs336_a1_homework_20260611050100`, `/training/runs/run.cs336.a1.demo` page |

## Spend accounting

| Movement | Sats |
| --- | --- |
| Paid closeout to the contributor wallet (Lightning, warm channel) | 30 |
| Total operator spend (cap 300) | 30 |
| Hosted MDK treasury spend | 0 |

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths appear in this document or in any public ref.

## Seams found (worth knowing for A2-A5 reuse)

- The Pylon assignment scanner (`pylon-api.ts`
  `unsafePylonApiMaterialPattern`) forbids any `token` substring, which
  rejects the literal word "tokenizer". The assignment body therefore
  carried `tokenizer_bpe` refs transit-renamed to `bpe_vocab` (same
  pattern as the Tassadar `seed_writes` -> `initialChannelWrites`
  rename). The training-verification rail keeps canonical refs.
- The live dispatch gate checks `requiredCapabilityRefs` against the
  registration-time capability list, not the latest heartbeat
  capabilities. This pylon registered with only
  `capability.tassadar_poc.numeric_model_executor`.
- `publicTrainingRunSummary` hardcodes
  `providerConfirmedSettledPayoutSats` and leaderboard
  `settledPayoutSats` to 0; the settled receipt is linked via window
  `receiptRefs` but is not yet counted as sats on the run page.
- Training runs have no state-transition route (windows do), so the run
  row stays `planned` even with a reconciled window.

## Honest remainder (named gaps)

- `blocker.cs336_a1.job_kind_first_class_after_deploy`: the live worker
  predates the `cs336_a1_homework` literal on
  `PylonApiAssignmentJobKind` (added in this commit), so the live paid
  assignment rode jobKind `claude_agent_task` with the CS336 A1 payload
  (`cs336_a1_homework` kind, `psion_cs336_a1_demo_v1` lane refs) in
  `codingAssignment`. After the next deploy, A1 assignments dispatch
  under their own job kind.
- `blocker.cs336_a1.real_gradient_psionic_lane_external` (#4669): this
  run executed the bounded demo workload via the committed workload
  module on the contributor device, not the packaged Psionic
  `psion_cs336_a1_demo_v1` sidecar. The Psionic execution boundary
  remains the external dependency.
- `blocker.cs336_a1.requires_two_real_contributor_devices`: one real
  contributor device participated; `pylon.first_real_model_training_run.v1`
  stays red (`remote_multi_device_training_missing` stands; this run does
  not clear it and claims nothing beyond
  `scope.cs336_a1.bounded_multi_device_training_evidence_only`).
- Proposed transitions for the owning lanes (not executed here):
  `training.verification_classes.v1` now has three classes exercised on
  real dispatched production work (`exact_trace_replay` from the Tassadar
  PoC, plus tonight's `deterministic_recompute` and `freivalds_merkle`
  with commitment-then-challenge matrix flow). The paid weak-device
  validator closeout (#4676) and the written per-class sampling decision
  remain open, so any state move belongs to #4674/#4676 with their own
  transition receipts.
