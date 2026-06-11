# CS336 A5 Rollout And Grading: First Paid Verified Alignment Homework (issue #4682)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4682` (CS336 distributed-homework epic,
plan step 10 — A5 rollout and grading homework: RL fed by the compute
market).

Registry version during run: `2026-06-11.7` (live worker `241db545`).
This commit ships **no** registry edit and records **no** promise
transition: the named promise `training.post_training_arc.v1` stays
`planned` behind `blocker.product_promises.instruct_sft_lane_missing`,
`blocker.product_promises.preference_rollout_work_missing`, and
`blocker.product_promises.vibe_test_artifact_missing`. None of those is
honestly cleared by one bounded synthetic rollout/grading run: no SFT
lane ran (the `cs336_a5_sft_packing` kind was not dispatched), the
rollouts are reward-graded rollouts rather than preference/DPO pairwise
work, and no vibe-test artifact exists.

Operator approval: `approval.operator.20260611.focus_cs336_issue4682`
(spend cap 300 sats). **Spend this leg: 40 sats.**

## Result

The deployed public CS336 A5 eval dashboard at
`GET /api/training/evals/a5` left its honest empty state for the first
time and now serves, with `blockerRefs: []`:

- One receipted public eval suite
  `eval.cs336_a5.synthetic_math.bounded_combined.4682.1`:
  `taskSetRef: math`, `metric: accuracy`, `score: 0.59375`,
  `sampleCount: 256`, `verifiedSampleCount: 256`, four settlement
  receipt refs and four Verified challenge refs, scope-labeled
  `eval_results_not_capability_claims`,
  `no_raw_prompts_or_answers_public`, and
  `update_step_waits_on_training_boundary_4669`.
- `GET /api/training/leaderboards/a5_accuracy` now ranks its first row
  (rank 1, score 0.59375, `metric.cs336_a5.math.accuracy`, four receipt
  refs), leaving
  `blocker.training_leaderboard.a5_accuracy.requires_verified_receipts`
  for the first time.

The task set is honestly labeled `math`: a bounded **synthetic**
seeded arithmetic task set, not GSM8K or MMLU. No hosted LLM was
involved anywhere (per the TOP-FOCUS scope: deterministic/verifiable,
not hosted-LLM-dependent), and the eval score is evidence about this
bounded artifact only.

## What actually computed

`scripts/cs336-a5-alignment-run.ts` executed
`src/cs336-a5-rollout-workload.ts` on the contributor device per stage:

- **Rollout batches** (`cs336_a5_rollout_batch`,
  `seeded_replication`): a seeded stochastic policy sampled 4 rollouts
  for each of 32 seeded synthetic arithmetic tasks per split (128
  rollouts per split), emitting GSM8K-format `#### <value>` completions
  with task-dependent correctness propensity and a seeded ~5% malformed
  fraction. The full rollout set is a pure function of the committed
  seed, so an opposite-Pylon replay reproduces the digest exactly.
- **Reward grading** (`cs336_a5_reward_grading`,
  `deterministic_recompute`): bounded GSM8K-format final-value
  extraction, exact-match rewards against the task reference values,
  and group-normalized advantages per task group (committed at fixed
  six-decimal precision), mirroring the group-normalized rewards in the
  Psionic `psion_cs336_a5_alignment_reference_v1` lane
  (`psionic#1101`). The grading digest binds the exact rollout input
  set, so tampering any completion changes the commitment even when the
  reward vector is unchanged.
- Task seeds derive from the A1 tokenizer shard digest
  (`cs336-a1-homework-workload.ts`), binding A5 to the same committed
  corpus pipeline as the #4675/#4679/#4680 runs.

