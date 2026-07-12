/**
 * OpenAgents Desktop shell (#8574).
 *
 * The whole screen is typed Effect Native data: state, intents, and a pure
 * `state -> View` projection over the shared vendored catalog
 * (`@effect-native/core`, catalog v29). No React, no local UI primitives —
 * catalog gaps go to `docs/effect-native/DEMAND_REGISTER.md` (D-DESK-01),
 * never local code.
 *
 * Chat chrome rides the real v29 message contract (effect-native#72, adapted
 * from the Khala Code desktop transcript + Khala mobile chat stories): sender
 * labels and timestamps are typed `TranscriptMessage` data drawn by the
 * renderer in a meta row separated from the body — never text concatenated
 * into the body — with role-differentiated rows (user end-aligned bubbles,
 * system muted prose). The composer is a `TextField` with contract-level
 * `clearOnSubmit` plus a `pending` state binding that disables it while a
 * submission is in flight.
 */
import {
  Badge,
  BackgroundGradient,
  Button,
  Card,
  ComponentValueBinding,
  IconButton,
  Image,
  IntentRef,
  Icon,
  NavRail,
  Spacer,
  SplitPane,
  Stack,
  Table,
  Text,
  TextField,
  Tooltip,
  Transcript,
  StaticPayload,
  defineIntent,
  type IntentHandlers,
  type TextView,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"
import type { DesktopMessageMeta, DesktopQuestionCard, DesktopRuntimeCard, DesktopThread } from "../chat-contract.ts"
import {
  contextGroupSummary,
  humanizeToolInvocation,
  projectTranscriptEntries,
  toolCardIcon,
  toolResultSnippet,
  type ContextGroupModel,
  type ToolCardModel,
} from "./tool-cards.ts"
import {
  childInterruptable,
  childSteerLine,
  childStatusChip,
  planProgressSummary,
  planStatusGlyph,
  type RuntimeChildCardPayload,
  type RuntimePlanCardPayload,
  type RuntimeQueueChipPayload,
} from "./runtime-cards.ts"
import {
  resolveLiveAgentGraphSelection,
  type LiveAgentGraphPresentation,
} from "../agent-graph-presentation.ts"
import { chatMarkdownBody } from "./markdown.ts"
import { runtimeAgentGraphView } from "./runtime-agent-graph.ts"
import { desktopCommandRegistry, formatCommandChord } from "./command-registry.ts"
import {
  normalizeDesktopCommandChord,
  type DesktopCommandBindingProjection,
  type DesktopCommandId,
} from "../desktop-command-contract.ts"
import {
  emptyFleetWorkspaceState,
  fleetWorkspaceIntents,
  fleetWorkspaceView,
  makeFleetWorkspaceHandlers,
  refreshFleetAccounts,
  unavailableFleetAccountsBridge,
  type FleetAccountsBridge,
  type FleetWorkspaceState,
} from "./fleet-workspace.ts"
import {
  emptyGitPanelState,
  gitPanelIntents,
  gitPanelView,
  makeGitPanelHandlers,
  refreshGitPanel,
  unavailableGitGithubBridge,
  type GitGithubBridge,
  type GitPanelState,
} from "./git-panel.ts"
import { sidebarAccountsView } from "./sidebar-accounts.ts"
import {
  emptyWorkspaceBrowserState,
  makeWorkspaceBrowserHandlers,
  unavailableWorkspaceBrowserBridge,
  workspaceBrowserIntents,
  workspaceBrowserView,
  type WorkspaceBrowserBridge,
  type WorkspaceBrowserState,
} from "./workspace-browser.ts"
import {
  emptyWorkspaceEditorState,
  makeWorkspaceEditorHandlers,
  unavailableWorkspaceDocumentBridge,
  workspaceEditorIntents,
  workspaceEditorRecoverySnapshot,
  workspaceEditorView,
  withWorkspaceEditorRenamed,
  type WorkspaceDocumentBridge,
  type WorkspaceEditorRecoverySnapshot,
  type WorkspaceEditorState,
} from "./workspace-editor.ts"
import { emptyHistoryWorkspaceState, historyCatalogPageSize, historyItemPageOffset, historyItemPageSize, historySearchActive, historySearchField, historySearchResultSidebarItems, historySourceBadgeLabel, historyTailOffset, historyWorkspaceIntents, historyWorkspaceView, mergeHistoryWindowDown, mergeHistoryWindowUp, type HistoryWorkspaceState } from "./history-workspace.ts"
import type { CodexHistoryCatalog, CodexHistoryPage, CodexHistorySearchResponse } from "../codex-history-contract.ts"
import {
  emptyDesktopCodingCatalogProjection,
  desktopWorkspaceForCodingFocus,
  filterDesktopCodingCatalog,
  parseDesktopCodingCatalogQuery,
  type DesktopCodingCatalogProjection,
} from "../coding-catalog-contract.ts"

import {
  initialSettingsState,
  makeSettingsHandlers,
  settingsIntents,
  settingsView,
  unavailableCodexSettingsBridge,
  unavailableMcpConfigSettingsBridge,
  unavailableOpenAgentsSessionSettingsBridge,
  unavailableProviderAccountsSettingsBridge,
  type CodexSettingsBridge,
  type McpConfigSettingsBridge,
  type OpenAgentsSessionSettingsBridge,
  type ProviderAccountsSettingsBridge,
  type SettingsState,
} from "./settings.ts"

import {
  addComposerImage,
  canAttachMoreImages,
  composerImageDataUrl,
  formatImageSize,
  removeComposerImage,
  toStartImages,
  type ComposerImageAttachment,
} from "./composer-images.ts"
import type { FableLocalImageAttachment } from "../fable-local-contract.ts"

export type DesktopNoteEntry = Readonly<{
  key: string
  role: "user" | "assistant" | "system"
  text: string
  /** Preformatted display timestamp (the catalog ships no date formatting). */
  timestamp: string
  /** Host-observed message metadata (#8712) — see DesktopMessageMetaSchema. */
  meta?: DesktopMessageMeta
  /** Present only on interactive question notes (EP250 question cards). */
  question?: DesktopQuestionCard
  /** Present only on runtime-capability cards (EP250 wave-2: plan/child/queue). */
  runtime?: DesktopRuntimeCard
}>

/**
 * One answered question in the FROZEN bridge shape: the question text exactly
 * as question_pending carried it plus the selected option labels. Labels stay
 * an array even for single-select; the runtime comma-joins for multiSelect.
 */
export type QuestionAnswer = Readonly<{
  question: string
  labels: ReadonlyArray<string>
}>

/** Local (renderer-only) interaction state for one pending question card. */
export type QuestionCardInteraction = Readonly<{
  /** Selected option labels per question index. */
  selections: ReadonlyArray<ReadonlyArray<string>>
  /** True once the answer was handed to the typed bridge. */
  answered: boolean
  /** The submitted answers (for the collapsed answered rendering). */
  answers: ReadonlyArray<QuestionAnswer> | null
}>

export const desktopWorkspaceNames = ["fleet", "chat", "home", "files", "review", "terminal", "inbox", "settings"] as const
export type DesktopWorkspaceName = (typeof desktopWorkspaceNames)[number]
export const codingSessionFilters = ["active", "recovery", "archived"] as const
export type CodingSessionFilter = (typeof codingSessionFilters)[number]

export const desktopHarnessNames = ["fable", "codex"] as const
export type DesktopHarnessName = (typeof desktopHarnessNames)[number]

/**
 * Evidence-gated composer affordances (#8712): a harness chip is enabled only
 * when its lane can actually run a turn. `reason` is the visible caption when
 * it cannot. A control that cannot act must not accept the action.
 */
export type HarnessLaneAvailability = Readonly<{
  available: boolean
  reason: string | null
}>
export type HarnessLanes = Readonly<{
  fable: HarnessLaneAvailability
  codex: HarnessLaneAvailability
}>

export type DesktopShellState = Readonly<{
  /** Host identity decoded from the preload bridge ("electron/darwin" etc.). */
  host: string
  input: string
  /** True while a submission is in flight; the composer disables itself. */
  pending: boolean
  /**
   * Pending composer image attachments (capability I1): bounded base64 held in
   * the renderer, shown as thumbnails with remove, threaded into the next
   * turn's start payload. Cleared on submit.
   */
  composerImages: ReadonlyArray<ComposerImageAttachment>
  /**
   * Transient honest rejection copy for the last attach attempt (oversize /
   * wrong type / count limit). Not a standing caption — cleared on the next
   * successful attach or submit.
   */
  composerImageNotice: string | null
  notes: ReadonlyArray<DesktopNoteEntry>
  /** Which coding harness new turns target; "codex" preserves prior behavior. */
  selectedHarness: DesktopHarnessName
  /** Probed lane availability; boot replaces this with real evidence pre-mount. */
  harnessLanes: HarnessLanes
  threads: ReadonlyArray<DesktopThread>
  activeThreadId: string | null
  /**
   * The transcript message whose metadata inspector is open (#8712: "if I
   * click on the message, I see the metadata of the message in the right
   * sidebar"). Null means no inspector.
   */
  selectedMessageKey: string | null
  /**
   * Tool cards whose raw bounded details are expanded (EP250 tool cards).
   * Keys are the started-note keys — stable across in-place status updates.
   */
  expandedToolCards: ReadonlyArray<string>
  /**
   * Local interaction state for pending question cards, keyed by questionRef
   * (EP250 question cards). Selections are per-question option-label arrays;
   * `answered` marks a locally submitted answer before question_resolved.
   */
  questionCards: Readonly<Record<string, QuestionCardInteraction>>
  /**
   * Evidence-gated answering: true only when the preload bridge actually
   * exposes fableLocal.answerQuestion. Absent bridge (runtime lane not
   * merged yet) renders question cards read-only pending.
   */
  questionAnswerHostAvailable: boolean
  /** Confirmed Runtime Gateway v8 graph presentation for the active thread. */
  agentGraph: LiveAgentGraphPresentation | null
  agentGraphExpanded: boolean
  selectedAgentRef: string | null
  codingCatalog: DesktopCodingCatalogProjection
  codingSessionFilter: CodingSessionFilter
  codingSessionQuery: string
  workspace: DesktopWorkspaceName
  /** Grant-scoped, root-relative Files workspace projection. */
  workspaceBrowser: WorkspaceBrowserState
  /** Effect Native document tabs and conflict-safe draft lifecycle. */
  workspaceEditor: WorkspaceEditorState
  commandPaletteOpen: boolean
  /** Public-safe result of the latest deferred/native command admission. */
  commandNotice: string | null
  commandBindings: DesktopCommandBindingProjection | null
  commandBindingSelectedId: DesktopCommandId | null
  commandBindingDraft: string
  /** True only while the platform command modifier is held. */
  historyShortcutHintsVisible: boolean
  /** The desktop-only planning deck; it has no deployment authority itself. */
  fleetDeskOpen: boolean
  /** The current, explicitly unsubmitted FleetRun objective draft. */
  fleetObjective: string
  /** Honest deployment posture: local UI cannot invent a FleetRun receipt. */
  fleetDeployment: "not_requested" | "dispatching" | "accepted" | "rejected" | "unavailable"
  /** Count of completed button -> intent -> state -> re-render round trips. */
  loopProofs: number
  /** Codex account reconnect state, shown by the "settings" workspace (see ./settings.ts). */
  settings: SettingsState
  history: HistoryWorkspaceState
  /** Read-only fleet accounts projection (see ./fleet-workspace.ts). */
  fleet: FleetWorkspaceState
  /** Typed Git/GitHub review panel (see ./git-panel.ts). */
  git: GitPanelState
}>

/** "18:04" — display-string timestamps for the typed message contract. */
export const formatShellTimestamp = (date: Date): string =>
  `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`

export const formatRelativeTimestamp = (updatedAt: string, now: Date = new Date()): string => {
  const elapsed = Math.max(0, now.getTime() - Date.parse(updatedAt))
  if (!Number.isFinite(elapsed) || elapsed < 60_000) return "now"
  if (elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)}m`
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)}h`
  return `${Math.floor(elapsed / 86_400_000)}d`
}

/**
 * The initial transcript is local presentation state only: it neither creates
 * a FleetRun nor impersonates a server-authorized turn.
 */
export const initialDesktopShellState = (
  host: string,
  timestamp: string = formatShellTimestamp(new Date()),
): DesktopShellState => ({
  host,
  input: "",
  pending: false,
  composerImages: [],
  composerImageNotice: null,
  notes: [],
  selectedHarness: "codex",
  // Unproven until boot's availability probe lands (before first mount):
  // an unproven lane is disabled, not optimistically enabled.
  harnessLanes: {
    fable: { available: false, reason: "Fable — checking local availability" },
    codex: { available: false, reason: "Codex — checking availability" },
  },
  threads: [],
  activeThreadId: null,
  selectedMessageKey: null,
  expandedToolCards: [],
  questionCards: {},
  questionAnswerHostAvailable: false,
  agentGraph: null,
  agentGraphExpanded: false,
  selectedAgentRef: null,
  codingCatalog: emptyDesktopCodingCatalogProjection(),
  codingSessionFilter: "active",
  codingSessionQuery: "",
  workspace: "chat",
  workspaceBrowser: emptyWorkspaceBrowserState(),
  workspaceEditor: emptyWorkspaceEditorState(),
  commandPaletteOpen: false,
  commandNotice: null,
  commandBindings: null,
  commandBindingSelectedId: null,
  commandBindingDraft: "",
  historyShortcutHintsVisible: false,
  fleetDeskOpen: false,
  fleetObjective: "",
  fleetDeployment: "not_requested",
  loopProofs: 0,
  settings: initialSettingsState(),
  history: emptyHistoryWorkspaceState(),
  fleet: emptyFleetWorkspaceState(),
  git: emptyGitPanelState(),
})

// ---------------------------------------------------------------------------
// Intents — typed end-to-end: DOM event -> IntentRef -> registry decode ->
// handler -> SubscriptionRef update -> viewStream re-render.
// ---------------------------------------------------------------------------

export const DesktopInputChanged = defineIntent("DesktopInputChanged", Schema.String)
/**
 * Null payload happens on button press (the renderer passes no component
 * value); the handler then falls back to the composer state.
 */
export const DesktopNoteSubmitted = defineIntent(
  "DesktopNoteSubmitted",
  Schema.NullOr(Schema.String),
)
/**
 * Interrupt the streaming turn (EP250 audit gap #9, "cheapest fix" — 240
 * interrupts observed). Fired by the composer Stop button while `pending`;
 * the handler dispatches the active local lane's already-plumbed interrupt IPC
 * path (FableLocal/CodexLocal interrupt channel). The terminal turn result
 * reverts the control to Send.
 */
export const DesktopTurnInterrupted = defineIntent("DesktopTurnInterrupted", Schema.Null)
/**
 * Interrupt a running delegate child (EP250 wave-2 G4). Fired by the Interrupt
 * control on a running child card; the handler signals the active local lane's
 * frozen steer-child channel by exact { turnRef, childRef } with action
 * "interrupt" (the only supported control — the SDK/codex cannot MESSAGE an
 * in-flight child). The runtime's typed child_steered event renders the
 * outcome; the handler invents none.
 */
export const DesktopChildInterruptRequested = defineIntent(
  "DesktopChildInterruptRequested",
  Schema.Struct({ turnRef: Schema.String, childRef: Schema.String }),
)
/**
 * Composer image attachment intents (capability I1). `DesktopComposerImageAdded`
 * carries an already-decoded bounded attachment (the drop/paste/picker handlers
 * decode the File in the renderer — never a filesystem read here);
 * `DesktopComposerImageRemoved` drops one by id; `DesktopComposerImagesRejected`
 * surfaces honest rejection copy; `DesktopComposerImagePickRequested` opens the
 * main-mediated native file picker.
 */
