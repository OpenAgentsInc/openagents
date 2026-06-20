# training.post_training_arc.v1 — vibe-test rubric + DPO preference workload

Date: 2026-06-20

---

## 2026-06-20 (h) — vibe-test closeout grading challenge bridge

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip, no
registry edit).

### Blocker advanced

`blocker.product_promises.vibe_test_artifact_missing` — advanced, **not
cleared**.

The DPO and instruct-SFT lanes already had a committed grading-challenge bridge
(`cs336-a5-dpo-grading-challenge.ts`, `psion-instruct-sft-grading-challenge.ts`)
turning their deterministic answer keys into the `deterministic_recompute`
challenge shape the 2026-06-11 alignment run used for its four Verified
challenges. The vibe-test lane had the rubric + closeout digest + public
projection but **no such bridge** — there was no committed path from the
`closeoutDigestHex` answer key to the verification envelope a paid/reviewed
vibe-test grading dispatch would settle against. This change adds that bridge.

### What was built

- `apps/openagents.com/workers/api/src/post-training-vibe-test-grading-challenge.ts`
  - `buildPostTrainingVibeTestGradingChallengeSpec`: recomputes the closeout
    in-repo and packages a public-safe `deterministic_recompute` challenge spec
    (`expectedDigestHex`, `transcriptCount`, `meanScoreMicro`, `thresholdMicro`,
    `closeoutAcceptable`, `reviewerSigned=false`, refs). Exposes only
    digests/counts/refs/stats — never prompts, completions, or transcript text.
  - `verifyPostTrainingVibeTestGradingResponse`: performs a TRUE in-repo
    recompute (the rubric scorer runs in this worker, unlike the SFT Rust lane),
    rejecting a stale/forged spec and verifying a worker's CLAIMED closeout
    digest. `Verified` only when stored expected, fresh recompute, and the claim
    all agree; otherwise `Rejected` with `DigestMismatch` /
    `OutputDigestMissing` / `DimensionMismatch` / `VerificationClassUnknown`.
  - `buildPostTrainingVibeTestGradingChallengeCreateRequest`: bridges the spec
    into the rail-side `TrainingVerificationChallengeCreateRequest` envelope and
    **decodes it against the real `training-verification` schema**, throwing
    `PostTrainingVibeTestGradingChallengeError` on a structurally invalid
    request (e.g. a non-public-safe `trainingRunRef`).
- `apps/openagents.com/workers/api/src/post-training-vibe-test-grading-challenge.test.ts`
  - 16 committed tests: deterministic answer-key spec build matching the
    closeout, re-build stability, threshold-range guard, spec public-safety,
    Verified happy path, case-insensitive digest, tampered/malformed-claim
    rejects, transcript-count-mismatch reject, stale-spec reject,
    malformed-expected-digest reject, schema-valid create-request with
    round-trip decode, `windowRef` omission, payload public-safety,
    non-public-safe `trainingRunRef` reject, and malformed-spec-digest reject.

### Honesty boundary (why the blocker stays open)

The transcripts remain REPO-OWNED FIXTURE TEXT, not real Psion instruct-model
outputs, and `reviewerSigned` is hard-coded `false` — this module never forges a
reviewer signature. It only constructs/verifies/validates the challenge request;
it submits nothing, takes no lease, spends no sats, settles nothing, and creates
no rail-side challenge. No public route, registry edit, or green transition is
added.

### What genuinely remains for `vibe_test_artifact_missing`

Still open: no real Psion instruct-model transcripts have been graded, no
reviewer-signed closeout artifact exists, and no paid/reviewed vibe-test grading
dispatch / lease / settlement / on-rail Verified challenge has run.

---

## 2026-06-20 (g) — instruct-SFT lane grading challenge bridge

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip, no
registry edit).

### Blocker advanced

`blocker.product_promises.instruct_sft_paid_dispatch_missing` — advanced, **not
cleared**. This is the first piece built toward paid SFT dispatch; prior passes
only published the fixture-scale lane receipt.

