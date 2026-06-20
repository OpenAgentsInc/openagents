import { Schema as S } from 'effect'

import {
  PostTrainingVibeTestCloseoutRef,
  PostTrainingVibeTestDefaultThreshold,
  PostTrainingVibeTestRubricRef,
  runPostTrainingVibeTestCloseout,
} from './post-training-vibe-test-rubric'
import { PublicProductPromisesVersion } from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  InstructSftPaidDispatchMissingBlocker,
  PreferenceRolloutWorkMissingBlocker,
  VibeTestArtifactMissingBlocker,
} from './training-post-training-instruct-sft'

export const TrainingPostTrainingVibeTestRubricEndpoint =
  '/api/public/training/post-training-arc/vibe-test-rubric'
export const TrainingPostTrainingVibeTestRubricSchemaVersion =
  'openagents.training.post_training_arc.vibe_test_rubric.v1'
export const TrainingPostTrainingVibeTestRubricReceiptRef =
  'receipt.training.post_training_arc.vibe_test_rubric.fixture_closeout.v1'
export const TrainingPostTrainingVibeTestRubricStaleness =
  liveAtReadStaleness([
    'post_training_vibe_test_rubric_receipt_published',
    'product_promise_registry_updated',
  ])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const entryRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs)].sort()

const vibeTestSourceRefs = [
  'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
  'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.ts',
  'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.test.ts',
  'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.ts',
  'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.test.ts',
]

export class TrainingPostTrainingVibeTestRubricUnsafe extends Error {
  readonly _tag = 'TrainingPostTrainingVibeTestRubricUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingPostTrainingVibeTestRubricUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export class TrainingPostTrainingVibeTestRubricReceipt extends S.Class<TrainingPostTrainingVibeTestRubricReceipt>(
  'TrainingPostTrainingVibeTestRubricReceipt',
)({
  artifactRef: S.Literal(PostTrainingVibeTestCloseoutRef),
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clearsBlockerRefs: S.Array(S.String),
  closeoutAcceptable: S.Boolean,
  closeoutDigestHex: S.String,
  criteriaRefs: S.Array(S.String),
  fixtureTranscriptBoundary: S.Literal(
    'repo_owned_fixture_not_model_output',
  ),
  publicSafe: S.Literal(true),
  receiptRef: S.Literal(TrainingPostTrainingVibeTestRubricReceiptRef),
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
  unsafeCopy: S.String,
  verificationClass: S.Literal('deterministic_recompute'),
}) {}