export const DesktopComposerImageAdded = defineIntent(
  "DesktopComposerImageAdded",
  Schema.Struct({
    id: Schema.String,
    mediaType: Schema.Literals(["image/png", "image/jpeg", "image/webp", "image/gif"]),
    data: Schema.String,
    name: Schema.String,
    sizeBytes: Schema.Number,
  }),
)
export const DesktopComposerImageRemoved = defineIntent("DesktopComposerImageRemoved", Schema.String)
export const DesktopComposerImagesRejected = defineIntent("DesktopComposerImagesRejected", Schema.String)
export const DesktopComposerImagePickRequested = defineIntent("DesktopComposerImagePickRequested", Schema.Null)
export const DesktopLoopPinged = defineIntent("DesktopLoopPinged", Schema.Null)
export const DesktopFleetDeskToggled = defineIntent("DesktopFleetDeskToggled", Schema.Null)
export const DesktopFleetObjectiveChanged = defineIntent(
  "DesktopFleetObjectiveChanged",
  Schema.String,
)
export const DesktopFleetDeploymentRequested = defineIntent(
  "DesktopFleetDeploymentRequested",
  Schema.Null,
)
export const DesktopNewChat = defineIntent("DesktopNewChat", Schema.Null)
export const DesktopHarnessSelected = defineIntent(
  "DesktopHarnessSelected",
  Schema.Literals(desktopHarnessNames),
)
export const DesktopChatSelected = defineIntent("DesktopChatSelected", Schema.String)
/**
 * Message metadata inspector selection (#8712). Payload is the transcript
 * message key; the empty string or re-selecting the open key deselects
 * (Escape and the Close affordance dispatch the empty payload).
 */
export const DesktopMessageSelected = defineIntent("DesktopMessageSelected", Schema.String)
/** Expand/collapse a tool card's bounded raw details (EP250 tool cards). */
export const DesktopToolCardToggled = defineIntent("DesktopToolCardToggled", Schema.String)
/**
 * Toggle window fullscreen (owner contract EP250: "add a hotkey for
 * maximizing (command+something) to fullscreen like command f").
 */
export const DesktopFullscreenToggled = defineIntent("DesktopFullscreenToggled", Schema.Null)
/**
 * Question card option activation (EP250 question cards). Single-select
 * questions record the label and auto-submit once every question in the card
 * has a selection; multiSelect questions toggle the label and wait for the
 * explicit confirm intent.
 */
export const DesktopQuestionOptionSelected = defineIntent(
  "DesktopQuestionOptionSelected",
  Schema.Struct({
    questionRef: Schema.String,
    questionIndex: Schema.Number,
    label: Schema.String,
  }),
)
export const DesktopQuestionSubmitted = defineIntent("DesktopQuestionSubmitted", Schema.String)
export const DesktopAgentGraphToggled = defineIntent("DesktopAgentGraphToggled", Schema.Null)
export const DesktopAgentAction = defineIntent("DesktopAgentAction", Schema.Struct({
  kind: Schema.Literals(["inspect_agent", "focus_agent"]),
  agentRef: Schema.String,
}))
export const DesktopCodingCatalogFilterSelected = defineIntent(
  "DesktopCodingCatalogFilterSelected",
  Schema.Literals(codingSessionFilters),
)
export const DesktopCodingCatalogQueryChanged = defineIntent("DesktopCodingCatalogQueryChanged", Schema.String)
export const DesktopCodingCatalogChooseRequested = defineIntent("DesktopCodingCatalogChooseRequested", Schema.Null)
export const DesktopCodingSessionOpened = defineIntent("DesktopCodingSessionOpened", Schema.String)
export const DesktopCodingSessionArchived = defineIntent("DesktopCodingSessionArchived", Schema.String)
export const DesktopCodingSessionRecovered = defineIntent("DesktopCodingSessionRecovered", Schema.String)
export const DesktopWorkspaceSelected = defineIntent(
  "DesktopWorkspaceSelected",
  Schema.Literals(desktopWorkspaceNames),
)
export const DesktopWorkspacePickerRequested = defineIntent("DesktopWorkspacePickerRequested", Schema.Null)
export const DesktopCommandPaletteToggled = defineIntent("DesktopCommandPaletteToggled", Schema.Null)
export const DesktopCommandPaletteDismissed = defineIntent("DesktopCommandPaletteDismissed", Schema.Null)
export const DesktopCommandBindingSelected = defineIntent("DesktopCommandBindingSelected", Schema.String)
export const DesktopCommandBindingDraftChanged = defineIntent("DesktopCommandBindingDraftChanged", Schema.String)
export const DesktopCommandBindingSaved = defineIntent("DesktopCommandBindingSaved", Schema.Null)
export const DesktopCommandBindingRemoved = defineIntent("DesktopCommandBindingRemoved", Schema.Null)
export const DesktopCommandBindingsReset = defineIntent("DesktopCommandBindingsReset", Schema.Null)
export const DesktopHistoryShortcutHintsChanged = defineIntent("DesktopHistoryShortcutHintsChanged", Schema.Boolean)
export const DesktopHistoryConversationPreviewed = defineIntent("DesktopHistoryConversationPreviewed", Schema.String)

export const desktopShellIntents = [
  DesktopInputChanged,
  DesktopNoteSubmitted,
  DesktopTurnInterrupted,
  DesktopChildInterruptRequested,
  DesktopComposerImageAdded,
  DesktopComposerImageRemoved,
  DesktopComposerImagesRejected,
  DesktopComposerImagePickRequested,
  DesktopLoopPinged,
  DesktopFleetDeskToggled,
  DesktopFleetObjectiveChanged,
  DesktopFleetDeploymentRequested,
  DesktopNewChat,
  DesktopHarnessSelected,
  DesktopChatSelected,
  DesktopMessageSelected,
  DesktopToolCardToggled,
  DesktopFullscreenToggled,
  DesktopQuestionOptionSelected,
  DesktopQuestionSubmitted,
  DesktopAgentGraphToggled,
  DesktopAgentAction,
  DesktopCodingCatalogFilterSelected,
  DesktopCodingCatalogQueryChanged,
  DesktopCodingCatalogChooseRequested,
  DesktopCodingSessionOpened,
  DesktopCodingSessionArchived,
  DesktopCodingSessionRecovered,
  DesktopWorkspaceSelected,
  DesktopWorkspacePickerRequested,
  DesktopCommandPaletteToggled,
  DesktopCommandPaletteDismissed,
  DesktopCommandBindingSelected,
  DesktopCommandBindingDraftChanged,
  DesktopCommandBindingSaved,
  DesktopCommandBindingRemoved,
  DesktopCommandBindingsReset,
  DesktopHistoryShortcutHintsChanged,
  DesktopHistoryConversationPreviewed,
  ...settingsIntents,
  ...historyWorkspaceIntents,
  ...fleetWorkspaceIntents,
  ...gitPanelIntents,
  ...workspaceBrowserIntents,
  ...workspaceEditorIntents,
] as const

export type CodexHistoryHost = Readonly<{
  catalog: () => Promise<CodexHistoryCatalog | null>
  page: (threadRef: string, offset: number, limit: number) => Promise<CodexHistoryPage | null>
  search?: (query: string, limit: number) => Promise<CodexHistorySearchResponse | null>
  save?: (value: Readonly<{ rootThreadRef: string; selectedThreadRef: string; offset: number; selectedItemRef: string | null; railCollapsed: boolean; anchorItemRef: string | null; expandedThreadRefs?:ReadonlyArray<string> }>) => void
}>

// ---------------------------------------------------------------------------
// Pure state transitions (unit-tested directly).
// ---------------------------------------------------------------------------

export const withInput = (state: DesktopShellState, input: string): DesktopShellState => ({
  ...state,
  input,
})

export const withPending = (state: DesktopShellState, pending: boolean): DesktopShellState => ({
  ...state,
  pending,
})

export const withFleetDesk = (state: DesktopShellState): DesktopShellState => ({
  ...state,
  fleetDeskOpen: !state.fleetDeskOpen,
})

export const withFleetObjective = (
  state: DesktopShellState,
  fleetObjective: string,
): DesktopShellState => ({ ...state, fleetObjective })

export const withNewChat = (state: DesktopShellState, thread: DesktopThread): DesktopShellState => ({
  ...state,
  input: "",
  pending: false,
  notes: thread.notes,
  threads: [thread, ...state.threads.filter((item) => item.id !== thread.id)].slice(0, 5),
  activeThreadId: thread.id,
  selectedMessageKey: null,
  expandedToolCards: [],
  questionCards: {},
  agentGraph: thread.agentGraph ?? null,
  agentGraphExpanded: thread.agentGraph !== undefined && (
    thread.agentGraph.totalCount <= 8 || thread.agentGraph.attentionCount > 0
  ),
  selectedAgentRef: thread.agentGraph === undefined
    ? null
    : resolveLiveAgentGraphSelection(thread.agentGraph, null),
  workspace: "chat",
  // New chat must land in a fresh empty transcript even when a historical
  // Codex page is loaded: the chat workspace renders the history page
  // whenever one is present, so clear the in-memory page (persisted
  // restoration prefs stay untouched — this is a view exit, not a deletion).
  history: {
    ...state.history,
    page: null,
    selectedItemRef: null,
    pendingThreadRef: null,
    expandedThreadRefs: [],
  },
  fleetDeskOpen: false,
  fleetObjective: "",
  fleetDeployment: "not_requested",
  commandPaletteOpen: false,
})

export const withChatSelected = (state: DesktopShellState, thread: DesktopThread): DesktopShellState => ({
  ...state,
  notes: thread.notes,
  activeThreadId: thread.id,
  questionCards: state.activeThreadId === thread.id
    ? pruneQuestionCards(state.questionCards, thread.notes)
    : {},
  // Keep an open inspector only while its message still exists in the
  // projected transcript (streaming keys are replaced at finalize).
  selectedMessageKey: state.activeThreadId === thread.id &&
    state.selectedMessageKey !== null &&
    thread.notes.some((note) => note.key === state.selectedMessageKey)
    ? state.selectedMessageKey
    : null,
  agentGraph: thread.agentGraph ?? null,
  agentGraphExpanded: thread.agentGraph === undefined
    ? false
    : state.activeThreadId === thread.id
      ? state.agentGraphExpanded || thread.agentGraph.attentionCount > 0
      : thread.agentGraph.totalCount <= 8 || thread.agentGraph.attentionCount > 0,
  selectedAgentRef: thread.agentGraph === undefined
    ? null
    : resolveLiveAgentGraphSelection(
        thread.agentGraph,
        state.activeThreadId === thread.id ? state.selectedAgentRef : null,
      ),
  fleetDeskOpen: false,
  workspace: "chat",
  commandPaletteOpen: false,
})

/** Toggle-style message inspector selection ("" or same key deselects). */
export const withMessageSelected = (
  state: DesktopShellState,
  key: string,
): DesktopShellState => {
  const selectedMessageKey = key === "" || state.selectedMessageKey === key ? null : key
  return selectedMessageKey === state.selectedMessageKey
    ? state
    : { ...state, selectedMessageKey }
}

/** Expand/collapse one tool card's bounded raw details. */
export const withToolCardToggled = (state: DesktopShellState, key: string): DesktopShellState => ({
  ...state,
  expandedToolCards: state.expandedToolCards.includes(key)
    ? state.expandedToolCards.filter((item) => item !== key)
    : [...state.expandedToolCards, key],
})

/** The question note carrying the given questionRef, if still projected. */
export const questionNoteFor = (
  state: DesktopShellState,
  questionRef: string,
): DesktopNoteEntry | undefined =>
  state.notes.find((note) => note.question?.questionRef === questionRef)

/** Drop interaction state for question cards no longer in the transcript. */
export const pruneQuestionCards = (
  cards: Readonly<Record<string, QuestionCardInteraction>>,
  notes: ReadonlyArray<DesktopNoteEntry>,
): Readonly<Record<string, QuestionCardInteraction>> => {
  const live = new Set(
    notes.flatMap((note) => (note.question === undefined ? [] : [note.question.questionRef])),
  )
  const entries = Object.entries(cards).filter(([ref]) => live.has(ref))
  return entries.length === Object.keys(cards).length ? cards : Object.fromEntries(entries)
}

/**
 * Records an option activation. Single-select questions replace the
 * selection; multiSelect questions toggle the label. No-ops on resolved,
 * locally answered, or unknown cards.
 */
export const withQuestionSelection = (
  state: DesktopShellState,
  questionRef: string,
  questionIndex: number,
  label: string,
): DesktopShellState => {
  const note = questionNoteFor(state, questionRef)
  const card = note?.question
  if (card === undefined || card.status !== "pending") return state
  const question = card.questions[questionIndex]
  if (question === undefined || !question.options.some((option) => option.label === label)) {
    return state
  }
  const interaction = state.questionCards[questionRef] ?? {
    selections: card.questions.map(() => []),
    answered: false,
    answers: null,
  }
  if (interaction.answered) return state
  const current = interaction.selections[questionIndex] ?? []
  const selection = question.multiSelect
    ? current.includes(label) ? current.filter((item) => item !== label) : [...current, label]
    : [label]
  const selections = card.questions.map((_, index) =>
    index === questionIndex ? selection : interaction.selections[index] ?? [])
  return {
    ...state,
    questionCards: {
      ...state.questionCards,
      [questionRef]: { ...interaction, selections },
    },
  }
}

/** True once every question in the card has at least one selected option. */
export const questionAnswersReady = (
  card: DesktopQuestionCard,
  interaction: QuestionCardInteraction,
): boolean =>
  card.questions.every((_, index) => (interaction.selections[index]?.length ?? 0) >= 1)

/**
 * Answers in the FROZEN bridge shape: one `{ question, labels }` entry per
 * question — the question text exactly as question_pending carried it, and
 * the selected labels as an array even for single-select (the runtime
 * comma-joins multiSelect labels, the SDK's documented encoding).
 */
export const questionAnswersFor = (
  card: DesktopQuestionCard,
  interaction: QuestionCardInteraction,
): ReadonlyArray<QuestionAnswer> =>
  card.questions.map((question, index) => ({
    question: question.question,
    labels: interaction.selections[index] ?? [],
  }))

export const withQuestionAnswered = (
  state: DesktopShellState,
  questionRef: string,
  answers: ReadonlyArray<QuestionAnswer>,
): DesktopShellState => {
  const interaction = state.questionCards[questionRef]
  if (interaction === undefined || interaction.answered) return state
  return {
    ...state,
    questionCards: {
      ...state.questionCards,
      [questionRef]: { ...interaction, answered: true, answers },
    },
  }
}

/**
 * The typed bridge rejected the answer (`false`): the card returns to
 * pending, selections retained, so the user can retry or the runtime's
 * question_resolved outcome can land.
 */
export const withQuestionAnswerRejected = (
  state: DesktopShellState,
  questionRef: string,
): DesktopShellState => {
  const interaction = state.questionCards[questionRef]
  if (interaction === undefined || !interaction.answered) return state
  return {
    ...state,
    questionCards: {
      ...state.questionCards,
      [questionRef]: { ...interaction, answered: false, answers: null },
    },
  }
}

export const withWorkspace = (
  state: DesktopShellState,
  workspace: DesktopWorkspaceName,
): DesktopShellState => ({ ...state, workspace, commandPaletteOpen: false })

export const withCommandPalette = (
  state: DesktopShellState,
  commandPaletteOpen: boolean,
): DesktopShellState => ({ ...state, commandPaletteOpen })

/**
 * Submit resets the composer value binding in the same transition that
 * appends the note — the state contract half of clear-on-submit. The renderer
 * half is the v29 `clearOnSubmit` TextField contract (effect-native#72).
 */
export const withNote = (
  state: DesktopShellState,
  text: string,
  timestamp: string,
): DesktopShellState => {
  const trimmed = text.trim()
  // Capability I1: an images-only turn (empty text, ≥1 attachment) is valid;
  // an empty turn with no images is a no-op. The persisted user-note text
  // falls back to a bounded count label so the row is never blank.
  const hasImages = state.composerImages.length > 0
  if (trimmed === "" && !hasImages) return state
  const noteText = trimmed !== ""
    ? trimmed
    : state.composerImages.length === 1
      ? "(1 image attached)"
      : `(${state.composerImages.length} images attached)`
  return {
    ...state,
    input: "",
    pending: true,
    composerImages: [],
    composerImageNotice: null,
    notes: [
      ...state.notes,
      { key: `pending-${state.notes.length}`, role: "user", text: noteText, timestamp },
    ],
  }
}

/** Add a decoded attachment (capability I1); bounded, clears the notice. */
export const withComposerImageAdded = (
  state: DesktopShellState,
  attachment: ComposerImageAttachment,
): DesktopShellState => ({
  ...state,
  composerImages: addComposerImage(state.composerImages, attachment),
  composerImageNotice: null,
})

export const withComposerImageRemoved = (
  state: DesktopShellState,
  id: string,
): DesktopShellState => ({
  ...state,
  composerImages: removeComposerImage(state.composerImages, id),
})

export const withComposerImageNotice = (
  state: DesktopShellState,
  notice: string | null,
): DesktopShellState => ({ ...state, composerImageNotice: notice })