The instruct-SFT lane receipt commits a deterministic `reportDigest` answer key
for the `psion_instruct_sft_v1` smoke run, but there was no committed bridge
from that answer key to the verification layer a paid `psion_instruct_sft`
dispatch would settle against — the same `deterministic_recompute` challenge
shape the 2026-06-11 alignment run used for its four Verified challenges. This
change adds that bridge, mirroring the committed DPO grading-challenge pattern.

### What was built

- `apps/openagents.com/workers/api/src/psion-instruct-sft-grading-challenge.ts`
  - `buildPsionInstructSftGradingChallengeSpec`: packages the committed lane
    report digest into a public-safe `deterministic_recompute` challenge spec
    (`expectedReportDigest`, `completedSteps`, template/manifest digests, refs).
    Exposes only digests/counts/refs — never prompts, completions, or weights.
  - `verifyPsionInstructSftGradingResponse`: verifies a worker's CLAIMED lane
    report digest against the committed answer key and returns a
    `TrainingVerificationVerdict` (`Verified` only when the well-formed expected
    digest, the claim, and any supplied completed-step count agree; otherwise
    `Rejected` with `DigestMismatch` / `OutputDigestMissing` /
    `DimensionMismatch` / `VerificationClassUnknown`).
  - `buildPsionInstructSftGradingChallengeCreateRequest`: bridges the spec into
    the rail-side `TrainingVerificationChallengeCreateRequest` envelope a paid
    dispatch would POST and **decodes it against the real `training-verification`
    schema**, throwing `PsionInstructSftGradingChallengeError` on a
    structurally invalid request (e.g. a non-public-safe `trainingRunRef`).
- `apps/openagents.com/workers/api/src/psion-instruct-sft-grading-challenge.test.ts`
  - 15 committed tests, including a guard test asserting the answer-key
    constants stay in sync with the published instruct-SFT lane receipt
    (`projectTrainingPostTrainingInstructSft`), so a digest drift fails loudly.

### Honesty boundary (why the blocker stays open)

The SFT lane runs in the Psionic Rust crate, not this worker, so there is **no
in-repo recompute** — the spec's answer key is the committed fixture report
digest, and the verifier compares a claim against that committed answer key. A
real paid dispatch's rail-side `deterministic_recompute` verifier would re-run
the Psionic lane. This module only constructs/validates the request and computes
the verdict math; it submits nothing, takes no lease, spends no sats, settles
nothing, and creates no rail-side challenge. No public route, registry edit, or
green transition is added.

### What genuinely remains for `instruct_sft_paid_dispatch_missing`

Still open: no paid `psion_instruct_sft` dispatch / lease / settlement / on-rail
Verified challenge has run, and the rail-side verifier that re-runs the Psionic
lane (rather than comparing against the committed answer key) is not yet wired.

---

## 2026-06-20 (f) — DPO grading challenge create-request builder

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip, no
registry edit).

### Blocker advanced

`blocker.product_promises.preference_rollout_work_missing` — advanced, **not
cleared**.

The previous pass (e) added the DPO grading challenge VERIFIER but left a gap:
there was no committed bridge from the verifier's `Cs336A5DpoGradingChallengeSpec`
to the actual rail-side `TrainingVerificationChallengeCreateRequest` envelope a
paid `cs336_a5_dpo_grading` dispatch would POST. This change adds that builder so
the create-request is constructed and schema-validated in-repo rather than
hand-assembled at dispatch time.

### What was built

