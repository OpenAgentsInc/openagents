import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const TrainingPostTrainingInstructSftEndpoint =
  '/api/public/training/post-training-arc/instruct-sft-lane'
export const TrainingPostTrainingInstructSftSchemaVersion =
  'openagents.training.post_training_arc.instruct_sft_lane.v1'
export const TrainingPostTrainingInstructSftReceiptRef =
  'receipt.training.post_training_arc.instruct_sft_lane.psion_fixture.v1'
export const TrainingPostTrainingInstructSftStaleness =
  liveAtReadStaleness([
    'post_training_instruct_sft_lane_receipt_published',
    'product_promise_registry_updated',
  ])

export const InstructSftLaneMissingBlocker =
  'blocker.product_promises.instruct_sft_lane_missing'
export const InstructSftPaidDispatchMissingBlocker =
  'blocker.product_promises.instruct_sft_paid_dispatch_missing'
export const PreferenceRolloutWorkMissingBlocker =
  'blocker.product_promises.preference_rollout_work_missing'
export const VibeTestArtifactMissingBlocker =
  'blocker.product_promises.vibe_test_artifact_missing'

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const entryRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs)].sort()

export class TrainingPostTrainingInstructSftUnsafe extends Error {
  readonly _tag = 'TrainingPostTrainingInstructSftUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingPostTrainingInstructSftUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

const githubPsionicRef = (path: string): string =>
  `https://github.com/OpenAgentsInc/psionic/blob/main/${path}`

export class TrainingPostTrainingInstructSftReceipt extends S.Class<TrainingPostTrainingInstructSftReceipt>(
  'TrainingPostTrainingInstructSftReceipt',
)({
  adapterArtifactDigest: S.String,
  adapterIdentityDigest: S.String,
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clearsBlockerRefs: S.Array(S.String),
  completedSteps: S.Int,
  committedReportFixtureSyncAvailable: S.Boolean,
  corpus: S.Struct({
    datasetIdentity: S.String,
    exampleCorpus: S.Boolean,
    manifestDigest: S.String,
    recordCount: S.Int,
    rightsPosture: S.Literal('repo_owned_example_text'),
    totalMaskedTokens: S.Int,
    totalTrainableTokens: S.Int,
  }),
  evidenceRefs: S.Array(S.String),
  laneId: S.Literal('psion_instruct_sft_v1'),
  learningRateRatioBps: S.Int,
  lossImproved: S.Boolean,
  paidDispatchState: S.Literal('not_dispatched'),
  publicSafe: S.Literal(true),
  receiptRef: S.Literal(TrainingPostTrainingInstructSftReceiptRef),
  reportDigest: S.String,
  resumeDrill: S.Struct({
    checkpointAtStep: S.Int,
    postResumeReceiptDigestsMatch: S.Boolean,
    resumeBitExact: S.Boolean,
    resumedSteps: S.Int,
  }),
  runId: S.Literal('psion-instruct-sft-smoke-001'),
  schedulerKind: S.Literal('cosine_annealing'),
  sourceRefs: S.Array(S.String),
  template: S.Struct({
    templateDigest: S.String,
    templateId: S.Literal('psion_chat_template'),
    templateVersion: S.Literal('v1'),
  }),
  unsafeCopy: S.String,
}) {}

export class TrainingPostTrainingInstructSftProjection extends S.Class<TrainingPostTrainingInstructSftProjection>(
  'TrainingPostTrainingInstructSftProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TrainingPostTrainingInstructSftEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    greenGateSatisfied: S.Boolean,
    committedReportFixtureSyncAvailable: S.Boolean,
    instructSftLaneAvailable: S.Boolean,
    instructSftPaidDispatchAvailable: S.Boolean,
    preferenceRolloutWorkAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    vibeTestArtifactAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:training.post_training_arc.v1'),
  promiseState: S.Literal('planned'),
  receiptSummary: S.Struct({
    instructSftReceiptCount: S.Int,
    paidDispatchCount: S.Int,
    preferenceRolloutReceiptCount: S.Int,
    vibeTestArtifactReceiptCount: S.Int,
  }),
  receipts: S.Array(TrainingPostTrainingInstructSftReceipt),
  schemaVersion: S.Literal(TrainingPostTrainingInstructSftSchemaVersion),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('instruct_sft_lane_receipt_available'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

const buildInstructSftReceipt =
  (): TrainingPostTrainingInstructSftReceipt => {
    const sourceRefs = [
      githubPsionicRef('fixtures/psion/instruct/psion_chat_template_v1.json'),
      githubPsionicRef(
        'fixtures/psion/instruct/psion_instruct_corpus_manifest_v1.json',
      ),
      githubPsionicRef(
        'fixtures/psion/instruct/psion_instruct_generation_mask_fixture_v1.json',
      ),
      githubPsionicRef(
        'fixtures/psion/instruct/psion_instruct_sft_lane_report_v1.json',
      ),
      githubPsionicRef('scripts/check-psion-instruct-sft-lane.sh'),
      githubPsionicRef(
        'crates/psionic-train/src/psion_instruct_sft_lane.rs',
      ),
      githubPsionicRef(
        'crates/psionic-train/examples/psion_instruct_sft_lane_fixtures.rs',
      ),
      'https://github.com/OpenAgentsInc/psionic/pull/1132',
    ]
    const receipt = new TrainingPostTrainingInstructSftReceipt({
      adapterArtifactDigest:
        'sha256:f1c3386d40e0c7caa52cf908f2358b9158983608251afc5b5a0c7c389aec5354',
      adapterIdentityDigest:
        'sha256:5b7665196d4ae78251d05b0cf4b77de0cf770c089b9787febd059d4ad516808c',
      authorityBoundary:
        'This receipt proves only that Psionic has a bounded fixture-scale instruct SFT lane with an owned chat template, repo-owned example corpus, assistant-token loss mask, deterministic smoke run, bit-exact resume drill, and committed report fixture synchronized with the deterministic generator. It grants no paid dispatch, settlement, preference optimization, vibe-test, model-quality, service-availability, or green-claim authority.',
      blockerRefs: [InstructSftPaidDispatchMissingBlocker],
      caveatRefs: [
        'caveat.training_post_training.instruct_sft_fixture_scale_only',
        'caveat.training_post_training.instruct_sft_not_paid_openagents_dispatch',
        'caveat.training_post_training.no_general_instruct_capability_claim',
      ],
      clearsBlockerRefs: [InstructSftLaneMissingBlocker],
      completedSteps: 8,
      committedReportFixtureSyncAvailable: true,
      corpus: {
        datasetIdentity: 'psion_instruct_corpus_example@v1',
        exampleCorpus: true,
        manifestDigest:
          'sha256:1ce60a17a18975a729fd7d9d81baab556541af6fd280c0fadfb29e09b7e18cc7',
        recordCount: 4,
        rightsPosture: 'repo_owned_example_text',
        totalMaskedTokens: 65,
        totalTrainableTokens: 93,
      },
      evidenceRefs: [
        TrainingPostTrainingInstructSftReceiptRef,
        'lane.psion_instruct_sft_v1',
        'template.psion_chat_template.v1',
        'dataset.psion_instruct_corpus_example.v1',
        'report.psion_instruct_sft_lane_report.v1',
        'script:psionic/scripts/check-psion-instruct-sft-lane.sh',
        'https://github.com/OpenAgentsInc/psionic/pull/1132',
      ],
      laneId: 'psion_instruct_sft_v1',
      learningRateRatioBps: 1000,
      lossImproved: true,
      paidDispatchState: 'not_dispatched',
      publicSafe: true,
      receiptRef: TrainingPostTrainingInstructSftReceiptRef,
      reportDigest:
        'sha256:76b5524234b4dd6507560c0cda6f28e782fe097c1fb022108aaaae40794d6871',
      resumeDrill: {
        checkpointAtStep: 3,
        postResumeReceiptDigestsMatch: true,
        resumeBitExact: true,
        resumedSteps: 5,
      },
      runId: 'psion-instruct-sft-smoke-001',
      schedulerKind: 'cosine_annealing',
      sourceRefs,
      template: {
        templateDigest:
          'sha256:7337ec749e64dbf1b23dbfeb3478788846c67e8247813f386d97b1ed1076fca3',
        templateId: 'psion_chat_template',
        templateVersion: 'v1',
      },
      unsafeCopy:
        'Do not claim a paid OpenAgents SFT assignment ran, that a Psion instruct model exists, that preference/DPO or vibe-test gates are satisfied, or that this fixture-scale lane makes training.post_training_arc.v1 green.',
    })

    assertPublicSafeValue('Training post-training instruct SFT receipt', receipt)

    return receipt
  }

export const projectTrainingPostTrainingInstructSft = (
  input: { generatedAt?: string | undefined } = {},
): TrainingPostTrainingInstructSftProjection => {
  const receipts = [buildInstructSftReceipt()]
  const projection = new TrainingPostTrainingInstructSftProjection({
    authorityBoundary:
      'Read-only public instruct-SFT lane receipt projection for training.post_training_arc.v1. It narrows the missing-lane blocker to paid-dispatch and later-stage blockers; it grants no dispatch, spend, settlement, model-promotion, service, preference-optimization, vibe-test, or green-claim authority.',
    endpoint: TrainingPostTrainingInstructSftEndpoint,
    gate: {
      clearsBlockerRefs: [InstructSftLaneMissingBlocker],
      greenGateSatisfied: false,
      committedReportFixtureSyncAvailable: true,
      instructSftLaneAvailable: true,
      instructSftPaidDispatchAvailable: false,
      preferenceRolloutWorkAvailable: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [
        InstructSftPaidDispatchMissingBlocker,
        PreferenceRolloutWorkMissingBlocker,
        VibeTestArtifactMissingBlocker,
      ],
      vibeTestArtifactAvailable: false,
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:training.post_training_arc.v1',
    promiseState: 'planned',
    receiptSummary: {
      instructSftReceiptCount: receipts.length,
      paidDispatchCount: receipts.filter(
        receipt => receipt.paidDispatchState !== 'not_dispatched',
      ).length,
      preferenceRolloutReceiptCount: 0,
      vibeTestArtifactReceiptCount: 0,
    },
    receipts,
    schemaVersion: TrainingPostTrainingInstructSftSchemaVersion,
    sourceRefs: entryRefs([
      'docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md',
      'docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md',
      'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
      'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
      'apps/openagents.com/workers/api/src/training-post-training-instruct-sft.ts',
      ...receipts.flatMap(receipt => receipt.sourceRefs),
    ]),
    staleness: TrainingPostTrainingInstructSftStaleness,
    status: 'instruct_sft_lane_receipt_available',
    statusLabel:
      'Psionic instruct SFT lane receipt is available and the committed report fixture is synchronized with deterministic generator output; paid OpenAgents SFT dispatch, preference rollout work, and vibe-test artifact remain missing.',
    unsafeCopy:
      'Do not claim the post-training arc is live or green, that an instruct Psion model exists, that paid SFT or preference work ran, or that a vibe-test closeout artifact exists.',
  })

  assertPublicSafeValue(
    'Training post-training instruct SFT projection',
    projection,
  )

  return projection
}
