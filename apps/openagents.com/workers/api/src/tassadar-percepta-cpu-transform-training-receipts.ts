import { Schema as S } from 'effect'

import {
  ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
  ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
} from './artanis-distillation-dataset-receipt'
import { PublicProductPromisesVersion } from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  TassadarPerceptaArchitectureReceiptRef,
  TassadarPerceptaArchitectureReceiptsEndpoint,
  TassadarPerceptaCpuTransformTrainingReceiptBlocker,
} from './tassadar-percepta-architecture-receipts'

export const TassadarPerceptaCpuTransformTrainingReceiptsEndpoint =
  '/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts'
export const TassadarPerceptaCpuTransformTrainingReceiptsSchemaVersion =
  'openagents.models.tassadar_percepta_executor.cpu_transform_training_receipts.v1'
export const TassadarPerceptaCpuTransformTrainingReceiptSchemaVersion =
  'openagents.models.tassadar_percepta_executor.cpu_transform_training_receipt.v1'
export const TassadarPerceptaCpuTransformTrainingReceiptRefPattern =
  'receipt.models.tassadar_percepta_executor.cpu_transform_training.{assignmentRef}'
export const TassadarPerceptaCpuTransformTrainingFixtureAssignmentRef =
  'assignment.models.tassadar_percepta_executor.cpu_transform_fixture.v1'
export const TassadarPerceptaCpuTransformTrainingFixtureReceiptRef =
  'receipt.models.tassadar_percepta_executor.cpu_transform_training.cpu_transform_fixture_v1'
export const TassadarPerceptaCpuTransformTrainingFixtureVerifierVerdictRef =
  'verdict.tassadar_cpu_transform.exact_replay.cpu_transform_fixture_v1'
export const TassadarPerceptaCpuTransformTrainingFixtureArtifactRef =
  'artifact.tassadar_percepta_executor.cpu_transform_checkpoint.sha256.8feaf5488599a4b618b8d2188ed8ea0b68ec9fb5f58a55db3064e52ae9ff73d9'
export const TassadarPerceptaCpuTransformRealSettlementBlocker =
  'blocker.product_promises.tassadar_cpu_transform_real_settlement_missing'
export const TassadarPerceptaCpuTransformOwnerGreenSignoffBlocker =
  'blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing'
export const TassadarPerceptaCpuTransformTrainingReceiptsStaleness =
  liveAtReadStaleness([
    'tassadar_percepta_cpu_transform_training_receipt_published',
    'artanis_tassadar_distillation_dataset_receipt_published',
    'tassadar_percepta_architecture_receipt_published',
    'product_promise_registry_updated',
  ])

export const ArtanisTassadarDistillationDatasetEndpoint =
  '/api/public/artanis/tassadar-distillation-dataset'

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const entryRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].sort()

const cpuTransformSourceRefs = [
  'docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md',
  'docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md',
  'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
  'docs/training/2026-06-20-artanis-distillation-dataset-receipt.md',
  'apps/pylon/src/tassadar-cpu-transform-training.ts',
  'apps/pylon/tests/tassadar-cpu-transform-training.test.ts',
  'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.ts',
  'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.test.ts',
]

export class TassadarPerceptaCpuTransformTrainingReceiptsUnsafe extends Error {
  readonly _tag = 'TassadarPerceptaCpuTransformTrainingReceiptsUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TassadarPerceptaCpuTransformTrainingReceiptsUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export class TassadarPerceptaCpuTransformTrainingInputRef extends S.Class<TassadarPerceptaCpuTransformTrainingInputRef>(
  'TassadarPerceptaCpuTransformTrainingInputRef',
)({
  available: S.Boolean,
  endpoint: S.String,
  inputKind: S.Literals([
    'architecture_receipt',
    'distillation_dataset_receipt',
  ]),
  receiptRef: S.String,
  sourceRefs: S.Array(S.String),
}) {}

export class TassadarPerceptaCpuTransformTrainingRequirement extends S.Class<TassadarPerceptaCpuTransformTrainingRequirement>(
  'TassadarPerceptaCpuTransformTrainingRequirement',
)({
  available: S.Boolean,
  requirementKind: S.Literals([
    'pylon_assignment_receipt',
    'accepted_work_receipt',
    'verifier_verdict_receipt',
    'real_settlement_receipt',
    'trained_artifact_digest',
  ]),
  required: S.Literal(true),
  requiredRefPattern: S.String,
  statusLabel: S.String,
}) {}

