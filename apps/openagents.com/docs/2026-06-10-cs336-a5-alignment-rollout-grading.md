# CS336 A5 Alignment Rollout And Grading Homework

Date: 2026-06-10

Issue: `OpenAgentsInc/openagents#4682`

Forum claim: `https://openagents.com/forum/t/7417ea4f-84fa-4954-9f00-93cf04d64196`

## Scope

This slice adds the OpenAgents Worker contracts for CS336 A5 rollout, reward
grading, and SFT packing homework. It also adds a public eval-suite projection
for receipted MMLU/GSM8K summaries.

The public route is:

- `GET /api/training/evals/a5`

The no-spend smoke is:

- `bun run smoke:cs336-a5:alignment`

## Job Kinds

The dispatchable job kinds are:

- `cs336_a5_rollout_batch`
- `cs336_a5_reward_grading`
- `cs336_a5_sft_packing`

Rollout batches use `seeded_replication` verification because seeded sampling
can be repeated. Reward grading and SFT packing use
`deterministic_recompute`.

All three job contracts point at Psionic lane
`psion_cs336_a5_alignment_reference_v1`, request schema
`openagents.cs336_a5_alignment_homework_request.v1`, and output schema
`openagents.cs336_a5_alignment_homework_output.v1`.

## Update Boundary

The policy-gradient update step is not implemented in this issue. It remains
behind `issue.github.openagents.4669`, where the Psionic training boundary
owns model-coupled response log-probs and `grpo_train_step`.

This issue only supplies rollout, grading, SFT-packing, and public eval inputs
that the update boundary can consume.

## Public Eval Projection

The public projection exposes only eval-suite summaries:

- eval suite ref
- task set: `gsm8k`, `mmlu`, or `math`
- split ref
- metric
- score
- sample count
- verified sample count
- receipt refs
- verification refs
- scope labels

Rows are labeled as eval results, not model capability claims. The projection
rejects raw prompts, answers, completions, wallet material, payment material,
private paths, and secret-shaped material before publication.

## Psionic External Asks

Psionic-side first tranche landed in `OpenAgentsInc/psionic#1101` under
`psion_cs336_a5_alignment_reference_v1`.

Remaining asks:

- response log-probs with model-coupled execution
- `grpo_train_step` forward/backward through the #4669 training boundary
- fixture conformance for bounded MMLU/GSM8K parsers before paid grading

## Current Live Boundary

The route can be deployed before real eval rows exist. With no receipted eval
suite rows, it returns blockers:

- `blocker.cs336_a5.requires_rollout_receipts`
- `blocker.cs336_a5.requires_grading_verification`
- `blocker.cs336_a5.requires_public_eval_suite_receipt`
- `blocker.cs336_a5.policy_gradient_update_waits_on_4669`

Update (2026-06-11): the first paid + verified rollout/grading run cleared
those route blockers on production with a receipted bounded synthetic `math`
eval suite (`eval.cs336_a5.synthetic_math.bounded_combined.4682.1`). The real
workload lives in `src/cs336-a5-rollout-workload.ts`, the admission seam in
`src/training-alignment-evals.ts` plus
`POST /api/training/runs/{trainingRunRef}/alignment-eval-evidence`, and the
full live evidence in
`docs/2026-06-11-cs336-a5-rollout-grading-paid-evidence.md`. A real GSM8K or
MMLU suite remains blocked behind the Psionic parser fixture-conformance ask
and a model-backed rollout source; the policy-gradient update still waits on
#4669.
