import { Effect, Schema, type Stream, SubscriptionRef } from "@effect-native/core/effect"
import {
  Button,
  ComponentValueBinding,
  defineIntent,
  IconButton,
  IntentRef,
  type IntentHandlers,
  type IntentReporter,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  SegmentedControl,
  Sheet,
  SplitPane,
  Spacer,
  Stack,
  StaticPayload,
  SwipeableListItem,
  Text,
  TextField,
  Toolbar,
  type View,
} from "@effect-native/core"
import {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
  resolveLiveAgentGraphSelection,
  type ScopeSyncState,
  type ConfirmedPortableSessionSnapshot,
  type ConfirmedRuntimeAttentionSnapshot,
} from "@openagentsinc/khala-sync-client"
import type { FleetRunClientProjection, FullAutoRunControlAction } from "@openagentsinc/khala-sync"
import {
  FullAutoRunLifecycleStateLabel,
  isFullAutoRunLifecycleTerminal,
  isFullAutoRunProjectionActive,
  isFullAutoRunProjectionFresh,
  truncateFullAutoRunObjective,
  type FullAutoRunLifecycleState,
  type FullAutoRunProjectionResult,
} from "../full-auto/full-auto-run-projection"
import type { FullAutoRunControlDispatchOutcome } from "../full-auto/full-auto-run-control-intent"

import type {
  MobileCodingDirectory,
  MobileCodingTarget,
} from "../coding/mobile-coding-navigation"
import {
  projectMobileControllerDirectory,
  type MobileControllerDirectory,
  type MobileControllerSession,
} from "../coding/mobile-controller-directory"
import {
  mobileCodingComposerText,
  type MobileCodingAttachmentUpdateResult,
  type MobileCodingComposerSession,
} from "../coding/mobile-coding-composer"
import type { MobileCodingAttachmentDeliveryResult } from "../coding/mobile-coding-attachment-delivery"
import type { MobileExecutionTargetOption } from "../coding/mobile-execution-targets"
import {
  normalizeMobileComposerPathQuery,
  searchMobileComposerPaths,
  type MobileComposerPathSearchPort,
} from "../coding/mobile-composer-path-context"
import {
  initialMobileRepositoryBrowserState,
  loadMobileRepositoryPreview,
  loadMobileRepositoryTree,
  type MobileRepositoryBrowserState,
  type MobileRepositoryFilesPort,
  type MobileRepositoryScope,
} from "../coding/mobile-repository-files"
import {
  decodeMobileChangeSummary,
  decodeMobileFileDiff,
  decodeMobileReviewReceipt,
  initialMobileRepositoryReviewState,
  MOBILE_REVIEW_COMMENT_MAX,
  type MobileRepositoryReviewPort,
  type MobileRepositoryReviewState,
} from "../coding/mobile-repository-review"
import {
  decodeMobileGitMutationResult,
  decodeMobileGitStatus,
  initialMobileRepositoryGitState,
  MOBILE_GIT_COMMIT_MESSAGE_MAX,
  type MobileGitMutationRequest,
  type MobileRepositoryGitPort,
  type MobileRepositoryGitState,
} from "../coding/mobile-repository-git"
import {
  applyMobileTerminalReplay,
  decodeMobileTerminalCommandReceipt,
  decodeMobileTerminalReplay,
  decodeMobileTerminalSnapshot,
  initialMobileRepositoryTerminalState,
  MOBILE_TERMINAL_MAX_INPUT_BYTES,
  type MobileRepositoryTerminalPort,
  type MobileRepositoryTerminalState,
  type MobileTerminalCommand,
} from "../coding/mobile-repository-terminal"
import {
  projectMobilePortableSessionControl,
  type MobilePortableControlAction,
  type MobilePortableSessionControl,
  type MobilePortableUnavailableReason,
} from "../coding/mobile-portable-session-controls"
import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
  MobileConversationThreadSummary,
  MobileRuntimeControlAction,
} from "../conversation/mobile-conversation"
import {
  MobileAttentionTargetSchemaVersion,
  resolveMobileAttentionTarget,
  type MobileAttentionTarget,
} from "../attention/mobile-attention-target"
import {
  MOBILE_WORKSPACE_MAX_SEARCH,
  projectMobileWorkspaceNavigation,
  type MobileWorkspaceRow,
  type MobileWorkspaceStatusFilter,
} from "./mobile-workspace-navigation"
import {
  clampMobileWorkspaceSidebar,
  mobileWorkspaceActiveDescendant,
  mobileWorkspaceLayoutMode,
  MOBILE_WORKSPACE_SIDEBAR_DEFAULT,
  MOBILE_WORKSPACE_SIDEBAR_MAX,
  MOBILE_WORKSPACE_SIDEBAR_MIN,
  type MobileWorkspaceFocusTarget,
  type MobileWorkspaceLayoutMode,
} from "./mobile-adaptive-workspace"
import type { MobileWorkspaceKeyboardCommand } from "./mobile-workspace-keyboard"
import { renderMobileFilesView } from "./mobile-files-view"
import { renderMobileChangesView } from "./mobile-changes-view"
import { renderMobileGitView } from "./mobile-git-view"
import { renderMobileTerminalView } from "./mobile-terminal-view"
import { renderMobileSettingsView } from "./mobile-settings-view"
import {
  decodeMobileEnvironmentDirectory,
  decodeMobileEnvironmentReceipt,
  initialMobileSettingsState,
  mobileShareComposerText,
  normalizeMobilePairingCode,
  type MobileEnvironmentConnectionsPort,
  type MobileNotificationSettingsPort,
  type MobileSettingsSection,
  type MobileSettingsState,
  type MobileShareIntake,
} from "../settings/mobile-settings"

import {
  AgentRowSelected,
  AgentStackToggled,
  initialKhalaState,
  khalaHandlers,
  khalaIntentDefinitions,
  defaultMobileAccessibilityProfile,
  MOBILE_AGENT_GRAPH_MAX_ROWS,
  mobileInteractiveStyle,
  normalizeMobileAccessibilityProfile,
  renderKhalaSurface,
  TranscriptAttachmentLoadSettled,
  TranscriptAttachmentOpened,
  TranscriptAttachmentRetryRequested,
  TranscriptAttachmentViewerDismissed,
  TranscriptEarlierHistoryRequested,
  TranscriptJumpToLatestRequested,
  TranscriptPinnedChanged,
  WorkGroupToggled,
  WorkItemToggled,
  type FullAutoRunHeaderView,
  type KhalaState,
  type KhalaTurnClient,
  type MobileAccessibilityProfile,
} from "./khala-core"
import type { SarahPrincipalProjection } from "@openagentsinc/sarah"
import { mobileAttachmentRef } from "./mobile-transcript-attachment"
import {
  initialMobileTranscriptVisibleCount,
  newlyConfirmedTranscriptEntryCount,
  nextMobileTranscriptVisibleCount,
} from "./mobile-transcript-history"
import { projectMobileWorkGroup } from "./mobile-work-log"
import {
  mobileComposerSlashTrigger,
  mobileComposerPathTrigger,
  mobileSlashCommandIds,
  mobileSlashCommands,
  type MobileSlashCommandContext,
  type MobileComposerPathDiscoveryState,
} from "./mobile-composer-discovery"

export {
  defaultMobileAccessibilityProfile,
  normalizeMobileAccessibilityProfile,
  type MobileAccessibilityProfile,
} from "./khala-core"

/**
 * Conversation-first mobile home. Sarah re-enters only as the authenticated
 * owner's durable orchestrator projection inside this existing shell; there is
 * no standalone persona app, presentation demo, or public front door. Confirmed
 * personal Sync remains the authority for hosted conversation state.
 */
export type SurfaceMode = "openagents" | "khala"

export interface SurfaceModeOption {
  readonly id: SurfaceMode
  readonly label: string
}

export const surfaceModeOptions: ReadonlyArray<SurfaceModeOption> = [
  { id: "openagents", label: "OpenAgents" },
  { id: "khala", label: "Khala" },
]

export interface HomeState {
  readonly drawerOpen: boolean
  readonly workspaceLayoutMode: MobileWorkspaceLayoutMode
  readonly workspaceSidebarWidth: number
  readonly workspaceSidebarCollapsed: boolean
  readonly workspaceFocusTarget: MobileWorkspaceFocusTarget
  readonly surfaceMode: SurfaceMode
  readonly workbenchRoute: "conversation" | "files" | "changes" | "git" | "terminal" | "settings"
  readonly repositoryBrowser: MobileRepositoryBrowserState
  readonly repositoryReview: MobileRepositoryReviewState
  readonly repositoryGit: MobileRepositoryGitState
  readonly repositoryTerminal: MobileRepositoryTerminalState
  readonly settings: MobileSettingsState
  readonly modeMenuOpen: boolean
  readonly syncPhase: MobileSyncPhase
  readonly conversationAuthority: "local" | "sync"
  readonly conversationThreads: ReadonlyArray<MobileConversationThreadSummary>
  readonly archivedConversationThreads: ReadonlyArray<MobileConversationThreadSummary>
  readonly activeThreadRef: string | null
  readonly sarah: SarahPrincipalProjection | null
  readonly workspaceSearch: string
  readonly workspaceStatusFilter: MobileWorkspaceStatusFilter
  readonly workspaceProjectFilter: string | null
  readonly threadLifecycle: Readonly<{
    actionThreadRef: string | null
    editingThreadRef: string | null
    renameDraft: string
    deleteConfirmThreadRef: string | null
    pendingAction: "archive" | "delete" | "rename" | "restore" | null
    notice: Readonly<{ kind: "confirmed" | "rejected"; message: string }> | null
  }>
  readonly codingDirectory: MobileCodingDirectory | null
  readonly controllerDestination: MobileControllerDestination
  readonly inspectedControllerSessionRef: string | null
  readonly portableSnapshot: ConfirmedPortableSessionSnapshot | null
  readonly attentionSnapshot: ConfirmedRuntimeAttentionSnapshot | null
  readonly selectedPortableDestinationRef: string | null
  readonly portableSubmittingAction: MobilePortableControlAction | null
  readonly portableNotice: Readonly<{
    kind: "queued" | "rejected"
    message: string
  }> | null
  readonly attentionNotice: string | null
  readonly codingComposer: MobileCodingComposerSession | null
  readonly codingComposerTargetPickerOpen: boolean
  readonly codingComposerTargetSearch: string
  readonly codingPathDiscovery: MobileComposerPathDiscoveryState
  readonly codingExecutionTargets: ReadonlyArray<MobileExecutionTargetOption>
  readonly fleetRuns?: FleetRunClientProjection
  /** Live `FullAutoRun` mobile projection (openagents #8982); `null` means
   * no fetch has resolved yet or the account has no active run. The state
   * header renders only when this projection's `threadRef` matches
   * `activeThreadRef` and the run is still active/fresh — see
   * `fullAutoRunHeaderForState`. */
  readonly fullAutoRun: FullAutoRunProjectionResult | null
  /** MOB-FA-02 (#8994): the action currently in flight from THIS device
   * (dispatched, awaiting a durable applied/rejected outcome), or `null`.
   * Never optimistic completion -- the header renders "Pausing…" etc. while
   * this is set and only clears once `fullAutoControlOutcome` resolves. */
  readonly fullAutoControlPending: FullAutoRunControlAction | null
  /** MOB-FA-02 (#8994): the most recent typed outcome for this device's
   * last dispatched control intent (applied/rejected/pending/unauthorized/
   * unavailable), surfaced honestly rather than assumed from silence. */
  readonly fullAutoControlOutcome: FullAutoRunControlDispatchOutcome | null
  readonly codingExecutionTargetCatalogRequired: boolean
  readonly codingAttachmentPicking: boolean
  readonly codingAttachmentMutatingRef: string | null
  readonly codingAttachmentStatus: Readonly<{
    kind: "ready" | "failed"
    message: string
  }> | null
  readonly accessibility: MobileAccessibilityProfile
  readonly khala: KhalaState
}

export type MobileSyncPhase =
  | ScopeSyncState["phase"]
  | "authenticating"
  | "credential_present_unverified"
  | "local_ready"
  | "session_ready"
  | "unconfigured"
  | "unavailable"
  | "stale"

export type MobileControllerDestination = "recent" | "repositories" | "attention"

export interface SyncStatusCopy {
  readonly title: string
  readonly detail: string
}

const syncStatusCopyByPhase: Record<MobileSyncPhase, SyncStatusCopy> = {
  authenticating: {
    title: "Updating session",
    detail: "Complete the secure browser step. Shared work stays hidden until OpenAgents verifies the session.",
  },
  credential_present_unverified: {
    title: "Session verification required",
    detail: "Stored credentials remain private. Shared work stays hidden until the server verifies them.",
  },
  local_ready: {
    title: "Local device ready",
    detail: "Coding, conversations, and fleets work without an account. Link OpenAgents only for cross-device Sync and network features.",
  },
  session_ready: {
    title: "Session verified",
    detail: "OpenAgents accepted this session. Shared work is ready to connect.",
  },
  unconfigured: {
    title: "Sync not configured",
    detail: "Connect an OpenAgents session to view shared work, repositories, and Fleet state.",
  },
  idle: {
    title: "Sync idle",
    detail: "Sync is ready to connect. Shared work is not loaded yet.",
  },
  bootstrapping: {
    title: "Loading shared work",
    detail: "Fetching the current authorized projection.",
  },
  catching_up: {
    title: "Catching up",
    detail: "Applying confirmed updates before shared work is ready.",
  },
  live: {
    title: "Sync live",
    detail: "Shared work is current.",
  },
  stale: {
    title: "Sync stale",
    detail: "Shared work may be outdated. Controls stay unavailable until it reconnects.",
  },
  must_refetch: {
    title: "Sync needs refresh",
    detail: "The authorized projection must be fetched again before it can be used.",
  },
  denied: {
    title: "Sync access removed",
    detail: "This device can no longer show shared work for the previous session.",
  },
  unavailable: {
    title: "Sync unavailable",
    detail: "Shared work cannot be loaded right now.",
  },
}

export const syncStatusCopy = (phase: MobileSyncPhase): SyncStatusCopy => syncStatusCopyByPhase[phase]

export type MobileAccountControl = "sign_in" | "sign_out" | "none"

/**
 * Every confirmed post-authentication phase — a verified session that has
 * connected and is bootstrapping/catching up/live, plus the degraded-but-still-
 * linked stale/must-refetch states — is a linked account and shows "Sign out".
 * Only genuinely unauthenticated phases (never linked, signed out, denied, or
 * unverified) show the "Link OpenAgents account" sign-in control, and an
 * in-flight browser step shows neither.
 */
const authenticatedAccountPhases: ReadonlySet<MobileSyncPhase> = new Set<MobileSyncPhase>([
  "session_ready",
  "bootstrapping",
  "catching_up",
  "live",
  "must_refetch",
  "stale",
])

export const mobileAccountControl = (phase: MobileSyncPhase): MobileAccountControl =>
  phase === "authenticating"
    ? "none"
    : authenticatedAccountPhases.has(phase)
      ? "sign_out"
      : "sign_in"

export const initialHomeState: HomeState = {
  drawerOpen: false,
  workspaceLayoutMode: "compact",
  workspaceSidebarWidth: MOBILE_WORKSPACE_SIDEBAR_DEFAULT,
  workspaceSidebarCollapsed: false,
  workspaceFocusTarget: "transcript",
  surfaceMode: "khala",
  workbenchRoute: "conversation",
  repositoryBrowser: initialMobileRepositoryBrowserState,
  repositoryReview: initialMobileRepositoryReviewState,
  repositoryGit: initialMobileRepositoryGitState,
  repositoryTerminal: initialMobileRepositoryTerminalState,
  settings: initialMobileSettingsState,
  modeMenuOpen: false,
  syncPhase: "unconfigured",
  conversationAuthority: "local",
  conversationThreads: [],
  archivedConversationThreads: [],
  activeThreadRef: null,
  sarah: null,
  workspaceSearch: "",
  workspaceStatusFilter: "all",
  workspaceProjectFilter: null,
  threadLifecycle: {
    actionThreadRef: null,
    editingThreadRef: null,
    renameDraft: "",
    deleteConfirmThreadRef: null,
    pendingAction: null,
    notice: null,
  },
  codingDirectory: null,
  controllerDestination: "recent",
  inspectedControllerSessionRef: null,
  portableSnapshot: null,
  attentionSnapshot: null,
  selectedPortableDestinationRef: null,
  portableSubmittingAction: null,
  portableNotice: null,
  attentionNotice: null,
  codingComposer: null,
  codingComposerTargetPickerOpen: false,
  codingComposerTargetSearch: "",
  codingPathDiscovery: { state: "idle" },
  codingExecutionTargets: [],
  fullAutoRun: null,
  fullAutoControlPending: null,
  fullAutoControlOutcome: null,
  codingExecutionTargetCatalogRequired: false,
  codingAttachmentPicking: false,
  codingAttachmentMutatingRef: null,
  codingAttachmentStatus: null,
  accessibility: defaultMobileAccessibilityProfile,
  khala: initialKhalaState,
}

/** Visible OTA tag for the authenticated owner-orchestrator reboot. */
export const BUNDLE_TAG = "2026-07-18.sarah-owner-orchestrator-05"

const EmptyPayload = Schema.Struct({})

export const DrawerToggled = defineIntent("DrawerToggled", EmptyPayload)
export const WorkspaceSidebarResized = defineIntent(
  "WorkspaceSidebarResized",
  Schema.Struct({ paneId: Schema.String, size: Schema.Number }),
)
export const WorkspaceViewportWidthChanged = defineIntent(
  "WorkspaceViewportWidthChanged",
  Schema.Number,
)
export const WorkspaceKeyboardCommandReceived = defineIntent(
  "WorkspaceKeyboardCommandReceived",
  Schema.Literals(["new_task", "navigation", "detail", "dismiss"]),
)
export const NewChatPressed = defineIntent("NewChatPressed", EmptyPayload)
export const SettingsPressed = defineIntent("SettingsPressed", EmptyPayload)
export const SettingsSectionSelected = defineIntent("SettingsSectionSelected", Schema.Struct({
  section: Schema.Literals(["root", "account", "environments", "notifications", "appearance", "accessibility", "storage", "diagnostics", "legal", "share"]),
}))
export const EnvironmentDirectoryRequested = defineIntent("EnvironmentDirectoryRequested", EmptyPayload)
export const EnvironmentPairingCodeChanged = defineIntent("EnvironmentPairingCodeChanged", Schema.String)
export const EnvironmentPairRequested = defineIntent("EnvironmentPairRequested", EmptyPayload)
export const EnvironmentInspected = defineIntent("EnvironmentInspected", Schema.Struct({ environmentRef: Schema.String }))
export const EnvironmentReconnectRequested = defineIntent("EnvironmentReconnectRequested", Schema.Struct({ environmentRef: Schema.String }))
export const NotificationPermissionRequested = defineIntent("NotificationPermissionRequested", EmptyPayload)
export const NotificationPreferenceToggled = defineIntent("NotificationPreferenceToggled", Schema.Struct({
  preference: Schema.Literals(["attention", "completion", "approvals"]),
}))
export const IncomingShareInserted = defineIntent("IncomingShareInserted", EmptyPayload)
export const IncomingShareDismissed = defineIntent("IncomingShareDismissed", EmptyPayload)
export const OpenAgentsSignInPressed = defineIntent("OpenAgentsSignInPressed", EmptyPayload)
export const OpenAgentsSignOutPressed = defineIntent("OpenAgentsSignOutPressed", EmptyPayload)
export const SurfaceModeSelected = defineIntent(
  "SurfaceModeSelected",
  Schema.Struct({ mode: Schema.Literals(["openagents", "khala"]) }),
)
export const ConversationThreadSelected = defineIntent(
  "ConversationThreadSelected",
  Schema.Struct({ threadRef: Schema.String }),
)
export const WorkspaceSearchChanged = defineIntent(
  "WorkspaceSearchChanged",
  Schema.String,
)
export const WorkspaceStatusFilterSelected = defineIntent(
  "WorkspaceStatusFilterSelected",
  Schema.Literals(["all", "active", "attention", "idle", "archived"]),
)
export const WorkspaceProjectFilterSelected = defineIntent(
  "WorkspaceProjectFilterSelected",
  Schema.String,
)
export const WorkspaceFiltersCleared = defineIntent("WorkspaceFiltersCleared", EmptyPayload)
export const WorkspaceRowActionsToggled = defineIntent(
  "WorkspaceRowActionsToggled",
  Schema.Struct({ threadRef: Schema.String }),
)
export const WorkspaceLifecycleSheetDismissed = defineIntent(
  "WorkspaceLifecycleSheetDismissed",
  EmptyPayload,
)
export const WorkspaceRowActionSelected = defineIntent(
  "WorkspaceRowActionSelected",
  Schema.String,
)
export const ConversationThreadRenameStarted = defineIntent(
  "ConversationThreadRenameStarted",
  Schema.Struct({ threadRef: Schema.String }),
)
export const ConversationThreadRenameChanged = defineIntent(
  "ConversationThreadRenameChanged",
  Schema.String,
)
export const ConversationThreadRenameSubmitted = defineIntent(
  "ConversationThreadRenameSubmitted",
  EmptyPayload,
)
export const ConversationThreadRenameCancelled = defineIntent(
  "ConversationThreadRenameCancelled",
  EmptyPayload,
)
export const ConversationThreadLifecycleRequested = defineIntent(
  "ConversationThreadLifecycleRequested",
  Schema.Struct({
    action: Schema.Literals(["archive", "restore"]),
    threadRef: Schema.String,
  }),
)
export const ConversationThreadDeleteRequested = defineIntent(
  "ConversationThreadDeleteRequested",
  Schema.Struct({ threadRef: Schema.String }),
)
export const ConversationThreadDeleteConfirmed = defineIntent(
  "ConversationThreadDeleteConfirmed",
  EmptyPayload,
)
export const ConversationThreadDeleteCancelled = defineIntent(
  "ConversationThreadDeleteCancelled",
  EmptyPayload,
)
export const CodingSessionSelected = defineIntent(
  "CodingSessionSelected",
  Schema.Struct({
    repositoryRef: Schema.String,
    sessionRef: Schema.String,
    threadRef: Schema.String,
  }),
)
export const ControllerDestinationSelected = defineIntent(
  "ControllerDestinationSelected",
  Schema.Literals(["recent", "repositories", "attention"]),
)
export const ControllerSessionInspected = defineIntent(
  "ControllerSessionInspected",
  Schema.Struct({ sessionRef: Schema.String }),
)
export const ControllerAttentionSelected = defineIntent(
  "ControllerAttentionSelected",
  Schema.Struct({
    schema: Schema.Literal(MobileAttentionTargetSchemaVersion),
    attentionRef: Schema.String,
    threadRef: Schema.String,
    turnRef: Schema.String,
  }),
)
export const PortableDestinationSelected = defineIntent(
  "PortableDestinationSelected",
  Schema.String,
)
export const PortableControlRequested = defineIntent(
  "PortableControlRequested",
  Schema.Struct({ action: Schema.Literals(["stop", "checkpoint", "move", "resume", "failback"]) }),
)
export const CodingComposerAttachmentsRequested = defineIntent(
  "CodingComposerAttachmentsRequested",
  EmptyPayload,
)
export const CodingComposerAttachmentRemoved = defineIntent(
  "CodingComposerAttachmentRemoved",
  Schema.Struct({ attachmentId: Schema.String }),
)
export const CodingComposerAttachmentRetryRequested = defineIntent(
  "CodingComposerAttachmentRetryRequested",
  Schema.Struct({ attachmentId: Schema.String }),
)
export const CodingExecutionTargetSelected = defineIntent(
  "CodingExecutionTargetSelected",
  Schema.Struct({ targetId: Schema.String }),
)
export const CodingComposerTargetPickerOpened = defineIntent(
  "CodingComposerTargetPickerOpened",
  EmptyPayload,
)
export const CodingComposerTargetPickerDismissed = defineIntent(
  "CodingComposerTargetPickerDismissed",
  EmptyPayload,
)
export const CodingComposerTargetSearchChanged = defineIntent(
  "CodingComposerTargetSearchChanged",
  Schema.String,
)
export const CodingComposerSlashQueryChanged = defineIntent(
  "CodingComposerSlashQueryChanged",
  Schema.String,
)
export const CodingComposerSlashCommandSelected = defineIntent(
  "CodingComposerSlashCommandSelected",
  Schema.Literals(mobileSlashCommandIds),
)
export const CodingComposerPathQueryChanged = defineIntent(
  "CodingComposerPathQueryChanged",
  Schema.String,
)
export const CodingComposerPathSelected = defineIntent(
  "CodingComposerPathSelected",
  Schema.String,
)
export const ChangesRouteOpened = defineIntent("ChangesRouteOpened", EmptyPayload)
export const WorkbenchConversationOpened = defineIntent("WorkbenchConversationOpened", EmptyPayload)
export const RepositoryChangesRefreshed = defineIntent("RepositoryChangesRefreshed", EmptyPayload)
export const RepositoryChangedFileSelected = defineIntent("RepositoryChangedFileSelected", Schema.Struct({
  pathRef: Schema.String,
  source: Schema.Literals(["staged", "unstaged", "untracked"]),
  revisionRef: Schema.String,
}))
export const RepositoryReviewRowSelected = defineIntent(
  "RepositoryReviewRowSelected",
  Schema.Struct({ rowId: Schema.String }),
)
export const RepositoryReviewCommentChanged = defineIntent("RepositoryReviewCommentChanged", Schema.String)
export const RepositoryReviewSubmitted = defineIntent("RepositoryReviewSubmitted", EmptyPayload)
export const GitRouteOpened = defineIntent("GitRouteOpened", EmptyPayload)
export const RepositoryGitRefreshed = defineIntent("RepositoryGitRefreshed", EmptyPayload)
export const RepositoryGitBranchSelected = defineIntent("RepositoryGitBranchSelected", Schema.Struct({
  branchRef: Schema.String,
  name: Schema.String,
}))
export const RepositoryGitFileToggled = defineIntent("RepositoryGitFileToggled", Schema.Struct({ pathRef: Schema.String }))
export const RepositoryGitCommitMessageChanged = defineIntent("RepositoryGitCommitMessageChanged", Schema.String)
export const RepositoryGitCommitRequested = defineIntent("RepositoryGitCommitRequested", EmptyPayload)
export const RepositoryGitPushRequested = defineIntent("RepositoryGitPushRequested", EmptyPayload)
export const RepositoryGitConfirmationCancelled = defineIntent("RepositoryGitConfirmationCancelled", EmptyPayload)
export const RepositoryGitConfirmationAccepted = defineIntent("RepositoryGitConfirmationAccepted", EmptyPayload)
export const TerminalRouteOpened = defineIntent("TerminalRouteOpened", EmptyPayload)
export const RepositoryTerminalRefreshed = defineIntent("RepositoryTerminalRefreshed", EmptyPayload)
export const RepositoryTerminalForegrounded = defineIntent("RepositoryTerminalForegrounded", EmptyPayload)
export const RepositoryTerminalCreateRequested = defineIntent("RepositoryTerminalCreateRequested", EmptyPayload)
export const RepositoryTerminalSelected = defineIntent("RepositoryTerminalSelected", Schema.Struct({ terminalRef: Schema.String }))
export const RepositoryTerminalHostEvent = defineIntent("RepositoryTerminalHostEvent", Schema.Union([
  Schema.Struct({ type: Schema.Literal("data"), data: Schema.String }),
  Schema.Struct({ type: Schema.Literal("resize"), cols: Schema.Number, rows: Schema.Number }),
]))
export const RepositoryTerminalAccessoryKeyPressed = defineIntent("RepositoryTerminalAccessoryKeyPressed", Schema.Struct({ data: Schema.String }))
export const RepositoryTerminalInterruptRequested = defineIntent("RepositoryTerminalInterruptRequested", EmptyPayload)
export const RepositoryTerminalRestartRequested = defineIntent("RepositoryTerminalRestartRequested", EmptyPayload)
export const RepositoryTerminalCloseRequested = defineIntent("RepositoryTerminalCloseRequested", EmptyPayload)
export const FilesRouteOpened = defineIntent("FilesRouteOpened", EmptyPayload)
export const FilesRouteClosed = defineIntent("FilesRouteClosed", EmptyPayload)
export const RepositoryDirectoryToggled = defineIntent(
  "RepositoryDirectoryToggled",
  Schema.Struct({ pathRef: Schema.String, revisionRef: Schema.String }),
)
export const RepositoryFileSelected = defineIntent(
  "RepositoryFileSelected",
  Schema.Struct({ pathRef: Schema.String, revisionRef: Schema.String }),
)
export const RepositoryFilesRefreshed = defineIntent("RepositoryFilesRefreshed", EmptyPayload)
export const RuntimeInteractionOptionToggled = defineIntent(
  "RuntimeInteractionOptionToggled",
  Schema.Struct({
    interactionRef: Schema.String,
    questionRef: Schema.String,
    optionRef: Schema.String,
    multiSelect: Schema.Boolean,
  }),
)
export const RuntimeInteractionDecisionSubmitted = defineIntent(
  "RuntimeInteractionDecisionSubmitted",
  Schema.Union([
    Schema.Struct({ interactionRef: Schema.String, turnRef: Schema.String, kind: Schema.Literal("provider_question") }),
    Schema.Struct({ interactionRef: Schema.String, turnRef: Schema.String, kind: Schema.Literal("tool_approval"), outcome: Schema.Literals(["approve", "deny"]) }),
    Schema.Struct({ interactionRef: Schema.String, turnRef: Schema.String, kind: Schema.Literal("plan_review"), outcome: Schema.Literals(["accept", "request_changes", "replan"]) }),
  ]),
)
export const RuntimeTurnControlRequested = defineIntent(
  "RuntimeTurnControlRequested",
  Schema.Struct({
    action: Schema.Literals(["cancel", "close", "resume", "retry"]),
    runRef: Schema.String,
  }),
)
export const RuntimeTurnStopConfirmationRequested = defineIntent(
  "RuntimeTurnStopConfirmationRequested",
  Schema.Struct({ runRef: Schema.String }),
)
export const RuntimeTurnStopConfirmationDismissed = defineIntent(
  "RuntimeTurnStopConfirmationDismissed",
  Schema.Struct({ runRef: Schema.String }),
)
export const RuntimeTurnStopConfirmed = defineIntent(
  "RuntimeTurnStopConfirmed",
  Schema.Struct({ runRef: Schema.String }),
)
/** MOB-FA-02 (#8994): a phone-dispatched Pause/Resume/Stop against a
 * Desktop-owned FullAutoRun -- a SEPARATE domain from
 * `RuntimeTurnControlRequested` above (that's Khala chat-turn control;
 * this is the Full Auto run lifecycle). Server-mediated and eventually
 * consistent: the handler dispatches through `options.fullAutoControl`
 * (`makeFullAutoRunControlDispatcher`), which durably records the intent
 * and polls for a receipted `applied`/`rejected` outcome -- the UI never
 * completes from an optimistic guess. */
