const routeIntents = new Set([
  "DrawerToggled", "SettingsPressed", "SettingsSectionSelected", "FilesRouteOpened", "FilesRouteClosed",
  "ChangesRouteOpened", "GitRouteOpened", "TerminalRouteOpened", "WorkbenchConversationOpened",
  "CodingComposerTargetPickerOpened", "CodingComposerTargetPickerDismissed", "WorkspaceLifecycleSheetDismissed",
])

const selectionIntents = new Set([
  "ConversationThreadSelected", "CodingSessionSelected", "WorkspaceStatusFilterSelected",
  "WorkspaceProjectFilterSelected", "ControllerDestinationSelected", "ControllerSessionInspected",
  "SettingsSectionSelected", "EnvironmentInspected", "NotificationPreferenceToggled",
  "RepositoryChangedFileSelected", "RepositoryReviewRowSelected", "RepositoryGitBranchSelected",
  "RepositoryGitFileToggled", "RepositoryTerminalSelected", "RepositoryFileSelected",
])

const actionIntents = new Set([
  "KhalaTurnSubmitted", "RuntimeInteractionDecisionSubmitted", "RuntimeTurnControlRequested",
  "PortableControlRequested", "RepositoryReviewSubmitted", "RepositoryGitConfirmationAccepted",
  "RepositoryTerminalInterruptRequested", "RepositoryTerminalRestartRequested", "EnvironmentPairRequested",
  "EnvironmentReconnectRequested", "IncomingShareInserted",
])

const warningIntents = new Set([
  "RuntimeTurnStopConfirmationRequested", "ConversationThreadDeleteRequested",
  "RepositoryGitCommitRequested", "RepositoryGitPushRequested",
])

export type MobileNativeFeedbackKind = "none" | "selection" | "action" | "warning"

export const mobileNativeFeedbackKind = (intentName: string): MobileNativeFeedbackKind =>
  warningIntents.has(intentName) ? "warning" : actionIntents.has(intentName) ? "action" :
    selectionIntents.has(intentName) ? "selection" : "none"

export const mobileIntentUsesRouteTransition = (intentName: string): boolean => routeIntents.has(intentName)
