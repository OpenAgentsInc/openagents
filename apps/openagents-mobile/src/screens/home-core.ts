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
  Spacer,
  Stack,
  StaticPayload,
  Text,
  Toolbar,
  type View,
} from "@effect-native/core"
import {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
  resolveLiveAgentGraphSelection,
  type ScopeSyncState,
} from "@openagentsinc/khala-sync-client"
import type { FleetRunClientProjection } from "@openagentsinc/khala-sync"

import type {
  MobileCodingDirectory,
  MobileCodingTarget,
} from "../coding/mobile-coding-navigation"
import {
  mobileCodingComposerText,
  type MobileCodingAttachmentUpdateResult,
  type MobileCodingComposerSession,
} from "../coding/mobile-coding-composer"
import type { MobileCodingAttachmentDeliveryResult } from "../coding/mobile-coding-attachment-delivery"
import type { MobileExecutionTargetOption } from "../coding/mobile-execution-targets"
import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
  MobileConversationThreadSummary,
  MobileRuntimeControlAction,
} from "../conversation/mobile-conversation"

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
  type KhalaState,
  type KhalaTurnClient,
  type MobileAccessibilityProfile,
} from "./khala-core"

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
  readonly activeThreadRef: string | null
  readonly codingDirectory: MobileCodingDirectory | null
  readonly codingComposer: MobileCodingComposerSession | null
  readonly codingExecutionTargets: ReadonlyArray<MobileExecutionTargetOption>
  readonly fleetRuns?: FleetRunClientProjection
  readonly codingExecutionTargetCatalogRequired: boolean
  readonly codingAttachmentPicking: boolean
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
  activeThreadRef: null,
  codingDirectory: null,
  codingComposer: null,
  codingExecutionTargets: [],
  codingExecutionTargetCatalogRequired: false,
  codingAttachmentPicking: false,
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
export const CodingSessionSelected = defineIntent(
  "CodingSessionSelected",
  Schema.Struct({
    repositoryRef: Schema.String,
    sessionRef: Schema.String,
    threadRef: Schema.String,
  }),
)
export const CodingComposerAttachmentsRequested = defineIntent(
  "CodingComposerAttachmentsRequested",
  EmptyPayload,
)
export const CodingExecutionTargetSelected = defineIntent(
  "CodingExecutionTargetSelected",
  Schema.Struct({ targetId: Schema.String }),
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
export const AgentStackToggledIntent = defineIntent(AgentStackToggled, EmptyPayload)
export const AgentRowSelectedIntent = defineIntent(
  AgentRowSelected,
  Schema.Struct({ agentRef: Schema.String }),
)

export const homeIntentDefinitions = [
  DrawerToggled,
  NewChatPressed,
  SettingsPressed,
  OpenAgentsSignInPressed,
  OpenAgentsSignOutPressed,
  SurfaceModeSelected,
  ConversationThreadSelected,
  CodingSessionSelected,
  CodingComposerAttachmentsRequested,
  CodingExecutionTargetSelected,
  RuntimeInteractionOptionToggled,
  RuntimeInteractionDecisionSubmitted,
  RuntimeTurnControlRequested,
  AgentStackToggledIntent,
  AgentRowSelectedIntent,
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
          state.accessibility,
          state.codingExecutionTargets,
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
      gap: "2",
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
          surface: "glass",
          style: { width: "full", minHeight: state.accessibility.minTouchTarget },
        },
        [
          IconButton({
            key: "home-navigation",
            icon: "Menu",
            accessibilityLabel: state.drawerOpen ? "Close navigation" : "Open navigation",
            onPress: IntentRef("DrawerToggled", StaticPayload({})),
            style: mobileInteractiveStyle(state.accessibility),
          }),
          Button({
            key: "home-surface-mode",
            label: chromeProps(state).pillLabel,
            variant: "ghost",
            onPress: IntentRef(
              "SurfaceModeSelected",
              StaticPayload({ mode: state.surfaceMode === "khala" ? "openagents" : "khala" }),
            ),
            style: mobileInteractiveStyle(state.accessibility),
          }),
          Spacer({ key: "home-toolbar-space", size: "1", style: { flex: 1 } }),
          IconButton({
            key: "home-new-chat",
            icon: "Compose",
            accessibilityLabel: "New chat",
            onPress: IntentRef("NewChatPressed", StaticPayload({})),
            style: mobileInteractiveStyle(state.accessibility),
          }),
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

export const renderDrawerView = (state: HomeState): View =>
  Stack(
    { key: "drawer-root", direction: "column", gap: "2", padding: "4", style: { width: "full", height: "full", backgroundColor: "surface" } },
    [
      Spacer({ key: "drawer-top-space", size: "10" }),
      drawerRow({ key: "drawer-new-chat", label: "New chat", onPress: IntentRef("NewChatPressed", StaticPayload({})), selected: state.surfaceMode === "khala" && state.khala.entries.length === 0 }, state.accessibility),
      drawerRow({ key: "drawer-khala", label: state.conversationAuthority === "sync" ? "OpenAgents" : "Khala", onPress: IntentRef("SurfaceModeSelected", StaticPayload({ mode: "khala" })), selected: state.surfaceMode === "khala" }, state.accessibility),
      ...(state.codingDirectory?.authority === "confirmed" && state.codingDirectory.sessions.length > 0
        ? [
            Text({
              key: "drawer-coding-title",
              content: "Coding sessions",
              variant: "caption",
              color: "textMuted",
            }),
            ...state.codingDirectory.repositories.flatMap(repository => [
              Text({
                key: `drawer-coding-repository-${repository.repositoryRef}`,
                content: `${repository.displayName} · ${repository.sessionCount} ${repository.sessionCount === 1 ? "session" : "sessions"}`,
                variant: "body",
                color: "textPrimary",
              }),
              ...state.codingDirectory!.sessions
                .filter(session => session.repositoryRef === repository.repositoryRef)
                .map(session => drawerRow({
                  key: `drawer-coding-session-${session.sessionRef}`,
                  label: codingSessionStateLabel(session.state),
                  onPress: IntentRef("CodingSessionSelected", StaticPayload({
                    repositoryRef: session.repositoryRef,
                    sessionRef: session.sessionRef,
                    threadRef: session.threadRef,
                  })),
                  selected: state.activeThreadRef === session.threadRef,
                }, state.accessibility)),
            ]),
          ]
        : []),
      ...state.conversationThreads.map(thread => drawerRow({
        key: `drawer-thread-${thread.threadRef}`,
        label: thread.title,
        onPress: IntentRef("ConversationThreadSelected", StaticPayload({ threadRef: thread.threadRef })),
        selected: state.activeThreadRef === thread.threadRef,
      }, state.accessibility)),
      ...codingOfflineCacheAccountingRows(state),
      ...(state.fleetRuns?.runs ?? []).flatMap(run => [
        Text({
          key: `fleet-run-${run.runRef}`,
          content: `Fleet run · ${run.executionState}\n${run.runRef}`,
          variant: "caption",
          color: "textPrimary",
        }),
        ...run.attempts.map(attempt => Text({
          key: `fleet-attempt-${attempt.workClaimRef}`,
          content: `${attempt.requestedTarget} → ${attempt.selectedTarget} · ${attempt.outcome}\n${attempt.workClaimRef}\n${attempt.assignmentRef ?? "Assignment pending"}\n${attempt.closeoutRef ?? "Closeout pending"}`,
          variant: "caption",
          color: "textMuted",
        })),
      ]),
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
    prepareComposerSubmission?: (
      session: MobileCodingComposerSession,
      message: string,
    ) => Promise<MobileCodingAttachmentDeliveryResult>
    clearComposer?: (
      session: MobileCodingComposerSession,
    ) => Promise<MobileCodingComposerSession | null>
  }>
}

const confirmedKhalaState = (
  thread: MobileConversationThread | null,
  turnCounter = 0,
  interactionActionsAvailable = false,
  runtimeControlActionsAvailable = false,
  previousGraphView: Readonly<{
    agentGraphExpanded: boolean
    selectedAgentRef: string | null
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
  const runtimeEntries = (thread?.timeline?.events ?? []).flatMap<KhalaState["entries"][number]>(event => {
    const item = event.item
    if (item == null) return []
    switch (item.kind) {
      case "text":
        return [{ key: event.eventRef, role: "assistant" as const, text: item.text, status: "done" as const, createdAt: event.createdAt, version: event.version }]
      case "reasoning":
        return [{ key: event.eventRef, role: "system" as const, text: `Reasoning · ${item.text}`, status: "done" as const, createdAt: event.createdAt, version: event.version }]
      case "connected":
        return [{ key: event.eventRef, role: "system" as const, text: `Connected · ${item.lane}`, status: "done" as const, createdAt: event.createdAt, version: event.version }]
      case "tool":
        return [{ key: event.eventRef, role: "system" as const, text: `${item.toolName} · ${item.status}`, status: item.status === "failed" ? "failed" as const : "done" as const, createdAt: event.createdAt, version: event.version }]
      case "plan":
        return [{
          key: event.eventRef, role: "system" as const, text: `Plan · ${item.status}`,
          status: "done" as const, createdAt: event.createdAt, version: event.version,
          ...(item.interactionRef === undefined || item.prompt === undefined ||
            (item.status !== "pending" && item.status !== "resolved" && item.status !== "expired" && item.status !== "revoked")
            ? {}
            : { interaction: { kind: "plan_review" as const, interactionRef: item.interactionRef, turnRef: event.runRef, status: item.status, title: "Review plan", prompt: item.prompt, questions: [], ...(item.decisionRef === undefined ? {} : { decisionRef: item.decisionRef }) } }),
        }]
      case "usage":
        return [{ key: event.eventRef, role: "system" as const, text: `Usage · ${item.totalTokens ?? 0} tokens`, status: "done" as const, createdAt: event.createdAt, version: event.version }]
      case "terminal":
        return [{ key: event.eventRef, role: "system" as const, text: `Turn ${item.status}`, status: item.status === "failed" ? "failed" as const : "done" as const, createdAt: event.createdAt, version: event.version }]
      case "interrupted":
        return [{ key: event.eventRef, role: "system" as const, text: "Turn interrupted", status: "done" as const, createdAt: event.createdAt, version: event.version }]
      case "heartbeat":
      case "reconnect":
      case "stale":
        return [{ key: event.eventRef, role: "system" as const, text: item.detail, status: "done" as const, createdAt: event.createdAt, version: event.version }]
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
      case "error":
        return [{ key: event.eventRef, role: "system" as const, text: item.messageSafe, status: "failed" as const, createdAt: event.createdAt, version: event.version }]
    }
  })
  const messageEntries = (thread?.messages ?? []).map(message => ({
    key: message.messageRef,
    role: "user" as const,
    text: message.body,
    status: "done" as const,
    createdAt: message.createdAt,
    version: message.version,
  }))
  return {
    draft: "",
    entries: [...messageEntries, ...runtimeEntries].sort((left, right) =>
      (left.createdAt ?? "").localeCompare(right.createdAt ?? "")),
    // A confirmed running turn is observable state, not an in-flight mobile
    // mutation. Keep the composer available so a second device can safely
    // append to that exact run. Only pre-dispatch queued state blocks input.
    pending: thread?.timeline?.run?.status === "queued",
    turnCounter,
    interactionSelections: {},
    interactionSubmittingRef: null,
    interactionActionsAvailable,
    runtimeTurn: thread?.timeline?.run === null || thread?.timeline?.run === undefined
      ? null
      : {
          runRef: thread.timeline.run.runRef,
          status: thread.timeline.run.status,
        },
    runtimeControlSubmittingAction: null,
    runtimeControlActionsAvailable,
    agentGraph,
    agentGraphExpanded,
    selectedAgentRef,
  }
}

const withConfirmedThread = (
  state: HomeState,
  thread: MobileConversationThread,
): HomeState => ({
  ...state,
  drawerOpen: false,
  surfaceMode: "khala",
  activeThreadRef: thread.threadRef,
  conversationThreads: [
    {
      threadRef: thread.threadRef,
      title: thread.title,
      messageCount: thread.messageCount,
      lastMessageAt: thread.lastMessageAt,
      updatedAt: thread.updatedAt,
      version: thread.version,
    },
    ...state.conversationThreads.filter(item => item.threadRef !== thread.threadRef),
  ],
  khala: {
    ...confirmedKhalaState(
      thread,
      state.khala.turnCounter,
      state.khala.interactionActionsAvailable,
      state.khala.runtimeControlActionsAvailable,
      state.activeThreadRef === thread.threadRef
        ? {
            agentGraphExpanded: state.khala.agentGraphExpanded,
            selectedAgentRef: state.khala.selectedAgentRef,
          }
        : null,
    ),
    draft: state.khala.draft,
  },
})

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
) => ({
  NewChatPressed: () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    if (before.khala.pending) return
    if (coding !== undefined) yield* Effect.promise(coding.clearSelection)
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      drawerOpen: false,
      surfaceMode: "khala" as const,
      codingComposer: null,
      codingAttachmentPicking: false,
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
    yield* SubscriptionRef.update(state, current => result.ok
      ? withConfirmedThread(current, result.thread)
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
      codingAttachmentPicking: false,
      codingAttachmentStatus: null,
      khala: { ...current.khala, pending: true },
    }))
    const thread = yield* Effect.promise(() => host.openThread(payload.threadRef))
    yield* SubscriptionRef.update(state, current => thread === null
      ? failedConversationState(current, "Conversation is still pending reconciliation.")
      : withConfirmedThread(current, thread))
  }),
  KhalaDraftChanged: (text: string) => Effect.gen(function* () {
    const bounded = text.length > 4_000 ? `${text.slice(0, 4_000)}…` : text
    const before = yield* SubscriptionRef.get(state)
    const composer = before.codingComposer
    if (composer === null || coding === undefined) {
      yield* SubscriptionRef.update(state, current => ({
        ...current,
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
      ? { ...withConfirmedThread(current, result.thread), codingComposer: settledComposer }
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

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
  options: HomeProgramOptions = {},
): IntentHandlers<typeof homeIntentDefinitions> => {
  const synced = options.conversation === undefined
    ? undefined
    : makeSyncedConversationHandlers(state, options.conversation.host, options.coding)
  return {
    DrawerToggled: () => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: !current.drawerOpen })),
    NewChatPressed: synced?.NewChatPressed ??
      (() => SubscriptionRef.update(state, (current) => ({
        ...current,
        drawerOpen: false,
        surfaceMode: "khala" as const,
        codingComposer: null,
        codingAttachmentPicking: false,
        codingAttachmentStatus: null,
        khala: initialKhalaState,
      }))),
    CodingComposerAttachmentsRequested: options.coding === undefined
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
            if (current.codingComposer?.draft.draftRef !== composer.draft.draftRef) {
              return current
            }
            switch (result.status) {
              case "cancelled":
                return { ...current, codingAttachmentPicking: false }
              case "failed":
                return {
                  ...current,
                  codingAttachmentPicking: false,
                  codingAttachmentStatus: {
                    kind: "failed" as const,
                    message: result.error,
                  },
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
        }),
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
              : { ...current, codingComposer: updated })
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
      ? Effect.void
      : Effect.promise(options.sessionActions.signOut),
    SurfaceModeSelected: (payload) => SubscriptionRef.update(state, (current) => ({ ...current, drawerOpen: false, surfaceMode: payload.mode as SurfaceMode })),
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
    ConversationThreadSelected: synced?.ConversationThreadSelected ?? (() => Effect.void),
    RuntimeInteractionOptionToggled: synced?.RuntimeInteractionOptionToggled ?? (() => Effect.void),
    RuntimeInteractionDecisionSubmitted: synced?.RuntimeInteractionDecisionSubmitted ?? (() => Effect.void),
    RuntimeTurnControlRequested: synced?.RuntimeTurnControlRequested ?? (() => Effect.void),
    CodingSessionSelected: options.coding === undefined
      ? () => Effect.void
      : payload => Effect.gen(function* () {
          const before = yield* SubscriptionRef.get(state)
          if (before.khala.pending) return
          yield* SubscriptionRef.update(state, current => ({
            ...current,
            drawerOpen: false,
            codingAttachmentPicking: false,
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
                codingAttachmentPicking: false,
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
    readonly toggleAgentStack: () => void
    readonly selectAgentRow: (agentRef: string) => void
  }
  readonly sync: {
    readonly setPhase: (phase: MobileSyncPhase) => void
  }
  readonly accessibility: {
    readonly setProfile: (profile: MobileAccessibilityProfile) => void
  }
  readonly coding: {
    readonly selectSession: (target: MobileCodingTarget) => void
    readonly pickAttachments: () => void
    readonly selectTarget: (targetId: string) => void
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
      const registry = yield* makeIntentRegistry(homeIntentDefinitions, makeHomeHandlers(state, options))
      const report: IntentReporter = (ref, runtimeValue) => registry.dispatch(resolveIntentRef(ref, runtimeValue))
      const fireRef = (ref: ReturnType<typeof IntentRef>): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(ref))))
      }
      const fireText = (ref: ReturnType<typeof IntentRef>, value: string): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(ref, value))))
      }
      const fire = (name: string) => (): void => fireRef(IntentRef(name, StaticPayload({})))
      const submitKhala = (text: string): void => fireText(IntentRef("KhalaTurnSubmitted", ComponentValueBinding()), text)
      return {
        initialState: programInitialState,
        viewStream: makeViewProgramFromState(state, renderHomeView).viewStream,
        contentViewStream: makeViewProgramFromState(state, renderContentView).viewStream,
        drawerViewStream: makeViewProgramFromState(state, renderDrawerView).viewStream,
        report,
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
          toggleAgentStack: fire(AgentStackToggled),
          selectAgentRow: agentRef => fireRef(IntentRef(
            AgentRowSelected,
            StaticPayload({ agentRef }),
          )),
        },
        sync: {
          setPhase: phase => {
            Effect.runFork(SubscriptionRef.update(state, current =>
              current.conversationAuthority === "sync" && phase !== "live" && phase !== "catching_up"
                ? {
                    ...current,
                    syncPhase: phase,
                    conversationThreads: [],
                    activeThreadRef: null,
                    codingComposer: null,
                    codingAttachmentPicking: false,
                    codingAttachmentStatus: null,
                    khala: initialKhalaState,
                  }
                : { ...current, syncPhase: phase }))
          },
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
          selectTarget: targetId => fireRef(IntentRef(
            "CodingExecutionTargetSelected",
            StaticPayload({ targetId }),
          )),
        },
        session: {
          signIn: fire("OpenAgentsSignInPressed"),
          signOut: fire("OpenAgentsSignOutPressed"),
        },
      }
    }),
  )