export const FullAutoRunControlRequested = defineIntent(
  "FullAutoRunControlRequested",
  Schema.Struct({
    action: Schema.Literals(["pause", "resume", "stop"]),
    runRef: Schema.String,
  }),
)
export const AgentStackToggledIntent = defineIntent(AgentStackToggled, EmptyPayload)
export const AgentRowSelectedIntent = defineIntent(
  AgentRowSelected,
  Schema.Struct({ agentRef: Schema.String }),
)
export const WorkGroupToggledIntent = defineIntent(
  WorkGroupToggled,
  Schema.Struct({ groupRef: Schema.String }),
)
export const WorkItemToggledIntent = defineIntent(
  WorkItemToggled,
  Schema.Struct({ itemRef: Schema.String }),
)
export const TranscriptPinnedChangedIntent = defineIntent(
  TranscriptPinnedChanged,
  Schema.Boolean,
)
export const TranscriptEarlierHistoryRequestedIntent = defineIntent(
  TranscriptEarlierHistoryRequested,
  EmptyPayload,
)
export const TranscriptJumpToLatestRequestedIntent = defineIntent(
  TranscriptJumpToLatestRequested,
  EmptyPayload,
)
export const TranscriptAttachmentOpenedIntent = defineIntent(
  TranscriptAttachmentOpened,
  Schema.Struct({ attachmentRef: Schema.String }),
)
export const TranscriptAttachmentLoadSettledIntent = defineIntent(
  TranscriptAttachmentLoadSettled,
  Schema.Struct({ attachmentRef: Schema.String, outcome: Schema.Literals(["ready", "failed"]) }),
)
export const TranscriptAttachmentRetryRequestedIntent = defineIntent(
  TranscriptAttachmentRetryRequested,
  Schema.Struct({ attachmentRef: Schema.String }),
)
export const TranscriptAttachmentViewerDismissedIntent = defineIntent(
  TranscriptAttachmentViewerDismissed,
  Schema.Struct({ attachmentRef: Schema.String }),
)

export const homeIntentDefinitions = [
  DrawerToggled,
  WorkspaceSidebarResized,
  WorkspaceViewportWidthChanged,
  WorkspaceKeyboardCommandReceived,
  NewChatPressed,
  SettingsPressed,
  SettingsSectionSelected,
  EnvironmentDirectoryRequested,
  EnvironmentPairingCodeChanged,
  EnvironmentPairRequested,
  EnvironmentInspected,
  EnvironmentReconnectRequested,
  NotificationPermissionRequested,
  NotificationPreferenceToggled,
  IncomingShareInserted,
  IncomingShareDismissed,
  OpenAgentsSignInPressed,
  OpenAgentsSignOutPressed,
  SurfaceModeSelected,
  ConversationThreadSelected,
  WorkspaceSearchChanged,
  WorkspaceStatusFilterSelected,
  WorkspaceProjectFilterSelected,
  WorkspaceFiltersCleared,
  WorkspaceRowActionsToggled,
  WorkspaceLifecycleSheetDismissed,
  WorkspaceRowActionSelected,
  ConversationThreadRenameStarted,
  ConversationThreadRenameChanged,
  ConversationThreadRenameSubmitted,
  ConversationThreadRenameCancelled,
  ConversationThreadLifecycleRequested,
  ConversationThreadDeleteRequested,
  ConversationThreadDeleteConfirmed,
  ConversationThreadDeleteCancelled,
  CodingSessionSelected,
  ControllerDestinationSelected,
  ControllerSessionInspected,
  ControllerAttentionSelected,
  PortableDestinationSelected,
  PortableControlRequested,
  CodingComposerAttachmentsRequested,
  CodingComposerAttachmentRemoved,
  CodingComposerAttachmentRetryRequested,
  CodingExecutionTargetSelected,
  CodingComposerTargetPickerOpened,
  CodingComposerTargetPickerDismissed,
  CodingComposerTargetSearchChanged,
  CodingComposerSlashQueryChanged,
  CodingComposerSlashCommandSelected,
  CodingComposerPathQueryChanged,
  CodingComposerPathSelected,
  ChangesRouteOpened,
  WorkbenchConversationOpened,
  RepositoryChangesRefreshed,
  RepositoryChangedFileSelected,
  RepositoryReviewRowSelected,
  RepositoryReviewCommentChanged,
  RepositoryReviewSubmitted,
  GitRouteOpened,
  RepositoryGitRefreshed,
  RepositoryGitBranchSelected,
  RepositoryGitFileToggled,
  RepositoryGitCommitMessageChanged,
  RepositoryGitCommitRequested,
  RepositoryGitPushRequested,
  RepositoryGitConfirmationCancelled,
  RepositoryGitConfirmationAccepted,
  TerminalRouteOpened,
  RepositoryTerminalRefreshed,
  RepositoryTerminalForegrounded,
  RepositoryTerminalCreateRequested,
  RepositoryTerminalSelected,
  RepositoryTerminalHostEvent,
  RepositoryTerminalAccessoryKeyPressed,
  RepositoryTerminalInterruptRequested,
  RepositoryTerminalRestartRequested,
  RepositoryTerminalCloseRequested,
  FilesRouteOpened,
  FilesRouteClosed,
  RepositoryDirectoryToggled,
  RepositoryFileSelected,
  RepositoryFilesRefreshed,
  RuntimeInteractionOptionToggled,
  RuntimeInteractionDecisionSubmitted,
  RuntimeTurnControlRequested,
  RuntimeTurnStopConfirmationRequested,
  RuntimeTurnStopConfirmationDismissed,
  RuntimeTurnStopConfirmed,
  FullAutoRunControlRequested,
  AgentStackToggledIntent,
  AgentRowSelectedIntent,
  WorkGroupToggledIntent,
  WorkItemToggledIntent,
  TranscriptPinnedChangedIntent,
  TranscriptEarlierHistoryRequestedIntent,
  TranscriptJumpToLatestRequestedIntent,
  TranscriptAttachmentOpenedIntent,
  TranscriptAttachmentLoadSettledIntent,
  TranscriptAttachmentRetryRequestedIntent,
  TranscriptAttachmentViewerDismissedIntent,
  ...khalaIntentDefinitions.map((definition) => defineIntent(definition.name, definition.payload)),
] as const

export interface ChromeProps {
  readonly pillLabel: string
  readonly composerPlaceholder: string
  readonly chromeVisible: boolean
  readonly glassComposerVisible: boolean
  readonly surfaceMode: SurfaceMode
  readonly draft: string
  readonly sending: boolean
}

export interface MobileHeaderProps {
  readonly title: string
  readonly subtitle: string | null
}

/**
 * T3-style compact thread identity for the native mobile header. The header
 * stays projection-only: titles and repository/worktree context come from the
 * already-confirmed conversation/composer state and never become navigation
 * authority of their own.
 */
export const mobileHeaderProps = (state: HomeState): MobileHeaderProps => {
  if (state.workbenchRoute === "settings") {
    return { title: "Settings", subtitle: state.settings.section === "root" ? "OpenAgents mobile" : state.settings.section }
  }
  if (state.workbenchRoute === "terminal") {
    const active = state.repositoryTerminal.sessions.find(session => session.terminalRef === state.repositoryTerminal.activeRef)
    return { title: "Terminal", subtitle: active?.label ?? "Exact worktree" }
  }
  if (state.workbenchRoute === "git") {
    const status = state.repositoryGit.status
    return { title: "Git", subtitle: status?.branch ?? (status?.detached ? "Detached HEAD" : "Exact worktree") }
  }
  if (state.workbenchRoute === "changes") {
    const scope = state.repositoryReview.scope
    return { title: "Changes", subtitle: scope === null ? "Review workbench" : `${scope.repositoryRef} · ${scope.worktreeRef}` }
  }
  if (state.workbenchRoute === "files") {
    const scope = state.repositoryBrowser.scope
    return {
      title: "Files",
      subtitle: scope === null ? "Repository workbench" : `${scope.repositoryRef} · ${scope.worktreeRef}`,
    }
  }
  if (state.surfaceMode === "openagents") {
    return { title: "OpenAgents", subtitle: syncStatusCopy(state.syncPhase).title }
  }

  if (state.sarah !== null && state.activeThreadRef === state.sarah.threadRef) {
    return {
      title: state.sarah.displayName,
      subtitle: null,
    }
  }

  const selectedTitle = state.khala.threadHistory?.title ??
    state.conversationThreads.find(thread => thread.threadRef === state.activeThreadRef)?.title
  const composer = state.codingComposer
  return {
    title: selectedTitle ?? (state.conversationAuthority === "sync" ? "OpenAgents" : "Khala"),
    subtitle: composer === null
      ? null
      : `${composer.repositoryLabel} · ${composer.worktreeLabel}`,
  }
}

const sarahIsActive = (state: HomeState): boolean =>
  state.sarah !== null && state.activeThreadRef === state.sarah.threadRef

/** MOB-FA-02 (#8994): the control actions legal from a given lifecycle
 * state, in display order. Mirrors Desktop's exact legality (Pause is legal
 * only from `running`, Resume only from `paused`, Stop from any
 * non-terminal, non-draft state) -- never presents a button whose tap would
 * just come back rejected as `illegal_transition`. */
const fullAutoRunControlActionsFor = (
  lifecycleState: FullAutoRunLifecycleState,
): ReadonlyArray<FullAutoRunControlAction> => {
  const actions: Array<FullAutoRunControlAction> = []
  if (lifecycleState === "running") actions.push("pause")
  if (lifecycleState === "paused") actions.push("resume")
  if (lifecycleState !== "draft" && !isFullAutoRunLifecycleTerminal(lifecycleState)) actions.push("stop")
  return actions
}

/** MOB-FA-02 (#8994): an honest, short status line for the most recent
 * dispatched intent's outcome. Never phrased as success unless the outcome
 * is a receipted `applied`. */
const fullAutoRunControlOutcomeLabel = (
  outcome: FullAutoRunControlDispatchOutcome | null,
): string | null => {
  if (outcome === null) return null
  if (outcome.state === "applied") return "Done."
  if (outcome.state === "rejected") return `Couldn't complete: ${outcome.reason.replace(/_/g, " ")}.`
  if (outcome.state === "rejected_at_dispatch") return `Couldn't send the request (${outcome.code.replace(/_/g, " ")}).`
  if (outcome.state === "pending") return "Still pending — Desktop hasn't responded yet."
  if (outcome.state === "unauthorized") return "Sign in to control this run."
  return "Couldn't reach the server. Try again."
}

/** Live state header data (openagents #8982, extended MOB-FA-02 #8994):
 * objective + lifecycle state + rotation/lane/account/cap + Pause/Resume/
 * Stop control affordances + a terminal run-report summary, shown above the
 * transcript only while the active thread IS the live Full Auto run's
 * thread and that run is still active-and-fresh OR (newly) terminal-and-
 * fresh (so the report stays visible for a while right after the run ends).
 * Falls through to `null` (no header, unchanged default rendering) whenever
 * there is no run, the projection hasn't resolved yet, the projection is
 * stale, or the user has navigated to a different thread than the run's. */
export const fullAutoRunHeaderForState = (state: HomeState): FullAutoRunHeaderView | null => {
  if (state.fullAutoRun?.state !== "active") return null
  const projection = state.fullAutoRun.projection
  // `threadRef` is nullable (a run can exist before Desktop binds it to a
  // thread) — `null === null` must never read as "this is the active
  // thread"; only a real matching threadRef counts.
  if (projection.threadRef === null || state.activeThreadRef !== projection.threadRef) return null
  const stillRelevant = isFullAutoRunProjectionActive(projection) ||
    (isFullAutoRunLifecycleTerminal(projection.lifecycleState) && isFullAutoRunProjectionFresh(projection, Date.now()))
  if (!stillRelevant) return null
  return {
    runRef: projection.runRef,
    lifecycleLabel: FullAutoRunLifecycleStateLabel[projection.lifecycleState],
    objective: truncateFullAutoRunObjective(projection.objective),
    workspaceLabel: projection.workspaceLabel ?? "",
    laneRef: projection.laneRef,
    accountRef: projection.accountRef,
    turnCap: projection.turnCap,
    successfulAttempts: projection.successfulAttempts,
    failedAttempts: projection.failedAttempts,
    rotationCount: projection.rotationCount,
    control: {
      availableActions: fullAutoRunControlActionsFor(projection.lifecycleState),
      pendingAction: state.fullAutoControlPending,
      lastOutcomeLabel: fullAutoRunControlOutcomeLabel(state.fullAutoControlOutcome),
    },
    receipt: projection.receiptSummary === null ? null : {
      successfulAttempts: projection.receiptSummary.successfulAttempts,
      failedAttempts: projection.receiptSummary.failedAttempts,
      providerIdentities: projection.receiptSummary.providerIdentities,
      livenessGapCount: projection.receiptSummary.livenessGapCount,
    },
  }
}

export const chromeProps = (state: HomeState): ChromeProps => ({
  pillLabel: state.conversationAuthority === "sync" && state.surfaceMode === "khala"
    ? "OpenAgents"
    : surfaceModeOptions.find((option) => option.id === state.surfaceMode)?.label ?? "OpenAgents",
  composerPlaceholder: state.sarah !== null && state.activeThreadRef === state.sarah.threadRef
    ? "Message Sarah"
    : state.conversationAuthority === "sync" ? "Continue conversation" : "Message Khala",
  chromeVisible: !state.drawerOpen,
  glassComposerVisible: !state.drawerOpen && state.surfaceMode === "khala",
  surfaceMode: state.surfaceMode,
  draft: state.khala.draft,
  sending: state.khala.pending,
})

export const renderContentView = (state: HomeState): View =>
  Stack(
    {
      key: "home-root",
      direction: "column",
      style: { width: "full", height: "full", backgroundColor: "background" },
    },
    state.workbenchRoute === "terminal"
      ? [renderMobileTerminalView(state.repositoryTerminal, state.accessibility)]
      : state.workbenchRoute === "settings"
      ? [renderMobileSettingsView(state.settings, state.accessibility, {
          ...syncStatusCopy(state.syncPhase),
          control: mobileAccountControl(state.syncPhase),
        })]
      : state.workbenchRoute === "git"
      ? [renderMobileGitView(state.repositoryGit, state.accessibility)]
      : state.workbenchRoute === "changes"
      ? [renderMobileChangesView(state.repositoryReview, state.accessibility)]
      : state.workbenchRoute === "files"
      ? [renderMobileFilesView(state.repositoryBrowser, state.accessibility)]
      : state.surfaceMode === "khala"
      ? [renderKhalaSurface(
          state.khala,
          state.conversationAuthority,
          state.codingComposer,
          state.codingAttachmentPicking,
          state.codingAttachmentStatus,
          state.codingAttachmentMutatingRef,
          state.accessibility,
          state.codingExecutionTargets,
          {
            pickerOpen: state.codingComposerTargetPickerOpen,
            search: state.codingComposerTargetSearch,
          },
          state.codingPathDiscovery,
          state.syncPhase === "catching_up"
            ? "refreshing"
            : state.conversationAuthority === "sync" && state.syncPhase !== "live"
              ? "unavailable"
              : "live",
          fullAutoRunHeaderForState(state),
          state.sarah !== null && state.activeThreadRef === state.sarah.threadRef
            ? "hidden"
            : "visible",
        )]
      : [
          Spacer({ key: "openagents-top-space", size: "16" }),
          Text({ key: "openagents-title", content: "OpenAgents", variant: "title", color: "textPrimary" }),
          Text({
            key: "openagents-sync-title",
            content: syncStatusCopy(state.syncPhase).title,
            variant: "heading",
            color: state.syncPhase === "unconfigured" ? "warning" : "textPrimary",
          }),
          Text({
            key: "openagents-sync-detail",
            content: syncStatusCopy(state.syncPhase).detail,
            variant: "body",
            color: "textMuted",
          }),
          ...renderMobileControllerShell(state),
          ...(mobileAccountControl(state.syncPhase) === "none"
            ? []
            : mobileAccountControl(state.syncPhase) === "sign_out"
              ? [Button({
                  key: "openagents-sign-out",
                  label: "Sign out",
                  variant: "secondary",
                  onPress: IntentRef("OpenAgentsSignOutPressed", StaticPayload({})),
                  style: mobileInteractiveStyle(state.accessibility),
                })]
              : [Button({
                  key: "openagents-sign-in",
                  label: "Link OpenAgents account",
                  variant: "primary",
                  onPress: IntentRef("OpenAgentsSignInPressed", StaticPayload({})),
                  style: mobileInteractiveStyle(state.accessibility),
                })]),
        ],
  )

const controllerSessionLabel = (session: MobileControllerSession): string => {
  const state = codingSessionStateLabel(session.state)
  const target = session.targetReadiness === "ready"
    ? "Target ready"
    : session.targetReadiness === "recovery_required"
      ? "Recovery required"
      : session.targetReadiness === "provider_unavailable"
        ? "Provider unavailable"
        : "Runtime unavailable"
  return `${session.repositoryName} · ${state}\n${target}`
}

const controllerSessionButton = (
  session: MobileControllerSession,
  accessibility: MobileAccessibilityProfile,
): View => Button({
  key: `controller-session-${session.sessionRef}`,
  label: controllerSessionLabel(session),
  variant: session.attention === "needs_recovery" ? "secondary" : "ghost",
  onPress: IntentRef("ControllerSessionInspected", StaticPayload({
    sessionRef: session.sessionRef,
  })),
  a11y: { label: `${controllerSessionLabel(session)}, inspect session` },
  style: { width: "full", ...mobileInteractiveStyle(accessibility) },
})

const controllerFactLabel = (
  fact: MobileControllerSession["provider"] | MobileControllerSession["runtime"],
): string => fact.state === "known"
  ? ("providerRef" in fact ? fact.providerRef : fact.runtimeRef)
  : `Unavailable · ${fact.reason.replaceAll("_", " ")}`

const portableUnavailableCopy = (reason: MobilePortableUnavailableReason): string => {
  switch (reason) {
    case "authority_unavailable": return "Portable authority is not live on this device."
    case "projection_invalid": return "Portable controls are withheld because confirmed state is incomplete."
    case "session_not_portable": return "This session is not portable yet."
    case "target_directory_missing": return "No confirmed destination directory is available."
    case "attachment_authority_ambiguous": return "The active session attachment is ambiguous."
    case "source_target_missing": return "The confirmed source target is unavailable."
    case "command_in_flight": return "Wait for the current portable command to reconcile."
    case "action_requires_active_attachment": return "This action requires an active session attachment."
    case "action_requires_suspended_attachment": return "Resume requires a suspended session attachment."
    case "destination_required": return "Choose a destination first."
    case "destination_not_ready": return "The selected destination is not ready."
    case "destination_is_source": return "Choose a destination different from the source."
    case "failback_target_missing": return "No ready owner-local failback target is available."
    case "invalid_invocation": return "The portable command could not be admitted."
  }
}

const portableActionLabel = (action: MobilePortableControlAction): string => {
  switch (action) {
    case "checkpoint": return "Checkpoint"
    case "move": return "Move session"
    case "resume": return "Resume session"
    case "failback": return "Fail back"
    case "stop": return "Stop session"
  }
}

const portableControlViews = (
  control: MobilePortableSessionControl,
  state: HomeState,
): ReadonlyArray<View> => {
  if (control.state === "unavailable") return [Text({
    key: "portable-controls-unavailable",
    content: portableUnavailableCopy(control.reason),
    variant: "caption",
    color: "warning",
  })]
  const actionButtons: ReadonlyArray<MobilePortableControlAction> = [
    "checkpoint", "move", "resume", "failback", "stop",
  ]
  return [
    Text({
      key: "portable-source",
      content: `Portable source ${control.sourceTarget.targetClass} · ${control.sourceTarget.targetRef}\nGeneration ${control.sourceAttachment.generation} · ${control.sourceAttachment.state}`,
      variant: "caption",
      color: "textMuted",
    }),
    ...control.actions.move.destinations.map(target => Button({
      key: `portable-destination-${target.targetRef}`,
      label: `${state.selectedPortableDestinationRef === target.targetRef ? "Selected · " : ""}${target.targetClass} · ${target.targetRef}`,
      variant: state.selectedPortableDestinationRef === target.targetRef ? "secondary" : "ghost",
      onPress: IntentRef("PortableDestinationSelected", StaticPayload(target.targetRef)),
      a11y: { label: `Select portable destination ${target.targetRef}, ${target.health}` },
      style: { width: "full", ...mobileInteractiveStyle(state.accessibility) },
    })),
    ...(control.pendingCommand === null
      ? control.pendingLocalCommandCount > 0
        ? [Text({
            key: "portable-local-pending",
            content: "Command queued · awaiting server acceptance",
            variant: "caption",
            color: "warning",
          })]
        : []
      : [Text({
          key: "portable-command-pending",
          content: `Pending ${control.pendingCommand.kind} · ${control.pendingCommand.commandRef}`,
          variant: "caption",
          color: "warning",
        })]),
    ...(control.latestOutcome === null ? [] : [Text({
      key: "portable-command-outcome",
      content: `Last confirmed command · ${control.latestOutcome.status}\n${control.latestOutcome.commandRef} · Evidence ${control.latestOutcome.evidenceRefs.length}`,
      variant: "caption",
      color: control.latestOutcome.status === "completed" ? "textPrimary" : "warning",
    })]),
    ...(state.portableNotice === null ? [] : [Text({
      key: "portable-command-notice",
      content: state.portableNotice.message,
      variant: "caption",
      color: state.portableNotice.kind === "queued" ? "textMuted" : "warning",
    })]),
    ...actionButtons.map(action => {
      const availability = control.actions[action]
      const needsDestination = action === "move" || action === "failback"
      const selectedDestinationAllowed = !needsDestination || (
        state.selectedPortableDestinationRef !== null &&
        availability.destinations.some(target => target.targetRef === state.selectedPortableDestinationRef)
      )
      const unavailableReason = availability.reason ?? (needsDestination && !selectedDestinationAllowed
        ? "destination_required"
        : null)
      const disabled = state.portableSubmittingAction !== null || !availability.available || !selectedDestinationAllowed
      const label = state.portableSubmittingAction === action
        ? `${portableActionLabel(action)} queued…`
        : portableActionLabel(action)
      return Button({
        key: `portable-action-${action}`,
        label,
        variant: action === "move" ? "primary" : action === "stop" ? "ghost" : "secondary",
        onPress: IntentRef("PortableControlRequested", StaticPayload({ action })),
        disabled,
        a11y: {
          label: disabled && unavailableReason !== null
            ? `${portableActionLabel(action)} unavailable. ${portableUnavailableCopy(unavailableReason)}`
            : `${portableActionLabel(action)} from ${control.sourceTarget.targetRef}${needsDestination ? ` to ${state.selectedPortableDestinationRef}` : ""}`,
        },
        style: { width: "full", ...mobileInteractiveStyle(state.accessibility) },
      })
    }),
  ]
}