/**
 * Applies probed lane evidence. If the currently selected lane just became
 * unavailable while the other lane can act, selection moves to the available
 * lane — the composer must never park the user on a dead default.
 */
export const withHarnessLanes = (
  state: DesktopShellState,
  harnessLanes: HarnessLanes,
): DesktopShellState => {
  const selected = state.selectedHarness
  const other: DesktopHarnessName = selected === "fable" ? "codex" : "fable"
  const selectedHarness =
    !harnessLanes[selected].available && harnessLanes[other].available ? other : selected
  return { ...state, harnessLanes, selectedHarness }
}

export type ChatHost = Readonly<{
  listThreads: () => Promise<ReadonlyArray<DesktopThread>>
  newThread: () => Promise<DesktopThread | null>
  openThread: (id: string) => Promise<DesktopThread | null>
  hydrateThread?: (id: string) => Promise<DesktopThread | null>
  sendMessage: (input: Readonly<{
    id: string
    message: string
    harness?: DesktopHarnessName
    /** Optional image attachments threaded into the turn payload (capability I1). */
    images?: ReadonlyArray<FableLocalImageAttachment>
    onUpdate?: (thread: DesktopThread) => void
  }>) => Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>>
  /**
   * Interrupt the currently-streaming turn (EP250 Stop button). Resolves true
   * when an active turn was signalled through the lane's interrupt IPC path,
   * false when no turn is active or the host cannot interrupt. Optional: a host
   * without a local streaming lane (e.g. the read-only runtime adapter) omits
   * it and the Stop intent no-ops.
   */
  interruptActive?: () => Promise<boolean>
  /**
   * Interrupt a running delegate child of the active turn (EP250 wave-2 G4).
   * Resolves the runtime's typed outcome; a host with no active local lane
   * returns not_found. Only `interrupt` is offered (mid-flight message is
   * capability-unsupported). Optional: a host without a local streaming lane
   * omits it and the child-interrupt intent no-ops.
   */
  steerChild?: (input: Readonly<{ turnRef: string; childRef: string }>) => Promise<
    Readonly<{ ok: boolean; outcome: string }>
  >
  /**
   * Enqueue a follow-up while a turn streams (EP250 wave-2 A3), delivered at
   * the current turn's completion (queue-until-idle). Resolves the runtime's
   * typed queue outcome. Optional: a host that cannot queue (e.g. the runtime
   * adapter) omits it and a mid-turn submit is a no-op that keeps the draft.
   */
  queueFollowup?: (input: Readonly<{ threadRef: string; message: string }>) => Promise<
    Readonly<{ ok: boolean; queued: boolean }>
  >
}>

/**
 * Typed question-answer bridge (EP250 question cards). `answer` is null when
 * the preload surface has no fableLocal.answerQuestion (defensive: cards then
 * render read-only pending). The input mirrors the FROZEN
 * FableLocalAnswerQuestionRequest shape.
 */
export type QuestionHost = Readonly<{
  answer:
    | ((input: Readonly<{
        turnRef: string
        threadRef?: string
        questionRef: string
        answers: ReadonlyArray<QuestionAnswer>
      }>) => Promise<unknown>)
    | null
}>

/**
 * Main-mediated image file picker (capability I1). The attach affordance
 * dispatches through this to open the native dialog in main; main reads the
 * files (never the renderer) and returns decoded base64 attachments. An absent
 * host resolves to no attachments (drop/paste still work in-renderer).
 */
export type ComposerImagePickerHost = Readonly<{
  pick: () => Promise<ReadonlyArray<FableLocalImageAttachment>>
}>

export type WorkspaceHost = Readonly<{
  /** Opens the native picker; true means a new WorkContext is installed. */
  choose: () => Promise<unknown>
  browser?: WorkspaceBrowserBridge
  documents?: WorkspaceDocumentBridge
  recovery?: Readonly<{
    load: (workspaceSessionRef: string) => WorkspaceEditorRecoverySnapshot | null
    save?: (workspaceSessionRef: string, snapshot: WorkspaceEditorRecoverySnapshot) => void
  }>
}>

export type CodingCatalogHost = Readonly<{
  snapshot: () => Promise<DesktopCodingCatalogProjection>
  choose: () => Promise<DesktopCodingCatalogProjection>
  open: (sessionRef: string) => Promise<DesktopCodingCatalogProjection>
  archive: (sessionRef: string) => Promise<DesktopCodingCatalogProjection>
  recover: (sessionRef: string) => Promise<DesktopCodingCatalogProjection>
}>

const unavailableCodingCatalogHost: CodingCatalogHost = {
  snapshot: async () => emptyDesktopCodingCatalogProjection(),
  choose: async () => emptyDesktopCodingCatalogProjection(),
  open: async () => emptyDesktopCodingCatalogProjection(),
  archive: async () => emptyDesktopCodingCatalogProjection(),
  recover: async () => emptyDesktopCodingCatalogProjection(),
}

export type CommandBindingHost = Readonly<{
  snapshot: () => Promise<DesktopCommandBindingProjection | null>
  save: (input: Readonly<{ commandId: DesktopCommandId; chord: string | null }>) => Promise<DesktopCommandBindingProjection | null>
  reset: () => Promise<DesktopCommandBindingProjection | null>
}>

const unavailableCommandBindingHost: CommandBindingHost = {
  snapshot: async () => null,
  save: async () => null,
  reset: async () => null,
}

export const withThreads = (state: DesktopShellState, threads: ReadonlyArray<DesktopThread>): DesktopShellState => {
  const active = state.activeThreadId === null ? threads[0] : threads.find((thread) => thread.id === state.activeThreadId)
  return active === undefined
    ? { ...state, threads: threads.slice(0, 5) }
    : {
        ...state,
        threads: threads.slice(0, 5),
        activeThreadId: active.id,
        notes: active.notes,
        agentGraph: active.agentGraph ?? null,
        agentGraphExpanded: active.agentGraph === undefined
          ? false
          : state.agentGraphExpanded || active.agentGraph.attentionCount > 0 || active.agentGraph.totalCount <= 8,
        selectedAgentRef: active.agentGraph === undefined
          ? null
          : resolveLiveAgentGraphSelection(active.agentGraph, state.selectedAgentRef),
      }
}

export const withTurnResult = (state: DesktopShellState, result: Awaited<ReturnType<ChatHost["sendMessage"]>>, timestamp: string): DesktopShellState => {
  if (result.ok && result.thread) return { ...withChatSelected(state, result.thread), pending: false, threads: [result.thread, ...state.threads.filter((thread) => thread.id !== result.thread!.id)].slice(0, 5) }
  const confirmedNotes = state.notes.filter((note) => !note.key.startsWith("pending-"))
  return { ...state, pending: false, notes: [...confirmedNotes, { key: `error-${state.notes.length}`, role: "system", text: result.error ?? "The model request failed.", timestamp }] }
}

/**
 * This transition deliberately stops at an authority boundary. A desktop
 * renderer can prepare a request but cannot report a runRef, claim, or launch
 * until the local Pylon/server authority has returned exact evidence.
 */
export const withFleetDeploymentRequested = (
  state: DesktopShellState,
): DesktopShellState => {
  const objective = state.fleetObjective.trim()
  if (objective === "" || state.fleetDeployment === "dispatching") return state
  return {
    ...state,
    fleetDeployment: "dispatching",
  }
}

export type FleetStageResult = Readonly<{
  state: "accepted" | "rejected" | "unavailable"
  message: string
  intentStatus: string | null
}>

export type FleetStage = (input: Readonly<{ objective: string }>) => Promise<FleetStageResult>

export const withFleetDeploymentResult = (
  state: DesktopShellState,
  result: FleetStageResult,
  timestamp: string,
): DesktopShellState => ({
  ...state,
  fleetDeployment: result.state,
  notes: [
    ...state.notes,
    {
      key: `note-${state.notes.length}`,
      role: "system",
      text: result.message,
      timestamp,
    },
  ],
})

export const withLoopProof = (state: DesktopShellState, timestamp: string): DesktopShellState => {
  const loopProofs = state.loopProofs + 1
  return {
    ...state,
    loopProofs,
    notes: [
      ...state.notes,
      {
        key: `note-${state.notes.length}`,
        role: "system",
        text: `Typed intent loop proof ${loopProofs}: button press dispatched DesktopLoopPinged through the intent registry and re-rendered this view.`,
        timestamp,
      },
    ],
  }
}

/** Host seam for window-level controls (fullscreen toggle). */
export type DesktopWindowHost = { readonly toggleFullScreen: () => Promise<boolean> }