- `apps/openagents.com/workers/api/src/cs336-a5-dpo-grading-challenge.ts`
  - `Cs336A5DpoGradingHomeworkKind` (`admin_dispatched_homework`, the same lane
    the 2026-06-11 alignment run recorded its challenges under).
  - `buildCs336A5DpoGradingChallengeCreateRequest`: turns a DPO grading challenge
    spec + `trainingRunRef` (+ optional `windowRef`) into a
    `deterministic_recompute` / `per_contribution` create-request and **decodes
    it against the real `training-verification` schema**, throwing
    `Cs336A5DpoGradingChallengeError` if the request is structurally invalid
    (e.g. a non-public-safe `trainingRunRef`) or the spec digest is malformed.
    The payload carries only public-safe digests, counts, and refs.
- `apps/openagents.com/workers/api/src/cs336-a5-dpo-grading-challenge.test.ts`
  - 5 new committed tests (16 total in this file): schema-valid request build
    with round-trip decode, `windowRef` omission, public-safety of the payload,
    non-public-safe `trainingRunRef` rejection, and malformed-spec-digest
    rejection.

### Honesty boundary (why the blocker stays open)

This only **constructs and validates** the request object — it does not submit
it, create a challenge, take a lease, spend sats, or settle anything. No rail
mutation, no hosted LLM, no real policy/reference model is involved. It is the
request envelope a paid preference-grading dispatch would send, not the paid
dispatch itself, so the blocker stays open.

### What genuinely remains for `preference_rollout_work_missing`

Still open: no paid `cs336_a5_dpo_grading` dispatch / lease / settlement / on-rail
Verified challenge over preference pairs has run, and real model log-probs are
not yet wired.

---

## 2026-06-20 (e) — DPO preference-grading challenge verifier

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip, no
registry edit).

### Blocker advanced

`blocker.product_promises.preference_rollout_work_missing` — advanced, **not
cleared**.

The DPO reference workload (`cs336-a5-dpo-preference-workload.ts`) emits a
deterministic `outputDigestHex` answer key, but there was no committed bridge
from that answer key to the verification layer a paid `cs336_a5_dpo_grading`
dispatch would settle against — the same `deterministic_recompute` challenge
shape the 2026-06-11 alignment run used for its four Verified challenges. This
change adds that verifier.

### What was built

- `apps/openagents.com/workers/api/src/cs336-a5-dpo-grading-challenge.ts`
  - `buildCs336A5DpoGradingChallengeSpec`: recomputes the reference DPO grade
    for a split and packages a public-safe `deterministic_recompute` challenge
    spec (`expectedDigestHex`, `pairCount`, `betaMicro`, refs, update-boundary).
    Exposes only digests/counts/refs — never prompts, completions, log-probs,
    or weights.
  - `verifyCs336A5DpoGradingResponse`: verifies a worker's CLAIMED grading
    digest by recomputing the reference grade from the committed seed and
    returns a `TrainingVerificationVerdict`. `Verified` only when the stored
    expected digest, the fresh recompute, and the claim all agree (and any
    supplied pair count matches). Otherwise `Rejected` with precise failure
    codes (`DigestMismatch`, `OutputDigestMissing`, `DimensionMismatch`, or
    `VerificationClassUnknown` for a malformed spec). The verifier never trusts
    the spec's stored digest blindly — a stale/forged spec is rejected.
- `apps/openagents.com/workers/api/src/cs336-a5-dpo-grading-challenge.test.ts`
  - 11 committed tests: deterministic spec build matching the reference digest,
    spec re-build stability, non-positive-beta guard, public-safety of the spec,
    Verified happy path, case-insensitive digest, tampered-digest reject,
    malformed-claim reject, pair-count-mismatch reject, stale-spec reject, and
    malformed-expected-digest reject.

### Honesty boundary (why the blocker stays open)

The verdict is a pure function over deterministic digests. No hosted LLM, real
policy/reference model, paid dispatch, lease, settlement, or rail-side Verified
challenge is created — this is the verification math a paid preference-grading
dispatch would record against, not the paid work itself. The DPO/policy-gradient
update stays behind the #4669 training boundary. No public route, registry edit,
or green transition is added here.

### What genuinely remains for `preference_rollout_work_missing`

