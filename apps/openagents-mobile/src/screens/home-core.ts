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
  Spacer,
  Stack,
  StaticPayload,
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
import type { FleetRunClientProjection } from "@openagentsinc/khala-sync"

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
  type KhalaState,
  type KhalaTurnClient,
  type MobileAccessibilityProfile,
} from "./khala-core"
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
 * Persona-neutral mobile home. Sol roadmap rev-24 explicitly pauses named
 * assistants as a product front door: mobile owns truthful supervision and continuity, not
 * relationship state or presentation demos. The existing conversation surface
 * is driven by confirmed personal Sync when live and the public Khala client
 * when startup selects the explicit local fallback.
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
  readonly surfaceMode: SurfaceMode
  readonly modeMenuOpen: boolean
  readonly syncPhase: MobileSyncPhase
  readonly conversationAuthority: "local" | "sync"
  readonly conversationThreads: ReadonlyArray<MobileConversationThreadSummary>
  readonly archivedConversationThreads: ReadonlyArray<MobileConversationThreadSummary>
  readonly activeThreadRef: string | null
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
  surfaceMode: "khala",
  modeMenuOpen: false,
  syncPhase: "unconfigured",
  conversationAuthority: "local",
  conversationThreads: [],
  archivedConversationThreads: [],
  activeThreadRef: null,
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
  codingExecutionTargetCatalogRequired: false,
  codingAttachmentPicking: false,
  codingAttachmentMutatingRef: null,
  codingAttachmentStatus: null,
  accessibility: defaultMobileAccessibilityProfile,
  khala: initialKhalaState,
}

/** Visible embedded-binary tag; build 116 removes the named-persona front door. */
export const BUNDLE_TAG = "2026-07-12.cut-09-runtime-selector-atomic"

const EmptyPayload = Schema.Struct({})

export const DrawerToggled = defineIntent("DrawerToggled", EmptyPayload)
export const NewChatPressed = defineIntent("NewChatPressed", EmptyPayload)
export const SettingsPressed = defineIntent("SettingsPressed", EmptyPayload)
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
  NewChatPressed,
  SettingsPressed,
  OpenAgentsSignInPressed,
  OpenAgentsSignOutPressed,
  SurfaceModeSelected,
  ConversationThreadSelected,
  WorkspaceSearchChanged,
  WorkspaceStatusFilterSelected,
  WorkspaceProjectFilterSelected,
  WorkspaceFiltersCleared,
  WorkspaceRowActionsToggled,
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
  RuntimeInteractionOptionToggled,
  RuntimeInteractionDecisionSubmitted,
  RuntimeTurnControlRequested,
  RuntimeTurnStopConfirmationRequested,
  RuntimeTurnStopConfirmationDismissed,
  RuntimeTurnStopConfirmed,
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
  if (state.surfaceMode === "openagents") {
    return { title: "OpenAgents", subtitle: syncStatusCopy(state.syncPhase).title }
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

export const chromeProps = (state: HomeState): ChromeProps => ({
  pillLabel: state.conversationAuthority === "sync" && state.surfaceMode === "khala"
    ? "OpenAgents"
    : surfaceModeOptions.find((option) => option.id === state.surfaceMode)?.label ?? "OpenAgents",
  composerPlaceholder: state.conversationAuthority === "sync" ? "Continue conversation" : "Message Khala",
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
    state.surfaceMode === "khala"
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
            accessibilityLabel: state.drawerOpen ? "Close threads list" : "Go to threads list",
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
              style: { surface: "glass", borderRadius: "full" },
            },
            [
              IconButton({
                key: "home-new-chat",
                icon: "Compose",
                accessibilityLabel: "New chat",
                onPress: IntentRef("NewChatPressed", StaticPayload({})),
                style: mobileInteractiveStyle(state.accessibility),
              }),
              IconButton({
                key: "home-more",
                icon: "Ellipsis",
                accessibilityLabel: "Open settings",
                onPress: IntentRef("SettingsPressed", StaticPayload({})),
                style: mobileInteractiveStyle(state.accessibility),
              }),
            ],
          ),
        ],
      ),
      state.drawerOpen ? renderDrawerView(state) : renderContentView(state),
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