export const makeDesktopShellHandlers = (
  state: SubscriptionRef.SubscriptionRef<DesktopShellState>,
  now: () => string = () => formatShellTimestamp(new Date()),
  stageFleet: FleetStage = async () => ({
    state: "unavailable",
    message: "Local Pylon control is unavailable. No fleet work was dispatched.",
    intentStatus: null,
  }),
  chat: ChatHost = {
    listThreads: async () => [], newThread: async () => null, openThread: async () => null,
    sendMessage: async () => ({ ok: false, error: "Desktop chat is unavailable." }),
  },
  workspaceHost: WorkspaceHost = {
    choose: async () => false,
  },
  codexBridge: CodexSettingsBridge = unavailableCodexSettingsBridge,
  settingsSleep?: (ms: number) => Promise<void>,
  openAgentsBridge: OpenAgentsSessionSettingsBridge = unavailableOpenAgentsSessionSettingsBridge,
  historyHost: CodexHistoryHost = { catalog: async () => null, page: async () => null },
  fleetBridge: FleetAccountsBridge = unavailableFleetAccountsBridge,
  providerAccountsBridge: ProviderAccountsSettingsBridge = unavailableProviderAccountsSettingsBridge,
  codingCatalogHost: CodingCatalogHost = unavailableCodingCatalogHost,
  questionHost: QuestionHost = { answer: null },
  commandBindingHost: CommandBindingHost = unavailableCommandBindingHost,
  windowHost: DesktopWindowHost = { toggleFullScreen: async () => false },
  gitBridge: GitGithubBridge = unavailableGitGithubBridge,
  mcpConfigBridge: McpConfigSettingsBridge = unavailableMcpConfigSettingsBridge,
  imagePickerHost: ComposerImagePickerHost = { pick: async () => [] },
): IntentHandlers<typeof desktopShellIntents> => {
  const settingsHandlers = makeSettingsHandlers(state, codexBridge, openAgentsBridge, settingsSleep, undefined, providerAccountsBridge, mcpConfigBridge)
  const workspaceBrowserHandlers = makeWorkspaceBrowserHandlers(
    state,
    workspaceHost.browser ?? unavailableWorkspaceBrowserBridge,
  )
  const persistWorkspaceRecovery = (current: DesktopShellState): void => {
    const workspaceSessionRef = current.codingCatalog.selectedSessionRef ??
      current.codingCatalog.sessions.find(session => session.state === "active")?.sessionRef ??
      current.codingCatalog.sessions[0]?.sessionRef ?? null
    if (workspaceSessionRef !== null) {
      workspaceHost.recovery?.save?.(workspaceSessionRef, workspaceEditorRecoverySnapshot(current.workspaceEditor))
    }
  }
  const workspaceEditorHandlers = makeWorkspaceEditorHandlers(
    state,
    workspaceHost.documents ?? unavailableWorkspaceDocumentBridge,
    persistWorkspaceRecovery,
  )
  const recoverWorkspaceEditor = Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    if (current.workspaceEditor.tabs.length > 0) return
    const workspaceSessionRef = current.codingCatalog.selectedSessionRef ??
      current.codingCatalog.sessions.find(session => session.state === "active")?.sessionRef ??
      current.codingCatalog.sessions[0]?.sessionRef ?? null
    const grantRef = current.workspaceBrowser.grantRef
    if (workspaceSessionRef === null || grantRef === null) return
    const snapshot = workspaceHost.recovery?.load(workspaceSessionRef) ?? null
    if (snapshot === null || snapshot.tabs.length === 0) return
    yield* workspaceEditorHandlers.WorkspaceEditorRecoveryRequested({ grantRef, snapshot })
  })
  /**
   * Hands one completed answer set to the typed bridge. The card collapses
   * only after the bridge confirms success. This preserves the frozen local
   * behavior while preventing a durable Sync enqueue receipt from being
   * presented as an authoritative resolution.
   */
  const submitQuestion = (
    card: NonNullable<DesktopNoteEntry["question"]>,
    interaction: QuestionCardInteraction,
  ) =>
    Effect.gen(function* () {
      const answer = questionHost.answer
      if (answer === null || interaction.answered) return
      if (!questionAnswersReady(card, interaction)) return
      const answers = questionAnswersFor(card, interaction)
      const result = yield* Effect.promise(() =>
        answer({
          turnRef: card.turnRef,
          ...(card.threadRef === undefined ? {} : { threadRef: card.threadRef }),
          questionRef: card.questionRef,
          answers,
        }).catch(() => null))
      if (result === true) {
        yield* SubscriptionRef.update(state, (current) =>
          withQuestionAnswered(current, card.questionRef, answers))
      }
    })
  return ({
  ...settingsHandlers,
  ...makeFleetWorkspaceHandlers(state, fleetBridge, () => settingsHandlers.DesktopSettingsToggled()),
  ...makeGitPanelHandlers(state, gitBridge),
  ...workspaceBrowserHandlers,
  ...workspaceEditorHandlers,
  WorkspaceBrowserEntrySelected: (pathRef) => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const knownEntry = Object.values(before.workspaceBrowser.pages)
      .flatMap(page => page.entries)
      .find(entry => entry.pathRef === pathRef)
    const isSearchResult = before.workspaceBrowser.searchPage?.state === "available" &&
      before.workspaceBrowser.searchPage.matches.some(match => match.pathRef === pathRef)
    yield* workspaceBrowserHandlers.WorkspaceBrowserEntrySelected(pathRef)
    if (before.workspaceBrowser.grantRef === null || knownEntry?.kind === "directory" || (knownEntry === undefined && !isSearchResult)) {
      return
    }
    yield* workspaceEditorHandlers.WorkspaceEditorOpenRequested({
      grantRef: before.workspaceBrowser.grantRef,
      pathRef,
    })
  }),
  WorkspaceBrowserEditorSubmitted: () => Effect.gen(function* () {
    const before = yield* SubscriptionRef.get(state)
    const sourcePathRef = before.workspaceBrowser.editor?.kind === "rename"
      ? before.workspaceBrowser.editor.pathRef
      : null
    yield* workspaceBrowserHandlers.WorkspaceBrowserEditorSubmitted()
    if (sourcePathRef === null) return
    const after = yield* SubscriptionRef.get(state)
    if (after.workspaceBrowser.operation?.state !== "renamed") return
    const next = {
      ...after,
      workspaceEditor: withWorkspaceEditorRenamed(
        after.workspaceEditor,
        sourcePathRef,
        after.workspaceBrowser.operation.entry.pathRef,
      ),
    }
    yield* SubscriptionRef.set(state, next)
    persistWorkspaceRecovery(next)
  }),
  DesktopSettingsToggled: () => Effect.gen(function* () {
    yield* settingsHandlers.DesktopSettingsToggled()
    const bindings = yield* Effect.promise(commandBindingHost.snapshot)
    yield* SubscriptionRef.update(state, current => ({ ...current, commandBindings: bindings }))
  }),
  DesktopCommandBindingSelected: (commandId) => SubscriptionRef.update(state, current => {
    const row = current.commandBindings?.rows.find(value => value.commandId === commandId)
    return row === undefined
      ? current
      : {
          ...current,
          commandBindingSelectedId: row.commandId,
          commandBindingDraft: row.overrideBinding ?? row.effectiveBindings[0] ?? "",
        }
  }),
  DesktopCommandBindingDraftChanged: (value) => SubscriptionRef.update(state, current => ({
    ...current,
    commandBindingDraft: value.slice(0, 80),
  })),
  DesktopCommandBindingSaved: () => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    if (current.commandBindingSelectedId === null) return
    let chord: string
    try {
      chord = normalizeDesktopCommandChord(current.commandBindingDraft)
    } catch {
      yield* SubscriptionRef.update(state, value => ({ ...value, commandNotice: "Use a shortcut such as Meta+Shift+K or Control+K." }))
      return
    }
    const bindings = yield* Effect.promise(() => commandBindingHost.save({ commandId: current.commandBindingSelectedId!, chord }))
    yield* SubscriptionRef.update(state, value => ({
      ...value,
      commandBindings: bindings,
      commandNotice: bindings === null ? "Keybindings are unavailable." : null,
    }))
  }),
  DesktopCommandBindingRemoved: () => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    if (current.commandBindingSelectedId === null) return
    const bindings = yield* Effect.promise(() => commandBindingHost.save({ commandId: current.commandBindingSelectedId!, chord: null }))
    yield* SubscriptionRef.update(state, value => ({
      ...value,
      commandBindings: bindings,
      commandBindingDraft: bindings?.rows.find(row => row.commandId === current.commandBindingSelectedId)?.effectiveBindings[0] ?? "",
      commandNotice: bindings === null ? "Keybindings are unavailable." : null,
    }))
  }),
  DesktopCommandBindingsReset: () => Effect.gen(function* () {
    const bindings = yield* Effect.promise(commandBindingHost.reset)
    yield* SubscriptionRef.update(state, value => ({
      ...value,
      commandBindings: bindings,
      commandBindingSelectedId: null,
      commandBindingDraft: "",
      commandNotice: bindings === null ? "Keybindings are unavailable." : null,
    }))
  }),
  DesktopInputChanged: (value) =>
    SubscriptionRef.update(state, (current) => withInput(current, value)),
  DesktopNoteSubmitted: (value) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (current.activeThreadId === null) return
      const message =
        typeof value === "string" && value.trim() !== "" ? value : current.input
      // A3 queue-until-idle (EP250 wave-2): the composer stays usable while a
      // turn streams; a submit mid-turn ENQUEUES a text follow-up instead of
      // starting a new turn. Delivery is at the current turn's completion (the
      // runtime's followup_queued/followup_promoted events drive the chip and
      // the promoted next turn). A host that cannot queue keeps the draft. An
      // empty follow-up is never queued; image attachments are not queued
      // mid-turn (they ride the next started turn).
      if (current.pending) {
        if (message.trim() === "") return
        if (chat.queueFollowup === undefined) return
        yield* SubscriptionRef.update(state, (next) => withInput(next, ""))
        yield* Effect.promise(() =>
          chat.queueFollowup!({ threadRef: current.activeThreadId!, message: message.trim() }))
        return
      }
      // Evidence-gated send (#8712): an unavailable selected lane must not
      // accept the action — the composer keeps the draft and the caption
      // already names the reason. Never substitute another lane silently.
      if (!current.harnessLanes[current.selectedHarness].available) return
      // Capability I1: a turn is submittable with text OR ≥1 image; an empty
      // turn with no images is a no-op (withNote returns state unchanged).
      if (message.trim() === "" && current.composerImages.length === 0) return
      // Capture the pending attachments BEFORE withNote clears them.
      const images = toStartImages(current.composerImages)
      yield* SubscriptionRef.set(state, withNote(current, message, now()))
      const result = yield* Effect.promise(() => chat.sendMessage({
        id: current.activeThreadId!,
        message,
        harness: current.selectedHarness,
        ...(images.length > 0 ? { images } : {}),
        onUpdate: thread => {
          Effect.runFork(SubscriptionRef.update(state, next =>
            next.activeThreadId === thread.id
              ? { ...withChatSelected(next, thread), pending: true }
              : next))
        },
      }))
      yield* SubscriptionRef.update(state, (next) => withTurnResult(next, result, now()))
    }),
  DesktopTurnInterrupted: () =>
    Effect.gen(function* () {
      // Only meaningful while a turn streams; the terminal turn result (a
      // typed `interrupted` failure) is what reverts pending -> Send. The Stop
      // handler never fabricates that terminal state itself.
      const current = yield* SubscriptionRef.get(state)
      if (!current.pending) return
      yield* Effect.promise(() => chat.interruptActive?.() ?? Promise.resolve(false))
    }),
  DesktopChildInterruptRequested: ({ turnRef, childRef }) =>
    Effect.gen(function* () {
      // G4: signal the frozen steer-child channel by exact ref. The runtime's
      // typed child_steered event renders the outcome; the handler invents no
      // state, and a host without a local streaming lane simply no-ops.
      const current = yield* SubscriptionRef.get(state)
      if (!current.pending || chat.steerChild === undefined) return
      yield* Effect.promise(() => chat.steerChild!({ turnRef, childRef }))
    }),
  DesktopComposerImageAdded: (attachment) =>
    SubscriptionRef.update(state, (current) => withComposerImageAdded(current, attachment)),
  DesktopComposerImageRemoved: (id) =>
    SubscriptionRef.update(state, (current) => withComposerImageRemoved(current, id)),
  DesktopComposerImagesRejected: (message) =>
    SubscriptionRef.update(state, (current) => withComposerImageNotice(current, message)),
  DesktopComposerImagePickRequested: () =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (current.pending || !canAttachMoreImages(current.composerImages)) return
      const picked = yield* Effect.promise(() => imagePickerHost.pick())
      if (picked.length === 0) return
      yield* SubscriptionRef.update(state, (value) => {
        let next = value
        for (const image of picked) {
          next = withComposerImageAdded(next, {
            id: globalThis.crypto.randomUUID(),
            mediaType: image.mediaType,
            data: image.data,
            name: image.name ?? "image",
            // Decoded size for the caption; base64 length approximates it.
            sizeBytes: Math.floor((image.data.length * 3) / 4),
          })
        }
        return next
      })
    }),
  DesktopLoopPinged: () =>
    SubscriptionRef.update(state, (current) => withLoopProof(current, now())),
  DesktopFleetDeskToggled: () =>
    SubscriptionRef.update(state, withFleetDesk),
  DesktopFleetObjectiveChanged: (value) =>
    SubscriptionRef.update(state, (current) => withFleetObjective(current, value)),
  DesktopFleetDeploymentRequested: () =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const dispatching = withFleetDeploymentRequested(current)
      if (dispatching === current) return
      yield* SubscriptionRef.set(state, dispatching)
      const result = yield* Effect.promise(() => stageFleet({ objective: dispatching.fleetObjective }))
      yield* SubscriptionRef.update(state, (next) => withFleetDeploymentResult(next, result, now()))
    }),
  DesktopNewChat: () => Effect.gen(function* () { const thread = yield* Effect.promise(chat.newThread); if (thread) yield* SubscriptionRef.update(state, (current) => withNewChat(current, thread)) }),
  DesktopHarnessSelected: (harness) =>
    SubscriptionRef.update(state, (current) => current.selectedHarness === harness ? current : { ...current, selectedHarness: harness }),
  DesktopMessageSelected: (key) =>
    SubscriptionRef.update(state, (current) => withMessageSelected(current, key)),
  DesktopToolCardToggled: (key) =>
    SubscriptionRef.update(state, (current) => withToolCardToggled(current, key)),
  DesktopFullscreenToggled: () => Effect.promise(async () => { await windowHost.toggleFullScreen() }),
  DesktopQuestionOptionSelected: ({ questionRef, questionIndex, label }) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      // Evidence-gated: no typed answer bridge means read-only pending cards.
      if (!current.questionAnswerHostAvailable || questionHost.answer === null) return
      if (current.questionCards[questionRef]?.answered === true) return
      const next = withQuestionSelection(current, questionRef, questionIndex, label)
      if (next === current) return
      yield* SubscriptionRef.set(state, next)
      const card = questionNoteFor(next, questionRef)?.question
      const interaction = next.questionCards[questionRef]
      if (card === undefined || interaction === undefined) return
      // Single-select cards dispatch as soon as every question is answered;
      // any multiSelect question keeps the explicit confirm affordance.
      if (!card.questions.some((question) => question.multiSelect) &&
        questionAnswersReady(card, interaction)) {
        yield* submitQuestion(card, interaction)
      }
    }),
  DesktopQuestionSubmitted: (questionRef) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (!current.questionAnswerHostAvailable) return
      const card = questionNoteFor(current, questionRef)?.question
      const interaction = current.questionCards[questionRef]
      if (card === undefined || card.status !== "pending" || interaction === undefined) return
      yield* submitQuestion(card, interaction)
    }),
  DesktopAgentGraphToggled: () =>
    SubscriptionRef.update(state, current => current.agentGraph === null
      ? current
      : { ...current, agentGraphExpanded: !current.agentGraphExpanded }),
  DesktopAgentAction: ({ kind, agentRef }) =>
    SubscriptionRef.update(state, current => {
      if (current.agentGraph === null) return current
      const selectedAgentRef = agentRef === "" || (kind === "inspect_agent" && current.selectedAgentRef === agentRef)
        ? null
        : resolveLiveAgentGraphSelection(current.agentGraph, agentRef)
      return selectedAgentRef === current.selectedAgentRef
        ? current
        : { ...current, selectedAgentRef }
    }),
  DesktopCodingCatalogFilterSelected: (filter) =>
    SubscriptionRef.update(state, current => ({ ...current, codingSessionFilter: filter })),
  DesktopCodingCatalogQueryChanged: (query) =>
    SubscriptionRef.update(state, current => ({ ...current, codingSessionQuery: query.slice(0, 512) })),
  DesktopCodingCatalogChooseRequested: () => Effect.gen(function* () {
    const codingCatalog = yield* Effect.promise(codingCatalogHost.choose)
    yield* SubscriptionRef.update(state, (current): DesktopShellState => ({
      ...current,
      codingCatalog,
      workspace: "home",
      codingSessionFilter: "active",
      codingSessionQuery: "",
    }))
  }),
  DesktopCodingSessionOpened: (sessionRef) => Effect.gen(function* () {
    const codingCatalog = yield* Effect.promise(() => codingCatalogHost.open(sessionRef))
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      codingCatalog,
      workspace: desktopWorkspaceForCodingFocus(codingCatalog.focus),
    }))
  }),
  DesktopCodingSessionArchived: (sessionRef) => Effect.gen(function* () {
    const codingCatalog = yield* Effect.promise(() => codingCatalogHost.archive(sessionRef))
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      codingCatalog,
      workspace: codingCatalog.selectedSessionRef === null
        ? "home"
        : desktopWorkspaceForCodingFocus(codingCatalog.focus),
    }))
  }),
  DesktopCodingSessionRecovered: (sessionRef) => Effect.gen(function* () {
    const codingCatalog = yield* Effect.promise(() => codingCatalogHost.recover(sessionRef))
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      codingCatalog,
      workspace: desktopWorkspaceForCodingFocus(codingCatalog.focus),
    }))
  }),
  DesktopChatSelected: (id) => Effect.gen(function* () {
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      activeThreadId: id,
      notes: [],
      expandedToolCards: [],
      questionCards: {},
      agentGraph: null,
      agentGraphExpanded: false,
      selectedAgentRef: null,
    }))
    const thread = yield* Effect.promise(() => chat.openThread(id)); if (!thread) return
    yield* SubscriptionRef.update(state, current => current.activeThreadId === id
      ? withChatSelected(current, thread)
      : current)
    if (chat.hydrateThread !== undefined) {
      const hydrated = yield* Effect.promise(() => chat.hydrateThread!(id))
      if (hydrated) yield* SubscriptionRef.update(state, current => current.activeThreadId === id ? withChatSelected(current, hydrated) : current)
    }
  }),
  HistoryConversationSelected: (id) => Effect.gen(function* () {
    yield* SubscriptionRef.update(state,current=>({...current,history:{...current.history,pendingThreadRef:id}}))
    // Open at the END (EP250 bottom-anchored flow): probe the total, then
    // fetch the LAST page so the newest items render first; older pages
    // auto-load as the reader scrolls up. Fetch order only — the
    // completeness equation and counted gaps are whole-conversation truth.
    const probe = yield* Effect.promise(() => historyHost.page(id, 0, 1))
    const page = probe === null ? null : yield* Effect.promise(() => historyHost.page(id, historyTailOffset(probe.totalItems), historyItemPageSize))
    if (page) { yield* SubscriptionRef.update(state, (current): DesktopShellState => { if(current.history.pendingThreadRef!==id)return current; const expandedThreadRefs=page.agents.filter(agent=>agent.descendantCount>0).map(agent=>agent.threadRef); const history={ ...current.history, page, selectedItemRef: null, expandedThreadRefs, pendingThreadRef:null, loadingEdge:null }; historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:null,railCollapsed:history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs}); return { ...current, workspace: "chat", history } }) }
    else yield* SubscriptionRef.update(state,current=>current.history.pendingThreadRef===id?({...current,history:{...current.history,pendingThreadRef:null}}):current)
  }),
  HistoryAgentSelected: (id) => Effect.gen(function* () {
    const probe = yield* Effect.promise(() => historyHost.page(id, 0, 1))
    const page = probe === null ? null : yield* Effect.promise(() => historyHost.page(id, historyTailOffset(probe.totalItems), historyItemPageSize))
    if (page) { yield* SubscriptionRef.update(state, current => { const history={ ...current.history, page, selectedItemRef: null, loadingEdge:null }; historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:null,railCollapsed:history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:history.expandedThreadRefs}); return { ...current, history } }) }
  }),
  HistoryItemSelected: (id) => SubscriptionRef.update(state, current => { const selectedItemRef=id===""||current.history.selectedItemRef===id?null:id; const page=current.history.page;if(page){const selectedSequence=selectedItemRef===null?null:page.items.find(item=>item.itemRef===selectedItemRef)?.sequence??null;historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:selectedSequence===null?page.offset:historyItemPageOffset(selectedSequence),selectedItemRef,railCollapsed:current.history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:current.history.expandedThreadRefs})}return { ...current, history: { ...current.history, selectedItemRef } } }),
  // Auto-load on scroll (EP250): no pager — the loaded window extends up
  // (older) or down (newer) and merges in place; a thin honest loading row
  // marks the fetching edge.
  HistoryOlderRequested: () => Effect.gen(function* () {
    const before = (yield* SubscriptionRef.get(state)).history
    const window = before.page
    if (window === null || before.loadingEdge !== null || window.offset <= 0) return
    yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, loadingEdge: "top" as const } }))
    const fetchOffset = Math.max(0, window.offset - historyItemPageSize)
    const older = yield* Effect.promise(() => historyHost.page(window.selectedThreadRef, fetchOffset, window.offset - fetchOffset))
    yield* SubscriptionRef.update(state, current => {
      const live = current.history.page
      if (older === null || live === null || live.selectedThreadRef !== window.selectedThreadRef) return { ...current, history: { ...current.history, loadingEdge: null } }
      const merged = mergeHistoryWindowUp(live, older)
      historyHost.save?.({rootThreadRef:merged.rootThreadRef,selectedThreadRef:merged.selectedThreadRef,offset:merged.offset,selectedItemRef:current.history.selectedItemRef,railCollapsed:current.history.railCollapsed,anchorItemRef:merged.items[0]?.itemRef??null,expandedThreadRefs:current.history.expandedThreadRefs})
      return { ...current, history: { ...current.history, page: merged, loadingEdge: null } }
    })
  }),
  HistoryNewerRequested: () => Effect.gen(function* () {
    const before = (yield* SubscriptionRef.get(state)).history
    const window = before.page
    const windowEnd = window === null ? 0 : window.offset + window.items.length
    if (window === null || before.loadingEdge !== null || windowEnd >= window.totalItems) return
    yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, loadingEdge: "bottom" as const } }))
    const newer = yield* Effect.promise(() => historyHost.page(window.selectedThreadRef, windowEnd, historyItemPageSize))
    yield* SubscriptionRef.update(state, current => {
      const live = current.history.page
      if (newer === null || live === null || live.selectedThreadRef !== window.selectedThreadRef) return { ...current, history: { ...current.history, loadingEdge: null } }
      return { ...current, history: { ...current.history, page: mergeHistoryWindowDown(live, newer), loadingEdge: null } }
    })
  }),
  HistoryInspectorToggled: () => SubscriptionRef.update(state, current => { const railCollapsed=!current.history.railCollapsed;const page=current.history.page;if(page)historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:current.history.selectedItemRef,railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:current.history.expandedThreadRefs});return { ...current, history: { ...current.history, railCollapsed } } }),
  HistoryAgentExpandedToggled: (id) => SubscriptionRef.update(state,current=>{const expandedThreadRefs=current.history.expandedThreadRefs.includes(id)?current.history.expandedThreadRefs.filter(ref=>ref!==id):[...current.history.expandedThreadRefs,id];const page=current.history.page;if(page)historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:current.history.selectedItemRef,railCollapsed:current.history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs});return {...current,history:{...current.history,expandedThreadRefs}}}),
  HistoryCatalogMoreRequested: () => SubscriptionRef.update(state,current=>({...current,history:{...current.history,visibleRootCount:Math.min(current.history.catalog.roots.length,current.history.visibleRootCount+historyCatalogPageSize)}})),
  // Free-text session search (#8712 H4). The query drives the bounded local
  // index cache; results replace the catalog list until cleared. Blank query
  // clears. Never mutates the loss-accounted catalog/page truth.
  HistorySearchChanged: (query) => Effect.gen(function* () {
    yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, searchQuery: query } }))
    if (query.trim() === "") { yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, searchResults: [], searchTruncated: false } })); return }
    if (historyHost.search === undefined) return
    const response = yield* Effect.promise(() => historyHost.search!(query, 40))
    yield* SubscriptionRef.update(state, current => current.history.searchQuery === query
      ? { ...current, history: { ...current.history, searchResults: response?.results ?? [], searchTruncated: response?.truncated ?? false } }
      : current)
  }),
  HistorySearchCleared: () => SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, searchQuery: "", searchResults: [], searchTruncated: false } })),
  // Open a ranked result: window the session on its matching item (content
  // result) or at its end (title result), reusing the bottom-anchored flow.
  HistorySearchResultOpened: (threadRef) => Effect.gen(function* () {
    const before = (yield* SubscriptionRef.get(state)).history
    const result = before.searchResults.find(candidate => candidate.threadRef === threadRef)
    if (result === undefined) return
    yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, pendingThreadRef: threadRef } }))
    const probe = yield* Effect.promise(() => historyHost.page(threadRef, 0, 1))
    const anchorSequence = result.matchSequence
    const offset = probe === null ? 0 : anchorSequence === null ? historyTailOffset(probe.totalItems) : historyItemPageOffset(anchorSequence)
    const page = probe === null ? null : yield* Effect.promise(() => historyHost.page(threadRef, offset, historyItemPageSize))
    if (page === null) { yield* SubscriptionRef.update(state, current => current.history.pendingThreadRef === threadRef ? ({ ...current, history: { ...current.history, pendingThreadRef: null } }) : current); return }
    const selectedItemRef = result.matchItemRef !== null && page.items.some(item => item.itemRef === result.matchItemRef) ? result.matchItemRef : null
    yield* SubscriptionRef.update(state, (current): DesktopShellState => {
      if (current.history.pendingThreadRef !== threadRef) return current
      const expandedThreadRefs = page.agents.filter(agent => agent.descendantCount > 0).map(agent => agent.threadRef)
      const history = { ...current.history, page, selectedItemRef, expandedThreadRefs, pendingThreadRef: null, loadingEdge: null }
      historyHost.save?.({ rootThreadRef: page.rootThreadRef, selectedThreadRef: page.selectedThreadRef, offset: page.offset, selectedItemRef, railCollapsed: history.railCollapsed, anchorItemRef: page.items[0]?.itemRef ?? null, expandedThreadRefs })
      return { ...current, workspace: "chat", history }
    })
  }),
  DesktopWorkspaceSelected: (workspace) =>
    Effect.gen(function* () {
      yield* SubscriptionRef.update(state, (current) => withWorkspace(current, workspace))
      if (workspace === "fleet") {
        yield* refreshFleetAccounts(state, fleetBridge)
      }
      if (workspace === "files") {
        yield* workspaceBrowserHandlers.WorkspaceBrowserOpened()
        yield* recoverWorkspaceEditor
      }
      if (workspace === "home") {
        const codingCatalog = yield* Effect.promise(codingCatalogHost.snapshot)
        yield* SubscriptionRef.update(state, current => ({ ...current, codingCatalog }))
      }
      if (workspace === "review") {
        yield* refreshGitPanel(state, gitBridge)
      }
    }),
  DesktopWorkspacePickerRequested: () =>
    Effect.gen(function* () {
      const selected = yield* Effect.promise(workspaceHost.choose)
      if (selected !== true) return
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        workspaceEditor: emptyWorkspaceEditorState(),
      }))
      const codingCatalog = yield* Effect.promise(codingCatalogHost.snapshot)
      yield* SubscriptionRef.update(state, current => ({ ...current, codingCatalog }))
      yield* workspaceBrowserHandlers.WorkspaceBrowserOpened()
      yield* recoverWorkspaceEditor
    }),
  DesktopCommandPaletteToggled: () =>
    SubscriptionRef.update(state, (current) => withCommandPalette(current, !current.commandPaletteOpen)),
  DesktopCommandPaletteDismissed: () =>
    SubscriptionRef.update(state, (current) => withCommandPalette(current, false)),
  DesktopHistoryShortcutHintsChanged: (visible) =>
    SubscriptionRef.update(state, (current) => current.historyShortcutHintsVisible === visible ? current : { ...current, historyShortcutHintsVisible: visible }),
  DesktopHistoryConversationPreviewed: (id) =>
    SubscriptionRef.update(state, (current) => current.history.pendingThreadRef === id ? current : { ...current, history: { ...current.history, pendingThreadRef: id } }),
  })
}

