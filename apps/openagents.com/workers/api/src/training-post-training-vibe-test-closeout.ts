import { Schema as S } from 'effect'

import {
  PostTrainingVibeTestCloseoutRef,
  PostTrainingVibeTestDefaultThreshold,
  PostTrainingVibeTestRubricRef,
  buildVibeTestExampleTranscripts,
} from './post-training-vibe-test-rubric'
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

export const TrainingPostTrainingVibeTestCloseoutEndpoint =
  '/api/public/training/post-training-arc/vibe-test-closeout'
export const TrainingPostTrainingVibeTestCloseoutSchemaVersion =
  'openagents.training.post_training_arc.vibe_test_closeout.v1'
export const TrainingPostTrainingVibeTestCloseoutReceiptRef =
  'receipt.training.post_training_arc.vibe_test_closeout.machine_checked.v1'

/**
 * Committed machine-checked closeout digest produced by
 * `runPostTrainingVibeTestCloseout()` over the repo-owned example
 * transcript set at the default threshold. The colocated unit test
 * recomputes the digest live and asserts it equals this constant, so any
 * drift in the rubric or example corpus fails CI rather than silently
 * shipping a stale projection.
 */
export const TrainingPostTrainingVibeTestCloseoutDigestHex =
  '6312b3054b0a94e5a3a45bc3818e3014416f34f6c82582070d088e742370efc8'
export const TrainingPostTrainingVibeTestCloseoutThresholdMicro = Math.round(
  PostTrainingVibeTestDefaultThreshold * 1_000_000,
)
export const TrainingPostTrainingVibeTestCloseoutStats = {
  meanScoreMicro: 1_000_000,
  passRateBp: 10_000,
  passedTranscriptCount: 4,
  thresholdMicro: TrainingPostTrainingVibeTestCloseoutThresholdMicro,
  transcriptCount: 4,
} as const
export const TrainingPostTrainingVibeTestCloseoutStaleness =
  liveAtReadStaleness([
    'post_training_vibe_test_closeout_receipt_published',
    'product_promise_registry_updated',
  ])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const entryRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs)].sort()

export class TrainingPostTrainingVibeTestCloseoutUnsafe extends Error {
  readonly _tag = 'TrainingPostTrainingVibeTestCloseoutUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingPostTrainingVibeTestCloseoutUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export class TrainingPostTrainingVibeTestCloseoutReceipt extends S.Class<TrainingPostTrainingVibeTestCloseoutReceipt>(
  'TrainingPostTrainingVibeTestCloseoutReceipt',
)({
  artifactRef: S.Literal(PostTrainingVibeTestCloseoutRef),
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clearsBlockerRefs: S.Array(S.String),
  closeoutAcceptable: S.Boolean,
  closeoutDigestHex: S.Literal(
    TrainingPostTrainingVibeTestCloseoutDigestHex,
  ),
  machineCheckedAvailable: S.Literal(true),
  publicSafe: S.Literal(true),
  receiptRef: S.Literal(TrainingPostTrainingVibeTestCloseoutReceiptRef),
  reviewerSigned: S.Literal(false),
  rubricRef: S.Literal(PostTrainingVibeTestRubricRef),
  sourceRefs: S.Array(S.String),
  stats: S.Struct({
    meanScoreMicro: S.Int,
    passRateBp: S.Int,
    passedTranscriptCount: S.Int,
    thresholdMicro: S.Int,
    transcriptCount: S.Int,
  }),
  transcriptRefs: S.Array(S.String),
  unsafeCopy: S.String,
  verificationClass: S.Literal('deterministic_recompute'),
}) {}