| Stage | Split | Output digest (prefix) | Public counts |
| --- | --- | --- | --- |
| `rollout_batch` | `split_a` | `136c47278a947675…` | 128 rollouts, 32 tasks, group size 4 |
| `reward_grading` | `split_a` | `9eb8041009e49735…` | 128 graded, 83 correct (accuracy 0.6484), 4 unparseable, 32 groups |
| `rollout_batch` | `split_b` | `9e888bdce419bcbb…` | 128 rollouts, 32 tasks, group size 4 |
| `reward_grading` | `split_b` | `d7554d7274b75a95…` | 128 graded, 69 correct (accuracy 0.5391), 8 unparseable, 32 groups |

Combined: 152/256 correct = 0.59375 accuracy.

## Live route chain (all production)

| Stage | Evidence |
| --- | --- |
| run plan (admin) | `run.cs336.a5.alignment.demo`, promiseRef `training.post_training_arc.v1` (substrate link only; no transition claimed) |
| window plan + activate (admin) | `training.window.cs336_a5.demo.20260611.w1` (worker Pylon), `training.window.cs336_a5.demo.20260611.w2` (validator Pylon), homeworkKind `admin_dispatched_homework`, dataset `dataset.cs336_a5.bounded_synthetic_math_tasks.v1` |
| window leases (contributors) | `training.lease.81649d57-8387-41ba-a062-fcfe665ca34e` (w1, `pylon.24819249b4634a4c9d5e`), `training.lease.efb47a8c-98c4-4763-baa8-194ff16328a7` (w2, `pylon.4f4ef3d029e57674be98`) |
| wallet readiness + payout targets | `pylon_event.wallet_readiness.3223afc3-544b-4ad4-9fcf-b7d463345431` / `pylon_event.payout_target_admission.de293f84-ae8e-409d-b648-84fcb4629a38` (`payout_target.public.cs336_a5.admitted_4682_worker`), `pylon_event.wallet_readiness.d42c01d9-b82b-4597-b248-5c5f01164e99` / `pylon_event.payout_target_admission.b71450a4-a13d-4bd2-a325-828af70d9609` (`payout_target.public.cs336_a5.admitted_4682_validator`) |
| paid assignment dispatch (admin) | `assignment.cs336_a5.alignment.{rollout_batch_split_a,reward_grading_split_b,rollout_batch_split_b,reward_grading_split_a}_20260611032611`, paymentMode `payable_pending_settlement`, jobKind rail `claude_agent_task` with the `cs336A5` payload and `rail.job_kind.claude_agent_task_until_cs336_a5_alignment_deploys`; dispatch gate `ready` all four, one active assignment per Pylon, stages split worker/{rollout split_a, grading split_b} validator/{rollout split_b, grading split_a} — each Pylon grades the *other* Pylon's split |
| worker chains (agent bearer) | per-stage acceptance, progress, artifact proof metadata (`commitment.cs336_a5.rollout_batch.split_a.sha256_136c47278a947675`, `commitment.cs336_a5.reward_grading.split_b.sha256_d7554d7274b75a95`, `commitment.cs336_a5.rollout_batch.split_b.sha256_9e888bdce419bcbb`, `commitment.cs336_a5.reward_grading.split_a.sha256_9eb8041009e49735` + public count result refs), worker closeouts |
| verification (admin create, open claim, admin finalize) | four challenges, all `Verified`, zero failure codes: `training.verification.challenge.cb1d4f39-5b33-4650-8659-afcc33131af5` (rollout split_a, `seeded_replication`, samplingPolicy `aggregate`, observed replication match score 1 against minimum 1), `.ae8e57a5-d37a-4f3d-9276-cc9c3d4237ff` (rollout split_b, `seeded_replication`), `.9fdd8d75-12a0-4688-bdb1-9c56664637b0` (grading split_b, `deterministic_recompute`, samplingPolicy `per_contribution`), `.9f8696c1-105c-46d6-b3c7-5c1618d290ce` (grading split_a, `deterministic_recompute`); each payload binds expected vs opposite-Pylon re-executed full-digest refs; validator `validator.cs336_a5.{seeded_replication,deterministic_recompute}_issue4682` |
| operator closeouts (admin) | `accepted_work.cs336_a5.{stage}_{split}_4682_commitment_matched` x4, all paid assignments `accepted_work`; acceptance basis is the pre-registered deterministic commitment match with the opposite-Pylon re-execution verdict landed before payment |
| payments | 4 x 10 sats over Lightning from the operator edge payer wallet (warm channels); payer 1655 -> 1615, worker wallet 314 -> 334, validator wallet 294 -> 313 (provider-confirmed both sides; a 1-sat receive-side fee is visible on one validator receive in the wallet ledger); redacted payment refs `payment.redacted.mdk_agent_wallet.{cb5b52c47dd3668218c04a9e, 9f9e81bb85ed6dd46ad1d9f8, 7d8713a4f0b5d2ebf214d775, 72f60e4e44bc1cdd60a612a0}` (sha256 derivations over private payment material, not hash prefixes) |
| pylon events | per-stage `pylon_event.payment_receipt.{c5a66990…, d3d269a2…, 11a9889c…, 0d946d77…}` and `pylon_event.settlement_status.{b26e92a2…, 9ec33a81…, f8846de7…, 9c30b395…}` |
| settlement bridges (admin) | `receipt.nexus_pylon.settlement.assignment_cs336_a5_alignment_{rollout_batch_split_a,reward_grading_split_b,rollout_batch_split_b,reward_grading_split_a}_20260611032611`, adapter `mdk_agent_wallet`, all four public receipt routes 200 (`settled`, `amountSats: 10`, `movementMode: real_bitcoin`, `realBitcoinMoved: true`) |
| window seal + reconcile (admin) | both windows `reconciled` (w1 seal `closeout.cs336_a5.operator_accepted_rollout_batch_split_a_4682`, reconcile `receipt.nexus_pylon.settlement.assignment_cs336_a5_alignment_rollout_batch_split_a_20260611032611`; w2 seal `closeout.cs336_a5.operator_accepted_reward_grading_split_a_4682`, reconcile `receipt.nexus_pylon.settlement.assignment_cs336_a5_alignment_reward_grading_split_a_20260611032611`) |
| evidence admission | eval suite + four work shards validated locally through the committed `admitCs336A5AlignmentEvidence` and re-projected through the exact deployed `publicCs336A5EvalProjection` code path against the exact remote run row, then applied as one operator-staged D1 `UPDATE` of route-equivalent validated JSON (the deployed worker predates this commit's admission route — same precedent as the #4679/#4681 passes) |
| public 200s | `route:/api/training/evals/a5` (`blockerRefs: []`, 1 receipted suite, 4 Verified verification refs), `route:/api/training/leaderboards/a5_accuracy` (first ranked row, lane blockers cleared), `route:/api/training/runs/run.cs336.a5.alignment.demo`, all four public settlement receipts |

## Spend accounting

| Movement | Sats |
| --- | --- |
| 4 alignment-stage closeouts x 10 sats (Lightning, warm channels) | 40 |
| Total operator spend (cap 300) | 40 |
| Hosted MDK treasury spend | 0 |

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens,
or wallet-home paths appear in this document or in any public ref.
Redacted payment refs are sha256 derivations, not hash prefixes.

## What this commit adds (integrated surfaces)

- `src/cs336-a5-rollout-workload.ts` (+ tests): the real bounded
  rollout/grading workload — seeded synthetic task generation, the
  seeded rollout policy, bounded GSM8K-format final-value parsing,
  exact-match rewards, group-normalized advantages, digest commitments
  with input binding, and the eval-suite summary builder with honest
  `math` task-set labeling.
- `src/training-alignment-evals.ts` (+ tests): the previously missing
  admission seam. `admitCs336A5AlignmentEvidence` enforces
  receipted-suites-only, sample-count and score bounds,
  receipted-shards-only, and a public-safety guard at admission time.
- `POST /api/training/runs/{trainingRunRef}/alignment-eval-evidence`
  (admin) with OpenAPI operation `admitTrainingA5AlignmentEvalEvidence`.
- `cs336_a5_alignment` is now a first-class
  `PylonApiAssignmentJobKind` literal; tonight's live assignments rode
  jobKind `claude_agent_task` with the `cs336A5` payload under the
  documented rail, the same pattern as the A1/A3/A4 pre-deploy runs.
- `scripts/cs336-a5-alignment-run.ts`: the contributor-side executor
  (public-safe output only; no network, no secrets, no spend).
- `smoke:cs336-a5:alignment` now covers the workload, the admission
  seam, the route, and the leaderboard wiring.

## Verification (all green, from `apps/openagents.com/workers/api`)

- `bun run smoke:cs336-a5:alignment` — 5 files / 26 tests passed.
- `bunx vitest run src/cs336-a5-rollout-workload.test.ts
  src/training-alignment-evals.test.ts src/openagents-openapi-routes.test.ts
  src/pylon-api-routes.test.ts` — passed.
- `bun run typecheck` — exit 0.
- `bun run check:architecture` — passed.
- `bun run check:effect-topology` — passed.

## Registry decision (propose only, no edit)

`training.post_training_arc.v1` stays `planned` at `2026-06-11.7`.
Tonight's leg supplies the rollout/grading dispatch-and-verification
substrate the promise's update step will consume, but none of the three
standing blockers is honestly cleared (no SFT lane ran, reward-graded
rollouts are not preference rollouts, no vibe-test artifact exists).
Proposed for the registry-owning lane: refresh the promise `safeCopy`
to note that the first paid + verified A5 rollout/grading assignments
ran on a bounded synthetic math task set with public settled receipts
and a receipted public eval suite, without any state move.

## Honest remainders (named gaps)

- `remainder.cs336_a5.synthetic_math_not_gsm8k_or_mmlu`: the receipted
  eval suite is a bounded synthetic arithmetic task set labeled `math`.
  Real GSM8K/MMLU suites stay blocked behind the Psionic parser
  fixture-conformance ask
  (`psionic.todo.parser_fixture_conformance_before_paid_grading`) and a
  real model-backed rollout source.
- `remainder.cs336_a5.seeded_sampler_policy_not_model_policy`: the
  rollout policy is a seeded stochastic sampler, not a language model.
  Model-backed rollouts wait on the kind-5050 serving lane (#4638) and
  the model-coupled response log-probs ask
  (`psionic.todo.response_log_probs_model_coupled`).
- `remainder.cs336_a5.policy_gradient_update_waits_on_4669`: nothing
  here performs or claims a GRPO update; the dashboard still publishes
  `updateBoundaryRef: issue.github.openagents.4669`.
- `remainder.cs336_a5.sft_packing_not_dispatched`: the
  `cs336_a5_sft_packing` job kind remains contract-only; no SFT shard
  was executed or paid.
- `remainder.cs336_a5.deploy_then_live_admission`: the
  `alignment-eval-evidence` admission route is not on the deployed
  worker; tonight's admission was an operator-staged D1 write of
  route-equivalent validated JSON. After the next deploy, admissions go
  through the route and A5 assignments can dispatch under their own job
  kind.
- `caveat.cs336_a5.single_physical_host_two_pylons`: both Pylons run on
  one physical machine; the opposite-Pylon re-runs are real
  cross-process re-executions, not cross-machine replication.
- `caveat.cs336_a5.run_state_stays_planned`: training runs still have
  no state-transition route, so the run row stays `planned` with two
  reconciled windows (known #4675 seam).
- Operator-staged lane: dispatch, challenge create/finalize, closeout,
  payment execution (hosted-MDK programmatic payouts remain disabled),
  and bridge were operator actions; a standing rollout/grading market
  needs self-serve admission and automated settlement.