export class TassadarPerceptaCpuTransformTrainingReceipt extends S.Class<TassadarPerceptaCpuTransformTrainingReceipt>(
  'TassadarPerceptaCpuTransformTrainingReceipt',
)({
  acceptedWorkReceiptRef: S.String,
  artifactDigest: S.String,
  artifactRef: S.Literal(TassadarPerceptaCpuTransformTrainingFixtureArtifactRef),
  assignmentRef: S.Literal(
    TassadarPerceptaCpuTransformTrainingFixtureAssignmentRef,
  ),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clearsBlockerRefs: S.Array(S.String),
  completedSteps: S.Int,
  cpuOnly: S.Literal(true),
  fixtureScale: S.Literal(true),
  inputVectorCount: S.Int,
  learningRateBps: S.Int,
  lossAfterMicros: S.Int,
  lossBeforeMicros: S.Int,
  lossImproved: S.Boolean,
  parameterCount: S.Int,
  publicSafe: S.Literal(true),
  realBitcoinMoved: S.Literal(false),
  receiptRef: S.Literal(
    TassadarPerceptaCpuTransformTrainingFixtureReceiptRef,
  ),
  runRef: S.Literal(
    'run.tassadar_percepta_executor.cpu_transform_fixture.v1',
  ),
  schemaVersion: S.Literal(
    TassadarPerceptaCpuTransformTrainingReceiptSchemaVersion,
  ),
  settlementState: S.Literal('not_settled'),
  sourceRefs: S.Array(S.String),
  trainingKind: S.Literal('bounded_cpu_computation_transform'),
  unsafeCopy: S.String,
  verifierVerdict: S.Literal('accepted'),
  verifierVerdictRef: S.Literal(
    TassadarPerceptaCpuTransformTrainingFixtureVerifierVerdictRef,
  ),
}) {}

export class TassadarPerceptaCpuTransformTrainingReceiptsProjection extends S.Class<TassadarPerceptaCpuTransformTrainingReceiptsProjection>(
  'TassadarPerceptaCpuTransformTrainingReceiptsProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TassadarPerceptaCpuTransformTrainingReceiptsEndpoint),
  expectedReceiptSurface: S.Struct({
    emittedReceiptCount: S.Int,
    expectedReceiptRefPattern: S.Literal(
      TassadarPerceptaCpuTransformTrainingReceiptRefPattern,
    ),
    expectedReceiptSchemaVersion: S.Literal(
      TassadarPerceptaCpuTransformTrainingReceiptSchemaVersion,
    ),
    requirements: S.Array(TassadarPerceptaCpuTransformTrainingRequirement),
    routePublishesReceipts: S.Literal(true),
    routePublishesStatusOnly: S.Literal(false),
  }),
  gate: S.Struct({
    acceptedWorkReceiptAvailable: S.Boolean,
    architectureReceiptAvailable: S.Boolean,
    clearsBlockerRefs: S.Array(S.String),
    cpuTransformTrainingReceiptAvailable: S.Boolean,
    distillationDatasetReceiptInputAvailable: S.Boolean,
    greenGateSatisfied: S.Boolean,
    pylonAssignmentReceiptAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    realSettlementReceiptAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    trainedModelArtifactAvailable: S.Boolean,
    verifierVerdictReceiptAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  inputRefs: S.Array(TassadarPerceptaCpuTransformTrainingInputRef),
  promiseRef: S.Literal('promise:models.tassadar_percepta_executor.v1'),
  promiseState: S.Literal('planned'),
  receiptSummary: S.Struct({
    architectureReceiptCount: S.Int,
    distillationDatasetReceiptCount: S.Int,
    emittedCpuTransformTrainingReceiptCount: S.Int,
    requiredAcceptedTraceCount: S.Int,
  }),
  receipts: S.Array(TassadarPerceptaCpuTransformTrainingReceipt),
  registryVersion: S.Literal(PublicProductPromisesVersion),
  schemaVersion: S.Literal(
    TassadarPerceptaCpuTransformTrainingReceiptsSchemaVersion,
  ),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('cpu_transform_training_receipt_available'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

const requiredReceiptEvidence =
  (): ReadonlyArray<TassadarPerceptaCpuTransformTrainingRequirement> => [
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: true,
      required: true,
      requiredRefPattern: 'assignment.models.tassadar_percepta_executor.*',
      requirementKind: 'pylon_assignment_receipt',
      statusLabel:
        'A bounded Pylon fixture assignment receipt exists for Tassadar CPU-transform training.',
    }),
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: true,
      required: true,
      requiredRefPattern:
        'receipt.nexus_pylon.tassadar_cpu_transform_closeout.*',
      requirementKind: 'accepted_work_receipt',
      statusLabel:
        'A bounded accepted-work closeout receipt exists for the fixture-scale CPU-transform step.',
    }),
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: true,
      required: true,
      requiredRefPattern: 'verdict.tassadar_cpu_transform.exact_replay.*',
      requirementKind: 'verifier_verdict_receipt',
      statusLabel:
        'A verifier verdict receipt accepts the deterministic fixture replay.',
    }),
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: false,
      required: true,
      requiredRefPattern:
        'receipt.nexus_pylon.settlement.assignment_tassadar_cpu_transform.*',
      requirementKind: 'real_settlement_receipt',
      statusLabel:
        'No real settlement receipt exists for Tassadar CPU-transform training.',
    }),
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: true,
      required: true,
      requiredRefPattern:
        'artifact.tassadar_percepta_executor.cpu_transform_checkpoint.sha256.*',
      requirementKind: 'trained_artifact_digest',
      statusLabel:
        'A fixture-scale CPU-transform checkpoint digest exists; it is not a promoted model artifact.',
    }),
  ]

