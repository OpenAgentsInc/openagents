import { Schema as S } from 'effect'

import {
  PublicProductPromisesVersion,
  publicProductPromisesDocument,
} from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import { TassadarPerceptaCpuTransformTrainingReceiptsEndpoint } from './tassadar-percepta-cpu-transform-training-receipts'
import { TrainingMarathonOperationsEndpoint } from './training-marathon-operations'
import { TrainingModelLadderRungsEndpoint } from './training-model-ladder-rungs'
import { TrainingPublicDistributedRunScaleEndpoint } from './training-public-distributed-run-scale'

export const TrainingFullPipelineProgramEndpoint =
  '/api/public/training/full-pipeline-program'
export const TrainingFullPipelineProgramSchemaVersion =
  'openagents.training.full_pipeline_program.v1'
export const TrainingFullPipelineProgramBlocker =
  'blocker.product_promises.training_pipeline_rails_incomplete'

export const TrainingFullPipelineProgramStaleness = liveAtReadStaleness([
  'product_promise_registry_updated',
  'training_pipeline_stage_receipt_published',
  'training_pipeline_stage_route_published',
])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const promiseStates = ['green', 'yellow', 'red', 'planned', 'degraded'] as const
const receiptStates = [
  'green_receipts_live',
  'partial_receipt_surface_live',
  'green_ready_owner_gated',
  'methodology_only',
  'contract_only',
  'missing',
] as const

type PromiseState = (typeof promiseStates)[number]
type ReceiptState = (typeof receiptStates)[number]

export class TrainingFullPipelineProgramStage extends S.Class<TrainingFullPipelineProgramStage>(
  'TrainingFullPipelineProgramStage',
)({
  blockerRefs: S.Array(S.String),
  endpointRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  promiseId: S.String,
  promiseState: S.Literals(promiseStates),
  receiptState: S.Literals(receiptStates),
  role: S.String,
  stageId: S.String,
  statusLabel: S.String,
}) {}