const controllerSessionDetail = (
  session: MobileControllerSession,
  state: HomeState,
): View => Stack({
  key: `controller-session-detail-${session.sessionRef}`,
  direction: "column",
  gap: "2",
  padding: "3",
  style: { width: "full", backgroundColor: "surfaceRaised", borderRadius: "lg" },
  a11y: { role: "region", label: `Session overview for ${session.repositoryName}` },
}, [
  Text({
    key: "controller-session-detail-title",
    content: `${session.repositoryName} · ${codingSessionStateLabel(session.state)}`,
    variant: "heading",
    color: "textPrimary",
  }),
  Text({
    key: "controller-session-detail-refs",
    content: `Session ${session.sessionRef}\nThread ${session.threadRef}`,
    variant: "caption",
    color: "textMuted",
  }),
  Text({
    key: "controller-session-detail-target",
    content: `Provider ${controllerFactLabel(session.provider)}\nRuntime ${controllerFactLabel(session.runtime)}`,
    variant: "body",
    color: session.targetReadiness === "ready" ? "textPrimary" : "warning",
  }),
  Text({
    key: "controller-session-detail-activity",
    content: `Last activity ${session.lastActiveAt}\nCursor ${session.canonicalEventCursor}${session.currentCheckpointRef === null ? " · No checkpoint projected" : ` · ${session.currentCheckpointRef}`}`,
    variant: "caption",
    color: "textMuted",
  }),
  Button({
    key: "controller-session-detail-continue",
    label: "Continue session",
    variant: "primary",
    onPress: IntentRef("CodingSessionSelected", StaticPayload({
      repositoryRef: session.repositoryRef,
      sessionRef: session.sessionRef,
      threadRef: session.threadRef,
    })),
    disabled: session.targetReadiness === "recovery_required",
    a11y: {
      label: session.targetReadiness === "recovery_required"
        ? "Continue unavailable until session recovery"
        : `Continue ${session.repositoryName} session`,
    },
    style: { width: "full", ...mobileInteractiveStyle(state.accessibility) },
  }),
  ...(state.portableSnapshot === null
    ? [Text({
        key: "portable-authority-unavailable",
        content: "Portable controls are unavailable until confirmed portable authority is projected.",
        variant: "caption",
        color: "textMuted",
      })]
    : portableControlViews(
        projectMobilePortableSessionControl(state.portableSnapshot, session.sessionRef),
        state,
      )),
])

const controllerDestinationRows = (
  directory: MobileControllerDirectory,
  destination: MobileControllerDestination,
  state: HomeState,
): ReadonlyArray<View> => {
  if (destination === "repositories") {
    return directory.repositories.flatMap(repository => [
      Text({
        key: `controller-repository-${repository.repositoryRef}`,
        content: `${repository.displayName} · ${repository.sessions.length} ${repository.sessions.length === 1 ? "session" : "sessions"}`,
        variant: "heading",
        color: "textPrimary",
      }),
      ...repository.sessions.map(session => controllerSessionButton(session, state.accessibility)),
    ])
  }
  if (destination === "attention") {
    const snapshot = state.attentionSnapshot
    const attentionRows: ReadonlyArray<View> = snapshot === null
      ? [Text({
          key: "controller-attention-authority-unavailable",
          content: "Attention is unavailable until the confirmed personal inbox is live.",
          variant: "body",
          color: "textMuted",
        })]
      : snapshot.issues.length > 0
        ? [Text({
            key: "controller-attention-projection-invalid",
            content: "Attention is withheld because the confirmed inbox needs reconciliation.",
            variant: "body",
            color: "danger",
          })]
        : [
            ...snapshot.pending.map(item => Button({
              key: `controller-attention-${item.attentionRef}`,
              label: `${item.kind === "provider_question" ? "Question" : item.kind === "tool_approval" ? "Approval" : "Plan review"} · ${item.threadRef}`,
              variant: "secondary",
              onPress: IntentRef("ControllerAttentionSelected", StaticPayload({
                schema: MobileAttentionTargetSchemaVersion,
                attentionRef: item.attentionRef,
                threadRef: item.threadRef,
                turnRef: item.turnRef,
              })),
              a11y: { label: `Open pending ${item.kind.replaceAll("_", " ")} in ${item.threadRef}` },
              style: { width: "full", ...mobileInteractiveStyle(state.accessibility) },
            })),
            ...(snapshot.terminal.length === 0 ? [] : [Text({
              key: "controller-attention-terminal-count",
              content: `${snapshot.terminal.length} recently resolved ${snapshot.terminal.length === 1 ? "request" : "requests"} · not actionable`,
              variant: "caption",
              color: "textMuted",
            })]),
          ]
    return [
      ...attentionRows,
      ...(state.attentionNotice === null ? [] : [Text({
        key: "controller-attention-notice",
        content: state.attentionNotice,
        variant: "caption",
        color: "danger",
      })]),
      ...directory.attention.map(session => controllerSessionButton(session, state.accessibility)),
      ...(snapshot?.pending.length === 0 && directory.attention.length === 0
        ? [Text({
            key: "controller-attention-empty",
            content: "No sessions need attention.",
            variant: "body",
            color: "textMuted",
          })]
        : []),
    ]
  }
  const sessions = directory.recent
  if (sessions.length > 0) return sessions.map(session => controllerSessionButton(session, state.accessibility))
  return [Text({
    key: `controller-${destination}-empty`,
    content: "No confirmed coding sessions yet.",
    variant: "body",
    color: "textMuted",
  })]
}

export const renderMobileControllerShell = (state: HomeState): ReadonlyArray<View> => {
  if (state.codingDirectory === null) return []
  const directory = projectMobileControllerDirectory(state.codingDirectory)
  if (directory.authority !== "confirmed") return codingOfflineCacheAccountingRows(state)
  const inspected = state.inspectedControllerSessionRef === null
    ? null
    : directory.recent.find(session => session.sessionRef === state.inspectedControllerSessionRef) ?? null
  const confirmedAttentionCount = state.attentionSnapshot?.issues.length === 0
    ? state.attentionSnapshot.pending.length
    : 0
  return [
    Text({
      key: "controller-summary",
      content: `${directory.summary.repositoryCount} ${directory.summary.repositoryCount === 1 ? "repository" : "repositories"} · ${directory.summary.sessionCount} ${directory.summary.sessionCount === 1 ? "session" : "sessions"} · ${directory.summary.attentionCount + confirmedAttentionCount} need attention`,
      variant: "caption",
      color: "textMuted",
    }),
    SegmentedControl({
      key: "controller-destinations",
      value: state.controllerDestination,
      options: [
        { id: "recent", label: "Recent" },
        { id: "repositories", label: "Repositories" },
        { id: "attention", label: "Attention" },
      ],
      onChange: IntentRef("ControllerDestinationSelected", ComponentValueBinding()),
      a11y: { label: "Mobile controller destination" },
      style: { width: "full" },
    }),
    ...(inspected === null ? [] : [controllerSessionDetail(inspected, state)]),
    ...controllerDestinationRows(directory, state.controllerDestination, state),
  ]
}

/**
 * The complete application-owned mobile home tree. React Native owns only the
 * safe-area/keyboard mount; chrome, drawer, composer, state, and actions are
 * catalog data lowered by Effect Native.
 */
export const renderHomeView = (state: HomeState): View =>
  Stack(
    {
      key: "home-application-root",
      direction: "column",
      gap: "0",
      padding: "2",
      a11y: {
        role: "region",
        label: `OpenAgents mobile home, ${state.accessibility.textScale} text scale, reduced motion ${state.accessibility.reduceMotion ? "on" : "off"}`,
        activeDescendant: mobileWorkspaceActiveDescendant(
          state.workspaceLayoutMode,
          state.drawerOpen,
          state.workspaceSidebarCollapsed,
          state.workspaceFocusTarget,
        ),
      },
      style: { width: "full", height: "full", backgroundColor: "background" },
    },
    [
      Toolbar(
        {
          key: "home-toolbar",
          placement: "top",
          style: {
            width: "full",
            minHeight: state.accessibility.minTouchTarget,
            padding: "0",
            gap: "1",
            borderWidth: 0,
            borderRadius: "none",
            backgroundColor: "background",
          },
        },
        [
          IconButton({
            key: "home-navigation",
            icon: "Menu",
            accessibilityLabel: state.workspaceLayoutMode === "regular"
              ? state.workspaceSidebarCollapsed ? "Show workspace navigation" : "Hide workspace navigation"
              : state.drawerOpen ? "Close workspace navigation" : "Go to workspace navigation",
            onPress: IntentRef("DrawerToggled", StaticPayload({})),
            surface: "glass",
            style: mobileInteractiveStyle(state.accessibility),
          }),
          Stack(
            {
              key: "home-header-identity",
              direction: "column",
              gap: "0",
              align: "center",
              justify: "center",
              style: { flex: 1, minHeight: state.accessibility.minTouchTarget },
            },
            [
              Text({
                key: "home-header-title",
                content: mobileHeaderProps(state).title,
                variant: "title",
                color: "textPrimary",
                style: { width: "full", textAlign: "center", fontWeight: "bold" },
              }),
              ...(mobileHeaderProps(state).subtitle === null
                ? []
                : [Text({
                    key: "home-header-subtitle",
                    content: mobileHeaderProps(state).subtitle!,
                    variant: "caption",
                    color: "textMuted",
                    style: { width: "full", textAlign: "center" },
                  })]),
            ],
          ),
          Stack(
            {
              key: "home-header-actions",
              direction: "row",
              gap: "0.5",
              align: "center",
              style: sarahIsActive(state)
                ? {}
                : { surface: "glass", borderRadius: "full" },
            },
            [
              ...(state.codingComposer === null || sarahIsActive(state)
                ? []
                : [IconButton({
                    key: "home-files",
                    icon: state.workbenchRoute === "files" ? "Chats" : "Folder",
                    accessibilityLabel: state.workbenchRoute === "files"
                      ? "Return to conversation"
                      : "Open repository files",
                    onPress: IntentRef(
                      state.workbenchRoute === "files" ? "FilesRouteClosed" : "FilesRouteOpened",
                      StaticPayload({}),
                    ),
                    style: mobileInteractiveStyle(state.accessibility),
                  }), IconButton({
                    key: "home-changes",
                    icon: state.workbenchRoute === "changes" ? "Chats" : "Compare",
                    accessibilityLabel: state.workbenchRoute === "changes"
                      ? "Return to conversation"
                      : "Open repository changes",
                    onPress: IntentRef(
                      state.workbenchRoute === "changes" ? "WorkbenchConversationOpened" : "ChangesRouteOpened",
                      StaticPayload({}),
                    ),
                    style: mobileInteractiveStyle(state.accessibility),
                  }), IconButton({
                    key: "home-git",
                    icon: state.workbenchRoute === "git" ? "Chats" : "Branch",
                    accessibilityLabel: state.workbenchRoute === "git"
                      ? "Return to conversation"
                      : "Open Git workbench",
                    onPress: IntentRef(
                      state.workbenchRoute === "git" ? "WorkbenchConversationOpened" : "GitRouteOpened",
                      StaticPayload({}),
                    ),
                    style: mobileInteractiveStyle(state.accessibility),
                  }), IconButton({
                    key: "home-terminal",
                    icon: state.workbenchRoute === "terminal" ? "Chats" : "Terminal",
                    accessibilityLabel: state.workbenchRoute === "terminal"
                      ? "Return to conversation"
                      : "Open terminal workbench",
                    onPress: IntentRef(
                      state.workbenchRoute === "terminal" ? "WorkbenchConversationOpened" : "TerminalRouteOpened",
                      StaticPayload({}),
                    ),
                    style: mobileInteractiveStyle(state.accessibility),
                  })]),
              ...(sarahIsActive(state)
                ? []
                : [IconButton({
                    key: "home-new-chat",
                    icon: "Compose",
                    accessibilityLabel: "New chat",
                    onPress: IntentRef("NewChatPressed", StaticPayload({})),
                    style: mobileInteractiveStyle(state.accessibility),
                  })]),
              IconButton({
                key: "home-more",
                icon: "Ellipsis",
                accessibilityLabel: "Open settings",
                onPress: IntentRef("SettingsPressed", StaticPayload({})),
                ...(sarahIsActive(state) ? { surface: "glass" as const } : {}),
                style: mobileInteractiveStyle(state.accessibility),
              }),
            ],
          ),
        ],
      ),
      state.workspaceLayoutMode === "regular"
        ? SplitPane({
            key: "workspace-regular-split",
            orientation: "row",
            panes: [{
              id: "navigation",
              size: state.workspaceSidebarWidth,
              min: MOBILE_WORKSPACE_SIDEBAR_MIN,
              max: MOBILE_WORKSPACE_SIDEBAR_MAX,
              collapsed: state.workspaceSidebarCollapsed,
              content: renderDrawerView(state),
            }, {
              id: "detail",
              content: renderContentView(state),
            }],
            onResize: IntentRef("WorkspaceSidebarResized", ComponentValueBinding()),
            style: { width: "full", height: "full", backgroundColor: "background" },
          })
        : state.drawerOpen ? renderDrawerView(state) : renderContentView(state),
    ],
  )

const drawerRow = (
  input: {
    readonly key: string
    readonly label: string
    readonly onPress: ReturnType<typeof IntentRef>
    readonly selected?: boolean
  },
  accessibility: MobileAccessibilityProfile,
): View =>
  Button({
    key: input.key,
    label: input.label,
    variant: input.selected === true ? "secondary" : "ghost",
    onPress: input.onPress,
    style: {
      width: "full",
      ...mobileInteractiveStyle(accessibility),
      ...(input.selected === true ? { backgroundColor: "surfaceRaised" } : {}),
    },
  })

const codingSessionStateLabel = (state: MobileCodingDirectory["sessions"][number]["state"]): string => {
  switch (state) {
    case "active": return "Active"
    case "idle": return "Ready"
    case "recovery_required": return "Needs recovery"
    case "archived": return "Archived"
  }
}

/**
 * Loss-accounted offline cache line: while hosted coding authority is
 * withheld, name exactly how many confirmed rows stay cached-but-hidden so
 * an offline directory never silently masquerades as an empty account.
 */
export const codingOfflineCacheLabel = (directory: MobileCodingDirectory): string => {
  const { cachedRepositoryCount, cachedSessionCount } = directory.offlineCache
  const counts = `${cachedRepositoryCount} ${cachedRepositoryCount === 1 ? "repository" : "repositories"} · ${cachedSessionCount} ${cachedSessionCount === 1 ? "session" : "sessions"}`
  return directory.cacheState === "purged_after_denial"
    ? `Coding cache · ${counts} withheld after denial`
    : `Coding cache · ${counts} hidden until reconnect`
}

const codingOfflineCacheAccountingRows = (state: HomeState): ReadonlyArray<View> =>
  state.codingDirectory !== null &&
    state.codingDirectory.authority === "withheld" &&
    state.codingDirectory.offlineCache.accounting === "withheld_counted" &&
    (state.codingDirectory.offlineCache.cachedRepositoryCount > 0 ||
      state.codingDirectory.offlineCache.cachedSessionCount > 0)
    ? [
        Text({
          key: "drawer-coding-offline-cache",
          content: codingOfflineCacheLabel(state.codingDirectory),
          variant: "caption",
          color: "textMuted",
        }),
      ]
    : []

const threadLifecycleRows = (state: HomeState): ReadonlyArray<View> => {
  if (state.conversationAuthority !== "sync") return []
  const pending = state.threadLifecycle.pendingAction !== null
  const selected = [...state.conversationThreads, ...state.archivedConversationThreads]
    .find(thread => thread.threadRef === state.threadLifecycle.actionThreadRef)
  const deleting = [...state.conversationThreads, ...state.archivedConversationThreads]
    .find(thread => thread.threadRef === state.threadLifecycle.deleteConfirmThreadRef)
  const rows: Array<View> = []
  if (selected !== undefined) {
    rows.push(Text({
      key: "drawer-thread-controls-title",
      content: `Actions · ${selected.title}`,
      variant: "caption",
      color: "textMuted",
    }))
    if (state.threadLifecycle.editingThreadRef === selected.threadRef) {
      rows.push(TextField({
        key: "drawer-thread-rename-field",
        value: state.threadLifecycle.renameDraft,
        label: "Chat title",
        placeholder: "Chat title",
        disabled: pending,
        onChange: IntentRef("ConversationThreadRenameChanged", ComponentValueBinding()),
        onSubmit: IntentRef("ConversationThreadRenameSubmitted", StaticPayload({})),
        variant: "outline",
        size: "md",
        style: { width: "full" },
      }))
      rows.push(Stack({ key: "drawer-thread-rename-actions", direction: "row", gap: "2" }, [
        Button({
          key: "drawer-thread-rename-save",
          label: pending ? "Saving…" : "Save",
          variant: "primary",
          disabled: pending || state.threadLifecycle.renameDraft.trim() === "",
          onPress: IntentRef("ConversationThreadRenameSubmitted", StaticPayload({})),
          style: mobileInteractiveStyle(state.accessibility),
        }),
        Button({
          key: "drawer-thread-rename-cancel",
          label: "Cancel",
          variant: "ghost",
          disabled: pending,
          onPress: IntentRef("ConversationThreadRenameCancelled", StaticPayload({})),
          style: mobileInteractiveStyle(state.accessibility),
        }),
      ]))
    } else if (selected.status === "active") {
      rows.push(Stack({ key: "drawer-thread-active-actions", direction: "row", gap: "2" }, [
        Button({
          key: "drawer-thread-rename",
          label: "Rename",
          variant: "ghost",
          disabled: pending,
          onPress: IntentRef("ConversationThreadRenameStarted", StaticPayload({ threadRef: selected.threadRef })),
          style: mobileInteractiveStyle(state.accessibility),
        }),
        Button({
          key: "drawer-thread-archive",
          label: pending && state.threadLifecycle.pendingAction === "archive" ? "Archiving…" : "Archive",
          variant: "secondary",
          disabled: pending,
          onPress: IntentRef("ConversationThreadLifecycleRequested", StaticPayload({ action: "archive", threadRef: selected.threadRef })),
          style: mobileInteractiveStyle(state.accessibility),
        }),
        Button({
          key: "drawer-thread-delete",
          label: "Delete",
          tone: "danger",
          variant: "soft",
          disabled: pending,
          onPress: IntentRef("ConversationThreadDeleteRequested", StaticPayload({ threadRef: selected.threadRef })),
          style: mobileInteractiveStyle(state.accessibility),
        }),
      ]))
    } else if (selected.status === "archived") {
      rows.push(Stack({ key: `drawer-archived-actions-${selected.threadRef}`, direction: "row", gap: "2" }, [
        Button({
          key: `drawer-restore-${selected.threadRef}`,
          label: pending && state.threadLifecycle.pendingAction === "restore" ? "Restoring…" : "Restore",
          variant: "secondary",
          disabled: pending,
          onPress: IntentRef("ConversationThreadLifecycleRequested", StaticPayload({ action: "restore", threadRef: selected.threadRef })),
          style: mobileInteractiveStyle(state.accessibility),
        }),
        Button({
          key: `drawer-delete-archived-${selected.threadRef}`,
          label: "Delete",
          tone: "danger",
          variant: "soft",
          disabled: pending,
          onPress: IntentRef("ConversationThreadDeleteRequested", StaticPayload({ threadRef: selected.threadRef })),
          style: mobileInteractiveStyle(state.accessibility),
        }),
      ]))
    }
  }
  if (deleting !== undefined) {
    rows.push(Text({
      key: "drawer-thread-delete-confirm-copy",
      content: `Delete “${deleting.title}”? This removes it from every synced device and cannot be undone.`,
      variant: "body",
      color: "danger",
    }))
    rows.push(Stack({ key: "drawer-thread-delete-confirm-actions", direction: "row", gap: "2" }, [
      Button({
        key: "drawer-thread-delete-confirm",
        label: pending ? "Deleting…" : "Delete permanently",
        tone: "danger",
        variant: "solid",
        disabled: pending,
        onPress: IntentRef("ConversationThreadDeleteConfirmed", StaticPayload({})),
        style: mobileInteractiveStyle(state.accessibility),
      }),
      Button({
        key: "drawer-thread-delete-cancel",
        label: "Cancel",
        variant: "ghost",
        disabled: pending,
        onPress: IntentRef("ConversationThreadDeleteCancelled", StaticPayload({})),
        style: mobileInteractiveStyle(state.accessibility),
      }),
    ]))
  }
  if (state.threadLifecycle.notice !== null) {
    rows.push(Text({
      key: "drawer-thread-lifecycle-notice",
      content: state.threadLifecycle.notice.message,
      variant: "caption",
      color: state.threadLifecycle.notice.kind === "confirmed" ? "success" : "danger",
    }))
  }
  return rows
}

const workspaceRowOpenIntent = (row: MobileWorkspaceRow) =>
  row.attentionTarget !== null
    ? IntentRef("ControllerAttentionSelected", StaticPayload({
        schema: MobileAttentionTargetSchemaVersion,
        ...row.attentionTarget,
      }))
    : row.kind === "coding_session" && row.sessionRef !== null && row.repositoryRef !== null
      ? IntentRef("CodingSessionSelected", StaticPayload({
          repositoryRef: row.repositoryRef,
          sessionRef: row.sessionRef,
          threadRef: row.threadRef,
        }))
      : IntentRef("ConversationThreadSelected", StaticPayload({ threadRef: row.threadRef }))

const workspaceRow = (row: MobileWorkspaceRow, state: HomeState): View => {
  const openIntent = workspaceRowOpenIntent(row)
  const canOpen = row.state !== "archived" && row.state !== "recovery"
  const hasLifecycle = state.conversationAuthority === "sync" &&
    [...state.conversationThreads, ...state.archivedConversationThreads]
      .some(thread => thread.threadRef === row.threadRef)
  const metadata = [
    row.projectLabel,
    row.worktreeLabel === null ? null : `Worktree ${row.worktreeLabel}`,
    row.recencyLabel,
  ].filter((value): value is string => value !== null).join(" · ")
  const body = Stack({
    key: `workspace-row-${row.rowId}`,
    direction: "column",
    gap: "1",
    padding: "2",
    style: {
      width: "full",
      backgroundColor: row.selected ? "surfaceRaised" : "surface",
      borderRadius: "lg",
    },
    a11y: { role: "region", label: `${row.title}, ${row.stateLabel}, ${metadata}` },
  }, [
    Stack({ key: `workspace-row-heading-${row.rowId}`, direction: "row", gap: "2" }, [
      Button({
        key: `workspace-row-open-${row.rowId}`,
        label: row.title,
        variant: row.selected ? "secondary" : "ghost",
        disabled: !canOpen,
        onPress: openIntent,
        a11y: {
          label: row.attentionTarget === null
            ? `${canOpen ? "Open" : "Unavailable"} ${row.title}`
            : `Open ${row.stateLabel.toLocaleLowerCase()} in ${row.title}`,
        },
        style: { width: "full", ...mobileInteractiveStyle(state.accessibility) },
      }),
      ...(hasLifecycle
        ? [Button({
            key: `workspace-row-actions-${row.rowId}`,
            label: state.threadLifecycle.actionThreadRef === row.threadRef ? "Close" : "More",
            variant: "ghost",
            onPress: IntentRef("WorkspaceRowActionsToggled", StaticPayload({ threadRef: row.threadRef })),
            a11y: { label: `Actions for ${row.title}` },
            style: mobileInteractiveStyle(state.accessibility),
          })]
        : []),
    ]),
    Text({
      key: `workspace-row-meta-${row.rowId}`,
      content: metadata,
      variant: "caption",
      color: "textMuted",
    }),
    Text({
      key: `workspace-row-state-${row.rowId}`,
      content: row.stateLabel,
      variant: "caption",
      color: row.state === "attention" || row.state === "recovery"
        ? "warning"
        : row.state === "active"
          ? "success"
          : "textMuted",
    }),
  ])
  if (!hasLifecycle) return body
  const archived = row.state === "archived"
  const reversibleAction = archived ? "restore" : "archive"
  const actionId = (action: "archive" | "restore" | "delete") => `${action}:${row.threadRef}`
  return SwipeableListItem({
    key: `workspace-swipe-${row.rowId}`,
    child: body,
    trailingActions: [{
      id: actionId(reversibleAction),
      label: archived ? "Restore" : "Archive",
      icon: archived ? "Undo" : "Archive",
      tone: "neutral",
    }, {
      id: actionId("delete"),
      label: "Delete",
      icon: "Trash",
      tone: "danger",
      destructive: true,
    }],
    fullSwipeActionId: actionId(reversibleAction),
    onAction: IntentRef("WorkspaceRowActionSelected", ComponentValueBinding()),
    style: { width: "full" },
  })
}

const workspaceNavigationRows = (state: HomeState): ReadonlyArray<View> => {
  const directory = state.codingDirectory === null
    ? null
    : projectMobileControllerDirectory(state.codingDirectory)
  const projection = projectMobileWorkspaceNavigation({
    threads: state.conversationThreads,
    archivedThreads: state.archivedConversationThreads,
    directory,
    attention: state.attentionSnapshot,
    activeThreadRef: state.activeThreadRef,
    search: state.workspaceSearch,
    status: state.workspaceStatusFilter,
    projectRef: state.workspaceProjectFilter,
  })
  const filtersActive = state.workspaceSearch.trim() !== "" ||
    state.workspaceStatusFilter !== "all" || state.workspaceProjectFilter !== null
  return [
    TextField({
      key: "workspace-search",
      value: state.workspaceSearch,
      label: "Search workspaces",
      placeholder: "Search chats, projects, worktrees",
      onChange: IntentRef("WorkspaceSearchChanged", ComponentValueBinding()),
      variant: "outline",
      size: "md",
      style: { width: "full" },
    }),
    SegmentedControl({
      key: "workspace-status-filter",
      value: state.workspaceStatusFilter,
      options: [
        { id: "all", label: "All" },
        { id: "active", label: "Active" },
        { id: "attention", label: "Needs you" },
        { id: "idle", label: "Idle" },
        { id: "archived", label: "Archived" },
      ],
      onChange: IntentRef("WorkspaceStatusFilterSelected", ComponentValueBinding()),
      a11y: { label: "Workspace status filter" },
      style: { width: "full" },
    }),
    ...(projection.projectFilters.length === 0
      ? []
      : [
          Text({ key: "workspace-project-filter-title", content: "Projects", variant: "caption", color: "textMuted" }),
          Stack({ key: "workspace-project-filters", direction: "column", gap: "1" }, [
            Button({
              key: "workspace-project-all",
              label: "All projects",
              variant: state.workspaceProjectFilter === null ? "secondary" : "ghost",
              onPress: IntentRef("WorkspaceProjectFilterSelected", StaticPayload("")),
              style: mobileInteractiveStyle(state.accessibility),
            }),
            ...projection.projectFilters.map(project => Button({
              key: `workspace-project-${project.id}`,
              label: project.label,
              variant: state.workspaceProjectFilter === project.id ? "secondary" : "ghost",
              onPress: IntentRef("WorkspaceProjectFilterSelected", StaticPayload(project.id)),
              style: mobileInteractiveStyle(state.accessibility),
            })),
          ]),
        ]),
    ...(filtersActive
      ? [Button({
          key: "workspace-clear-filters",
          label: "Clear filters",
          variant: "ghost",
          onPress: IntentRef("WorkspaceFiltersCleared", StaticPayload({})),
          style: mobileInteractiveStyle(state.accessibility),
        })]
      : []),
    ...(projection.rows.length === 0
      ? [Text({
          key: "workspace-empty-results",
          content: filtersActive
            ? "No workspaces match these filters."
            : "No confirmed chats or coding sessions yet.",
          variant: "body",
          color: "textMuted",
        })]
      : projection.rows.map(row => workspaceRow(row, state))),
    ...(projection.hiddenRowCount === 0
      ? []
      : [Text({
          key: "workspace-hidden-row-count",
          content: `${projection.hiddenRowCount} more results hidden. Refine search to continue.`,
          variant: "caption",
          color: "textMuted",
        })]),
  ]
}

