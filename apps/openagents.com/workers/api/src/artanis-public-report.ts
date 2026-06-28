import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  ArtanisForumPublicationQueueRecord,
  exampleArtanisForumPublicationQueue,
  projectArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import {
  ArtanisForumRewardSmokeProjection,
  exampleArtanisForumRewardSmokeRecord,
  projectArtanisForumRewardSmoke,
} from './artanis-forum-reward-smoke'
import {
  ArtanisForumRewardVisibilityProjection,
  exampleArtanisForumAcceptedContributionBridgeProjections,
  exampleArtanisForumRewardVisibilityRecord,
  projectArtanisForumRewardVisibility,
} from './artanis-forum-reward-visibility'
import {
  ArtanisGepaProductionSmokeProjection,
  exampleArtanisGepaProductionSmokeRecord,
  projectArtanisGepaProductionSmoke,
} from './artanis-gepa-production-smoke'
import {
  ArtanisGepaScheduledRunnerProofProjection,
  exampleArtanisGepaScheduledRunnerProofRecord,
  projectArtanisGepaScheduledRunnerProof,
} from './artanis-gepa-scheduled-runner-proof'
import {
  ArtanisHealthOverallState,
  ArtanisHealthSignalRecord,
  ArtanisHealthSnapshotRecord,
  projectArtanisHealthSnapshot,
} from './artanis-health'
import {
  ARTANIS_LOOP_READ_ONLY_AUTHORITY,
  ArtanisLoopLedgerRecord,
  ArtanisLoopRecord,
  type ArtanisLoopState,
  type ArtanisLoopTickRecord,
  exampleArtanisLoopLedger,
  projectArtanisLoopLedger,
} from './artanis-loop'
import type { ArtanisTickMonitor } from './artanis-tick-monitor'
import {
  exampleArtanisModelLabContext,
  projectArtanisModelLabContext,
} from './artanis-model-lab-context'
import {
  ArtanisProductionLaunchGateProjection,
  exampleArtanisProductionLaunchGateProjection,
} from './artanis-production-launch-gate'
import {
  ArtanisPylonV02LaunchCommunicationProjection,
  exampleArtanisPylonV02LaunchCommunicationProjection,
} from './artanis-pylon-v02-launch-communications'
import {
  ArtanisPylonV02ReleaseParityProjection,
  exampleArtanisPylonV02ReleaseParityEvidence,
  projectArtanisPylonV02ReleaseParity,
} from './artanis-pylon-v02-release-parity'
import { exampleArtanisRuntime, projectArtanisRuntime } from './artanis-runtime'
import {
  exampleArtanisStandaloneClaimLedger,
  projectArtanisStandaloneClaimLedger,
} from './artanis-standalone-claim-ledger'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { exampleOmniBenchmarkCloud } from './omni-model-lab-benchmark-cloud'
import { exampleOmniModelLabEvidenceGraph } from './omni-model-lab-evidence-graph'
import { exampleOmniModelArtifact } from './omni-model-lab-model-artifact'
import { exampleOmniPromotionDecisionLedger } from './omni-model-lab-promotion-decision'
import { exampleOmniModelLabReport } from './omni-model-lab-report'
import { exampleOmniModelLabRetainedFailureLoop } from './omni-model-lab-retained-failure-loop'
import { exampleOmniTrainingRun } from './omni-model-lab-training-run'
import {
  ProbeGepaCodingOutcomeMetricSnapshot,
  ProbeGepaOutcomeMetricsProjection,
  ProbeGepaOutcomeMetricsSchemaVersion,
  projectProbeGepaOutcomeMetricsForAudience,
} from './probe-gepa-outcome-metrics'
import { PublicClaimState } from './public-claim-state'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
  projectionDataAgeSeconds,
  projectionStalenessExceeded,
  rebuiltOnTransitionStaleness,
} from './public-projection-staleness'
import {
  PublicPylonAcceptedWorkSettlementGate,
  PublicPylonEarningLaunchGate,
  PublicPylonStats,
} from './public-pylon-stats'
import { publicRefTriggersAgentSecretScanner } from './public-ref-scanner-safety'
import {
  PylonV02OmegaReleaseGateProjection,
  currentPylonV02OmegaReleaseGateRecord,
  projectPylonV02OmegaReleaseGate,
} from './pylon-v02-omega-release-gate'
import {
  projectR10PylonCampaign,
  r10PylonCampaignInput,
} from './r10-pylon-campaign'
import {
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

export const ArtanisPublicReportLoopState = S.Literals([
  'blocked',
  'completed',
  'failed',
  'paused',
  'queued',
  'running',
  'waiting_for_approval',
])
export type ArtanisPublicReportLoopState =
  typeof ArtanisPublicReportLoopState.Type

export const ArtanisPublicReportReadiness = S.Literals([
  'blocked',
  'missing_evidence',
  'partial',
  'ready',
])
export type ArtanisPublicReportReadiness =
  typeof ArtanisPublicReportReadiness.Type

export class ArtanisPublicReportForumLink extends S.Class<ArtanisPublicReportForumLink>(
  'ArtanisPublicReportForumLink',
)({
  description: S.String,
  href: S.String,
  label: S.String,
  topicRef: S.String,
}) {}

export class ArtanisPublicReportLoopSummary extends S.Class<ArtanisPublicReportLoopSummary>(
  'ArtanisPublicReportLoopSummary',
)({
  active: S.Boolean,
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  forumPublicationIntentRefs: S.Array(S.String),
  /**
   * Age in whole seconds of the newest persisted loop tick at
   * generation time; null when no persisted tick backs this summary
   * (the typed-example fallback, which `source` then labels).
   */
  latestTickAgeSeconds: S.NullOr(S.Number),
  latestTickRef: S.NullOr(S.String),
  latestTickState: S.NullOr(ArtanisPublicReportLoopState),
  loopRef: S.String,
  nextTickDisplay: S.NullOr(S.String),
  /** True when the newest persisted tick's own next-tick promise has passed. */
  nextTickOverdue: S.Boolean,
  /**
   * True when this summary cannot meet its declared staleness contract
   * — the projection says so instead of asserting stale state as
   * current (epic #4751).
   */
  projectionStale: S.Boolean,
  receiptRefs: S.Array(S.String),
  source: S.Literals(['persisted_loop_ticks', 'typed_example_fallback']),
  staleness: PublicProjectionStalenessContract,
  state: ArtanisPublicReportLoopState,
  tickCount: S.Number,
}) {}

export class ArtanisPublicReportPylonSummary extends S.Class<ArtanisPublicReportPylonSummary>(
  'ArtanisPublicReportPylonSummary',
)({
  acceptedWorkBitcoin24h: S.String,
  acceptedWorkSettlementGate: PublicPylonAcceptedWorkSettlementGate,
  acceptedWorkSettlementReceiptRefs: S.Array(S.String),
  acceptedWorkBitcoinTotal: S.String,
  asOfDisplay: S.NullOr(S.String),
  feedStatus: S.String,
  assignmentReadyPylonsOnlineNow: S.Number,
  earningLaunchGate: PublicPylonEarningLaunchGate,
  nexusPublicRefs: S.Array(S.String),
  omegaPublicRefs: S.Array(S.String),
  pylonPublicRefs: S.Array(S.String),
  pylonsOnlineNow: S.Number,
  sessionsOnlineNow: S.Number,
  sellablePylonsOnlineNow: S.Number,
  sourceRefs: S.Array(S.String),
  trainingAcceptedContributors: S.Number,
  trainingAssignedContributors: S.Number,
  walletReadyPylonsOnlineNow: S.Number,
}) {}

export class ArtanisPublicReportModelLabSummary extends S.Class<ArtanisPublicReportModelLabSummary>(
  'ArtanisPublicReportModelLabSummary',
)({
  blockerRefs: S.Array(S.String),
  claimState: S.NullOr(S.String),
  completeSectionCount: S.Number,
  consumedContractRefs: S.Array(S.String),
  missingContractRefs: S.Array(S.String),
  missingEvidenceRefs: S.Array(S.String),
  publicForumSummaryReportRefs: S.Array(S.String),
  publicPromotionClaimRefs: S.Array(S.String),
  readiness: ArtanisPublicReportReadiness,
  reportRef: S.NullOr(S.String),
  sectionCount: S.Number,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisPublicReportProbeGepaSummary extends S.Class<ArtanisPublicReportProbeGepaSummary>(
  'ArtanisPublicReportProbeGepaSummary',
)({
  acceptedOutcomeRefs: S.Array(S.String),
  candidateHash: S.String,
  candidateRef: S.String,
  candidateState: S.String,
  claimText: S.String,
  productOutcomeClaimAllowed: S.Boolean,
  publicProofRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  selectedSignatureRefs: S.Array(S.String),
  toolMenuRefs: S.Array(S.String),
  workroomComparisonRefs: S.Array(S.String),
  workroomOutcomeRefs: S.Array(S.String),
}) {}

export class ArtanisPublicReportHealthSummary extends S.Class<ArtanisPublicReportHealthSummary>(
  'ArtanisPublicReportHealthSummary',
)({
  attentionLabels: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  overclaimBlocked: S.Boolean,
  overallState: ArtanisHealthOverallState,
  pendingApprovalCount: S.Number,
  publicRecoveryActionRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleOrBlockedSignalCount: S.Number,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisPublicReportAuthoritySummary extends S.Class<ArtanisPublicReportAuthoritySummary>(
  'ArtanisPublicReportAuthoritySummary',
)({
  authorityBlockerRefs: S.Array(S.String),
  dispatchAuthorityAllowed: S.Boolean,
  dispatcherGateGreen: S.Boolean,
  forumAutoPublishAllowed: S.Boolean,
  forumIntentIdempotencyRefs: S.Array(S.String),
  greenLaunchCopyAllowed: S.Boolean,
  operatorApprovalRequired: S.Boolean,
  providerMutationAuthorityAllowed: S.Boolean,
  runbookCommandRefs: S.Array(S.String),
  scheduledRunnerDispatchAllowed: S.Boolean,
  settlementAuthorityAllowed: S.Boolean,
  spendAuthorityAllowed: S.Boolean,
  statusProjectionAllowed: S.Boolean,
}) {}

export class ArtanisPublicReportClaimSummary extends S.Class<ArtanisPublicReportClaimSummary>(
  'ArtanisPublicReportClaimSummary',
)({
  area: S.String,
  blockedByRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimRef: S.String,
  description: S.String,
  evidenceRefs: S.Array(S.String),
  label: S.String,
  state: PublicClaimState,
  stateLabel: S.String,
}) {}

export class ArtanisPublicReportActivityTickerEntry extends S.Class<ArtanisPublicReportActivityTickerEntry>(
  'ArtanisPublicReportActivityTickerEntry',
)({
  activityRef: S.String,
  assignmentRef: S.NullOr(S.String),
  createdAtDisplay: S.String,
  detail: S.String,
  issueNumber: S.NullOr(S.Number),
  label: S.String,
  sourceRefs: S.Array(S.String),
  state: S.String,
}) {}

export class ArtanisPublicReportDecisionFailureMode extends S.Class<ArtanisPublicReportDecisionFailureMode>(
  'ArtanisPublicReportDecisionFailureMode',
)({
  count: S.Number,
  failureModeRef: S.String,
  label: S.String,
  latestDecisionRef: S.NullOr(S.String),
  resultingPublicIssueNumber: S.NullOr(S.Number),
  sourceRefs: S.Array(S.String),
  state: S.String,
}) {}

export class ArtanisPublicReportDecisionLog extends S.Class<ArtanisPublicReportDecisionLog>(
  'ArtanisPublicReportDecisionLog',
)({
  authorityBoundary: S.String,
  countsByState: S.Record(S.String, S.Number),
  failureModes: S.Array(ArtanisPublicReportDecisionFailureMode),
  generatedAtDisplay: S.String,
  sourceRefs: S.Array(S.String),
  ticker: S.Array(ArtanisPublicReportActivityTickerEntry),
}) {}

export class ArtanisPublicReportStateCaveat extends S.Class<ArtanisPublicReportStateCaveat>(
  'ArtanisPublicReportStateCaveat',
)({
  caveats: S.Array(S.String),
  description: S.String,
  label: S.String,
  state: PublicClaimState,
}) {}

export class ArtanisPublicReport extends S.Class<ArtanisPublicReport>(
  'ArtanisPublicReport',
)({
  agentId: S.String,
  agentRef: S.String,
  artifactRefs: S.Array(S.String),
  authoritySummary: ArtanisPublicReportAuthoritySummary,
  autonomousLoop: ArtanisPublicReportLoopSummary,
  campaignRef: S.String,
  claimStateCaveats: S.Array(ArtanisPublicReportStateCaveat),
  decisionLog: ArtanisPublicReportDecisionLog,
  displayName: S.String,
  forumLinks: S.Array(ArtanisPublicReportForumLink),
  forumRewardSmoke: ArtanisForumRewardSmokeProjection,
  forumRewardVisibility: ArtanisForumRewardVisibilityProjection,
  /**
   * Numeric because this payload's safety scan bans raw ISO timestamps
   * in string fields; epoch milliseconds carry the same fact safely.
   */
  generatedAtUnixMs: S.Number,
  healthSummary: ArtanisPublicReportHealthSummary,
  modelLabSummary: ArtanisPublicReportModelLabSummary,
  nexusPublicRefs: S.Array(S.String),
  gepaScheduledRunner: ArtanisGepaScheduledRunnerProofProjection,
  probeGepaProductionSmoke: ArtanisGepaProductionSmokeProjection,
  probeGepaSummary: ArtanisPublicReportProbeGepaSummary,
  publicBlockerRefs: S.Array(S.String),
  publicCaveatRefs: S.Array(S.String),
  publicGoalRefs: S.Array(S.String),
  publicUrls: S.Array(S.String),
  pylonOmegaReleaseGate: PylonV02OmegaReleaseGateProjection,
  pylonLaunchCommunication: ArtanisPylonV02LaunchCommunicationProjection,
  pylonReleaseParity: ArtanisPylonV02ReleaseParityProjection,
  productionLaunchGate: ArtanisProductionLaunchGateProjection,
  pylonSummary: ArtanisPublicReportPylonSummary,
  r10Claims: S.Array(ArtanisPublicReportClaimSummary),
  receiptRefs: S.Array(S.String),
  reportRef: S.String,
  runtimeState: S.String,
  staleness: PublicProjectionStalenessContract,
  standaloneClaims: S.Array(ArtanisPublicReportClaimSummary),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisPublicReportUnsafe extends S.TaggedErrorClass<ArtanisPublicReportUnsafe>()(
  'ArtanisPublicReportUnsafe',
  {
    reason: S.String,
  },
) {}

const unsafePublicReportPattern =
  /(@|\/Users\/|(^|https:\/\/openagents\.com)\/autopilot($|[/?#])|\/home\/|access[_-]?token|auth\.json|authGrantRef|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hiddenSteering|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payloadJson|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[_-]?raw|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|raw[_-]?payout[_-]?target|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const unique = <A>(values: ReadonlyArray<A>): ReadonlyArray<A> => [
  ...new Set(values),
]

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...unique(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const safeRefSuffix = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '') || 'unknown'

const publicReportStrings = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(publicReportStrings)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(publicReportStrings)
  }

  return []
}

export const artanisPublicReportHasPrivateMaterial = (
  report: ArtanisPublicReport,
): boolean =>
  publicReportStrings(report).some(
    value =>
      containsProviderSecretMaterial(value) ||
      publicRefTriggersAgentSecretScanner(value) ||
      unsafePublicReportPattern.test(value) ||
      rawTimestampPattern.test(value),
  )

const bitcoinFromSats = (value: number | null): string =>
  value === null
    ? 'unavailable'
    : `${(value / 100_000_000).toFixed(8)} bitcoin (${new Intl.NumberFormat('en-US').format(value)} sats)`

const healthSignal = (
  input: Omit<ArtanisHealthSignalRecord, 'signalRef'> & {
    signalRefSuffix: string
  },
): ArtanisHealthSignalRecord => {
  const { signalRefSuffix, ...record } = input

  return new ArtanisHealthSignalRecord({
    ...record,
    signalRef: `health.public.artanis.${signalRefSuffix}`,
  })
}

const pylonStatsFresh = (
  pylonStats: PublicPylonStats,
  nowIso: string,
): boolean =>
  pylonStats.available &&
  pylonStats.asOfUnixMs !== null &&
  Date.parse(nowIso) - pylonStats.asOfUnixMs <= 15 * 60 * 1000

const currentArtanisHealthSnapshot = (input: {
  modelLab: ArtanisPublicReportModelLabSummary
  nowIso: string
  productionLaunchGate: ArtanisProductionLaunchGateProjection
  pylonStats: PublicPylonStats
}): ArtanisHealthSnapshotRecord => {
  const nowIso = input.nowIso
  const modelLabReady = input.modelLab.readiness === 'ready'
  const pylonFresh = pylonStatsFresh(input.pylonStats, nowIso)
  const earningGateReady = input.pylonStats.earningLaunchGate.state === 'ready'
  const productionReady =
    input.productionLaunchGate.state === 'ready' &&
    input.productionLaunchGate.failedOrPendingRequiredCount === 0
  const modelLabSourceRefs =
    input.modelLab.reportRef === null ? [] : [input.modelLab.reportRef]
  const signals = [
    healthSignal({
      blockerRefs: [],
      caveatRefs: ['caveat.public.loop_fresh'],
      count: 1,
      kind: 'loop_freshness',
      label: 'Loop is fresh',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.loop.current_report'],
      publicRecoveryActionRefs: [],
      publicStatusRefs: ['health.public.artanis.loop_fresh'],
      signalRefSuffix: 'loop_freshness',
      sourceRefs: ['loop.public.artanis.pylon_model_lab'],
      state: 'fresh' as const,
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: [],
      caveatRefs: ['caveat.public.last_tick_recorded'],
      count: 1,
      kind: 'last_tick',
      label: 'Last tick recorded',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.last_tick'],
      publicRecoveryActionRefs: [],
      publicStatusRefs: ['health.public.artanis.last_tick_seen'],
      signalRefSuffix: 'last_tick',
      sourceRefs: ['tick.public.artanis.gepa_scheduled_runner'],
      state: 'fresh' as const,
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: productionReady
        ? []
        : ['blocker.public.artanis.operator_approval_pending'],
      caveatRefs: ['caveat.public.approval_needed_before_dispatch'],
      count: productionReady ? 0 : 1,
      kind: 'pending_approvals',
      label: productionReady
        ? 'No pending launch approvals'
        : 'Approval pending',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.pending_approval_detail'],
      publicRecoveryActionRefs: productionReady
        ? []
        : ['recovery.public.artanis.wait_for_operator'],
      publicStatusRefs: [
        productionReady
          ? 'health.public.artanis.approvals_clear'
          : 'health.public.artanis.approval_pending',
      ],
      signalRefSuffix: 'pending_approvals',
      sourceRefs: [input.productionLaunchGate.gateRef],
      state: productionReady ? ('available' as const) : ('blocked' as const),
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: modelLabReady
        ? []
        : ['blocker.public.artanis.model_lab_report_stale'],
      caveatRefs: ['caveat.public.model_lab_report_public_summary'],
      count: modelLabReady ? 0 : 1,
      kind: 'blocker_reason',
      label: modelLabReady
        ? 'No stale Model Lab blocker'
        : 'Blocked by stale Model Lab report',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.blocker_detail'],
      publicRecoveryActionRefs: modelLabReady
        ? []
        : ['recovery.public.artanis.refresh_model_lab_summary'],
      publicStatusRefs: [
        modelLabReady
          ? 'health.public.artanis.blockers_clear'
          : 'health.public.artanis.blocked_model_lab_stale',
      ],
      signalRefSuffix: 'blocker_reason',
      sourceRefs: modelLabSourceRefs,
      state: modelLabReady ? ('available' as const) : ('blocked' as const),
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: [],
      caveatRefs: ['caveat.public.forum_updates_are_operator_authorized'],
      count: 0,
      kind: 'forum_publication_lag',
      label: 'Forum publication path reviewed',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.forum_publication_lag'],
      publicRecoveryActionRefs: [],
      publicStatusRefs: ['health.public.artanis.forum_publication_reviewed'],
      signalRefSuffix: 'forum_publication_lag',
      sourceRefs: ['forum.public.artanis.status'],
      state: 'fresh' as const,
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: pylonFresh
        ? []
        : ['blocker.public.artanis.pylon_stats_stale'],
      caveatRefs: ['caveat.public.pylon_stats_fresh'],
      count: input.pylonStats.pylonsOnlineNow,
      kind: 'pylon_stats_freshness',
      label: pylonFresh ? 'Pylon stats are fresh' : 'Pylon stats need refresh',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.pylon_stats'],
      publicRecoveryActionRefs: pylonFresh
        ? []
        : ['recovery.public.artanis.refresh_pylon_stats'],
      publicStatusRefs: [
        pylonFresh
          ? 'health.public.artanis.pylon_stats_fresh'
          : 'health.public.artanis.pylon_stats_stale',
      ],
      signalRefSuffix: 'pylon_stats_freshness',
      sourceRefs: ['pylon.public.stats', 'route:/api/public/pylon-stats'],
      state: pylonFresh ? ('fresh' as const) : ('stale' as const),
      subjectUpdatedAtIso:
        input.pylonStats.asOfUnixMs === null
          ? null
          : epochMillisToIsoTimestamp(input.pylonStats.asOfUnixMs),
    }),
    healthSignal({
      blockerRefs: earningGateReady
        ? []
        : input.pylonStats.earningLaunchGate.blockerRefs,
      caveatRefs: ['caveat.public.omega_pylon_stats_fresh'],
      count: input.pylonStats.pylonsAssignmentReadyNow,
      kind: 'nexus_public_stats_freshness',
      label: earningGateReady
        ? 'Omega public Pylon readiness counters are green'
        : 'Omega public Pylon readiness counters are blocked',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.omega_pylon_stats'],
      publicRecoveryActionRefs: earningGateReady
        ? []
        : ['recovery.public.artanis.refresh_live_pylon_readiness'],
      publicStatusRefs: [
        earningGateReady
          ? 'health.public.artanis.omega_pylon_stats_fresh'
          : 'health.public.artanis.omega_pylon_stats_blocked',
      ],
      signalRefSuffix: 'nexus_public_stats_freshness',
      sourceRefs: ['omega.public.pylon_api.registrations'],
      state: earningGateReady ? ('fresh' as const) : ('blocked' as const),
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: modelLabReady
        ? []
        : ['blocker.public.artanis.model_lab_report_stale'],
      caveatRefs: ['caveat.public.model_lab_report_public_summary'],
      count: modelLabReady ? 0 : 1,
      kind: 'model_lab_report_freshness',
      label: modelLabReady
        ? 'Model Lab report is fresh'
        : 'Model Lab report is stale',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.model_lab_report'],
      publicRecoveryActionRefs: modelLabReady
        ? []
        : ['recovery.public.artanis.refresh_model_lab_summary'],
      publicStatusRefs: [
        modelLabReady
          ? 'health.public.artanis.model_lab_report_fresh'
          : 'health.public.artanis.model_lab_report_stale',
      ],
      signalRefSuffix: 'model_lab_report_freshness',
      sourceRefs: modelLabSourceRefs,
      state: modelLabReady ? ('fresh' as const) : ('stale' as const),
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: [],
      caveatRefs: ['caveat.public.runner_backend_available'],
      count: 1,
      kind: 'runner_backend_availability',
      label: 'Runner backend available',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.runner_backend'],
      publicRecoveryActionRefs: [],
      publicStatusRefs: ['health.public.artanis.runner_backend_available'],
      signalRefSuffix: 'runner_backend_availability',
      sourceRefs: ['runner_backend.public.artanis.shc'],
      state: 'available' as const,
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: [],
      caveatRefs: [
        'authority.public.khala_readiness.credentialless_read_only',
        'authority.public.khala_readiness.no_chat_call',
        'authority.public.khala_readiness.no_mutation',
        'authority.public.khala_readiness.no_paid_call',
        'caveat.public.khala_public_catalog_single_model',
      ],
      count: 0,
      kind: 'khala_readiness',
      label: 'Khala no-spend readiness is clean',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.khala_readiness'],
      publicRecoveryActionRefs: [],
      publicStatusRefs: ['health.public.artanis.khala_ready'],
      signalRefSuffix: 'khala_readiness',
      sourceRefs: [
        'gateway.public.openagents.models',
        'gateway.public.openagents.readiness',
        'model.public.openagents.khala',
      ],
      state: 'available' as const,
      subjectUpdatedAtIso: nowIso,
    }),
    healthSignal({
      blockerRefs: productionReady
        ? []
        : ['blocker.public.artanis.fleet_overseer_live_proof_missing'],
      caveatRefs: [
        'authority.public.artanis.fleet_overseer.read_only_signal',
        'caveat.public.artanis.fleet_overseer_default_off',
      ],
      count: productionReady ? 1 : 0,
      kind: 'fleet_overseer',
      label: productionReady
        ? 'Fleet overseer launch gate is ready'
        : 'Fleet overseer awaits live proof',
      observedAtIso: nowIso,
      operatorDetailRefs: ['health.operator.artanis.fleet_overseer'],
      publicRecoveryActionRefs: productionReady
        ? []
        : ['recovery.public.artanis.complete_fleet_overseer_live_proof'],
      publicStatusRefs: [
        productionReady
          ? 'health.public.artanis.fleet_overseer_ready'
          : 'health.public.artanis.fleet_overseer_blocked',
      ],
      signalRefSuffix: 'fleet_overseer',
      sourceRefs: ['tick.public.artanis.fleet_overseer'],
      state: productionReady ? ('available' as const) : ('blocked' as const),
      subjectUpdatedAtIso: nowIso,
    }),
  ]
  const attentionSignals = signals.filter(signal =>
    [
      'blocked',
      'degraded',
      'missing',
      'stale',
      'unavailable',
      'unknown',
    ].includes(signal.state),
  )
  const healthy = attentionSignals.length === 0

  return new ArtanisHealthSnapshotRecord({
    agentId: 'agent_artanis',
    blockerRefs: uniqueRefs(signals.flatMap(signal => signal.blockerRefs)),
    caveatRefs: ['caveat.public.artanis.health_blocks_overclaiming'],
    createdAtIso: nowIso,
    latestTickRef: 'tick.public.artanis.gepa_scheduled_runner',
    loopRef: 'loop.public.artanis.pylon_model_lab',
    operatorRecoveryActionRefs: healthy
      ? []
      : ['recovery.operator.artanis.inspect_current_evidence'],
    overallState: healthy ? 'healthy' : 'stale',
    overclaimBlocked: !healthy,
    overclaimBlockerRefs: healthy
      ? []
      : ['overclaim.public.artanis.health_stale'],
    pendingApprovalRefs: productionReady
      ? []
      : ['approval.public.artanis.pylon_dispatch_pending'],
    publicStatusRefs: [
      healthy
        ? 'health.public.artanis.status.healthy'
        : 'health.public.artanis.status.stale',
    ],
    runnerBackendRefs: ['runner_backend.public.artanis.shc'],
    signals,
    snapshotRef: `health.public.artanis.snapshot.${safeRefSuffix(nowIso)}`,
    sourceRefs: uniqueRefs([
      'loop.public.artanis.pylon_model_lab',
      'forum.public.artanis.status',
      ...modelLabSourceRefs,
      'pylon.public.stats',
      'omega.public.pylon_api.registrations',
      'runner_backend.public.artanis.shc',
    ]),
    updatedAtIso: nowIso,
  })
}

const baseForumLinks: ReadonlyArray<ArtanisPublicReportForumLink> = [
  {
    description: 'Main public Forum section for Artanis status and questions.',
    href: '/forum/f/artanis',
    label: 'Artanis Forum',
    topicRef: 'forum.public.artanis',
  },
  {
    description: 'Canonical Artanis status topic.',
    href: '/forum/t/88888888-4001-4001-8001-888888888888',
    label: 'Status topic',
    topicRef: 'topic.public.forum.artanis.status',
  },
  {
    description: 'Pylon campaign state and public blockers.',
    href: '/forum/t/88888888-4002-4002-8002-888888888888',
    label: 'Pylon campaign',
    topicRef: 'topic.public.forum.artanis.pylon_campaign',
  },
  {
    description: 'Model Lab evidence and continual-learning summaries.',
    href: '/forum/t/88888888-4003-4003-8003-888888888888',
    label: 'Model Lab',
    topicRef: 'topic.public.forum.artanis.model_lab',
  },
  {
    description: 'Pylon v0.2 launch and release readiness updates.',
    href: '/forum/t/88888888-4004-4004-8004-888888888888',
    label: 'Pylon release updates',
    topicRef: 'topic.public.forum.artanis.pylon_release_work_log',
  },
  {
    description: 'Local Pylon resource modes and setup questions.',
    href: '/forum/t/88888888-4007-4007-8007-888888888888',
    label: 'Resource modes',
    topicRef: 'topic.public.forum.artanis.resource_modes',
  },
]

const statusTopicHref = '/forum/t/88888888-4001-4001-8001-888888888888'

const forumLinksForQueue = (
  queue: ReturnType<typeof projectArtanisForumPublicationQueue>,
): ReadonlyArray<ArtanisPublicReportForumLink> => {
  const latestDeliveredStatus = queue.intents.find(
    intent =>
      intent.deliveryState === 'delivered' &&
      intent.postRef !== null &&
      intent.targetTopicRef === 'topic.public.forum.artanis.status',
  )

  if (latestDeliveredStatus === undefined) {
    return baseForumLinks
  }

  return [
    ...baseForumLinks,
    {
      description: `Latest delivered Artanis status Forum post: ${latestDeliveredStatus.postRef}.`,
      href: statusTopicHref,
      label: 'Latest status post',
      topicRef: latestDeliveredStatus.targetTopicRef,
    },
  ]
}

const claimAreaLabel = (area: string): string =>
  area
    .split('_')
    .map(word => `${word.slice(0, 1).toUpperCase()}${word.slice(1)}`)
    .join(' ')

const receiptRefsFromRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  uniqueRefs(refs.filter(ref => ref.startsWith('receipt.')))

export const publicNexusPylonReceiptRouteRefsFromRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  uniqueRefs(
    refs
      .filter(
        ref =>
          ref.startsWith('assignment.artanis_admin.') ||
          ref.startsWith('receipt.nexus_pylon.artanis_admin_closeout.'),
      )
      .map(ref => `route:/api/public/nexus-pylon/receipts/${ref}`),
  )

const persistedLoopTerminalStates = new Set<ArtanisLoopState>([
  'blocked',
  'failed',
  'paused',
])

const persistedLoopStateForLatestTick = (
  tick: ArtanisLoopTickRecord,
): ArtanisLoopState => tick.state === 'completed' ? 'running' : tick.state

const sortLoopTicks = (
  ticks: ReadonlyArray<ArtanisLoopTickRecord>,
): ReadonlyArray<ArtanisLoopTickRecord> =>
  [...ticks].sort((left, right) => {
    const updated = left.updatedAtIso.localeCompare(right.updatedAtIso)

    return updated === 0 ? left.tickRef.localeCompare(right.tickRef) : updated
  })

/**
 * Declared staleness contracts for this surface (epic #4751). The
 * report itself composes live at read from persisted tick rows plus a
 * live pylon-stats snapshot; the loop summary inside it projects a
 * stored tick ledger that rebuilds on tick closeout (#4745) and must
 * flag itself stale instead of asserting an old tick as current.
 */
export const ARTANIS_PUBLIC_REPORT_STALENESS = liveAtReadStaleness([
  'artanis_loop_tick_closeout',
  'public_pylon_stats_source_write',
])

export const ARTANIS_LOOP_TICK_PROJECTION_MAX_STALENESS_SECONDS = 86_400

export const ARTANIS_LOOP_TICK_PROJECTION_STALENESS =
  rebuiltOnTransitionStaleness(
    ARTANIS_LOOP_TICK_PROJECTION_MAX_STALENESS_SECONDS,
    ['artanis_loop_tick_closeout'],
  )

export const ARTANIS_LOOP_PROJECTION_STALE_CAVEAT_REF =
  'caveat.public.artanis.loop_tick_projection_exceeds_declared_staleness'

export const ARTANIS_LOOP_PROJECTION_EXAMPLE_FALLBACK_CAVEAT_REF =
  'caveat.public.artanis.loop_projection_example_fallback_not_live_state'

type ArtanisLoopProjectionFreshness = Readonly<{
  latestTickAgeSeconds: number | null
  nextTickOverdue: boolean
  projectionStale: boolean
  source: 'persisted_loop_ticks' | 'typed_example_fallback'
}>

const artanisLoopProjectionFreshness = (
  loopTicks: ReadonlyArray<ArtanisLoopTickRecord> | undefined,
  nowIso: string,
): ArtanisLoopProjectionFreshness => {
  const sortedTicks = sortLoopTicks(loopTicks ?? [])
  const latestTick = sortedTicks[sortedTicks.length - 1]

  if (latestTick === undefined) {
    return {
      latestTickAgeSeconds: null,
      nextTickOverdue: false,
      // Example-composed loop state is never current loop truth; the
      // summary must say so rather than present it as a live tick.
      projectionStale: true,
      source: 'typed_example_fallback',
    }
  }

  const latestTickAgeSeconds = projectionDataAgeSeconds(
    latestTick.updatedAtIso,
    nowIso,
  )
  const nextTickAtMs =
    latestTick.nextTickAtIso === null
      ? Number.NaN
      : Date.parse(latestTick.nextTickAtIso)
  const nextTickOverdue =
    !Number.isNaN(nextTickAtMs) && Date.parse(nowIso) > nextTickAtMs

  return {
    latestTickAgeSeconds,
    nextTickOverdue,
    projectionStale:
      projectionStalenessExceeded(
        ARTANIS_LOOP_TICK_PROJECTION_STALENESS,
        latestTickAgeSeconds,
      ) || nextTickOverdue,
    source: 'persisted_loop_ticks',
  }
}

const artanisLoopLedgerForReport = (
  loopTicks: ReadonlyArray<ArtanisLoopTickRecord> | undefined,
): ArtanisLoopLedgerRecord => {
  const sortedTicks = sortLoopTicks(loopTicks ?? [])

  if (sortedTicks.length === 0) {
    return exampleArtanisLoopLedger()
  }

  const latestTick = sortedTicks[sortedTicks.length - 1]!
  const ticksByLoopRef = sortedTicks.reduce(
    (groups, tick) => {
      groups.set(tick.loopRef, [...(groups.get(tick.loopRef) ?? []), tick])

      return groups
    },
    new Map<string, ReadonlyArray<ArtanisLoopTickRecord>>(),
  )
  const loops = [...ticksByLoopRef.entries()].map(([loopRef, ticks]) => {
    const first = ticks[0]!
    const newest = ticks[ticks.length - 1]!
    const state = persistedLoopStateForLatestTick(newest)

    return new ArtanisLoopRecord({
      active:
        newest.tickRef === latestTick.tickRef &&
        !persistedLoopTerminalStates.has(state),
      agentId: 'agent_artanis',
      blockerRefs: uniqueRefs(ticks.flatMap(tick => tick.blockerRefs)),
      caveatRefs: uniqueRefs(ticks.flatMap(tick => tick.caveatRefs)),
      createdAtIso: first.createdAtIso,
      goalRefs: uniqueRefs(ticks.map(tick => tick.goalRef)),
      loopRef,
      scopeRef: 'scope.public.artanis.persistence.tick',
      state,
      ticks,
      updatedAtIso: newest.updatedAtIso,
    })
  })

  return new ArtanisLoopLedgerRecord({
    agentId: 'agent_artanis',
    authority: ARTANIS_LOOP_READ_ONLY_AUTHORITY,
    caveatRefs: uniqueRefs(sortedTicks.flatMap(tick => tick.caveatRefs)),
    createdAtIso: sortedTicks[0]!.createdAtIso,
    ledgerRef: 'ledger.public.artanis.persistence.report',
    loops,
    updatedAtIso: latestTick.updatedAtIso,
  })
}

const publicIssueNumberPattern =
  /(?:#|issue[:._ -]?|openagentsinc\/openagents#)(\d{2,7})/i

const publicIssueNumberFromText = (value: string): number | null => {
  const match = publicIssueNumberPattern.exec(value)
  if (match === null) {
    return null
  }

  const parsed = Number(match[1])
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null
}

const decisionStateLabel = (state: string): string =>
  state === 'dispatched'
    ? 'Executor dispatched'
    : state === 'no_action'
      ? 'No action'
      : state === 'blocked'
        ? 'Blocked'
        : state === 'dispatch_failed'
          ? 'Dispatch failed'
          : 'Decision recorded'

const decisionFailureModeLabel = (state: string): string =>
  state === 'no_action'
    ? 'No-action decisions'
    : state === 'blocked'
      ? 'Blocked decisions'
      : state === 'dispatch_failed'
        ? 'Dispatch failures'
        : 'Other non-dispatch decisions'

const decisionLogFromTickMonitor = (
  monitor: ArtanisTickMonitor | undefined,
  nowIso: string,
): ArtanisPublicReportDecisionLog => {
  if (monitor === undefined) {
    return new ArtanisPublicReportDecisionLog({
      authorityBoundary:
        'Read-only public-safe decision summary. Grants no dispatch, spend, assignment, settlement, or issue-write authority.',
      countsByState: {},
      failureModes: [],
      generatedAtDisplay: friendlyBlueprintMissionBriefingTime(nowIso, nowIso),
      sourceRefs: ['route:/api/public/artanis/admin-ticks'],
      ticker: [],
    })
  }

  const ticker = monitor.decisions.slice(0, 12).map(decision => {
    const issueNumber = publicIssueNumberFromText(decision.reason)
    return new ArtanisPublicReportActivityTickerEntry({
      activityRef: decision.decisionRef,
      assignmentRef: decision.assignmentRef,
      createdAtDisplay: friendlyBlueprintMissionBriefingTime(
        decision.createdAt,
        nowIso,
      ),
      detail:
        decision.assignmentRef === null
          ? 'Public-safe decision recorded'
          : 'Public-safe assignment decision recorded',
      issueNumber,
      label: decisionStateLabel(decision.state),
      sourceRefs: uniqueRefs([
        'route:/api/public/artanis/admin-ticks',
        decision.decisionRef,
        ...(decision.assignmentRef === null ? [] : [decision.assignmentRef]),
        ...(issueNumber === null ? [] : [`issue.github.${issueNumber}`]),
      ]),
      state: decision.state,
    })
  })
  const nonDispatchDecisions = monitor.decisions.filter(
    decision => decision.state !== 'dispatched',
  )
  const grouped = nonDispatchDecisions.reduce((groups, decision) => {
    const current = groups.get(decision.state) ?? []
    groups.set(decision.state, [...current, decision])

    return groups
  }, new Map<string, typeof nonDispatchDecisions>())
  const failureModes = [...grouped.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([state, decisions]) => {
      const issueNumber =
        decisions
          .map(decision => publicIssueNumberFromText(decision.reason))
          .find((issue): issue is number => issue !== null) ?? null
      const latestDecision = decisions[0] ?? null

      return new ArtanisPublicReportDecisionFailureMode({
        count: decisions.length,
        failureModeRef: `failure.public.artanis.decision.${safeRefSuffix(
          state,
        )}`,
        label: decisionFailureModeLabel(state),
        latestDecisionRef: latestDecision?.decisionRef ?? null,
        resultingPublicIssueNumber: issueNumber,
        sourceRefs: uniqueRefs([
          'route:/api/public/artanis/admin-ticks',
          ...(latestDecision === null ? [] : [latestDecision.decisionRef]),
          ...(issueNumber === null ? [] : [`issue.github.${issueNumber}`]),
        ]),
        state,
      })
    })

  return new ArtanisPublicReportDecisionLog({
    authorityBoundary: monitor.authorityBoundary,
    countsByState: monitor.countsByState,
    failureModes,
    generatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      monitor.generatedAt,
      nowIso,
    ),
    sourceRefs: ['route:/api/public/artanis/admin-ticks'],
    ticker,
  })
}

const probeGepaOutcomeSnapshot = (
  overrides: Partial<ProbeGepaCodingOutcomeMetricSnapshot> = {},
): ProbeGepaCodingOutcomeMetricSnapshot =>
  new ProbeGepaCodingOutcomeMetricSnapshot({
    acceptanceRateBps: 5_000,
    artifactCompletenessBps: 7_000,
    closeoutQualityBps: 6_000,
    costPerAcceptedOutcomeRef: 'cost_per_accepted_outcome.coding.probe_gepa',
    failureFamilyReductionBps: 1_000,
    humanReviewMinutes: 14,
    privateProofState: 'private_proof_available',
    proofBundleCompletenessBps: 7_000,
    publicProofState: 'redacted',
    regressionCount: 3,
    retriesPerAcceptedOutcome: 2,
    retryCount: 3,
    turnsPerAcceptedOutcome: 8,
    ...overrides,
  })

const exampleProbeGepaOutcomeMetricsProjection =
  (): ProbeGepaOutcomeMetricsProjection =>
    new ProbeGepaOutcomeMetricsProjection({
      acceptedOutcomeRefs: [],
      after: probeGepaOutcomeSnapshot({
        acceptanceRateBps: 6_250,
        artifactCompletenessBps: 8_500,
        closeoutQualityBps: 7_500,
        failureFamilyReductionBps: 2_500,
        humanReviewMinutes: 10,
        proofBundleCompletenessBps: 8_500,
        regressionCount: 1,
        retriesPerAcceptedOutcome: 1,
        retryCount: 1,
        turnsPerAcceptedOutcome: 6,
      }),
      before: probeGepaOutcomeSnapshot(),
      benchmarkCampaignRefs: [
        'campaign.probe_gepa.stage0.live_shc_harbor_smoke.2026_06_08',
      ],
      benchmarkValidationRefs: [
        'benchmark_result.probe_gepa.live_stage0.retained.001',
      ],
      candidateHash:
        'sha256:0000000000000000000000000000000000000000000000000000000000001880',
      candidateRef: 'candidate.probe_gepa.stage0.live_smoke.seed',
      candidateState: 'shadow',
      claimBoundaryRef: 'claim_boundary.probe_gepa.retained_smoke_only',
      closeoutQualityRef: 'closeout_quality.probe_gepa.live_stage0',
      failureFamilyRefs: ['failure_family.runner_supervision'],
      privateProofRefs: [],
      publicProofRefs: [],
      regressionRefs: ['regression.probe_gepa.live_stage0.none_blocking'],
      routeScorecardRefs: [
        'route_scorecard.probe_gepa.live_stage0.demo_1',
        'route_scorecard.probe_gepa.live_stage0.demo_2',
      ],
      schemaVersion: ProbeGepaOutcomeMetricsSchemaVersion,
      selectedSignatureRefs: [
        'program_signature.probe.benchmark.runner_supervision.v1',
      ],
      toolMenuRefs: ['tool_menu.probe.terminal_bench.db_wal_recovery.v1'],
      workroomComparisonRefs: [
        'workroom_comparison.coding_autopilot.probe_gepa.live_stage0',
      ],
      workroomOutcomeRefs: [],
      workroomRefs: ['workroom.coding_autopilot.probe_gepa.live_stage0'],
    })

export const artanisPublicReportSnapshot = (input: {
  forumPublicationQueue?: ArtanisForumPublicationQueueRecord | undefined
  tickMonitor?: ArtanisTickMonitor | undefined
  loopTicks?: ReadonlyArray<ArtanisLoopTickRecord> | undefined
  nowIso?: string | undefined
  pylonStats: PublicPylonStats
}): ArtanisPublicReport => {
  const nowIso = input.nowIso ?? currentIsoTimestamp()
  const runtime = projectArtanisRuntime(
    exampleArtanisRuntime(),
    'public',
    nowIso,
  )
  const loop = projectArtanisLoopLedger(
    artanisLoopLedgerForReport(input.loopTicks),
    'public',
    nowIso,
  )
  const publicationQueue = projectArtanisForumPublicationQueue(
    input.forumPublicationQueue ?? exampleArtanisForumPublicationQueue(),
    nowIso,
  )
  const forumLinks = forumLinksForQueue(publicationQueue)
  const modelLab = projectArtanisModelLabContext(
    exampleArtanisModelLabContext({
      benchmarkCloud: exampleOmniBenchmarkCloud(),
      evidenceGraph: exampleOmniModelLabEvidenceGraph(),
      modelArtifact: exampleOmniModelArtifact(),
      promotionDecisionLedger: exampleOmniPromotionDecisionLedger(),
      publicReport: exampleOmniModelLabReport(),
      retainedFailureLoop: exampleOmniModelLabRetainedFailureLoop(),
      trainingRun: exampleOmniTrainingRun(),
    }),
    'public_artanis',
    nowIso,
  )
  const campaign = projectR10PylonCampaign(
    r10PylonCampaignInput(),
    'public',
    nowIso,
  )
  const standaloneClaimsLedger = projectArtanisStandaloneClaimLedger(
    exampleArtanisStandaloneClaimLedger(),
    'public',
    nowIso,
  )
  const forumRewardVisibility = projectArtanisForumRewardVisibility(
    exampleArtanisForumRewardVisibilityRecord(),
    exampleArtanisForumAcceptedContributionBridgeProjections('public', nowIso),
    'public',
    nowIso,
  )
  const forumRewardSmoke = projectArtanisForumRewardSmoke(
    exampleArtanisForumRewardSmokeRecord(),
    'public',
    nowIso,
  )
  const pylonLaunchCommunication =
    exampleArtanisPylonV02LaunchCommunicationProjection(nowIso)
  const pylonReleaseParity = projectArtanisPylonV02ReleaseParity(
    exampleArtanisPylonV02ReleaseParityEvidence(),
    'public',
    nowIso,
  )
  const pylonOmegaReleaseGate = projectPylonV02OmegaReleaseGate(
    currentPylonV02OmegaReleaseGateRecord(),
    'public',
    nowIso,
  )
  const decisionLog = decisionLogFromTickMonitor(input.tickMonitor, nowIso)
  const productionLaunchGate =
    exampleArtanisProductionLaunchGateProjection(nowIso)
  const health = projectArtanisHealthSnapshot(
    currentArtanisHealthSnapshot({
      modelLab: {
        blockerRefs: modelLab.blockerRefs,
        claimState: modelLab.publicReport?.claimState ?? null,
        completeSectionCount: modelLab.publicReport?.completeSectionCount ?? 0,
        consumedContractRefs: modelLab.consumedContractRefs,
        missingContractRefs: modelLab.missingContractRefs,
        missingEvidenceRefs: modelLab.missingEvidenceRefs,
        publicForumSummaryReportRefs: modelLab.publicForumSummaryReportRefs,
        publicPromotionClaimRefs: modelLab.publicPromotionClaimRefs,
        readiness: modelLab.readiness,
        reportRef: modelLab.publicReport?.reportRef ?? null,
        sectionCount: modelLab.publicReport?.sectionCount ?? 0,
        updatedAtDisplay: modelLab.updatedAtDisplay,
      },
      nowIso,
      productionLaunchGate,
      pylonStats: input.pylonStats,
    }),
    'public_artanis',
    nowIso,
  )
  const probeGepaProductionSmoke = projectArtanisGepaProductionSmoke(
    exampleArtanisGepaProductionSmokeRecord(nowIso),
    nowIso,
  )
  const gepaScheduledRunner = projectArtanisGepaScheduledRunnerProof(
    exampleArtanisGepaScheduledRunnerProofRecord(nowIso),
    nowIso,
  )
  const probeGepaOutcomeMetrics = projectProbeGepaOutcomeMetricsForAudience(
    exampleProbeGepaOutcomeMetricsProjection(),
    'public',
  )
  const activeLoop =
    loop.loops.find(candidate => candidate.active) ?? loop.loops[0]
  const latestTick = activeLoop?.ticks[activeLoop.ticks.length - 1] ?? null
  const loopFreshness = artanisLoopProjectionFreshness(input.loopTicks, nowIso)
  const loopFreshnessCaveatRefs = [
    ...(loopFreshness.source === 'typed_example_fallback'
      ? [ARTANIS_LOOP_PROJECTION_EXAMPLE_FALLBACK_CAVEAT_REF]
      : []),
    ...(loopFreshness.projectionStale
      ? [ARTANIS_LOOP_PROJECTION_STALE_CAVEAT_REF]
      : []),
  ]
  const r10Claims = campaign.entries.map(entry => ({
    area: entry.area,
    blockedByRefs: entry.blockedByRefs,
    caveatRefs: entry.caveatRefs,
    claimRef: entry.claimRef,
    description: entry.state.description,
    evidenceRefs: entry.evidenceRefs,
    label: claimAreaLabel(entry.area),
    state: entry.state.state,
    stateLabel: entry.state.label,
  }))
  const standaloneClaims = standaloneClaimsLedger.entries.map(entry => ({
    area: entry.area,
    blockedByRefs: entry.blockedByRefs,
    caveatRefs: entry.caveatRefs,
    claimRef: entry.claimRef,
    description: entry.state.description,
    evidenceRefs: entry.evidenceRefs,
    label: claimAreaLabel(entry.area),
    state: entry.state.state,
    stateLabel: entry.state.label,
  }))
  const claimStateCounts = [
    ...standaloneClaimsLedger.stateCounts,
    ...campaign.stateCounts,
  ].reduce((counts, count) => {
    counts.set(count.state, (counts.get(count.state) ?? 0) + count.count)

    return counts
  }, new Map<PublicClaimState, number>())
  const claimStateCaveats = [...claimStateCounts]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([state]) => {
      const firstEntry =
        standaloneClaimsLedger.entries.find(
          entry => entry.state.state === state,
        ) ?? campaign.entries.find(entry => entry.state.state === state)

      return {
        caveats: firstEntry?.state.caveats ?? [],
        description:
          firstEntry?.state.description ??
          'No public claim-state description is available.',
        label: firstEntry?.state.label ?? claimAreaLabel(state),
        state,
      }
    })
  const pylonBlockers = [
    ...(input.pylonStats.available
      ? []
      : ['blocker.public.artanis.pylon_stats_unavailable']),
    ...input.pylonStats.earningLaunchGate.blockerRefs,
  ]
  const healthRecoveryRefs = uniqueRefs(
    health.signals.flatMap(signal => signal.publicRecoveryActionRefs),
  )
  const healthAttentionLabels = health.signals
    .filter(signal =>
      [
        'blocked',
        'degraded',
        'missing',
        'stale',
        'unavailable',
        'unknown',
      ].includes(signal.state),
    )
    .map(signal => signal.label)
  const healthAllowsGreenLaunchCopy = ![
    'blocked',
    'degraded',
    'stale',
    'unavailable',
    'unknown',
  ].includes(health.overallState)
  const statusProjectionAllowed =
    productionLaunchGate.canClaimBoundedStatusProjection &&
    gepaScheduledRunner.state === 'retained'
  const greenLaunchCopyAllowed =
    statusProjectionAllowed && healthAllowsGreenLaunchCopy
  const authoritySummary = new ArtanisPublicReportAuthoritySummary({
    authorityBlockerRefs: uniqueRefs([
      ...(greenLaunchCopyAllowed
        ? []
        : [
            ...(healthAllowsGreenLaunchCopy
              ? []
              : ['blocker.public.artanis.green_launch_copy.health_stale']),
            ...(statusProjectionAllowed
              ? []
              : [
                  'blocker.public.artanis.green_launch_copy.status_projection_not_ready',
                ]),
          ]),
      ...(productionLaunchGate.dispatchAuthorityAllowed
        ? []
        : ['blocker.public.artanis.dispatch_authority_not_granted']),
      ...(productionLaunchGate.walletSpendAuthorityAllowed
        ? []
        : ['blocker.public.artanis.spend_authority_not_granted']),
      ...(productionLaunchGate.settlementAuthorityAllowed
        ? []
        : ['blocker.public.artanis.settlement_authority_not_granted']),
      ...(productionLaunchGate.providerMutationAuthorityAllowed
        ? []
        : ['blocker.public.artanis.provider_mutation_not_granted']),
      ...(productionLaunchGate.forumAutoPublishAllowed
        ? []
        : ['blocker.public.artanis.forum_auto_publish_not_granted']),
    ]),
    dispatchAuthorityAllowed: productionLaunchGate.dispatchAuthorityAllowed,
    dispatcherGateGreen: productionLaunchGate.dispatchAuthorityAllowed,
    forumAutoPublishAllowed: productionLaunchGate.forumAutoPublishAllowed,
    forumIntentIdempotencyRefs: gepaScheduledRunner.noDuplicateForumPostRefs,
    greenLaunchCopyAllowed,
    operatorApprovalRequired: true,
    providerMutationAuthorityAllowed:
      productionLaunchGate.providerMutationAuthorityAllowed,
    runbookCommandRefs: productionLaunchGate.runbookCommandRefs,
    scheduledRunnerDispatchAllowed:
      gepaScheduledRunner.assignmentDispatchAllowed,
    settlementAuthorityAllowed: productionLaunchGate.settlementAuthorityAllowed,
    spendAuthorityAllowed: productionLaunchGate.walletSpendAuthorityAllowed,
    statusProjectionAllowed,
  })
  const publicBlockerRefs = uniqueRefs([
    ...runtime.blockerRefs,
    ...(activeLoop?.blockerRefs ?? []),
    ...(latestTick?.blockerRefs ?? []),
    ...health.blockerRefs,
    ...health.overclaimBlockerRefs,
    ...health.signals.flatMap(signal => signal.blockerRefs),
    ...modelLab.blockerRefs,
    ...publicationQueue.intents.flatMap(intent => intent.blockerRefs),
    ...forumRewardVisibility.blockerRefs,
    ...pylonOmegaReleaseGate.blockerRefs,
    ...(pylonOmegaReleaseGate.hostedMdkDirectPayoutClaimAllowed
      ? []
      : ['blocker.mdk.hosted_programmatic_payouts_disabled']),
    ...productionLaunchGate.blockerRefs,
    ...authoritySummary.authorityBlockerRefs,
    ...probeGepaProductionSmoke.blockerRefs,
    ...gepaScheduledRunner.blockerRefs,
    ...pylonReleaseParity.blockerRefs,
    ...campaign.entries.flatMap(entry => entry.blockedByRefs),
    ...standaloneClaimsLedger.entries.flatMap(entry => entry.blockedByRefs),
    ...pylonBlockers,
  ])
  const pylonSummary: ArtanisPublicReportPylonSummary = {
    acceptedWorkBitcoin24h: bitcoinFromSats(
      input.pylonStats.nexusAcceptedWorkPayoutSatsPaid24h,
    ),
    acceptedWorkSettlementGate:
      input.pylonStats.nexusAcceptedWorkSettlementGate,
    acceptedWorkSettlementReceiptRefs:
      input.pylonStats.nexusAcceptedWorkPayoutReceiptRefs,
    acceptedWorkBitcoinTotal: bitcoinFromSats(
      input.pylonStats.nexusAcceptedWorkPayoutSatsPaidTotal,
    ),
    asOfDisplay:
      input.pylonStats.asOfLabel === null
        ? null
        : friendlyBlueprintMissionBriefingTime(
            input.pylonStats.asOfLabel,
            nowIso,
          ),
    assignmentReadyPylonsOnlineNow: input.pylonStats.pylonsAssignmentReadyNow,
    earningLaunchGate: input.pylonStats.earningLaunchGate,
    feedStatus: input.pylonStats.status,
    nexusPublicRefs: uniqueRefs([
      ...(input.pylonStats.hostedNexusRelayUrl === null
        ? []
        : [input.pylonStats.hostedNexusRelayUrl]),
      ...(input.pylonStats.nexusAcceptedWorkPayoutSatsPaidTotal === null
        ? []
        : ['nexus.public.accepted_work_payout_receipts']),
      ...input.pylonStats.nexusAcceptedWorkPayoutReceiptRefs,
      ...input.pylonStats.nexusAcceptedWorkPayoutReceiptRefs.map(
        receiptRef => `route:/api/public/nexus-pylon/receipts/${receiptRef}`,
      ),
      ...publicNexusPylonReceiptRouteRefsFromRefs([
        ...(latestTick?.receiptRefs ?? []),
        ...(latestTick?.closeoutReceiptRefs ?? []),
      ]),
    ]),
    omegaPublicRefs: uniqueRefs([
      'omega.public.pylon_api.registrations',
      ...input.pylonStats.sourceRefs,
      input.pylonStats.sourceUrl,
    ]),
    pylonPublicRefs: uniqueRefs([
      'pylon.public.resource_modes',
      'pylon.public.v0_2_readiness',
      `pylon.public.minimum_client_version.${safeRefSuffix(
        input.pylonStats.minimumClientVersion,
      )}`,
      ...input.pylonStats.recentPylons
        .slice(0, 4)
        .map(pylon => `pylon.public.${pylon.nostrPubkeyShort}`),
    ]),
    pylonsOnlineNow: input.pylonStats.pylonsOnlineNow,
    sessionsOnlineNow: input.pylonStats.pylonSessionsOnlineNow,
    sellablePylonsOnlineNow: input.pylonStats.sellablePylonsOnlineNow,
    sourceRefs: uniqueRefs([
      'route:/api/public/pylon-stats',
      ...input.pylonStats.sourceRefs,
    ]),
    trainingAcceptedContributors: input.pylonStats.trainingAcceptedContributors,
    trainingAssignedContributors: input.pylonStats.trainingAssignedContributors,
    walletReadyPylonsOnlineNow: input.pylonStats.pylonsWalletReadyNow,
  }
  const modelLabReport = modelLab.publicReport
  const report: ArtanisPublicReport = {
    agentId: runtime.agentId,
    agentRef: runtime.agentRef,
    artifactRefs: uniqueRefs([
      ...(latestTick?.artifactRefs ?? []),
      ...publicationQueue.intents.flatMap(intent => intent.artifactRefs),
      ...(modelLabReport?.artifactRefs ?? []),
    ]),
    authoritySummary,
    autonomousLoop: {
      active: activeLoop?.active ?? false,
      artifactRefs: latestTick?.artifactRefs ?? [],
      blockerRefs: uniqueRefs([
        ...(activeLoop?.blockerRefs ?? []),
        ...(latestTick?.blockerRefs ?? []),
      ]),
      caveatRefs: uniqueRefs([
        ...(activeLoop?.caveatRefs ?? []),
        ...(latestTick?.caveatRefs ?? []),
        ...loopFreshnessCaveatRefs,
      ]),
      forumPublicationIntentRefs: latestTick?.forumPublicationIntentRefs ?? [],
      latestTickAgeSeconds: loopFreshness.latestTickAgeSeconds,
      latestTickRef: latestTick?.tickRef ?? null,
      latestTickState: latestTick?.state ?? null,
      loopRef: activeLoop?.loopRef ?? 'loop.public.artanis.none',
      nextTickDisplay: latestTick?.nextTickDisplay ?? null,
      nextTickOverdue: loopFreshness.nextTickOverdue,
      projectionStale: loopFreshness.projectionStale,
      receiptRefs: uniqueRefs([
        ...(latestTick?.receiptRefs ?? []),
        ...(latestTick?.closeoutReceiptRefs ?? []),
      ]),
      source: loopFreshness.source,
      staleness: ARTANIS_LOOP_TICK_PROJECTION_STALENESS,
      state: activeLoop?.state ?? 'blocked',
      tickCount: activeLoop?.tickCount ?? 0,
    },
    campaignRef: campaign.campaignRef,
    claimStateCaveats,
    decisionLog,
    displayName: runtime.displayName,
    forumLinks,
    forumRewardSmoke,
    forumRewardVisibility,
    generatedAtUnixMs: Date.parse(nowIso),
    healthSummary: {
      attentionLabels: uniqueRefs(healthAttentionLabels),
      blockerRefs: uniqueRefs([
        ...health.blockerRefs,
        ...health.overclaimBlockerRefs,
        ...health.signals.flatMap(signal => signal.blockerRefs),
      ]),
      overclaimBlocked: health.overclaimBlocked,
      overallState: health.overallState,
      pendingApprovalCount: health.pendingApprovalCount,
      publicRecoveryActionRefs: healthRecoveryRefs,
      publicStatusRefs: uniqueRefs([
        ...health.publicStatusRefs,
        ...health.signals.flatMap(signal => signal.publicStatusRefs),
      ]),
      sourceRefs: health.sourceRefs,
      staleOrBlockedSignalCount: health.staleOrBlockedSignalCount,
      updatedAtDisplay: health.updatedAtDisplay,
    },
    modelLabSummary: {
      blockerRefs: modelLab.blockerRefs,
      claimState: modelLabReport?.claimState ?? null,
      completeSectionCount: modelLabReport?.completeSectionCount ?? 0,
      consumedContractRefs: modelLab.consumedContractRefs,
      missingContractRefs: modelLab.missingContractRefs,
      missingEvidenceRefs: modelLab.missingEvidenceRefs,
      publicForumSummaryReportRefs: modelLab.publicForumSummaryReportRefs,
      publicPromotionClaimRefs: modelLab.publicPromotionClaimRefs,
      readiness: modelLab.readiness,
      reportRef: modelLabReport?.reportRef ?? null,
      sectionCount: modelLabReport?.sectionCount ?? 0,
      updatedAtDisplay: modelLab.updatedAtDisplay,
    },
    nexusPublicRefs: pylonSummary.nexusPublicRefs,
    gepaScheduledRunner,
    probeGepaProductionSmoke,
    probeGepaSummary: {
      acceptedOutcomeRefs: probeGepaOutcomeMetrics.acceptedOutcomeRefs,
      candidateHash: probeGepaOutcomeMetrics.candidateHash,
      candidateRef: probeGepaOutcomeMetrics.candidateRef,
      candidateState: probeGepaOutcomeMetrics.candidateState,
      claimText: probeGepaOutcomeMetrics.claimText,
      productOutcomeClaimAllowed:
        probeGepaOutcomeMetrics.productOutcomeClaimAllowed,
      publicProofRefs: probeGepaOutcomeMetrics.publicProofRefs,
      routeScorecardRefs: probeGepaOutcomeMetrics.routeScorecardRefs,
      selectedSignatureRefs: probeGepaOutcomeMetrics.selectedSignatureRefs,
      toolMenuRefs: probeGepaOutcomeMetrics.toolMenuRefs,
      workroomComparisonRefs: probeGepaOutcomeMetrics.workroomComparisonRefs,
      workroomOutcomeRefs: probeGepaOutcomeMetrics.workroomOutcomeRefs,
    },
    publicBlockerRefs,
    publicCaveatRefs: uniqueRefs([
      ...runtime.caveatRefs,
      ...loopFreshnessCaveatRefs,
      ...loop.caveatRefs,
      ...health.caveatRefs,
      ...health.signals.flatMap(signal => signal.caveatRefs),
      ...publicationQueue.caveatRefs,
      ...forumRewardVisibility.caveatRefs,
      ...forumRewardSmoke.caveatRefs,
      ...pylonReleaseParity.caveatRefs,
      ...pylonOmegaReleaseGate.payoutModeGate.caveatRefs,
      ...probeGepaProductionSmoke.caveatRefs,
      ...(pylonOmegaReleaseGate.state === 'ready_for_operator_release_review' ||
      pylonOmegaReleaseGate.state === 'limited_launcher_release_shipped'
        ? []
        : ['caveat.public.pylon_v0_2_omega_release_gate_blocked']),
      ...(productionLaunchGate.canClaimContinuouslyRunning
        ? []
        : [
            'caveat.public.artanis.production_launch_gate_blocks_autonomy_claims',
          ]),
      ...modelLab.caveatRefs,
      ...campaign.entries.flatMap(entry => [
        ...entry.caveatRefs,
        ...entry.state.caveats,
      ]),
      ...standaloneClaimsLedger.entries.flatMap(entry => [
        ...entry.caveatRefs,
        ...entry.state.caveats,
      ]),
    ]),
    publicGoalRefs: uniqueRefs([
      ...runtime.goalRefs,
      ...loop.loops.flatMap(loop => loop.goalRefs),
      ...publicationQueue.intents.flatMap(intent => intent.goalRefs),
    ]),
    publicUrls: runtime.publicUrls,
    pylonOmegaReleaseGate,
    pylonLaunchCommunication,
    pylonReleaseParity,
    productionLaunchGate,
    pylonSummary,
    r10Claims,
    receiptRefs: uniqueRefs([
      ...(latestTick?.receiptRefs ?? []),
      ...(latestTick?.closeoutReceiptRefs ?? []),
      ...publicationQueue.intents.flatMap(intent => [
        ...intent.receiptRefs,
        ...intent.deliveryReceiptRefs,
      ]),
      ...pylonReleaseParity.acceptedWorkProofRefs,
      ...pylonReleaseParity.paidWorkReceiptRefs,
      ...pylonReleaseParity.settlementReceiptRefs,
      ...input.pylonStats.nexusAcceptedWorkPayoutReceiptRefs,
      ...receiptRefsFromRefs(pylonOmegaReleaseGate.evidenceRefs),
      ...receiptRefsFromRefs(pylonOmegaReleaseGate.multiPylonProofRefs),
      ...receiptRefsFromRefs(probeGepaProductionSmoke.closeoutBundleRefs),
      ...receiptRefsFromRefs(probeGepaProductionSmoke.psionicImportRefs),
      ...receiptRefsFromRefs(gepaScheduledRunner.closeoutReceiptRefs),
      ...forumRewardVisibility.forumReceiptRefs,
      ...forumRewardSmoke.receiptProjectionRefs,
    ]),
    reportRef: 'report.public.artanis.status_aggregator',
    runtimeState: runtime.state,
    staleness: ARTANIS_PUBLIC_REPORT_STALENESS,
    standaloneClaims,
    updatedAtDisplay: runtime.updatedAtDisplay,
  }

  if (artanisPublicReportHasPrivateMaterial(report)) {
    throw new ArtanisPublicReportUnsafe({
      reason: 'Artanis public report contains private or raw material.',
    })
  }

  return report
}