export class TrainingFullPipelineProgramProjection extends S.Class<TrainingFullPipelineProgramProjection>(
  'TrainingFullPipelineProgramProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TrainingFullPipelineProgramEndpoint),
  gate: S.Struct({
    endToEndRunReceiptAvailable: S.Boolean,
    everyWorkstreamAtLeastYellow: S.Boolean,
    greenGateSatisfied: S.Boolean,
    ladderRungEndToEndReceiptAvailable: S.Boolean,
    paidNetworkWorkloadBroadlyLive: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:training.full_pipeline_program.v1'),
  promiseState: S.Literal('planned'),
  registryVersion: S.Literal(PublicProductPromisesVersion),
  schemaVersion: S.Literal(TrainingFullPipelineProgramSchemaVersion),
  sourceRefs: S.Array(S.String),
  stageSummary: S.Struct({
    greenReadyOwnerGatedStageCount: S.Int,
    liveEndpointCount: S.Int,
    partialReceiptSurfaceCount: S.Int,
    stageCount: S.Int,
    states: S.Record(S.String, S.Int),
  }),
  stages: S.Array(TrainingFullPipelineProgramStage),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('training_pipeline_program_status_projection'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

export class TrainingFullPipelineProgramUnsafe extends Error {
  readonly _tag = 'TrainingFullPipelineProgramUnsafe'
}

type StageDefinition = Readonly<{
  evidenceRefs: ReadonlyArray<string>
  endpointRefs: ReadonlyArray<string>
  promiseId: string
  receiptState: ReceiptState
  role: string
  stageId: string
  statusLabel: string
}>

const stageDefinitions: ReadonlyArray<StageDefinition> = [
  {
    endpointRefs: ['/api/training/refinery/a4'],
    evidenceRefs: [
      'apps/openagents.com/workers/api/src/training-data-refinery.ts',
      'apps/openagents.com/workers/api/src/training-leaderboards.ts',
      'apps/openagents.com/docs/2026-06-10-cs336-a4-data-refinery-payment-policy.md',
    ],
    promiseId: 'training.data_refinery_corpus.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'Corpus refinery and eval-delta lane.',
    stageId: 'data_refinery',
    statusLabel:
      'A4 deterministic refinery and empty eval-delta leaderboard exist; crawl-scale paid shards and eval-delta payment remain missing.',
  },
  {
    endpointRefs: ['/api/public/training/ablation-derisking-ledger'],
    evidenceRefs: [
      'docs/training/2026-06-20-ablation-one-delta-harness.md',
      'docs/training/2026-06-20-ablation-eval-reproduction-receipt.md',
    ],
    promiseId: 'training.ablation_system.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'One-delta ablation harness and eval-reproduction ledger.',
    stageId: 'ablation',
    statusLabel:
      'Ablation ledger, one-delta manifest checks, and retained eval reproduction are live; paid ablation dispatch remains missing.',
  },
  {
    endpointRefs: ['/api/public/training/public-gradient-windows'],
    evidenceRefs: [
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.test.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.test.ts',
      'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt.ts',
      'apps/openagents.com/workers/api/src/training-public-gradient-windows.ts',
    ],
    promiseId: 'training.public_gradient_windows.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'Public gradient-window quarantine, recompute, canary, promotion, and rollback gate.',
    stageId: 'public_gradient_windows',
    statusLabel:
      'The intake admission predicate, promotion regime, promoted-window receipt emitter, and public status projection are code-backed, but no public contributor gradient window has been accepted, promoted, paid, or settled.',
  },
  {
    endpointRefs: [
      TrainingPublicDistributedRunScaleEndpoint,
      '/api/public/training/runs/run.tassadar.executor.20260615',
      '/api/public/training/runs/run.tassadar.executor.20260615/settlements',
    ],
    evidenceRefs: [
      'docs/promises/2026-06-19-training-live-run-evidence-destale.md',
      'docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md',
      'apps/openagents.com/workers/api/src/training-public-distributed-run-scale.ts',
    ],
    promiseId: 'training.public_distributed_training_run.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'Public distributed run scale, accepted work, validation, and settlement.',
    stageId: 'public_distributed_run',
    statusLabel:
      'The bounded live run has canary-scale accepted and settled receipts, now projected against the >=50 qualified-contributor threshold; network-scale broad receipts remain missing.',
  },
  {
    endpointRefs: [TrainingMarathonOperationsEndpoint],
    evidenceRefs: [
      'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
      'apps/openagents.com/workers/api/src/training-marathon-operations.ts',
      'apps/openagents.com/workers/api/src/training-window-bootstrap.ts',
      'apps/openagents.com/workers/api/src/training-run-window-authority.ts',
      'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
    ],
    promiseId: 'training.marathon_operations.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'Durable seals, standby contributors, restart criteria, and curtailment drill.',
    stageId: 'marathon_operations',
    statusLabel:
      'Seal/bootstrap and standby predicates are publicly projected; durable remote checkpoint read-back, live standby promotion, and curtailment drill receipts remain missing.',
  },
  {
    endpointRefs: [
      '/api/public/training/post-training-arc/instruct-sft-lane',
      '/api/public/training/post-training-arc/dpo-preference-workload',
    ],
    evidenceRefs: [
      'docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md',
      'docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md',
      'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
    ],
    promiseId: 'training.post_training_arc.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'Instruct SFT, preference rollout, and vibe-test artifacts.',
    stageId: 'post_training',
    statusLabel:
      'Fixture-scale instruct-SFT lane, fixture sync, and deterministic DPO reference workload receipts exist; paid SFT dispatch, paid preference work, and vibe-test artifact remain missing.',
  },
  {
    endpointRefs: [TrainingModelLadderRungsEndpoint],
    evidenceRefs: [
      'docs/training/2026-06-19-model-ladder-rung-economics.md',
      'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
      'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
    ],
    promiseId: 'training.model_ladder.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'R0 through R4 rung sequencing and economics gates.',
    stageId: 'model_ladder',
    statusLabel:
      'R0 is retained and the R1 closeout/economics format is publicly projected; no rung above R0 has run to a closeout receipt.',
  },
  {
    endpointRefs: ['/api/training/device-capabilities/a2'],
    evidenceRefs: [
      'docs/training/2026-06-20-cs336-a2-second-device-class-x86_64-linux-intel.md',
      'docs/training/2026-06-20-cs336-a2-thermal-throttle-classifier.md',
    ],
    promiseId: 'training.device_capability_dataset.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'Device-capability measurements and thermal/replication labels.',
    stageId: 'device_capability',
    statusLabel:
      'The public dataset covers two observed classes and a thermal classifier; verified thermal rows and cross-machine replication remain missing.',
  },
  {
    endpointRefs: [
      '/api/public/training/verification-challenges/{challengeRef}',
    ],
    evidenceRefs: [
      'docs/promises/2026-06-20-verification-class-sampling-policy.md',
    ],
    promiseId: 'training.verification_classes.v1',
    receiptState: 'green_receipts_live',
    role: 'Named verification classes on training work.',
    stageId: 'verification_classes',
    statusLabel:
      'Verification-class policy is green and exact-trace replay is exercised on real paid work.',
  },
  {
    endpointRefs: [
      '/api/public/models/tassadar-percepta-executor/architecture-receipts',
      TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
    ],
    evidenceRefs: [
      'docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md',
      'docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md',
      'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
      'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.ts',
      'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.test.ts',
    ],
    promiseId: 'models.tassadar_percepta_executor.v1',
    receiptState: 'partial_receipt_surface_live',
    role: 'Tassadar Percepta executor model/spec, architecture receipts, and CPU-transform receipt status.',
    stageId: 'tassadar_percepta_executor',
    statusLabel:
      'Model/spec, architecture receipts, and CPU-transform receipt status exist; Pylon CPU-transform assignment, accepted work, verifier verdict, settlement, and trained artifact receipts remain missing.',
  },
  {
    endpointRefs: [
      '/api/public/artanis/tick-streak',
      '/api/public/artanis/tassadar-distillation-dataset',
    ],
    evidenceRefs: [
      'docs/training/2026-06-20-artanis-distillation-dataset-receipt.md',
    ],
    promiseId: 'artanis.tassadar_evolution_loop.v1',
    receiptState: 'green_ready_owner_gated',
    role: 'Artanis unattended tick streak and Tassadar distillation dataset.',
    stageId: 'artanis_evolution_loop',
    statusLabel:
      'Both acceptance legs are receipted and blockerRefs are empty; green transition remains owner-signed.',
  },
]

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingFullPipelineProgramUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

const promiseById = () => {
  const document = publicProductPromisesDocument()

  return new Map(
    document.promises.map(promise => [
      promise.promiseId,
      {
        blockerRefs: promise.blockerRefs,
        evidenceRefs: promise.evidenceRefs,
        state: promise.state,
      },
    ]),
  )
}

const toPromiseState = (state: string | undefined): PromiseState =>
  promiseStates.find(candidate => candidate === state) ?? 'planned'

const buildStages = (): ReadonlyArray<TrainingFullPipelineProgramStage> => {
  const promises = promiseById()

  return stageDefinitions.map(definition => {
    const promise = promises.get(definition.promiseId)
    const evidenceRefs = [
      ...new Set([
        ...definition.evidenceRefs,
        ...(promise?.evidenceRefs.filter(
          ref => ref.startsWith('route:') || ref.startsWith('docs/training/'),
        ) ?? []),
      ]),
    ].sort()

    return new TrainingFullPipelineProgramStage({
      blockerRefs: [...(promise?.blockerRefs ?? [])].sort(),
      endpointRefs: [...definition.endpointRefs].sort(),
      evidenceRefs,
      promiseId: definition.promiseId,
      promiseState: toPromiseState(promise?.state),
      receiptState: definition.receiptState,
      role: definition.role,
      stageId: definition.stageId,
      statusLabel: definition.statusLabel,
    })
  })
}

const countByState = (
  stages: ReadonlyArray<TrainingFullPipelineProgramStage>,
): Record<string, number> =>
  stages.reduce<Record<string, number>>((counts, stage) => {
    counts[stage.promiseState] = (counts[stage.promiseState] ?? 0) + 1

    return counts
  }, {})

export const projectTrainingFullPipelineProgram = (
  input: { generatedAt?: string | undefined } = {},
): TrainingFullPipelineProgramProjection => {
  const stages = buildStages()
  const everyWorkstreamAtLeastYellow = stages.every(stage =>
    ['green', 'yellow'].includes(stage.promiseState),
  )
  const projection = new TrainingFullPipelineProgramProjection({
    authorityBoundary:
      'Read-only public training-pipeline program status projection for training.full_pipeline_program.v1. It reports stage receipts and blockers only; it grants no assignment, dispatch, spend, settlement, canonical-checkpoint mutation, model promotion, service availability, or green product-promise authority.',
    endpoint: TrainingFullPipelineProgramEndpoint,
    gate: {
      endToEndRunReceiptAvailable: false,
      everyWorkstreamAtLeastYellow,
      greenGateSatisfied: false,
      ladderRungEndToEndReceiptAvailable: false,
      paidNetworkWorkloadBroadlyLive: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [TrainingFullPipelineProgramBlocker],
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:training.full_pipeline_program.v1',
    promiseState: 'planned',
    registryVersion: PublicProductPromisesVersion,
    schemaVersion: TrainingFullPipelineProgramSchemaVersion,
    sourceRefs: [
      'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
      'docs/training/2026-06-10-training-program-status.md',
      'docs/training/2026-06-20-training-full-pipeline-program-status.md',
      'apps/openagents.com/workers/api/src/training-full-pipeline-program.ts',
    ],
    stageSummary: {
      greenReadyOwnerGatedStageCount: stages.filter(
        stage => stage.receiptState === 'green_ready_owner_gated',
      ).length,
      liveEndpointCount: stages.filter(stage => stage.endpointRefs.length > 0)
        .length,
      partialReceiptSurfaceCount: stages.filter(
        stage => stage.receiptState === 'partial_receipt_surface_live',
      ).length,
      stageCount: stages.length,
      states: countByState(stages),
    },
    stages,
    staleness: TrainingFullPipelineProgramStaleness,
    status: 'training_pipeline_program_status_projection',
    statusLabel:
      'Training pipeline stage status projection is live; the full-pipeline umbrella remains planned until every workstream is at least yellow and a ladder rung completes end to end with receipts.',
    unsafeCopy:
      'Do not claim OpenAgents operates an end-to-end training pipeline, that all stages are paid network workloads, that a rung above R0 has run, or that public gradients, model promotion, settlement, or full-pipeline green status are live.',
  })

  assertPublicSafeValue('Training full pipeline program projection', projection)

  return projection
}
