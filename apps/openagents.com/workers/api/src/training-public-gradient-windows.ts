import { Schema as S } from 'effect'

import { PublicProductPromisesVersion } from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import { TassadarGradientWindowIntakeSchemaVersion } from './tassadar-gradient-window-intake'
import { TassadarGradientWindowPromotionLineageSchemaVersion } from './tassadar-gradient-window-promotion-lineage'
import { TassadarGradientWindowPromotionReceiptFeedSchemaVersion } from './tassadar-gradient-window-promotion-receipt-feed'
import { TassadarGradientWindowPromotionReceiptSchemaVersion } from './tassadar-gradient-window-promotion-receipt'
import { TassadarGradientWindowPromotionReceiptVerificationSchemaVersion } from './tassadar-gradient-window-promotion-receipt-verify'
import { TassadarGradientWindowQuarantineRecordSchemaVersion } from './tassadar-gradient-window-quarantine-record'
import { TassadarGradientWindowQuarantineRecordVerificationSchemaVersion } from './tassadar-gradient-window-quarantine-record-verify'

export const TrainingPublicGradientWindowsEndpoint =
  '/api/public/training/public-gradient-windows'
export const TrainingPublicGradientWindowsSchemaVersion =
  'openagents.training.public_gradient_windows.v1'
export const TrainingPublicGradientLiveWindowRuntimeBlocker =
  'blocker.product_promises.public_gradient_live_window_runtime_missing'
export const TrainingPublicGradientPromotedWindowReceiptBlocker =
  'blocker.product_promises.public_gradient_promoted_window_receipts_missing'
export const TrainingPublicGradientSettlementReceiptBlocker =
  'blocker.product_promises.public_gradient_settlement_receipts_missing'

export const TrainingPublicGradientWindowsStaleness = liveAtReadStaleness([
  'training_public_gradient_window_intake_admission_changed',
  'training_public_gradient_window_runtime_started',
  'training_public_gradient_window_receipt_published',
  'training_public_gradient_window_settlement_published',
  'product_promise_registry_updated',
])

const remainingBlockerRefs = [
  TrainingPublicGradientLiveWindowRuntimeBlocker,
  TrainingPublicGradientPromotedWindowReceiptBlocker,
  TrainingPublicGradientSettlementReceiptBlocker,
]

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