const sarahFocusedNavigationRows = (state: HomeState): ReadonlyArray<View> => {
  const directory = state.codingDirectory === null
    ? null
    : projectMobileControllerDirectory(state.codingDirectory)
  const rows = projectMobileWorkspaceNavigation({
    threads: state.conversationThreads,
    archivedThreads: state.archivedConversationThreads,
    directory,
    attention: state.attentionSnapshot,
    activeThreadRef: state.activeThreadRef,
    search: "",
    status: "all",
    projectRef: null,
  }).rows
    .filter(row => row.threadRef !== state.sarah?.threadRef && row.state !== "archived")
    .slice(0, 5)

  return rows.length === 0
    ? []
    : [
        Text({
          key: "sarah-recent-title",
          content: "Recent conversations",
          variant: "caption",
          color: "textMuted",
        }),
        ...rows.map(row => Button({
          key: `sarah-recent-${row.rowId}`,
          label: row.title,
          variant: "ghost",
          disabled: row.state === "recovery",
          onPress: workspaceRowOpenIntent(row),
          style: { width: "full", ...mobileInteractiveStyle(state.accessibility) },
        })),
      ]
}

export const renderDrawerView = (state: HomeState): View =>
  Stack(
    { key: "drawer-root", direction: "column", gap: "2", padding: "4", style: { width: "full", height: "full", backgroundColor: "surface" } },
    [
      Spacer({ key: "drawer-top-space", size: "10" }),
      drawerRow({
        key: "drawer-sarah",
        label: state.sarah === null
          ? "Sarah · Sign in as owner"
          : "Sarah",
        onPress: state.sarah === null
          ? IntentRef("OpenAgentsSignInPressed", StaticPayload({}))
          : IntentRef("ConversationThreadSelected", StaticPayload({ threadRef: state.sarah.threadRef })),
        selected: state.sarah !== null && state.activeThreadRef === state.sarah.threadRef,
      }, state.accessibility),
      drawerRow({ key: "drawer-new-chat", label: "New chat", onPress: IntentRef("NewChatPressed", StaticPayload({})), selected: state.surfaceMode === "khala" && state.khala.entries.length === 0 }, state.accessibility),
      ...(sarahIsActive(state)
        ? sarahFocusedNavigationRows(state)
        : [
            drawerRow({
              key: "drawer-current-surface",
              label: state.conversationAuthority === "sync" ? "OpenAgents" : "Khala",
              onPress: IntentRef("SurfaceModeSelected", StaticPayload({ mode: "khala" })),
              selected: state.surfaceMode === "khala",
            }, state.accessibility),
            ...workspaceNavigationRows(state),
          ]),
      ...(sarahIsActive(state) || (state.fleetRuns?.runs.length ?? 0) === 0
        ? []
        : [Text({
            key: "workspace-fleet-summary",
            content: `Fleet activity · ${state.fleetRuns!.runs.length} ${state.fleetRuns!.runs.length === 1 ? "run" : "runs"} · ${state.fleetRuns!.runs.filter(run => run.executionState === "completed").length} completed`,
            variant: "caption",
            color: "textMuted",
          })]),
      ...(sarahIsActive(state)
        ? []
        : state.workspaceLayoutMode === "compact"
        ? [Sheet({
            key: "workspace-lifecycle-sheet",
            open: state.threadLifecycle.actionThreadRef !== null ||
              state.threadLifecycle.deleteConfirmThreadRef !== null,
            dismissable: state.threadLifecycle.pendingAction === null,
            edge: "bottom",
            detents: ["md", "lg"],
            presentationDetents: ["half", "full"],
            onDismiss: IntentRef("WorkspaceLifecycleSheetDismissed", StaticPayload({})),
          }, threadLifecycleRows(state))]
        : threadLifecycleRows(state)),
      ...(sarahIsActive(state) ? [] : codingOfflineCacheAccountingRows(state)),
      Spacer({ key: "drawer-flex-space", size: "8" }),
      drawerRow({ key: "drawer-settings", label: "Settings", onPress: IntentRef("SettingsPressed", StaticPayload({})) }, state.accessibility),
      ...(sarahIsActive(state)
        ? []
        : [Text({ key: "drawer-bundle", content: `Bundle ${BUNDLE_TAG}`, variant: "caption", color: "textMuted" })]),
    ],
  )

export interface HomeProgramOptions {
  readonly khalaTurn?: KhalaTurnClient
  readonly workspaceWidth?: number
  readonly sessionActions?: Readonly<{
    signIn: () => Promise<void>
    signOut: () => Promise<void>
  }>
  readonly conversation?: Extract<MobileConversationSelection, { readonly mode: "sync" }>
  readonly accessibility?: MobileAccessibilityProfile
  readonly sarah?: SarahPrincipalProjection
  /** Initial live `FullAutoRun` mobile projection, when already resolved at
   * selection time (openagents #8982). Later updates flow through
   * `program.fullAuto.setProjection`, not another `buildHomeProgram` call. */
  readonly fullAutoRun?: FullAutoRunProjectionResult
  /** MOB-FA-02 (#8994): dispatches a Pause/Resume/Stop control intent and
   * resolves once a durable applied/rejected/pending outcome is known
   * (`makeFullAutoRunControlDispatcher`). Absent means Full Auto remote
   * control is unavailable on this build; the header renders no buttons. */
  readonly fullAutoControl?: (input: Readonly<{
    runRef: string
    action: FullAutoRunControlAction
  }>) => Promise<FullAutoRunControlDispatchOutcome>
  readonly settings?: Readonly<{
    environments?: MobileEnvironmentConnectionsPort
    notifications?: MobileNotificationSettingsPort
    incomingShare?: MobileShareIntake | null
    onShareConsumed?: () => void
  }>
  readonly coding?: Readonly<{
    directory: MobileCodingDirectory
    portableSnapshot?: ConfirmedPortableSessionSnapshot | null
    attentionSnapshot?: ConfirmedRuntimeAttentionSnapshot | null
    requestPortableAction?: (input: Readonly<{
      sessionRef: string
      action: MobilePortableControlAction
      destinationTargetRef?: string
    }>) => Promise<Readonly<
      | { state: "queued"; snapshot: ConfirmedPortableSessionSnapshot }
      | { state: "rejected"; reason: MobilePortableUnavailableReason; snapshot: ConfirmedPortableSessionSnapshot | null }
    >>
    activeComposer: () => MobileCodingComposerSession | null
    executionTargets?: ReadonlyArray<MobileExecutionTargetOption>
    fleetRuns?: FleetRunClientProjection
    clearSelection: () => Promise<void>
    selectSession: (
      target: MobileCodingTarget,
      onUpdate: (thread: MobileConversationThread) => void,
    ) => Promise<Readonly<{
      thread: MobileConversationThread
      composer: MobileCodingComposerSession | null
    }> | null>
    updateComposerText: (
      session: MobileCodingComposerSession,
      text: string,
    ) => Promise<MobileCodingComposerSession | null>
    selectComposerTarget?: (
      session: MobileCodingComposerSession,
      target: MobileExecutionTargetOption,
    ) => Promise<MobileCodingComposerSession | null>
    pickComposerAttachments: (
      session: MobileCodingComposerSession,
    ) => Promise<MobileCodingAttachmentUpdateResult>
    searchComposerPaths?: MobileComposerPathSearchPort["search"]
    repositoryFiles?: MobileRepositoryFilesPort
    repositoryReview?: MobileRepositoryReviewPort
    repositoryGit?: MobileRepositoryGitPort
    repositoryTerminal?: MobileRepositoryTerminalPort
    removeComposerAttachment?: (
      session: MobileCodingComposerSession,
      attachmentId: string,
    ) => Promise<MobileCodingComposerSession | null>
    retryComposerAttachment?: (
      session: MobileCodingComposerSession,
      attachmentId: string,
    ) => Promise<MobileCodingComposerSession | null>
    prepareComposerSubmission?: (
      session: MobileCodingComposerSession,
      message: string,
    ) => Promise<MobileCodingAttachmentDeliveryResult>
    clearComposer?: (
      session: MobileCodingComposerSession,
    ) => Promise<MobileCodingComposerSession | null>
  }>
}

export type MobileSelectedThreadLeaseController = Readonly<{
  activate: (
    threadRef: string,
    onUpdate: (thread: MobileConversationThread) => void,
  ) => Promise<MobileConversationThread | null>
  clear: () => Promise<void>
  close: () => Promise<void>
  active: () => Readonly<{ generation: number; threadRef: string }> | null
}>

/** Owns the sole ordinary-chat live lease. Coding-session activation retains
 * its separate catalog-owned lease and clears this controller before binding. */
export const makeMobileSelectedThreadLeaseController = (
  host: MobileConversationHost,
): MobileSelectedThreadLeaseController => {
  let generation = 0
  let selectedThreadRef: string | null = null
  let lease: Awaited<ReturnType<NonNullable<MobileConversationHost["watchThread"]>>> = null

  const clear = async (): Promise<void> => {
    generation += 1
    selectedThreadRef = null
    const previous = lease
    lease = null
    await previous?.close()
  }

  return {
    active: () => selectedThreadRef === null
      ? null
      : { generation, threadRef: selectedThreadRef },
    activate: async (threadRef, onUpdate) => {
      await clear()
      const selectedGeneration = generation
      selectedThreadRef = threadRef
      const initial = await host.openThread(threadRef)
      if (selectedGeneration !== generation || selectedThreadRef !== threadRef || initial === null) {
        return null
      }
      if (host.watchThread !== undefined) {
        const opened = await host.watchThread(threadRef, thread => {
          if (selectedGeneration === generation && selectedThreadRef === thread.threadRef) {
            onUpdate(thread)
          }
        })
        if (selectedGeneration !== generation || selectedThreadRef !== threadRef) {
          await opened?.close()
          return null
        }
        lease = opened
      }
      return initial
    },
    clear,
    close: clear,
  }
}

const confirmedKhalaState = (
  thread: MobileConversationThread | null,
  turnCounter = 0,
  interactionActionsAvailable = false,
  runtimeControlActionsAvailable = false,
  previousGraphView: Readonly<{
    agentGraphExpanded: boolean
    selectedAgentRef: string | null
    expandedWorkGroups: Readonly<Record<string, boolean>>
    expandedWorkItems: Readonly<Record<string, boolean>>
  }> | null = null,
): KhalaState => {
  const confirmedGraph = newestLiveAgentGraph(thread?.graphs ?? [])
  const agentGraph = confirmedGraph === null
    ? null
    : projectLiveAgentGraphPresentation(confirmedGraph, {
        maxRows: MOBILE_AGENT_GRAPH_MAX_ROWS,
      })
  // Attention auto-opens the stack; an explicit prior expansion is preserved.
  const agentGraphExpanded = agentGraph !== null &&
    ((previousGraphView?.agentGraphExpanded ?? false) || agentGraph.attentionCount > 0)
  // Selection survives rapid graph replacement through the deterministic
  // shared fallback; no row is auto-inspected before the first explicit tap.
  const selectedAgentRef = agentGraph === null ||
      previousGraphView === null || previousGraphView.selectedAgentRef === null
    ? null
    : resolveLiveAgentGraphSelection(agentGraph, previousGraphView.selectedAgentRef)
  const workGroup = projectMobileWorkGroup(
    thread?.timeline?.run ?? null,
    (thread?.timeline?.events ?? []).filter(event =>
      event.runRef === thread?.timeline?.run?.runRef),
  )
  const runtimeEntries = (thread?.timeline?.events ?? []).flatMap<KhalaState["entries"][number]>(event => {
    const item = event.item
    if (item == null) return []
    switch (item.kind) {
      case "text":
        return [{ key: event.eventRef, role: "assistant" as const, text: item.text, status: "done" as const, createdAt: event.createdAt, version: event.version }]
      case "reasoning":
      case "connected":
      case "tool":
        return []
      case "plan":
        if (item.interactionRef === undefined) return []
        return [{
          key: event.eventRef, role: "system" as const, text: `Plan · ${item.status}`,
          status: "done" as const, createdAt: event.createdAt, version: event.version,
          ...(item.interactionRef === undefined || item.prompt === undefined ||
            (item.status !== "pending" && item.status !== "resolved" && item.status !== "expired" && item.status !== "revoked")
            ? {}
            : { interaction: { kind: "plan_review" as const, interactionRef: item.interactionRef, turnRef: event.runRef, status: item.status, title: "Review plan", prompt: item.prompt, questions: [], ...(item.decisionRef === undefined ? {} : { decisionRef: item.decisionRef }) } }),
        }]
      case "usage":
      case "terminal":
      case "interrupted":
      case "heartbeat":
      case "reconnect":
      case "stale":
      case "error":
        return []
      case "approval":
        return [{
          key: event.eventRef, role: "system" as const, text: `Approval · ${item.status}`,
          status: "done" as const, createdAt: event.createdAt, version: event.version,
          ...(item.interactionRef === undefined || item.prompt === undefined ||
            (item.status !== "pending" && item.status !== "resolved" && item.status !== "expired" && item.status !== "revoked")
            ? {}
            : { interaction: { kind: "tool_approval" as const, interactionRef: item.interactionRef, turnRef: event.runRef, status: item.status, title: "Tool approval", prompt: item.prompt, questions: [], ...(item.decisionRef === undefined ? {} : { decisionRef: item.decisionRef }) } }),
        }]
      case "question":
        return [{
          key: event.eventRef, role: "system" as const, text: item.prompt,
          status: "done" as const, createdAt: event.createdAt, version: event.version,
          ...(item.status === "pending" || item.status === "resolved" || item.status === "expired" || item.status === "revoked"
            ? { interaction: { kind: "provider_question" as const, interactionRef: item.questionRef, turnRef: event.runRef, status: item.status, title: item.title ?? "Question", prompt: item.prompt, questions: item.questions ?? [], ...(item.decisionRef === undefined ? {} : { decisionRef: item.decisionRef }) } }
            : {}),
        }]
    }
  })
  const messageEntries = (thread?.messages ?? []).map(message => ({
    key: message.messageRef,
    role: "user" as const,
    text: message.body,
    status: "done" as const,
    createdAt: message.createdAt,
    version: message.version,
    ...(message.attachments === undefined ? {} : { attachments: message.attachments }),
  }))
  const entries: KhalaState["entries"] = [
    ...messageEntries,
    ...runtimeEntries,
    ...(workGroup === null ? [] : [{
      key: workGroup.groupRef,
      role: "tool" as const,
      text: workGroup.summary,
      status: workGroup.status === "failure"
        ? "failed" as const
        : workGroup.status === "running" ? "pending" as const : "done" as const,
      createdAt: workGroup.createdAt,
      version: thread?.timeline?.run?.version,
      work: workGroup,
    }]),
  ].sort((left, right) =>
    (left.createdAt ?? "").localeCompare(right.createdAt ?? ""))
  return {
    draft: "",
    entries,
    // A confirmed running turn is observable state, not an in-flight mobile
    // mutation. Keep the composer available so a second device can safely
    // append to that exact run. Only pre-dispatch queued state blocks input.
    pending: thread?.timeline?.run?.status === "queued",
    turnCounter,
    interactionSelections: {},
    interactionSubmittingRef: null,
    interactionActionsAvailable,
    expandedWorkGroups: previousGraphView?.expandedWorkGroups ?? {},
    expandedWorkItems: previousGraphView?.expandedWorkItems ?? {},
    transcriptVisibleCount: initialMobileTranscriptVisibleCount(entries.length),
    transcriptPinned: true,
    transcriptUnreadCount: 0,
    transcriptScrollToKey: null,
    attachmentPreviewStates: {},
    attachmentRetryEpochs: {},
    viewingAttachmentRef: null,
    runtimeTurn: thread?.timeline?.run === null || thread?.timeline?.run === undefined
      ? null
      : {
          runRef: thread.timeline.run.runRef,
          status: thread.timeline.run.status,
        },
    runtimeControlSubmittingAction: null,
    runtimeControlActionsAvailable,
    runtimeStopConfirmationRunRef: null,
    runtimeQueueReceipt: null,
    agentGraph,
    agentGraphExpanded,
    selectedAgentRef,
    threadHistory: thread === null
      ? null
      : {
          title: thread.title,
          totalMessageCount: thread.messageCount,
          retainedMessageCount: thread.messages.length,
          retainedEventCount: thread.timeline?.events.length ?? 0,
        },
  }
}

const withConfirmedThread = (
  state: HomeState,
  thread: MobileConversationThread,
): HomeState => {
  const sameThread = state.activeThreadRef === thread.threadRef
  const nextKhala = confirmedKhalaState(
    thread,
    state.khala.turnCounter,
    state.khala.interactionActionsAvailable,
    state.khala.runtimeControlActionsAvailable,
    sameThread
      ? {
          agentGraphExpanded: state.khala.agentGraphExpanded,
          selectedAgentRef: state.khala.selectedAgentRef,
          expandedWorkGroups: state.khala.expandedWorkGroups,
          expandedWorkItems: state.khala.expandedWorkItems,
        }
      : null,
  )
  const addedEntryCount = sameThread
    ? newlyConfirmedTranscriptEntryCount(state.khala.entries, nextKhala.entries)
    : 0
  const khala: KhalaState = {
    ...nextKhala,
    draft: state.khala.draft,
    ...(sameThread
      ? {
          transcriptVisibleCount: Math.min(
            nextKhala.entries.length,
            Math.max(
              state.khala.transcriptVisibleCount + (state.khala.transcriptPinned ? 0 : addedEntryCount),
              initialMobileTranscriptVisibleCount(nextKhala.entries.length),
            ),
          ),
          transcriptPinned: state.khala.transcriptPinned,
          transcriptUnreadCount: state.khala.transcriptPinned
            ? 0
            : state.khala.transcriptUnreadCount + addedEntryCount,
          transcriptScrollToKey: state.khala.transcriptPinned
            ? state.khala.transcriptScrollToKey
            : null,
          attachmentPreviewStates: state.khala.attachmentPreviewStates,
          attachmentRetryEpochs: state.khala.attachmentRetryEpochs,
          viewingAttachmentRef: state.khala.viewingAttachmentRef,
          runtimeStopConfirmationRunRef:
            nextKhala.runtimeTurn !== null &&
              (nextKhala.runtimeTurn.status === "queued" ||
                nextKhala.runtimeTurn.status === "running" ||
                nextKhala.runtimeTurn.status === "waiting_for_input") &&
              nextKhala.runtimeTurn.runRef === state.khala.runtimeStopConfirmationRunRef
              ? state.khala.runtimeStopConfirmationRunRef
              : null,
          runtimeQueueReceipt: state.khala.runtimeQueueReceipt === null ||
              nextKhala.runtimeTurn === null ||
              nextKhala.runtimeTurn.runRef === state.khala.runtimeQueueReceipt.parentRunRef
            ? state.khala.runtimeQueueReceipt
            : null,
        }
      : {}),
  }
  return {
    ...state,
    drawerOpen: false,
    workspaceFocusTarget: "transcript",
    surfaceMode: "khala",
    activeThreadRef: thread.threadRef,
    conversationThreads: [
      {
        threadRef: thread.threadRef,
        title: thread.title,
        status: thread.status,
        messageCount: thread.messageCount,
        lastMessageAt: thread.lastMessageAt,
        updatedAt: thread.updatedAt,
        version: thread.version,
      },
      ...state.conversationThreads.filter(item => item.threadRef !== thread.threadRef),
    ],
    khala,
  }
}

const failedConversationState = (
  state: HomeState,
  error: string,
): HomeState => ({
  ...state,
  khala: {
    ...state.khala,
    pending: false,
    entries: [
      ...state.khala.entries.filter(entry => entry.status !== "pending"),
      {
        key: `sync-error-${state.khala.turnCounter}`,
        role: "system",
        text: error,
        status: "failed",
      },
    ],
  },
})

const refreshComposerPathDiscovery = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  coding: NonNullable<HomeProgramOptions["coding"]>,
  composer: MobileCodingComposerSession,
  text: string,
) => Effect.gen(function* () {
  const trigger = mobileComposerPathTrigger(text)
  if (trigger === null) {
    yield* SubscriptionRef.update(state, current =>
      current.codingPathDiscovery.state === "idle"
        ? current
        : { ...current, codingPathDiscovery: { state: "idle" as const } })
    return
  }
  const query = normalizeMobileComposerPathQuery(trigger.query)
  const repository = composer.draft.context.find(item => item.kind === "repository")
  const worktree = composer.draft.context.find(item => item.kind === "worktree")
  if (repository?.kind !== "repository" || worktree?.kind !== "worktree" ||
    coding.searchComposerPaths === undefined) {
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      codingPathDiscovery: {
        state: "unavailable" as const,
        query,
        message: "Connect the exact worktree environment to search repository files.",
      },
    }))
    return
  }
  if (query === "") {
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      codingPathDiscovery: { state: "ready" as const, query, entries: [] },
    }))
    return
  }
  yield* SubscriptionRef.update(state, current => ({
    ...current,
    codingPathDiscovery: { state: "loading" as const, query },
  }))
  const result = yield* Effect.promise(() => searchMobileComposerPaths(
    { search: coding.searchComposerPaths! },
    {
      repositoryRef: repository.repositoryRef,
      worktreeRef: worktree.worktreeRef,
      query,
    },
  ))
  yield* SubscriptionRef.update(state, current => {
    const currentTrigger = mobileComposerPathTrigger(current.khala.draft)
    if (current.codingComposer?.draft.draftRef !== composer.draft.draftRef ||
      normalizeMobileComposerPathQuery(currentTrigger?.query ?? "") !== query) return current
    return {
      ...current,
      codingPathDiscovery: result.state === "ready"
        ? { state: "ready" as const, query, entries: result.page.entries }
        : { state: "failed" as const, query, message: result.message },
    }
  })
})

export const initialHomeStateForConversation = (
  selection: HomeProgramOptions["conversation"],
  accessibility: MobileAccessibilityProfile = defaultMobileAccessibilityProfile,
): HomeState => {
  const profile = normalizeMobileAccessibilityProfile(accessibility)
  return selection === undefined
    ? { ...initialHomeState, accessibility: profile }
    : {
        ...initialHomeState,
        accessibility: profile,
        syncPhase: "live",
        conversationAuthority: "sync",
        conversationThreads: selection.threads,
        archivedConversationThreads: selection.archivedThreads,
        activeThreadRef: selection.activeThread?.threadRef ?? null,
        codingDirectory: null,
        khala: confirmedKhalaState(
          selection.activeThread,
          0,
          selection.host.decideInteraction !== undefined,
          selection.host.controlTurn !== undefined,
        ),
      }
}

const makeSyncedConversationHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  host: MobileConversationHost,
  coding: HomeProgramOptions["coding"],
  selectedThreadLease: MobileSelectedThreadLeaseController,
) => {
  const reconcileLifecycle = (
    action: "archive" | "delete" | "rename" | "restore",
    threadRef: string,
    title?: string,
  ) => Effect.gen(function* () {
    if (host.updateThread === undefined) return
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      threadLifecycle: { ...current.threadLifecycle, pendingAction: action, notice: null },
    }))
    const result = yield* Effect.promise(() => host.updateThread!({
      action,
      threadRef,
      ...(title === undefined ? {} : { title }),
    }))
    if (result.ok && result.thread.status !== "active" &&
        selectedThreadLease.active()?.threadRef === result.thread.threadRef) {
      yield* Effect.promise(selectedThreadLease.clear)
    }
    yield* SubscriptionRef.update(state, current => {
      if (!result.ok) return {
        ...current,
        threadLifecycle: {
          ...current.threadLifecycle,
          pendingAction: null,
          notice: { kind: "rejected" as const, message: result.error },
        },
      }
      const activeWithout = current.conversationThreads.filter(thread => thread.threadRef !== threadRef)
      const archivedWithout = current.archivedConversationThreads.filter(thread => thread.threadRef !== threadRef)
      const remainsActive = result.thread.status === "active"
      const remainsArchived = result.thread.status === "archived"
      const activeRemoved = current.activeThreadRef === threadRef && !remainsActive
      return {
        ...current,
        conversationThreads: remainsActive ? [result.thread, ...activeWithout] : activeWithout,
        archivedConversationThreads: remainsArchived ? [result.thread, ...archivedWithout] : archivedWithout,
        activeThreadRef: activeRemoved ? null : current.activeThreadRef,
        codingComposer: activeRemoved ? null : current.codingComposer,
        codingComposerTargetPickerOpen: activeRemoved ? false : current.codingComposerTargetPickerOpen,
        codingComposerTargetSearch: activeRemoved ? "" : current.codingComposerTargetSearch,
        codingPathDiscovery: activeRemoved ? { state: "idle" as const } : current.codingPathDiscovery,
        codingAttachmentMutatingRef: activeRemoved ? null : current.codingAttachmentMutatingRef,
        threadLifecycle: {
          actionThreadRef: null,
          editingThreadRef: null,
          renameDraft: "",
          deleteConfirmThreadRef: null,
          pendingAction: null,
          notice: {
            kind: "confirmed" as const,
            message: action === "rename"
              ? "Chat title confirmed."
              : action === "archive"
                ? "Chat archived on every synced device."
                : action === "restore"
                  ? "Chat restored."
                  : "Chat deleted from synced navigation.",
          },
        },
        khala: activeRemoved
          ? confirmedKhalaState(
              null,
              current.khala.turnCounter,
              current.khala.interactionActionsAvailable,
              current.khala.runtimeControlActionsAvailable,
            )
          : current.khala,
      }
    })
  })

  return ({
  NewChatPressed: () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    if (before.khala.pending) return
    yield* Effect.promise(selectedThreadLease.clear)
    if (coding !== undefined) yield* Effect.promise(coding.clearSelection)
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      workspaceFocusTarget: "transcript" as const,
      surfaceMode: "khala" as const,
      workbenchRoute: "conversation" as const,
      repositoryBrowser: initialMobileRepositoryBrowserState,
      repositoryReview: initialMobileRepositoryReviewState,
      repositoryGit: initialMobileRepositoryGitState,
      repositoryTerminal: initialMobileRepositoryTerminalState,
      codingComposer: null,
      codingComposerTargetPickerOpen: false,
      codingComposerTargetSearch: "",
      codingPathDiscovery: { state: "idle" as const },
      codingAttachmentPicking: false,
      codingAttachmentMutatingRef: null,
      codingAttachmentStatus: null,
      khala: {
        ...current.khala,
        pending: true,
        entries: [{
          key: `pending-new-thread-${current.khala.turnCounter + 1}`,
          role: "system" as const,
          text: "Creating chat…",
          status: "pending" as const,
        }],
      },
    }))
    const result = yield* Effect.promise(host.newThread)
    const confirmed = !result.ok
      ? null
      : yield* Effect.promise(() => selectedThreadLease.activate(
          result.thread.threadRef,
          update => {
            Effect.runFork(SubscriptionRef.update(state, current =>
              current.activeThreadRef === update.threadRef
                ? withConfirmedThread(current, update)
                : current))
          },
        ))
    yield* SubscriptionRef.update(state, current => result.ok
      ? withConfirmedThread(current, confirmed ?? result.thread)
      : failedConversationState(current, result.error))
  }),
  ConversationThreadSelected: (payload: { readonly threadRef: string }) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const selectingSarah = before.sarah?.threadRef === payload.threadRef
    if (before.khala.pending && !selectingSarah) {
      return
    }
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      activeThreadRef: payload.threadRef,
      workspaceFocusTarget: "transcript" as const,
      workbenchRoute: "conversation" as const,
      repositoryBrowser: initialMobileRepositoryBrowserState,
      repositoryReview: initialMobileRepositoryReviewState,
      repositoryGit: initialMobileRepositoryGitState,
      repositoryTerminal: initialMobileRepositoryTerminalState,
      codingComposer: null,
      codingComposerTargetPickerOpen: false,
      codingComposerTargetSearch: "",
      codingPathDiscovery: { state: "idle" as const },
      codingAttachmentPicking: false,
      codingAttachmentMutatingRef: null,
      codingAttachmentStatus: null,
      khala: selectingSarah
        ? {
            ...confirmedKhalaState(
              null,
              current.khala.turnCounter,
              current.khala.interactionActionsAvailable,
              current.khala.runtimeControlActionsAvailable,
            ),
            pending: true,
            entries: [{
              key: "pending-sarah-thread",
              role: "system" as const,
              text: "Opening Sarah…",
              status: "pending" as const,
            }],
          }
        : { ...current.khala, pending: true },
    }))
    if (coding !== undefined) {
      yield* Effect.promise(coding.clearSelection).pipe(Effect.ignore)
    }
    const thread = yield* Effect.promise(() => selectedThreadLease.activate(
      payload.threadRef,
      update => {
        Effect.runFork(SubscriptionRef.update(state, current =>
          current.activeThreadRef === update.threadRef
            ? withConfirmedThread(current, update)
            : current))
      },
    ))
    yield* SubscriptionRef.update(state, current => thread === null
      ? failedConversationState(current, "Conversation is still pending reconciliation.")
      : withConfirmedThread(current, thread))
  }),
  ConversationThreadRenameStarted: (payload: { readonly threadRef: string }) =>
    SubscriptionRef.update(state, current => {
      const thread = current.conversationThreads.find(item => item.threadRef === payload.threadRef)
      return thread === undefined || current.threadLifecycle.pendingAction !== null
        ? current
        : {
            ...current,
            threadLifecycle: {
              ...current.threadLifecycle,
              actionThreadRef: thread.threadRef,
              editingThreadRef: thread.threadRef,
              renameDraft: thread.title,
              deleteConfirmThreadRef: null,
              notice: null,
            },
          }
    }),
  ConversationThreadRenameChanged: (text: string) =>
    SubscriptionRef.update(state, current => ({
      ...current,
      threadLifecycle: { ...current.threadLifecycle, renameDraft: text.slice(0, 160) },
    })),
  ConversationThreadRenameSubmitted: () => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const threadRef = current.threadLifecycle.editingThreadRef
    const title = current.threadLifecycle.renameDraft.trim()
    if (threadRef === null || title === "" || current.threadLifecycle.pendingAction !== null) return
    yield* reconcileLifecycle("rename", threadRef, title)
  }),
  ConversationThreadRenameCancelled: () => SubscriptionRef.update(state, current => ({
    ...current,
    threadLifecycle: { ...current.threadLifecycle, editingThreadRef: null, renameDraft: "" },
  })),
  ConversationThreadLifecycleRequested: (payload: Readonly<{
    action: "archive" | "restore"
    threadRef: string
  }>) => reconcileLifecycle(payload.action, payload.threadRef),
  ConversationThreadDeleteRequested: (payload: { readonly threadRef: string }) =>
    SubscriptionRef.update(state, current => current.threadLifecycle.pendingAction !== null
      ? current
      : {
          ...current,
          threadLifecycle: {
            ...current.threadLifecycle,
            editingThreadRef: null,
            renameDraft: "",
            deleteConfirmThreadRef: payload.threadRef,
            notice: null,
          },
        }),
  ConversationThreadDeleteConfirmed: () => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    if (current.threadLifecycle.deleteConfirmThreadRef === null ||
        current.threadLifecycle.pendingAction !== null) return
    yield* reconcileLifecycle("delete", current.threadLifecycle.deleteConfirmThreadRef)
  }),
  ConversationThreadDeleteCancelled: () => SubscriptionRef.update(state, current => ({
    ...current,
    threadLifecycle: { ...current.threadLifecycle, deleteConfirmThreadRef: null },
  })),
  KhalaDraftChanged: (text: string) => Effect.gen(function* () {
    const bounded = text.length > 4_000 ? `${text.slice(0, 4_000)}…` : text
    const before = yield* SubscriptionRef.get(state)
    const composer = before.codingComposer
    if (composer === null || coding === undefined) {
      const pathTrigger = mobileComposerPathTrigger(bounded)
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        codingPathDiscovery: pathTrigger === null
          ? { state: "idle" as const }
          : {
              state: "unavailable" as const,
              query: normalizeMobileComposerPathQuery(pathTrigger.query),
              message: "Open a coding session before searching repository files.",
            },
        khala: { ...current.khala, draft: bounded },
      }))
      return
    }
    const updated = yield* Effect.promise(() =>
      coding.updateComposerText(composer, bounded))
    if (updated === null) return
    yield* SubscriptionRef.update(state, current =>
      current.codingComposer?.draft.draftRef !== composer.draft.draftRef
        ? current
        : {
            ...current,
            codingComposer: updated,
            khala: { ...current.khala, draft: bounded },
          })
    yield* refreshComposerPathDiscovery(state, coding, updated, bounded)
  }),
  RuntimeInteractionOptionToggled: (payload: Readonly<{
    interactionRef: string
    questionRef: string
    optionRef: string
    multiSelect: boolean
  }>) => SubscriptionRef.update(state, current => {
    if (current.khala.interactionSubmittingRef !== null) return current
    const interaction = current.khala.entries.find(entry =>
      entry.interaction?.interactionRef === payload.interactionRef)?.interaction
    if (interaction?.status !== "pending") return current
    const interactionSelections = current.khala.interactionSelections[payload.interactionRef] ?? {}
    const selected = interactionSelections[payload.questionRef] ?? []
    const next = payload.multiSelect
      ? selected.includes(payload.optionRef)
        ? selected.filter(value => value !== payload.optionRef)
        : [...selected, payload.optionRef]
      : [payload.optionRef]
    return {
      ...current,
      khala: {
        ...current.khala,
        interactionSelections: {
          ...current.khala.interactionSelections,
          [payload.interactionRef]: {
            ...interactionSelections,
            [payload.questionRef]: next,
          },
        },
      },
    }
  }),
  RuntimeInteractionDecisionSubmitted: (payload: Readonly<
    | { interactionRef: string; turnRef: string; kind: "provider_question" }
    | { interactionRef: string; turnRef: string; kind: "tool_approval"; outcome: "approve" | "deny" }
    | { interactionRef: string; turnRef: string; kind: "plan_review"; outcome: "accept" | "request_changes" | "replan" }
  >) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const interaction = before.khala.entries.find(entry =>
      entry.interaction?.interactionRef === payload.interactionRef)?.interaction
    if (
      interaction?.status !== "pending" || interaction.kind !== payload.kind ||
      before.activeThreadRef === null || host.decideInteraction === undefined ||
      before.khala.interactionSubmittingRef !== null
    ) return
    const selections = before.khala.interactionSelections[payload.interactionRef] ?? {}
    const decision = payload.kind === "provider_question"
      ? {
          kind: "provider_question" as const,
          answers: interaction.questions.map(question => ({
            questionRef: question.questionRef,
            optionRefs: [...(selections[question.questionRef] ?? [])],
          })),
        }
      : payload.kind === "tool_approval"
        ? { kind: "tool_approval" as const, outcome: payload.outcome }
        : { kind: "plan_review" as const, outcome: payload.outcome }
    if (decision.kind === "provider_question" &&
      decision.answers.some(answer => answer.optionRefs.length === 0)) return
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      khala: { ...current.khala, interactionSubmittingRef: payload.interactionRef },
    }))
    const result = yield* Effect.promise(() => host.decideInteraction!({
      interactionRef: payload.interactionRef,
      threadRef: before.activeThreadRef!,
      turnRef: payload.turnRef,
      decision,
      onUpdate: thread => {
        Effect.runFork(SubscriptionRef.update(state, current =>
          current.activeThreadRef === thread.threadRef
            ? withConfirmedThread(current, thread)
            : current))
      },
    }))
    yield* SubscriptionRef.update(state, current => result.ok
      ? withConfirmedThread(current, result.thread)
      : {
          ...failedConversationState(current, result.error),
          khala: {
            ...failedConversationState(current, result.error).khala,
            interactionSubmittingRef: null,
          },
        })
  }),
  RuntimeTurnControlRequested: (payload: Readonly<{
    action: MobileRuntimeControlAction
    runRef: string
  }>) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const turn = before.khala.runtimeTurn
    if (
      before.activeThreadRef === null || host.controlTurn === undefined ||
      before.khala.runtimeControlSubmittingAction !== null ||
      turn?.runRef !== payload.runRef
    ) return
    const allowed = payload.action === "cancel"
      ? turn.status === "queued" || turn.status === "running" ||
        turn.status === "waiting_for_input"
      : payload.action === "resume"
        ? turn.status === "canceled"
        : turn.status === "completed" || turn.status === "failed" ||
          turn.status === "canceled"
    if (!allowed) return
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      khala: {
        ...current.khala,
        runtimeControlSubmittingAction: payload.action,
        runtimeStopConfirmationRunRef: null,
      },
    }))
    const result = yield* Effect.promise(() => host.controlTurn!({
      action: payload.action,
      runRef: payload.runRef,
      threadRef: before.activeThreadRef!,
      onUpdate: thread => {
        Effect.runFork(SubscriptionRef.update(state, current => {
          if (current.activeThreadRef !== thread.threadRef) return current
          const updated = withConfirmedThread(current, thread)
          return {
            ...updated,
            khala: {
              ...updated.khala,
              runtimeControlSubmittingAction:
                current.khala.runtimeControlSubmittingAction,
            },
          }
        }))
      },
    }))
    yield* SubscriptionRef.update(state, current => result.ok
      ? withConfirmedThread(current, result.thread)
      : {
          ...failedConversationState(current, result.error),
          khala: {
            ...failedConversationState(current, result.error).khala,
            runtimeControlSubmittingAction: null,
            runtimeStopConfirmationRunRef: null,
          },
        })
  }),
  KhalaTurnSubmitted: (raw: string) => Effect.gen(function* () {
    const message = raw.trim()
    const before = yield* SubscriptionRef.get(state)
    const sarahSelected = before.sarah !== null &&
      before.activeThreadRef === before.sarah.threadRef
    const composer = sarahSelected ? null : before.codingComposer
    if (message === "" && (composer?.draft.doc.attachments.length ?? 0) === 0) return
    if (before.khala.pending) return
    if (composer !== null && composer.draft.target.readiness !== "ready") return
    const selectedExecutionTarget = composer === null
      ? undefined
      : before.codingExecutionTargets.find(option =>
          option.targetId === composer.draft.target.executionTargetRef &&
          option.readiness === "ready")
    if (composer !== null &&
      before.codingExecutionTargetCatalogRequired &&
      selectedExecutionTarget === undefined) {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        codingAttachmentStatus: {
          kind: "failed" as const,
          message: "The selected execution target is no longer available. Your draft was kept.",
        },
      }))
      return
    }
    const prepared = composer === null || coding === undefined
      ? { ok: true as const, body: message }
      : coding.prepareComposerSubmission === undefined
        ? composer.draft.doc.attachments.length === 0
          ? { ok: true as const, body: message }
          : { ok: false as const, error: "Attachment delivery is unavailable. The draft was kept." }
        : yield* Effect.promise(() => coding.prepareComposerSubmission!(
            composer,
            message,
          ))
    if (!prepared.ok) {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        codingAttachmentStatus: { kind: "failed" as const, message: prepared.error },
      }))
      return
    }
    const turn = before.khala.turnCounter + 1
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      codingComposer: sarahSelected ? null : current.codingComposer,
      khala: {
        ...current.khala,
        draft: "",
        pending: true,
        turnCounter: turn,
        entries: [
          ...current.khala.entries,
          {
            key: `pending-mobile-${turn}`,
            role: "user" as const,
            text: message.length > 4_000 ? `${message.slice(0, 4_000)}…` : message,
            status: "pending" as const,
          },
        ],
      },
    }))

    let threadRef = before.activeThreadRef
    if (threadRef === null) {
      const created = yield* Effect.promise(host.newThread)
      if (!created.ok) {
        yield* SubscriptionRef.update(state, current => failedConversationState(current, created.error))
        return
      }
      threadRef = created.thread.threadRef
      yield* SubscriptionRef.update(state, current => ({
        ...withConfirmedThread(current, created.thread),
        khala: {
          ...confirmedKhalaState(
            created.thread,
            turn,
            current.khala.interactionActionsAvailable,
            current.khala.runtimeControlActionsAvailable,
          ),
          pending: true,
          entries: current.khala.entries,
        },
      }))
      yield* Effect.promise(() => selectedThreadLease.activate(
        created.thread.threadRef,
        update => {
          Effect.runFork(SubscriptionRef.update(state, current =>
            current.activeThreadRef === update.threadRef
              ? withConfirmedThread(current, update)
              : current))
        },
      ))
    }

    const submittedThreadRef = threadRef
    const result = yield* Effect.promise(() => host.sendMessage({
      threadRef: submittedThreadRef,
      body: prepared.body,
      ...(prepared.attachments === undefined ? {} : { attachments: prepared.attachments }),
      ...(selectedExecutionTarget === undefined
        ? sarahSelected
          ? { runtimeTarget: { lane: "hosted_khala" as const } }
          : {}
        : { runtimeTarget: selectedExecutionTarget.runtimeTarget }),
      onUpdate: thread => {
        Effect.runFork(SubscriptionRef.update(state, current => {
          if (current.activeThreadRef !== thread.threadRef) return current
          const updated = withConfirmedThread(current, thread)
          return {
            ...updated,
            khala: { ...updated.khala, pending: current.khala.pending },
          }
        }))
      },
    }))
    const queuedForReconciliation = !result.ok && result.queuedForReconciliation === true
    const settledComposer = (!result.ok && !queuedForReconciliation) || composer === null || coding === undefined
      ? composer
      : coding.clearComposer === undefined
        ? yield* Effect.promise(() => coding.updateComposerText(composer, ""))
        : yield* Effect.promise(() => coding.clearComposer!(composer))
    yield* SubscriptionRef.update(state, current =>
      current.activeThreadRef !== submittedThreadRef
        ? current
        : result.ok
      ? (() => {
          const confirmed = withConfirmedThread(current, result.thread)
          return {
            ...confirmed,
            codingComposer: settledComposer,
            khala: {
              ...confirmed.khala,
              runtimeQueueReceipt: result.queueReceipt ?? confirmed.khala.runtimeQueueReceipt,
            },
          }
        })()
      : queuedForReconciliation
        ? {
            ...current,
            codingComposer: settledComposer,
            khala: {
              ...current.khala,
              draft: "",
              pending: true,
            },
          }
      : {
          ...failedConversationState(current, result.error),
          codingComposer: settledComposer,
          khala: {
            ...failedConversationState(current, result.error).khala,
            draft: settledComposer === null
              ? message
              : mobileCodingComposerText(settledComposer.draft),
          },
        })
  }),
})
}

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  options: HomeProgramOptions,
  selectedThreadLease: MobileSelectedThreadLeaseController,
): IntentHandlers<typeof homeIntentDefinitions> => {
  const synced = options.conversation === undefined
    ? undefined
    : makeSyncedConversationHandlers(
        state,
        options.conversation.host,
        options.coding,
        selectedThreadLease,
      )
  const repositoryScope = (composer: MobileCodingComposerSession | null): MobileRepositoryScope | null => {
    if (composer === null) return null
    const repository = composer.draft.context.find(item => item.kind === "repository")
    const worktree = composer.draft.context.find(item => item.kind === "worktree")
    return repository?.kind !== "repository" || worktree?.kind !== "worktree"
      ? null
      : {
          sessionRef: composer.draft.sessionRef,
          repositoryRef: repository.repositoryRef,
          worktreeRef: worktree.worktreeRef,
        }
  }
  const loadRepositoryRoot = () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const scope = repositoryScope(before.codingComposer)
    const epoch = before.repositoryBrowser.requestEpoch + 1
    if (scope === null || options.coding?.repositoryFiles === undefined) {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        workbenchRoute: "files" as const,
        repositoryBrowser: {
          ...initialMobileRepositoryBrowserState,
          scope,
          state: "unavailable" as const,
          requestEpoch: epoch,
          message: scope === null
            ? "Select a coding session with an exact repository and worktree first."
            : "Connect the paired worktree environment to browse repository files.",
        },
      }))
      return
    }
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      workbenchRoute: "files" as const,
      drawerOpen: false,
      workspaceFocusTarget: "transcript" as const,
      repositoryBrowser: {
        ...initialMobileRepositoryBrowserState,
        scope,
        state: "loading" as const,
        requestEpoch: epoch,
        message: null,
      },
    }))
    const result = yield* Effect.promise(() => loadMobileRepositoryTree(
      options.coding!.repositoryFiles!,
      { ...scope, directoryRef: "", cursor: null },
    ))
    yield* SubscriptionRef.update(state, current => {
      const browser = current.repositoryBrowser
      if (current.workbenchRoute !== "files" || browser.requestEpoch !== epoch ||
        browser.scope?.sessionRef !== scope.sessionRef ||
        browser.scope.repositoryRef !== scope.repositoryRef ||
        browser.scope.worktreeRef !== scope.worktreeRef) return current
      return result.state === "ready"
        ? {
            ...current,
            repositoryBrowser: {
              ...browser,
              state: "ready" as const,
              pages: { "": result.page },
              message: null,
            },
          }
        : {
            ...current,
            repositoryBrowser: { ...browser, state: "failed" as const, message: result.message },
          }
    })
  })
  const loadRepositoryChanges = () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const scope = repositoryScope(before.codingComposer)
    const epoch = before.repositoryReview.requestEpoch + 1
    if (scope === null || options.coding?.repositoryReview === undefined) {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        workbenchRoute: "changes" as const,
        repositoryReview: {
          ...initialMobileRepositoryReviewState,
          scope,
          state: "unavailable" as const,
          requestEpoch: epoch,
          message: scope === null
            ? "Select an exact coding worktree before reviewing changes."
            : "Connect the paired worktree environment to review changes.",
        },
      }))
      return
    }
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      workbenchRoute: "changes" as const,
      drawerOpen: false,
      repositoryReview: { ...initialMobileRepositoryReviewState, scope, state: "loading" as const, requestEpoch: epoch, message: null },
    }))
    let raw: unknown
    try { raw = yield* Effect.promise(() => options.coding!.repositoryReview!.status(scope)) } catch { raw = null }
    const summary = decodeMobileChangeSummary(raw, scope)
    yield* SubscriptionRef.update(state, current =>
      current.workbenchRoute !== "changes" || current.repositoryReview.requestEpoch !== epoch ||
        current.repositoryReview.scope?.sessionRef !== scope.sessionRef
        ? current
        : summary === null
          ? { ...current, repositoryReview: { ...current.repositoryReview, state: "failed" as const, message: "The environment returned an invalid or unavailable change summary." } }
          : { ...current, repositoryReview: { ...current.repositoryReview, state: "ready" as const, summary, message: null } })
  })
  const loadRepositoryGit = () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const scope = repositoryScope(before.codingComposer)
    const epoch = before.repositoryGit.requestEpoch + 1
    if (scope === null || options.coding?.repositoryGit === undefined) {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        workbenchRoute: "git" as const,
        repositoryGit: {
          ...initialMobileRepositoryGitState,
          scope,
          state: "unavailable" as const,
          requestEpoch: epoch,
          message: scope === null
            ? "Select an exact coding worktree before using Git."
            : "Connect the paired worktree environment to use Git.",
        },
      }))
      return
    }
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      workbenchRoute: "git" as const,
      repositoryGit: { ...initialMobileRepositoryGitState, scope, state: "loading" as const, requestEpoch: epoch },
    }))
    let raw: unknown
    try { raw = yield* Effect.promise(() => options.coding!.repositoryGit!.gitStatus(scope)) } catch { raw = null }
    const status = decodeMobileGitStatus(raw, scope)
    yield* SubscriptionRef.update(state, current =>
      current.workbenchRoute !== "git" || current.repositoryGit.requestEpoch !== epoch ||
        current.repositoryGit.scope?.sessionRef !== scope.sessionRef
        ? current
        : status === null
          ? { ...current, repositoryGit: { ...current.repositoryGit, state: "failed" as const, message: "The environment returned invalid or unavailable Git status." } }
          : { ...current, repositoryGit: { ...current.repositoryGit, state: "ready" as const, status,
            selectedPaths: status.files.filter(file => file.status !== "unmerged").map(file => file.pathRef), message: null } })
  })
  const runRepositoryGitMutation = (request: MobileGitMutationRequest) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    if (before.workbenchRoute !== "git" || before.repositoryGit.status?.statusRef !== request.statusRef ||
      before.repositoryGit.pendingConfirmation?.idempotencyRef !== request.idempotencyRef ||
      options.coding?.repositoryGit === undefined) return
    const epoch = before.repositoryGit.requestEpoch + 1
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      repositoryGit: { ...current.repositoryGit, requestEpoch: epoch, submitting: true, message: null, failureCode: null },
    }))
    let raw: unknown
    try { raw = yield* Effect.promise(() => options.coding!.repositoryGit!.gitMutate(request)) } catch { raw = null }
    const result = decodeMobileGitMutationResult(raw, request)
    yield* SubscriptionRef.update(state, current => {
      if (current.workbenchRoute !== "git" || current.repositoryGit.requestEpoch !== epoch ||
        current.repositoryGit.pendingConfirmation?.idempotencyRef !== request.idempotencyRef ||
        current.repositoryGit.status?.statusRef !== request.statusRef) return current
      if (result === null) return { ...current, repositoryGit: { ...current.repositoryGit, submitting: false,
        pendingConfirmation: null, failureCode: "operation_failed" as const, message: "The environment returned no valid Git receipt." } }
      if ("code" in result) return { ...current, repositoryGit: { ...current.repositoryGit, submitting: false,
        pendingConfirmation: null, failureCode: result.code, message: result.message } }
      return { ...current, repositoryGit: { ...current.repositoryGit, state: "ready" as const, status: result.status,
        selectedPaths: result.status.files.filter(file => file.status !== "unmerged").map(file => file.pathRef),
        commitMessage: request.op === "commit" ? "" : current.repositoryGit.commitMessage,
        pendingConfirmation: null, submitting: false, receipts: [...current.repositoryGit.receipts, result],
        failureCode: null, message: null } }
    })
  })
  const loadRepositoryTerminals = () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const scope = repositoryScope(before.codingComposer)
    const epoch = before.repositoryTerminal.requestEpoch + 1
    if (scope === null || options.coding?.repositoryTerminal === undefined) {
      yield* SubscriptionRef.update(state, current => ({ ...current, workbenchRoute: "terminal" as const,
        repositoryTerminal: { ...initialMobileRepositoryTerminalState, scope, state: "unavailable" as const,
          requestEpoch: epoch, message: scope === null ? "Select an exact coding worktree before opening a terminal." : "Connect the paired worktree environment to use terminals." } }))
      return
    }
    yield* SubscriptionRef.update(state, current => ({ ...current, drawerOpen: false, workbenchRoute: "terminal" as const,
      repositoryTerminal: { ...current.repositoryTerminal, scope, state: "loading" as const, requestEpoch: epoch, message: null } }))
    let raw: unknown
    try { raw = yield* Effect.promise(() => options.coding!.repositoryTerminal!.terminalSnapshot(scope)) } catch { raw = null }
    const snapshot = decodeMobileTerminalSnapshot(raw, scope)
    yield* SubscriptionRef.update(state, current => {
      if (current.workbenchRoute !== "terminal" || current.repositoryTerminal.requestEpoch !== epoch) return current
      if (snapshot === null) return { ...current, repositoryTerminal: { ...current.repositoryTerminal, state: "failed" as const, message: "The environment returned invalid or unavailable terminal state." } }
      const activeRef = current.repositoryTerminal.activeRef !== null && snapshot.sessions.some(session => session.terminalRef === current.repositoryTerminal.activeRef)
        ? current.repositoryTerminal.activeRef : snapshot.sessions.find(session => session.status === "running")?.terminalRef ?? snapshot.sessions[0]?.terminalRef ?? null
      return { ...current, repositoryTerminal: { ...current.repositoryTerminal, state: "ready" as const, snapshotRef: snapshot.snapshotRef,
        sessions: snapshot.sessions, activeRef, submitting: false, message: snapshot.sessions.some(session => session.gap) ? "Some earlier terminal output is unavailable." : null } }
    })
  })
  const replayRepositoryTerminal = (terminalRef: string) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const scope = before.repositoryTerminal.scope
    const session = before.repositoryTerminal.sessions.find(item => item.terminalRef === terminalRef)
    if (before.workbenchRoute !== "terminal" || scope === null || session === undefined || options.coding?.repositoryTerminal === undefined) return
    const epoch = before.repositoryTerminal.requestEpoch + 1
    const request = { ...scope, terminalRef, sessionVersionRef: session.sessionVersionRef, afterSeq: session.lastSeq, limit: 500 }
    yield* SubscriptionRef.update(state, current => ({ ...current, repositoryTerminal: { ...current.repositoryTerminal, requestEpoch: epoch } }))
    let raw: unknown
    try { raw = yield* Effect.promise(() => options.coding!.repositoryTerminal!.terminalReplay(request)) } catch { raw = null }
    const page = decodeMobileTerminalReplay(raw, request)
    yield* SubscriptionRef.update(state, current => {
      const active = current.repositoryTerminal.sessions.find(item => item.terminalRef === terminalRef)
      if (current.workbenchRoute !== "terminal" || current.repositoryTerminal.requestEpoch !== epoch || active?.sessionVersionRef !== session.sessionVersionRef) return current
      if (page === null) return { ...current, repositoryTerminal: { ...current.repositoryTerminal, message: "Terminal replay was stale or invalid." } }
      return { ...current, repositoryTerminal: { ...current.repositoryTerminal,
        sessions: current.repositoryTerminal.sessions.map(item => item.terminalRef === terminalRef ? applyMobileTerminalReplay(item, page) : item),
        message: page.gap || page.truncated ? "Some earlier terminal output is unavailable." : null } }
    })
  })
  const commandRepositoryTerminal = (
    op: MobileTerminalCommand,
    details: Readonly<{ data?: string; cols?: number; rows?: number }> = {},
  ) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const terminal = before.repositoryTerminal
    const scope = terminal.scope
    const session = terminal.sessions.find(item => item.terminalRef === terminal.activeRef)
    if (before.workbenchRoute !== "terminal" || scope === null || session === undefined || terminal.submitting ||
      options.coding?.repositoryTerminal === undefined || (op === "input" && (details.data === undefined ||
        new TextEncoder().encode(details.data).byteLength > MOBILE_TERMINAL_MAX_INPUT_BYTES))) return
    const epoch = terminal.requestEpoch + 1
    const nonce = `${Date.now().toString(36)}.${session.lastSeq}`
    const request = { ...scope, terminalRef: session.terminalRef, sessionVersionRef: session.sessionVersionRef, op,
      idempotencyRef: `terminal.mobile.${op}.${nonce}`, ...details }
    yield* SubscriptionRef.update(state, current => ({ ...current, repositoryTerminal: { ...current.repositoryTerminal,
      requestEpoch: epoch, submitting: true, message: null } }))
    let raw: unknown
    try { raw = yield* Effect.promise(() => options.coding!.repositoryTerminal!.terminalCommand(request)) } catch { raw = null }
    const receipt = decodeMobileTerminalCommandReceipt(raw, request)
    yield* SubscriptionRef.update(state, current => {
      const active = current.repositoryTerminal.sessions.find(item => item.terminalRef === session.terminalRef)
      if (current.workbenchRoute !== "terminal" || current.repositoryTerminal.requestEpoch !== epoch || active?.sessionVersionRef !== session.sessionVersionRef) return current
      if (receipt === null) return { ...current, repositoryTerminal: { ...current.repositoryTerminal, submitting: false, message: "The terminal command was not acknowledged." } }
      if (op === "close") {
        const sessions = current.repositoryTerminal.sessions.filter(item => item.terminalRef !== session.terminalRef)
        return { ...current, repositoryTerminal: { ...current.repositoryTerminal, sessions, activeRef: sessions[0]?.terminalRef ?? null,
          submitting: false, lastReceipt: receipt } }
      }
      return { ...current, repositoryTerminal: { ...current.repositoryTerminal, submitting: false, lastReceipt: receipt,
        sessions: current.repositoryTerminal.sessions.map(item => item.terminalRef !== session.terminalRef ? item : { ...item,
          sessionVersionRef: receipt.sessionVersionRef, status: receipt.status === "exited" ? "exited" as const : "running" as const,
          ...(op === "resize" && details.cols !== undefined && details.rows !== undefined ? { cols: details.cols, rows: details.rows } : {}),
          ...(op === "restart" ? { tail: "", lastSeq: 0, gap: false, recovered: false } : {}) }) } }
    })
    if (receipt !== null && op !== "close" && op !== "resize") yield* replayRepositoryTerminal(session.terminalRef)
  })
  const mutateComposerAttachment = (
    attachmentId: string,
    action: "remove" | "retry",
  ) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const composer = before.codingComposer
    const mutate = action === "remove"
      ? options.coding?.removeComposerAttachment
      : options.coding?.retryComposerAttachment
    if (composer === null || mutate === undefined || before.khala.pending ||
      before.codingAttachmentMutatingRef !== null ||
      !composer.draft.doc.attachments.some(attachment =>
        attachment.id === attachmentId && (action === "remove" || attachment.status === "error"))) return
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      codingAttachmentMutatingRef: attachmentId,
      codingAttachmentStatus: null,
    }))
    const updated = yield* Effect.promise(async () => {
      try {
        return await mutate(composer, attachmentId)
      } catch {
        return null
      }
    })
    yield* SubscriptionRef.update(state, current => {
      if (current.codingComposer?.draft.draftRef !== composer.draft.draftRef ||
        current.codingAttachmentMutatingRef !== attachmentId) return current
      return updated === null
        ? {
            ...current,
            codingAttachmentMutatingRef: null,
            codingAttachmentStatus: {
              kind: "failed" as const,
              message: action === "remove"
                ? "That attachment could not be removed. The draft was kept."
                : "That attachment could not be reverified. Remove it and attach it again.",
            },
          }
        : {
            ...current,
            codingComposer: updated,
            codingAttachmentMutatingRef: null,
            codingAttachmentStatus: {
              kind: "ready" as const,
              message: action === "remove"
                ? "Attachment removed from this draft."
                : "Attachment reverified on this device.",
            },
          }
    })
  })
  const requestComposerAttachments = options.coding === undefined
    ? () => Effect.void
    : () => Effect.gen(function* () {
        const before = yield* SubscriptionRef.get(state)
        const composer = before.codingComposer
        if (composer === null || before.khala.pending || before.codingAttachmentPicking) return
        yield* SubscriptionRef.update(state, current => ({
          ...current,
          codingAttachmentPicking: true,
          codingAttachmentStatus: null,
        }))
        const result = yield* Effect.promise(async () => {
          try {
            return await options.coding!.pickComposerAttachments(composer)
          } catch {
            return {
              status: "failed" as const,
              error: "The file or image picker is unavailable right now.",
            }
          }
        })
        yield* SubscriptionRef.update(state, current => {
          if (current.codingComposer?.draft.draftRef !== composer.draft.draftRef) return current
          switch (result.status) {
            case "cancelled":
              return { ...current, codingAttachmentPicking: false }
            case "failed":
              return {
                ...current,
                codingAttachmentPicking: false,
                codingAttachmentStatus: { kind: "failed" as const, message: result.error },
              }
            case "updated":
              return {
                ...current,
                codingComposer: result.session,
                codingAttachmentPicking: false,
                codingAttachmentStatus: {
                  kind: "ready" as const,
                  message: `${result.addedCount} ${result.addedCount === 1 ? "attachment" : "attachments"} stored on this device.`,
                },
              }
          }
        })
      })
  const draftChanged = synced?.KhalaDraftChanged ?? khalaHandlers(state, options.khalaTurn).KhalaDraftChanged
  const refreshEnvironmentDirectory = () => Effect.gen(function* () {
    if (options.settings?.environments === undefined) {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        settings: { ...current.settings, environmentState: "unavailable" as const, notice: "Link a verified OpenAgents account to inspect environments." },
      }))
      return
    }
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      settings: { ...current.settings, environmentState: "loading" as const, notice: null },
    }))
    const value = yield* Effect.promise(() => options.settings!.environments!.environmentDirectory().catch(() => null))
    const directory = decodeMobileEnvironmentDirectory(value)
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      settings: directory === null
        ? { ...current.settings, environmentState: "unavailable" as const, environments: null, notice: "Environment health is unavailable or invalid." }
        : { ...current.settings, environmentState: "ready" as const, environments: directory, notice: null },
    }))
  })
  const refreshNotificationSnapshot = () => Effect.gen(function* () {
    if (options.settings?.notifications === undefined) return
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      settings: { ...current.settings, notificationLoading: true },
    }))
    const snapshot = yield* Effect.promise(() => options.settings!.notifications!.snapshot().catch(() => null))
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      settings: snapshot === null
        ? { ...current.settings, notificationLoading: false, notice: "Notification health is unavailable on this installation." }
        : { ...current.settings, notificationLoading: false, notification: snapshot, notice: null },
    }))
  })
  const slashContext = (current: HomeState): MobileSlashCommandContext => ({
    composerAvailable: current.codingComposer !== null,
    targetCatalogAvailable: current.codingExecutionTargets.length > 0,
    attachmentPickerAvailable: options.coding !== undefined && !current.codingAttachmentPicking,
    activeTurnRef: current.khala.runtimeTurn?.runRef ?? null,
    activeTurnCancelable: current.khala.runtimeControlActionsAvailable && (
      current.khala.runtimeTurn?.status === "queued" ||
      current.khala.runtimeTurn?.status === "running" ||
      current.khala.runtimeTurn?.status === "waiting_for_input"
    ),
    pendingAction: current.khala.pending || current.codingAttachmentPicking ||
      current.codingAttachmentMutatingRef !== null ||
      current.khala.runtimeControlSubmittingAction !== null,
  })
  return {
    DrawerToggled: () => SubscriptionRef.update(state, current => current.workspaceLayoutMode === "regular"
      ? {
          ...current,
          workspaceSidebarCollapsed: !current.workspaceSidebarCollapsed,
          workspaceFocusTarget: current.workspaceSidebarCollapsed ? "navigation" as const : "transcript" as const,
        }
      : {
          ...current,
          drawerOpen: !current.drawerOpen,
          workspaceFocusTarget: current.drawerOpen ? "transcript" as const : "navigation" as const,
        }),
    WorkspaceSidebarResized: ({ paneId, size }: { readonly paneId: string; readonly size: number }) =>
      SubscriptionRef.update(state, current =>
        current.workspaceLayoutMode !== "regular" || paneId !== "navigation"
          ? current
          : {
              ...current,
              workspaceSidebarWidth: clampMobileWorkspaceSidebar(size),
              workspaceSidebarCollapsed: false,
              workspaceFocusTarget: "navigation" as const,
            }),
    WorkspaceViewportWidthChanged: (width: number) =>
      SubscriptionRef.update(state, current => {
        const nextMode = mobileWorkspaceLayoutMode(width)
        if (nextMode === current.workspaceLayoutMode) return current
        return {
          ...current,
          workspaceLayoutMode: nextMode,
          drawerOpen: false,
          workspaceSidebarCollapsed: false,
          workspaceFocusTarget: "transcript" as const,
        }
      }),
    WorkspaceKeyboardCommandReceived: (command: MobileWorkspaceKeyboardCommand) => {
      if (command === "new_task") {
        return synced?.NewChatPressed() ?? SubscriptionRef.update(state, current => ({
          ...initialHomeState,
          accessibility: current.accessibility,
          workspaceLayoutMode: current.workspaceLayoutMode,
          workspaceSidebarWidth: current.workspaceSidebarWidth,
        }))
      }
      return SubscriptionRef.update(state, current => {
        if (command === "navigation") return {
          ...current,
          drawerOpen: current.workspaceLayoutMode === "compact" ? true : false,
          workspaceSidebarCollapsed: false,
          workspaceFocusTarget: "navigation" as const,
        }
        if (command === "detail") return {
          ...current,
          drawerOpen: false,
          workspaceFocusTarget: "transcript" as const,
        }
        if (command === "dismiss" && current.workbenchRoute !== "conversation") return {
          ...current,
          drawerOpen: false,
          workbenchRoute: "conversation" as const,
          workspaceFocusTarget: "transcript" as const,
          repositoryBrowser: {
            ...current.repositoryBrowser,
            requestEpoch: current.repositoryBrowser.requestEpoch + 1,
          },
          repositoryReview: {
            ...current.repositoryReview,
            requestEpoch: current.repositoryReview.requestEpoch + 1,
          },
          repositoryGit: {
            ...current.repositoryGit,
            requestEpoch: current.repositoryGit.requestEpoch + 1,
          },
          repositoryTerminal: {
            ...current.repositoryTerminal,
            requestEpoch: current.repositoryTerminal.requestEpoch + 1,
          },
        }
        if (current.threadLifecycle.pendingAction !== null) return current
        return {
          ...current,
          drawerOpen: false,
          workspaceFocusTarget: "transcript" as const,
          threadLifecycle: {
            ...current.threadLifecycle,
            actionThreadRef: null,
            editingThreadRef: null,
            renameDraft: "",
            deleteConfirmThreadRef: null,
            notice: null,
          },
        }
      })
    },
    WorkspaceSearchChanged: (search: string) => SubscriptionRef.update(state, current => ({
      ...current,
      workspaceSearch: search.slice(0, MOBILE_WORKSPACE_MAX_SEARCH),
      threadLifecycle: { ...current.threadLifecycle, actionThreadRef: null },
    })),
    WorkspaceStatusFilterSelected: (status: MobileWorkspaceStatusFilter) =>
      SubscriptionRef.update(state, current => ({
        ...current,
        workspaceStatusFilter: status,
        workspaceProjectFilter: status === "archived" ? null : current.workspaceProjectFilter,
        threadLifecycle: { ...current.threadLifecycle, actionThreadRef: null },
      })),
    WorkspaceProjectFilterSelected: (projectRef: string) =>
      SubscriptionRef.update(state, current => {
        const selected = projectRef === "" ? null : projectRef
        const allowed = selected === null || (
          current.codingDirectory?.authority === "confirmed" &&
          current.codingDirectory.repositories.some(repository => repository.projectRef === selected)
        )
        return allowed
          ? {
              ...current,
              workspaceProjectFilter: selected,
              threadLifecycle: { ...current.threadLifecycle, actionThreadRef: null },
            }
          : current
      }),
    WorkspaceFiltersCleared: () => SubscriptionRef.update(state, current => ({
      ...current,
      workspaceSearch: "",
      workspaceStatusFilter: "all" as const,
      workspaceProjectFilter: null,
      threadLifecycle: { ...current.threadLifecycle, actionThreadRef: null },
    })),
    WorkspaceRowActionsToggled: ({ threadRef }: { readonly threadRef: string }) =>
      SubscriptionRef.update(state, current => {
        const exists = [...current.conversationThreads, ...current.archivedConversationThreads]
          .some(thread => thread.threadRef === threadRef)
        if (!exists || current.threadLifecycle.pendingAction !== null) return current
        return {
          ...current,
          threadLifecycle: {
            ...current.threadLifecycle,
            actionThreadRef: current.threadLifecycle.actionThreadRef === threadRef ? null : threadRef,
            editingThreadRef: null,
            renameDraft: "",
            deleteConfirmThreadRef: null,
            notice: null,
          },
        }
      }),
    WorkspaceLifecycleSheetDismissed: () => SubscriptionRef.update(state, current =>
      current.threadLifecycle.pendingAction !== null
        ? current
        : {
            ...current,
            threadLifecycle: {
              ...current.threadLifecycle,
              actionThreadRef: null,
              editingThreadRef: null,
              renameDraft: "",
              deleteConfirmThreadRef: null,
              notice: null,
            },
            workspaceFocusTarget: "navigation" as const,
          }),
    WorkspaceRowActionSelected: synced === undefined
      ? () => Effect.void
      : (actionId: string) => Effect.gen(function* () {
          const separator = actionId.indexOf(":")
          if (separator <= 0) return
          const action = actionId.slice(0, separator)
          const threadRef = actionId.slice(separator + 1)
          if (threadRef === "" || (action !== "archive" && action !== "restore" && action !== "delete")) return
          const current = yield* SubscriptionRef.get(state)
          if (current.threadLifecycle.pendingAction !== null) return
          const active = current.conversationThreads.some(thread => thread.threadRef === threadRef)
          const archived = current.archivedConversationThreads.some(thread => thread.threadRef === threadRef)
          if (action === "archive" && active) {
            yield* synced.ConversationThreadLifecycleRequested({ action, threadRef })
          } else if (action === "restore" && archived) {
            yield* synced.ConversationThreadLifecycleRequested({ action, threadRef })
          } else if (action === "delete" && (active || archived)) {
            yield* synced.ConversationThreadDeleteRequested({ threadRef })
          }
        }),
    NewChatPressed: synced?.NewChatPressed ??
      (() => SubscriptionRef.update(state, (current) => ({
        ...current,
        drawerOpen: false,
        workspaceFocusTarget: "transcript" as const,
        surfaceMode: "khala" as const,
        workbenchRoute: "conversation" as const,
        repositoryBrowser: initialMobileRepositoryBrowserState,
        repositoryReview: initialMobileRepositoryReviewState,
        repositoryGit: initialMobileRepositoryGitState,
        repositoryTerminal: initialMobileRepositoryTerminalState,
        codingComposer: null,
        codingComposerTargetPickerOpen: false,
        codingComposerTargetSearch: "",
        codingPathDiscovery: { state: "idle" as const },
        codingAttachmentPicking: false,
        codingAttachmentMutatingRef: null,
        codingAttachmentStatus: null,
        khala: initialKhalaState,
      }))),
    CodingComposerAttachmentsRequested: requestComposerAttachments,
    CodingComposerAttachmentRemoved: payload =>
      mutateComposerAttachment(payload.attachmentId, "remove"),
    CodingComposerAttachmentRetryRequested: payload =>
      mutateComposerAttachment(payload.attachmentId, "retry"),
    CodingExecutionTargetSelected: options.coding === undefined
      ? () => Effect.void
      : payload => Effect.gen(function* () {
          const before = yield* SubscriptionRef.get(state)
          const composer = before.codingComposer
          const target = options.coding!.executionTargets?.find(
            option => option.targetId === payload.targetId,
          )
          if (composer === null || target === undefined ||
            target.readiness !== "ready" || before.khala.pending ||
            options.coding!.selectComposerTarget === undefined) return
          const updated = yield* Effect.promise(() =>
            options.coding!.selectComposerTarget!(composer, target))
          if (updated === null) return
          yield* SubscriptionRef.update(state, current =>
            current.codingComposer?.draft.draftRef !== composer.draft.draftRef
              ? current
              : {
                  ...current,
                  codingComposer: updated,
                  codingComposerTargetPickerOpen: false,
                  codingComposerTargetSearch: "",
                })
        }),
    CodingComposerTargetPickerOpened: () =>
      SubscriptionRef.update(state, current =>
        current.codingComposer === null || current.codingExecutionTargets.length === 0
          ? current
          : { ...current, codingComposerTargetPickerOpen: true }),
    CodingComposerTargetPickerDismissed: () =>
      SubscriptionRef.update(state, current => ({
        ...current,
        codingComposerTargetPickerOpen: false,
        codingComposerTargetSearch: "",
      })),
    CodingComposerTargetSearchChanged: (search: string) =>
      SubscriptionRef.update(state, current =>
        !current.codingComposerTargetPickerOpen
          ? current
          : { ...current, codingComposerTargetSearch: search.slice(0, 160) }),
    CodingComposerSlashQueryChanged: (query: string) => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const trigger = mobileComposerSlashTrigger(before.khala.draft)
      if (trigger === null) return
      const bounded = query.replace(/[^A-Za-z0-9_-]/gu, "").slice(0, 64)
      yield* draftChanged(`${before.khala.draft.slice(0, trigger.replaceFrom)}/${bounded}`)
    }),
    CodingComposerSlashCommandSelected: commandId => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const trigger = mobileComposerSlashTrigger(before.khala.draft)
      const selected = mobileSlashCommands(slashContext(before)).find(value => value.id === commandId)
      if (trigger === null || selected?.available !== true) return
      const remainingDraft = before.khala.draft.slice(0, trigger.replaceFrom).trimEnd()
      switch (commandId) {
        case "mobile.command.choose_target":
          yield* draftChanged(remainingDraft)
          yield* SubscriptionRef.update(state, current =>
            current.codingComposer === null || current.codingExecutionTargets.length === 0
              ? current
              : { ...current, codingComposerTargetPickerOpen: true })
          return
        case "mobile.command.attach":
          yield* draftChanged(remainingDraft)
          yield* requestComposerAttachments()
          return
        case "mobile.command.stop_turn": {
          const runRef = before.khala.runtimeTurn?.runRef
          if (runRef === undefined || synced === undefined) return
          yield* draftChanged(remainingDraft)
          yield* synced.RuntimeTurnControlRequested({ action: "cancel", runRef })
          return
        }
        case "mobile.command.new_chat":
          if (synced !== undefined) {
            yield* synced.NewChatPressed()
          } else {
            yield* SubscriptionRef.set(state, {
              ...initialHomeState,
              accessibility: before.accessibility,
            })
          }
      }
    }),
    CodingComposerPathQueryChanged: (query: string) => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const trigger = mobileComposerPathTrigger(before.khala.draft)
      if (trigger === null) return
      const bounded = query.replace(/[\s@\\]/gu, "").slice(0, 128)
      yield* draftChanged(`${before.khala.draft.slice(0, trigger.replaceFrom)}@${bounded}`)
    }),
    CodingComposerPathSelected: (pathRef: string) => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const trigger = mobileComposerPathTrigger(before.khala.draft)
      const selected = before.codingPathDiscovery.state === "ready"
        ? before.codingPathDiscovery.entries.find(entry => entry.pathRef === pathRef)
        : undefined
      if (trigger === null || selected === undefined) return
      yield* draftChanged(`${before.khala.draft.slice(0, trigger.replaceFrom)}@${selected.pathRef} `)
    }),
    FilesRouteOpened: loadRepositoryRoot,
    FilesRouteClosed: () => SubscriptionRef.update(state, current => ({
      ...current,
      workbenchRoute: "conversation" as const,
      workspaceFocusTarget: "transcript" as const,
      repositoryBrowser: {
        ...current.repositoryBrowser,
        requestEpoch: current.repositoryBrowser.requestEpoch + 1,
      },
    })),
    RepositoryFilesRefreshed: loadRepositoryRoot,
    RepositoryDirectoryToggled: payload => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const browser = before.repositoryBrowser
      const scope = browser.scope
      const entry = Object.values(browser.pages).flatMap(page => page.entries).find(candidate =>
        candidate.kind === "directory" && candidate.pathRef === payload.pathRef &&
        candidate.revisionRef === payload.revisionRef)
      if (before.workbenchRoute !== "files" || browser.state !== "ready" || scope === null || entry === undefined) return
      if (browser.expandedRefs.includes(entry.pathRef)) {
        yield* SubscriptionRef.update(state, current => ({
          ...current,
          repositoryBrowser: {
            ...current.repositoryBrowser,
            expandedRefs: current.repositoryBrowser.expandedRefs.filter(ref => ref !== entry.pathRef),
          },
        }))
        return
      }
      if (browser.pages[entry.pathRef] !== undefined) {
        yield* SubscriptionRef.update(state, current => ({
          ...current,
          repositoryBrowser: {
            ...current.repositoryBrowser,
            expandedRefs: [...current.repositoryBrowser.expandedRefs, entry.pathRef],
          },
        }))
        return
      }
      if (options.coding?.repositoryFiles === undefined) return
      const epoch = browser.requestEpoch + 1
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        repositoryBrowser: { ...current.repositoryBrowser, requestEpoch: epoch, message: null },
      }))
      const result = yield* Effect.promise(() => loadMobileRepositoryTree(
        options.coding!.repositoryFiles!,
        { ...scope, directoryRef: entry.pathRef, cursor: null },
      ))
      yield* SubscriptionRef.update(state, current => {
        const active = current.repositoryBrowser
        if (current.workbenchRoute !== "files" || active.requestEpoch !== epoch ||
          active.scope?.sessionRef !== scope.sessionRef) return current
        return result.state === "ready"
          ? {
              ...current,
              repositoryBrowser: {
                ...active,
                pages: { ...active.pages, [entry.pathRef]: result.page },
                expandedRefs: [...active.expandedRefs, entry.pathRef],
                message: null,
              },
            }
          : {
              ...current,
              repositoryBrowser: { ...active, message: result.message },
            }
      })
    }),
    RepositoryFileSelected: payload => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const browser = before.repositoryBrowser
      const scope = browser.scope
      const entry = Object.values(browser.pages).flatMap(page => page.entries).find(candidate =>
        candidate.kind === "file" && candidate.pathRef === payload.pathRef &&
        candidate.revisionRef === payload.revisionRef)
      if (before.workbenchRoute !== "files" || scope === null || entry === undefined ||
        options.coding?.repositoryFiles === undefined) return
      const epoch = browser.requestEpoch + 1
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        repositoryBrowser: {
          ...current.repositoryBrowser,
          requestEpoch: epoch,
          preview: { state: "loading" as const, pathRef: entry.pathRef, revisionRef: entry.revisionRef },
        },
      }))
      const result = yield* Effect.promise(() => loadMobileRepositoryPreview(
        options.coding!.repositoryFiles!,
        { ...scope, pathRef: entry.pathRef, expectedRevisionRef: entry.revisionRef },
      ))
      yield* SubscriptionRef.update(state, current => {
        const active = current.repositoryBrowser
        if (current.workbenchRoute !== "files" || active.requestEpoch !== epoch ||
          active.scope?.sessionRef !== scope.sessionRef) return current
        return {
          ...current,
          repositoryBrowser: {
            ...active,
            preview: result.state === "ready"
              ? { state: "ready" as const, preview: result.preview }
              : { state: "failed" as const, pathRef: entry.pathRef, message: result.message },
          },
        }
      })
    }),
    ChangesRouteOpened: loadRepositoryChanges,
    RepositoryChangesRefreshed: loadRepositoryChanges,
    WorkbenchConversationOpened: () => SubscriptionRef.update(state, current => ({
      ...current,
      workbenchRoute: "conversation" as const,
      repositoryReview: { ...current.repositoryReview, requestEpoch: current.repositoryReview.requestEpoch + 1 },
      repositoryGit: { ...current.repositoryGit, requestEpoch: current.repositoryGit.requestEpoch + 1 },
      repositoryTerminal: { ...current.repositoryTerminal, requestEpoch: current.repositoryTerminal.requestEpoch + 1 },
      workspaceFocusTarget: "transcript" as const,
    })),
    RepositoryChangedFileSelected: payload => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const review = before.repositoryReview
      const scope = review.scope
      const file = review.summary?.files.find(item => item.pathRef === payload.pathRef &&
        item.source === payload.source && item.revisionRef === payload.revisionRef)
      if (before.workbenchRoute !== "changes" || scope === null || file === undefined ||
        file.source === "untracked" || file.binary || file.status === "unmerged" ||
        options.coding?.repositoryReview === undefined || review.summary === null) return
      const epoch = review.requestEpoch + 1
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        repositoryReview: { ...current.repositoryReview, requestEpoch: epoch, diff: null, selectedRowRef: null, commentDraft: "", message: null },
      }))
      const request = { ...scope, statusRef: review.summary.statusRef, pathRef: file.pathRef, source: file.source, expectedRevisionRef: file.revisionRef }
      let raw: unknown
      try { raw = yield* Effect.promise(() => options.coding!.repositoryReview!.diff(request)) } catch { raw = null }
      const diff = decodeMobileFileDiff(raw, request)
      yield* SubscriptionRef.update(state, current =>
        current.workbenchRoute !== "changes" || current.repositoryReview.requestEpoch !== epoch
          ? current
          : diff === null
            ? { ...current, repositoryReview: { ...current.repositoryReview, message: "That diff is stale, unsafe, or unavailable." } }
            : { ...current, repositoryReview: { ...current.repositoryReview, diff, message: null } })
    }),
    RepositoryReviewRowSelected: payload => SubscriptionRef.update(state, current => {
      const row = current.repositoryReview.diff?.hunks.flatMap(hunk => hunk.rows).find(item => item.rowRef === payload.rowId)
      return current.workbenchRoute !== "changes" || row === undefined
        ? current
        : { ...current, repositoryReview: { ...current.repositoryReview, selectedRowRef: row.rowRef, commentDraft: "", message: null } }
    }),
    RepositoryReviewCommentChanged: (comment: string) => SubscriptionRef.update(state, current =>
      current.repositoryReview.selectedRowRef === null || current.repositoryReview.submitting
        ? current
        : { ...current, repositoryReview: { ...current.repositoryReview, commentDraft: comment.slice(0, MOBILE_REVIEW_COMMENT_MAX) } }),
    RepositoryReviewSubmitted: () => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const review = before.repositoryReview
      const scope = review.scope
      const diff = review.diff
      const rowRef = review.selectedRowRef
      const comment = review.commentDraft.trim()
      if (before.workbenchRoute !== "changes" || scope === null || diff === null || rowRef === null || comment === "" ||
        review.submitting || options.coding?.repositoryReview === undefined ||
        !diff.hunks.some(hunk => hunk.rows.some(row => row.rowRef === rowRef))) return
      const epoch = review.requestEpoch + 1
      const idempotencyRef = `review.mobile.${Date.now().toString(36)}.${review.receipts.length}`
      const request = { ...scope, statusRef: diff.statusRef, pathRef: diff.pathRef, rowRef, expectedRevisionRef: diff.revisionRef, comment, idempotencyRef }
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        repositoryReview: { ...current.repositoryReview, requestEpoch: epoch, submitting: true, message: null },
      }))
      let raw: unknown
      try { raw = yield* Effect.promise(() => options.coding!.repositoryReview!.submitReview(request)) } catch { raw = null }
      const receipt = decodeMobileReviewReceipt(raw, request)
      yield* SubscriptionRef.update(state, current =>
        current.workbenchRoute !== "changes" || current.repositoryReview.requestEpoch !== epoch ||
          current.repositoryReview.scope?.sessionRef !== scope.sessionRef || current.repositoryReview.diff?.revisionRef !== diff.revisionRef ||
          current.repositoryReview.selectedRowRef !== rowRef
          ? current
          : receipt === null
            ? { ...current, repositoryReview: { ...current.repositoryReview, submitting: false, message: "The review instruction was not recorded." } }
            : { ...current, repositoryReview: { ...current.repositoryReview, submitting: false, selectedRowRef: null, commentDraft: "", receipts: [...current.repositoryReview.receipts, receipt], message: null } })
    }),
    GitRouteOpened: loadRepositoryGit,
    RepositoryGitRefreshed: loadRepositoryGit,
    RepositoryGitBranchSelected: payload => SubscriptionRef.update(state, current => {
      const git = current.repositoryGit
      const status = git.status
      const branch = status?.branches.find(item => item.branchRef === payload.branchRef && item.name === payload.name)
      if (current.workbenchRoute !== "git" || git.scope === null || status === null || branch === undefined || branch.current || git.submitting) return current
      const nonce = `${Date.now().toString(36)}.${git.receipts.length}`
      return { ...current, repositoryGit: { ...git, pendingConfirmation: {
        ...git.scope, op: "checkout" as const, statusRef: status.statusRef, expectedHeadRef: status.headRef,
        branchRef: branch.branchRef, branchName: branch.name, idempotencyRef: `git.mobile.checkout.${nonce}`,
        confirmationRef: `confirmation.mobile.checkout.${nonce}`,
      }, failureCode: null, message: null } }
    }),
    RepositoryGitFileToggled: payload => SubscriptionRef.update(state, current => {
      const git = current.repositoryGit
      const file = git.status?.files.find(item => item.pathRef === payload.pathRef && item.status !== "unmerged")
      if (current.workbenchRoute !== "git" || file === undefined || git.submitting) return current
      return { ...current, repositoryGit: { ...git, selectedPaths: git.selectedPaths.includes(file.pathRef)
        ? git.selectedPaths.filter(pathRef => pathRef !== file.pathRef)
        : [...git.selectedPaths, file.pathRef] } }
    }),
    RepositoryGitCommitMessageChanged: (message: string) => SubscriptionRef.update(state, current =>
      current.workbenchRoute !== "git" || current.repositoryGit.submitting
        ? current
        : { ...current, repositoryGit: { ...current.repositoryGit, commitMessage: message.slice(0, MOBILE_GIT_COMMIT_MESSAGE_MAX) } }),
    RepositoryGitCommitRequested: () => SubscriptionRef.update(state, current => {
      const git = current.repositoryGit
      const status = git.status
      const paths = status?.files.filter(file => git.selectedPaths.includes(file.pathRef) && file.status !== "unmerged").map(file => file.pathRef) ?? []
      const message = git.commitMessage.trim()
      if (current.workbenchRoute !== "git" || git.scope === null || status === null || status.detached ||
        git.submitting || paths.length === 0 || message === "") return current
      const nonce = `${Date.now().toString(36)}.${git.receipts.length}`
      return { ...current, repositoryGit: { ...git, pendingConfirmation: {
        ...git.scope, op: "commit" as const, statusRef: status.statusRef, expectedHeadRef: status.headRef,
        paths, message, idempotencyRef: `git.mobile.commit.${nonce}`, confirmationRef: `confirmation.mobile.commit.${nonce}`,
      }, failureCode: null, message: null } }
    }),
    RepositoryGitPushRequested: () => SubscriptionRef.update(state, current => {
      const git = current.repositoryGit
      const status = git.status
      if (current.workbenchRoute !== "git" || git.scope === null || status === null || status.detached ||
        status.branch === null || status.upstream === null || status.ahead === 0 || git.submitting) return current
      const nonce = `${Date.now().toString(36)}.${git.receipts.length}`
      return { ...current, repositoryGit: { ...git, pendingConfirmation: {
        ...git.scope, op: "push" as const, statusRef: status.statusRef, expectedHeadRef: status.headRef,
        branchName: status.branch, idempotencyRef: `git.mobile.push.${nonce}`, confirmationRef: `confirmation.mobile.push.${nonce}`,
      }, failureCode: null, message: null } }
    }),
    RepositoryGitConfirmationCancelled: () => SubscriptionRef.update(state, current =>
      current.repositoryGit.submitting ? current : { ...current, repositoryGit: { ...current.repositoryGit, pendingConfirmation: null } }),
    RepositoryGitConfirmationAccepted: () => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const request = before.repositoryGit.pendingConfirmation
      if (request === null) return
      yield* runRepositoryGitMutation(request)
    }),
    TerminalRouteOpened: loadRepositoryTerminals,
    RepositoryTerminalRefreshed: loadRepositoryTerminals,
    RepositoryTerminalForegrounded: () => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      if (before.workbenchRoute === "terminal") yield* loadRepositoryTerminals()
    }),
    RepositoryTerminalCreateRequested: () => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const scope = before.repositoryTerminal.scope
      if (before.workbenchRoute !== "terminal" || scope === null || before.repositoryTerminal.submitting ||
        before.repositoryTerminal.sessions.length >= 12 || options.coding?.repositoryTerminal === undefined) return
      const epoch = before.repositoryTerminal.requestEpoch + 1
      const request = { ...scope, cols: 80, rows: 24, idempotencyRef: `terminal.mobile.create.${Date.now().toString(36)}.${before.repositoryTerminal.sessions.length}` }
      yield* SubscriptionRef.update(state, current => ({ ...current, repositoryTerminal: { ...current.repositoryTerminal, requestEpoch: epoch, submitting: true, message: null } }))
      let raw: unknown
      try { raw = yield* Effect.promise(() => options.coding!.repositoryTerminal!.terminalCreate(request)) } catch { raw = null }
      const snapshot = decodeMobileTerminalSnapshot(raw, scope)
      yield* SubscriptionRef.update(state, current => {
        if (current.workbenchRoute !== "terminal" || current.repositoryTerminal.requestEpoch !== epoch) return current
        if (snapshot === null) return { ...current, repositoryTerminal: { ...current.repositoryTerminal, submitting: false, message: "The terminal session was not created." } }
        const previous = new Set(current.repositoryTerminal.sessions.map(session => session.terminalRef))
        const created = snapshot.sessions.find(session => !previous.has(session.terminalRef)) ?? snapshot.sessions.at(-1)
        return { ...current, repositoryTerminal: { ...current.repositoryTerminal, state: "ready" as const, snapshotRef: snapshot.snapshotRef,
          sessions: snapshot.sessions, activeRef: created?.terminalRef ?? current.repositoryTerminal.activeRef, submitting: false, message: null } }
      })
    }),
    RepositoryTerminalSelected: payload => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      if (before.workbenchRoute !== "terminal" || !before.repositoryTerminal.sessions.some(session => session.terminalRef === payload.terminalRef)) return
      yield* SubscriptionRef.update(state, current => ({ ...current, repositoryTerminal: { ...current.repositoryTerminal, activeRef: payload.terminalRef } }))
      yield* replayRepositoryTerminal(payload.terminalRef)
    }),
    RepositoryTerminalHostEvent: event => event.type === "data"
      ? commandRepositoryTerminal("input", { data: event.data })
      : event.cols < 1 || event.cols > 1_000 || event.rows < 1 || event.rows > 1_000
        ? Effect.void
        : commandRepositoryTerminal("resize", { cols: Math.floor(event.cols), rows: Math.floor(event.rows) }),
    RepositoryTerminalAccessoryKeyPressed: payload => commandRepositoryTerminal("input", { data: payload.data }),
    RepositoryTerminalInterruptRequested: () => commandRepositoryTerminal("interrupt"),
    RepositoryTerminalRestartRequested: () => commandRepositoryTerminal("restart"),
    RepositoryTerminalCloseRequested: () => commandRepositoryTerminal("close"),
    RuntimeTurnStopConfirmationRequested: payload =>
      SubscriptionRef.update(state, current => {
        const turn = current.khala.runtimeTurn
        const active = turn?.status === "queued" || turn?.status === "running" ||
          turn?.status === "waiting_for_input"
        if (!active || turn?.runRef !== payload.runRef ||
          !current.khala.runtimeControlActionsAvailable ||
          current.khala.runtimeControlSubmittingAction !== null) return current
        return {
          ...current,
          khala: { ...current.khala, runtimeStopConfirmationRunRef: payload.runRef },
        }
      }),
    RuntimeTurnStopConfirmationDismissed: payload =>
      SubscriptionRef.update(state, current =>
        current.khala.runtimeStopConfirmationRunRef !== payload.runRef
          ? current
          : {
              ...current,
              khala: { ...current.khala, runtimeStopConfirmationRunRef: null },
            }),
    RuntimeTurnStopConfirmed: payload => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const turn = before.khala.runtimeTurn
      const active = turn?.status === "queued" || turn?.status === "running" ||
        turn?.status === "waiting_for_input"
      if (!active || turn?.runRef !== payload.runRef ||
        before.khala.runtimeStopConfirmationRunRef !== payload.runRef ||
        !before.khala.runtimeControlActionsAvailable || synced === undefined) return
      yield* synced.RuntimeTurnControlRequested({ action: "cancel", runRef: payload.runRef })
    }),
    SettingsPressed: () => Effect.gen(function* () {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        drawerOpen: false,
        workspaceFocusTarget: "transcript" as const,
        workbenchRoute: "settings" as const,
        settings: { ...current.settings, section: "root" as const, incomingShare: options.settings?.incomingShare ?? current.settings.incomingShare },
      }))
      yield* Effect.all([refreshEnvironmentDirectory(), refreshNotificationSnapshot()], { concurrency: "unbounded" })
    }),
    SettingsSectionSelected: ({ section }: { readonly section: MobileSettingsSection }) =>
      SubscriptionRef.update(state, current => ({
        ...current,
        settings: { ...current.settings, section, notice: null },
      })),
    EnvironmentDirectoryRequested: refreshEnvironmentDirectory,
    EnvironmentPairingCodeChanged: (value: string) => SubscriptionRef.update(state, current => ({
      ...current,
      settings: { ...current.settings, pairingCode: normalizeMobilePairingCode(value), environmentReceipt: null, notice: null },
    })),
    EnvironmentPairRequested: () => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const pairingCode = before.settings.pairingCode.trim()
      if (options.settings?.environments === undefined || pairingCode.length === 0 || before.settings.submittingEnvironment) return
      yield* SubscriptionRef.update(state, current => ({ ...current, settings: { ...current.settings, submittingEnvironment: true, notice: null } }))
      const value = yield* Effect.promise(() => options.settings!.environments!.pairEnvironment({ pairingCode, idempotencyRef: `mobile.pair.${Date.now()}` }).catch(() => null))
      const receipt = decodeMobileEnvironmentReceipt(value, "pair")
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        settings: receipt === null
          ? { ...current.settings, submittingEnvironment: false, notice: "Pairing was not confirmed. Check the code and try again." }
          : { ...current.settings, submittingEnvironment: false, pairingCode: "", environments: receipt.directory, environmentState: "ready" as const, selectedEnvironmentRef: receipt.environmentRef, environmentReceipt: receipt, notice: null },
      }))
    }),
    EnvironmentInspected: ({ environmentRef }: { readonly environmentRef: string }) => SubscriptionRef.update(state, current => ({
      ...current,
      settings: { ...current.settings, selectedEnvironmentRef: current.settings.environments?.environments.some(item => item.environmentRef === environmentRef) ? environmentRef : null },
    })),
    EnvironmentReconnectRequested: ({ environmentRef }: { readonly environmentRef: string }) => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      if (options.settings?.environments === undefined || before.settings.environments === null || before.settings.submittingEnvironment ||
        !before.settings.environments.environments.some(item => item.environmentRef === environmentRef)) return
      yield* SubscriptionRef.update(state, current => ({ ...current, settings: { ...current.settings, submittingEnvironment: true, notice: null } }))
      const value = yield* Effect.promise(() => options.settings!.environments!.reconnectEnvironment({
        environmentRef,
        directoryRef: before.settings.environments!.directoryRef,
        idempotencyRef: `mobile.reconnect.${Date.now()}`,
      }).catch(() => null))
      const receipt = decodeMobileEnvironmentReceipt(value, "reconnect")
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        settings: receipt === null
          ? { ...current.settings, submittingEnvironment: false, notice: "Reconnect was not confirmed." }
          : { ...current.settings, submittingEnvironment: false, environments: receipt.directory, environmentReceipt: receipt, selectedEnvironmentRef: receipt.environmentRef, notice: null },
      }))
    }),
    NotificationPermissionRequested: () => Effect.gen(function* () {
      if (options.settings?.notifications === undefined) return
      yield* SubscriptionRef.update(state, current => ({ ...current, settings: { ...current.settings, notificationLoading: true, notice: null } }))
      const snapshot = yield* Effect.promise(() => options.settings!.notifications!.requestPermission().catch(() => null))
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        settings: snapshot === null
          ? { ...current.settings, notificationLoading: false, notice: "Notification permission could not be updated." }
          : { ...current.settings, notificationLoading: false, notification: snapshot, notice: null },
      }))
    }),
    NotificationPreferenceToggled: ({ preference }: { readonly preference: "attention" | "completion" | "approvals" }) => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      if (options.settings?.notifications === undefined || before.settings.notificationLoading) return
      const preferences = { ...before.settings.notification.preferences, [preference]: !before.settings.notification.preferences[preference] }
      yield* SubscriptionRef.update(state, current => ({ ...current, settings: { ...current.settings, notificationLoading: true, notice: null } }))
      const snapshot = yield* Effect.promise(() => options.settings!.notifications!.setPreferences(preferences).catch(() => null))
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        settings: snapshot === null
          ? { ...current.settings, notificationLoading: false, notice: "Notification preferences could not be saved." }
          : { ...current.settings, notificationLoading: false, notification: snapshot, notice: null },
      }))
    }),
    IncomingShareInserted: () => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      if (before.settings.incomingShare === null) return
      const text = mobileShareComposerText(before.settings.incomingShare)
      yield* draftChanged(text)
      options.settings?.onShareConsumed?.()
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        workbenchRoute: "conversation" as const,
        surfaceMode: "khala" as const,
        settings: { ...current.settings, incomingShare: null, section: "root" as const, notice: null },
      }))
    }),
    IncomingShareDismissed: () => Effect.gen(function* () {
      options.settings?.onShareConsumed?.()
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        settings: { ...current.settings, incomingShare: null, section: "root" as const, notice: null },
      }))
    }),
    OpenAgentsSignInPressed: () => options.sessionActions === undefined
      ? Effect.void
      : Effect.promise(options.sessionActions.signIn),
    OpenAgentsSignOutPressed: () => options.sessionActions === undefined
      ? Effect.promise(selectedThreadLease.clear)
      : Effect.gen(function* () {
          yield* Effect.promise(selectedThreadLease.clear)
          yield* Effect.promise(options.sessionActions!.signOut)
        }),
    SurfaceModeSelected: (payload) => SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      workspaceFocusTarget: "transcript" as const,
      workbenchRoute: "conversation" as const,
      surfaceMode: payload.mode as SurfaceMode,
    })),
    ControllerDestinationSelected: (destination) => SubscriptionRef.update(state, current => ({
      ...current,
      controllerDestination: destination,
    })),
    ControllerSessionInspected: ({ sessionRef }) => SubscriptionRef.update(state, current => {
      if (current.codingDirectory === null) return current
      const directory = projectMobileControllerDirectory(current.codingDirectory)
      return directory.authority === "confirmed" &&
          directory.recent.some(session => session.sessionRef === sessionRef)
        ? {
            ...current,
            inspectedControllerSessionRef: sessionRef,
            selectedPortableDestinationRef: null,
            portableNotice: null,
          }
        : {
            ...current,
            inspectedControllerSessionRef: null,
            selectedPortableDestinationRef: null,
            portableNotice: null,
          }
    }),
    ControllerAttentionSelected: options.conversation === undefined || options.coding === undefined
      ? () => Effect.void
      : (target: MobileAttentionTarget) => Effect.gen(function* () {
          const before = yield* SubscriptionRef.get(state)
          if (before.khala.pending || before.attentionSnapshot === null) return
          const resolution = resolveMobileAttentionTarget(before.attentionSnapshot, {
            source: "in_app",
            target,
          })
          if (resolution.state !== "ready") {
            yield* SubscriptionRef.update(state, current => ({
              ...current,
              attentionNotice: "That request is no longer actionable.",
            }))
            return
          }
          if (options.coding !== undefined) yield* Effect.promise(options.coding.clearSelection)
          yield* SubscriptionRef.update(state, current => ({
            ...current,
            controllerDestination: "attention" as const,
            drawerOpen: false,
            workspaceFocusTarget: "transcript" as const,
            codingComposer: null,
            codingComposerTargetPickerOpen: false,
            codingComposerTargetSearch: "",
            codingPathDiscovery: { state: "idle" as const },
            codingAttachmentPicking: false,
            codingAttachmentMutatingRef: null,
            codingAttachmentStatus: null,
            attentionNotice: null,
            khala: { ...current.khala, pending: true },
          }))
          const thread = yield* Effect.promise(() => selectedThreadLease.activate(
            resolution.target.threadRef,
            update => {
              Effect.runFork(SubscriptionRef.update(state, current =>
                current.activeThreadRef === update.threadRef
                  ? withConfirmedThread(current, update)
                  : current))
            },
          ))
          yield* SubscriptionRef.update(state, current => thread === null
            ? {
                ...failedConversationState(current, "The confirmed attention target is still reconciling."),
                attentionNotice: "The confirmed attention target is still reconciling.",
              }
            : withConfirmedThread(current, thread))
        }),
    PortableDestinationSelected: targetRef => SubscriptionRef.update(state, current => {
      if (current.inspectedControllerSessionRef === null || current.portableSnapshot === null) return current
      const control = projectMobilePortableSessionControl(
        current.portableSnapshot,
        current.inspectedControllerSessionRef,
      )
      return control.state === "ready" && control.targets.some(target =>
        target.targetRef === targetRef && target.health === "ready" &&
        target.targetRef !== control.sourceTarget.targetRef)
        ? { ...current, selectedPortableDestinationRef: targetRef, portableNotice: null }
        : current
    }),
    PortableControlRequested: options.coding?.requestPortableAction === undefined
      ? () => Effect.void
      : ({ action }) => Effect.gen(function* () {
          const before = yield* SubscriptionRef.get(state)
          if (before.inspectedControllerSessionRef === null ||
              before.portableSubmittingAction !== null) return
          if (before.portableSnapshot === null) {
            yield* SubscriptionRef.update(state, current => ({
              ...current,
              portableNotice: { kind: "rejected" as const, message: portableUnavailableCopy("authority_unavailable") },
            }))
            return
          }
          const control = projectMobilePortableSessionControl(
            before.portableSnapshot,
            before.inspectedControllerSessionRef,
          )
          const destinationRequired = action === "move" || action === "failback"
          const availability = control.state === "ready" ? control.actions[action] : null
          const destinationAllowed = !destinationRequired || (
            before.selectedPortableDestinationRef !== null &&
            availability?.destinations.some(target =>
              target.targetRef === before.selectedPortableDestinationRef) === true
          )
          const rejection = control.state === "unavailable"
            ? control.reason
            : !availability!.available
              ? availability!.reason ?? "invalid_invocation"
              : !destinationAllowed
                ? "destination_required"
                : null
          if (rejection !== null) {
            yield* SubscriptionRef.update(state, current => ({
              ...current,
              portableNotice: { kind: "rejected" as const, message: portableUnavailableCopy(rejection) },
            }))
            return
          }
          yield* SubscriptionRef.update(state, current => ({
            ...current,
            portableSubmittingAction: action,
            portableNotice: null,
          }))
          const result = yield* Effect.promise(() => options.coding!.requestPortableAction!({
            sessionRef: before.inspectedControllerSessionRef!,
            action,
            ...(destinationRequired && before.selectedPortableDestinationRef !== null
              ? { destinationTargetRef: before.selectedPortableDestinationRef }
              : {}),
          }))
          yield* SubscriptionRef.update(state, current => ({
            ...current,
            portableSnapshot: result.snapshot ?? current.portableSnapshot,
            portableSubmittingAction: null,
            portableNotice: result.state === "queued"
              ? { kind: "queued" as const, message: `${action} queued for confirmed reconciliation.` }
              : { kind: "rejected" as const, message: portableUnavailableCopy(result.reason) },
          }))
        }),
    [AgentStackToggled]: () => SubscriptionRef.update(state, current => ({
      ...current,
      khala: {
        ...current.khala,
        agentGraphExpanded: !current.khala.agentGraphExpanded,
      },
    })),
    [AgentRowSelected]: (payload: Readonly<{ agentRef: string }>) =>
      SubscriptionRef.update(state, current => ({
        ...current,
        khala: {
          ...current.khala,
          // A second tap on the inspected row closes its inspector; any other
          // tap resolves through the shared deterministic selection fallback.
          selectedAgentRef: current.khala.agentGraph === null ||
              current.khala.selectedAgentRef === payload.agentRef
            ? null
            : resolveLiveAgentGraphSelection(current.khala.agentGraph, payload.agentRef),
        },
      })),
    [WorkGroupToggled]: (payload: Readonly<{ groupRef: string }>) =>
      SubscriptionRef.update(state, current => {
        const exists = current.khala.entries.some(entry => entry.work?.groupRef === payload.groupRef)
        return !exists ? current : {
          ...current,
          khala: {
            ...current.khala,
            expandedWorkGroups: {
              ...current.khala.expandedWorkGroups,
              [payload.groupRef]: current.khala.expandedWorkGroups[payload.groupRef] !== true,
            },
          },
        }
      }),
    [WorkItemToggled]: (payload: Readonly<{ itemRef: string }>) =>
      SubscriptionRef.update(state, current => {
        const exists = current.khala.entries.some(entry =>
          entry.work?.items.some(item => item.itemRef === payload.itemRef) === true)
        return !exists ? current : {
          ...current,
          khala: {
            ...current.khala,
            expandedWorkItems: {
              ...current.khala.expandedWorkItems,
              [payload.itemRef]: current.khala.expandedWorkItems[payload.itemRef] !== true,
            },
          },
        }
      }),
    [TranscriptPinnedChanged]: (pinned: boolean) =>
      SubscriptionRef.update(state, current => ({
        ...current,
        khala: {
          ...current.khala,
          transcriptPinned: pinned,
          transcriptUnreadCount: pinned ? 0 : current.khala.transcriptUnreadCount,
          transcriptScrollToKey: null,
        },
      })),
    [TranscriptEarlierHistoryRequested]: () =>
      SubscriptionRef.update(state, current => ({
        ...current,
        khala: {
          ...current.khala,
          transcriptVisibleCount: nextMobileTranscriptVisibleCount(
            current.khala.transcriptVisibleCount,
            current.khala.entries.length,
          ),
        },
      })),
    [TranscriptJumpToLatestRequested]: () =>
      SubscriptionRef.update(state, current => {
        const latest = current.khala.entries.at(-1)
        return latest === undefined ? current : {
          ...current,
          khala: {
            ...current.khala,
            transcriptPinned: true,
            transcriptUnreadCount: 0,
            transcriptScrollToKey: latest.key,
          },
        }
      }),
    [TranscriptAttachmentOpened]: (payload: Readonly<{ attachmentRef: string }>) =>
      SubscriptionRef.update(state, current => {
        const exists = current.khala.entries.some(entry =>
          (entry.attachments ?? []).some((_, index) =>
            mobileAttachmentRef(entry.key, index) === payload.attachmentRef))
        return !exists || current.khala.attachmentPreviewStates[payload.attachmentRef] !== "ready"
          ? current
          : {
              ...current,
              khala: { ...current.khala, viewingAttachmentRef: payload.attachmentRef },
            }
      }),
    [TranscriptAttachmentLoadSettled]: (payload: Readonly<{
      attachmentRef: string
      outcome: "ready" | "failed"
    }>) => SubscriptionRef.update(state, current => {
      const exists = current.khala.entries.some(entry =>
        (entry.attachments ?? []).some((_, index) =>
          mobileAttachmentRef(entry.key, index) === payload.attachmentRef))
      return !exists ? current : {
        ...current,
        khala: {
          ...current.khala,
          attachmentPreviewStates: {
            ...current.khala.attachmentPreviewStates,
            [payload.attachmentRef]: payload.outcome,
          },
          viewingAttachmentRef: payload.outcome === "failed" &&
              current.khala.viewingAttachmentRef === payload.attachmentRef
            ? null
            : current.khala.viewingAttachmentRef,
        },
      }
    }),
    [TranscriptAttachmentRetryRequested]: (payload: Readonly<{ attachmentRef: string }>) =>
      SubscriptionRef.update(state, current => {
        if (current.khala.attachmentPreviewStates[payload.attachmentRef] !== "failed") return current
        const exists = current.khala.entries.some(entry =>
          (entry.attachments ?? []).some((_, index) =>
            mobileAttachmentRef(entry.key, index) === payload.attachmentRef))
        return !exists ? current : {
          ...current,
          khala: {
            ...current.khala,
            attachmentPreviewStates: {
              ...current.khala.attachmentPreviewStates,
              [payload.attachmentRef]: "loading" as const,
            },
            attachmentRetryEpochs: {
              ...current.khala.attachmentRetryEpochs,
              [payload.attachmentRef]: (current.khala.attachmentRetryEpochs[payload.attachmentRef] ?? 0) + 1,
            },
          },
        }
      }),
    [TranscriptAttachmentViewerDismissed]: (payload: Readonly<{ attachmentRef: string }>) =>
      SubscriptionRef.update(state, current =>
        current.khala.viewingAttachmentRef !== payload.attachmentRef
          ? current
          : {
              ...current,
              khala: { ...current.khala, viewingAttachmentRef: null },
            }),
    ConversationThreadSelected: synced?.ConversationThreadSelected ?? (() => Effect.void),
    ConversationThreadRenameStarted: synced?.ConversationThreadRenameStarted ?? (() => Effect.void),
    ConversationThreadRenameChanged: synced?.ConversationThreadRenameChanged ?? (() => Effect.void),
    ConversationThreadRenameSubmitted: synced?.ConversationThreadRenameSubmitted ?? (() => Effect.void),
    ConversationThreadRenameCancelled: synced?.ConversationThreadRenameCancelled ?? (() => Effect.void),
    ConversationThreadLifecycleRequested: synced?.ConversationThreadLifecycleRequested ?? (() => Effect.void),
    ConversationThreadDeleteRequested: synced?.ConversationThreadDeleteRequested ?? (() => Effect.void),
    ConversationThreadDeleteConfirmed: synced?.ConversationThreadDeleteConfirmed ?? (() => Effect.void),
    ConversationThreadDeleteCancelled: synced?.ConversationThreadDeleteCancelled ?? (() => Effect.void),
    RuntimeInteractionOptionToggled: synced?.RuntimeInteractionOptionToggled ?? (() => Effect.void),
    RuntimeInteractionDecisionSubmitted: synced?.RuntimeInteractionDecisionSubmitted ?? (() => Effect.void),
    RuntimeTurnControlRequested: synced?.RuntimeTurnControlRequested ?? (() => Effect.void),
    // MOB-FA-02 (#8994): dispatches through `options.fullAutoControl`
    // (`makeFullAutoRunControlDispatcher`), which durably records the
    // intent and polls for a receipted outcome. Independent of the
    // conversation `synced` host -- Full Auto remote control works even
    // when the phone is not currently viewing a synced conversation thread.
    FullAutoRunControlRequested: (payload: Readonly<{ action: FullAutoRunControlAction; runRef: string }>) =>
      Effect.gen(function* () {
        if (options.fullAutoControl === undefined) return
        const before = yield* SubscriptionRef.get(state)
        // One in-flight control intent at a time -- a second tap while one
        // is already pending is a no-op, never a second dispatch racing the
        // first (the durable idempotency key would dedupe server-side
        // anyway, but this also avoids a confusing double "Pausing…" UI).
        if (before.fullAutoControlPending !== null) return
        yield* SubscriptionRef.update(state, current => ({
          ...current,
          fullAutoControlPending: payload.action,
          fullAutoControlOutcome: null,
        }))
        const outcome = yield* Effect.promise(() => options.fullAutoControl!({
          runRef: payload.runRef,
          action: payload.action,
        }))
        yield* SubscriptionRef.update(state, current => {
          // A receipted `applied` outcome IS the durable confirmed truth
          // (not an optimistic guess) -- fold the resulting lifecycle state
          // into the displayed projection immediately rather than waiting
          // for the next poll cycle, same as any other confirmed-state
          // update in this program.
          const projection = current.fullAutoRun
          const nextFullAutoRun = outcome.state === "applied" && outcome.resultLifecycleState !== null &&
              projection?.state === "active" && projection.projection.runRef === payload.runRef
            ? {
                ...projection,
                projection: { ...projection.projection, lifecycleState: outcome.resultLifecycleState },
              }
            : current.fullAutoRun
          return {
            ...current,
            fullAutoControlPending: null,
            fullAutoControlOutcome: outcome,
            fullAutoRun: nextFullAutoRun,
          }
        })
      }),
    CodingSessionSelected: options.coding === undefined
      ? () => Effect.void
      : payload => Effect.gen(function* () {
          const before = yield* SubscriptionRef.get(state)
          if (before.khala.pending) return
          yield* Effect.promise(selectedThreadLease.clear)
          yield* SubscriptionRef.update(state, current => ({
            ...current,
            drawerOpen: false,
            workspaceFocusTarget: "transcript" as const,
            workbenchRoute: "conversation" as const,
            repositoryBrowser: initialMobileRepositoryBrowserState,
            repositoryReview: initialMobileRepositoryReviewState,
            repositoryGit: initialMobileRepositoryGitState,
            repositoryTerminal: initialMobileRepositoryTerminalState,
            codingComposerTargetPickerOpen: false,
            codingComposerTargetSearch: "",
            codingPathDiscovery: { state: "idle" as const },
            codingAttachmentPicking: false,
            codingAttachmentMutatingRef: null,
            codingAttachmentStatus: null,
            khala: { ...current.khala, pending: true },
          }))
          const selected = yield* Effect.promise(() => options.coding!.selectSession({
            schema: "openagents.mobile.coding_target.v1",
            repositoryRef: payload.repositoryRef,
            sessionRef: payload.sessionRef,
            threadRef: payload.threadRef,
          }, update => {
            Effect.runFork(SubscriptionRef.update(state, current =>
              current.activeThreadRef === update.threadRef
                ? withConfirmedThread(current, update)
                : current))
          }))
          yield* SubscriptionRef.update(state, current => selected === null
            ? failedConversationState(current, "Coding session is unavailable or no longer authorized.")
            : {
                ...withConfirmedThread(current, selected.thread),
                codingComposer: selected.composer,
                codingComposerTargetPickerOpen: false,
                codingComposerTargetSearch: "",
                codingPathDiscovery: { state: "idle" as const },
                codingAttachmentPicking: false,
                codingAttachmentMutatingRef: null,
                codingAttachmentStatus: null,
                khala: {
                  ...withConfirmedThread(current, selected.thread).khala,
                  draft: selected.composer === null
                    ? ""
                    : mobileCodingComposerText(selected.composer.draft),
                },
              })
        }),
    ...(synced === undefined
      ? khalaHandlers(state, options.khalaTurn)
      : {
          KhalaDraftChanged: synced.KhalaDraftChanged,
          KhalaTurnSubmitted: synced.KhalaTurnSubmitted,
        }),
  }
}

