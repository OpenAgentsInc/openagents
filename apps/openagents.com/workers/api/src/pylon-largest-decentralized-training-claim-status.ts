import { Schema as S } from 'effect'

import { PublicProductPromisesVersion } from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  TrainingPublicDistributedRunNetworkScaleQualifiedContributorThreshold,
  TrainingPublicDistributedRunScaleEndpoint,
  projectTrainingPublicDistributedRunScaleFromEnvelope,
} from './training-public-distributed-run-scale'

export const PylonLargestDecentralizedTrainingClaimEndpoint =
  '/api/public/pylon/largest-decentralized-training-claim'
export const PylonLargestDecentralizedTrainingClaimSchemaVersion =
  'openagents.pylon.largest_decentralized_training_claim.status.v1'
export const PylonLargestDecentralizedTrainingClaimBlocker =
  'blocker.product_promises.public_training_contributor_receipts_missing'
export const PylonLargestConcreteComparableContributorBenchmark = 70
export const PylonLargestTranscriptTargetContributorBenchmark = 200

export const PylonLargestDecentralizedTrainingClaimStaleness =
  liveAtReadStaleness([
    'training_run_state_transition_recorded',
    'training_verification_challenge_verified_transition_recorded',
    'training_run_settlement_receipt_recorded',
    'product_promise_registry_updated',
  ])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const thresholdDeficit = (current: number, required: number): number =>
  Math.max(0, required - current)

export class PylonLargestTrainingBenchmarkRow extends S.Class<PylonLargestTrainingBenchmarkRow>(
  'PylonLargestTrainingBenchmarkRow',
)({
  benchmarkId: S.String,
  currentQualifiedContributorCount: S.Int,
  deficit: S.Int,
  label: S.String,
  requiredQualifiedContributorCount: S.Int,
  sourceRefs: S.Array(S.String),
  thresholdMet: S.Boolean,
}) {}