export class TrainingPostTrainingVibeTestRubricProjection extends S.Class<TrainingPostTrainingVibeTestRubricProjection>(
  'TrainingPostTrainingVibeTestRubricProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TrainingPostTrainingVibeTestRubricEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    closeoutAcceptable: S.Boolean,
    deterministicCloseoutDigestAvailable: S.Boolean,
    greenGateSatisfied: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    realModelTranscriptArtifactAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    remainingProductBlockerRefs: S.Array(S.String),
    repoOwnedFixtureTranscriptsAvailable: S.Boolean,
    reviewerSignedCloseoutAvailable: S.Boolean,
    rubricAvailable: S.Boolean,
    vibeTestArtifactAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:training.post_training_arc.v1'),
  promiseState: S.Literal('planned'),
  receiptSummary: S.Struct({
    realModelTranscriptArtifactCount: S.Int,
    reviewerSignedCloseoutCount: S.Int,
    rubricReceiptCount: S.Int,
  }),
  receipts: S.Array(TrainingPostTrainingVibeTestRubricReceipt),
  registryVersion: S.String,
  schemaVersion: S.Literal(TrainingPostTrainingVibeTestRubricSchemaVersion),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('vibe_test_rubric_available'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

const buildVibeTestRubricReceipt =
  async (): Promise<TrainingPostTrainingVibeTestRubricReceipt> => {
    const closeout = await runPostTrainingVibeTestCloseout({
      threshold: PostTrainingVibeTestDefaultThreshold,
    })
    const receiptStats = {
      meanScoreMicro: Math.round(closeout.summary.meanScore * 1_000_000),
      passRateBp: Math.round(closeout.summary.passRate * 10_000),
      passedTranscriptCount: closeout.summary.passedTranscriptCount,
      thresholdMicro: Math.round(closeout.summary.threshold * 1_000_000),
      transcriptCount: closeout.summary.transcriptCount,
    }
    const receipt = new TrainingPostTrainingVibeTestRubricReceipt({
      artifactRef: closeout.artifactRef,
      authorityBoundary:
        'This receipt proves only that the owned vibe-test rubric and repo-owned fixture transcript closeout are deterministic and reproducible. It grants no paid dispatch, real model transcript review, reviewer signature, model promotion, service, or green-claim authority.',
      blockerRefs: [VibeTestArtifactMissingBlocker],
      caveatRefs: [
        'caveat.training_post_training.vibe_test_fixture_text_only',
        'caveat.training_post_training.vibe_test_reviewer_signature_missing',
        'caveat.training_post_training.no_real_model_transcript_artifact',
      ],
      clearsBlockerRefs: [],
      closeoutAcceptable: closeout.closeoutAcceptable,
      closeoutDigestHex: closeout.closeoutDigestHex,
      criteriaRefs: [
        'criterion.training_post_training.vibe_test.nonempty_response.v1',
        'criterion.training_post_training.vibe_test.within_length_budget.v1',
        'criterion.training_post_training.vibe_test.instruction_followed.v1',
        'criterion.training_post_training.vibe_test.refusal_when_required.v1',
        'criterion.training_post_training.vibe_test.no_unsafe_leakage.v1',
      ],
      fixtureTranscriptBoundary: 'repo_owned_fixture_not_model_output',
      publicSafe: true,
      receiptRef: TrainingPostTrainingVibeTestRubricReceiptRef,
      reviewerSigned: closeout.reviewerSigned,
      rubricRef: closeout.rubricRef,
      sourceRefs: vibeTestSourceRefs,
      stats: receiptStats,
      unsafeCopy:
        'Do not claim a reviewed vibe-test closeout exists, that real Psion model transcripts passed review, that a reviewer signed the closeout, that a model was promoted, or that training.post_training_arc.v1 is green.',
      verificationClass: 'deterministic_recompute',
    })

    assertPublicSafeValue(
      'Training post-training vibe-test rubric receipt',
      receipt,
    )

    return receipt
  }

export const projectTrainingPostTrainingVibeTestRubric = async (
  input: { generatedAt?: string | undefined } = {},
): Promise<TrainingPostTrainingVibeTestRubricProjection> => {
  const receipts = [await buildVibeTestRubricReceipt()]
  const projection = new TrainingPostTrainingVibeTestRubricProjection({
    authorityBoundary:
      'Read-only public vibe-test rubric projection for training.post_training_arc.v1. It publishes the deterministic rubric receipt and the exact missing review gates; it grants no assignment, spend, settlement, model promotion, service, reviewed artifact, or green-claim authority.',
    endpoint: TrainingPostTrainingVibeTestRubricEndpoint,
    gate: {
      clearsBlockerRefs: [],
      closeoutAcceptable: receipts[0]?.closeoutAcceptable ?? false,
      deterministicCloseoutDigestAvailable:
        /^[0-9a-f]{64}$/.test(receipts[0]?.closeoutDigestHex ?? ''),
      greenGateSatisfied: false,
      publicProjectionAvailable: true,
      realModelTranscriptArtifactAvailable: false,
      remainingBlockerRefs: [VibeTestArtifactMissingBlocker],
      remainingProductBlockerRefs: [
        InstructSftPaidDispatchMissingBlocker,
        PreferenceRolloutWorkMissingBlocker,
        VibeTestArtifactMissingBlocker,
      ],
      repoOwnedFixtureTranscriptsAvailable: true,
      reviewerSignedCloseoutAvailable: false,
      rubricAvailable: true,
      vibeTestArtifactAvailable: false,
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:training.post_training_arc.v1',
    promiseState: 'planned',
    receiptSummary: {
      realModelTranscriptArtifactCount: 0,
      reviewerSignedCloseoutCount: 0,
      rubricReceiptCount: receipts.length,
    },
    receipts,
    registryVersion: PublicProductPromisesVersion,
    schemaVersion: TrainingPostTrainingVibeTestRubricSchemaVersion,
    sourceRefs: entryRefs([
      `route:${TrainingPostTrainingVibeTestRubricEndpoint}`,
      ...vibeTestSourceRefs,
      ...receipts.flatMap(receipt => receipt.sourceRefs),
    ]),
    staleness: TrainingPostTrainingVibeTestRubricStaleness,
    status: 'vibe_test_rubric_available',
    statusLabel:
      'Owned vibe-test rubric and deterministic fixture closeout digest are available; real model transcript artifact, reviewer signature, model promotion, and green gate remain missing.',
    unsafeCopy:
      'Do not claim the post-training arc is live or green, that a reviewed vibe-test artifact exists, that real model transcripts passed review, or that a model was promoted.',
  })

  assertPublicSafeValue(
    'Training post-training vibe-test rubric projection',
    projection,
  )

  return projection
}
