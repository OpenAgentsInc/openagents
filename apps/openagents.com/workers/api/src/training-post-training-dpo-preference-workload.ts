import { Schema as S } from 'effect'

import {
  Cs336A5DpoDefaultBeta,
  Cs336A5DpoJobKind,
  Cs336A5DpoPreferenceWorkloadRef,
  Cs336A5DpoUpdateBoundaryRef,
} from './cs336-a5-dpo-preference-workload'
import {
  InstructSftPaidDispatchMissingBlocker,
  PreferenceRolloutWorkMissingBlocker,
  VibeTestArtifactMissingBlocker,
} from './training-post-training-instruct-sft'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const TrainingPostTrainingDpoPreferenceWorkloadEndpoint =
  '/api/public/training/post-training-arc/dpo-preference-workload'
export const TrainingPostTrainingDpoPreferenceWorkloadSchemaVersion =
  'openagents.training.post_training_arc.dpo_preference_workload.v1'
export const TrainingPostTrainingDpoPreferenceWorkloadReceiptRef =
  'receipt.training.post_training_arc.dpo_preference_workload.reference_grading.split_a.v1'
export const TrainingPostTrainingDpoPreferenceWorkloadSplitRef = 'split_a'
export const TrainingPostTrainingDpoPreferenceWorkloadOutputDigestHex =
  'ad419c324105c46a889bd5cd13a9e94d66fe9166b6763a0a2add0c77c938ac62'
export const TrainingPostTrainingDpoPreferenceWorkloadStats = {
  chosenRewardMeanMicro: 60313,
  correctlyRankedCount: 25,
  meanLossMicro: 634237,
  pairCount: 25,
  rankingAccuracyBp: 10000,
  rejectedRewardMeanMicro: -61220,
  rewardMarginMeanMicro: 121532,
} as const
export const TrainingPostTrainingDpoPreferenceWorkloadStaleness =
  liveAtReadStaleness([
    'post_training_dpo_preference_workload_receipt_published',
    'product_promise_registry_updated',
  ])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const entryRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs)].sort()

export class TrainingPostTrainingDpoPreferenceWorkloadUnsafe extends Error {
  readonly _tag = 'TrainingPostTrainingDpoPreferenceWorkloadUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingPostTrainingDpoPreferenceWorkloadUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export class TrainingPostTrainingDpoPreferenceWorkloadReceipt extends S.Class<TrainingPostTrainingDpoPreferenceWorkloadReceipt>(
  'TrainingPostTrainingDpoPreferenceWorkloadReceipt',
)({
  authorityBoundary: S.String,
  beta: S.Number,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clearsBlockerRefs: S.Array(S.String),
  deterministicRecomputeAvailable: S.Literal(true),
  dpoUpdateBoundaryRef: S.Literal(Cs336A5DpoUpdateBoundaryRef),
  jobKind: S.Literal(Cs336A5DpoJobKind),
  outputDigestHex: S.String,
  paidDispatchState: S.Literal('not_dispatched'),
  pairCount: S.Int,
  publicSafe: S.Literal(true),
  receiptRef: S.Literal(TrainingPostTrainingDpoPreferenceWorkloadReceiptRef),
  sourceRefs: S.Array(S.String),
  splitRef: S.Literal(TrainingPostTrainingDpoPreferenceWorkloadSplitRef),
  stats: S.Struct({
    chosenRewardMeanMicro: S.Int,
    correctlyRankedCount: S.Int,
    meanLossMicro: S.Int,
    pairCount: S.Int,
    rankingAccuracyBp: S.Int,
    rejectedRewardMeanMicro: S.Int,
    rewardMarginMeanMicro: S.Int,
  }),
  syntheticLogprobBoundary: S.Literal(
    'deterministic_synthetic_public_safe',
  ),
  unsafeCopy: S.String,
  verificationClass: S.Literal('deterministic_recompute'),
  workloadRef: S.Literal(Cs336A5DpoPreferenceWorkloadRef),
}) {}

