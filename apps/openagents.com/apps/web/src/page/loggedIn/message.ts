import {
  CursorGap,
  InferenceAnalyticsResponse,
  SyncPatch,
  SyncSnapshot,
  TokenUsageAggregateResponse,
  TokenUsageLeaderboardPreferenceResponse,
  TokenUsageLeaderboardsResponse,
} from '@openagentsinc/sync-schema'
import { Schema as S } from 'effect'
import { File as FileSchema } from 'foldkit/file'
import { m } from 'foldkit/message'

import {
  BillingCheckoutResponse,
  BillingSetupIntentResponse,
  BillingSummaryResponse,
  OnboardingRepositoriesResponse,
  OnboardingStatusResponse,
  OnboardingStep,
  ProviderDeviceLoginStartResponse,
  ProviderDeviceLoginStatusResponse,
} from '../../domain/session'
import {
  AdminAdjutantAssignmentReviewResponse,
  AdminAdjutantAssignmentsResponse,
  AdminAdjutantEnrichmentActionResponse,
  AdminOverviewResponse,
  AdminSiteDeploymentActionResponse,
  AdminSiteGenerationResponse,
  AgentGoalAction,
  AgentGoalResponse,
  AgentRunDetailResponse,
  AgentRunLaunchResponse,
  ArtanisOperatorApprovalAction,
  ArtanisOperatorConsoleResponse,
  ArtanisOperatorDashboardResponse,
  AutopilotDecisionActionResponse,
  AutopilotDecisionListResponse,
  AutopilotMorningReportResponse,
  AutopilotWorkBriefingResponse,
  AutopilotWorkComposerField,
  AutopilotWorkEventsResponse,
  AutopilotWorkListResponse,
  AutopilotWorkResponse,
  AutopilotWorkReviewAction,
  CustomerFulfillmentArtifactsResponse,
  CustomerOneCohortProjection,
  CustomerOrderResponse,
  CustomerOrdersResponse,
  CustomerSiteBuilderEventsResponse,
  CustomerSiteBuilderFileListResponse,
  CustomerSiteBuilderFileReadResponse,
  CustomerSiteBuilderFileTreeResponse,
  CustomerSiteBuilderSessionResponse,
  CustomerSiteFeedbackResponse,
  CustomerSiteRevisionsResponse,
  GenerateImageResponse,
  ImageGenerationAspectRatio,
  ImageGenerationImageSize,
  ImageGenerationModelId,
  ImageGenerationProvider,
  PrefilledWorkspaceResponse,
  ProAgentDashboardResponse,
  ProviderAccountPoolManualResetResponse,
  ProviderAccountPoolResponse,
  SubmitCustomerSiteFeedbackResponse,
  TeamChatMessagesResponse,
  TeamChatPostResponse,
  ThreadFileDetailResponse,
  TokenUsageStatsFilterKey,
  TokenUsageStatsFilters,
} from './model'
import { ThreadFileUploadResponse, ThreadFilesResponse } from './model'
import { MulletBootstrapResponse } from './mullet/model'
import {
  FailedLoadWorkroomLifecycle,
  FailedLoadWorkroomSurface,
  FailedWorkroomLifecycleDecision,
  RequestedLoadWorkroomLifecycle,
  RequestedLoadWorkroomSurface,
  SelectedWorkroomTab,
  SubmittedWorkroomLifecycleDecision,
  SucceededLoadWorkroomLifecycle,
  SucceededLoadWorkroomSurface,
  SucceededWorkroomLifecycleDecision,
} from './page/workroom'
import { SiteElementContext } from './site-element-context'

export {
  FailedLoadWorkroomLifecycle,
  FailedLoadWorkroomSurface,
  FailedWorkroomLifecycleDecision,
  RequestedLoadWorkroomLifecycle,
  RequestedLoadWorkroomSurface,
  SelectedWorkroomTab,
  SubmittedWorkroomLifecycleDecision,
  SucceededLoadWorkroomLifecycle,
  SucceededLoadWorkroomSurface,
  SucceededWorkroomLifecycleDecision,
}

// MESSAGE

