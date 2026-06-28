import { Match as M } from 'effect'

import { updateAdmin } from './admin/transitions'
import { updateArtanisConsole } from './artanis-console/transitions'
import { updateAutopilotWork } from './autopilot-work/transitions'
import { updateBilling } from './billing/transitions'
import { updateCustomerOrder } from './customer-order/transitions'
import { updateAutopilotDecisions } from './decisions/transitions'
import { updateAgentGoals } from './goals/transitions'
import { updateImages } from './images/transitions'
import { Message } from './message'
import { Model } from './model'
import { updateMullet } from './mullet/transitions'
import { updateNotifications } from './notifications/transitions'
import { updateOnboarding } from './onboarding/transitions'
import { updateProviders } from './providers/transitions'
import { updateRunState } from './runs/transitions'
import { updateSessionChrome } from './session/transitions'
import { updateStats } from './stats/transitions'
import { updateSync } from './sync/transitions'
import { updateTeamChat } from './team-chat/transitions'
import { updateThreadFiles } from './thread-files/transitions'
import { type UpdateReturn } from './transition'
import { updateWorkroom } from './workroom/transitions'
import { updatePrefilledWorkspace } from './workspace/transitions'

export {
  FocusChatComposer,
  ScrollChatTimelineToEnd,
  SetAutopilotThreadUrl,
} from './commands/dom'
export { LoadAgentGoal } from './goals/commands'
export { FetchAutopilotRun, LaunchAutopilotRun } from './runs/commands'
export { chatRunIsBusy } from './runs/transitions'
export { LoadSyncSnapshot } from './sync/commands'
export { initialCommands } from './initial-commands'
export {
  DeployAdminSiteVersion,
  GenerateAdminSite,
  LoadAdminAdjutantAssignments,
  LoadAdminAdjutantReview,
  LoadAdminOverview,
  ReviewAdminAdjutantResearchBrief,
  ReviewAdminAdjutantSourceCard,
  RunAdminSiteDeploymentAction,
  RunAdminAdjutantEnrichment,
} from './admin/transitions'
export {
  DownloadThreadFile,
  LoadThreadFileDetail,
  LoadThreadFiles,
  UpdateThreadFileDownload,
  UploadThreadFile,
} from './thread-files/commands'
export { LoadTeamChatMessages, PostTeamChatMessage } from './team-chat/commands'
export { LoadMulletBootstrap } from './mullet/transitions'
export { LoadTokenUsageStats } from './stats/transitions'
export { LoadPrefilledWorkspace } from './workspace/transitions'
export {
  personalChatThreadId,
  teamChatRoomKey,
  teamChatThreadId,
  teamProjectChatThreadId,
  teamFilesScopeKey,
  threadFilesScopeKey,
} from './chatState'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const update = (model: Model, message: Message): UpdateReturn => {
  const customerOrder = () => updateCustomerOrder(model, message)
  const artanisConsole = () => updateArtanisConsole(model, message)
  const autopilotWork = () => updateAutopilotWork(model, message)
  const autopilotDecisions = () => updateAutopilotDecisions(model, message)
  const workroom = () => updateWorkroom(model, message)

  return M.value(message).pipe(
    withUpdateReturn,
    M.tagsExhaustive({
      ClickedLogout: () => updateSessionChrome(model, message),
      UpdatedInviteCode: () => updateSessionChrome(model, message),
      SubmittedInviteCode: () => updateSessionChrome(model, message),
      UpdatedChatComposer: () => updateSessionChrome(model, message),
      CompletedSetAutopilotThreadUrl: () => updateSessionChrome(model, message),
      CompletedInstallAccountMenuOutsideClick: () =>
        updateSessionChrome(model, message),
      CompletedScrollChatTimelineToEnd: () =>
        updateSessionChrome(model, message),
      CompletedFocusChatComposer: () => updateSessionChrome(model, message),
      ClickedBillingPackage: () => updateBilling(model, message),
      UpdatedBillingCouponCode: () => updateBilling(model, message),
      SubmittedBillingCoupon: () => updateBilling(model, message),
      ClickedPrepareBillingCardSetup: () => updateBilling(model, message),
      ClickedEnableBillingAutoTopUp: () => updateBilling(model, message),
      ClickedDisableBillingAutoTopUp: () => updateBilling(model, message),
      ClickedRunBillingAutoTopUp: () => updateBilling(model, message),
      SucceededRedeemBillingCoupon: () => updateBilling(model, message),
      FailedRedeemBillingCoupon: () => updateBilling(model, message),
      SucceededCreateBillingCheckout: () => updateBilling(model, message),
      FailedCreateBillingCheckout: () => updateBilling(model, message),
      SucceededPrepareBillingCardSetup: () => updateBilling(model, message),
      FailedPrepareBillingCardSetup: () => updateBilling(model, message),
      SucceededUpdateBillingAutoTopUpPolicy: () =>
        updateBilling(model, message),
      FailedUpdateBillingAutoTopUpPolicy: () => updateBilling(model, message),
      SucceededRunBillingAutoTopUp: () => updateBilling(model, message),
      FailedRunBillingAutoTopUp: () => updateBilling(model, message),
      ClickedStartProviderDeviceLogin: () => updateProviders(model, message),
      ClickedPollProviderDeviceLogin: () => updateProviders(model, message),
      SucceededStartProviderDeviceLogin: () => updateProviders(model, message),
      FailedStartProviderDeviceLogin: () => updateProviders(model, message),
      SucceededPollProviderDeviceLogin: () => updateProviders(model, message),
      FailedPollProviderDeviceLogin: () => updateProviders(model, message),
      RequestedLoadProviderAccountPool: () => updateProviders(model, message),
      SucceededLoadProviderAccountPool: () => updateProviders(model, message),
      FailedLoadProviderAccountPool: () => updateProviders(model, message),

      RequestedLoadOnboardingRepositories: () =>
        updateOnboarding(model, message),
      SucceededLoadOnboardingRepositories: () =>
        updateOnboarding(model, message),
      FailedLoadOnboardingRepositories: () => updateOnboarding(model, message),
      UpdatedOnboardingRepositorySearch: () => updateOnboarding(model, message),
      ClickedPreviousOnboardingRepositoryPage: () =>
        updateOnboarding(model, message),
      ClickedNextOnboardingRepositoryPage: () =>
        updateOnboarding(model, message),
      ClickedPreviousOnboardingStep: () => updateOnboarding(model, message),
      ClickedOnboardingStep: () => updateOnboarding(model, message),
      SelectedOnboardingRepository: () => updateOnboarding(model, message),
      UpdatedOnboardingManualRepositoryOwner: () =>
        updateOnboarding(model, message),
      UpdatedOnboardingManualRepositoryName: () =>
        updateOnboarding(model, message),
      SubmittedOnboardingRepository: () => updateOnboarding(model, message),
      ClickedSkipOnboardingRepository: () => updateOnboarding(model, message),
      SucceededSelectOnboardingRepository: () =>
        updateOnboarding(model, message),
      FailedSelectOnboardingRepository: () => updateOnboarding(model, message),
      SucceededSkipOnboardingRepository: () => updateOnboarding(model, message),
      FailedSkipOnboardingRepository: () => updateOnboarding(model, message),
      ClickedSkipOnboardingBilling: () => updateOnboarding(model, message),
      SucceededSkipOnboardingBilling: () => updateOnboarding(model, message),
      FailedSkipOnboardingBilling: () => updateOnboarding(model, message),
      UpdatedOnboardingGoal: () => updateOnboarding(model, message),
      SubmittedOnboardingGoal: () => updateOnboarding(model, message),
      SucceededSubmitOnboardingGoal: () => updateOnboarding(model, message),
      FailedSubmitOnboardingGoal: () => updateOnboarding(model, message),
      RequestedLoadCustomerOrder: customerOrder,
      SucceededLoadCustomerOrder: customerOrder,
      FailedLoadCustomerOrder: customerOrder,
      RequestedLoadCustomerOrders: customerOrder,
      SucceededLoadCustomerOrders: customerOrder,
      FailedLoadCustomerOrders: customerOrder,
      RequestedLoadAutopilotWorkList: autopilotWork,
      SucceededLoadAutopilotWorkList: autopilotWork,
      FailedLoadAutopilotWorkList: autopilotWork,
      RequestedLoadCustomerOneCohort: autopilotWork,
      SucceededLoadCustomerOneCohort: autopilotWork,
      FailedLoadCustomerOneCohort: autopilotWork,
      RequestedLoadAutopilotMorningReport: autopilotWork,
      SucceededLoadAutopilotMorningReport: autopilotWork,
      FailedLoadAutopilotMorningReport: autopilotWork,
      UpdatedAutopilotWorkComposerField: autopilotWork,
      SubmittedAutopilotWorkComposer: autopilotWork,
      SucceededAutopilotWorkComposer: autopilotWork,
      FailedAutopilotWorkComposer: autopilotWork,
      SelectedForgeAutomationTemplate: autopilotWork,
      SubmittedForgeAutomationRun: autopilotWork,
      RequestedLoadAutopilotWorkDetail: autopilotWork,
      SucceededLoadAutopilotWorkDetail: autopilotWork,
      FailedLoadAutopilotWorkDetail: autopilotWork,
      SucceededLoadAutopilotWorkEvents: autopilotWork,
      FailedLoadAutopilotWorkEvents: autopilotWork,
      SucceededLoadAutopilotWorkBriefing: autopilotWork,
      FailedLoadAutopilotWorkBriefing: autopilotWork,
      SubmittedAutopilotWorkReview: autopilotWork,
      SucceededAutopilotWorkReview: autopilotWork,
      FailedAutopilotWorkReview: autopilotWork,
      RequestedLoadAutopilotDecisions: autopilotDecisions,
      SucceededLoadAutopilotDecisions: autopilotDecisions,
      FailedLoadAutopilotDecisions: autopilotDecisions,
      SubmittedAutopilotDecisionAction: autopilotDecisions,
      SucceededAutopilotDecisionAction: autopilotDecisions,
      FailedAutopilotDecisionAction: autopilotDecisions,
      SelectedWorkroomTab: workroom,
      RequestedLoadWorkroomSurface: workroom,
      SucceededLoadWorkroomSurface: workroom,
      FailedLoadWorkroomSurface: workroom,
      RequestedLoadWorkroomLifecycle: workroom,
      SucceededLoadWorkroomLifecycle: workroom,
      FailedLoadWorkroomLifecycle: workroom,
      SubmittedWorkroomLifecycleDecision: workroom,
      SucceededWorkroomLifecycleDecision: workroom,
      FailedWorkroomLifecycleDecision: workroom,
      UpdatedCustomerOrderDraft: customerOrder,
      SubmittedCustomerOrder: customerOrder,
      SucceededSubmitCustomerOrder: customerOrder,
      FailedSubmitCustomerOrder: customerOrder,
      RequestedLoadCustomerFulfillmentArtifacts: customerOrder,
      SucceededLoadCustomerFulfillmentArtifacts: customerOrder,
      FailedLoadCustomerFulfillmentArtifacts: customerOrder,
      RequestedLoadCustomerSiteRevisions: customerOrder,
      SucceededLoadCustomerSiteRevisions: customerOrder,
      FailedLoadCustomerSiteRevisions: customerOrder,
      RequestedLoadCustomerSiteFeedback: customerOrder,
      SucceededLoadCustomerSiteFeedback: customerOrder,
      FailedLoadCustomerSiteFeedback: customerOrder,
      UpdatedCustomerSiteFeedbackDraft: customerOrder,
      SelectedCustomerSiteElementContext: customerOrder,
      SubmittedCustomerSiteFeedback: customerOrder,
      SucceededSubmitCustomerSiteFeedback: customerOrder,
      FailedSubmitCustomerSiteFeedback: customerOrder,
      RequestedOpenCustomerSiteBuilderSession: customerOrder,
      SucceededOpenCustomerSiteBuilderSession: customerOrder,
      FailedOpenCustomerSiteBuilderSession: customerOrder,
      RequestedLoadCustomerSiteBuilderSession: customerOrder,
      SucceededLoadCustomerSiteBuilderSession: customerOrder,
      FailedLoadCustomerSiteBuilderSession: customerOrder,
      RequestedLoadCustomerSiteBuilderFiles: customerOrder,
      SucceededLoadCustomerSiteBuilderFiles: customerOrder,
      FailedLoadCustomerSiteBuilderFiles: customerOrder,
      SelectedCustomerSiteBuilderFile: customerOrder,
      SucceededLoadCustomerSiteBuilderFile: customerOrder,
      FailedLoadCustomerSiteBuilderFile: customerOrder,
      RequestedLoadCustomerSiteBuilderEvents: customerOrder,
      SucceededLoadCustomerSiteBuilderEvents: customerOrder,
      FailedLoadCustomerSiteBuilderEvents: customerOrder,
      RequestedLoadAdminOverview: () => updateAdmin(model, message),
      SucceededLoadAdminOverview: () => updateAdmin(model, message),
      FailedLoadAdminOverview: () => updateAdmin(model, message),
      RequestedLoadTokenUsageStats: () => updateStats(model, message),
      SucceededLoadTokenUsageStats: () => updateStats(model, message),
      FailedLoadTokenUsageStats: () => updateStats(model, message),
      UpdatedTokenUsageStatsFilter: () => updateStats(model, message),
      RequestedLoadPrefilledWorkspace: () =>
        updatePrefilledWorkspace(model, message),
      SucceededLoadPrefilledWorkspace: () =>
        updatePrefilledWorkspace(model, message),
      FailedLoadPrefilledWorkspace: () =>
        updatePrefilledWorkspace(model, message),
      RequestedGenerateAdminSite: () => updateAdmin(model, message),
      SucceededGenerateAdminSite: () => updateAdmin(model, message),
      FailedGenerateAdminSite: () => updateAdmin(model, message),
      RequestedLoadAdminAdjutantAssignments: () => updateAdmin(model, message),
      SucceededLoadAdminAdjutantAssignments: () => updateAdmin(model, message),
      FailedLoadAdminAdjutantAssignments: () => updateAdmin(model, message),
      RequestedLoadAdminAdjutantReview: () => updateAdmin(model, message),
      SucceededLoadAdminAdjutantReview: () => updateAdmin(model, message),
      FailedLoadAdminAdjutantReview: () => updateAdmin(model, message),
      RequestedRunAdminAdjutantEnrichment: () => updateAdmin(model, message),
      RequestedReviewAdminAdjutantSourceCard: () => updateAdmin(model, message),
      RequestedReviewAdminAdjutantResearchBrief: () =>
        updateAdmin(model, message),
      SucceededAdminAdjutantEnrichmentAction: () => updateAdmin(model, message),
      FailedAdminAdjutantEnrichmentAction: () => updateAdmin(model, message),
      RequestedDeployAdminSiteVersion: () => updateAdmin(model, message),
      RequestedAdminSiteDeploymentAction: () => updateAdmin(model, message),
      SucceededAdminSiteDeploymentAction: () => updateAdmin(model, message),
      FailedAdminSiteDeploymentAction: () => updateAdmin(model, message),
      RequestedLoadMulletBootstrap: () => updateMullet(model, message),
      SelectedMulletScenarioTemplate: () => updateMullet(model, message),
      SelectedMulletSensitivityAxis: () => updateMullet(model, message),
      UpdatedMulletAssumption: () => updateMullet(model, message),
      SucceededLoadMulletBootstrap: () => updateMullet(model, message),
      FailedLoadMulletBootstrap: () => updateMullet(model, message),

      UpdatedImageGenerationPrompt: () => updateImages(model, message),
      SelectedImageGenerationProvider: () => updateImages(model, message),
      SelectedImageGenerationModel: () => updateImages(model, message),
      SelectedImageGenerationAspectRatio: () => updateImages(model, message),
      SelectedImageGenerationImageSize: () => updateImages(model, message),
      UpdatedImageGenerationCount: () => updateImages(model, message),
      SubmittedImageGeneration: () => updateImages(model, message),
      SucceededGenerateImage: () => updateImages(model, message),
      FailedGenerateImage: () => updateImages(model, message),

      ClickedNewChat: () => updateRunState(model, message),
      ClickedRunMetadataInfo: () => updateRunState(model, message),
      ClosedRunMetadataInfo: () => updateRunState(model, message),
      EnteredAutopilotRunRoute: () => updateRunState(model, message),
      SucceededLaunchAutopilotRun: () => updateRunState(model, message),
      FailedLaunchAutopilotRun: () => updateRunState(model, message),
      RequestedPollAutopilotRun: () => updateRunState(model, message),
      SucceededFetchAutopilotRun: () => updateRunState(model, message),
      FailedFetchAutopilotRun: () => updateRunState(model, message),

      RequestedNotificationPermission: () =>
        updateNotifications(model, message),
      ResolvedNotificationPermission: () => updateNotifications(model, message),
      RaisedBrowserNotifications: () => updateNotifications(model, message),
      DismissedNotifications: () => updateNotifications(model, message),

      RequestedLoadAgentGoal: () => updateAgentGoals(model, message),
      SucceededLoadAgentGoal: () => updateAgentGoals(model, message),
      FailedLoadAgentGoal: () => updateAgentGoals(model, message),
      UpdatedAgentGoalObjectiveDraft: () => updateAgentGoals(model, message),
      UpdatedAgentGoalBudgetDraft: () => updateAgentGoals(model, message),
      ClickedEditAgentGoal: () => updateAgentGoals(model, message),
      ClickedCancelEditAgentGoal: () => updateAgentGoals(model, message),
      SubmittedAgentGoal: () => updateAgentGoals(model, message),
      SucceededSaveAgentGoal: () => updateAgentGoals(model, message),
      FailedSaveAgentGoal: () => updateAgentGoals(model, message),
      ClickedAgentGoalAction: () => updateAgentGoals(model, message),
      SucceededAgentGoalAction: () => updateAgentGoals(model, message),
      FailedAgentGoalAction: () => updateAgentGoals(model, message),
      RequestedLoadArtanisOperatorConsole: artanisConsole,
      SucceededLoadArtanisOperatorConsole: artanisConsole,
      FailedLoadArtanisOperatorConsole: artanisConsole,
      RequestedLoadArtanisOperatorDashboard: artanisConsole,
      SucceededLoadArtanisOperatorDashboard: artanisConsole,
      FailedLoadArtanisOperatorDashboard: artanisConsole,
      UpdatedArtanisOperatorDashboardCallerIdFilter: artanisConsole,
      SubmittedArtanisOperatorDashboardFilter: artanisConsole,
      SelectedArtanisOperatorDashboardThread: artanisConsole,
      RequestedLoadArtanisOperatorGoal: artanisConsole,
      SucceededLoadArtanisOperatorGoal: artanisConsole,
      FailedLoadArtanisOperatorGoal: artanisConsole,
      UpdatedArtanisOperatorGoalObjectiveDraft: artanisConsole,
      SubmittedArtanisOperatorGoal: artanisConsole,
      ClickedArtanisOperatorGoalAction: artanisConsole,
      SucceededSaveArtanisOperatorGoal: artanisConsole,
      FailedSaveArtanisOperatorGoal: artanisConsole,
      SucceededArtanisOperatorGoalAction: artanisConsole,
      FailedArtanisOperatorGoalAction: artanisConsole,
      ClickedArtanisOperatorApprovalAction: artanisConsole,
      SucceededArtanisOperatorApprovalAction: artanisConsole,
      FailedArtanisOperatorApprovalAction: artanisConsole,

      SubmittedChatComposer: () => updateTeamChat(model, message),
      RequestedLoadTeamChatMessages: () => updateTeamChat(model, message),
      SucceededLoadTeamChatMessages: () => updateTeamChat(model, message),
      FailedLoadTeamChatMessages: () => updateTeamChat(model, message),
      SucceededPostTeamChatMessage: () => updateTeamChat(model, message),
      FailedPostTeamChatMessage: () => updateTeamChat(model, message),

      RequestedLoadThreadFiles: () => updateThreadFiles(model, message),
      SucceededLoadThreadFiles: () => updateThreadFiles(model, message),
      FailedLoadThreadFiles: () => updateThreadFiles(model, message),
      RequestedLoadThreadFileDetail: () => updateThreadFiles(model, message),
      SucceededLoadThreadFileDetail: () => updateThreadFiles(model, message),
      FailedLoadThreadFileDetail: () => updateThreadFiles(model, message),
      ClickedThreadFileDownload: () => updateThreadFiles(model, message),
      SucceededDownloadThreadFile: () => updateThreadFiles(model, message),
      FailedDownloadThreadFile: () => updateThreadFiles(model, message),
      ClickedThreadFileDownloadToggle: () => updateThreadFiles(model, message),
      SucceededUpdateThreadFileDownload: () =>
        updateThreadFiles(model, message),
      FailedUpdateThreadFileDownload: () => updateThreadFiles(model, message),
      SubmittedThreadFileUpload: () => updateThreadFiles(model, message),
      SucceededUploadThreadFile: () => updateThreadFiles(model, message),
      FailedUploadThreadFile: () => updateThreadFiles(model, message),

      RequestedLoadSyncSnapshot: () => updateSync(model, message),
      SucceededLoadSyncSnapshot: () => updateSync(model, message),
      FailedLoadSyncSnapshot: () => updateSync(model, message),
      OpenedSyncStream: () => updateSync(model, message),
      ClosedSyncStream: () => updateSync(model, message),
      FailedSyncStream: () => updateSync(model, message),
      ReceivedSyncPatch: () => updateSync(model, message),
      ReceivedSyncCursorGap: () => updateSync(model, message),
    }),
  )
}
