# training.post_training_arc.v1 — vibe-test rubric + DPO preference workload

Date: 2026-06-20

---

## 2026-06-20 (c) — public vibe-test rubric projection

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip).

### Blocker advanced

`blocker.product_promises.vibe_test_artifact_missing` — advanced, **not
cleared**.

The rubric module from the previous pass is now exposed as a public-safe,
live-at-read projection:

- `GET /api/public/training/post-training-arc/vibe-test-rubric`
- `apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.ts`
- `apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.test.ts`

The projection publishes only refs, counts, aggregate stats, gate booleans, and
the deterministic closeout digest for
`rubric.training.post_training_arc.vibe_test.v1` /
`receipt.training.post_training_arc.vibe_test_rubric.fixture_closeout.v1`:

- `rubricAvailable=true`
- `deterministicCloseoutDigestAvailable=true`
- `repoOwnedFixtureTranscriptsAvailable=true`
- `closeoutAcceptable=true`
- `realModelTranscriptArtifactAvailable=false`
- `reviewerSignedCloseoutAvailable=false`
- `vibeTestArtifactAvailable=false`
- `greenGateSatisfied=false`

Registry version `2026-06-20.44` cites this projection as evidence. It does not
clear `vibe_test_artifact_missing`, because the closeout still uses repo-owned
fixture text rather than real Psion model transcripts and has no reviewer
signature.

### Honesty boundary

No assignment, spend, settlement, model promotion, fine-tuning service, reviewed
vibe-test artifact, or green transition is created. The remaining vibe-test gate
is a reviewer-signed closeout over real model transcripts, referenced from the
post-training closeout artifact.

---

## 2026-06-20 (b) — vibe-test rubric reference module

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip, no
registry edit).

### Blocker advanced

`blocker.product_promises.vibe_test_artifact_missing` — advanced, **not
cleared**.

The arc copy requires "a reviewed vibe-test transcript artifact in the
closeout" to gate each post-training checkpoint promotion. That review was ad
hoc — there was no owned, versioned rubric and no deterministic scorer to turn a
transcript set into a reproducible closeout decision. This change adds the
machine-checked half of that artifact.

### What was built

- `apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.ts`
  - An owned, versioned rubric `rubric.training.post_training_arc.vibe_test.v1`
    with five deterministic, weighted criteria: `nonempty_response`,
    `within_length_budget`, `instruction_followed`, `refusal_when_required`
    (safety-critical), and `no_unsafe_leakage` (safety-critical, reusing the
    instruct-SFT public-safety pattern).
  - Exact scoring: `scoreVibeTestTranscript` (weight-fraction score plus a
    safety gate) and `gradeVibeTestCloseout` (mean score, pass rate,
    `allSafetyPassed` — a single safety failure blocks the closeout regardless
    of average quality).
  - `buildVibeTestExampleTranscripts` repo-owned fixture transcripts and
    `runPostTrainingVibeTestCloseout`, which reproduces its `closeoutDigestHex`
    bit-for-bit on re-run. `reviewerSigned` is always `false`.
- `apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.test.ts`
  - 9 committed tests: perfect/marker-miss scoring, safety-comply block,
    credential-leak detection, empty/duplicate-ref guards, example-set
    acceptance, deterministic digest, safety-regression digest change, and the
    default threshold.

### Honesty boundary (why the blocker stays open)

The example transcripts are **repo-owned fixture text**, not outputs of a real
Psion instruct model, and **no human reviewer has signed a closeout**
(`reviewerSigned` is hard-coded `false`). This module produces only the
machine-checked half — the exact, unit-tested rubric and a reproducible
closeout digest a reviewed vibe-test would settle against. It is a prerequisite
for the reviewed artifact, not the artifact itself.

### Validation

- `bunx tsc -p tsconfig.json --noEmit` (workers/api): the two new files add **0
  errors**. One pre-existing, unrelated error remains at HEAD without this
  change — `src/training-data-refinery.ts(18,3): TS6133
  'Cs336A4EvalDeltaMeasurementRef' is declared but its value is never read`
  (the `training.data_refinery_corpus.v1` promise, untouched here).
- `bun run check:deploy`: passes (exit 0).