export class TrainingPublicGradientWindowsProjection extends S.Class<TrainingPublicGradientWindowsProjection>(
  'TrainingPublicGradientWindowsProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TrainingPublicGradientWindowsEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    greenGateSatisfied: S.Boolean,
    intakeAdmissionPredicateAvailable: S.Boolean,
    liveWindowRuntimeAvailable: S.Boolean,
    promotedWindowReceiptAvailable: S.Boolean,
    promotionReceiptEmitterAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    regimeGateAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    settlementReceiptAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:training.public_gradient_windows.v1'),
  promiseState: S.Literal('planned'),
  intakeSurface: S.Struct({
    acceptedSubmissionCount: S.Int,
    admittedQuarantineRecordCount: S.Int,
    predicateAvailable: S.Boolean,
    quarantineRecordFormatAvailable: S.Boolean,
    quarantineRecordSchemaVersion: S.Literal(
      TassadarGradientWindowQuarantineRecordSchemaVersion,
    ),
    quarantineRecordVerifierAvailable: S.Boolean,
    quarantineRecordVerifierSchemaVersion: S.Literal(
      TassadarGradientWindowQuarantineRecordVerificationSchemaVersion,
    ),
    quarantineRouteAvailable: S.Boolean,
    schemaVersion: S.Literal(TassadarGradientWindowIntakeSchemaVersion),
    sourceRefs: S.Array(S.String),
  }),
  receiptSurface: S.Struct({
    emittedReceiptCount: S.Int,
    expectedReceiptRefPattern: S.String,
    promotionLineageGuardAvailable: S.Boolean,
    promotionLineageSchemaVersion: S.Literal(
      TassadarGradientWindowPromotionLineageSchemaVersion,
    ),
    receiptFeedFormatAvailable: S.Boolean,
    receiptFeedSchemaVersion: S.Literal(
      TassadarGradientWindowPromotionReceiptFeedSchemaVersion,
    ),
    receiptRouteAvailable: S.Boolean,
    receiptSchemaVersion: S.Literal(
      TassadarGradientWindowPromotionReceiptSchemaVersion,
    ),
    receiptVerifierAvailable: S.Boolean,
    receiptVerifierSchemaVersion: S.Literal(
      TassadarGradientWindowPromotionReceiptVerificationSchemaVersion,
    ),
    sourceRefs: S.Array(S.String),
  }),
  registryVersion: S.Literal(PublicProductPromisesVersion),
  runtimeSurface: S.Struct({
    acceptedPublicWindowCount: S.Int,
    canonicalCheckpointMutationCount: S.Int,
    currentRuntimeState: S.Literal('not_live'),
    promotedPublicWindowCount: S.Int,
    settlementReceiptCount: S.Int,
  }),
  schemaVersion: S.Literal(TrainingPublicGradientWindowsSchemaVersion),
  sourceRefs: S.Array(S.String),
  stageRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('public_gradient_window_status_projection'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

export class TrainingPublicGradientWindowsUnsafe extends Error {
  readonly _tag = 'TrainingPublicGradientWindowsUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingPublicGradientWindowsUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export const projectTrainingPublicGradientWindows = (
  input: { generatedAt?: string | undefined } = {},
): TrainingPublicGradientWindowsProjection => {
  const projection = new TrainingPublicGradientWindowsProjection({
    authorityBoundary:
      'Read-only public-gradient-window status projection for training.public_gradient_windows.v1. It exposes the intake admission predicate, regime gate, and promoted-window receipt emitter only; it grants no assignment, dispatch, spend, settlement, aggregation, direct submission, compiled-core gradient mutation, canonical-checkpoint mutation, model promotion, or green product-promise authority.',
    endpoint: TrainingPublicGradientWindowsEndpoint,
    gate: {
      clearsBlockerRefs: [],
      greenGateSatisfied: false,
      intakeAdmissionPredicateAvailable: true,
      liveWindowRuntimeAvailable: false,
      promotedWindowReceiptAvailable: false,
      promotionReceiptEmitterAvailable: true,
      publicProjectionAvailable: true,
      regimeGateAvailable: true,
      remainingBlockerRefs,
      settlementReceiptAvailable: false,
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:training.public_gradient_windows.v1',
    promiseState: 'planned',
    intakeSurface: {
      acceptedSubmissionCount: 0,
      admittedQuarantineRecordCount: 0,
      predicateAvailable: true,
      quarantineRecordFormatAvailable: true,
      quarantineRecordSchemaVersion:
        TassadarGradientWindowQuarantineRecordSchemaVersion,
      quarantineRecordVerifierAvailable: true,
      quarantineRecordVerifierSchemaVersion:
        TassadarGradientWindowQuarantineRecordVerificationSchemaVersion,
      quarantineRouteAvailable: false,
      schemaVersion: TassadarGradientWindowIntakeSchemaVersion,
      sourceRefs: [
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.test.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record.test.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record-verify.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record-verify.test.ts',
      ],
    },
    receiptSurface: {
      emittedReceiptCount: 0,
      expectedReceiptRefPattern:
        'receipt.public.tassadar_gradient_window.promoted.{windowRef}',
      promotionLineageGuardAvailable: true,
      promotionLineageSchemaVersion:
        TassadarGradientWindowPromotionLineageSchemaVersion,
      receiptFeedFormatAvailable: true,
      receiptFeedSchemaVersion:
        TassadarGradientWindowPromotionReceiptFeedSchemaVersion,
      receiptRouteAvailable: false,
      receiptSchemaVersion:
        TassadarGradientWindowPromotionReceiptSchemaVersion,
      receiptVerifierAvailable: true,
      receiptVerifierSchemaVersion:
        TassadarGradientWindowPromotionReceiptVerificationSchemaVersion,
      sourceRefs: [
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt.test.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt-verify.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt-verify.test.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt-feed.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt-feed.test.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-lineage.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-lineage.test.ts',
      ],
    },
    registryVersion: PublicProductPromisesVersion,
    runtimeSurface: {
      acceptedPublicWindowCount: 0,
      canonicalCheckpointMutationCount: 0,
      currentRuntimeState: 'not_live',
      promotedPublicWindowCount: 0,
      settlementReceiptCount: 0,
    },
    schemaVersion: TrainingPublicGradientWindowsSchemaVersion,
    sourceRefs: [
      'docs/tassadar/RESEARCH_PLAN.md',
      'docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md#track-h--hybrid-ring-later-gradients-enter-only-here-2d-4-item-5',
      'docs/launch/vertex-fleet/training.public_gradient_windows.v1.md',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt-verify.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-lineage.ts',
      'apps/openagents.com/workers/api/src/training-public-gradient-windows.ts',
    ],
    stageRefs: [
      'submitted',
      'quarantined',
      'recomputed',
      'replicated',
      'canary_passed',
      'promoted',
      'blocked',
    ],
    staleness: TrainingPublicGradientWindowsStaleness,
    status: 'public_gradient_window_status_projection',
    statusLabel:
      'Public-gradient-window intake predicate, regime gate, and promoted-window receipt surface are visible; no live public gradient window has been accepted, promoted, paid, or settled.',
    unsafeCopy:
      'Do not claim public gradient training is live, that any public contributor window has promoted, that canonical checkpoints are being mutated by public gradients, or that public-gradient settlement receipts exist.',
  })

  assertPublicSafeValue('Training public gradient windows projection', projection)

  return projection
}