const workspaceRow = (row: MobileWorkspaceRow, state: HomeState): View => {
  const openIntent = row.attentionTarget !== null
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
  const canOpen = row.state !== "archived" && row.state !== "recovery"
  const hasLifecycle = state.conversationAuthority === "sync" &&
    [...state.conversationThreads, ...state.archivedConversationThreads]
      .some(thread => thread.threadRef === row.threadRef)
  const metadata = [
    row.projectLabel,
    row.worktreeLabel === null ? null : `Worktree ${row.worktreeLabel}`,
    row.recencyLabel,
  ].filter((value): value is string => value !== null).join(" · ")
  return Stack({
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

export const renderDrawerView = (state: HomeState): View =>
  Stack(
    { key: "drawer-root", direction: "column", gap: "2", padding: "4", style: { width: "full", height: "full", backgroundColor: "surface" } },
    [
      Spacer({ key: "drawer-top-space", size: "10" }),
      drawerRow({ key: "drawer-new-chat", label: "New chat", onPress: IntentRef("NewChatPressed", StaticPayload({})), selected: state.surfaceMode === "khala" && state.khala.entries.length === 0 }, state.accessibility),
      drawerRow({
        key: "drawer-current-surface",
        label: state.conversationAuthority === "sync" ? "OpenAgents" : "Khala",
        onPress: IntentRef("SurfaceModeSelected", StaticPayload({ mode: "khala" })),
        selected: state.surfaceMode === "khala",
      }, state.accessibility),
      ...workspaceNavigationRows(state),
      ...((state.fleetRuns?.runs.length ?? 0) === 0
        ? []
        : [Text({
            key: "workspace-fleet-summary",
            content: `Fleet activity · ${state.fleetRuns!.runs.length} ${state.fleetRuns!.runs.length === 1 ? "run" : "runs"} · ${state.fleetRuns!.runs.filter(run => run.executionState === "completed").length} completed`,
            variant: "caption",
            color: "textMuted",
          })]),
      ...threadLifecycleRows(state),
      ...codingOfflineCacheAccountingRows(state),
      Spacer({ key: "drawer-flex-space", size: "8" }),
      drawerRow({ key: "drawer-settings", label: "Settings", onPress: IntentRef("SettingsPressed", StaticPayload({})) }, state.accessibility),
      Text({ key: "drawer-bundle", content: `Bundle ${BUNDLE_TAG}`, variant: "caption", color: "textMuted" }),
    ],
  )

export interface HomeProgramOptions {
  readonly khalaTurn?: KhalaTurnClient
  readonly sessionActions?: Readonly<{
    signIn: () => Promise<void>
    signOut: () => Promise<void>
  }>
  readonly conversation?: Extract<MobileConversationSelection, { readonly mode: "sync" }>
  readonly accessibility?: MobileAccessibilityProfile
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
    thread?.timeline?.events ?? [],
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
      surfaceMode: "khala" as const,
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
    if (before.khala.pending) return
    if (coding !== undefined) yield* Effect.promise(coding.clearSelection)
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      codingComposer: null,
      codingComposerTargetPickerOpen: false,
      codingComposerTargetSearch: "",
      codingPathDiscovery: { state: "idle" as const },
      codingAttachmentPicking: false,
      codingAttachmentMutatingRef: null,
      codingAttachmentStatus: null,
      khala: { ...current.khala, pending: true },
    }))
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
    if (message === "" && (before.codingComposer?.draft.doc.attachments.length ?? 0) === 0) return
    if (before.khala.pending) return
    if (before.codingComposer !== null &&
      before.codingComposer.draft.target.readiness !== "ready") return
    const selectedExecutionTarget = before.codingComposer === null
      ? undefined
      : before.codingExecutionTargets.find(option =>
          option.targetId === before.codingComposer!.draft.target.executionTargetRef &&
          option.readiness === "ready")
    if (before.codingComposer !== null &&
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
    const prepared = before.codingComposer === null || coding === undefined
      ? { ok: true as const, body: message }
      : coding.prepareComposerSubmission === undefined
        ? before.codingComposer.draft.doc.attachments.length === 0
          ? { ok: true as const, body: message }
          : { ok: false as const, error: "Attachment delivery is unavailable. The draft was kept." }
        : yield* Effect.promise(() => coding.prepareComposerSubmission!(
            before.codingComposer!,
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

    const result = yield* Effect.promise(() => host.sendMessage({
      threadRef,
      body: prepared.body,
      ...(prepared.attachments === undefined ? {} : { attachments: prepared.attachments }),
      ...(selectedExecutionTarget === undefined
        ? {}
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
    const settledComposer = !result.ok || before.codingComposer === null || coding === undefined
      ? before.codingComposer
      : coding.clearComposer === undefined
        ? yield* Effect.promise(() => coding.updateComposerText(before.codingComposer!, ""))
        : yield* Effect.promise(() => coding.clearComposer!(before.codingComposer!))
    yield* SubscriptionRef.update(state, current => result.ok
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
    DrawerToggled: () => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: !current.drawerOpen })),
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
    NewChatPressed: synced?.NewChatPressed ??
      (() => SubscriptionRef.update(state, (current) => ({
        ...current,
        drawerOpen: false,
        surfaceMode: "khala" as const,
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
    SettingsPressed: () => SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      surfaceMode: surfaceModeOptions[0]?.id ?? current.surfaceMode,
    })),
    OpenAgentsSignInPressed: () => options.sessionActions === undefined
      ? Effect.void
      : Effect.promise(options.sessionActions.signIn),
    OpenAgentsSignOutPressed: () => options.sessionActions === undefined
      ? Effect.promise(selectedThreadLease.clear)
      : Effect.gen(function* () {
          yield* Effect.promise(selectedThreadLease.clear)
          yield* Effect.promise(options.sessionActions!.signOut)
        }),
    SurfaceModeSelected: (payload) => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: false, surfaceMode: payload.mode as SurfaceMode })),
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
    CodingSessionSelected: options.coding === undefined
      ? () => Effect.void
      : payload => Effect.gen(function* () {
          const before = yield* SubscriptionRef.get(state)
          if (before.khala.pending) return
          yield* Effect.promise(selectedThreadLease.clear)
          yield* SubscriptionRef.update(state, current => ({
            ...current,
            drawerOpen: false,
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
}

export const buildHomeProgram = (options: HomeProgramOptions = {}): HomeProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const baseInitialState = initialHomeStateForConversation(
        options.conversation,
        options.accessibility,
      )
      const activeComposer = options.coding?.activeComposer() ?? null
      const programInitialState: HomeState = {
        ...baseInitialState,
        codingDirectory: options.coding?.directory ?? null,
        portableSnapshot: options.coding?.portableSnapshot ?? null,
        attentionSnapshot: options.coding?.attentionSnapshot ?? null,
        codingComposer: activeComposer,
        codingExecutionTargets: options.coding?.executionTargets ?? [],
        ...(options.coding?.fleetRuns === undefined
          ? {}
          : { fleetRuns: options.coding.fleetRuns }),
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
      }
    }),
  )