const buildCpuTransformTrainingReceipt =
  (): TassadarPerceptaCpuTransformTrainingReceipt =>
    new TassadarPerceptaCpuTransformTrainingReceipt({
      acceptedWorkReceiptRef:
        'receipt.nexus_pylon.tassadar_cpu_transform_closeout.cpu_transform_fixture_v1',
      artifactDigest:
        'sha256:8feaf5488599a4b618b8d2188ed8ea0b68ec9fb5f58a55db3064e52ae9ff73d9',
      artifactRef: TassadarPerceptaCpuTransformTrainingFixtureArtifactRef,
      assignmentRef: TassadarPerceptaCpuTransformTrainingFixtureAssignmentRef,
      blockerRefs: [
        TassadarPerceptaCpuTransformRealSettlementBlocker,
        TassadarPerceptaCpuTransformOwnerGreenSignoffBlocker,
      ],
      caveatRefs: [
        'caveat.tassadar_cpu_transform.fixture_scale_only',
        'caveat.tassadar_cpu_transform.no_trained_model_claim',
        'caveat.tassadar_cpu_transform.no_settlement_or_earning_claim',
      ],
      clearsBlockerRefs: [TassadarPerceptaCpuTransformTrainingReceiptBlocker],
      completedSteps: 1,
      cpuOnly: true,
      fixtureScale: true,
      inputVectorCount: 3,
      learningRateBps: 2500,
      lossAfterMicros: 546296,
      lossBeforeMicros: 1666666,
      lossImproved: true,
      parameterCount: 2,
      publicSafe: true,
      realBitcoinMoved: false,
      receiptRef: TassadarPerceptaCpuTransformTrainingFixtureReceiptRef,
      runRef: 'run.tassadar_percepta_executor.cpu_transform_fixture.v1',
      schemaVersion: TassadarPerceptaCpuTransformTrainingReceiptSchemaVersion,
      settlementState: 'not_settled',
      sourceRefs: [
        'apps/pylon/src/tassadar-cpu-transform-training.ts',
        'apps/pylon/tests/tassadar-cpu-transform-training.test.ts',
        'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
      ],
      trainingKind: 'bounded_cpu_computation_transform',
      unsafeCopy:
        'Do not claim this fixture-scale CPU transform step is a trained Tassadar model, a public earning path, a settled assignment, a promoted checkpoint, or a green product promise.',
      verifierVerdict: 'accepted',
      verifierVerdictRef:
        TassadarPerceptaCpuTransformTrainingFixtureVerifierVerdictRef,
    })