export interface HomeProgramHandle {
  readonly initialState: HomeState
  readonly viewStream: Stream.Stream<View>
  readonly contentViewStream: Stream.Stream<View>
  readonly drawerViewStream: Stream.Stream<View>
  readonly report: IntentReporter
  readonly close: () => Promise<void>
  readonly stateChanges: Stream.Stream<HomeState>
  readonly chrome: {
    readonly toggleDrawer: () => void
    readonly pressNewChat: () => void
    readonly pressSettings: () => void
    readonly selectSurfaceMode: (mode: SurfaceMode) => void
  }
  readonly khala: {
    readonly draftChanged: (text: string) => void
    readonly submitTurn: (text: string) => void
    readonly toggleInteractionOption: (input: Readonly<{
      interactionRef: string
      questionRef: string
      optionRef: string
      multiSelect: boolean
    }>) => void
    readonly submitInteractionDecision: (input: Readonly<
      | { interactionRef: string; turnRef: string; kind: "provider_question" }
      | { interactionRef: string; turnRef: string; kind: "tool_approval"; outcome: "approve" | "deny" }
      | { interactionRef: string; turnRef: string; kind: "plan_review"; outcome: "accept" | "request_changes" | "replan" }
    >) => void
    readonly controlTurn: (input: Readonly<{
      action: MobileRuntimeControlAction
      runRef: string
    }>) => void
    readonly requestStopConfirmation: (runRef: string) => void
    readonly dismissStopConfirmation: (runRef: string) => void
    readonly confirmStop: (runRef: string) => void
    readonly toggleAgentStack: () => void
    readonly selectAgentRow: (agentRef: string) => void
    readonly toggleWorkGroup: (groupRef: string) => void
    readonly toggleWorkItem: (itemRef: string) => void
    readonly setTranscriptPinned: (pinned: boolean) => void
    readonly loadEarlierTranscript: () => void
    readonly jumpToLatestTranscript: () => void
    readonly openAttachment: (attachmentRef: string) => void
    readonly settleAttachmentLoad: (attachmentRef: string, outcome: "ready" | "failed") => void
    readonly retryAttachment: (attachmentRef: string) => void
    readonly dismissAttachmentViewer: (attachmentRef: string) => void
  }
  readonly sync: {
    readonly setPhase: (phase: MobileSyncPhase) => void
  }
  readonly workspace: {
    readonly setWidth: (width: number) => void
    readonly dispatchKeyboardCommand: (command: MobileWorkspaceKeyboardCommand) => void
  }
  readonly fullAuto: {
    /** Pushes a freshly-polled `FullAutoRun` projection into state (openagents
     * #8982), mirroring `sync.setPhase`'s "push external state in" shape so
     * the state header updates live without an app restart. */
    readonly setProjection: (result: FullAutoRunProjectionResult | null) => void
    /** MOB-FA-02 (#8994): dispatches a Pause/Resume/Stop control intent
     * against the named run. Fire-and-forget from the caller's perspective;
     * the resulting pending/applied/rejected state is observable through
     * `fullAutoRunHeaderForState`'s `control` field on the next render. */
    readonly dispatchControl: (runRef: string, action: FullAutoRunControlAction) => void
  }
  readonly controller: {
    readonly selectDestination: (destination: MobileControllerDestination) => void
    readonly inspectSession: (sessionRef: string) => void
    readonly selectAttention: (target: MobileAttentionTarget) => Promise<void>
    readonly selectPortableDestination: (targetRef: string) => void
    readonly requestPortableControl: (action: MobilePortableControlAction) => void
  }
  readonly accessibility: {
    readonly setProfile: (profile: MobileAccessibilityProfile) => void
  }
  readonly coding: {
    readonly selectSession: (target: MobileCodingTarget) => void
    readonly openChanges: () => void
    readonly selectChangedFile: (pathRef: string, source: "staged" | "unstaged" | "untracked", revisionRef: string) => void
    readonly selectReviewRow: (rowId: string) => void
    readonly changeReviewComment: (comment: string) => void
    readonly submitReview: () => void
    readonly openGit: () => void
    readonly selectGitBranch: (branchRef: string, name: string) => void
    readonly toggleGitFile: (pathRef: string) => void
    readonly changeGitCommitMessage: (message: string) => void
    readonly requestGitCommit: () => void
    readonly requestGitPush: () => void
    readonly acceptGitConfirmation: () => void
    readonly cancelGitConfirmation: () => void
    readonly openTerminal: () => void
    readonly refreshTerminal: () => void
    readonly recoverTerminal: () => void
    readonly createTerminal: () => void
    readonly selectTerminal: (terminalRef: string) => void
    readonly sendTerminalData: (data: string) => void
    readonly resizeTerminal: (cols: number, rows: number) => void
    readonly interruptTerminal: () => void
    readonly restartTerminal: () => void
    readonly closeTerminal: () => void
    readonly openFiles: () => void
    readonly closeFiles: () => void
    readonly refreshFiles: () => void
    readonly toggleDirectory: (pathRef: string, revisionRef: string) => void
    readonly selectFile: (pathRef: string, revisionRef: string) => void
    readonly pickAttachments: () => void
    readonly removeAttachment: (attachmentId: string) => void
    readonly retryAttachment: (attachmentId: string) => void
    readonly selectTarget: (targetId: string) => void
    readonly openTargetPicker: () => void
    readonly dismissTargetPicker: () => void
    readonly searchTargets: (search: string) => void
    readonly searchSlashCommands: (query: string) => void
    readonly selectSlashCommand: (commandId: (typeof mobileSlashCommandIds)[number]) => void
    readonly searchPaths: (query: string) => void
    readonly selectPath: (pathRef: string) => void
  }
  readonly session: {
    readonly signIn: () => void
    readonly signOut: () => void
  }
  readonly settings: {
    readonly setIncomingShare: (share: MobileShareIntake | null) => void
  }
}

