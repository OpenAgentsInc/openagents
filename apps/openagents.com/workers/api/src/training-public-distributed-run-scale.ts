import { Schema as S } from 'effect'

import { PublicProductPromisesVersion } from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  DEFAULT_TASSADAR_RUN_REF,
  PublicTassadarRunSummarySchemaVersion,
} from './public-tassadar-run-summary-routes'
import { currentIsoTimestamp } from './runtime-primitives'

export const TrainingPublicDistributedRunScaleEndpoint =
  '/api/public/training/public-distributed-run-scale'
export const TrainingPublicDistributedRunScaleSchemaVersion =
  'openagents.training.public_distributed_run.scale_status.v1'
export const TrainingPublicDistributedRunReceiptsBlocker =
  'blocker.product_promises.public_distributed_training_run_receipts_missing'
export const TrainingPublicDistributedRunNetworkScaleQualifiedContributorThreshold = 50

export const TrainingPublicDistributedRunScaleStaleness = liveAtReadStaleness([
  'training_run_state_transition_recorded',
  'training_verification_challenge_verified_transition_recorded',
  'training_run_settlement_receipt_recorded',
  'product_promise_registry_updated',
])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const recordAt = (
  record: Record<string, unknown>,
  key: string,
): Record<string, unknown> => {
  const value = record[key]
  return isRecord(value) ? value : {}
}

const stringAt = (
  record: Record<string, unknown>,
  key: string,
  fallback: string,
): string => {
  const value = record[key]
  return typeof value === 'string' && value.trim() !== '' ? value : fallback
}

const numberAt = (
  record: Record<string, unknown>,
  key: string,
  fallback = 0,
): number => {
  const value = record[key]
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback
}

const stringArrayAt = (
  record: Record<string, unknown>,
  key: string,
): ReadonlyArray<string> => {
  const value = record[key]
  return Array.isArray(value)
    ? uniqueRefs(
        value.filter((item): item is string => typeof item === 'string'),
      )
    : []
}

const metricValue = (metrics: Record<string, unknown>, key: string): number =>
  numberAt(recordAt(metrics, key), 'value')

const metricSourceRefs = (
  metrics: Record<string, unknown>,
  key: string,
): ReadonlyArray<string> => stringArrayAt(recordAt(metrics, key), 'sourceRefs')

export class TrainingPublicDistributedRunScaleAxis extends S.Class<TrainingPublicDistributedRunScaleAxis>(
  'TrainingPublicDistributedRunScaleAxis',
)({
  axisId: S.String,
  currentValue: S.Int,
  label: S.String,
  provenanceLabel: S.String,
  requiredValue: S.Int,
  sourceRefs: S.Array(S.String),
  thresholdMet: S.Boolean,
}) {}