export const projectTassadarPerceptaCpuTransformTrainingReceipts = (
  input: { generatedAt?: string | undefined } = {},
): TassadarPerceptaCpuTransformTrainingReceiptsProjection => {
  const requirements = requiredReceiptEvidence()
  const receipts = [buildCpuTransformTrainingReceipt()]
  const inputRefs = [
    new TassadarPerceptaCpuTransformTrainingInputRef({
      available: true,
      endpoint: TassadarPerceptaArchitectureReceiptsEndpoint,
      inputKind: 'architecture_receipt',
      receiptRef: TassadarPerceptaArchitectureReceiptRef,
      sourceRefs: [
        `route:${TassadarPerceptaArchitectureReceiptsEndpoint}`,
        'docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md',
      ],
    }),
    new TassadarPerceptaCpuTransformTrainingInputRef({
      available: true,
      endpoint: ArtanisTassadarDistillationDatasetEndpoint,
      inputKind: 'distillation_dataset_receipt',
      receiptRef: ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
      sourceRefs: [
        `route:${ArtanisTassadarDistillationDatasetEndpoint}`,
        'docs/training/2026-06-20-artanis-distillation-dataset-receipt.md',
      ],
    }),
  ]
  const projection = new TassadarPerceptaCpuTransformTrainingReceiptsProjection(
    {
      authorityBoundary:
        'Read-only public receipt projection for models.tassadar_percepta_executor.v1 CPU-transform training. It names prerequisite input receipts and one bounded fixture receipt; it grants no dispatch, spend, settlement, model promotion, inference, trained-model, or green-claim authority.',
      endpoint: TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
      expectedReceiptSurface: {
        emittedReceiptCount: receipts.length,
        expectedReceiptRefPattern:
          TassadarPerceptaCpuTransformTrainingReceiptRefPattern,
        expectedReceiptSchemaVersion:
          TassadarPerceptaCpuTransformTrainingReceiptSchemaVersion,
        requirements,
        routePublishesReceipts: true,
        routePublishesStatusOnly: false,
      },
      gate: {
        acceptedWorkReceiptAvailable: true,
        architectureReceiptAvailable: true,
        clearsBlockerRefs: [TassadarPerceptaCpuTransformTrainingReceiptBlocker],
        cpuTransformTrainingReceiptAvailable: true,
        distillationDatasetReceiptInputAvailable: true,
        greenGateSatisfied: false,
        pylonAssignmentReceiptAvailable: true,
        publicProjectionAvailable: true,
        realSettlementReceiptAvailable: false,
        remainingBlockerRefs: [
          TassadarPerceptaCpuTransformRealSettlementBlocker,
          TassadarPerceptaCpuTransformOwnerGreenSignoffBlocker,
        ],
        trainedModelArtifactAvailable: true,
        verifierVerdictReceiptAvailable: true,
      },
      generatedAt: input.generatedAt ?? currentIsoTimestamp(),
      inputRefs,
      promiseRef: 'promise:models.tassadar_percepta_executor.v1',
      promiseState: 'planned',
      receiptSummary: {
        architectureReceiptCount: 1,
        distillationDatasetReceiptCount: 1,
        emittedCpuTransformTrainingReceiptCount: receipts.length,
        requiredAcceptedTraceCount:
          ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
      },
      receipts,
      registryVersion: PublicProductPromisesVersion,
      schemaVersion: TassadarPerceptaCpuTransformTrainingReceiptsSchemaVersion,
      sourceRefs: entryRefs([
        `route:${TassadarPerceptaCpuTransformTrainingReceiptsEndpoint}`,
        ...cpuTransformSourceRefs,
        ...inputRefs.flatMap(inputRef => inputRef.sourceRefs),
      ]),
      staleness: TassadarPerceptaCpuTransformTrainingReceiptsStaleness,
      status: 'cpu_transform_training_receipt_available',
      statusLabel:
        'Architecture and Artanis distillation dataset inputs are visible, and one bounded Pylon CPU-transform training fixture receipt has been emitted; real settlement and owner green sign-off remain missing.',
      unsafeCopy:
        'Do not claim a trained Tassadar Percepta model, broad CPU-transform training completion, public contributor training at scale, real settlement, model promotion, inference, or a green product promise from this fixture-scale receipt projection.',
    },
  )

  assertPublicSafeValue(
    'Tassadar Percepta CPU-transform training receipts projection',
    projection,
  )

  return projection
}