### What remains for `vibe_test_artifact_missing`

Still open: no real Psion instruct-model transcripts have been graded, and no
owner/reviewer-signed closeout artifact referencing this rubric exists. A green
closeout still needs reviewed real transcripts plus a human signature.

---

## 2026-06-20 (a) — DPO preference-pair reference workload

Date: 2026-06-20

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip).

## Blocker advanced

`blocker.product_promises.preference_rollout_work_missing` — advanced, **not
cleared**.

The promise already holds GRPO rollout/grading as paid network work (the
2026-06-11 CS336 A5 alignment run). What was missing for the preference lane was
the pairwise-preference (DPO) side: there was no reference math and no
deterministic grading function for preference pairs. This change adds that
reference math with committed tests — the verifiable grading function a paid
`cs336_a5_dpo_grading` dispatch would settle against.

## What was built

- `apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.ts`
  - Exact DPO reference math: `softplus` (numerically stable),
    `dpoImplicitReward` (`beta * (logpPolicy - logpReference)`), `dpoPairLoss`
    (`-log sigmoid(margin) = softplus(-margin)`), and `gradeDpoPreferenceBatch`
    (mean loss, ranking accuracy, mean implicit rewards).
  - `buildCs336A5PreferencePairs`: constructs bounded (chosen, rejected) pairs
    from the SAME seeded synthetic math environment as the existing GRPO rollout
    workload (`cs336-a5-rollout-workload.ts`), reusing `buildCs336A5Tasks`,
    `runCs336A5RolloutBatch`, and the exact-match `parseCs336A5FinalValue`
    reward. Pair records expose only numeric log-probs and refs — no prompts,
    completions, or weights.
  - `runCs336A5DpoPreferenceGrading`: end-to-end `deterministic_recompute`
    grading that reproduces its `outputDigestHex` bit-for-bit on re-run.
- `apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.test.ts`
  - 9 committed tests: softplus stability at extremes, implicit-reward formula,
    zero-margin loss = `log 2`, loss-decreases-when-policy-prefers-chosen,
    beta/empty-batch guards, batch aggregation, public-safety of pairs, and
    cross-split deterministic digest recompute.

## 2026-06-20 public projection wiring

The DPO reference workload now has a public-safe live-at-read projection:

- `GET /api/public/training/post-training-arc/dpo-preference-workload`
- `apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.ts`
- `apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.test.ts`

The projection publishes the deterministic reference receipt for
`workload.cs336_a5.dpo_preference_pair_reference_grading.v1`:

- `deterministicReferenceWorkloadAvailable=true`
- `splitRef=split_a`
- `pairCount=25`
- `outputDigestHex=ad419c324105c46a889bd5cd13a9e94d66fe9166b6763a0a2add0c77c938ac62`
- `paidPreferenceDispatchAvailable=false`
- `realModelLogprobMeasurementAvailable=false`
- `verifiedChallengeAvailable=false`
- `settlementReceiptAvailable=false`
- `dpoUpdateAvailable=false`
- `preferenceRolloutWorkAvailable=false`
- `greenGateSatisfied=false`

This makes the missing paid-work boundary inspectable. It does not claim a paid
`cs336_a5_dpo_grading` dispatch, real policy/reference-model log-probs,
settlement, DPO update, model promotion, or green transition.

## Honesty boundary (why the blocker stays open)

The per-response log-probs are **synthetic** — derived deterministically from
the completion text and the exact-match reward (the policy is nudged toward the
chosen response). No hosted LLM and no real policy/reference model is queried.
The contribution is the exact, unit-tested DPO reference math plus a
deterministic grading digest; it is a prerequisite for paid preference work, not
the paid work itself. The DPO/policy-gradient update step also stays behind the
`#4669` training boundary.

## What genuinely remains for this promise

- `blocker.product_promises.preference_rollout_work_missing` — still open: no
  paid `cs336_a5_dpo_grading` dispatch / settlement / Verified challenge over
  preference pairs has run; real model log-probs are not yet wired.
- `blocker.product_promises.instruct_sft_paid_dispatch_missing` — unchanged.
- `blocker.product_promises.vibe_test_artifact_missing` — unchanged.