export const ClickedLogout = m('ClickedLogout')
export const ClickedNewChat = m('ClickedNewChat')
export const ClickedRunMetadataInfo = m('ClickedRunMetadataInfo')
export const ClosedRunMetadataInfo = m('ClosedRunMetadataInfo')
export const ClickedStartProviderDeviceLogin = m(
  'ClickedStartProviderDeviceLogin',
  {
    createNew: S.optionalKey(S.Boolean),
    providerAccountRef: S.optionalKey(S.String),
  },
)
export const ClickedPollProviderDeviceLogin = m(
  'ClickedPollProviderDeviceLogin',
  {
    attemptId: S.String,
  },
)
export const SucceededStartProviderDeviceLogin = m(
  'SucceededStartProviderDeviceLogin',
  {
    response: ProviderDeviceLoginStartResponse,
  },
)
export const FailedStartProviderDeviceLogin = m(
  'FailedStartProviderDeviceLogin',
  {
    error: S.String,
  },
)
export const SucceededPollProviderDeviceLogin = m(
  'SucceededPollProviderDeviceLogin',
  {
    response: ProviderDeviceLoginStatusResponse,
  },
)
export const FailedPollProviderDeviceLogin = m(
  'FailedPollProviderDeviceLogin',
  {
    error: S.String,
  },
)
export const RequestedLoadProviderAccountPool = m(
  'RequestedLoadProviderAccountPool',
)
export const SucceededLoadProviderAccountPool = m(
  'SucceededLoadProviderAccountPool',
  {
    response: ProviderAccountPoolResponse,
  },
)
export const FailedLoadProviderAccountPool = m(
  'FailedLoadProviderAccountPool',
  {
    error: S.String,
  },
)
export const ClickedResetProviderAccountPoolAccount = m(
  'ClickedResetProviderAccountPoolAccount',
  {
    providerAccountRef: S.String,
  },
)
export const SucceededResetProviderAccountPoolAccount = m(
  'SucceededResetProviderAccountPoolAccount',
  {
    response: ProviderAccountPoolManualResetResponse,
  },
)
export const FailedResetProviderAccountPoolAccount = m(
  'FailedResetProviderAccountPoolAccount',
  {
    error: S.String,
    providerAccountRef: S.String,
  },
)
export const ClickedBillingPackage = m('ClickedBillingPackage', {
  packageId: S.String,
})
export const UpdatedBillingCouponCode = m('UpdatedBillingCouponCode', {
  value: S.String,
})
export const SubmittedBillingCoupon = m('SubmittedBillingCoupon')
export const ClickedPrepareBillingCardSetup = m(
  'ClickedPrepareBillingCardSetup',
)
export const ClickedEnableBillingAutoTopUp = m('ClickedEnableBillingAutoTopUp')
export const ClickedDisableBillingAutoTopUp = m(
  'ClickedDisableBillingAutoTopUp',
)
export const ClickedRunBillingAutoTopUp = m('ClickedRunBillingAutoTopUp')
export const SucceededRedeemBillingCoupon = m('SucceededRedeemBillingCoupon', {
  response: BillingSummaryResponse,
})
export const FailedRedeemBillingCoupon = m('FailedRedeemBillingCoupon', {
  error: S.String,
})
export const SucceededCreateBillingCheckout = m(
  'SucceededCreateBillingCheckout',
  {
    response: BillingCheckoutResponse,
  },
)
export const FailedCreateBillingCheckout = m('FailedCreateBillingCheckout', {
  error: S.String,
})
export const SucceededPrepareBillingCardSetup = m(
  'SucceededPrepareBillingCardSetup',
  {
    response: BillingSetupIntentResponse,
  },
)
export const FailedPrepareBillingCardSetup = m(
  'FailedPrepareBillingCardSetup',
  {
    error: S.String,
  },
)
export const SucceededUpdateBillingAutoTopUpPolicy = m(
  'SucceededUpdateBillingAutoTopUpPolicy',
  {
    response: BillingSummaryResponse,
  },
)
export const FailedUpdateBillingAutoTopUpPolicy = m(
  'FailedUpdateBillingAutoTopUpPolicy',
  {
    error: S.String,
  },
)
export const SucceededRunBillingAutoTopUp = m('SucceededRunBillingAutoTopUp', {
  response: BillingSummaryResponse,
})
export const FailedRunBillingAutoTopUp = m('FailedRunBillingAutoTopUp', {
  error: S.String,
})
export const UpdatedInviteCode = m('UpdatedInviteCode', {
  value: S.String,
})
export const SubmittedInviteCode = m('SubmittedInviteCode')
export const RequestedLoadOnboardingRepositories = m(
  'RequestedLoadOnboardingRepositories',
)
export const SucceededLoadOnboardingRepositories = m(
  'SucceededLoadOnboardingRepositories',
  {
    response: OnboardingRepositoriesResponse,
  },
)
export const FailedLoadOnboardingRepositories = m(
  'FailedLoadOnboardingRepositories',
  {
    error: S.String,
  },
)
export const SelectedOnboardingRepository = m('SelectedOnboardingRepository', {
  repositoryId: S.String,
})
export const UpdatedOnboardingRepositorySearch = m(
  'UpdatedOnboardingRepositorySearch',
  {
    value: S.String,
  },
)
export const ClickedPreviousOnboardingRepositoryPage = m(
  'ClickedPreviousOnboardingRepositoryPage',
)
export const ClickedNextOnboardingRepositoryPage = m(
  'ClickedNextOnboardingRepositoryPage',
)
export const ClickedPreviousOnboardingStep = m('ClickedPreviousOnboardingStep')
export const ClickedOnboardingStep = m('ClickedOnboardingStep', {
  step: OnboardingStep,
})
export const UpdatedOnboardingManualRepositoryOwner = m(
  'UpdatedOnboardingManualRepositoryOwner',
  {
    value: S.String,
  },
)
export const UpdatedOnboardingManualRepositoryName = m(
  'UpdatedOnboardingManualRepositoryName',
  {
    value: S.String,
  },
)
export const ClickedSkipOnboardingRepository = m(
  'ClickedSkipOnboardingRepository',
)
export const SubmittedOnboardingRepository = m('SubmittedOnboardingRepository')
export const SucceededSelectOnboardingRepository = m(
  'SucceededSelectOnboardingRepository',
  {
    response: OnboardingStatusResponse,
  },
)
export const FailedSelectOnboardingRepository = m(
  'FailedSelectOnboardingRepository',
  {
    error: S.String,
  },
)
export const SucceededSkipOnboardingRepository = m(
  'SucceededSkipOnboardingRepository',
  {
    response: OnboardingStatusResponse,
  },
)
export const FailedSkipOnboardingRepository = m(
  'FailedSkipOnboardingRepository',
  {
    error: S.String,
  },
)
export const ClickedSkipOnboardingBilling = m('ClickedSkipOnboardingBilling')
export const SucceededSkipOnboardingBilling = m(
  'SucceededSkipOnboardingBilling',
  {
    response: OnboardingStatusResponse,
  },
)
export const FailedSkipOnboardingBilling = m('FailedSkipOnboardingBilling', {
  error: S.String,
})
export const UpdatedOnboardingGoal = m('UpdatedOnboardingGoal', {
  value: S.String,
})
export const SubmittedOnboardingGoal = m('SubmittedOnboardingGoal')
export const SucceededSubmitOnboardingGoal = m(
  'SucceededSubmitOnboardingGoal',
  {
    response: OnboardingStatusResponse,
  },
)
export const FailedSubmitOnboardingGoal = m('FailedSubmitOnboardingGoal', {
  error: S.String,
})
export const RequestedLoadCustomerOrder = m('RequestedLoadCustomerOrder')
export const SucceededLoadCustomerOrder = m('SucceededLoadCustomerOrder', {
  response: CustomerOrderResponse,
})
export const FailedLoadCustomerOrder = m('FailedLoadCustomerOrder', {
  error: S.String,
})
export const RequestedLoadCustomerOrders = m('RequestedLoadCustomerOrders')
export const SucceededLoadCustomerOrders = m('SucceededLoadCustomerOrders', {
  response: CustomerOrdersResponse,
})
export const FailedLoadCustomerOrders = m('FailedLoadCustomerOrders', {
  error: S.String,
})
export const RequestedLoadAutopilotWorkList = m(
  'RequestedLoadAutopilotWorkList',
)
export const SucceededLoadAutopilotWorkList = m(
  'SucceededLoadAutopilotWorkList',
  {
    response: AutopilotWorkListResponse,
  },
)
export const FailedLoadAutopilotWorkList = m('FailedLoadAutopilotWorkList', {
  error: S.String,
})
export const RequestedLoadCustomerOneCohort = m(
  'RequestedLoadCustomerOneCohort',
)
export const SucceededLoadCustomerOneCohort = m(
  'SucceededLoadCustomerOneCohort',
  {
    response: CustomerOneCohortProjection,
  },
)
export const FailedLoadCustomerOneCohort = m('FailedLoadCustomerOneCohort', {
  error: S.String,
})
export const RequestedLoadAutopilotMorningReport = m(
  'RequestedLoadAutopilotMorningReport',
)
export const SucceededLoadAutopilotMorningReport = m(
  'SucceededLoadAutopilotMorningReport',
  {
    response: AutopilotMorningReportResponse,
  },
)
export const FailedLoadAutopilotMorningReport = m(
  'FailedLoadAutopilotMorningReport',
  {
    error: S.String,
  },
)
export const UpdatedAutopilotWorkComposerField = m(
  'UpdatedAutopilotWorkComposerField',
  {
    field: AutopilotWorkComposerField,
    value: S.String,
  },
)
export const SubmittedAutopilotWorkComposer = m(
  'SubmittedAutopilotWorkComposer',
)
export const SucceededAutopilotWorkComposer = m(
  'SucceededAutopilotWorkComposer',
  {
    response: AutopilotWorkResponse,
  },
)
export const FailedAutopilotWorkComposer = m('FailedAutopilotWorkComposer', {
  error: S.String,
})
export const SelectedForgeAutomationTemplate = m(
  'SelectedForgeAutomationTemplate',
  {
    automationId: S.String,
  },
)
export const SubmittedForgeAutomationRun = m('SubmittedForgeAutomationRun', {
  automationId: S.String,
})
export const RequestedLoadAutopilotWorkDetail = m(
  'RequestedLoadAutopilotWorkDetail',
  {
    workOrderRef: S.String,
  },
)
export const SucceededLoadAutopilotWorkDetail = m(
  'SucceededLoadAutopilotWorkDetail',
  {
    response: AutopilotWorkResponse,
  },
)
export const FailedLoadAutopilotWorkDetail = m(
  'FailedLoadAutopilotWorkDetail',
  {
    error: S.String,
  },
)
export const SucceededLoadAutopilotWorkEvents = m(
  'SucceededLoadAutopilotWorkEvents',
  {
    response: AutopilotWorkEventsResponse,
  },
)
export const FailedLoadAutopilotWorkEvents = m(
  'FailedLoadAutopilotWorkEvents',
  {
    error: S.String,
  },
)
export const SucceededLoadAutopilotWorkBriefing = m(
  'SucceededLoadAutopilotWorkBriefing',
  {
    response: AutopilotWorkBriefingResponse,
  },
)
export const FailedLoadAutopilotWorkBriefing = m(
  'FailedLoadAutopilotWorkBriefing',
  {
    error: S.String,
  },
)
export const SubmittedAutopilotWorkReview = m('SubmittedAutopilotWorkReview', {
  action: AutopilotWorkReviewAction,
  workOrderRef: S.String,
})
export const SucceededAutopilotWorkReview = m('SucceededAutopilotWorkReview', {
  response: AutopilotWorkResponse,
})
export const FailedAutopilotWorkReview = m('FailedAutopilotWorkReview', {
  error: S.String,
})
export const RequestedLoadAutopilotDecisions = m(
  'RequestedLoadAutopilotDecisions',
)
export const SucceededLoadAutopilotDecisions = m(
  'SucceededLoadAutopilotDecisions',
  {
    response: AutopilotDecisionListResponse,
  },
)
export const FailedLoadAutopilotDecisions = m('FailedLoadAutopilotDecisions', {
  error: S.String,
})
export const SubmittedAutopilotDecisionAction = m(
  'SubmittedAutopilotDecisionAction',
  {
    action: AutopilotWorkReviewAction,
    decisionRef: S.String,
  },
)
export const SucceededAutopilotDecisionAction = m(
  'SucceededAutopilotDecisionAction',
  {
    response: AutopilotDecisionActionResponse,
  },
)
export const FailedAutopilotDecisionAction = m(
  'FailedAutopilotDecisionAction',
  {
    error: S.String,
  },
)
export const UpdatedCustomerOrderDraft = m('UpdatedCustomerOrderDraft', {
  value: S.String,
})
export const SubmittedCustomerOrder = m('SubmittedCustomerOrder')
export const SucceededSubmitCustomerOrder = m('SucceededSubmitCustomerOrder', {
  response: CustomerOrderResponse,
})
export const FailedSubmitCustomerOrder = m('FailedSubmitCustomerOrder', {
  error: S.String,
})
export const RequestedLoadCustomerFulfillmentArtifacts = m(
  'RequestedLoadCustomerFulfillmentArtifacts',
  {
    orderId: S.String,
  },
)
export const SucceededLoadCustomerFulfillmentArtifacts = m(
  'SucceededLoadCustomerFulfillmentArtifacts',
  {
    response: CustomerFulfillmentArtifactsResponse,
  },
)
export const FailedLoadCustomerFulfillmentArtifacts = m(
  'FailedLoadCustomerFulfillmentArtifacts',
  {
    error: S.String,
  },
)
export const RequestedLoadCustomerSiteRevisions = m(
  'RequestedLoadCustomerSiteRevisions',
  {
    orderId: S.String,
  },
)
export const SucceededLoadCustomerSiteRevisions = m(
  'SucceededLoadCustomerSiteRevisions',
  {
    response: CustomerSiteRevisionsResponse,
  },
)
export const FailedLoadCustomerSiteRevisions = m(
  'FailedLoadCustomerSiteRevisions',
  {
    error: S.String,
  },
)
export const RequestedLoadCustomerSiteFeedback = m(
  'RequestedLoadCustomerSiteFeedback',
  {
    orderId: S.String,
  },
)
export const SucceededLoadCustomerSiteFeedback = m(
  'SucceededLoadCustomerSiteFeedback',
  {
    response: CustomerSiteFeedbackResponse,
  },
)
export const FailedLoadCustomerSiteFeedback = m(
  'FailedLoadCustomerSiteFeedback',
  {
    error: S.String,
  },
)
export const UpdatedCustomerSiteFeedbackDraft = m(
  'UpdatedCustomerSiteFeedbackDraft',
  {
    value: S.String,
  },
)
export const SelectedCustomerSiteElementContext = m(
  'SelectedCustomerSiteElementContext',
  {
    context: SiteElementContext,
  },
)
export const SubmittedCustomerSiteFeedback = m(
  'SubmittedCustomerSiteFeedback',
  {
    orderId: S.String,
  },
)
export const SucceededSubmitCustomerSiteFeedback = m(
  'SucceededSubmitCustomerSiteFeedback',
  {
    response: SubmitCustomerSiteFeedbackResponse,
  },
)
export const FailedSubmitCustomerSiteFeedback = m(
  'FailedSubmitCustomerSiteFeedback',
  {
    error: S.String,
  },
)
export const RequestedOpenCustomerSiteBuilderSession = m(
  'RequestedOpenCustomerSiteBuilderSession',
  {
    orderId: S.String,
    promptSummary: S.String,
    siteId: S.String,
  },
)
export const SucceededOpenCustomerSiteBuilderSession = m(
  'SucceededOpenCustomerSiteBuilderSession',
  {
    response: CustomerSiteBuilderSessionResponse,
  },
)
export const FailedOpenCustomerSiteBuilderSession = m(
  'FailedOpenCustomerSiteBuilderSession',
  {
    error: S.String,
  },
)
export const RequestedLoadCustomerSiteBuilderSession = m(
  'RequestedLoadCustomerSiteBuilderSession',
  {
    sessionId: S.String,
  },
)
export const SucceededLoadCustomerSiteBuilderSession = m(
  'SucceededLoadCustomerSiteBuilderSession',
  {
    response: CustomerSiteBuilderSessionResponse,
  },
)
export const FailedLoadCustomerSiteBuilderSession = m(
  'FailedLoadCustomerSiteBuilderSession',
  {
    error: S.String,
  },
)
export const RequestedLoadCustomerSiteBuilderFiles = m(
  'RequestedLoadCustomerSiteBuilderFiles',
  {
    sessionId: S.String,
  },
)
export const SucceededLoadCustomerSiteBuilderFiles = m(
  'SucceededLoadCustomerSiteBuilderFiles',
  {
    filesResponse: CustomerSiteBuilderFileListResponse,
    treeResponse: CustomerSiteBuilderFileTreeResponse,
  },
)
export const FailedLoadCustomerSiteBuilderFiles = m(
  'FailedLoadCustomerSiteBuilderFiles',
  {
    error: S.String,
  },
)
export const SelectedCustomerSiteBuilderFile = m(
  'SelectedCustomerSiteBuilderFile',
  {
    path: S.String,
    sessionId: S.String,
  },
)
export const SucceededLoadCustomerSiteBuilderFile = m(
  'SucceededLoadCustomerSiteBuilderFile',
  {
    response: CustomerSiteBuilderFileReadResponse,
  },
)
export const FailedLoadCustomerSiteBuilderFile = m(
  'FailedLoadCustomerSiteBuilderFile',
  {
    error: S.String,
    path: S.String,
  },
)
export const RequestedLoadCustomerSiteBuilderEvents = m(
  'RequestedLoadCustomerSiteBuilderEvents',
  {
    cursor: S.optionalKey(S.Number),
    sessionId: S.String,
  },
)
export const SucceededLoadCustomerSiteBuilderEvents = m(
  'SucceededLoadCustomerSiteBuilderEvents',
  {
    response: CustomerSiteBuilderEventsResponse,
  },
)
export const FailedLoadCustomerSiteBuilderEvents = m(
  'FailedLoadCustomerSiteBuilderEvents',
  {
    error: S.String,
  },
)
export const RequestedLoadAdminOverview = m('RequestedLoadAdminOverview')
export const SucceededLoadAdminOverview = m('SucceededLoadAdminOverview', {
  response: AdminOverviewResponse,
})
export const FailedLoadAdminOverview = m('FailedLoadAdminOverview', {
  error: S.String,
})
export const RequestedLoadTokenUsageStats = m('RequestedLoadTokenUsageStats')
export const SucceededLoadTokenUsageStats = m('SucceededLoadTokenUsageStats', {
  analytics: InferenceAnalyticsResponse,
  filters: TokenUsageStatsFilters,
  leaderboards: TokenUsageLeaderboardsResponse,
  preference: TokenUsageLeaderboardPreferenceResponse,
  response: TokenUsageAggregateResponse,
})
export const FailedLoadTokenUsageStats = m('FailedLoadTokenUsageStats', {
  error: S.String,
  filters: TokenUsageStatsFilters,
})
export const UpdatedTokenUsageStatsFilter = m('UpdatedTokenUsageStatsFilter', {
  field: TokenUsageStatsFilterKey,
  value: S.String,
})
export const RequestedLoadProAgentDashboard = m(
  'RequestedLoadProAgentDashboard',
)
export const SucceededLoadProAgentDashboard = m(
  'SucceededLoadProAgentDashboard',
  {
    response: ProAgentDashboardResponse,
  },
)
export const FailedLoadProAgentDashboard = m('FailedLoadProAgentDashboard', {
  error: S.String,
})
export const RequestedLoadPrefilledWorkspace = m(
  'RequestedLoadPrefilledWorkspace',
  {
    workspaceId: S.String,
  },
)
export const SucceededLoadPrefilledWorkspace = m(
  'SucceededLoadPrefilledWorkspace',
  {
    response: PrefilledWorkspaceResponse,
  },
)
export const FailedLoadPrefilledWorkspace = m('FailedLoadPrefilledWorkspace', {
  error: S.String,
  workspaceId: S.String,
})
export const RequestedGenerateAdminSite = m('RequestedGenerateAdminSite', {
  siteId: S.String,
})
export const SucceededGenerateAdminSite = m('SucceededGenerateAdminSite', {
  response: AdminSiteGenerationResponse,
})
export const FailedGenerateAdminSite = m('FailedGenerateAdminSite', {
  error: S.String,
  siteId: S.String,
})
export const RequestedLoadAdminAdjutantAssignments = m(
  'RequestedLoadAdminAdjutantAssignments',
)
export const SucceededLoadAdminAdjutantAssignments = m(
  'SucceededLoadAdminAdjutantAssignments',
  {
    response: AdminAdjutantAssignmentsResponse,
  },
)
export const FailedLoadAdminAdjutantAssignments = m(
  'FailedLoadAdminAdjutantAssignments',
  {
    error: S.String,
  },
)
export const RequestedLoadAdminAdjutantReview = m(
  'RequestedLoadAdminAdjutantReview',
  {
    assignmentId: S.String,
  },
)
export const SucceededLoadAdminAdjutantReview = m(
  'SucceededLoadAdminAdjutantReview',
  {
    response: AdminAdjutantAssignmentReviewResponse,
  },
)
export const FailedLoadAdminAdjutantReview = m(
  'FailedLoadAdminAdjutantReview',
  {
    assignmentId: S.String,
    error: S.String,
  },
)
export const RequestedRunAdminAdjutantEnrichment = m(
  'RequestedRunAdminAdjutantEnrichment',
  {
    assignmentId: S.String,
    refresh: S.optionalKey(S.Boolean),
  },
)
export const RequestedReviewAdminAdjutantSourceCard = m(
  'RequestedReviewAdminAdjutantSourceCard',
  {
    assignmentId: S.String,
    reviewStatus: S.String,
    sourceId: S.String,
  },
)
export const RequestedReviewAdminAdjutantResearchBrief = m(
  'RequestedReviewAdminAdjutantResearchBrief',
  {
    assignmentId: S.String,
    briefId: S.String,
    status: S.String,
  },
)
export const SucceededAdminAdjutantEnrichmentAction = m(
  'SucceededAdminAdjutantEnrichmentAction',
  {
    action: S.String,
    assignmentId: S.String,
    message: S.String,
    response: AdminAdjutantEnrichmentActionResponse,
  },
)
export const FailedAdminAdjutantEnrichmentAction = m(
  'FailedAdminAdjutantEnrichmentAction',
  {
    action: S.String,
    assignmentId: S.String,
    error: S.String,
  },
)
export const RequestedDeployAdminSiteVersion = m(
  'RequestedDeployAdminSiteVersion',
  {
    assignmentId: S.String,
    publicLaunchChecklist: S.Boolean,
    siteId: S.String,
    versionId: S.String,
  },
)
export const RequestedAdminSiteDeploymentAction = m(
  'RequestedAdminSiteDeploymentAction',
  {
    action: S.Literals(['disable', 'rollback']),
    assignmentId: S.String,
    deploymentId: S.String,
    siteId: S.String,
  },
)
export const SucceededAdminSiteDeploymentAction = m(
  'SucceededAdminSiteDeploymentAction',
  {
    action: S.String,
    assignmentId: S.String,
    message: S.String,
    response: AdminSiteDeploymentActionResponse,
  },
)
export const FailedAdminSiteDeploymentAction = m(
  'FailedAdminSiteDeploymentAction',
  {
    action: S.String,
    assignmentId: S.String,
    error: S.String,
  },
)
export const RequestedLoadMulletBootstrap = m('RequestedLoadMulletBootstrap')
export const SelectedMulletScenarioTemplate = m(
  'SelectedMulletScenarioTemplate',
  {
    templateId: S.String,
  },
)
export const SelectedMulletSensitivityAxis = m(
  'SelectedMulletSensitivityAxis',
  {
    axisId: S.String,
  },
)
export const UpdatedMulletAssumption = m('UpdatedMulletAssumption', {
  assumptionId: S.String,
  field: S.Literals(['value', 'sourceLabel', 'provenance']),
  value: S.String,
})
export const SucceededLoadMulletBootstrap = m('SucceededLoadMulletBootstrap', {
  response: MulletBootstrapResponse,
})
export const FailedLoadMulletBootstrap = m('FailedLoadMulletBootstrap', {
  error: S.String,
})
export const UpdatedImageGenerationPrompt = m('UpdatedImageGenerationPrompt', {
  value: S.String,
})
export const SelectedImageGenerationProvider = m(
  'SelectedImageGenerationProvider',
  {
    provider: ImageGenerationProvider,
  },
)
export const SelectedImageGenerationModel = m('SelectedImageGenerationModel', {
  model: ImageGenerationModelId,
})
export const SelectedImageGenerationAspectRatio = m(
  'SelectedImageGenerationAspectRatio',
  {
    aspectRatio: ImageGenerationAspectRatio,
  },
)
export const SelectedImageGenerationImageSize = m(
  'SelectedImageGenerationImageSize',
  {
    imageSize: ImageGenerationImageSize,
  },
)
export const UpdatedImageGenerationCount = m('UpdatedImageGenerationCount', {
  value: S.String,
})
export const SubmittedImageGeneration = m('SubmittedImageGeneration')
export const SucceededGenerateImage = m('SucceededGenerateImage', {
  response: GenerateImageResponse,
})
export const FailedGenerateImage = m('FailedGenerateImage', {
  error: S.String,
})
export const UpdatedChatComposer = m('UpdatedChatComposer', {
  value: S.String,
})
export const SubmittedChatComposer = m('SubmittedChatComposer')
export const RequestedLoadTeamChatMessages = m(
  'RequestedLoadTeamChatMessages',
  {
    href: S.String,
    roomKey: S.String,
    teamId: S.String,
  },
)
export const SucceededLoadTeamChatMessages = m(
  'SucceededLoadTeamChatMessages',
  {
    response: TeamChatMessagesResponse,
    roomKey: S.String,
    teamId: S.String,
  },
)
export const FailedLoadTeamChatMessages = m('FailedLoadTeamChatMessages', {
  error: S.String,
  roomKey: S.String,
  teamId: S.String,
})
export const SucceededPostTeamChatMessage = m('SucceededPostTeamChatMessage', {
  requestId: S.String,
  response: TeamChatPostResponse,
})
export const FailedPostTeamChatMessage = m('FailedPostTeamChatMessage', {
  error: S.String,
  requestId: S.String,
  roomKey: S.String,
  teamId: S.String,
})
export const RequestedLoadThreadFiles = m('RequestedLoadThreadFiles', {
  href: S.String,
  scopeKey: S.String,
})
export const SucceededLoadThreadFiles = m('SucceededLoadThreadFiles', {
  response: ThreadFilesResponse,
  scopeKey: S.String,
})
export const FailedLoadThreadFiles = m('FailedLoadThreadFiles', {
  error: S.String,
  scopeKey: S.String,
})
export const RequestedLoadThreadFileDetail = m(
  'RequestedLoadThreadFileDetail',
  {
    fileId: S.String,
    href: S.String,
  },
)
export const SucceededLoadThreadFileDetail = m(
  'SucceededLoadThreadFileDetail',
  {
    fileId: S.String,
    response: ThreadFileDetailResponse,
  },
)
export const FailedLoadThreadFileDetail = m('FailedLoadThreadFileDetail', {
  error: S.String,
  fileId: S.String,
})
export const ClickedThreadFileDownload = m('ClickedThreadFileDownload', {
  downloadUrl: S.String,
  fileId: S.String,
  filename: S.String,
})
export const SucceededDownloadThreadFile = m('SucceededDownloadThreadFile', {
  fileId: S.String,
})
export const FailedDownloadThreadFile = m('FailedDownloadThreadFile', {
  error: S.String,
  fileId: S.String,
})
export const ClickedThreadFileDownloadToggle = m(
  'ClickedThreadFileDownloadToggle',
  {
    downloadEnabled: S.Boolean,
    fileId: S.String,
  },
)
export const SucceededUpdateThreadFileDownload = m(
  'SucceededUpdateThreadFileDownload',
  {
    fileId: S.String,
    response: ThreadFileDetailResponse,
  },
)
export const FailedUpdateThreadFileDownload = m(
  'FailedUpdateThreadFileDownload',
  {
    error: S.String,
    fileId: S.String,
  },
)
export const SubmittedThreadFileUpload = m('SubmittedThreadFileUpload', {
  file: FileSchema,
  inputId: S.String,
  scopeKey: S.String,
  teamId: S.NullOr(S.String),
  threadId: S.String,
})
export const SucceededUploadThreadFile = m('SucceededUploadThreadFile', {
  response: ThreadFileUploadResponse,
  scopeKey: S.String,
})
export const FailedUploadThreadFile = m('FailedUploadThreadFile', {
  error: S.String,
  scopeKey: S.String,
})
export const EnteredAutopilotRunRoute = m('EnteredAutopilotRunRoute', {
  runId: S.String,
})
export const SucceededLaunchAutopilotRun = m('SucceededLaunchAutopilotRun', {
  requestId: S.String,
  response: AgentRunLaunchResponse,
})
export const FailedLaunchAutopilotRun = m('FailedLaunchAutopilotRun', {
  requestId: S.String,
  error: S.String,
})
export const RequestedPollAutopilotRun = m('RequestedPollAutopilotRun', {
  runId: S.String,
})
export const SucceededFetchAutopilotRun = m('SucceededFetchAutopilotRun', {
  runId: S.String,
  response: AgentRunDetailResponse,
})
export const FailedFetchAutopilotRun = m('FailedFetchAutopilotRun', {
  runId: S.String,
  error: S.String,
})
export const RequestedNotificationPermission = m(
  'RequestedNotificationPermission',
)
export const ResolvedNotificationPermission = m(
  'ResolvedNotificationPermission',
  {
    granted: S.Boolean,
    canAskAgain: S.Boolean,
  },
)
export const RaisedBrowserNotifications = m('RaisedBrowserNotifications')
export const DismissedNotifications = m('DismissedNotifications')
export const RequestedLoadAgentGoal = m('RequestedLoadAgentGoal', {
  href: S.String,
  scopeKey: S.String,
})
export const SucceededLoadAgentGoal = m('SucceededLoadAgentGoal', {
  response: AgentGoalResponse,
  scopeKey: S.String,
})
export const FailedLoadAgentGoal = m('FailedLoadAgentGoal', {
  error: S.String,
  scopeKey: S.String,
})
export const UpdatedAgentGoalObjectiveDraft = m(
  'UpdatedAgentGoalObjectiveDraft',
  { value: S.String },
)
export const UpdatedAgentGoalBudgetDraft = m('UpdatedAgentGoalBudgetDraft', {
  value: S.String,
})
export const ClickedEditAgentGoal = m('ClickedEditAgentGoal')
export const ClickedCancelEditAgentGoal = m('ClickedCancelEditAgentGoal')
export const SubmittedAgentGoal = m('SubmittedAgentGoal')
export const SucceededSaveAgentGoal = m('SucceededSaveAgentGoal', {
  response: AgentGoalResponse,
  scopeKey: S.String,
})
export const FailedSaveAgentGoal = m('FailedSaveAgentGoal', {
  error: S.String,
  scopeKey: S.String,
})
export const ClickedAgentGoalAction = m('ClickedAgentGoalAction', {
  action: AgentGoalAction,
})
export const SucceededAgentGoalAction = m('SucceededAgentGoalAction', {
  action: AgentGoalAction,
  response: AgentGoalResponse,
  scopeKey: S.String,
})
export const FailedAgentGoalAction = m('FailedAgentGoalAction', {
  action: AgentGoalAction,
  error: S.String,
  scopeKey: S.String,
})
export const RequestedLoadArtanisOperatorConsole = m(
  'RequestedLoadArtanisOperatorConsole',
)
export const SucceededLoadArtanisOperatorConsole = m(
  'SucceededLoadArtanisOperatorConsole',
  {
    response: ArtanisOperatorConsoleResponse,
  },
)
export const FailedLoadArtanisOperatorConsole = m(
  'FailedLoadArtanisOperatorConsole',
  {
    error: S.String,
  },
)
export const RequestedLoadArtanisOperatorDashboard = m(
  'RequestedLoadArtanisOperatorDashboard',
  {
    callerIdFilter: S.String,
    threadRef: S.String,
  },
)
export const SucceededLoadArtanisOperatorDashboard = m(
  'SucceededLoadArtanisOperatorDashboard',
  {
    response: ArtanisOperatorDashboardResponse,
  },
)
export const FailedLoadArtanisOperatorDashboard = m(
  'FailedLoadArtanisOperatorDashboard',
  {
    error: S.String,
  },
)
export const UpdatedArtanisOperatorDashboardCallerIdFilter = m(
  'UpdatedArtanisOperatorDashboardCallerIdFilter',
  {
    value: S.String,
  },
)
export const SubmittedArtanisOperatorDashboardFilter = m(
  'SubmittedArtanisOperatorDashboardFilter',
)
export const SelectedArtanisOperatorDashboardThread = m(
  'SelectedArtanisOperatorDashboardThread',
  {
    threadRef: S.String,
  },
)
export const RequestedLoadArtanisOperatorGoal = m(
  'RequestedLoadArtanisOperatorGoal',
  {
    href: S.String,
    scopeKey: S.String,
  },
)
export const SucceededLoadArtanisOperatorGoal = m(
  'SucceededLoadArtanisOperatorGoal',
  {
    response: AgentGoalResponse,
    scopeKey: S.String,
  },
)
export const FailedLoadArtanisOperatorGoal = m(
  'FailedLoadArtanisOperatorGoal',
  {
    error: S.String,
    scopeKey: S.String,
  },
)
export const UpdatedArtanisOperatorGoalObjectiveDraft = m(
  'UpdatedArtanisOperatorGoalObjectiveDraft',
  { value: S.String },
)
export const SubmittedArtanisOperatorGoal = m('SubmittedArtanisOperatorGoal')
export const ClickedArtanisOperatorGoalAction = m(
  'ClickedArtanisOperatorGoalAction',
  {
    action: AgentGoalAction,
  },
)
export const SucceededSaveArtanisOperatorGoal = m(
  'SucceededSaveArtanisOperatorGoal',
  {
    response: AgentGoalResponse,
    scopeKey: S.String,
  },
)
export const FailedSaveArtanisOperatorGoal = m(
  'FailedSaveArtanisOperatorGoal',
  {
    error: S.String,
    scopeKey: S.String,
  },
)
export const SucceededArtanisOperatorGoalAction = m(
  'SucceededArtanisOperatorGoalAction',
  {
    action: AgentGoalAction,
    response: AgentGoalResponse,
    scopeKey: S.String,
  },
)
export const FailedArtanisOperatorGoalAction = m(
  'FailedArtanisOperatorGoalAction',
  {
    action: AgentGoalAction,
    error: S.String,
    scopeKey: S.String,
  },
)
export const ClickedArtanisOperatorApprovalAction = m(
  'ClickedArtanisOperatorApprovalAction',
  {
    action: ArtanisOperatorApprovalAction,
    gateRef: S.String,
  },
)
export const SucceededArtanisOperatorApprovalAction = m(
  'SucceededArtanisOperatorApprovalAction',
  {
    action: ArtanisOperatorApprovalAction,
    gateRef: S.String,
    response: ArtanisOperatorConsoleResponse,
  },
)
export const FailedArtanisOperatorApprovalAction = m(
  'FailedArtanisOperatorApprovalAction',
  {
    action: ArtanisOperatorApprovalAction,
    error: S.String,
    gateRef: S.String,
  },
)
export const RequestedLoadSyncSnapshot = m('RequestedLoadSyncSnapshot', {
  href: S.String,
  scope: S.String,
})
export const SucceededLoadSyncSnapshot = m('SucceededLoadSyncSnapshot', {
  scope: S.String,
  snapshot: SyncSnapshot,
})
export const FailedLoadSyncSnapshot = m('FailedLoadSyncSnapshot', {
  scope: S.String,
  error: S.String,
})
export const OpenedSyncStream = m('OpenedSyncStream', {
  scope: S.String,
})
export const ClosedSyncStream = m('ClosedSyncStream', {
  scope: S.String,
})
export const FailedSyncStream = m('FailedSyncStream', {
  scope: S.String,
  error: S.String,
})
export const ReceivedSyncPatch = m('ReceivedSyncPatch', {
  patch: SyncPatch,
})
export const ReceivedSyncCursorGap = m('ReceivedSyncCursorGap', {
  gap: CursorGap,
})
export const CompletedScrollChatTimelineToEnd = m(
  'CompletedScrollChatTimelineToEnd',
)
export const CompletedFocusChatComposer = m('CompletedFocusChatComposer')
export const CompletedSetAutopilotThreadUrl = m(
  'CompletedSetAutopilotThreadUrl',
)
export const CompletedInstallAccountMenuOutsideClick = m(
  'CompletedInstallAccountMenuOutsideClick',
)
export const Message = S.Union([
  ClickedLogout,
  ClickedNewChat,
  ClickedRunMetadataInfo,
  ClosedRunMetadataInfo,
  ClickedStartProviderDeviceLogin,
  ClickedPollProviderDeviceLogin,
  SucceededStartProviderDeviceLogin,
  FailedStartProviderDeviceLogin,
  SucceededPollProviderDeviceLogin,
  FailedPollProviderDeviceLogin,
  RequestedLoadProviderAccountPool,
  SucceededLoadProviderAccountPool,
  FailedLoadProviderAccountPool,
  ClickedResetProviderAccountPoolAccount,
  SucceededResetProviderAccountPoolAccount,
  FailedResetProviderAccountPoolAccount,
  ClickedBillingPackage,
  UpdatedBillingCouponCode,
  SubmittedBillingCoupon,
  ClickedPrepareBillingCardSetup,
  ClickedEnableBillingAutoTopUp,
  ClickedDisableBillingAutoTopUp,
  ClickedRunBillingAutoTopUp,
  SucceededRedeemBillingCoupon,
  FailedRedeemBillingCoupon,
  SucceededCreateBillingCheckout,
  FailedCreateBillingCheckout,
  SucceededPrepareBillingCardSetup,
  FailedPrepareBillingCardSetup,
  SucceededUpdateBillingAutoTopUpPolicy,
  FailedUpdateBillingAutoTopUpPolicy,
  SucceededRunBillingAutoTopUp,
  FailedRunBillingAutoTopUp,
  UpdatedInviteCode,
  SubmittedInviteCode,
  RequestedLoadOnboardingRepositories,
  SucceededLoadOnboardingRepositories,
  FailedLoadOnboardingRepositories,
  SelectedOnboardingRepository,
  UpdatedOnboardingRepositorySearch,
  ClickedPreviousOnboardingRepositoryPage,
  ClickedNextOnboardingRepositoryPage,
  ClickedPreviousOnboardingStep,
  ClickedOnboardingStep,
  UpdatedOnboardingManualRepositoryOwner,
  UpdatedOnboardingManualRepositoryName,
  ClickedSkipOnboardingRepository,
  SubmittedOnboardingRepository,
  SucceededSelectOnboardingRepository,
  FailedSelectOnboardingRepository,
  SucceededSkipOnboardingRepository,
  FailedSkipOnboardingRepository,
  ClickedSkipOnboardingBilling,
  SucceededSkipOnboardingBilling,
  FailedSkipOnboardingBilling,
  UpdatedOnboardingGoal,
  SubmittedOnboardingGoal,
  SucceededSubmitOnboardingGoal,
  FailedSubmitOnboardingGoal,
  RequestedLoadCustomerOrder,
  SucceededLoadCustomerOrder,
  FailedLoadCustomerOrder,
  RequestedLoadCustomerOrders,
  SucceededLoadCustomerOrders,
  FailedLoadCustomerOrders,
  RequestedLoadAutopilotWorkList,
  SucceededLoadAutopilotWorkList,
  FailedLoadAutopilotWorkList,
  RequestedLoadCustomerOneCohort,
  SucceededLoadCustomerOneCohort,
  FailedLoadCustomerOneCohort,
  RequestedLoadAutopilotMorningReport,
  SucceededLoadAutopilotMorningReport,
  FailedLoadAutopilotMorningReport,
  UpdatedAutopilotWorkComposerField,
  SubmittedAutopilotWorkComposer,
  SucceededAutopilotWorkComposer,
  FailedAutopilotWorkComposer,
  SelectedForgeAutomationTemplate,
  SubmittedForgeAutomationRun,
  RequestedLoadAutopilotWorkDetail,
  SucceededLoadAutopilotWorkDetail,
  FailedLoadAutopilotWorkDetail,
  SucceededLoadAutopilotWorkEvents,
  FailedLoadAutopilotWorkEvents,
  SucceededLoadAutopilotWorkBriefing,
  FailedLoadAutopilotWorkBriefing,
  SubmittedAutopilotWorkReview,
  SucceededAutopilotWorkReview,
  FailedAutopilotWorkReview,
  RequestedLoadAutopilotDecisions,
  SucceededLoadAutopilotDecisions,
  FailedLoadAutopilotDecisions,
  SubmittedAutopilotDecisionAction,
  SucceededAutopilotDecisionAction,
  FailedAutopilotDecisionAction,
  SelectedWorkroomTab,
  RequestedLoadWorkroomSurface,
  SucceededLoadWorkroomSurface,
  FailedLoadWorkroomSurface,
  RequestedLoadWorkroomLifecycle,
  SucceededLoadWorkroomLifecycle,
  FailedLoadWorkroomLifecycle,
  SubmittedWorkroomLifecycleDecision,
  SucceededWorkroomLifecycleDecision,
  FailedWorkroomLifecycleDecision,
  UpdatedCustomerOrderDraft,
  SubmittedCustomerOrder,
  SucceededSubmitCustomerOrder,
  FailedSubmitCustomerOrder,
  RequestedLoadCustomerFulfillmentArtifacts,
  SucceededLoadCustomerFulfillmentArtifacts,
  FailedLoadCustomerFulfillmentArtifacts,
  RequestedLoadCustomerSiteRevisions,
  SucceededLoadCustomerSiteRevisions,
  FailedLoadCustomerSiteRevisions,
  RequestedLoadCustomerSiteFeedback,
  SucceededLoadCustomerSiteFeedback,
  FailedLoadCustomerSiteFeedback,
  UpdatedCustomerSiteFeedbackDraft,
  SelectedCustomerSiteElementContext,
  SubmittedCustomerSiteFeedback,
  SucceededSubmitCustomerSiteFeedback,
  FailedSubmitCustomerSiteFeedback,
  RequestedOpenCustomerSiteBuilderSession,
  SucceededOpenCustomerSiteBuilderSession,
  FailedOpenCustomerSiteBuilderSession,
  RequestedLoadCustomerSiteBuilderSession,
  SucceededLoadCustomerSiteBuilderSession,
  FailedLoadCustomerSiteBuilderSession,
  RequestedLoadCustomerSiteBuilderFiles,
  SucceededLoadCustomerSiteBuilderFiles,
  FailedLoadCustomerSiteBuilderFiles,
  SelectedCustomerSiteBuilderFile,
  SucceededLoadCustomerSiteBuilderFile,
  FailedLoadCustomerSiteBuilderFile,
  RequestedLoadCustomerSiteBuilderEvents,
  SucceededLoadCustomerSiteBuilderEvents,
  FailedLoadCustomerSiteBuilderEvents,
  RequestedLoadAdminOverview,
  SucceededLoadAdminOverview,
  FailedLoadAdminOverview,
  RequestedLoadTokenUsageStats,
  SucceededLoadTokenUsageStats,
  FailedLoadTokenUsageStats,
  UpdatedTokenUsageStatsFilter,
  RequestedLoadProAgentDashboard,
  SucceededLoadProAgentDashboard,
  FailedLoadProAgentDashboard,
  RequestedLoadPrefilledWorkspace,
  SucceededLoadPrefilledWorkspace,
  FailedLoadPrefilledWorkspace,
  RequestedGenerateAdminSite,
  SucceededGenerateAdminSite,
  FailedGenerateAdminSite,
  RequestedLoadAdminAdjutantAssignments,
  SucceededLoadAdminAdjutantAssignments,
  FailedLoadAdminAdjutantAssignments,
  RequestedLoadAdminAdjutantReview,
  SucceededLoadAdminAdjutantReview,
  FailedLoadAdminAdjutantReview,
  RequestedRunAdminAdjutantEnrichment,
  RequestedReviewAdminAdjutantSourceCard,
  RequestedReviewAdminAdjutantResearchBrief,
  SucceededAdminAdjutantEnrichmentAction,
  FailedAdminAdjutantEnrichmentAction,
  RequestedDeployAdminSiteVersion,
  RequestedAdminSiteDeploymentAction,
  SucceededAdminSiteDeploymentAction,
  FailedAdminSiteDeploymentAction,
  RequestedLoadMulletBootstrap,
  SelectedMulletScenarioTemplate,
  SelectedMulletSensitivityAxis,
  UpdatedMulletAssumption,
  SucceededLoadMulletBootstrap,
  FailedLoadMulletBootstrap,
  UpdatedImageGenerationPrompt,
  SelectedImageGenerationProvider,
  SelectedImageGenerationModel,
  SelectedImageGenerationAspectRatio,
  SelectedImageGenerationImageSize,
  UpdatedImageGenerationCount,
  SubmittedImageGeneration,
  SucceededGenerateImage,
  FailedGenerateImage,
  UpdatedChatComposer,
  SubmittedChatComposer,
  RequestedLoadTeamChatMessages,
  SucceededLoadTeamChatMessages,
  FailedLoadTeamChatMessages,
  SucceededPostTeamChatMessage,
  FailedPostTeamChatMessage,
  RequestedLoadThreadFiles,
  SucceededLoadThreadFiles,
  FailedLoadThreadFiles,
  RequestedLoadThreadFileDetail,
  SucceededLoadThreadFileDetail,
  FailedLoadThreadFileDetail,
  ClickedThreadFileDownload,
  SucceededDownloadThreadFile,
  FailedDownloadThreadFile,
  ClickedThreadFileDownloadToggle,
  SucceededUpdateThreadFileDownload,
  FailedUpdateThreadFileDownload,
  SubmittedThreadFileUpload,
  SucceededUploadThreadFile,
  FailedUploadThreadFile,
  EnteredAutopilotRunRoute,
  SucceededLaunchAutopilotRun,
  FailedLaunchAutopilotRun,
  RequestedPollAutopilotRun,
  SucceededFetchAutopilotRun,
  FailedFetchAutopilotRun,
  RequestedNotificationPermission,
  ResolvedNotificationPermission,
  RaisedBrowserNotifications,
  DismissedNotifications,
  RequestedLoadAgentGoal,
  SucceededLoadAgentGoal,
  FailedLoadAgentGoal,
  UpdatedAgentGoalObjectiveDraft,
  UpdatedAgentGoalBudgetDraft,
  ClickedEditAgentGoal,
  ClickedCancelEditAgentGoal,
  SubmittedAgentGoal,
  SucceededSaveAgentGoal,
  FailedSaveAgentGoal,
  ClickedAgentGoalAction,
  SucceededAgentGoalAction,
  FailedAgentGoalAction,
  RequestedLoadArtanisOperatorConsole,
  SucceededLoadArtanisOperatorConsole,
  FailedLoadArtanisOperatorConsole,
  RequestedLoadArtanisOperatorDashboard,
  SucceededLoadArtanisOperatorDashboard,
  FailedLoadArtanisOperatorDashboard,
  UpdatedArtanisOperatorDashboardCallerIdFilter,
  SubmittedArtanisOperatorDashboardFilter,
  SelectedArtanisOperatorDashboardThread,
  RequestedLoadArtanisOperatorGoal,
  SucceededLoadArtanisOperatorGoal,
  FailedLoadArtanisOperatorGoal,
  UpdatedArtanisOperatorGoalObjectiveDraft,
  SubmittedArtanisOperatorGoal,
  ClickedArtanisOperatorGoalAction,
  SucceededSaveArtanisOperatorGoal,
  FailedSaveArtanisOperatorGoal,
  SucceededArtanisOperatorGoalAction,
  FailedArtanisOperatorGoalAction,
  ClickedArtanisOperatorApprovalAction,
  SucceededArtanisOperatorApprovalAction,
  FailedArtanisOperatorApprovalAction,
  RequestedLoadSyncSnapshot,
  SucceededLoadSyncSnapshot,
  FailedLoadSyncSnapshot,
  OpenedSyncStream,
  ClosedSyncStream,
  FailedSyncStream,
  ReceivedSyncPatch,
  ReceivedSyncCursorGap,
  CompletedScrollChatTimelineToEnd,
  CompletedFocusChatComposer,
  CompletedSetAutopilotThreadUrl,
  CompletedInstallAccountMenuOutsideClick,
])
export type Message = typeof Message.Type

// OUT MESSAGE

export const RequestedLogout = m('RequestedLogout')
export const CompletedOnboarding = m('CompletedOnboarding')
export const OutMessage = S.Union([RequestedLogout, CompletedOnboarding])
export type OutMessage = typeof OutMessage.Type