export class TrainingPostTrainingDpoPreferenceWorkloadProjection extends S.Class<TrainingPostTrainingDpoPreferenceWorkloadProjection>(
  'TrainingPostTrainingDpoPreferenceWorkloadProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TrainingPostTrainingDpoPreferenceWorkloadEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    deterministicReferenceWorkloadAvailable: S.Boolean,
    dpoUpdateAvailable: S.Boolean,
    greenGateSatisfied: S.Boolean,
    paidPreferenceDispatchAvailable: S.Boolean,
    preferenceRolloutWorkAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    realModelLogprobMeasurementAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    remainingProductBlockerRefs: S.Array(S.String),
    settlementReceiptAvailable: S.Boolean,
    verifiedChallengeAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:training.post_training_arc.v1'),
  promiseState: S.Literal('planned'),
  receiptSummary: S.Struct({
    paidPreferenceDispatchCount: S.Int,
    referenceWorkloadReceiptCount: S.Int,
    settlementReceiptCount: S.Int,
    verifiedChallengeCount: S.Int,
  }),
  receipts: S.Array(TrainingPostTrainingDpoPreferenceWorkloadReceipt),
  schemaVersion: S.Literal(
    TrainingPostTrainingDpoPreferenceWorkloadSchemaVersion,
  ),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('dpo_reference_workload_available'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

const buildDpoPreferenceWorkloadReceipt =
  (): TrainingPostTrainingDpoPreferenceWorkloadReceipt => {
    const sourceRefs = [
      'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
      'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.ts',
      'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.test.ts',
      'apps/openagents.com/workers/api/src/cs336-a5-rollout-workload.ts',
      'apps/openagents.com/docs/2026-06-11-cs336-a5-rollout-grading-paid-evidence.md',
      'https://github.com/OpenAgentsInc/openagents/issues/4669',
    ]
    const receipt = new TrainingPostTrainingDpoPreferenceWorkloadReceipt({
      authorityBoundary:
        'This receipt proves only that the bounded CS336 A5 DPO preference-pair reference grading workload is code-backed, deterministic, and public-safe. It grants no paid OpenAgents preference dispatch, real policy/reference-model log-prob measurement, DPO update, settlement, model promotion, vibe-test, service, or green-claim authority.',
      beta: Cs336A5DpoDefaultBeta,
      blockerRefs: [PreferenceRolloutWorkMissingBlocker],
      caveatRefs: [
        'caveat.training_post_training.dpo_logprobs_are_synthetic',
        'caveat.training_post_training.dpo_update_boundary_issue_4669',
        'caveat.training_post_training.no_paid_preference_dispatch',
      ],
      clearsBlockerRefs: [],
      deterministicRecomputeAvailable: true,
      dpoUpdateBoundaryRef: Cs336A5DpoUpdateBoundaryRef,
      jobKind: Cs336A5DpoJobKind,
      outputDigestHex:
        TrainingPostTrainingDpoPreferenceWorkloadOutputDigestHex,
      paidDispatchState: 'not_dispatched',
      pairCount: TrainingPostTrainingDpoPreferenceWorkloadStats.pairCount,
      publicSafe: true,
      receiptRef: TrainingPostTrainingDpoPreferenceWorkloadReceiptRef,
      sourceRefs,
      splitRef: TrainingPostTrainingDpoPreferenceWorkloadSplitRef,
      stats: TrainingPostTrainingDpoPreferenceWorkloadStats,
      syntheticLogprobBoundary: 'deterministic_synthetic_public_safe',
      unsafeCopy:
        'Do not claim paid preference/DPO work ran, real policy or reference model log-probs were measured, a DPO update occurred, a settlement happened, a post-trained model exists, or training.post_training_arc.v1 is green.',
      verificationClass: 'deterministic_recompute',
      workloadRef: Cs336A5DpoPreferenceWorkloadRef,
    })

    assertPublicSafeValue(
      'Training post-training DPO preference workload receipt',
      receipt,
    )

    return receipt
  }

export const projectTrainingPostTrainingDpoPreferenceWorkload = (
  input: { generatedAt?: string | undefined } = {},
): TrainingPostTrainingDpoPreferenceWorkloadProjection => {
  const receipts = [buildDpoPreferenceWorkloadReceipt()]
  const projection = new TrainingPostTrainingDpoPreferenceWorkloadProjection({
    authorityBoundary:
      'Read-only public DPO preference workload projection for training.post_training_arc.v1. It publishes the deterministic reference grading receipt and the exact missing paid-work gates; it grants no assignment, spend, settlement, model promotion, service, vibe-test, or green-claim authority.',
    endpoint: TrainingPostTrainingDpoPreferenceWorkloadEndpoint,
    gate: {
      clearsBlockerRefs: [],
      deterministicReferenceWorkloadAvailable: true,
      dpoUpdateAvailable: false,
      greenGateSatisfied: false,
      paidPreferenceDispatchAvailable: false,
      preferenceRolloutWorkAvailable: false,
      publicProjectionAvailable: true,
      realModelLogprobMeasurementAvailable: false,
      remainingBlockerRefs: [PreferenceRolloutWorkMissingBlocker],
      remainingProductBlockerRefs: [
        InstructSftPaidDispatchMissingBlocker,
        PreferenceRolloutWorkMissingBlocker,
        VibeTestArtifactMissingBlocker,
      ],
      settlementReceiptAvailable: false,
      verifiedChallengeAvailable: false,
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:training.post_training_arc.v1',
    promiseState: 'planned',
    receiptSummary: {
      paidPreferenceDispatchCount: 0,
      referenceWorkloadReceiptCount: receipts.length,
      settlementReceiptCount: 0,
      verifiedChallengeCount: 0,
    },
    receipts,
    schemaVersion: TrainingPostTrainingDpoPreferenceWorkloadSchemaVersion,
    sourceRefs: entryRefs([
      'route:/api/public/training/post-training-arc/dpo-preference-workload',
      'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
      'apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.ts',
      'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.ts',
      'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.test.ts',
      ...receipts.flatMap(receipt => receipt.sourceRefs),
    ]),
    staleness: TrainingPostTrainingDpoPreferenceWorkloadStaleness,
    status: 'dpo_reference_workload_available',
    statusLabel:
      'CS336 A5 DPO preference-pair reference grading is deterministic and public-safe; paid preference dispatch, real model log-probs, verified challenge, settlement, DPO update, and vibe-test artifact remain missing.',
    unsafeCopy:
      'Do not claim the post-training arc is live or green, that paid preference work ran, that real DPO optimization occurred, that a model was promoted, or that a vibe-test closeout artifact exists.',
  })

  assertPublicSafeValue(
    'Training post-training DPO preference workload projection',
    projection,
  )

  return projection
}