Still open: no paid `cs336_a5_dpo_grading` dispatch / lease / settlement / on-rail
Verified challenge over preference pairs has run, and real model log-probs are
not yet wired.

---

## 2026-06-20 (d) — overlong-penalty GRPO reward-shaping reference math

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip, no
registry edit).

### Blocker advanced

`blocker.product_promises.preference_rollout_work_missing` — advanced, **not
cleared**.

The CLAIM lists "no … overlong-penalty GRPO reward-shaping receipt" among the
missing pieces. The GRPO rollout/grading workload
(`cs336-a5-rollout-workload.ts`) graded exact-match correctness and
group-normalized advantages, but had no length-aware reward shaping — the DAPO
"soft overlong punishment" that keeps the policy from being rewarded for running
past a response budget. This change adds that reference math with committed
tests: the verifiable reward-shaping function a paid GRPO reward-grading
dispatch would settle against.

### What was built

- `apps/openagents.com/workers/api/src/cs336-a5-overlong-penalty-reward-shaping.ts`
  - Exact DAPO soft-overlong math: `overlongLengthPenalty` (0 inside budget, a
    linear ramp from `0` to `-1` across the soft buffer, `-1` past the hard
    cap), `shapeOverlongReward` (`shapedReward = baseReward + penalty` over an
    exact-match `{0,1}` base reward), and `gradeOverlongShapedBatch` (mean
    base/shaped rewards, mean penalty, within-budget / soft-zone / over-length
    counts).
  - `overlongResponseTokenLength`: a deterministic whitespace-token length
    proxy (real Psionic tokenizer-length conformance stays a Psionic ask).
  - `buildCs336A5OverlongShapedRewards` / `runCs336A5OverlongPenaltyRewardShaping`:
    reuse the SAME seeded synthetic math environment, rollout batch, and
    exact-match `parseCs336A5FinalValue` base reward as the GRPO workload, then
    shape and grade. Re-running a split reproduces its `outputDigestHex`
    bit-for-bit (the `deterministic_recompute` property).
- `apps/openagents.com/workers/api/src/cs336-a5-overlong-penalty-reward-shaping.test.ts`
  - 11 committed tests: zero/budget-boundary penalty, linear soft-buffer ramp,
    hard-cap full penalty, parameter guards, reward shaping in all three zones,
    non-exact-match reward guard, token-length proxy, batch aggregation,
    empty-batch guard, public-safety of shaped rows, and cross-split
    deterministic digest recompute.

Deterministic reference receipt for
`workload.cs336_a5.overlong_penalty_reward_shaping.v1` (split_a, defaults
`maxResponseLength=13`, `cacheLength=2`):

- `rowCount=128`
- `withinBudgetCount=124`, `overLengthCount=4`, `penalizedCount=4`
- `outputDigestHex=35d5a0eb3ea20dc3482c4fb2e0d3e33ad30f0f1f55707a1975292a645fff10e1`

### Honesty boundary (why the blocker stays open)

No hosted LLM is involved: response length is a whitespace-token proxy over the
bounded synthetic completions, and base rewards are the exact-match signal. This
is the exact, unit-tested reward-shaping math plus a deterministic shaping
digest — a prerequisite for paid GRPO reward grading, not the paid work itself.
No paid OpenAgents GRPO reward-shaping dispatch, Verified challenge, settlement,
or policy-gradient update exists; the update step stays behind the #4669
training boundary. No public projection/route, no registry edit, and no green
transition is created here.

### Validation

- `bunx tsc -p tsconfig.json --noEmit` (workers/api): **0 errors**.
- `bun run check:deploy`: see run-log below.

### What genuinely remains for `preference_rollout_work_missing`

Still open: no paid `cs336_a5_overlong_penalty_reward_shaping` /
`cs336_a5_dpo_grading` dispatch, settlement, or Verified challenge over shaped
rewards / preference pairs has run, and real model log-probs / token lengths are
not yet wired.

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