export class PylonLargestDecentralizedTrainingClaimProjection extends S.Class<PylonLargestDecentralizedTrainingClaimProjection>(
  'PylonLargestDecentralizedTrainingClaimProjection',
)({
  authorityBoundary: S.String,
  benchmark: S.Struct({
    concreteComparableContributorBenchmark: S.Int,
    concreteComparableDocRef: S.String,
    networkScaleQualifiedContributorThreshold: S.Int,
    transcriptTargetContributorBenchmark: S.Int,
  }),
  comparisonRows: S.Array(PylonLargestTrainingBenchmarkRow),
  endpoint: S.Literal(PylonLargestDecentralizedTrainingClaimEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    comparableRunResearchAvailable: S.Boolean,
    concreteComparableThresholdMet: S.Boolean,
    greenGateSatisfied: S.Boolean,
    ownerSignedUpgradeAvailable: S.Boolean,
    participantCountMethodologyAvailable: S.Boolean,
    publicContributorReceiptsAtClaimBenchmarkAvailable: S.Boolean,
    publicRunScaleProjectionAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    transcriptTargetThresholdMet: S.Boolean,
  }),
  generatedAt: S.String,
  promiseRef: S.Literal(
    'promise:pylon.largest_decentralized_training_claim.v1',
  ),
  promiseState: S.Literal('red'),
  registryVersion: S.Literal(PublicProductPromisesVersion),
  runScale: S.Struct({
    acceptedTraceCount: S.Int,
    currentScaleLabel: S.Literals(['idle', 'canary_scale', 'network_scale']),
    providerConfirmedSettledPayoutSats: S.Int,
    qualifiedContributorCount: S.Int,
    realSettlementReceiptCount: S.Int,
    runRef: S.String,
    runState: S.String,
    sourceScaleEndpoint: S.Literal(TrainingPublicDistributedRunScaleEndpoint),
  }),
  schemaVersion: S.Literal(PylonLargestDecentralizedTrainingClaimSchemaVersion),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('largest_decentralized_training_claim_status_projection'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

export class PylonLargestDecentralizedTrainingClaimUnsafe extends Error {
  readonly _tag = 'PylonLargestDecentralizedTrainingClaimUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new PylonLargestDecentralizedTrainingClaimUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export const projectPylonLargestDecentralizedTrainingClaimStatusFromEnvelope = (
  envelope: Record<string, unknown>,
): PylonLargestDecentralizedTrainingClaimProjection => {
  const scaleProjection =
    projectTrainingPublicDistributedRunScaleFromEnvelope(envelope)
  const qualifiedContributorCount =
    scaleProjection.runScale.qualifiedContributorCount
  const concreteComparableThresholdMet =
    qualifiedContributorCount >= PylonLargestConcreteComparableContributorBenchmark
  const transcriptTargetThresholdMet =
    qualifiedContributorCount >= PylonLargestTranscriptTargetContributorBenchmark
  const sourceRefs = uniqueRefs([
    'docs/launch/vertex-fleet/pylon.largest_decentralized_training_claim.v1.md',
    'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
    'docs/training/2026-06-19-comparable-decentralized-training-runs-research.md',
    'docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md',
    'apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status.ts',
    'apps/openagents.com/workers/api/src/training-public-distributed-run-scale.ts',
    TrainingPublicDistributedRunScaleEndpoint,
    ...scaleProjection.sourceRefs,
  ])

  const projection =
    new PylonLargestDecentralizedTrainingClaimProjection({
      authorityBoundary:
        'Read-only largest-run claim status projection for pylon.largest_decentralized_training_claim.v1. It compares existing public training-run counters to documented benchmarks only; it grants no contributor admission, training dispatch, spend, settlement, benchmark victory, largest-run claim, network-scale claim, or green product-promise authority.',
      benchmark: {
        concreteComparableContributorBenchmark:
          PylonLargestConcreteComparableContributorBenchmark,
        concreteComparableDocRef:
          'docs/training/2026-06-19-comparable-decentralized-training-runs-research.md',
        networkScaleQualifiedContributorThreshold:
          TrainingPublicDistributedRunNetworkScaleQualifiedContributorThreshold,
        transcriptTargetContributorBenchmark:
          PylonLargestTranscriptTargetContributorBenchmark,
      },
      comparisonRows: [
        new PylonLargestTrainingBenchmarkRow({
          benchmarkId: 'templar_covenant_72b_published_comparable',
          currentQualifiedContributorCount: qualifiedContributorCount,
          deficit: thresholdDeficit(
            qualifiedContributorCount,
            PylonLargestConcreteComparableContributorBenchmark,
          ),
          label:
            'Cited public comparable: Templar Covenant-72B at about 70 contributors.',
          requiredQualifiedContributorCount:
            PylonLargestConcreteComparableContributorBenchmark,
          sourceRefs,
          thresholdMet: concreteComparableThresholdMet,
        }),
        new PylonLargestTrainingBenchmarkRow({
          benchmarkId: 'episode_236_transcript_target',
          currentQualifiedContributorCount: qualifiedContributorCount,
          deficit: thresholdDeficit(
            qualifiedContributorCount,
            PylonLargestTranscriptTargetContributorBenchmark,
          ),
          label:
            'Episode 236 target benchmark for the largest-run claim: 200 contributors.',
          requiredQualifiedContributorCount:
            PylonLargestTranscriptTargetContributorBenchmark,
          sourceRefs,
          thresholdMet: transcriptTargetThresholdMet,
        }),
      ],
      endpoint: PylonLargestDecentralizedTrainingClaimEndpoint,
      gate: {
        clearsBlockerRefs: [],
        comparableRunResearchAvailable: true,
        concreteComparableThresholdMet,
        greenGateSatisfied: false,
        ownerSignedUpgradeAvailable: false,
        participantCountMethodologyAvailable: true,
        publicContributorReceiptsAtClaimBenchmarkAvailable:
          transcriptTargetThresholdMet,
        publicRunScaleProjectionAvailable: true,
        remainingBlockerRefs: [PylonLargestDecentralizedTrainingClaimBlocker],
        transcriptTargetThresholdMet,
      },
      generatedAt: scaleProjection.generatedAt,
      promiseRef: 'promise:pylon.largest_decentralized_training_claim.v1',
      promiseState: 'red',
      registryVersion: PublicProductPromisesVersion,
      runScale: {
        acceptedTraceCount: scaleProjection.runScale.acceptedTraceCount,
        currentScaleLabel: scaleProjection.runScale.currentScaleLabel,
        providerConfirmedSettledPayoutSats:
          scaleProjection.runScale.providerConfirmedSettledPayoutSats,
        qualifiedContributorCount,
        realSettlementReceiptCount:
          scaleProjection.runScale.realSettlementReceiptCount,
        runRef: scaleProjection.runScale.runRef,
        runState: scaleProjection.runScale.runState,
        sourceScaleEndpoint: TrainingPublicDistributedRunScaleEndpoint,
      },
      schemaVersion: PylonLargestDecentralizedTrainingClaimSchemaVersion,
      sourceRefs,
      staleness: PylonLargestDecentralizedTrainingClaimStaleness,
      status: 'largest_decentralized_training_claim_status_projection',
      statusLabel: transcriptTargetThresholdMet
        ? 'The 200-contributor benchmark is met in current counters, but owner-signed receipt-first upgrade remains required before any largest-run claim.'
        : `Current public run has ${qualifiedContributorCount}/${PylonLargestTranscriptTargetContributorBenchmark} qualified contributors, so the largest decentralized training claim remains red.`,
      unsafeCopy:
        'Do not say OpenAgents has the largest decentralized training run, has beaten Bittensor or Templar, has 200+ contributors, or has public training at largest-run scale unless benchmark-level qualified-contributor receipts and owner signoff exist.',
    })

  assertPublicSafeValue(
    'Pylon largest decentralized training claim projection',
    projection,
  )

  return projection
}
