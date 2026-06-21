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
    routePublishesReceipts: S.Literal(false),
    routePublishesStatusOnly: S.Literal(true),
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
  registryVersion: S.Literal(PublicProductPromisesVersion),
  schemaVersion: S.Literal(
    TassadarPerceptaCpuTransformTrainingReceiptsSchemaVersion,
  ),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('cpu_transform_training_receipts_missing'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

const requiredReceiptEvidence =
  (): ReadonlyArray<TassadarPerceptaCpuTransformTrainingRequirement> => [
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: false,
      required: true,
      requiredRefPattern: 'assignment.models.tassadar_percepta_executor.*',
      requirementKind: 'pylon_assignment_receipt',
      statusLabel:
        'No Pylon assignment receipt exists for Tassadar CPU-transform training.',
    }),
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: false,
      required: true,
      requiredRefPattern:
        'receipt.nexus_pylon.tassadar_cpu_transform_closeout.*',
      requirementKind: 'accepted_work_receipt',
      statusLabel:
        'No accepted-work closeout exists for Tassadar CPU-transform training.',
    }),
    new TassadarPerceptaCpuTransformTrainingRequirement({
      available: false,
      required: true,
      requiredRefPattern: 'verdict.tassadar_cpu_transform.exact_replay.*',
      requirementKind: 'verifier_verdict_receipt',
      statusLabel:
        'No verifier verdict receipt exists for Tassadar CPU-transform training.',
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
      available: false,
      required: true,
      requiredRefPattern:
        'artifact.tassadar_percepta_executor.cpu_transform_checkpoint.sha256.*',
      requirementKind: 'trained_artifact_digest',
      statusLabel:
        'No trained artifact digest exists for Tassadar CPU-transform training.',
    }),
  ]

export const projectTassadarPerceptaCpuTransformTrainingReceipts = (
  input: { generatedAt?: string | undefined } = {},
): TassadarPerceptaCpuTransformTrainingReceiptsProjection => {
  const requirements = requiredReceiptEvidence()
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
        'Read-only public status projection for models.tassadar_percepta_executor.v1 CPU-transform training receipts. It names the prerequisite input receipts and the exact missing receipt gates; it grants no assignment, dispatch, spend, settlement, model promotion, inference, trained-model, or green-claim authority.',
      endpoint: TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
      expectedReceiptSurface: {
        emittedReceiptCount: 0,
        expectedReceiptRefPattern:
          TassadarPerceptaCpuTransformTrainingReceiptRefPattern,
        expectedReceiptSchemaVersion:
          TassadarPerceptaCpuTransformTrainingReceiptSchemaVersion,
        requirements,
        routePublishesReceipts: false,
        routePublishesStatusOnly: true,
      },
      gate: {
        acceptedWorkReceiptAvailable: false,
        architectureReceiptAvailable: true,
        clearsBlockerRefs: [],
        cpuTransformTrainingReceiptAvailable: false,
        distillationDatasetReceiptInputAvailable: true,
        greenGateSatisfied: false,
        pylonAssignmentReceiptAvailable: false,
        publicProjectionAvailable: true,
        realSettlementReceiptAvailable: false,
        remainingBlockerRefs: [
          TassadarPerceptaCpuTransformTrainingReceiptBlocker,
        ],
        trainedModelArtifactAvailable: false,
        verifierVerdictReceiptAvailable: false,
      },
      generatedAt: input.generatedAt ?? currentIsoTimestamp(),
      inputRefs,
      promiseRef: 'promise:models.tassadar_percepta_executor.v1',
      promiseState: 'planned',
      receiptSummary: {
        architectureReceiptCount: 1,
        distillationDatasetReceiptCount: 1,
        emittedCpuTransformTrainingReceiptCount: 0,
        requiredAcceptedTraceCount:
          ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
      },
      registryVersion: PublicProductPromisesVersion,
      schemaVersion: TassadarPerceptaCpuTransformTrainingReceiptsSchemaVersion,
      sourceRefs: entryRefs([
        `route:${TassadarPerceptaCpuTransformTrainingReceiptsEndpoint}`,
        ...cpuTransformSourceRefs,
        ...inputRefs.flatMap(inputRef => inputRef.sourceRefs),
      ]),
      staleness: TassadarPerceptaCpuTransformTrainingReceiptsStaleness,
      status: 'cpu_transform_training_receipts_missing',
      statusLabel:
        'Architecture and Artanis distillation dataset inputs are visible; no Pylon CPU-transform training receipt has been emitted.',
      unsafeCopy:
        'Do not claim a trained Tassadar Percepta model, CPU-transform training completion, public contributor training, accepted work, verifier acceptance, real settlement, model promotion, inference, or a green product promise from this status projection.',
    },
  )

  assertPublicSafeValue(
    'Tassadar Percepta CPU-transform training receipts projection',
    projection,
  )

  return projection
}