export class TrainingPublicDistributedRunScaleProjection extends S.Class<TrainingPublicDistributedRunScaleProjection>(
  'TrainingPublicDistributedRunScaleProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TrainingPublicDistributedRunScaleEndpoint),
  gate: S.Struct({
    broadAcceptedWorkReceiptsAvailable: S.Boolean,
    clearsBlockerRefs: S.Array(S.String),
    greenGateSatisfied: S.Boolean,
    networkScaleThresholdMet: S.Boolean,
    ownerSignedUpgradeAvailable: S.Boolean,
    participantCountMethodologyAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    publicRunDefinitionAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    settlementRefsForMultipleContributorsAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  methodology: S.Struct({
    comparableLargestRunContributorBenchmark: S.Int,
    methodologyDocRef: S.String,
    networkScaleQualifiedContributorThreshold: S.Int,
    ruleSummary: S.String,
  }),
  promiseRef: S.Literal('promise:training.public_distributed_training_run.v1'),
  promiseState: S.Literal('red'),
  registryVersion: S.Literal(PublicProductPromisesVersion),
  runScale: S.Struct({
    acceptedTraceCount: S.Int,
    currentScaleLabel: S.Literals(['idle', 'canary_scale', 'network_scale']),
    providerConfirmedSettledPayoutSats: S.Int,
    qualifiedContributorCount: S.Int,
    qualifiedContributorDeficit: S.Int,
    realSettlementReceiptCount: S.Int,
    runRef: S.String,
    runState: S.String,
    sourceSchemaVersion: S.String,
  }),
  scaleAxes: S.Array(TrainingPublicDistributedRunScaleAxis),
  schemaVersion: S.Literal(TrainingPublicDistributedRunScaleSchemaVersion),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('public_distributed_run_scale_status_projection'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

export class TrainingPublicDistributedRunScaleUnsafe extends Error {
  readonly _tag = 'TrainingPublicDistributedRunScaleUnsafe'
}

type TrainingPublicDistributedRunScaleInput = Readonly<{
  acceptedTraceCount: number
  generatedAt?: string | undefined
  providerConfirmedSettledPayoutSats: number
  qualifiedContributorCount: number
  realSettlementReceiptCount: number
  runRef: string
  runState: string
  sourceRefs: ReadonlyArray<string>
  sourceSchemaVersion: string
}>

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingPublicDistributedRunScaleUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export const publicDistributedRunScaleInputFromSummaryEnvelope = (
  envelope: Record<string, unknown>,
): TrainingPublicDistributedRunScaleInput => {
  const metrics = recordAt(envelope, 'metrics')
  const corpus = recordAt(envelope, 'corpus')
  const settlement = recordAt(envelope, 'settlement')
  const runRef = stringAt(envelope, 'runRef', DEFAULT_TASSADAR_RUN_REF)
  const runState = stringAt(envelope, 'runState', 'planned')
  const generatedAt = stringAt(envelope, 'generatedAt', currentIsoTimestamp())

  return {
    acceptedTraceCount: numberAt(corpus, 'acceptedTraceCount'),
    generatedAt,
    providerConfirmedSettledPayoutSats: metricValue(
      metrics,
      'providerConfirmedSettledPayoutSats',
    ),
    qualifiedContributorCount: metricValue(
      metrics,
      'qualifiedContributorCount',
    ),
    realSettlementReceiptCount: numberAt(settlement, 'settledReceiptCount'),
    runRef,
    runState,
    sourceRefs: uniqueRefs([
      ...stringArrayAt(envelope, 'sourceRefs'),
      ...stringArrayAt(corpus, 'traceRefs'),
      ...stringArrayAt(corpus, 'verdictRefs'),
      ...stringArrayAt(settlement, 'sourceRefs'),
      ...metricSourceRefs(metrics, 'providerConfirmedSettledPayoutSats'),
      ...metricSourceRefs(metrics, 'qualifiedContributorCount'),
      ...metricSourceRefs(metrics, 'verifiedWorkCount'),
    ]),
    sourceSchemaVersion: stringAt(
      envelope,
      'schemaVersion',
      PublicTassadarRunSummarySchemaVersion,
    ),
  }
}

export const projectTrainingPublicDistributedRunScale = (
  input: TrainingPublicDistributedRunScaleInput,
): TrainingPublicDistributedRunScaleProjection => {
  const qualifiedContributorCount = Math.max(
    0,
    Math.trunc(input.qualifiedContributorCount),
  )
  const acceptedTraceCount = Math.max(0, Math.trunc(input.acceptedTraceCount))
  const realSettlementReceiptCount = Math.max(
    0,
    Math.trunc(input.realSettlementReceiptCount),
  )
  const providerConfirmedSettledPayoutSats = Math.max(
    0,
    Math.trunc(input.providerConfirmedSettledPayoutSats),
  )
  const threshold =
    TrainingPublicDistributedRunNetworkScaleQualifiedContributorThreshold
  const networkScaleThresholdMet = qualifiedContributorCount >= threshold
  const broadAcceptedWorkReceiptsAvailable = networkScaleThresholdMet
  const sourceRefs = uniqueRefs([
    'docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md',
    'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
    'docs/promises/2026-06-19-training-live-run-evidence-destale.md',
    'apps/openagents.com/workers/api/src/training-public-distributed-run-scale.ts',
    'apps/openagents.com/workers/api/src/public-tassadar-run-summary-routes.ts',
    ...input.sourceRefs,
  ])
  const scaleLabel = networkScaleThresholdMet
    ? 'network_scale'
    : qualifiedContributorCount > 0 || acceptedTraceCount > 0
      ? 'canary_scale'
      : 'idle'

  const projection = new TrainingPublicDistributedRunScaleProjection({
    authorityBoundary:
      'Read-only public distributed training scale-status projection for training.public_distributed_training_run.v1. It summarizes existing run, verification, and settlement receipts only; it grants no contributor admission, training dispatch, spend, settlement, model-quality, largest-run, network-scale, or green product-promise authority.',
    endpoint: TrainingPublicDistributedRunScaleEndpoint,
    gate: {
      broadAcceptedWorkReceiptsAvailable,
      clearsBlockerRefs: [],
      greenGateSatisfied: false,
      networkScaleThresholdMet,
      ownerSignedUpgradeAvailable: false,
      participantCountMethodologyAvailable: true,
      publicProjectionAvailable: true,
      publicRunDefinitionAvailable: input.runRef.trim() !== '',
      remainingBlockerRefs: [TrainingPublicDistributedRunReceiptsBlocker],
      settlementRefsForMultipleContributorsAvailable:
        qualifiedContributorCount >= 2 && realSettlementReceiptCount >= 2,
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    methodology: {
      comparableLargestRunContributorBenchmark: 200,
      methodologyDocRef:
        'docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md',
      networkScaleQualifiedContributorThreshold: threshold,
      ruleSummary:
        'Qualified contributors count only when they have admitted work, accepted replay-verified useful work, and public-safe provider-confirmed realBitcoinMoved:true settlement receipts linked to the run. Raw registrations and stale heartbeats never count.',
    },
    promiseRef: 'promise:training.public_distributed_training_run.v1',
    promiseState: 'red',
    registryVersion: PublicProductPromisesVersion,
    runScale: {
      acceptedTraceCount,
      currentScaleLabel: scaleLabel,
      providerConfirmedSettledPayoutSats,
      qualifiedContributorCount,
      qualifiedContributorDeficit: Math.max(
        0,
        threshold - qualifiedContributorCount,
      ),
      realSettlementReceiptCount,
      runRef: input.runRef,
      runState: input.runState,
      sourceSchemaVersion: input.sourceSchemaVersion,
    },
    scaleAxes: [
      new TrainingPublicDistributedRunScaleAxis({
        axisId: 'qualified_contributors',
        currentValue: qualifiedContributorCount,
        label: 'Qualified contributors with accepted work and real settlement.',
        provenanceLabel:
          'Derived from the public training-run summary qualifiedContributorCount metric.',
        requiredValue: threshold,
        sourceRefs,
        thresholdMet: networkScaleThresholdMet,
      }),
      new TrainingPublicDistributedRunScaleAxis({
        axisId: 'accepted_exact_trace_work',
        currentValue: acceptedTraceCount,
        label: 'Accepted replay-verified exact-trace work units.',
        provenanceLabel:
          'Derived from the public training-run summary corpus.acceptedTraceCount.',
        requiredValue: threshold,
        sourceRefs,
        thresholdMet: acceptedTraceCount >= threshold,
      }),
      new TrainingPublicDistributedRunScaleAxis({
        axisId: 'real_settlement_receipts',
        currentValue: realSettlementReceiptCount,
        label: 'Provider-confirmed realBitcoinMoved:true settlement receipts.',
        provenanceLabel:
          'Derived from the public training-run settlement reconciliation.',
        requiredValue: threshold,
        sourceRefs,
        thresholdMet: realSettlementReceiptCount >= threshold,
      }),
    ],
    schemaVersion: TrainingPublicDistributedRunScaleSchemaVersion,
    sourceRefs,
    staleness: TrainingPublicDistributedRunScaleStaleness,
    status: 'public_distributed_run_scale_status_projection',
    statusLabel: networkScaleThresholdMet
      ? 'Network-scale receipt threshold is met in the live counters, but owner-signed upgrade remains required before any green claim.'
      : `Current public run is ${scaleLabel}; ${qualifiedContributorCount}/${threshold} qualified contributors are counted, so the public distributed training run blocker remains active.`,
    unsafeCopy:
      'Do not claim a public network-scale distributed training run is live, broad paid contribution is available at scale, the run is the largest, or any model-quality/capability result exists until comparable-scale accepted-work and settlement receipts plus owner signoff exist.',
  })

  assertPublicSafeValue(
    'Training public distributed run scale projection',
    projection,
  )

  return projection
}

export const projectTrainingPublicDistributedRunScaleFromEnvelope = (
  envelope: Record<string, unknown>,
): TrainingPublicDistributedRunScaleProjection =>
  projectTrainingPublicDistributedRunScale(
    publicDistributedRunScaleInputFromSummaryEnvelope(envelope),
  )
