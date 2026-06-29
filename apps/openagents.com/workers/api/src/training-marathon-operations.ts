import { Schema as S } from 'effect'

import { PublicProductPromisesVersion } from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  CurtailmentDrillBlocker,
  CurtailmentDrillSchemaVersion,
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
} from './training-curtailment-drill'
import {
  DurableCheckpointSealBlocker,
  DurableCheckpointSealSchemaVersion,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import {
  MaxStandbyHeartbeatStalenessMs,
  StandbyDispatchBlocker,
  StandbyDispatchSchemaVersion,
} from './training-standby-dispatch'

export const TrainingMarathonOperationsEndpoint =
  '/api/public/training/marathon-operations'
export const TrainingMarathonOperationsSchemaVersion =
  'openagents.training.marathon_operations.status.v1'
export const TrainingMarathonCurtailmentDrillBlocker = CurtailmentDrillBlocker

export const TrainingMarathonOperationsStaleness = liveAtReadStaleness([
  'product_promise_registry_updated',
  'training_marathon_durable_checkpoint_receipt_published',
  'training_marathon_standby_promotion_receipt_published',
  'training_marathon_curtailment_drill_receipt_published',
])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const MarathonRemainingBlockerRefs = [
  DurableCheckpointSealBlocker,
  StandbyDispatchBlocker,
  TrainingMarathonCurtailmentDrillBlocker,
] as const

export class TrainingMarathonCheckpointSurface extends S.Class<TrainingMarathonCheckpointSurface>(
  'TrainingMarathonCheckpointSurface',
)({
  blockerRef: S.Literal(DurableCheckpointSealBlocker),
  bootstrapSelectsOnlyDurableSeal: S.Boolean,
  durableCheckpointSealReceiptAvailable: S.Boolean,
  liveSealBoundaryWired: S.Boolean,
  minimumDurableReplicationFactor: S.Int,
  predicateAvailable: S.Boolean,
  remoteCheckpointStoreReadbackReceiptAvailable: S.Boolean,
  schemaVersion: S.Literal(DurableCheckpointSealSchemaVersion),
  sourceRefs: S.Array(S.String),
  statusLabel: S.String,
}) {}

export class TrainingMarathonStandbySurface extends S.Class<TrainingMarathonStandbySurface>(
  'TrainingMarathonStandbySurface',
)({
  blockerRef: S.Literal(StandbyDispatchBlocker),
  liveHeartbeatTelemetryFeedAvailable: S.Boolean,
  livePromotionReceiptAvailable: S.Boolean,
  liveVacancyTelemetryFeedAvailable: S.Boolean,
  maxHeartbeatStalenessMs: S.Int,
  predicateAvailable: S.Boolean,
  preflightEndpoint: S.String,
  preflightRouteAvailable: S.Boolean,
  receiptBackedPromotionAvailable: S.Boolean,
  schemaVersion: S.Literal(StandbyDispatchSchemaVersion),
  sourceRefs: S.Array(S.String),
  statusLabel: S.String,
}) {}

export class TrainingMarathonCurtailmentSurface extends S.Class<TrainingMarathonCurtailmentSurface>(
  'TrainingMarathonCurtailmentSurface',
)({
  blockerRef: S.Literal(TrainingMarathonCurtailmentDrillBlocker),
  ackSlaMs: S.Int,
  checkpointResumeReceiptAvailable: S.Boolean,
  curtailmentDrillReceiptAvailable: S.Boolean,
  drillScheduled: S.Boolean,
  haltSlaMs: S.Int,
  flexibleLoadEvidenceCreated: S.Boolean,
  predicateAvailable: S.Boolean,
  preflightEndpoint: S.String,
  preflightRouteAvailable: S.Boolean,
  schemaVersion: S.Literal(CurtailmentDrillSchemaVersion),
  sourceRefs: S.Array(S.String),
  statusLabel: S.String,
}) {}

export class TrainingMarathonOperationsProjection extends S.Class<TrainingMarathonOperationsProjection>(
  'TrainingMarathonOperationsProjection',
)({
  authorityBoundary: S.String,
  checkpointSurface: TrainingMarathonCheckpointSurface,
  curtailmentSurface: TrainingMarathonCurtailmentSurface,
  endpoint: S.Literal(TrainingMarathonOperationsEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    curtailmentDrillReceiptAvailable: S.Boolean,
    durableCheckpointRemoteReadbackReceiptAvailable: S.Boolean,
    greenGateSatisfied: S.Boolean,
    liveStandbyPromotionReceiptAvailable: S.Boolean,
    marathonCloseoutReceiptAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
  }),
  generatedAt: S.String,
  operationsSummary: S.Struct({
    blockerCount: S.Int,
    openReceiptGateCount: S.Int,
    predicateSurfaceCount: S.Int,
    publicEndpointCount: S.Int,
    receiptBackedLiveOperationCount: S.Int,
  }),
  promiseRef: S.Literal('promise:training.marathon_operations.v1'),
  promiseState: S.Literal('planned'),
  registryVersion: S.Literal(PublicProductPromisesVersion),
  schemaVersion: S.Literal(TrainingMarathonOperationsSchemaVersion),
  sourceRefs: S.Array(S.String),
  standbySurface: TrainingMarathonStandbySurface,
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('training_marathon_operations_status_projection'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

export class TrainingMarathonOperationsUnsafe extends Error {
  readonly _tag = 'TrainingMarathonOperationsUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingMarathonOperationsUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

const worklogRef = 'docs/launch/vertex-fleet/training.marathon_operations.v1.md'
const buildoutPlanRef =
  'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md'
const pluralisRoadmapRef =
  'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md'

const sourceRefs = [
  worklogRef,
  buildoutPlanRef,
  pluralisRoadmapRef,
  'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.ts',
  'apps/openagents.com/workers/api/src/training-standby-dispatch.ts',
  'apps/openagents.com/workers/api/src/training-curtailment-drill.ts',
  'apps/openagents.com/workers/api/src/training-run-window-authority.ts',
  'apps/openagents.com/workers/api/src/training-window-bootstrap.ts',
  'apps/openagents.com/workers/api/src/training-marathon-operations.ts',
]

export const projectTrainingMarathonOperations = (
  input: { generatedAt?: string | undefined } = {},
): TrainingMarathonOperationsProjection => {
  const projection = new TrainingMarathonOperationsProjection({
    authorityBoundary:
      'Read-only public marathon-operations status projection for training.marathon_operations.v1. It exposes contract and preflight surfaces only; it grants no training dispatch, standby promotion, checkpoint storage authority, spend, settlement, energy-market claim, flexible-load claim, model promotion, or green product-promise authority.',
    checkpointSurface: new TrainingMarathonCheckpointSurface({
      blockerRef: DurableCheckpointSealBlocker,
      bootstrapSelectsOnlyDurableSeal: true,
      durableCheckpointSealReceiptAvailable: false,
      liveSealBoundaryWired: true,
      minimumDurableReplicationFactor: MinDurableReplicationFactor,
      predicateAvailable: true,
      remoteCheckpointStoreReadbackReceiptAvailable: false,
      schemaVersion: DurableCheckpointSealSchemaVersion,
      sourceRefs: [
        'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.ts',
        'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt.ts',
        'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt-verifier.ts',
        'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt-feed.ts',
        'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt-feed-verifier.ts',
        'apps/openagents.com/workers/api/src/training-run-window-authority.ts',
        'apps/openagents.com/workers/api/src/training-window-bootstrap.ts',
        worklogRef,
      ],
      statusLabel:
        'Durable-checkpoint seal predicate and bootstrap filtering are wired; no real remote checkpoint-store read-back receipt exists.',
    }),
    curtailmentSurface: new TrainingMarathonCurtailmentSurface({
      blockerRef: TrainingMarathonCurtailmentDrillBlocker,
      ackSlaMs: MaxCurtailmentAckLatencyMs,
      checkpointResumeReceiptAvailable: false,
      curtailmentDrillReceiptAvailable: false,
      drillScheduled: false,
      haltSlaMs: MaxCurtailmentHaltLatencyMs,
      flexibleLoadEvidenceCreated: false,
      predicateAvailable: true,
      preflightEndpoint:
        '/api/training/runs/{trainingRunRef}/curtailment-drill-preflight',
      preflightRouteAvailable: true,
      schemaVersion: CurtailmentDrillSchemaVersion,
      sourceRefs: [
        'apps/openagents.com/workers/api/src/training-curtailment-drill.ts',
        'apps/openagents.com/workers/api/src/training-curtailment-drill.test.ts',
        'apps/openagents.com/workers/api/src/training-curtailment-drill-receipt.ts',
        'apps/openagents.com/workers/api/src/training-curtailment-drill-receipt-verifier.ts',
        'apps/openagents.com/workers/api/src/training-curtailment-drill-receipt-feed.ts',
        'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
        buildoutPlanRef,
        worklogRef,
      ],
      statusLabel:
        'Curtailment-drill outcome predicate and admin preflight route exist; no scheduled shed-and-resume drill receipt or flexible-load proof has been produced.',
    }),
    endpoint: TrainingMarathonOperationsEndpoint,
    gate: {
      clearsBlockerRefs: [],
      curtailmentDrillReceiptAvailable: false,
      durableCheckpointRemoteReadbackReceiptAvailable: false,
      greenGateSatisfied: false,
      liveStandbyPromotionReceiptAvailable: false,
      marathonCloseoutReceiptAvailable: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [...MarathonRemainingBlockerRefs],
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    operationsSummary: {
      blockerCount: MarathonRemainingBlockerRefs.length,
      openReceiptGateCount: 3,
      predicateSurfaceCount: 3,
      publicEndpointCount: 1,
      receiptBackedLiveOperationCount: 0,
    },
    promiseRef: 'promise:training.marathon_operations.v1',
    promiseState: 'planned',
    registryVersion: PublicProductPromisesVersion,
    schemaVersion: TrainingMarathonOperationsSchemaVersion,
    sourceRefs,
    standbySurface: new TrainingMarathonStandbySurface({
      blockerRef: StandbyDispatchBlocker,
      liveHeartbeatTelemetryFeedAvailable: false,
      livePromotionReceiptAvailable: false,
      liveVacancyTelemetryFeedAvailable: false,
      maxHeartbeatStalenessMs: MaxStandbyHeartbeatStalenessMs,
      predicateAvailable: true,
      preflightEndpoint:
        '/api/training/runs/{trainingRunRef}/standby-dispatch-preflight',
      preflightRouteAvailable: true,
      receiptBackedPromotionAvailable: false,
      schemaVersion: StandbyDispatchSchemaVersion,
      sourceRefs: [
        'apps/openagents.com/workers/api/src/training-standby-dispatch.ts',
        'apps/openagents.com/workers/api/src/training-standby-dispatch-receipt.ts',
        'apps/openagents.com/workers/api/src/training-standby-dispatch-receipt-verifier.ts',
        'apps/openagents.com/workers/api/src/training-standby-dispatch-receipt-feed.ts',
        'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
        worklogRef,
      ],
      statusLabel:
        'Standby promotion predicate and admin preflight route exist; no live heartbeat/vacancy telemetry feed or receipt-backed standby promotion exists.',
    }),
    staleness: TrainingMarathonOperationsStaleness,
    status: 'training_marathon_operations_status_projection',
    statusLabel:
      'Marathon operation predicates are visible, including curtailment-drill evaluation; remote checkpoint read-back, live standby promotion, and curtailment drill receipts remain absent.',
    unsafeCopy:
      'Do not claim multi-day or multi-week network training is operationally supported, that standby dispatch is live, that durable remote checkpoint storage has been proven, that a curtailment drill happened, that training load is dispatchable for grid value, or that training.marathon_operations.v1 is green.',
  })

  assertPublicSafeValue('Training marathon operations projection', projection)

  return projection
}