export const buildHomeProgram = (options: HomeProgramOptions = {}): HomeProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const baseInitialState = initialHomeStateForConversation(
        options.conversation,
        options.accessibility,
      )
      const sarahIsInitialThread = options.sarah !== undefined &&
        baseInitialState.activeThreadRef === options.sarah.threadRef
      const activeComposer = sarahIsInitialThread
        ? null
        : options.coding?.activeComposer() ?? null
      const programInitialState: HomeState = {
        ...baseInitialState,
        sarah: options.sarah ?? null,
        workspaceLayoutMode: mobileWorkspaceLayoutMode(options.workspaceWidth ?? 390),
        codingDirectory: options.coding?.directory ?? null,
        portableSnapshot: options.coding?.portableSnapshot ?? null,
        attentionSnapshot: options.coding?.attentionSnapshot ?? null,
        codingComposer: activeComposer,
        codingExecutionTargets: options.coding?.executionTargets ?? [],
        ...(options.coding?.fleetRuns === undefined
          ? {}
          : { fleetRuns: options.coding.fleetRuns }),
        fullAutoRun: options.fullAutoRun ?? baseInitialState.fullAutoRun,
        settings: {
          ...baseInitialState.settings,
          incomingShare: options.settings?.incomingShare ?? null,
        },
        codingExecutionTargetCatalogRequired:
          options.coding?.executionTargets !== undefined,
        khala: activeComposer === null
          ? baseInitialState.khala
          : {
              ...baseInitialState.khala,
              draft: mobileCodingComposerText(activeComposer.draft),
            },
      }
      const state = yield* SubscriptionRef.make<HomeState>(programInitialState)
      const selectedThreadLease: MobileSelectedThreadLeaseController =
        options.conversation === undefined
          ? {
              active: () => null,
              activate: async () => null,
              clear: async () => undefined,
              close: async () => undefined,
            }
          : makeMobileSelectedThreadLeaseController(options.conversation.host)
      const registry = yield* makeIntentRegistry(
        homeIntentDefinitions,
        makeHomeHandlers(state, options, selectedThreadLease),
      )
      const report: IntentReporter = (ref, runtimeValue) => registry.dispatch(resolveIntentRef(ref, runtimeValue))
      const fireRef = (ref: ReturnType<typeof IntentRef>): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(ref))))
      }
      const fireText = (ref: ReturnType<typeof IntentRef>, value: string): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(ref, value))))
      }
      const fire = (name: string) => (): void => fireRef(IntentRef(name, StaticPayload({})))
      const submitKhala = (text: string): void => fireText(IntentRef("KhalaTurnSubmitted", ComponentValueBinding()), text)
      const initialThreadRef = programInitialState.activeThreadRef
      const initialThreadIsCoding = initialThreadRef !== null &&
        options.coding?.directory.sessions.some(session => session.threadRef === initialThreadRef) === true
      if (initialThreadRef !== null && !initialThreadIsCoding &&
          options.conversation?.host.watchThread !== undefined) {
        Effect.runFork(Effect.promise(() => selectedThreadLease.activate(
          initialThreadRef,
          update => {
            Effect.runFork(SubscriptionRef.update(state, current =>
              current.activeThreadRef === update.threadRef
                ? withConfirmedThread(current, update)
                : current))
          },
        )))
      }
      return {
        initialState: programInitialState,
        viewStream: makeViewProgramFromState(state, renderHomeView).viewStream,
        contentViewStream: makeViewProgramFromState(state, renderContentView).viewStream,
        drawerViewStream: makeViewProgramFromState(state, renderDrawerView).viewStream,
        report,
        close: selectedThreadLease.close,
        stateChanges: SubscriptionRef.changes(state),
        chrome: {
          toggleDrawer: fire("DrawerToggled"),
          pressNewChat: fire("NewChatPressed"),
          pressSettings: fire("SettingsPressed"),
          selectSurfaceMode: (mode) => fireRef(IntentRef("SurfaceModeSelected", StaticPayload({ mode }))),
        },
        khala: {
          draftChanged: (text) => fireText(IntentRef("KhalaDraftChanged", ComponentValueBinding()), text),
          submitTurn: submitKhala,
          toggleInteractionOption: input => fireRef(IntentRef(
            "RuntimeInteractionOptionToggled",
            StaticPayload(input),
          )),
          submitInteractionDecision: input => fireRef(IntentRef(
            "RuntimeInteractionDecisionSubmitted",
            StaticPayload(input),
          )),
          controlTurn: input => fireRef(IntentRef(
            "RuntimeTurnControlRequested",
            StaticPayload(input),
          )),
          requestStopConfirmation: runRef => fireRef(IntentRef(
            "RuntimeTurnStopConfirmationRequested",
            StaticPayload({ runRef }),
          )),
          dismissStopConfirmation: runRef => fireRef(IntentRef(
            "RuntimeTurnStopConfirmationDismissed",
            StaticPayload({ runRef }),
          )),
          confirmStop: runRef => fireRef(IntentRef(
            "RuntimeTurnStopConfirmed",
            StaticPayload({ runRef }),
          )),
          toggleAgentStack: fire(AgentStackToggled),
          selectAgentRow: agentRef => fireRef(IntentRef(
            AgentRowSelected,
            StaticPayload({ agentRef }),
          )),
          toggleWorkGroup: groupRef => fireRef(IntentRef(
            WorkGroupToggled,
            StaticPayload({ groupRef }),
          )),
          toggleWorkItem: itemRef => fireRef(IntentRef(
            WorkItemToggled,
            StaticPayload({ itemRef }),
          )),
          setTranscriptPinned: pinned => fireRef(IntentRef(
            TranscriptPinnedChanged,
            StaticPayload(pinned),
          )),
          loadEarlierTranscript: fire(TranscriptEarlierHistoryRequested),
          jumpToLatestTranscript: fire(TranscriptJumpToLatestRequested),
          openAttachment: attachmentRef => fireRef(IntentRef(
            TranscriptAttachmentOpened,
            StaticPayload({ attachmentRef }),
          )),
          settleAttachmentLoad: (attachmentRef, outcome) => fireRef(IntentRef(
            TranscriptAttachmentLoadSettled,
            StaticPayload({ attachmentRef, outcome }),
          )),
          retryAttachment: attachmentRef => fireRef(IntentRef(
            TranscriptAttachmentRetryRequested,
            StaticPayload({ attachmentRef }),
          )),
          dismissAttachmentViewer: attachmentRef => fireRef(IntentRef(
            TranscriptAttachmentViewerDismissed,
            StaticPayload({ attachmentRef }),
          )),
        },
        sync: {
          setPhase: phase => {
            if (phase !== "live" && phase !== "catching_up") {
              void selectedThreadLease.clear()
            }
            Effect.runFork(SubscriptionRef.update(state, current =>
              current.conversationAuthority === "sync" && phase !== "live" && phase !== "catching_up"
                ? {
                    ...current,
                    syncPhase: phase,
                    conversationThreads: [],
                    archivedConversationThreads: [],
                    activeThreadRef: null,
                    workbenchRoute: "conversation" as const,
                    repositoryBrowser: initialMobileRepositoryBrowserState,
                    repositoryReview: initialMobileRepositoryReviewState,
                    repositoryGit: initialMobileRepositoryGitState,
                    repositoryTerminal: initialMobileRepositoryTerminalState,
                    threadLifecycle: initialHomeState.threadLifecycle,
                    codingComposer: null,
                    codingComposerTargetPickerOpen: false,
                    codingComposerTargetSearch: "",
                    codingPathDiscovery: { state: "idle" as const },
                    portableSnapshot: null,
                    attentionSnapshot: null,
                    selectedPortableDestinationRef: null,
                    portableSubmittingAction: null,
                    portableNotice: null,
                    codingAttachmentPicking: false,
                    codingAttachmentMutatingRef: null,
                    codingAttachmentStatus: null,
                    khala: initialKhalaState,
                  }
                : { ...current, syncPhase: phase }))
          },
        },
        workspace: {
          setWidth: width => fireRef(IntentRef(
            "WorkspaceViewportWidthChanged",
            StaticPayload(width),
          )),
          dispatchKeyboardCommand: command => fireRef(IntentRef(
            "WorkspaceKeyboardCommandReceived",
            StaticPayload(command),
          )),
        },
        fullAuto: {
          setProjection: result => {
            Effect.runFork(SubscriptionRef.update(state, current => ({
              ...current,
              fullAutoRun: result,
            })))
          },
          dispatchControl: (runRef, action) => fireRef(IntentRef(
            "FullAutoRunControlRequested",
            StaticPayload({ runRef, action }),
          )),
        },
        controller: {
          selectDestination: destination => fireText(
            IntentRef("ControllerDestinationSelected", ComponentValueBinding()),
            destination,
          ),
          inspectSession: sessionRef => fireRef(IntentRef(
            "ControllerSessionInspected",
            StaticPayload({ sessionRef }),
          )),
          selectAttention: target => Effect.runPromise(Effect.exit(registry.dispatch(resolveIntentRef(
            IntentRef("ControllerAttentionSelected", StaticPayload(target)),
          ))).pipe(Effect.asVoid)),
          selectPortableDestination: targetRef => fireText(
            IntentRef("PortableDestinationSelected", ComponentValueBinding()),
            targetRef,
          ),
          requestPortableControl: action => fireRef(IntentRef(
            "PortableControlRequested",
            StaticPayload({ action }),
          )),
        },
        accessibility: {
          setProfile: profile => {
            Effect.runFork(SubscriptionRef.update(state, current => ({
              ...current,
              accessibility: normalizeMobileAccessibilityProfile(profile),
            })))
          },
        },
        coding: {
          selectSession: target => fireRef(IntentRef("CodingSessionSelected", StaticPayload({
            repositoryRef: target.repositoryRef,
            sessionRef: target.sessionRef,
            threadRef: target.threadRef,
          }))),
          openChanges: fire("ChangesRouteOpened"),
          selectChangedFile: (pathRef, source, revisionRef) => fireRef(IntentRef(
            "RepositoryChangedFileSelected",
            StaticPayload({ pathRef, source, revisionRef }),
          )),
          selectReviewRow: rowId => fireRef(IntentRef(
            "RepositoryReviewRowSelected",
            StaticPayload({ rowId }),
          )),
          changeReviewComment: comment => fireText(
            IntentRef("RepositoryReviewCommentChanged", ComponentValueBinding()),
            comment,
          ),
          submitReview: fire("RepositoryReviewSubmitted"),
          openGit: fire("GitRouteOpened"),
          selectGitBranch: (branchRef, name) => fireRef(IntentRef(
            "RepositoryGitBranchSelected",
            StaticPayload({ branchRef, name }),
          )),
          toggleGitFile: pathRef => fireRef(IntentRef(
            "RepositoryGitFileToggled",
            StaticPayload({ pathRef }),
          )),
          changeGitCommitMessage: message => fireText(
            IntentRef("RepositoryGitCommitMessageChanged", ComponentValueBinding()),
            message,
          ),
          requestGitCommit: fire("RepositoryGitCommitRequested"),
          requestGitPush: fire("RepositoryGitPushRequested"),
          acceptGitConfirmation: fire("RepositoryGitConfirmationAccepted"),
          cancelGitConfirmation: fire("RepositoryGitConfirmationCancelled"),
          openTerminal: fire("TerminalRouteOpened"),
          refreshTerminal: fire("RepositoryTerminalRefreshed"),
          recoverTerminal: fire("RepositoryTerminalForegrounded"),
          createTerminal: fire("RepositoryTerminalCreateRequested"),
          selectTerminal: terminalRef => fireRef(IntentRef(
            "RepositoryTerminalSelected",
            StaticPayload({ terminalRef }),
          )),
          sendTerminalData: data => fireRef(IntentRef(
            "RepositoryTerminalHostEvent",
            StaticPayload({ type: "data", data }),
          )),
          resizeTerminal: (cols, rows) => fireRef(IntentRef(
            "RepositoryTerminalHostEvent",
            StaticPayload({ type: "resize", cols, rows }),
          )),
          interruptTerminal: fire("RepositoryTerminalInterruptRequested"),
          restartTerminal: fire("RepositoryTerminalRestartRequested"),
          closeTerminal: fire("RepositoryTerminalCloseRequested"),
          openFiles: fire("FilesRouteOpened"),
          closeFiles: fire("FilesRouteClosed"),
          refreshFiles: fire("RepositoryFilesRefreshed"),
          toggleDirectory: (pathRef, revisionRef) => fireRef(IntentRef(
            "RepositoryDirectoryToggled",
            StaticPayload({ pathRef, revisionRef }),
          )),
          selectFile: (pathRef, revisionRef) => fireRef(IntentRef(
            "RepositoryFileSelected",
            StaticPayload({ pathRef, revisionRef }),
          )),
          pickAttachments: fire("CodingComposerAttachmentsRequested"),
          removeAttachment: attachmentId => fireRef(IntentRef(
            "CodingComposerAttachmentRemoved",
            StaticPayload({ attachmentId }),
          )),
          retryAttachment: attachmentId => fireRef(IntentRef(
            "CodingComposerAttachmentRetryRequested",
            StaticPayload({ attachmentId }),
          )),
          selectTarget: targetId => fireRef(IntentRef(
            "CodingExecutionTargetSelected",
            StaticPayload({ targetId }),
          )),
          openTargetPicker: fire("CodingComposerTargetPickerOpened"),
          dismissTargetPicker: fire("CodingComposerTargetPickerDismissed"),
          searchTargets: search => fireText(
            IntentRef("CodingComposerTargetSearchChanged", ComponentValueBinding()),
            search,
          ),
          searchSlashCommands: query => fireText(
            IntentRef("CodingComposerSlashQueryChanged", ComponentValueBinding()),
            query,
          ),
          selectSlashCommand: commandId => fireRef(IntentRef(
            "CodingComposerSlashCommandSelected",
            StaticPayload(commandId),
          )),
          searchPaths: query => fireText(
            IntentRef("CodingComposerPathQueryChanged", ComponentValueBinding()),
            query,
          ),
          selectPath: pathRef => fireText(
            IntentRef("CodingComposerPathSelected", ComponentValueBinding()),
            pathRef,
          ),
        },
        session: {
          signIn: fire("OpenAgentsSignInPressed"),
          signOut: fire("OpenAgentsSignOutPressed"),
        },
        settings: {
          setIncomingShare: share => {
            Effect.runFork(SubscriptionRef.update(state, current => ({
              ...current,
              settings: { ...current.settings, incomingShare: share },
            })))
          },
        },
      }
    }),
  )