export class TrainingPostTrainingVibeTestCloseoutProjection extends S.Class<TrainingPostTrainingVibeTestCloseoutProjection>(
  'TrainingPostTrainingVibeTestCloseoutProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TrainingPostTrainingVibeTestCloseoutEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    greenGateSatisfied: S.Boolean,
    machineCheckedCloseoutAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    remainingProductBlockerRefs: S.Array(S.String),
    reviewerSignedCloseoutAvailable: S.Boolean,
    vibeTestArtifactAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:training.post_training_arc.v1'),
  promiseState: S.Literal('planned'),
  receiptSummary: S.Struct({
    machineCheckedCloseoutCount: S.Int,
    reviewerSignedCloseoutCount: S.Int,
  }),
  receipts: S.Array(TrainingPostTrainingVibeTestCloseoutReceipt),
  schemaVersion: S.Literal(
    TrainingPostTrainingVibeTestCloseoutSchemaVersion,
  ),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('vibe_test_machine_checked_closeout_available'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

const buildVibeTestCloseoutReceipt =
  (): TrainingPostTrainingVibeTestCloseoutReceipt => {
    const sourceRefs = [
      'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
      'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.ts',
      'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.test.ts',
      'apps/openagents.com/workers/api/src/training-post-training-vibe-test-closeout.ts',
    ]
    const receipt = new TrainingPostTrainingVibeTestCloseoutReceipt({
      artifactRef: PostTrainingVibeTestCloseoutRef,
      authorityBoundary:
        'This receipt proves only that the bounded post-training vibe-test rubric and its machine-checked closeout over the repo-owned example transcript set are code-backed, deterministic, and public-safe. It grants no reviewer-signed closeout, model promotion, dispatch, spend, settlement, service, or green-claim authority.',
      blockerRefs: [VibeTestArtifactMissingBlocker],
      caveatRefs: [
        'caveat.training_post_training.vibe_test_transcripts_are_fixture',
        'caveat.training_post_training.vibe_test_not_reviewer_signed',
      ],
      clearsBlockerRefs: [],
      closeoutAcceptable: true,
      closeoutDigestHex: TrainingPostTrainingVibeTestCloseoutDigestHex,
      machineCheckedAvailable: true,
      publicSafe: true,
      receiptRef: TrainingPostTrainingVibeTestCloseoutReceiptRef,
      reviewerSigned: false,
      rubricRef: PostTrainingVibeTestRubricRef,
      sourceRefs,
      stats: TrainingPostTrainingVibeTestCloseoutStats,
      transcriptRefs: buildVibeTestExampleTranscripts().map(
        transcript => transcript.transcriptRef,
      ),
      unsafeCopy:
        'Do not claim a reviewer-signed vibe-test artifact exists, that a real Psion instruct model produced these transcripts, that a checkpoint was promoted, or that training.post_training_arc.v1 is green.',
      verificationClass: 'deterministic_recompute',
    })

    assertPublicSafeValue(
      'Training post-training vibe-test closeout receipt',
      receipt,
    )

    return receipt
  }

export const projectTrainingPostTrainingVibeTestCloseout = (
  input: { generatedAt?: string | undefined } = {},
): TrainingPostTrainingVibeTestCloseoutProjection => {
  const receipts = [buildVibeTestCloseoutReceipt()]
  const projection = new TrainingPostTrainingVibeTestCloseoutProjection({
    authorityBoundary:
      'Read-only public vibe-test closeout projection for training.post_training_arc.v1. It publishes the machine-checked half of the closeout (rubric ref, reproducible digest, aggregate stats) and the exact missing reviewer-signed gate; it grants no model promotion, assignment, spend, settlement, service, or green-claim authority.',
    endpoint: TrainingPostTrainingVibeTestCloseoutEndpoint,
    gate: {
      clearsBlockerRefs: [],
      greenGateSatisfied: false,
      machineCheckedCloseoutAvailable: true,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [VibeTestArtifactMissingBlocker],
      remainingProductBlockerRefs: [
        InstructSftPaidDispatchMissingBlocker,
        PreferenceRolloutWorkMissingBlocker,
        VibeTestArtifactMissingBlocker,
      ],
      reviewerSignedCloseoutAvailable: false,
      vibeTestArtifactAvailable: false,
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:training.post_training_arc.v1',
    promiseState: 'planned',
    receiptSummary: {
      machineCheckedCloseoutCount: receipts.length,
      reviewerSignedCloseoutCount: 0,
    },
    receipts,
    schemaVersion: TrainingPostTrainingVibeTestCloseoutSchemaVersion,
    sourceRefs: entryRefs([
      'route:/api/public/training/post-training-arc/vibe-test-closeout',
      'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
      'apps/openagents.com/workers/api/src/training-post-training-vibe-test-closeout.ts',
      'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.ts',
      'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.test.ts',
      ...receipts.flatMap(receipt => receipt.sourceRefs),
    ]),
    staleness: TrainingPostTrainingVibeTestCloseoutStaleness,
    status: 'vibe_test_machine_checked_closeout_available',
    statusLabel:
      'The post-training vibe-test rubric and its machine-checked closeout over the repo-owned example transcripts are deterministic and public-safe; a reviewer-signed closeout artifact remains missing.',
    unsafeCopy:
      'Do not claim the post-training arc is live or green, that a reviewer-signed vibe-test artifact exists, that a real instruct model produced these transcripts, or that a checkpoint was promoted.',
  })

  assertPublicSafeValue(
    'Training post-training vibe-test closeout projection',
    projection,
  )

  return projection
}