// ---------------------------------------------------------------------------
// View — pure `state -> View` over the shared catalog.
// ---------------------------------------------------------------------------

const text = (
  key: string,
  content: string,
  variant: TextView["variant"] = "body",
  color: TextView["color"] = "textPrimary",
): TextView => Text({ key, content, variant, color })

/** The comfortable desktop reading measure shared by conversation + composer. */
const columnWidth = 840

/**
 * Real v29 chat rows: sender label and timestamp are typed message data — the
 * renderer draws the meta row and the role treatment (user end-aligned
 * bubble, system muted prose).
 *
 * EP250 owner directives (#8712):
 * - "Remove where it says assistant." — assistant rows carry NO sender label
 *   (timestamp only); user rows keep YOU, system rows keep SYSTEM.
 * - Assistant bodies render markdown through the catalog Markdown/CodeBlock
 *   views (see ./markdown.ts) — user and system text stays literal.
 * - Every row carries a Details affordance dispatching the typed
 *   DesktopMessageSelected intent (the metadata inspector; keyboard
 *   accessible because it is a real catalog Button).
 */
/**
 * EP250 owner directive (verbatim): "that metadata button needs to be way
 * smaller and more like an icon button, not a huge ginormous circle."
 *
 * The catalog IconButton lowers to a fixed 44px circle in the DOM renderer
 * (inline sizing an app style cannot shrink), so the compact affordance is a
 * ghost catalog Button lowered to caption scale with zero padding — a dim,
 * roughly line-height text affordance that stays a real keyboard-focusable
 * button. Catalog-native; no local primitive.
 */
const compactDetailsButton = (input: Readonly<{
  key: string
  label: string
  onPress: ReturnType<typeof IntentRef>
  a11yLabel: string
}>): View =>
  Button({
    key: input.key,
    label: input.label,
    variant: "ghost",
    // Zero padding + zero border + caption scale + faint color: the third
    // level of the dim ladder (apps-sdk/OpenCode reconciliation, EP250) —
    // a hover-revealed hint affordance, dimmer than muted body copy. The
    // renderer's default button chrome is a 1px bordered chip — too loud
    // for a per-row affordance.
    style: { padding: "0", borderWidth: 0, typeScale: "caption", color: "textFaint" },
    onPress: input.onPress,
    a11y: { label: input.a11yLabel },
  })

export const noteMessage = (entry: DesktopNoteEntry): TranscriptMessage => ({
  key: entry.key,
  role: entry.role,
  ...(entry.role === "assistant"
    ? {}
    : {
        senderLabel: entry.role === "user"
          ? entry.key.startsWith("pending-") ? "YOU · PENDING" : "YOU"
          : "SYSTEM",
      }),
  timestamp: entry.timestamp,
  body: [
    ...(entry.role === "assistant"
      ? chatMarkdownBody(`${entry.key}-text`, entry.text)
      : [text(
          `${entry.key}-text`,
          entry.text,
          "body",
          entry.role === "system" ? "textMuted" : "textPrimary",
        )]),
    // Own row so the affordance never overlaps the message text.
    Stack(
      {
        key: `note-meta-row-${entry.key}`,
        direction: "row",
        gap: "1",
        align: "center",
        justify: entry.role === "user" ? "end" : "start",
      },
      [compactDetailsButton({
        key: `note-details-${entry.key}`,
        label: "details",
        onPress: IntentRef("DesktopMessageSelected", StaticPayload(entry.key)),
        a11yLabel: `Show message details, ${entry.role} message at ${entry.timestamp}`,
      })],
    ),
  ],
})

/**
 * One typed tool card per invocation (EP250, #8712): humanized primary line,
 * toned status chip that updates in place (started -> ok/failed), result or
 * failure line as content, and the bounded raw args/result reachable only
 * behind the compact details toggle. No SYSTEM role label — tool cards are
 * their own visual class (the tool title is the header); timestamps stay.
 */
/**
 * Design language ported from the opencode desktop reference (owner design
 * directive, EP250): a DENSE single-line tool trigger — 16px icon slot, 14px
 * medium title, inline muted single-line subtitle (their
 * `[data-component="tool-trigger"]` / `basic-tool-tool-title` /
 * `basic-tool-tool-subtitle`, projects/repos/opencode
 * packages/session-ui/src/components/basic-tool.css) — with collapsed-by-
 * default details, translated to typed Effect Native token styles on the
 * Protoss-blue theme (our label scale is 14px/500, Icon sm is 16px, gap "2"
 * is their 8px). Agent-class tools (Agent, mcp__codex__*) additionally get
 * their boxed `task-tool-card` treatment (8px/12px padding, 6px radius, thin
 * border on a raised translucent surface) via the catalog Card. See
 * docs/design-ports.md.
 */
export const toolCardMessage = (card: ToolCardModel, expanded: boolean): TranscriptMessage => {
  const human = humanizeToolInvocation(card.toolName, card.argsSummary)
  const chip = card.status === "running"
    ? { label: "Running", tone: "neutral" as const }
    : card.status === "ok"
      ? { label: "OK", tone: "success" as const }
      : { label: "Failed", tone: "danger" as const }
  // opencode's boxed task-tool-card applies to subagent-class tools only.
  const boxed = card.toolName === "Agent" || card.toolName.startsWith("mcp__codex__")
  const column = Stack(
      { key: `tool-card-${card.key}`, direction: "column", gap: "1", align: "start" },
      [
        Stack(
          { key: `tool-header-${card.key}`, direction: "row", gap: "2", align: "center" },
          [
            Icon({ key: `tool-icon-${card.key}`, name: toolCardIcon(card.toolName), size: "sm", color: "accent" }),
            // Running-state title shimmer (card reconciliation, EP250): a
            // key-marked title the host stylesheet animates as a 1200ms
            // opacity wave (the RN-safe mechanism the OpenCode spec allows —
            // never per-character background-clip). The key flips back at
            // completion, ending the animation.
            Text({
              key: card.status === "running" ? `tool-title-running-${card.key}` : `tool-title-${card.key}`,
              content: human.title,
              variant: "label",
              color: "textPrimary",
              weight: "medium",
            }),
            // Inline muted subtitle on the same trigger row (opencode's
            // basic-tool-tool-subtitle), already bounded by the humanizer.
            ...(human.detail === "" ? [] : [
              Text({ key: `tool-detail-${card.key}`, content: human.detail, variant: "body", color: "textMuted" }),
            ]),
            Badge({
              key: `tool-status-${card.key}`,
              label: chip.label,
              tone: chip.tone,
              a11y: { label: `${human.title} ${chip.label}` },
            }),
          ],
        ),
        // Failure text is content, not JSON — shown prominently.
        ...(card.status === "failed" && card.resultSummary !== null ? [
          Text({ key: `tool-failure-${card.key}`, content: card.resultSummary, variant: "body", color: "danger" }),
        ] : []),
        ...(card.status === "ok" && card.resultSummary !== null ? [
          Text({ key: `tool-result-${card.key}`, content: toolResultSnippet(card.resultSummary), variant: "caption", color: "textMuted" }),
        ] : []),
        compactDetailsButton({
          key: `tool-details-${card.key}`,
          label: expanded ? "hide details" : "details",
          onPress: IntentRef("DesktopToolCardToggled", StaticPayload(card.key)),
          a11yLabel: `${expanded ? "Hide" : "Show"} raw details for ${human.title}`,
        }),
        // The bounded raw payloads: reachable, never the default rendering.
        // The well caps at the 240px output bound (dimension "sm") with its
        // own scroll region; payload text sits at the faint dim level.
        ...(expanded ? [Stack(
          {
            key: `tool-raw-well-${card.key}`,
            direction: "column",
            gap: "1",
            align: "start",
            style: { width: "full", maxHeight: "sm" },
          },
          [
            Text({
              key: `tool-raw-args-${card.key}`,
              content: card.argsSummary === "" ? "(no recorded arguments)" : card.argsSummary,
              variant: "caption",
              color: "textFaint",
            }),
            ...(card.resultSummary === null ? [] : [Text({
              key: `tool-raw-result-${card.key}`,
              content: card.resultSummary,
              variant: "caption",
              color: "textFaint",
            })]),
          ],
        )] : []),
      ],
    )
  return {
    key: `tool-${card.key}`,
    role: "tool",
    timestamp: card.timestamp,
    body: [boxed
      ? Card({
          key: `tool-box-${card.key}`,
          padding: "2",
          // Quantized onto the shared khala radius scale: the OpenCode task
          // card's 6px maps to radius "lg" (spec §3 radius translation).
          radius: "lg",
          style: { width: "full", borderColor: "borderSubtle", borderWidth: 1, surface: "glass" },
        }, [column])
      : column],
  }
}

/**
 * Context group row (EP250 card reconciliation): consecutive read/glob/grep
 * invocations render as ONE quiet "Gathered context — N reads, M searches"
 * row; expanding indents the member rows at the tighter 4px sub-rhythm.
 * Members are plain rows — no chevrons, no per-member interaction.
 */
export const contextGroupMessage = (group: ContextGroupModel, expanded: boolean): TranscriptMessage => {
  const title = group.running ? "Gathering context…" : "Gathered context"
  return {
    key: `tool-${group.key}`,
    role: "tool",
    timestamp: group.cards[0]!.timestamp,
    body: [Stack(
      { key: `tool-card-${group.key}`, direction: "column", gap: "1", align: "start" },
      [
        Stack(
          { key: `tool-header-${group.key}`, direction: "row", gap: "2", align: "center" },
          [
            Icon({ key: `tool-icon-${group.key}`, name: "Folder", size: "sm", color: "accent" }),
            Text({
              key: group.running ? `tool-title-running-${group.key}` : `tool-title-${group.key}`,
              content: title,
              variant: "label",
              color: "textPrimary",
              weight: "medium",
            }),
            Text({ key: `tool-detail-${group.key}`, content: contextGroupSummary(group), variant: "body", color: "textMuted" }),
            ...(group.failed ? [Badge({
              key: `tool-status-${group.key}`,
              label: "Failed",
              tone: "danger",
              a11y: { label: `${title} — a member invocation failed` },
            })] : []),
          ],
        ),
        compactDetailsButton({
          key: `tool-details-${group.key}`,
          label: expanded ? "hide details" : "details",
          onPress: IntentRef("DesktopToolCardToggled", StaticPayload(group.key)),
          a11yLabel: `${expanded ? "Hide" : "Show"} the ${group.cards.length} grouped context invocations`,
        }),
        ...(expanded ? [Stack(
          {
            key: `tool-group-members-${group.key}`,
            direction: "column",
            gap: "1",
            align: "start",
            style: { width: "full", paddingLeft: "3" },
          },
          group.cards.map((member) => {
            const memberHuman = humanizeToolInvocation(member.toolName, member.argsSummary)
            return Stack(
              { key: `tool-group-member-${member.key}`, direction: "row", gap: "2", align: "center" },
              [
                Text({ key: `tool-group-member-title-${member.key}`, content: memberHuman.title, variant: "label", color: "textPrimary", weight: "medium" }),
                ...(memberHuman.detail === "" ? [] : [
                  Text({ key: `tool-group-member-detail-${member.key}`, content: memberHuman.detail, variant: "body", color: "textMuted" }),
                ]),
                ...(member.status === "failed" && member.resultSummary !== null ? [
                  Text({ key: `tool-group-member-failure-${member.key}`, content: member.resultSummary, variant: "caption", color: "danger" }),
                ] : []),
              ],
            )
          }),
        )] : []),
      ],
    )],
  }
}

