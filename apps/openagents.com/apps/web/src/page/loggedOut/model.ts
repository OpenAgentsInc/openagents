import { ShareProjectionV1 } from '@openagents/sync-schema'
import { Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import { LoggedOutRoute } from '../../route'

// MODEL

export const OnboardingStep = S.Literals([
  'github',
  'repository',
  'funding',
  'workspace',
])
export type OnboardingStep = typeof OnboardingStep.Type

export const OnboardingModel = ts('LoggedOutOnboarding', {
  couponCode: S.String,
  fundingAmount: S.Number,
  isCouponOpen: S.Boolean,
  selectedRepository: S.String,
  step: OnboardingStep,
})
export type OnboardingModel = typeof OnboardingModel.Type

export const PublicAgentGoalStatus = S.Literals([
  'active',
  'paused',
  'blocked',
  'usage_limited',
  'budget_limited',
  'complete',
])

export const PublicAgentGoal = S.Struct({
  id: S.String,
  agentId: S.String,
  objective: S.String,
  status: PublicAgentGoalStatus,
  currentRunId: S.NullOr(S.String),
  tokenBudget: S.NullOr(S.Int),
  tokensUsed: S.Int,
  timeUsedSeconds: S.Int,
  remainingTokens: S.NullOr(S.Int),
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  publicUrl: S.String,
})
export type PublicAgentGoal = typeof PublicAgentGoal.Type

export const PublicAgentGoalEvent = S.Struct({
  id: S.String,
  goalId: S.String,
  runId: S.NullOr(S.String),
  type: S.String,
  status: S.NullOr(S.String),
  summary: S.String,
  tokenDelta: S.Int,
  timeDeltaSeconds: S.Int,
  artifactRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  commitRefs: S.Array(S.String),
  createdAt: S.String,
})
export type PublicAgentGoalEvent = typeof PublicAgentGoalEvent.Type

export const PublicAgentGoalResponse = S.Struct({
  agentId: S.String,
  goal: S.NullOr(PublicAgentGoal),
  events: S.Array(PublicAgentGoalEvent),
})
export type PublicAgentGoalResponse = typeof PublicAgentGoalResponse.Type

export const PublicAdjutantActivityStage = S.Literals([
  'queued',
  'running',
  'reviewing',
  'deployed',
  'waiting_for_input',
  'unavailable',
])
export type PublicAdjutantActivityStage =
  typeof PublicAdjutantActivityStage.Type

export const PublicAdjutantActivityMilestone = S.Struct({
  id: S.String,
  kind: S.Literals(['order', 'site']),
  stage: PublicAdjutantActivityStage,
  label: S.String,
  summary: S.String,
  status: S.String,
  publicRef: S.String,
  siteSlug: S.NullOr(S.String),
  siteTitle: S.NullOr(S.String),
  siteUrl: S.NullOr(S.String),
  updatedAt: S.String,
})
export type PublicAdjutantActivityMilestone =
  typeof PublicAdjutantActivityMilestone.Type

export const PublicAdjutantDeployedSite = S.Struct({
  slug: S.String,
  title: S.String,
  url: S.String,
  status: S.String,
  publicRef: S.String,
  updatedAt: S.String,
})
export type PublicAdjutantDeployedSite = typeof PublicAdjutantDeployedSite.Type

export const PublicAdjutantActivity = S.Struct({
  milestones: S.Array(PublicAdjutantActivityMilestone),
  deployedSites: S.Array(PublicAdjutantDeployedSite),
})
export type PublicAdjutantActivity = typeof PublicAdjutantActivity.Type

export const ShareProjectionResponse = S.Struct({
  projection: ShareProjectionV1,
})
export type ShareProjectionResponse = typeof ShareProjectionResponse.Type

export const PublicRecentPylon = S.Struct({
  nodeLabel: S.NullOr(S.String),
  nostrPubkeyShort: S.String,
  clientVersion: S.NullOr(S.String),
  readyModel: S.NullOr(S.String),
  runtimeState: S.NullOr(S.String),
  lastSeenAtUnixMs: S.NullOr(S.Int),
  lastSeenAtLabel: S.NullOr(S.String),
  eligibleProductCount: S.Int,
  relayUrls: S.Array(S.String),
  products: S.Array(S.String),
})
export type PublicRecentPylon = typeof PublicRecentPylon.Type

export const PublicPylonEarningLaunchGate = S.Struct({
  gateRef: S.String,
  state: S.Literals(['blocked', 'ready']),
  stateLabel: S.String,
  publicEarningCopyAllowed: S.Boolean,
  requiredOnlinePylonsPresent: S.Boolean,
  requiredWalletReadyPylonsPresent: S.Boolean,
  requiredAssignmentReadyPylonsPresent: S.Boolean,
  blockerRefs: S.Array(S.String),
  blockedClaimRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type PublicPylonEarningLaunchGate =
  typeof PublicPylonEarningLaunchGate.Type

export const PublicPylonAcceptedWorkSettlementGate = S.Struct({
  gateRef: S.String,
  state: S.Literals(['blocked', 'ready', 'unavailable']),
  stateLabel: S.String,
  publicPaidWorkTotalsAllowed: S.Boolean,
  receiptBackedTotalsAvailable: S.Boolean,
  settledReceiptRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type PublicPylonAcceptedWorkSettlementGate =
  typeof PublicPylonAcceptedWorkSettlementGate.Type

export const PublicPylonStats = S.Struct({
  available: S.Boolean,
  status: S.Literals(['live', 'unavailable']),
  error: S.NullOr(S.String),
  sourceUrl: S.String,
  hostedNexusRelayUrl: S.NullOr(S.String),
  asOfUnixMs: S.NullOr(S.Int),
  asOfLabel: S.NullOr(S.String),
  minimumClientVersion: S.String,
  pylonsOnlineNow: S.Int,
  pylonsSeen24h: S.Int,
  pylonsRegisteredTotal: S.Int,
  pylonsWalletReadyNow: S.Int,
  pylonsAssignmentReadyNow: S.Int,
  pylonsByResourceMode: S.Record(S.String, S.Int),
  pylonsByClientVersion: S.Record(S.String, S.Int),
  pylonSessionsOnlineNow: S.Int,
  sellablePylonsOnlineNow: S.Int,
  nexusPayoutSatsPaidTotal: S.NullOr(S.Int),
  nexusAcceptedWorkPayoutSatsPaidTotal: S.NullOr(S.Int),
  nexusAcceptedWorkPayoutSatsPaid24h: S.NullOr(S.Int),
  nexusAcceptedWorkPayoutReceiptRefs: S.Array(S.String),
  nexusAcceptedWorkSettlementGate: PublicPylonAcceptedWorkSettlementGate,
  trainingAssignedContributors: S.Int,
  trainingAcceptedContributors: S.Int,
  trainingModelProgressContributors: S.Int,
  recentPylons: S.Array(PublicRecentPylon),
  earningLaunchGate: PublicPylonEarningLaunchGate,
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type PublicPylonStats = typeof PublicPylonStats.Type

export const PublicArtanisReportLoopState = S.Literals([
  'blocked',
  'completed',
  'failed',
  'paused',
  'queued',
  'running',
  'waiting_for_approval',
])
export type PublicArtanisReportLoopState =
  typeof PublicArtanisReportLoopState.Type

export const PublicArtanisReportReadiness = S.Literals([
  'blocked',
  'missing_evidence',
  'partial',
  'ready',
])
export type PublicArtanisReportReadiness =
  typeof PublicArtanisReportReadiness.Type

export const PublicArtanisReportClaimState = S.Literals([
  'blocked',
  'planned',
  'modeled',
  'measured',
  'prohibited',
  'verified',
  'settled',
])
export type PublicArtanisReportClaimState =
  typeof PublicArtanisReportClaimState.Type

export const PublicArtanisReportForumLink = S.Struct({
  description: S.String,
  href: S.String,
  label: S.String,
  topicRef: S.String,
})
export type PublicArtanisReportForumLink =
  typeof PublicArtanisReportForumLink.Type

export const PublicArtanisReportLoopSummary = S.Struct({
  active: S.Boolean,
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  forumPublicationIntentRefs: S.Array(S.String),
  latestTickRef: S.NullOr(S.String),
  latestTickState: S.NullOr(PublicArtanisReportLoopState),
  loopRef: S.String,
  nextTickDisplay: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  state: PublicArtanisReportLoopState,
  tickCount: S.Number,
})
export type PublicArtanisReportLoopSummary =
  typeof PublicArtanisReportLoopSummary.Type

export const PublicArtanisReportPylonSummary = S.Struct({
  acceptedWorkBitcoin24h: S.String,
  acceptedWorkSettlementGate: PublicPylonAcceptedWorkSettlementGate,
  acceptedWorkSettlementReceiptRefs: S.Array(S.String),
  acceptedWorkBitcoinTotal: S.String,
  asOfDisplay: S.NullOr(S.String),
  assignmentReadyPylonsOnlineNow: S.Number,
  earningLaunchGate: PublicPylonEarningLaunchGate,
  feedStatus: S.String,
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
})
export type PublicArtanisReportPylonSummary =
  typeof PublicArtanisReportPylonSummary.Type

export const PublicArtanisPylonLaunchCommunication = S.Struct({
  agentRef: S.String,
  artanisPageRefs: S.Array(S.String),
  authorityBoundaryRefs: S.Array(S.String),
  briefMarkdown: S.String,
  capabilityRefs: S.Array(S.String),
  docsPageRefs: S.Array(S.String),
  forumIntentReady: S.Boolean,
  forumIntentRef: S.String,
  forumPostBody: S.String,
  forumPostTitle: S.String,
  launchPackageRef: S.String,
  optionalSocialCopy: S.String,
  ownerSetupRefs: S.Array(S.String),
  primaryForumTopicRef: S.String,
  primaryForumTopicUrl: S.String,
  readinessRef: S.String,
  readinessStageRefs: S.Array(S.String),
  resourceModeCaveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  stageSummaryRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
})
export type PublicArtanisPylonLaunchCommunication =
  typeof PublicArtanisPylonLaunchCommunication.Type

export const PublicMdkPayoutModeGate = S.Struct({
  activeMode: S.Literals([
    'disabled',
    'hosted_mdk_direct_payout',
    'local_mdk_agent_wallet_bridge',
  ]),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  hostedDirectPayoutClaimAllowed: S.Boolean,
  localBridgePayoutClaimAllowed: S.Boolean,
  livePayoutClaimAllowed: S.Boolean,
  modeLabel: S.String,
  state: S.Literals(['blocked', 'ready', 'sandbox_ready']),
})
export type PublicMdkPayoutModeGate = typeof PublicMdkPayoutModeGate.Type

export const PublicPylonV02OmegaReleaseGate = S.Struct({
  agentRef: S.String,
  audience: S.String,
  blockerRefs: S.Array(S.String),
  canAnnouncePylonV02AcceptedWork: S.Boolean,
  canAnnouncePylonV02Payments: S.Boolean,
  canAnnouncePylonV02Release: S.Boolean,
  canAnnouncePylonV02Settlement: S.Boolean,
  checkCount: S.Number,
  checkRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  failedOrPendingRequiredCount: S.Number,
  gateRef: S.String,
  hostedMdkDirectPayoutClaimAllowed: S.Boolean,
  missingRequiredCheckRefs: S.Array(S.String),
  multiPylonObservedDistinctPylonCount: S.Number,
  multiPylonObservedPylonRefs: S.Array(S.String),
  multiPylonPaidWorkProofComplete: S.Boolean,
  multiPylonProofRefs: S.Array(S.String),
  multiPylonRequiredDistinctPylonCount: S.Number,
  oldGoogleCloudNexusRequired: S.Boolean,
  optionalTransitionEvidenceRefs: S.Array(S.String),
  payoutModeGate: PublicMdkPayoutModeGate,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  releaseCreationAllowedByThisRecord: S.Boolean,
  releasePublicationAllowed: S.Boolean,
  releaseRef: S.String,
  requiredCheckCount: S.Number,
  requiredPassedCount: S.Number,
  runbookRefs: S.Array(S.String),
  settlementMutationAllowed: S.Boolean,
  stageSummaryRefs: S.Array(S.String),
  state: S.Literals([
    'blocked',
    'limited_launcher_release_shipped',
    'ready_for_operator_release_review',
  ]),
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  walletSpendAllowed: S.Boolean,
})
export type PublicPylonV02OmegaReleaseGate =
  typeof PublicPylonV02OmegaReleaseGate.Type

export const PublicArtanisProductionLaunchGate = S.Struct({
  agentRef: S.String,
  blockerRefs: S.Array(S.String),
  canClaimContinuouslyRunning: S.Boolean,
  checkCount: S.Number,
  checkRefs: S.Array(S.String),
  docsRefs: S.Array(S.String),
  enableCommandRefs: S.Array(S.String),
  environmentRef: S.String,
  failedOrPendingRequiredCount: S.Number,
  gateRef: S.String,
  publicBlockedClaimPhrases: S.Array(S.String),
  publicSafeClaimPhrases: S.Array(S.String),
  requiredIssueRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  runbookCommandRefs: S.Array(S.String),
  state: S.Literals(['blocked', 'ready']),
  stateLabel: S.String,
  testRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  verificationTargetRefs: S.Array(S.String),
})
export type PublicArtanisProductionLaunchGate =
  typeof PublicArtanisProductionLaunchGate.Type

export const PublicArtanisForumRewardVisibility = S.Struct({
  acceptedContributionBridgeRefs: S.Array(S.String),
  acceptedContributionCount: S.Number,
  acceptedWorkPayoutClaimAllowed: S.Boolean,
  acceptedWorkProofRefs: S.Array(S.String),
  agentRef: S.String,
  audience: S.String,
  authority: S.Struct({
    noAcceptedWorkPayoutMutation: S.Boolean,
    noForumReceiptMutation: S.Boolean,
    noLiveWalletSpend: S.Boolean,
    noSettlementMutation: S.Boolean,
  }),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  contentRewardCount: S.Number,
  earningActorRefs: S.Array(S.String),
  forumReceiptRefs: S.Array(S.String),
  liveWalletSpendAllowed: S.Boolean,
  paidActionRefs: S.Array(S.String),
  postRewardRefs: S.Array(S.String),
  publicCopyRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spendCapRefs: S.Array(S.String),
  state: S.String,
  stateLabel: S.String,
  summaryRef: S.String,
  topicBoostRefs: S.Array(S.String),
  topicFundRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  walletAuthorityRefs: S.Array(S.String),
})
export type PublicArtanisForumRewardVisibility =
  typeof PublicArtanisForumRewardVisibility.Type

export const PublicArtanisForumRewardSmokeExchange = S.Struct({
  amountAsset: S.String,
  amountValue: S.Number,
  earningNotificationRef: S.String,
  fromAgentRef: S.String,
  postRef: S.String,
  previewChallengeRef: S.String,
  receiptProjectionRef: S.String,
  receiptRef: S.String,
  toAgentRef: S.String,
})
export type PublicArtanisForumRewardSmokeExchange =
  typeof PublicArtanisForumRewardSmokeExchange.Type

export const PublicArtanisForumRewardSmoke = S.Struct({
  acceptedContributionBoundaryRefs: S.Array(S.String),
  acceptedWorkPayoutClaimAllowed: S.Boolean,
  acceptedWorkPayoutRefs: S.Array(S.String),
  agentRef: S.String,
  audience: S.String,
  authority: S.Struct({
    noAcceptedWorkPayoutMutation: S.Boolean,
    noForumReceiptMutation: S.Boolean,
    noProviderSettlementMutation: S.Boolean,
    noWalletSpendExecution: S.Boolean,
  }),
  caveatRefs: S.Array(S.String),
  exchangeCount: S.Number,
  exchangeRecords: S.Array(PublicArtanisForumRewardSmokeExchange),
  mode: S.String,
  modeLabel: S.String,
  namedWalletRefs: S.Array(S.String),
  providerSettlementClaimAllowed: S.Boolean,
  providerSettlementRefs: S.Array(S.String),
  receiptProjectionRefs: S.Array(S.String),
  registeredAgentRefs: S.Array(S.String),
  runReasonRefs: S.Array(S.String),
  smokeRef: S.String,
  sourceRefs: S.Array(S.String),
  spendCapRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  usedLiveBitcoin: S.Boolean,
  walletAuthorityRefs: S.Array(S.String),
})
export type PublicArtanisForumRewardSmoke =
  typeof PublicArtanisForumRewardSmoke.Type

export const PublicArtanisReportModelLabSummary = S.Struct({
  blockerRefs: S.Array(S.String),
  claimState: S.NullOr(S.String),
  completeSectionCount: S.Number,
  consumedContractRefs: S.Array(S.String),
  missingContractRefs: S.Array(S.String),
  missingEvidenceRefs: S.Array(S.String),
  publicForumSummaryReportRefs: S.Array(S.String),
  publicPromotionClaimRefs: S.Array(S.String),
  readiness: PublicArtanisReportReadiness,
  reportRef: S.NullOr(S.String),
  sectionCount: S.Number,
  updatedAtDisplay: S.String,
})
export type PublicArtanisReportModelLabSummary =
  typeof PublicArtanisReportModelLabSummary.Type

export const PublicArtanisReportHealthSummary = S.Struct({
  attentionLabels: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  overclaimBlocked: S.Boolean,
  overallState: S.String,
  pendingApprovalCount: S.Number,
  publicRecoveryActionRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleOrBlockedSignalCount: S.Number,
  updatedAtDisplay: S.String,
})
export type PublicArtanisReportHealthSummary =
  typeof PublicArtanisReportHealthSummary.Type

export const PublicArtanisReportClaimSummary = S.Struct({
  area: S.String,
  blockedByRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimRef: S.String,
  description: S.String,
  evidenceRefs: S.Array(S.String),
  label: S.String,
  state: PublicArtanisReportClaimState,
  stateLabel: S.String,
})
export type PublicArtanisReportClaimSummary =
  typeof PublicArtanisReportClaimSummary.Type

export const PublicArtanisReportStateCaveat = S.Struct({
  caveats: S.Array(S.String),
  description: S.String,
  label: S.String,
  state: PublicArtanisReportClaimState,
})
export type PublicArtanisReportStateCaveat =
  typeof PublicArtanisReportStateCaveat.Type

export const PublicArtanisReport = S.Struct({
  agentId: S.String,
  agentRef: S.String,
  artifactRefs: S.Array(S.String),
  autonomousLoop: PublicArtanisReportLoopSummary,
  campaignRef: S.String,
  claimStateCaveats: S.Array(PublicArtanisReportStateCaveat),
  displayName: S.String,
  forumLinks: S.Array(PublicArtanisReportForumLink),
  forumRewardSmoke: PublicArtanisForumRewardSmoke,
  forumRewardVisibility: PublicArtanisForumRewardVisibility,
  healthSummary: PublicArtanisReportHealthSummary,
  modelLabSummary: PublicArtanisReportModelLabSummary,
  nexusPublicRefs: S.Array(S.String),
  publicBlockerRefs: S.Array(S.String),
  publicCaveatRefs: S.Array(S.String),
  publicGoalRefs: S.Array(S.String),
  publicUrls: S.Array(S.String),
  pylonOmegaReleaseGate: PublicPylonV02OmegaReleaseGate,
  pylonLaunchCommunication: PublicArtanisPylonLaunchCommunication,
  productionLaunchGate: PublicArtanisProductionLaunchGate,
  pylonSummary: PublicArtanisReportPylonSummary,
  r10Claims: S.Array(PublicArtanisReportClaimSummary),
  receiptRefs: S.Array(S.String),
  reportRef: S.String,
  runtimeState: S.String,
  standaloneClaims: S.Array(PublicArtanisReportClaimSummary),
  updatedAtDisplay: S.String,
})
export type PublicArtanisReport = typeof PublicArtanisReport.Type

export const IdlePublicAgent = ts('PublicAgentIdle', {})
export const LoadingPublicAgent = ts('PublicAgentLoading', {
  agentRef: S.String,
})
export const LoadedPublicAgent = ts('PublicAgentLoaded', {
  agentRef: S.String,
  response: PublicAgentGoalResponse,
})
export const FailedPublicAgent = ts('PublicAgentFailed', {
  agentRef: S.String,
  error: S.String,
})
export const PublicAgentModel = S.Union([
  IdlePublicAgent,
  LoadingPublicAgent,
  LoadedPublicAgent,
  FailedPublicAgent,
])
export type PublicAgentModel = typeof PublicAgentModel.Type

export const IdlePublicPylonStats = ts('PublicPylonStatsIdle', {})
export const LoadingPublicPylonStats = ts('PublicPylonStatsLoading', {})
export const LoadedPublicPylonStats = ts('PublicPylonStatsLoaded', {
  stats: PublicPylonStats,
})
export const FailedPublicPylonStats = ts('PublicPylonStatsFailed', {
  error: S.String,
})
export const PublicPylonStatsModel = S.Union([
  IdlePublicPylonStats,
  LoadingPublicPylonStats,
  LoadedPublicPylonStats,
  FailedPublicPylonStats,
])
export type PublicPylonStatsModel = typeof PublicPylonStatsModel.Type

export const IdlePublicArtanisReport = ts('PublicArtanisReportIdle', {})
export const LoadingPublicArtanisReport = ts('PublicArtanisReportLoading', {})
export const LoadedPublicArtanisReport = ts('PublicArtanisReportLoaded', {
  report: PublicArtanisReport,
})
export const FailedPublicArtanisReport = ts('PublicArtanisReportFailed', {
  error: S.String,
})
export const PublicArtanisReportModel = S.Union([
  IdlePublicArtanisReport,
  LoadingPublicArtanisReport,
  LoadedPublicArtanisReport,
  FailedPublicArtanisReport,
])
export type PublicArtanisReportModel = typeof PublicArtanisReportModel.Type

export const IdlePublicAdjutantActivity = ts('PublicAdjutantActivityIdle', {})
export const LoadingPublicAdjutantActivity = ts(
  'PublicAdjutantActivityLoading',
  {},
)
export const LoadedPublicAdjutantActivity = ts('PublicAdjutantActivityLoaded', {
  activity: PublicAdjutantActivity,
})
export const FailedPublicAdjutantActivity = ts('PublicAdjutantActivityFailed', {
  error: S.String,
})
export const PublicAdjutantActivityModel = S.Union([
  IdlePublicAdjutantActivity,
  LoadingPublicAdjutantActivity,
  LoadedPublicAdjutantActivity,
  FailedPublicAdjutantActivity,
])
export type PublicAdjutantActivityModel =
  typeof PublicAdjutantActivityModel.Type

export const IdleShareProjection = ts('ShareProjectionIdle', {})
export const LoadingShareProjection = ts('ShareProjectionLoading', {
  shareId: S.String,
})
export const LoadedShareProjection = ts('ShareProjectionLoaded', {
  projection: ShareProjectionV1,
})
export const FailedShareProjection = ts('ShareProjectionFailed', {
  shareId: S.String,
  error: S.String,
  status: S.Int,
})
export const ShareProjectionModel = S.Union([
  IdleShareProjection,
  LoadingShareProjection,
  LoadedShareProjection,
  FailedShareProjection,
])
export type ShareProjectionModel = typeof ShareProjectionModel.Type

export const Model = ts('LoggedOut', {
  route: LoggedOutRoute,
  onboarding: OnboardingModel,
  publicAgent: PublicAgentModel,
  publicArtanisReport: PublicArtanisReportModel,
  publicAdjutantActivity: PublicAdjutantActivityModel,
  publicPylonStats: PublicPylonStatsModel,
  shareProjection: ShareProjectionModel,
})

export type Model = typeof Model.Type

// INIT

export const init = (route: LoggedOutRoute): Model =>
  Model({
    route,
    onboarding: initOnboardingModel(),
    publicAgent:
      route._tag === 'PublicAgent'
        ? LoadingPublicAgent({ agentRef: route.agentRef })
        : IdlePublicAgent(),
    publicArtanisReport:
      route._tag === 'PublicAgent' && route.agentRef === 'artanis'
        ? LoadingPublicArtanisReport()
        : IdlePublicArtanisReport(),
    publicAdjutantActivity:
      route._tag === 'PublicAgent' && route.agentRef === 'adjutant'
        ? LoadingPublicAdjutantActivity()
        : IdlePublicAdjutantActivity(),
    publicPylonStats:
      route._tag === 'Home' || route._tag === 'PublicAgent'
        ? LoadingPublicPylonStats()
        : IdlePublicPylonStats(),
    shareProjection:
      route._tag === 'Share'
        ? LoadingShareProjection({ shareId: route.shareId })
        : IdleShareProjection(),
  })

export const initOnboardingModel = (): OnboardingModel =>
  OnboardingModel({
    couponCode: '',
    fundingAmount: 25,
    isCouponOpen: false,
    selectedRepository: 'openagents/autopilot-omega',
    step: 'github',
  })
