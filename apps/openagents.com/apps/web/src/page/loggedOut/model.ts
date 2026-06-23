import { ShareProjectionV1 } from '@openagentsinc/sync-schema'
import { Option, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import { LoggedOutRoute } from '../../route'
import { FlowModel, initFlowModel } from '../autopilot-onboarding/flow'

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
  pylonRef: S.optionalKey(S.NullOr(S.String)),
  ownerAgentRef: S.optionalKey(S.NullOr(S.String)),
  nodeLabel: S.NullOr(S.String),
  nostrPubkeyShort: S.String,
  clientVersion: S.NullOr(S.String),
  readyModel: S.NullOr(S.String),
  runtimeState: S.NullOr(S.String),
  lastSeenAtUnixMs: S.NullOr(S.Int),
  lastSeenAtLabel: S.NullOr(S.String),
  lastHeartbeatAgeSeconds: S.optionalKey(S.NullOr(S.Int)),
  onlineNow: S.optionalKey(S.NullOr(S.Boolean)),
  walletReadyNow: S.optionalKey(S.NullOr(S.Boolean)),
  assignmentReadyNow: S.optionalKey(S.NullOr(S.Boolean)),
  tippingAvailable: S.optionalKey(S.NullOr(S.Boolean)),
  tipEndpoint: S.optionalKey(S.NullOr(S.String)),
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
  treasuryPayoutSatsPaidTotal: S.optionalKey(S.NullOr(S.Int)),
  treasuryPayoutSatsPaid24h: S.optionalKey(S.NullOr(S.Int)),
  treasuryPayoutCountTotal: S.optionalKey(S.NullOr(S.Int)),
  treasuryPayoutCount24h: S.optionalKey(S.NullOr(S.Int)),
  publicRealSatsSettledTotal: S.optionalKey(S.NullOr(S.Int)),
  publicRealSatsSettled24h: S.optionalKey(S.NullOr(S.Int)),
  trainingAssignedContributors: S.Int,
  trainingAcceptedContributors: S.Int,
  trainingModelProgressContributors: S.Int,
  recentPylons: S.Array(PublicRecentPylon),
  earningLaunchGate: PublicPylonEarningLaunchGate,
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type PublicPylonStats = typeof PublicPylonStats.Type

export const PublicForumLaunchGateState = S.Literals([
  'blocked',
  'degraded',
  'gated',
  'planned',
  'ready',
])
export type PublicForumLaunchGateState = typeof PublicForumLaunchGateState.Type

export const PublicForumLaunchGateSeverity = S.Literals([
  'required',
  'recommended',
])
export type PublicForumLaunchGateSeverity =
  typeof PublicForumLaunchGateSeverity.Type

export const PublicForumLaunchGate = S.Struct({
  id: S.String,
  label: S.String,
  severity: PublicForumLaunchGateSeverity,
  state: PublicForumLaunchGateState,
  summary: S.String,
})
export type PublicForumLaunchGate = typeof PublicForumLaunchGate.Type

export const PublicForumTipPayerWalletReadiness = S.Struct({
  actorRef: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  configuredRefs: S.Array(S.String),
  fundedRefs: S.Array(S.String),
  sendReadyRefs: S.Array(S.String),
  sourceRef: S.String,
  state: S.Literals(['missing', 'configured', 'funded', 'send_ready']),
  tippingSpendAllowed: S.Boolean,
})
export type PublicForumTipPayerWalletReadiness =
  typeof PublicForumTipPayerWalletReadiness.Type

export const PublicForumLaunchStatus = S.Struct({
  gates: S.Array(PublicForumLaunchGate),
  orangeChecksSold: S.optionalKey(S.NullOr(S.Number)),
  publicPosting: S.Struct({
    listedForums: PublicForumLaunchGateState,
    voidLane: PublicForumLaunchGateState,
  }),
  publicTipping: S.Struct({
    gates: S.Array(PublicForumLaunchGate),
    onboarding: S.optionalKey(
      S.Struct({
        payerReadiness: PublicForumTipPayerWalletReadiness,
        publicCopyRefs: S.Array(S.String),
        recipientStateRefs: S.Array(S.String),
        settlementStateRefs: S.Array(S.String),
      }),
    ),
    postTips: PublicForumLaunchGateState,
    remainingBeforeLiveTips: S.Array(S.String),
    summary: S.String,
  }),
  remainingBeforeBroadLaunch: S.Array(S.String),
  status: PublicForumLaunchGateState,
  summary: S.String,
  updatedAt: S.String,
})
export type PublicForumLaunchStatus = typeof PublicForumLaunchStatus.Type

export const PublicForumActorSummary = S.Struct({
  actorId: S.String,
  actorRef: S.String,
  displayName: S.String,
  groupRefs: S.Array(S.String),
  isAgent: S.Boolean,
  slug: S.String,
})
export type PublicForumActorSummary = typeof PublicForumActorSummary.Type

export const PublicForumTipLeaderboardPost = S.Struct({
  author: PublicForumActorSummary,
  postId: S.String,
  postPermalink: S.String,
  tipCount: S.Number,
  topicId: S.String,
  totalPaidSats: S.Number,
  totalSettledSats: S.Number,
})
export type PublicForumTipLeaderboardPost =
  typeof PublicForumTipLeaderboardPost.Type

export const PublicForumTipLeaderboardCreator = S.Struct({
  actor: PublicForumActorSummary,
  tipCount: S.Number,
  totalPaidSats: S.Number,
  totalSettledSats: S.Number,
})
export type PublicForumTipLeaderboardCreator =
  typeof PublicForumTipLeaderboardCreator.Type

export const PublicForumTipLeaderboards = S.Struct({
  creators: S.Array(PublicForumTipLeaderboardCreator),
  generatedAt: S.String,
  posts: S.Array(PublicForumTipLeaderboardPost),
})
export type PublicForumTipLeaderboards = typeof PublicForumTipLeaderboards.Type

export const PublicProductPromiseState = S.Literals([
  'degraded',
  'green',
  'planned',
  'red',
  'withdrawn',
  'yellow',
])
export type PublicProductPromiseState = typeof PublicProductPromiseState.Type

export const PublicProductPromise = S.Struct({
  audience: S.Array(S.String),
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  claim: S.String,
  evidenceRefs: S.Array(S.String),
  productArea: S.String,
  promiseId: S.String,
  reportPath: S.String,
  safeCopy: S.String,
  sourceRefs: S.Array(S.String),
  state: PublicProductPromiseState,
  unsafeCopy: S.String,
  verification: S.String,
})
export type PublicProductPromise = typeof PublicProductPromise.Type

export const PublicProductPromiseBlockedSummary = S.Struct({
  blockerRefs: S.Array(S.String),
  promiseId: S.String,
  state: S.String,
})
export type PublicProductPromiseBlockedSummary =
  typeof PublicProductPromiseBlockedSummary.Type

export const PublicProductPromisesVerificationSummary = S.Struct({
  blockedPromiseCount: S.Int,
  evidenceRefCount: S.Int,
  promiseCount: S.Int,
  promisesWithBlockersCount: S.Int,
  topBlockedPromises: S.Array(PublicProductPromiseBlockedSummary),
  uniqueBlockerCount: S.Int,
  uniqueBlockers: S.Array(S.String),
})
export type PublicProductPromisesVerificationSummary =
  typeof PublicProductPromisesVerificationSummary.Type

export const PublicProductPromises = S.Struct({
  canonicalDocsUrl: S.String,
  currentMonorepoStatus: S.Struct({
    caveats: S.Array(S.String),
    liveDeploymentRefs: S.Array(S.String),
    pylonV03Refs: S.Array(S.String),
    status: S.String,
    summary: S.String,
  }),
  latestGapAuditUrl: S.String,
  lastUpdated: S.String,
  notes: S.Array(S.String),
  promises: S.Array(PublicProductPromise),
  publicDocsUrl: S.String,
  reportPath: S.Struct({
    defaultForumUrl: S.String,
    forumSlug: S.String,
    forumTopicApi: S.String,
    rule: S.String,
    strictBugForm: S.String,
  }),
  schemaVersion: S.String,
  sourceRefs: S.Array(S.String),
  states: S.Record(S.String, S.String),
  verificationSummary: PublicProductPromisesVerificationSummary,
  version: S.String,
})
export type PublicProductPromises = typeof PublicProductPromises.Type

export const PublicTrainingRunMetric = S.Struct({
  provenanceLabel: S.String,
  sourceRefs: S.Array(S.String),
  value: S.Number,
})
export type PublicTrainingRunMetric = typeof PublicTrainingRunMetric.Type

export const PublicTrainingRunLossPoint = S.Struct({
  provenanceLabel: S.String,
  sourceRefs: S.Array(S.String),
  step: S.Number,
  validationLoss: S.Number,
})
export type PublicTrainingRunLossPoint = typeof PublicTrainingRunLossPoint.Type

export const PublicTrainingRunLeaderboardRow = S.Struct({
  bestValidationLoss: S.NullOr(S.Number),
  provenanceLabel: S.String,
  pylonRef: S.String,
  rank: S.Number,
  settledPayoutSats: S.Number,
  sourceRefs: S.Array(S.String),
  trainingRunRef: S.String,
  verifiedWindowCount: S.Number,
})
export type PublicTrainingRunLeaderboardRow =
  typeof PublicTrainingRunLeaderboardRow.Type

export const PublicTrainingRunRealGradientStatus = S.Struct({
  closeoutRequirement: S.Struct({
    evalRef: S.NullOr(S.String),
    freivaldsCommitmentRefs: S.Array(S.String),
    gradientCloseoutRefs: S.Array(S.String),
    mergeRef: S.NullOr(S.String),
    provenanceLabel: S.String,
    satisfied: S.Boolean,
  }),
  deviceRequirement: S.Struct({
    observedDistinctContributorDevices: S.Number,
    provenanceLabel: S.String,
    requiredDistinctContributorDevices: S.Number,
    satisfied: S.Boolean,
    sourceRefs: S.Array(S.String),
  }),
  externalAsk: S.Struct({
    blockerRefs: S.Array(S.String),
    psionicLaneRef: S.String,
    requirementRefs: S.Array(S.String),
    status: S.Literals(['blocked_external', 'ready', 'observed']),
  }),
  leaderboardRows: S.Array(PublicTrainingRunLeaderboardRow),
  lossCurve: S.Array(PublicTrainingRunLossPoint),
  lossUnderBudget: S.Struct({
    budgetLabel: S.String,
    budgetRef: S.NullOr(S.String),
    finalValidationLoss: S.NullOr(S.Number),
    maxValidationLoss: S.NullOr(S.Number),
    provenanceLabel: S.String,
    satisfied: S.Boolean,
    sourceRefs: S.Array(S.String),
  }),
  scopeBoundaryRefs: S.Array(S.String),
})
export type PublicTrainingRunRealGradientStatus =
  typeof PublicTrainingRunRealGradientStatus.Type

export const PublicTrainingRunProjection = S.Struct({
  createdAtDisplay: S.String,
  promiseRef: S.String,
  receiptRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: S.Literals(['planned', 'active', 'sealed', 'reconciled']),
  trainingRunRef: S.String,
  updatedAtDisplay: S.String,
})
export type PublicTrainingRunProjection =
  typeof PublicTrainingRunProjection.Type

export const PublicTrainingWindowProjection = S.Struct({
  datasetRefs: S.Array(S.String),
  homeworkKind: S.Literals([
    'admin_dispatched_homework',
    'operator_planned_homework',
    'auto_starter',
  ]),
  plannedAtDisplay: S.String,
  priority: S.Number,
  receiptRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: S.Literals(['planned', 'active', 'sealed', 'reconciled']),
  trainingRunRef: S.String,
  updatedAtDisplay: S.String,
  windowRef: S.String,
})
export type PublicTrainingWindowProjection =
  typeof PublicTrainingWindowProjection.Type

export const PublicTrainingRunSummary = S.Struct({
  copyBoundaryRefs: S.Array(S.String),
  emptyState: S.Struct({
    idle: S.Boolean,
    reason: S.String,
  }),
  metrics: S.Struct({
    activeWindowCount: PublicTrainingRunMetric,
    assignedContributorCount: PublicTrainingRunMetric,
    pendingPayoutCount: PublicTrainingRunMetric,
    plannedWindowCount: PublicTrainingRunMetric,
    providerConfirmedSettledPayoutSats: PublicTrainingRunMetric,
    receiptRefCount: PublicTrainingRunMetric,
    reconciledWindowCount: PublicTrainingRunMetric,
    rejectedWorkCount: PublicTrainingRunMetric,
    sealedWindowCount: PublicTrainingRunMetric,
    verifiedWorkCount: PublicTrainingRunMetric,
  }),
  realGradient: PublicTrainingRunRealGradientStatus,
  receiptRefs: S.Array(S.String),
  run: PublicTrainingRunProjection,
  sourceRefs: S.Array(S.String),
  windows: S.Array(PublicTrainingWindowProjection),
})
export type PublicTrainingRunSummary = typeof PublicTrainingRunSummary.Type

export const PublicTrainingRunsResponse = S.Struct({
  runs: S.Array(PublicTrainingRunProjection),
  summaries: S.Array(PublicTrainingRunSummary),
})
export type PublicTrainingRunsResponse = typeof PublicTrainingRunsResponse.Type

export const PublicTrainingRunResponse = S.Struct({
  run: PublicTrainingRunProjection,
  summary: PublicTrainingRunSummary,
})
export type PublicTrainingRunResponse = typeof PublicTrainingRunResponse.Type

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

export const IdlePublicForumLaunchStatus = ts('PublicForumLaunchStatusIdle', {})
export const LoadingPublicForumLaunchStatus = ts(
  'PublicForumLaunchStatusLoading',
  {},
)
export const LoadedPublicForumLaunchStatus = ts(
  'PublicForumLaunchStatusLoaded',
  {
    status: PublicForumLaunchStatus,
  },
)
export const FailedPublicForumLaunchStatus = ts(
  'PublicForumLaunchStatusFailed',
  {
    error: S.String,
  },
)
export const PublicForumLaunchStatusModel = S.Union([
  IdlePublicForumLaunchStatus,
  LoadingPublicForumLaunchStatus,
  LoadedPublicForumLaunchStatus,
  FailedPublicForumLaunchStatus,
])
export type PublicForumLaunchStatusModel =
  typeof PublicForumLaunchStatusModel.Type

export const IdlePublicForumTipLeaderboards = ts(
  'PublicForumTipLeaderboardsIdle',
  {},
)
export const LoadingPublicForumTipLeaderboards = ts(
  'PublicForumTipLeaderboardsLoading',
  {},
)
export const LoadedPublicForumTipLeaderboards = ts(
  'PublicForumTipLeaderboardsLoaded',
  {
    leaderboards: PublicForumTipLeaderboards,
  },
)
export const FailedPublicForumTipLeaderboards = ts(
  'PublicForumTipLeaderboardsFailed',
  {
    error: S.String,
  },
)
export const PublicForumTipLeaderboardsModel = S.Union([
  IdlePublicForumTipLeaderboards,
  LoadingPublicForumTipLeaderboards,
  LoadedPublicForumTipLeaderboards,
  FailedPublicForumTipLeaderboards,
])
export type PublicForumTipLeaderboardsModel =
  typeof PublicForumTipLeaderboardsModel.Type

export const IdlePublicProductPromises = ts('PublicProductPromisesIdle', {})
export const LoadingPublicProductPromises = ts(
  'PublicProductPromisesLoading',
  {},
)
export const LoadedPublicProductPromises = ts('PublicProductPromisesLoaded', {
  promises: PublicProductPromises,
})
export const FailedPublicProductPromises = ts('PublicProductPromisesFailed', {
  error: S.String,
})
export const PublicProductPromisesModel = S.Union([
  IdlePublicProductPromises,
  LoadingPublicProductPromises,
  LoadedPublicProductPromises,
  FailedPublicProductPromises,
])
export type PublicProductPromisesModel = typeof PublicProductPromisesModel.Type

// Claim-upgrade audit panel: promise-transition receipts from
// /api/public/product-promises/transitions. Each receipt is the
// dereferenceable, registry-versioned proof for one proposed state flip.
export const PublicPromiseTransitionCheck = S.Struct({
  kind: S.String,
  result: S.String,
})
export type PublicPromiseTransitionCheck =
  typeof PublicPromiseTransitionCheck.Type

export const PublicPromiseTransitionException = S.Struct({
  approvedByRef: S.String,
  expiresAt: S.String,
  reasonRef: S.String,
})
export type PublicPromiseTransitionException =
  typeof PublicPromiseTransitionException.Type

export const PublicPromiseTransitionReceipt = S.Struct({
  checkedAt: S.String,
  checks: S.Array(PublicPromiseTransitionCheck),
  evidenceRefs: S.Array(S.String),
  exception: S.NullOr(PublicPromiseTransitionException),
  fromState: S.String,
  promiseId: S.String,
  receiptId: S.String,
  registryVersion: S.String,
  result: S.String,
  toState: S.String,
})
export type PublicPromiseTransitionReceipt =
  typeof PublicPromiseTransitionReceipt.Type

export const PublicPromiseTransitions = S.Struct({
  kind: S.String,
  publicSafe: S.Boolean,
  receipts: S.Array(PublicPromiseTransitionReceipt),
  rule: S.String,
})
export type PublicPromiseTransitions = typeof PublicPromiseTransitions.Type

export const IdlePublicPromiseTransitions = ts(
  'PublicPromiseTransitionsIdle',
  {},
)
export const LoadingPublicPromiseTransitions = ts(
  'PublicPromiseTransitionsLoading',
  {},
)
export const LoadedPublicPromiseTransitions = ts(
  'PublicPromiseTransitionsLoaded',
  {
    transitions: PublicPromiseTransitions,
  },
)
export const FailedPublicPromiseTransitions = ts(
  'PublicPromiseTransitionsFailed',
  {
    error: S.String,
  },
)
export const PublicPromiseTransitionsModel = S.Union([
  IdlePublicPromiseTransitions,
  LoadingPublicPromiseTransitions,
  LoadedPublicPromiseTransitions,
  FailedPublicPromiseTransitions,
])
export type PublicPromiseTransitionsModel =
  typeof PublicPromiseTransitionsModel.Type

export const IdlePublicTrainingRuns = ts('PublicTrainingRunsIdle', {})
export const LoadingPublicTrainingRuns = ts('PublicTrainingRunsLoading', {
  runId: S.NullOr(S.String),
})
export const LoadedPublicTrainingRuns = ts('PublicTrainingRunsLoaded', {
  response: PublicTrainingRunsResponse,
  selectedRunId: S.NullOr(S.String),
})
export const FailedPublicTrainingRuns = ts('PublicTrainingRunsFailed', {
  error: S.String,
  runId: S.NullOr(S.String),
})
export const PublicTrainingRunsModel = S.Union([
  IdlePublicTrainingRuns,
  LoadingPublicTrainingRuns,
  LoadedPublicTrainingRuns,
  FailedPublicTrainingRuns,
])
export type PublicTrainingRunsModel = typeof PublicTrainingRunsModel.Type

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

// Live settled feed (openagents #5311): public-safe settlement events streamed
// over the OpenAgents sync engine so the homepage updates in real-time as real
// Bitcoin settlements stream. Public-safe fields only — refs + integer amounts.
export const PublicSettledFeedEvent = S.Struct({
  amountSats: S.Number,
  challengeRef: S.String,
  contributorRef: S.String,
  eventRef: S.String,
  party: S.Literals(['worker', 'validator']),
  runRef: S.String,
  settledAt: S.String,
  totalSettledCount: S.Number,
  totalSettledSats: S.Number,
  windowRef: S.NullOr(S.String),
})
export type PublicSettledFeedEvent = typeof PublicSettledFeedEvent.Type

export const SettledFeedConnection = S.Literals([
  'idle',
  'connecting',
  'open',
  'closed',
  'failed',
])
export type SettledFeedConnection = typeof SettledFeedConnection.Type

// The settled feed renders live from the streamed events: a running settled
// total, a settled count, and the latest event. `cursor` tracks the last seq
// applied so reconnects can replay missed changes. When the socket is
// unavailable the homepage still renders the non-realtime totals it already
// fetches; this slice is purely additive live data.
export const SettledFeedModel = ts('LoggedOutSettledFeed', {
  connection: SettledFeedConnection,
  cursor: S.Number,
  events: S.Array(PublicSettledFeedEvent),
  totalSettledCount: S.Number,
  totalSettledSats: S.Number,
})
export type SettledFeedModel = typeof SettledFeedModel.Type

export const initSettledFeedModel = (): SettledFeedModel =>
  SettledFeedModel({
    connection: 'idle',
    cursor: 0,
    events: [],
    totalSettledCount: 0,
    totalSettledSats: 0,
  })

export const Model = ts('LoggedOut', {
  route: LoggedOutRoute,
  onboarding: OnboardingModel,
  // The /autopilot onboarding conversation flow (#6129). Holds the session,
  // transcript, accumulated Output Spec, composer draft, and request status.
  autopilotOnboarding: FlowModel,
  publicAgent: PublicAgentModel,
  publicArtanisReport: PublicArtanisReportModel,
  publicAdjutantActivity: PublicAdjutantActivityModel,
  publicPylonStats: PublicPylonStatsModel,
  publicForumLaunchStatus: PublicForumLaunchStatusModel,
  publicForumTipLeaderboards: PublicForumTipLeaderboardsModel,
  publicProductPromises: PublicProductPromisesModel,
  publicPromiseTransitions: PublicPromiseTransitionsModel,
  publicTrainingRuns: PublicTrainingRunsModel,
  settledFeed: SettledFeedModel,
  shareProjection: ShareProjectionModel,
  // True once the Tassadar "Copy Agent Instructions" button has written to the
  // clipboard, so the button can show a "Copied" affirmation.
  copiedAgentInstructions: S.Boolean,
})

export type Model = typeof Model.Type

// INIT

export const init = (route: LoggedOutRoute): Model =>
  Model({
    route,
    onboarding: initOnboardingModel(),
    autopilotOnboarding: initFlowModel(
      route._tag === 'AutopilotVertical'
        ? Option.some(route.vertical)
        : Option.none(),
    ),
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
      route._tag === 'Home' ||
      route._tag === 'Stats' ||
      route._tag === 'PublicStatsArchive' ||
      route._tag === 'PublicAgent'
        ? LoadingPublicPylonStats()
        : IdlePublicPylonStats(),
    publicForumLaunchStatus:
      route._tag === 'Home' ||
      route._tag === 'Stats' ||
      route._tag === 'PublicStatsArchive'
        ? LoadingPublicForumLaunchStatus()
        : IdlePublicForumLaunchStatus(),
    publicForumTipLeaderboards:
      route._tag === 'Home' ||
      route._tag === 'Stats' ||
      route._tag === 'PublicStatsArchive'
        ? LoadingPublicForumTipLeaderboards()
        : IdlePublicForumTipLeaderboards(),
    publicProductPromises:
      route._tag === 'ProductPromises'
        ? LoadingPublicProductPromises()
        : IdlePublicProductPromises(),
    publicPromiseTransitions:
      route._tag === 'ProductPromises'
        ? LoadingPublicPromiseTransitions()
        : IdlePublicPromiseTransitions(),
    publicTrainingRuns:
      route._tag === 'PublicTrainingRuns'
        ? LoadingPublicTrainingRuns({ runId: null })
        : route._tag === 'PublicTrainingRun'
          ? LoadingPublicTrainingRuns({ runId: route.runId })
          : IdlePublicTrainingRuns(),
    settledFeed: initSettledFeedModel(),
    shareProjection:
      route._tag === 'Share'
        ? LoadingShareProjection({ shareId: route.shareId })
        : IdleShareProjection(),
    copiedAgentInstructions: false,
  })

export const initOnboardingModel = (): OnboardingModel =>
  OnboardingModel({
    couponCode: '',
    fundingAmount: 25,
    isCouponOpen: false,
    selectedRepository: 'OpenAgentsInc/openagents',
    step: 'github',
  })