/**
 * Interactive question card (EP250 scope addition, owner verbatim: "make the
 * question UI too. Why not? proper effect native primitives and add some if
 * needed."). A first-class typed card in the tool-card visual family — never
 * raw JSON, no SYSTEM label. Option rows compose catalog primitives: the
 * label is a real Button (prominent, keyboard accessible), the description a
 * dim caption Text beneath it. Single-select dispatches on click (via the
 * auto-submit in the option handler); multiSelect toggles and confirms.
 * Without a typed answer bridge the card renders read-only pending.
 */
export const questionCardMessage = (
  note: DesktopNoteEntry,
  interaction: QuestionCardInteraction | undefined,
  answerAvailable: boolean,
): TranscriptMessage => {
  const card = note.question!
  const base = `question-${card.questionRef}`
  const locallyAnswered = interaction?.answered === true
  const resolved = card.status !== "pending" || locallyAnswered
  const outcome = card.status !== "pending" ? card.status : "answered"
  if (resolved) {
    const answers = interaction?.answers ?? null
    const answerText = answers === null
      ? null
      : answers
          .map((answer) => answer.labels.join(", "))
          .filter((value) => value !== "")
          .join(" · ")
    const summary = outcome === "answered"
      ? answerText === null || answerText === "" ? "Answered." : `Answered · ${answerText}`
      : outcome === "resolved"
        ? "Decision confirmed."
      : outcome === "timeout"
        ? "Timed out — no answer was sent."
        : outcome === "expired"
          ? "Expired — no decision was applied."
          : outcome === "revoked"
            ? "Revoked — authority is no longer available."
            : "Denied — the question was dismissed."
    const label = outcome === "answered" ? "Answered"
      : outcome === "resolved" ? "Resolved"
        : outcome === "timeout" ? "Timed out"
          : outcome === "expired" ? "Expired"
            : outcome === "revoked" ? "Revoked"
              : "Denied"
    return {
      key: base,
      role: "tool",
      timestamp: note.timestamp,
      body: [Stack({ key: `${base}-resolved-card`, direction: "column", gap: "1", align: "start" }, [
        Stack({ key: `${base}-resolved-header`, direction: "row", gap: "2", align: "center" }, [
          Badge({
            key: `${base}-outcome`,
            label,
            tone: outcome === "answered" || outcome === "resolved" ? "success" : "neutral",
          }),
          Text({ key: `${base}-resolved-question`, content: card.questions[0]?.question ?? "Question", variant: "caption", color: "textMuted" }),
        ]),
        Text({ key: `${base}-resolved-summary`, content: summary, variant: "body", color: "textMuted" }),
      ])],
    }
  }
  const anyMulti = card.questions.some((question) => question.multiSelect)
  return {
    key: base,
    role: "tool",
    timestamp: note.timestamp,
    body: [Stack({ key: `${base}-card`, direction: "column", gap: "2", align: "start" }, [
      Stack({ key: `${base}-header`, direction: "row", gap: "2", align: "center" }, [
        Badge({
          key: `${base}-chip`,
          label: (card.questions[0]?.header ?? "") === "" ? "Question" : card.questions[0]!.header,
          tone: "info",
          a11y: { label: "Agent question awaiting your answer" },
        }),
      ]),
      ...card.questions.flatMap((question, questionIndex) => {
        const selected = interaction?.selections[questionIndex] ?? []
        return [
          Text({
            key: `${base}-q${questionIndex}`,
            content: question.question,
            variant: "body",
            color: "textPrimary",
          }),
          ...question.options.flatMap((option, optionIndex) => [
            Stack(
              {
                key: `${base}-q${questionIndex}-opt-${optionIndex}`,
                direction: "column",
                gap: "0.5",
                align: "start",
              },
              [
                Button({
                  key: `${base}-q${questionIndex}-option-${optionIndex}`,
                  label: option.label,
                  variant: selected.includes(option.label) ? "secondary" : "ghost",
                  disabled: !answerAvailable,
                  onPress: IntentRef("DesktopQuestionOptionSelected", StaticPayload({
                    questionRef: card.questionRef,
                    questionIndex,
                    label: option.label,
                  })),
                  a11y: {
                    label: answerAvailable
                      ? `${question.multiSelect ? "Toggle" : "Answer"} ${option.label}`
                      : `${option.label} — answering unavailable until the runtime bridge connects`,
                  },
                }),
                ...(option.description === undefined ? [] : [Text({
                  key: `${base}-q${questionIndex}-option-${optionIndex}-description`,
                  content: option.description,
                  variant: "caption",
                  color: "textMuted",
                })]),
              ],
            ),
          ]),
        ]
      }),
      ...(anyMulti ? [Button({
        key: `${base}-confirm`,
        label: "Confirm",
        variant: "primary",
        disabled: !answerAvailable ||
          interaction === undefined ||
          !card.questions.every((_, index) => (interaction.selections[index]?.length ?? 0) >= 1),
        onPress: IntentRef("DesktopQuestionSubmitted", StaticPayload(card.questionRef)),
        a11y: { label: "Confirm selected answers" },
      })] : []),
    ])],
  }
}

/**
 * Plan/todo progress card (EP250 wave-2 J2/J4). A compact checklist — one row
 * per todo with a status glyph (pending/in_progress/completed from the exact
 * frozen enum), the in-progress row subtly emphasized. Updates in place as new
 * plan_updated events replace the entries (latest wins). Token styling only —
 * never raw JSON, no SYSTEM label.
 */
export const planCardMessage = (note: DesktopNoteEntry, plan: RuntimePlanCardPayload): TranscriptMessage => ({
  key: note.key,
  role: "tool",
  timestamp: note.timestamp,
  body: [Stack(
    { key: `plan-card-${note.key}`, direction: "column", gap: "1", align: "start", style: { width: "full" } },
    [
      Stack(
        { key: `plan-header-${note.key}`, direction: "row", gap: "2", align: "center" },
        [
          Icon({ key: `plan-icon-${note.key}`, name: "Compare", size: "sm", color: "accent" }),
          Text({ key: `plan-title-${note.key}`, content: "Plan", variant: "label", color: "textPrimary", weight: "medium" }),
          Text({ key: `plan-progress-${note.key}`, content: planProgressSummary(plan.entries), variant: "body", color: "textMuted" }),
        ],
      ),
      ...plan.entries.map((entry, index) => {
        const glyph = planStatusGlyph(entry.status)
        return Stack(
          { key: `plan-step-${note.key}-${index}`, direction: "row", gap: "2", align: "center" },
          [
            Icon({ key: `plan-step-icon-${note.key}-${index}`, name: glyph.icon, size: "sm", color: glyph.color }),
            Text({
              key: `plan-step-text-${note.key}-${index}`,
              content: entry.step,
              variant: "body",
              color: entry.status === "completed" ? "textMuted" : "textPrimary",
              weight: glyph.active ? "medium" : "regular",
            }),
          ],
        )
      }),
    ],
  )],
})

/**
 * Delegate-child lifecycle card (EP250 wave-2 G4). A running child (no terminal
 * and no interrupt yet) offers a single Interrupt control — the SDK/codex
 * cannot MESSAGE an in-flight child, so message is NOT offered
 * (capability-truthful). The child_steered outcome renders as a compact line.
 */
export const childCardMessage = (note: DesktopNoteEntry, child: RuntimeChildCardPayload): TranscriptMessage => {
  const chip = childStatusChip(child.status)
  const steerLine = childSteerLine(child.steered)
  return {
    key: note.key,
    role: "tool",
    timestamp: note.timestamp,
    body: [Card(
      {
        key: `child-box-${note.key}`,
        padding: "2",
        radius: "lg",
        style: { width: "full", borderColor: "borderSubtle", borderWidth: 1, surface: "glass" },
      },
      [Stack(
        { key: `child-card-${note.key}`, direction: "column", gap: "1", align: "start", style: { width: "full" } },
        [
          Stack(
            { key: `child-header-${note.key}`, direction: "row", gap: "2", align: "center" },
            [
              Icon({ key: `child-icon-${note.key}`, name: "Agent", size: "sm", color: "accent" }),
              Text({ key: `child-title-${note.key}`, content: child.title === "" ? "Delegate child" : child.title, variant: "label", color: "textPrimary", weight: "medium" }),
              Badge({ key: `child-status-${note.key}`, label: chip.label, tone: chip.tone, a11y: { label: `Delegate child ${chip.label}` } }),
              ...(childInterruptable(child)
                ? [Button({
                    key: `child-interrupt-${note.key}`,
                    label: "Interrupt",
                    variant: "ghost",
                    style: { padding: "0", borderWidth: 0, typeScale: "caption", color: "danger" },
                    onPress: IntentRef("DesktopChildInterruptRequested", StaticPayload({ turnRef: child.turnRef, childRef: child.childRef })),
                    a11y: { label: `Interrupt delegate child ${child.childRef}` },
                  })]
                : []),
            ],
          ),
          ...(child.detail === "" ? [] : [Text({ key: `child-detail-${note.key}`, content: child.detail, variant: "caption", color: "textMuted" })]),
          ...(steerLine === null ? [] : [Text({
            key: `child-steer-${note.key}`,
            content: steerLine,
            variant: "caption",
            color: child.steered?.outcome === "interrupted" ? "textMuted" : "textFaint",
          })]),
        ],
      )],
    )],
  }
}

/**
 * Queued follow-up chip (EP250 wave-2 A3). A compact "Queued follow-up (#N)"
 * badge shown while the follow-up sits in the queue-until-idle queue; it clears
 * when its followup_promoted lands (the promoted message becomes the next
 * turn). Honest semantics: delivered at the current turn's completion.
 */
export const queueChipMessage = (note: DesktopNoteEntry, queue: RuntimeQueueChipPayload): TranscriptMessage => ({
  key: note.key,
  role: "tool",
  timestamp: note.timestamp,
  body: [Stack(
    { key: `queue-chip-${note.key}`, direction: "row", gap: "2", align: "center" },
    [
      Icon({ key: `queue-icon-${note.key}`, name: "Pause", size: "sm", color: "textMuted" }),
      Badge({
        key: `queue-badge-${note.key}`,
        label: `Queued follow-up (#${queue.position})`,
        tone: "info",
        a11y: { label: `Follow-up queued at position ${queue.position}; delivered when this turn completes` },
      }),
      Text({ key: `queue-note-${note.key}`, content: "delivered when this turn completes", variant: "caption", color: "textFaint" }),
    ],
  )],
})

/** Dispatch a runtime-capability note to its typed card render (EP250 wave-2). */
export const runtimeCardMessage = (note: DesktopNoteEntry): TranscriptMessage => {
  const runtime = note.runtime!
  return runtime.kind === "plan"
    ? planCardMessage(note, runtime)
    : runtime.kind === "child"
      ? childCardMessage(note, runtime)
      : queueChipMessage(note, runtime)
}

const historySidebarItems = (state: DesktopShellState) => {
  const roots=state.history.catalog.roots.slice(0,state.history.visibleRootCount)
  const rows=roots.map((thread,index) => ({
    id:`sidebar-thread-${thread.threadRef}`,
    label:thread.title,
    meta:state.historyShortcutHintsVisible ? (index < 9 ? String(index + 1) : "") : `${historySourceBadgeLabel(thread.source)} · ${formatRelativeTimestamp(thread.updatedAt)}`,
    accessibilityLabel:`Open ${historySourceBadgeLabel(thread.source)} chat ${thread.title}, ${thread.descendantCount} descendant agents`,
    onSelect:IntentRef("HistoryConversationSelected",StaticPayload(thread.threadRef)),
  }))
  return state.history.visibleRootCount>=state.history.catalog.roots.length?rows:[...rows,{
    id:"sidebar-history-load-more",
    label:`Load ${Math.min(historyCatalogPageSize,state.history.catalog.roots.length-state.history.visibleRootCount)} more`,
    accessibilityLabel:`Load older Codex conversations, ${state.history.visibleRootCount} of ${state.history.catalog.roots.length} shown`,
    onSelect:IntentRef("HistoryCatalogMoreRequested"),
  }]
}

const localSidebarItems = (state: DesktopShellState) => state.threads.map((thread,index) => ({
  id:`sidebar-thread-${thread.id}`,
  label:thread.title,
  meta:state.historyShortcutHintsVisible ? (index < 9 ? String(index + 1) : "") : formatRelativeTimestamp(thread.updatedAt),
  accessibilityLabel:`Open chat ${thread.title}`,
  onSelect:IntentRef("DesktopChatSelected",StaticPayload(thread.id)),
}))

const shellSidebar = (state: DesktopShellState): View => {
  // Connected-accounts bottom box (EP250 owner contract verbatim: "in the
  // left sidebar, in a bottom box, like letting the chats flex up but show
  // up to 5 connected accounts with a progress bar showing remaining
  // weekly/hourly usage (grayed out if we dont have that data)"). The NavRail
  // above keeps flex:1/minHeight:0, so the chats list flexes up while this
  // box stays pinned at the column bottom; zero connected accounts render no
  // box at all.
  const accountsBox = sidebarAccountsView(state.fleet)
  return Stack(
    {
      key: "shell-sidebar",
      direction: "column",
      gap: "2",
      style: { height: "full", minHeight: 0, surface: "glass" },
    },
    [
      NavRail({
        key:"sidebar-navigation",
        activeId:state.history.pendingThreadRef!==null?`sidebar-thread-${state.history.pendingThreadRef}`:state.history.page!==null?`sidebar-thread-${state.history.page.rootThreadRef}`:state.activeThreadId!==null?`sidebar-thread-${state.activeThreadId}`:`workspace-${state.workspace}`,
        sections:[
          {id:"sidebar-workspace-dock",layout:"row",items:[
            // Owner directive (#8712): New chat is the top-leftmost dock item,
            // Fleet immediately to its right.
            {id:"workspace-new-chat",label:"New chat",icon:"ChatCompose",accessibilityLabel:"New chat",onSelect:IntentRef("DesktopNewChat")},
            {id:"workspace-fleet",label:"Fleet",icon:"Agent",selected:state.workspace==="fleet",accessibilityLabel:"Fleet",onSelect:IntentRef("DesktopWorkspaceSelected",StaticPayload("fleet"))},
            {id:"workspace-chat",label:"Chat",icon:"Chats",selected:state.workspace==="chat",accessibilityLabel:"Chat",onSelect:IntentRef("DesktopWorkspaceSelected",StaticPayload("chat"))},
            {id:"workspace-files",label:"Files",icon:"Folder",selected:state.workspace==="files",accessibilityLabel:"Files",onSelect:IntentRef("DesktopWorkspaceSelected",StaticPayload("files"))},
            {id:"workspace-home",label:"Project home",icon:"Home",selected:state.workspace==="home",accessibilityLabel:"Project home",onSelect:IntentRef("DesktopWorkspaceSelected",StaticPayload("home"))},
            {id:"shell-command-palette-toggle",label:"Commands",icon:"Menu",accessibilityLabel:"Open command palette",onSelect:IntentRef("DesktopCommandPaletteToggled")},
            {id:"shell-settings-toggle",label:"Settings",icon:"Settings",selected:state.workspace==="settings",accessibilityLabel:state.workspace==="settings"?"Close Settings":"Open Settings",onSelect:IntentRef("DesktopSettingsToggled")},
          ]},
          historySearchActive(state.history)
            ? {id:"sidebar-history-list",label:`Search · ${state.history.searchResults.length} result${state.history.searchResults.length===1?"":"s"}${state.history.searchTruncated?" (bounded)":""}`,items:historySearchResultSidebarItems(state.history)}
            : {id:"sidebar-history-list",label:"Coding history · all time",items:state.history.catalog.roots.length>0?historySidebarItems(state):localSidebarItems(state)},
        ],
        a11y:{role:"list",label:`${Math.min(state.history.visibleRootCount,state.history.catalog.roots.length)} of ${state.history.catalog.roots.length} sessions`},
        style:{flex:1,minHeight:0,width:"full"},
      }),
      historySearchField(state.history),
      ...(historySearchActive(state.history) && state.history.searchResults.length === 0 ? [Text({ key: "sidebar-search-empty", content: "No sessions match.", variant: "caption", color: "textMuted" })] : []),
      ...(state.history.catalog.roots.length === 0 && state.threads.length === 0 && !historySearchActive(state.history) ? [Text({ key: "sidebar-chats-empty", content: "No local coding history found.", variant: "body", color: "textMuted" })] : []),
      ...(accountsBox === null ? [] : [accountsBox]),
    ],
  )
}

const shellWelcome = (): View =>
  Stack(
    {
      key: "shell-welcome",
      direction: "column",
      gap: "2",
      style: { width: "full", maxWidth: columnWidth, alignSelf: "center" },
    },
    [
      Text({
        key: "shell-welcome-title",
        content: "No recent Codex chats found",
        variant: "heading",
        color: "textPrimary",
      }),
      Text({
        key: "shell-welcome-body",
        content: "Open Codex and start a top-level chat; it will appear here when Desktop next opens.",
        variant: "body",
        color: "textMuted",
      }),
    ],
  )

/** Read-only context for the currently selected local Codex history entry. */
const selectedCodexThreadDetails = (state: DesktopShellState): View | null => {
  const thread = state.threads.find(item => item.id === state.activeThreadId)
  if (thread === undefined) return null
  const fields = [
    `Updated ${thread.updatedAt.slice(0, 16).replace("T", " ")}`,
    ...(thread.createdAt === undefined ? [] : [`Started ${thread.createdAt.slice(0, 16).replace("T", " ")}`]),
    ...(thread.cwd === undefined ? [] : [thread.cwd]),
    ...(thread.model === undefined ? [] : [thread.model]),
    `${thread.notes.length} recent messages`,
  ]
  return Card(
    { key: "codex-thread-details", padding: "2", radius: "lg", style: { width: "full", maxWidth: columnWidth, alignSelf: "center", surface: "glass" } },
    [Stack({ key: "codex-thread-details-content", direction: "column", gap: "1" }, [
      Text({ key: "codex-thread-details-meta", content: fields.join(" · "), variant: "caption", color: "textMuted" }),
    ])],
  )
}

const projectHome = (state: DesktopShellState): View => {
  const parsedQuery = parseDesktopCodingCatalogQuery(state.codingSessionQuery)
  const queried = parsedQuery.state === "invalid"
    ? []
    : filterDesktopCodingCatalog(state.codingCatalog, parsedQuery.plan)
  const visible = queried.filter(session =>
    state.codingSessionFilter === "active"
      ? session.state === "active" || session.state === "idle"
      : state.codingSessionFilter === "archived"
        ? session.state === "archived"
        : session.recoveryReason !== null || session.state === "recovery_required")
  const activeCount = state.codingCatalog.sessions.filter(session =>
    session.state === "active" || session.state === "idle").length
  const recoveryCount = state.codingCatalog.sessions.filter(session =>
    session.recoveryReason !== null || session.state === "recovery_required").length
  const archivedCount = state.codingCatalog.sessions.filter(session => session.state === "archived").length
  const focusLabel = state.codingCatalog.focus.kind === "none"
    ? "No restored focus"
    : `Restored ${state.codingCatalog.focus.kind} focus`
  return Stack(
    {
      key: "workspace-home-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", maxWidth: columnWidth, alignSelf: "center", flex: 1, minHeight: 0 },
    },
    [
      Stack({ key: "workspace-home-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "workspace-home-title", content: "Coding sessions", variant: "heading", color: "textPrimary" }),
        Badge({ key: "workspace-home-authority", label: state.codingCatalog.authorityLabel, tone: "neutral" }),
      ]),
      Text({ key: "workspace-home-copy", content: "Resume the exact project, repository, worktree, and task context from this Mac.", variant: "body", color: "textMuted" }),
      Stack({ key: "workspace-home-actions", direction: "row", gap: "2", align: "center" }, [
        Button({
          key: "workspace-home-open-folder",
          label: "Add workspace",
          variant: "secondary",
          onPress: IntentRef("DesktopCodingCatalogChooseRequested"),
          a11y: { label: "Choose a workspace and create or resume its coding session" },
        }),
        Text({ key: "workspace-home-focus", content: focusLabel, variant: "caption", color: "textMuted" }),
      ]),
      Stack({ key: "workspace-home-filters", direction: "row", gap: "1", align: "center" }, [
        Button({ key: "workspace-home-filter-active", label: `Active ${activeCount}`, variant: state.codingSessionFilter === "active" ? "secondary" : "ghost", onPress: IntentRef("DesktopCodingCatalogFilterSelected", StaticPayload("active")) }),
        Button({ key: "workspace-home-filter-recovery", label: `Needs recovery ${recoveryCount}`, variant: state.codingSessionFilter === "recovery" ? "secondary" : "ghost", onPress: IntentRef("DesktopCodingCatalogFilterSelected", StaticPayload("recovery")) }),
        Button({ key: "workspace-home-filter-archived", label: `Archived ${archivedCount}`, variant: state.codingSessionFilter === "archived" ? "secondary" : "ghost", onPress: IntentRef("DesktopCodingCatalogFilterSelected", StaticPayload("archived")) }),
      ]),
      TextField({
        key: "workspace-home-query",
        value: state.codingSessionQuery,
        placeholder: "Filter by project:, repository:, or state:",
        a11y: { label: "Structured coding session search" },
        onChange: IntentRef("DesktopCodingCatalogQueryChanged", ComponentValueBinding()),
        style: { width: "full" },
      }),
      ...(parsedQuery.state === "invalid" ? [Text({
        key: "workspace-home-query-error",
        content: parsedQuery.reason,
        variant: "caption",
        color: "warning",
      })] : []),
      ...(visible.length === 0 ? [Text({
        key: "workspace-home-empty",
        content: state.codingSessionFilter === "active"
          ? "Add a workspace to create the first durable coding session."
          : state.codingSessionFilter === "recovery"
            ? "No sessions need recovery."
            : "No archived sessions.",
        variant: "body",
        color: "textMuted",
      })] : visible.map((session) => Stack(
        {
          key: `workspace-home-session-${session.sessionRef}`,
          direction: "row",
          gap: "3",
          align: "center",
          style: { width: "full", padding: "3", borderColor: "borderSubtle", borderWidth: 1, borderRadius: "lg" },
        },
        [
          Stack({ key: `workspace-home-session-copy-${session.sessionRef}`, direction: "column", gap: "0.5", style: { flex: 1, minWidth: 0 } }, [
            Text({ key: `workspace-home-session-title-${session.sessionRef}`, content: session.projectLabel, variant: "title", color: "textPrimary" }),
            Text({ key: `workspace-home-session-context-${session.sessionRef}`, content: `${session.repositoryLabel} · ${session.worktreeLabel} · ${formatRelativeTimestamp(session.lastActiveAt)}`, variant: "caption", color: "textMuted" }),
          ]),
          Badge({
            key: `workspace-home-session-state-${session.sessionRef}`,
            label: session.recoveryReason === null ? session.state : `Recovery · ${session.recoveryReason.replaceAll("_", " ")}`,
            tone: session.recoveryReason !== null ? "warn" : session.state === "archived" ? "neutral" : "info",
          }),
          ...(session.recoveryReason !== null || session.state === "archived" ? [Button({
            key: `workspace-home-session-recover-${session.sessionRef}`,
            label: "Recover",
            variant: "secondary",
            onPress: IntentRef("DesktopCodingSessionRecovered", StaticPayload(session.sessionRef)),
            a11y: { label: `Recover coding session for ${session.projectLabel}` },
          })] : [Button({
            key: `workspace-home-session-open-${session.sessionRef}`,
            label: state.codingCatalog.selectedSessionRef === session.sessionRef ? "Current" : "Open",
            variant: state.codingCatalog.selectedSessionRef === session.sessionRef ? "secondary" : "ghost",
            onPress: IntentRef("DesktopCodingSessionOpened", StaticPayload(session.sessionRef)),
            a11y: { label: `Open coding session for ${session.projectLabel}` },
          })]),
          ...(session.state === "archived" ? [] : [Button({
            key: `workspace-home-session-archive-${session.sessionRef}`,
            label: "Archive",
            variant: "ghost",
            onPress: IntentRef("DesktopCodingSessionArchived", StaticPayload(session.sessionRef)),
            a11y: { label: `Archive coding session for ${session.projectLabel}` },
          })]),
        ],
      ))),
    ],
  )
}

const workspaceFiles = (state: DesktopShellState): View =>
  SplitPane({
    key: "workspace-files-split",
    orientation: "row",
    style: { flex: 1, minWidth: 0, minHeight: 0 },
    panes: [
      {
        id: "workspace-files-browser",
        min: 320,
        max: 560,
        size: 400,
        content: workspaceBrowserView(state.workspaceBrowser),
      },
      {
        id: "workspace-files-editor",
        min: 360,
        content: workspaceEditorView(state.workspaceEditor),
      },
    ],
  })

const workspaceReview = (state: DesktopShellState): View => {
  return Stack(
    { key: "workspace-review-panel", direction: "column", gap: "3", style: { width: "full", minWidth: 0, flex: 1, minHeight: 0 } },
    [
      // The typed Git/GitHub surface owns status, review, commit, and remote
      // receipts using relative paths. The prior absolute-root diff adapter is
      // deliberately not composed.
      gitPanelView(state.git),
    ],
  )
}

const fleetDesk = (state: DesktopShellState): View => {
  const dispatching = state.fleetDeployment === "dispatching"
  const accepted = state.fleetDeployment === "accepted"
  return Card(
    {
      key: "fleet-desk",
      padding: "3",
      radius: "lg",
      style: {
        width: "full",
        maxWidth: columnWidth,
        alignSelf: "center",
        borderColor: "borderSubtle",
        borderWidth: 1,
        surface: "glass",
      },
    },
    [
      Stack({ key: "fleet-desk-content", direction: "column", gap: "2" }, [
        Text({ key: "fleet-desk-title", content: "Fleet deployment brief", variant: "title", color: "textPrimary" }),
        Text({
          key: "fleet-desk-copy",
          content: "Turn a clear objective into a bounded local-Pylon brief. A real FleetRun still requires authority-backed evidence.",
          variant: "body",
          color: "textMuted",
        }),
        TextField({
          key: "fleet-objective",
          value: state.fleetObjective,
          placeholder: "What should the fleet tackle?",
          disabled: dispatching,
          a11y: { label: "Fleet deployment objective" },
          onChange: IntentRef("DesktopFleetObjectiveChanged", ComponentValueBinding()),
          style: { width: "full" },
        }),
        Stack({ key: "fleet-desk-actions", direction: "row", gap: "2", align: "center" }, [
          Button({
            key: "fleet-stage-request",
            label: dispatching ? "Dispatching…" : accepted ? "Brief dispatched" : "Dispatch brief",
            variant: "primary",
            disabled: dispatching || state.fleetObjective.trim() === "",
            onPress: IntentRef("DesktopFleetDeploymentRequested"),
            a11y: { label: "Dispatch Fleet deployment brief to local Pylon" },
          }),
          Badge({
            key: "fleet-authority-status",
            label: accepted ? "Intent accepted" : dispatching ? "Dispatching" : "Draft",
            tone: accepted ? "success" : dispatching ? "warn" : "neutral",
            a11y: { label: accepted ? "Pylon accepted Fleet intent" : dispatching ? "Fleet request dispatching" : "Fleet request is a draft" },
          }),
        ]),
      ]),
    ],
  )
}

/**
 * Floating composer on the real catalog contract: `clearOnSubmit` empties the
 * field at submit time (effect-native#72) and `pending` disables it while a
 * submission is in flight.
 *
 * EP250 owner directive (#8712): NO standing caption text under or near the
 * composer ("Don't put that shit in the UI ever."). An unavailable lane is a
 * dimmed disabled chip whose reason lives only in its accessible label (and
 * the host logs/journal) — never a visible caption line.
 */
/**
 * One harness chip inside the recessed segmented control (apps-sdk 2.6
 * port): the selected chip is the "thumb" — an elevated `surfaceRaised`
 * fill — while idle chips stay ghost; the track sits BELOW the surface
 * (`background`) with a 2px gutter, and the nested-radius rule gives chips
 * the track radius minus the gutter ("lg" 6 - 2 -> "md" 4).
 */
/**
 * Disabled-control reason popover (owner contract, EP250 #8712 verbatim:
 * "i can't tell why the Codex option is disabled in the composer. for
 * things like that you need to put a popover on hover over the disabled
 * button explaining why."). A disabled control that carries a reason is
 * wrapped in the catalog Tooltip: hover or keyboard focus reveals the
 * reason as a small overlay (styled by the host stylesheet on the shared
 * overlay recipe); the accessible label keeps carrying the reason for
 * screen readers; NO standing caption is ever rendered (the
 * no-composer-disabled-caption contract stays intact — hover-only).
 * The reason text is whatever the control state carries — never hardcoded.
 */
export const withDisabledReason = (
  key: string,
  disabled: boolean,
  reason: string | null,
  control: View,
): View =>
  disabled && reason !== null && reason !== ""
    ? Tooltip(
        { key: `${key}-reason`, content: reason, placement: { side: "top", align: "start" } },
        [control],
      )
    : control

const harnessChip = (
  state: DesktopShellState,
  harness: DesktopHarnessName,
  label: string,
): View => {
  const selected = state.selectedHarness === harness
  const lane = state.harnessLanes[harness]
  const chip = Button({
    key: `shell-harness-${harness}`,
    label,
    variant: selected ? "secondary" : "ghost",
    style: selected
      ? { backgroundColor: "surfaceRaised", borderWidth: 0, borderRadius: "md", typeScale: "label", color: "textPrimary" }
      : { borderWidth: 0, borderRadius: "md", typeScale: "label", color: "textMuted" },
    disabled: state.pending || !lane.available,
    onPress: IntentRef("DesktopHarnessSelected", StaticPayload(harness)),
    a11y: {
      label: !lane.available
        ? lane.reason ?? `${label} — unavailable`
        : selected ? `${label} harness selected` : `Target new turns at ${label}`,
    },
  })
  return withDisabledReason(
    `shell-harness-${harness}`,
    state.pending || !lane.available,
    lane.available ? null : lane.reason,
    chip,
  )
}

/**
 * The composer's trailing action control (EP250 Stop button, audit gap #9).
 * While a turn streams (`state.pending`) the evidence-gated Send is replaced by
 * an icon-only Stop that dispatches DesktopTurnInterrupted for the active lane;
 * the existing interrupt IPC path aborts the turn and the terminal turn result
 * reverts this control to Send. Idle renders the exact icon-only Send as before
 * (evidence-gated by the selected lane's availability).
 */
const composerActionControl = (state: DesktopShellState): View => {
  if (state.pending) {
    return IconButton({
      key: "shell-stop",
      icon: "Stop",
      accessibilityLabel: "Stop turn",
      onPress: IntentRef("DesktopTurnInterrupted"),
      style: { backgroundColor: "surfaceRaised", color: "textPrimary", borderRadius: "md" },
    })
  }
  const lane = state.harnessLanes[state.selectedHarness]
  // ONE icon-only send control (owner statement 2026-07-11: "airplane icon in
  // composer OUTSIDE of the button is stupid. put it in , remove text 'send'"):
  // the paper-plane glyph lives INSIDE the button — no freestanding icon, no
  // "Send" text label. The disabled reason survives only in the accessible
  // label + hover popover (no standing caption).
  return withDisabledReason(
    "shell-note",
    !lane.available,
    lane.available ? null : lane.reason ?? "Send unavailable: selected lane cannot act",
    IconButton({
      key: "shell-note",
      icon: "Plane",
      accessibilityLabel: lane.available
        ? "Send message"
        : lane.reason ?? "Send unavailable: selected lane cannot act",
      disabled: !lane.available,
      onPress: IntentRef("DesktopNoteSubmitted"),
      style: { backgroundColor: "accent", color: "textInverse", borderRadius: "md" },
    }),
  )
}

/**
 * One pending image attachment (capability I1): a bounded thumbnail with a
 * remove affordance. The `source` is a renderer-only `data:` URL (CSP allows
 * `img-src data:`); the base64 itself never renders as text.
 */
const composerImageThumbnail = (attachment: ComposerImageAttachment): View =>
  Stack(
    {
      key: `composer-image-${attachment.id}`,
      direction: "column",
      gap: "0.5",
      align: "center",
      style: {
        backgroundColor: "surfaceRaised",
        borderRadius: "md",
        borderColor: "border",
        borderWidth: 1,
        padding: "1",
      },
    },
    [
      Image({
        key: `composer-image-preview-${attachment.id}`,
        source: composerImageDataUrl(attachment),
        alt: attachment.name,
        width: 56,
        height: 56,
        fit: "cover",
        style: { borderRadius: "sm" },
      }),
      Stack(
        {
          key: `composer-image-meta-${attachment.id}`,
          direction: "row",
          gap: "1",
          align: "center",
        },
        [
          Text({
            key: `composer-image-size-${attachment.id}`,
            content: formatImageSize(attachment.sizeBytes),
            variant: "caption",
            color: "textFaint",
          }),
          IconButton({
            key: `composer-image-remove-${attachment.id}`,
            icon: "X",
            accessibilityLabel: `Remove ${attachment.name}`,
            onPress: IntentRef("DesktopComposerImageRemoved", StaticPayload(attachment.id)),
            style: { backgroundColor: "surface", color: "textMuted", borderRadius: "sm" },
          }),
        ],
      ),
    ],
  )

/** The composer attachments strip + transient rejection notice (capability I1). */
const composerImageRegion = (state: DesktopShellState): ReadonlyArray<View> => {
  const rows: View[] = []
  if (state.composerImages.length > 0) {
    rows.push(Stack(
      {
        key: "shell-composer-images",
        direction: "row",
        gap: "2",
        align: "center",
        style: { width: "full" },
        a11y: { role: "list", label: "Attached images" },
      },
      state.composerImages.map(composerImageThumbnail),
    ))
  }
  if (state.composerImageNotice !== null) {
    rows.push(Text({
      key: "shell-composer-image-notice",
      content: state.composerImageNotice,
      variant: "caption",
      color: "danger",
    }))
  }
  return rows
}

/**
 * The composer's leading attach affordance (capability I1). Opens the native
 * image picker in main; drag-drop and paste feed the same attachment state
 * from boot.ts. Disabled while pending or at the count limit — the reason lives
 * in the accessible label (no standing caption, per the composer contract).
 */
const composerAttachControl = (state: DesktopShellState): View => {
  const atLimit = !canAttachMoreImages(state.composerImages)
  const disabled = state.pending || atLimit
  return IconButton({
    key: "shell-attach-image",
    icon: "Plus",
    accessibilityLabel: atLimit
      ? "Image limit reached (8 max)"
      : "Attach image",
    disabled,
    onPress: IntentRef("DesktopComposerImagePickRequested"),
    style: { backgroundColor: "surfaceRaised", color: "textMuted", borderRadius: "md" },
  })
}

const shellComposer = (state: DesktopShellState): View =>
  Card(
    {
      key: "shell-composer",
      padding: "2",
      // Radius capped at the shared scale's xl (8) — the apps-sdk 24px
      // composer radius is deliberately NOT ported (spec "not ported" list).
      radius: "xl",
      style: {
        width: "full",
        maxWidth: columnWidth,
        alignSelf: "center",
        borderColor: "border",
        borderWidth: 1,
        marginBottom: "4",
        surface: "glass",
      },
    },
    [
      Stack(
        {
          key: "shell-harness-row",
          direction: "row",
          gap: "0.5",
          align: "center",
          style: {
            backgroundColor: "background",
            borderRadius: "lg",
            padding: "0.5",
            alignSelf: "start",
          },
        },
        [
          harnessChip(state, "fable", "Fable"),
          harnessChip(state, "codex", "Codex"),
        ],
      ),
      // Capability I1: pending image thumbnails + transient rejection notice
      // sit above the input row (empty when nothing is attached).
      ...composerImageRegion(state),
      Stack(
        {
          key: "shell-composer-row",
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          // Leading attach affordance (capability I1) — picker + drop/paste.
          composerAttachControl(state),
          TextField({
            key: "shell-input",
            value: state.input,
            // A3 queue-until-idle (EP250 wave-2): the composer stays usable
            // while a turn streams so a follow-up can be queued (submit mid-turn
            // enqueues instead of starting a new turn); the placeholder names
            // the honest semantics. The Stop button still interrupts.
            placeholder: state.pending ? "Queue a follow-up…" : "Message",
            disabled: false,
            clearOnSubmit: true,
            a11y: { label: state.pending ? "Queue a follow-up, delivered when this turn completes" : "Message" },
            onChange: IntentRef("DesktopInputChanged", ComponentValueBinding()),
            onSubmit: IntentRef("DesktopNoteSubmitted", ComponentValueBinding()),
            style: { flex: 1 },
          }),
          // Icon-only Send (idle) or Stop (streaming) — see composerActionControl.
          composerActionControl(state),
        ],
      ),
    ],
  )

/**
 * Command palette panel (apps-sdk chrome port, EP250 #8712): the floating-
 * overlay recipe — one step above surfaceRaised (`surfaceOverlay`), radius
 * `xl`, hairline `borderSubtle` edge, 6px panel gutter (spacing "1.5"),
 * ghost item rows whose inner radius is the outer minus the gutter
 * (nested-radius rule -> "sm"), and the platform keybinding caption on rows
 * that carry a canonical chord. The overlay shadow + enter/exit motion ride
 * the host stylesheet's elevation/motion custom properties.
 */
const commandPalette = (state: DesktopShellState): View => {
  const darwin = state.host.includes("darwin")
  return Card(
    {
      key: "desktop-command-palette",
      padding: "1.5",
      radius: "xl",
      style: {
        width: "full",
        maxWidth: 420,
        backgroundColor: "surfaceOverlay",
        borderColor: "borderSubtle",
        borderWidth: 1,
      },
    },
    [
      Stack({ key: "desktop-command-palette-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "desktop-command-palette-title", content: "Commands", variant: "title", color: "textPrimary" }),
        Spacer({ key: "desktop-command-palette-heading-fill", flex: true }),
        Button({
          key: "desktop-command-palette-close",
          label: "Close",
          variant: "ghost",
          style: { borderWidth: 0, borderRadius: "sm", typeScale: "label", color: "textFaint" },
          onPress: IntentRef("DesktopCommandPaletteDismissed"),
          a11y: { label: "Close command palette" },
        }),
      ]),
      ...desktopCommandRegistry.flatMap((command) => {
        const chord = formatCommandChord(command.chords, darwin)
        return [Stack(
          { key: `desktop-command-row-${command.id}`, direction: "row", gap: "2", align: "center", style: { width: "full" } },
          [
            Button({
              key: `desktop-command-${command.id}`,
              label: command.label,
              variant: "ghost",
              style: { borderWidth: 0, borderRadius: "sm", typeScale: "label", color: "textPrimary", flex: 1 },
              onPress: command.payload === null
                ? IntentRef(command.intentName)
                : IntentRef(command.intentName, StaticPayload(command.payload)),
              a11y: { label: chord === null ? command.label : `${command.label} (${chord})` },
            }),
            ...(chord === null ? [] : [Text({
              key: `desktop-command-chord-${command.id}`,
              content: chord,
              variant: "caption",
              color: "textFaint",
            })]),
          ],
        )]
      }),
    ],
  )
}

/**
 * Right-side message metadata inspector (#8712: "if I click on the message,
 * I see the metadata of the message in the right sidebar"). Same visual
 * pattern as the Codex history item inspector rail, chat-scoped: role,
 * timestamp, and — for assistant messages — every fact the host recorded on
 * the persisted note (lane, SDK-reported effective model, account ref, turn
 * ref, request id, exact token total, duration).
 */
export const chatMessageMetadataFields = (
  entry: DesktopNoteEntry,
): ReadonlyArray<Readonly<{ label: string; value: string }>> => [
  { label: "Role", value: entry.role },
  { label: "Time", value: entry.timestamp },
  ...(entry.meta?.lane === undefined ? [] : [{ label: "Lane", value: entry.meta.lane }]),
  ...(entry.meta?.model === undefined ? [] : [{ label: "Effective model", value: entry.meta.model }]),
  ...(entry.meta?.accountRef === undefined ? [] : [{ label: "Account", value: entry.meta.accountRef }]),
  ...(entry.meta?.turnRef === undefined ? [] : [{ label: "Turn", value: entry.meta.turnRef }]),
  ...(entry.meta?.requestId === undefined ? [] : [{ label: "Request", value: entry.meta.requestId }]),
  ...(entry.meta?.totalTokens === undefined ? [] : [{
    label: "Tokens (total)",
    value: entry.meta.totalTokens === null ? "not reported" : String(entry.meta.totalTokens),
  }]),
  ...(entry.meta?.durationMs === undefined ? [] : [{
    label: "Duration",
    value: `${(entry.meta.durationMs / 1000).toFixed(1)}s`,
  }]),
]

const chatMessageInspector = (entry: DesktopNoteEntry): View => {
  const fields = chatMessageMetadataFields(entry)
  return Stack(
    {
      key: "chat-message-inspector",
      direction: "column",
      gap: "2",
      style: { minWidth: 0, minHeight: 0, flex: 1 },
      a11y: { role: "region", label: "Message details" },
    },
    [
      Button({
        key: "chat-message-inspector-close",
        label: "Close",
        variant: "ghost",
        style: { borderWidth: 0, borderRadius: "md", typeScale: "label", color: "textFaint", alignSelf: "start" },
        onPress: IntentRef("DesktopMessageSelected", StaticPayload("")),
        a11y: { label: "Close message details" },
      }),
      Text({ key: "chat-message-inspector-title", content: "Message details", variant: "title", color: "textPrimary" }),
      Badge({ key: "chat-message-inspector-role", label: entry.role, tone: "neutral" }),
      Table({
        key: "chat-message-inspector-fields",
        columns: [{ id: "field", header: "Field" }, { id: "value", header: "Value" }],
        rows: fields.map((field, index) => ({
          id: String(index),
          cells: [
            Text({ key: `chat-message-inspector-field-${index}`, content: field.label, variant: "caption", color: "textMuted" }),
            Text({ key: `chat-message-inspector-value-${index}`, content: field.value, variant: "body", color: "textPrimary" }),
          ],
        })),
      }),
    ],
  )
}

/**
 * The chat transcript area. When a message inspector is open the transcript
 * and composer share the left pane of a SplitPane whose right rail is the
 * metadata inspector (the codex-history inspector pattern, chat-scoped);
 * Escape anywhere inside deselects through the same typed intent.
 */
const chatTranscriptArea = (state: DesktopShellState): ReadonlyArray<View> => {
  const graph = state.agentGraph === null
    ? []
    : [runtimeAgentGraphView({
        graph: state.agentGraph,
        expanded: state.agentGraphExpanded,
        selectedAgentRef: state.selectedAgentRef,
      })]
  // EP250 tool/question cards: system trace notes fold into typed cards
  // (one updating card per tool invocation); everything else stays a note.
  const transcript = Transcript({
    key: "shell-transcript",
    pinToEnd: true,
    messages: projectTranscriptEntries(state.notes).map((entry) =>
      entry.kind === "tool"
        ? toolCardMessage(entry.card, state.expandedToolCards.includes(entry.card.key))
        : entry.kind === "context-group"
          ? contextGroupMessage(entry.group, state.expandedToolCards.includes(entry.group.key))
          : entry.kind === "runtime"
            ? runtimeCardMessage(entry.note)
            : entry.kind === "question"
              ? questionCardMessage(
                  entry.note,
                  entry.note.question === undefined
                    ? undefined
                    : state.questionCards[entry.note.question.questionRef],
                  state.questionAnswerHostAvailable,
                )
              : noteMessage(entry.note)),
    style: {
      width: "full",
      maxWidth: columnWidth,
      alignSelf: "center",
      flex: 1,
      minHeight: 0,
      paddingLeft: "4",
      paddingRight: "4",
      // The 24/12/4 rhythm (card reconciliation): 12px part gap between
      // rows; user turns get their extra 24px headroom via the host
      // stylesheet's turn rule; grouped members stack at 4px.
      gap: "3",
    },
  })
  const selected = state.selectedMessageKey === null
    ? undefined
    : state.notes.find((note) => note.key === state.selectedMessageKey)
  if (selected === undefined) return [...graph, transcript, shellComposer(state)]
  return [SplitPane({
    key: "chat-message-inspector-split",
    orientation: "row",
    style: { flex: 1, minWidth: 0, minHeight: 0 },
    interactions: {
      onKey: [{ key: "Escape", preventDefault: true, intent: IntentRef("DesktopMessageSelected", StaticPayload("")) }],
    },
    panes: [
      {
        id: "chat-center",
        min: 360,
        content: Stack(
          { key: "chat-center-column", direction: "column", gap: "3", style: { flex: 1, minWidth: 0, minHeight: 0 } },
          [...graph, transcript, shellComposer(state)],
        ),
      },
      { id: "chat-message-inspector-pane", min: 280, max: 480, size: 336, content: chatMessageInspector(selected) },
    ],
  })]
}

const commandBindingSettings = (state: DesktopShellState): View => {
  const selected = state.commandBindingSelectedId === null
    ? null
    : state.commandBindings?.rows.find(value => value.commandId === state.commandBindingSelectedId) ?? null
  return Stack(
    { key: "desktop-command-bindings", direction: "column", gap: "2", padding: "3", style: { width: "full" } },
    [
      Text({ key: "desktop-command-bindings-title", content: "Keyboard shortcuts", variant: "heading", color: "textPrimary" }),
      Text({
        key: "desktop-command-bindings-copy",
        content: "Select a command, enter one shortcut, and save. Conflicts disable that shortcut until you change or remove it.",
        variant: "body",
        color: "textMuted",
      }),
      ...(state.commandBindings === null
        ? [Text({ key: "desktop-command-bindings-unavailable", content: "Keyboard shortcuts are unavailable.", variant: "caption", color: "warning" })]
        : state.commandBindings.rows.map(row => Button({
            key: `desktop-command-binding-${row.commandId}`,
            label: `${row.label} · ${row.overrideBinding ?? (row.effectiveBindings.join(" / ") || "Unassigned")}${row.conflict ? " · Conflict" : ""}`,
            variant: state.commandBindingSelectedId === row.commandId ? "secondary" : "ghost",
            onPress: IntentRef("DesktopCommandBindingSelected", StaticPayload(row.commandId)),
            a11y: { label: `Edit shortcut for ${row.label}` },
          }))),
      ...(selected === null ? [] : [
        TextField({
          key: "desktop-command-binding-draft",
          value: state.commandBindingDraft,
          placeholder: "Meta+Shift+K",
          a11y: { label: `Shortcut for ${selected.label}` },
          onChange: IntentRef("DesktopCommandBindingDraftChanged", ComponentValueBinding()),
          style: { width: "full" },
        }),
        Stack({ key: "desktop-command-binding-actions", direction: "row", gap: "2" }, [
          Button({ key: "desktop-command-binding-save", label: "Save shortcut", variant: "primary", disabled: state.commandBindingDraft.trim() === "", onPress: IntentRef("DesktopCommandBindingSaved") }),
          Button({ key: "desktop-command-binding-remove", label: "Use defaults", variant: "secondary", onPress: IntentRef("DesktopCommandBindingRemoved") }),
        ]),
      ]),
      ...(state.commandBindings?.conflicts.map(conflict => Text({
        key: `desktop-command-binding-conflict-${conflict.chord}`,
        content: `${conflict.chord} conflicts across ${conflict.commandIds.length} commands and is disabled.`,
        variant: "caption",
        color: "warning",
      })) ?? []),
      Button({ key: "desktop-command-bindings-reset", label: "Reset all shortcuts", variant: "ghost", onPress: IntentRef("DesktopCommandBindingsReset") }),
    ],
  )
}

export const desktopShellView = (state: DesktopShellState): View =>
  BackgroundGradient(
    {
      key: "desktop-liquid-backdrop",
      direction: "radial",
      from: "background",
      to: "accent",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    [Stack(
    {
      key: "shell-root",
      direction: "row",
      gap: "3",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    [
      shellSidebar(state),
      Stack(
        {
          key: "shell-main",
          direction: "column",
          gap: "3",
          style: { flex: 1, minWidth: 0, minHeight: 0 },
        },
        [
          ...(state.commandNotice === null ? [] : [Text({
            key: "desktop-command-notice",
            content: state.commandNotice,
            variant: "caption",
            color: "warning",
          })]),
          ...(state.commandPaletteOpen ? [commandPalette(state)] : []),
          ...(state.workspace === "chat" && state.history.catalog.roots.length === 0 && state.threads.length === 0 ? [shellWelcome()] : []),
          ...(state.workspace === "chat" && state.history.page !== null ? [historyWorkspaceView(state.history)] : state.workspace === "chat" ? chatTranscriptArea(state) : state.workspace === "files" ? [workspaceFiles(state)] : state.workspace === "review" ? [workspaceReview(state)] : state.workspace === "settings" ? [Stack({ key: "desktop-settings-stack", direction: "column", gap: "3", style: { width: "full", minHeight: 0 } }, [settingsView(state.settings), commandBindingSettings(state)])] : state.workspace === "fleet" ? [fleetWorkspaceView(state.fleet)] : [projectHome(state)]),
        ],
      ),
    ],
  )],
  )
