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
  IntentRef,
  Icon,
  NavRail,
  Spacer,
  SplitPane,
  Stack,
  Table,
  Text,
  TextField,
  Transcript,
  StaticPayload,
  defineIntent,
  type IntentHandlers,
  type TextView,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"
import type { DesktopMessageMeta, DesktopQuestionCard, DesktopThread } from "../chat-contract.ts"
import {
  humanizeToolInvocation,
  projectTranscriptEntries,
  toolCardIcon,
  toolResultSnippet,
  type ToolCardModel,
} from "./tool-cards.ts"
import {
  resolveLiveAgentGraphSelection,
  type LiveAgentGraphPresentation,
} from "../agent-graph-presentation.ts"
import { chatMarkdownBody } from "./markdown.ts"
import { runtimeAgentGraphView } from "./runtime-agent-graph.ts"
import type {
  DesktopWorkspaceFile,
  DesktopWorkspaceGitDiff,
  DesktopWorkspaceGitStatus,
  DesktopWorkspaceSaveResult,
  DesktopWorkspaceSnapshot,
} from "../workspace-contract.ts"
import { desktopCommandRegistry } from "./command-registry.ts"
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
import { emptyHistoryWorkspaceState, historyCatalogPageSize, historyWorkspaceIntents, historyWorkspaceView, type HistoryWorkspaceState } from "./history-workspace.ts"
import type { CodexHistoryCatalog, CodexHistoryPage } from "../codex-history-contract.ts"
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
  unavailableOpenAgentsSessionSettingsBridge,
  unavailableProviderAccountsSettingsBridge,
  type CodexSettingsBridge,
  type OpenAgentsSessionSettingsBridge,
  type ProviderAccountsSettingsBridge,
  type SettingsState,
} from "./settings.ts"

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
  workspaceSnapshot: DesktopWorkspaceSnapshot | null
  workspaceFile: DesktopWorkspaceFile | null
  /** Unsaved bounded text only; never an authority-bearing workspace state. */
  workspaceDraft: string
  workspaceBaseRevision: string | null
  workspaceSave: "idle" | "saving" | "saved" | "conflict" | "unavailable"
  workspaceGitStatus: DesktopWorkspaceGitStatus
  workspaceGitDiff: DesktopWorkspaceGitDiff | null
  commandPaletteOpen: boolean
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
  workspaceSnapshot: null,
  workspaceFile: null,
  workspaceDraft: "",
  workspaceBaseRevision: null,
  workspaceSave: "idle",
  workspaceGitStatus: { state: "unavailable" },
  workspaceGitDiff: null,
  commandPaletteOpen: false,
  historyShortcutHintsVisible: false,
  fleetDeskOpen: false,
  fleetObjective: "",
  fleetDeployment: "not_requested",
  loopProofs: 0,
  settings: initialSettingsState(),
  history: emptyHistoryWorkspaceState(),
  fleet: emptyFleetWorkspaceState(),
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
export const DesktopWorkspaceFileSelected = defineIntent("DesktopWorkspaceFileSelected", Schema.String)
export const DesktopWorkspaceDraftChanged = defineIntent("DesktopWorkspaceDraftChanged", Schema.String)
export const DesktopWorkspaceSaveRequested = defineIntent("DesktopWorkspaceSaveRequested", Schema.Null)
export const DesktopWorkspaceReloadRequested = defineIntent("DesktopWorkspaceReloadRequested", Schema.Null)
export const DesktopWorkspaceGitDiffSelected = defineIntent("DesktopWorkspaceGitDiffSelected", Schema.String)
export const DesktopCommandPaletteToggled = defineIntent("DesktopCommandPaletteToggled", Schema.Null)
export const DesktopCommandPaletteDismissed = defineIntent("DesktopCommandPaletteDismissed", Schema.Null)
export const DesktopHistoryShortcutHintsChanged = defineIntent("DesktopHistoryShortcutHintsChanged", Schema.Boolean)
export const DesktopHistoryConversationPreviewed = defineIntent("DesktopHistoryConversationPreviewed", Schema.String)

export const desktopShellIntents = [
  DesktopInputChanged,
  DesktopNoteSubmitted,
  DesktopLoopPinged,
  DesktopFleetDeskToggled,
  DesktopFleetObjectiveChanged,
  DesktopFleetDeploymentRequested,
  DesktopNewChat,
  DesktopHarnessSelected,
  DesktopChatSelected,
  DesktopMessageSelected,
  DesktopToolCardToggled,
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
  DesktopWorkspaceFileSelected,
  DesktopWorkspaceDraftChanged,
  DesktopWorkspaceSaveRequested,
  DesktopWorkspaceReloadRequested,
  DesktopWorkspaceGitDiffSelected,
  DesktopCommandPaletteToggled,
  DesktopCommandPaletteDismissed,
  DesktopHistoryShortcutHintsChanged,
  DesktopHistoryConversationPreviewed,
  ...settingsIntents,
  ...historyWorkspaceIntents,
  ...fleetWorkspaceIntents,
] as const

export type CodexHistoryHost = Readonly<{
  catalog: () => Promise<CodexHistoryCatalog | null>
  page: (threadRef: string, offset: number, limit: number) => Promise<CodexHistoryPage | null>
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

export const withWorkspaceSnapshot = (
  state: DesktopShellState,
  workspaceSnapshot: DesktopWorkspaceSnapshot | null,
): DesktopShellState => ({
  ...state,
  workspaceSnapshot,
  workspaceFile: null,
  workspaceDraft: "",
  workspaceBaseRevision: null,
  workspaceSave: "idle",
  workspaceGitStatus: { state: "unavailable" },
  workspaceGitDiff: null,
})

export const withWorkspaceFile = (
  state: DesktopShellState,
  workspaceFile: DesktopWorkspaceFile | null,
): DesktopShellState => ({
  ...state,
  workspaceFile,
  workspaceDraft: workspaceFile?.content ?? "",
  workspaceBaseRevision: workspaceFile?.revision ?? null,
  workspaceSave: "idle",
})

export const withWorkspaceDraft = (
  state: DesktopShellState,
  workspaceDraft: string,
): DesktopShellState => ({
  ...state,
  workspaceDraft,
  workspaceSave: state.workspaceSave === "conflict" ? "conflict" : "idle",
})

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
  if (trimmed === "") return state
  return {
    ...state,
    input: "",
    pending: true,
    notes: [
      ...state.notes,
      { key: `pending-${state.notes.length}`, role: "user", text: trimmed, timestamp },
    ],
  }
}

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
    onUpdate?: (thread: DesktopThread) => void
  }>) => Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>>
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
        questionRef: string
        answers: ReadonlyArray<QuestionAnswer>
      }>) => Promise<unknown>)
    | null
}>

export type WorkspaceHost = Readonly<{
  summary: () => Promise<DesktopWorkspaceSnapshot | null>
  choose: () => Promise<DesktopWorkspaceSnapshot | null>
  readFile: (path: string) => Promise<DesktopWorkspaceFile | null>
  saveFile: (input: Readonly<{ path: string; content: string; expectedRevision: string }>) => Promise<DesktopWorkspaceSaveResult>
  gitStatus: () => Promise<DesktopWorkspaceGitStatus>
  gitDiff: (path: string) => Promise<DesktopWorkspaceGitDiff>
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
    summary: async () => null,
    choose: async () => null,
    readFile: async () => null,
    saveFile: async () => ({ state: "unavailable", message: "Workspace saving is unavailable." }),
    gitStatus: async () => ({ state: "unavailable" }),
    gitDiff: async () => ({ state: "unavailable", message: "Git review is unavailable." }),
  },
  codexBridge: CodexSettingsBridge = unavailableCodexSettingsBridge,
  settingsSleep?: (ms: number) => Promise<void>,
  openAgentsBridge: OpenAgentsSessionSettingsBridge = unavailableOpenAgentsSessionSettingsBridge,
  historyHost: CodexHistoryHost = { catalog: async () => null, page: async () => null },
  fleetBridge: FleetAccountsBridge = unavailableFleetAccountsBridge,
  providerAccountsBridge: ProviderAccountsSettingsBridge = unavailableProviderAccountsSettingsBridge,
  codingCatalogHost: CodingCatalogHost = unavailableCodingCatalogHost,
  questionHost: QuestionHost = { answer: null },
): IntentHandlers<typeof desktopShellIntents> => {
  const settingsHandlers = makeSettingsHandlers(state, codexBridge, openAgentsBridge, settingsSleep, undefined, providerAccountsBridge)
  /**
   * Hands one completed answer set to the typed bridge. Marks the card
   * locally answered first so the collapsed state renders immediately; the
   * runtime's question_resolved event stays the outcome authority. An
   * explicit typed rejection (`false`: unknown ref, already settled, no
   * matching question) reverts the local answered mark — the card returns to
   * pending with the selection retained rather than lying "Answered".
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
      yield* SubscriptionRef.update(state, (current) =>
        withQuestionAnswered(current, card.questionRef, answers))
      const result = yield* Effect.promise(() =>
        answer({ turnRef: card.turnRef, questionRef: card.questionRef, answers }).catch(() => null))
      if (result === false) {
        yield* SubscriptionRef.update(state, (current) =>
          withQuestionAnswerRejected(current, card.questionRef))
      }
    })
  return ({
  ...settingsHandlers,
  ...makeFleetWorkspaceHandlers(state, fleetBridge, () => settingsHandlers.DesktopSettingsToggled()),
  DesktopInputChanged: (value) =>
    SubscriptionRef.update(state, (current) => withInput(current, value)),
  DesktopNoteSubmitted: (value) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (current.pending || current.activeThreadId === null) return
      // Evidence-gated send (#8712): an unavailable selected lane must not
      // accept the action — the composer keeps the draft and the caption
      // already names the reason. Never substitute another lane silently.
      if (!current.harnessLanes[current.selectedHarness].available) return
      const message =
        typeof value === "string" && value.trim() !== "" ? value : current.input
      yield* SubscriptionRef.set(state, withNote(current, message, now()))
      const result = yield* Effect.promise(() => chat.sendMessage({
        id: current.activeThreadId!,
        message,
        harness: current.selectedHarness,
        onUpdate: thread => {
          Effect.runFork(SubscriptionRef.update(state, next =>
            next.activeThreadId === thread.id
              ? { ...withChatSelected(next, thread), pending: true }
              : next))
        },
      }))
      yield* SubscriptionRef.update(state, (next) => withTurnResult(next, result, now()))
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
    const workspaceSnapshot = yield* Effect.promise(workspaceHost.summary)
    yield* SubscriptionRef.update(state, (current): DesktopShellState => ({
      ...withWorkspaceSnapshot(current, workspaceSnapshot),
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
    const workspaceSnapshot = yield* Effect.promise(workspaceHost.summary)
    yield* SubscriptionRef.update(state, current => ({
      ...withWorkspaceSnapshot(current, workspaceSnapshot),
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
    const page = yield* Effect.promise(() => historyHost.page(id, 0, 50))
    if (page) { yield* SubscriptionRef.update(state, (current): DesktopShellState => { if(current.history.pendingThreadRef!==id)return current; const expandedThreadRefs=page.agents.filter(agent=>agent.descendantCount>0).map(agent=>agent.threadRef); const history={ ...current.history, page, selectedItemRef: null, expandedThreadRefs, pendingThreadRef:null }; historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:null,railCollapsed:history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs}); return { ...current, workspace: "chat", history } }) }
    else yield* SubscriptionRef.update(state,current=>current.history.pendingThreadRef===id?({...current,history:{...current.history,pendingThreadRef:null}}):current)
  }),
  HistoryAgentSelected: (id) => Effect.gen(function* () {
    const page = yield* Effect.promise(() => historyHost.page(id, 0, 50))
    if (page) { yield* SubscriptionRef.update(state, current => { const history={ ...current.history, page, selectedItemRef: null }; historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:null,railCollapsed:history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:history.expandedThreadRefs}); return { ...current, history } }) }
  }),
  HistoryItemSelected: (id) => SubscriptionRef.update(state, current => { const selectedItemRef=id===""||current.history.selectedItemRef===id?null:id; const page=current.history.page;if(page)historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef,railCollapsed:current.history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:current.history.expandedThreadRefs});return { ...current, history: { ...current.history, selectedItemRef } } }),
  HistoryPageRequested: (offset) => Effect.gen(function* () {
    const selected = (yield* SubscriptionRef.get(state)).history.page?.selectedThreadRef
    if (!selected) return
    const page = yield* Effect.promise(() => historyHost.page(selected, offset, 50))
    if (page) { yield* SubscriptionRef.update(state, current => { const history={ ...current.history, page, selectedItemRef: null }; historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:null,railCollapsed:history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:history.expandedThreadRefs}); return { ...current, history } }) }
  }),
  HistoryInspectorToggled: () => SubscriptionRef.update(state, current => { const railCollapsed=!current.history.railCollapsed;const page=current.history.page;if(page)historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:current.history.selectedItemRef,railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:current.history.expandedThreadRefs});return { ...current, history: { ...current.history, railCollapsed } } }),
  HistoryAgentExpandedToggled: (id) => SubscriptionRef.update(state,current=>{const expandedThreadRefs=current.history.expandedThreadRefs.includes(id)?current.history.expandedThreadRefs.filter(ref=>ref!==id):[...current.history.expandedThreadRefs,id];const page=current.history.page;if(page)historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef:current.history.selectedItemRef,railCollapsed:current.history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs});return {...current,history:{...current.history,expandedThreadRefs}}}),
  HistoryCatalogMoreRequested: () => SubscriptionRef.update(state,current=>({...current,history:{...current.history,visibleRootCount:Math.min(current.history.catalog.roots.length,current.history.visibleRootCount+historyCatalogPageSize)}})),
  DesktopWorkspaceSelected: (workspace) =>
    Effect.gen(function* () {
      yield* SubscriptionRef.update(state, (current) => withWorkspace(current, workspace))
      if (workspace === "fleet") {
        yield* refreshFleetAccounts(state, fleetBridge)
      }
      if (workspace === "home" || workspace === "files" || workspace === "review") {
        const snapshot = yield* Effect.promise(workspaceHost.summary)
        yield* SubscriptionRef.update(state, (current) => withWorkspaceSnapshot(current, snapshot))
        if (workspace === "home") {
          const codingCatalog = yield* Effect.promise(codingCatalogHost.snapshot)
          yield* SubscriptionRef.update(state, current => ({ ...current, codingCatalog }))
        }
        if (workspace === "review") {
          const gitStatus = yield* Effect.promise(workspaceHost.gitStatus)
          yield* SubscriptionRef.update(state, (current) => ({ ...current, workspaceGitStatus: gitStatus }))
        }
      }
    }),
  DesktopWorkspacePickerRequested: () =>
    Effect.gen(function* () {
      const snapshot = yield* Effect.promise(workspaceHost.choose)
      yield* SubscriptionRef.update(state, (current) => withWorkspaceSnapshot(current, snapshot))
    }),
  DesktopWorkspaceFileSelected: (path) =>
    Effect.gen(function* () {
      const file = yield* Effect.promise(() => workspaceHost.readFile(path))
      yield* SubscriptionRef.update(state, (current) => withWorkspaceFile(current, file))
    }),
  DesktopWorkspaceDraftChanged: (value) =>
    SubscriptionRef.update(state, (current) => withWorkspaceDraft(current, value)),
  DesktopWorkspaceSaveRequested: () =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const file = current.workspaceFile
      if (
        file === null ||
        file.truncated ||
        current.workspaceBaseRevision === null ||
        current.workspaceSave === "saving" ||
        current.workspaceSave === "conflict"
      ) return
      yield* SubscriptionRef.update(state, (next): DesktopShellState => ({ ...next, workspaceSave: "saving" }))
      const result = yield* Effect.promise(() => workspaceHost.saveFile({
        path: file.path,
        content: current.workspaceDraft,
        expectedRevision: current.workspaceBaseRevision!,
      }))
      yield* SubscriptionRef.update(state, (next): DesktopShellState => {
        if (result.state === "saved") return { ...withWorkspaceFile(next, result.file), workspaceSave: "saved" }
        if (result.state === "conflict") {
          // Preserve the editor's draft, disable save, and require an explicit
          // reload rather than silently replacing or overwriting work.
          return { ...next, workspaceFile: result.file, workspaceSave: "conflict" }
        }
        return { ...next, workspaceSave: "unavailable" }
      })
    }),
  DesktopWorkspaceReloadRequested: () =>
    SubscriptionRef.update(state, (current) => withWorkspaceFile(current, current.workspaceFile)),
  DesktopWorkspaceGitDiffSelected: (relativePath) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (current.workspaceSnapshot === null) return
      const root = current.workspaceSnapshot.root
      const diff = yield* Effect.promise(() => workspaceHost.gitDiff(`${root}/${relativePath}`))
      yield* SubscriptionRef.update(state, (next) => ({ ...next, workspaceGitDiff: diff }))
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
    // Zero padding + zero border + caption scale + muted color: a dim,
    // line-height text affordance (the renderer's default button chrome is a
    // 1px bordered chip — too loud for a per-row affordance).
    style: { padding: "0", borderWidth: 0, typeScale: "caption", color: "textMuted" },
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
export const toolCardMessage = (card: ToolCardModel, expanded: boolean): TranscriptMessage => {
  const human = humanizeToolInvocation(card.toolName, card.argsSummary)
  const chip = card.status === "running"
    ? { label: "Running", tone: "neutral" as const }
    : card.status === "ok"
      ? { label: "OK", tone: "success" as const }
      : { label: "Failed", tone: "danger" as const }
  return {
    key: `tool-${card.key}`,
    role: "tool",
    timestamp: card.timestamp,
    // One column Stack so detail/result/raw lines stack (Text lowers to an
    // inline span in the DOM renderer — bare siblings would run together).
    body: [Stack(
      { key: `tool-card-${card.key}`, direction: "column", gap: "1", align: "start" },
      [
        Stack(
          { key: `tool-header-${card.key}`, direction: "row", gap: "2", align: "center" },
          [
            Icon({ key: `tool-icon-${card.key}`, name: toolCardIcon(card.toolName), size: "sm", color: "accent" }),
            Text({ key: `tool-title-${card.key}`, content: human.title, variant: "label", color: "textPrimary" }),
            Badge({
              key: `tool-status-${card.key}`,
              label: chip.label,
              tone: chip.tone,
              a11y: { label: `${human.title} ${chip.label}` },
            }),
          ],
        ),
        ...(human.detail === "" ? [] : [
          Text({ key: `tool-detail-${card.key}`, content: human.detail, variant: "body", color: "textPrimary" }),
        ]),
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
        ...(expanded ? [
          Text({
            key: `tool-raw-args-${card.key}`,
            content: card.argsSummary === "" ? "(no recorded arguments)" : card.argsSummary,
            variant: "caption",
            color: "textMuted",
          }),
          ...(card.resultSummary === null ? [] : [Text({
            key: `tool-raw-result-${card.key}`,
            content: card.resultSummary,
            variant: "caption",
            color: "textMuted",
          })]),
        ] : []),
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
      : outcome === "timeout"
        ? "Timed out — no answer was sent."
        : "Denied — the question was dismissed."
    return {
      key: base,
      role: "tool",
      timestamp: note.timestamp,
      body: [Stack({ key: `${base}-resolved-card`, direction: "column", gap: "1", align: "start" }, [
        Stack({ key: `${base}-resolved-header`, direction: "row", gap: "2", align: "center" }, [
          Badge({
            key: `${base}-outcome`,
            label: outcome === "answered" ? "Answered" : outcome === "timeout" ? "Timed out" : "Denied",
            tone: outcome === "answered" ? "success" : "neutral",
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

const historySidebarItems = (state: DesktopShellState) => {
  const roots=state.history.catalog.roots.slice(0,state.history.visibleRootCount)
  const rows=roots.map((thread,index) => ({
    id:`sidebar-thread-${thread.threadRef}`,
    label:thread.title,
    meta:state.historyShortcutHintsVisible ? (index < 9 ? String(index + 1) : "") : formatRelativeTimestamp(thread.updatedAt),
    accessibilityLabel:`Open historical chat ${thread.title}, ${thread.descendantCount} descendant agents`,
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

const shellSidebar = (state: DesktopShellState): View =>
  Stack(
    {
      key: "shell-sidebar",
      direction: "column",
      gap: "2",
      style: { height: "full", minHeight: 0, surface: "glass" },
    },
    [
      Stack({ key: "sidebar-brand-row", direction: "row", gap: "2", align: "center" }, [
        Icon({ key: "sidebar-brand-icon", name: "Terminal", size: "sm", color: "accent" }),
        Text({ key: "sidebar-brand", content: "OpenAgents", variant: "title", color: "textPrimary" }),
      ]),
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
          {id:"sidebar-history-list",label:"Codex history · all time",items:state.history.catalog.roots.length>0?historySidebarItems(state):localSidebarItems(state)},
        ],
        a11y:{role:"list",label:`${Math.min(state.history.visibleRootCount,state.history.catalog.roots.length)} of ${state.history.catalog.roots.length} Codex conversations`},
        style:{flex:1,minHeight:0,width:"full"},
      }),
      ...(state.history.catalog.roots.length === 0 && state.threads.length === 0 ? [Text({ key: "sidebar-chats-empty", content: "No local Codex history found.", variant: "body", color: "textMuted" })] : []),
    ],
  )

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
          style: { width: "full", padding: "3", borderColor: "border", borderWidth: 1, borderRadius: "lg" },
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
  Stack(
    {
      key: "workspace-files-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", minWidth: 0, flex: 1, minHeight: 0 },
    },
    [
      Stack({ key: "workspace-files-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "workspace-files-title", content: state.workspaceSnapshot?.label ?? "Files", variant: "heading", color: "textPrimary" }),
        Button({
          key: "workspace-files-choose",
          label: "Choose folder",
          variant: "ghost",
          onPress: IntentRef("DesktopWorkspacePickerRequested"),
          a11y: { label: "Choose local workspace folder" },
        }),
      ]),
      ...(state.workspaceSnapshot === null ? [Text({ key: "workspace-files-empty", content: "Choose a local folder to inspect its files.", variant: "body", color: "textMuted" })] : [
        Text({ key: "workspace-files-status", content: state.workspaceSnapshot.git === "clean" ? "No local changes" : state.workspaceSnapshot.git === "changed" ? "Local changes" : "Git status unavailable", variant: "caption", color: "textMuted" }),
        Stack({ key: "workspace-files-layout", direction: "row", gap: "3", style: { width: "full", flex: 1, minHeight: 0 } }, [
          Stack({ key: "workspace-files-list", direction: "column", gap: "1", style: { minWidth: 240, maxWidth: 320, flex: 1 } }, state.workspaceSnapshot.entries.map((entry) => Button({
            key: `workspace-file-${entry.path}`,
            label: entry.kind === "directory" ? `${entry.name}/` : entry.name,
            variant: "ghost",
            disabled: entry.kind === "directory",
            onPress: IntentRef("DesktopWorkspaceFileSelected", StaticPayload(entry.path)),
            a11y: { label: entry.kind === "directory" ? `Folder ${entry.name}` : `Open file ${entry.name}` },
          }))),
          Card({ key: "workspace-file-preview", padding: "3", radius: "lg", style: { flex: 2, minWidth: 0, surface: "glass" } }, [
            Text({ key: "workspace-file-preview-title", content: state.workspaceFile?.path ?? "Select a file", variant: "caption", color: "textMuted" }),
            ...(state.workspaceFile === null ? [
              Text({ key: "workspace-file-preview-empty", content: "Choose a bounded text file to inspect or edit.", variant: "body", color: "textMuted" }),
            ] : state.workspaceFile.truncated ? [
              Text({ key: "workspace-file-preview-truncated", content: "This file is too large to edit safely. It remains read-only.", variant: "body", color: "warning" }),
              Text({ key: "workspace-file-preview-content", content: state.workspaceFile.content, variant: "body", color: "textPrimary" }),
            ] : [
              TextField({
                key: "workspace-file-editor",
                value: state.workspaceDraft,
                placeholder: "File contents",
                disabled: state.workspaceSave === "saving" || state.workspaceSave === "conflict",
                a11y: { label: "Workspace file editor" },
                onChange: IntentRef("DesktopWorkspaceDraftChanged", ComponentValueBinding()),
              }),
              Stack({ key: "workspace-file-actions", direction: "row", gap: "2", align: "center" }, [
                Button({
                  key: "workspace-file-save",
                  label: state.workspaceSave === "saving" ? "Saving…" : "Save",
                  variant: "primary",
                  disabled: state.workspaceSave === "saving" || state.workspaceSave === "conflict",
                  onPress: IntentRef("DesktopWorkspaceSaveRequested"),
                  a11y: { label: "Save workspace file" },
                }),
                ...(state.workspaceSave === "conflict" ? [Button({
                  key: "workspace-file-reload",
                  label: "Reload changed file",
                  variant: "secondary",
                  onPress: IntentRef("DesktopWorkspaceReloadRequested"),
                  a11y: { label: "Reload changed workspace file" },
                })] : []),
                ...(state.workspaceSave === "saved" ? [Text({ key: "workspace-file-saved", content: "Saved", variant: "caption", color: "success" })] : []),
                ...(state.workspaceSave === "conflict" ? [Text({ key: "workspace-file-conflict", content: "Changed elsewhere. Reload before saving.", variant: "caption", color: "warning" })] : []),
                ...(state.workspaceSave === "unavailable" ? [Text({ key: "workspace-file-unavailable", content: "Save unavailable. The file was not changed.", variant: "caption", color: "warning" })] : []),
              ]),
            ]),
          ]),
        ]),
      ]),
    ],
  )

const workspaceReview = (state: DesktopShellState): View => {
  const statusRows: View[] = state.workspaceGitStatus.state === "unavailable"
    ? [Text({ key: "workspace-review-unavailable", content: "Git status is unavailable for this workspace.", variant: "body", color: "textMuted" })]
    : state.workspaceGitStatus.changes.length === 0
      ? [Text({ key: "workspace-review-clean", content: "No local changes", variant: "body", color: "textMuted" })]
      : state.workspaceGitStatus.changes.map((change) => Button({
          key: `workspace-review-change-${change.path}`,
          label: `${change.kind} · ${change.path}`,
          variant: "ghost",
          onPress: IntentRef("DesktopWorkspaceGitDiffSelected", StaticPayload(change.path)),
          a11y: { label: `Review ${change.kind} file ${change.path}` },
        }))
  if (state.workspaceGitStatus.state === "available" && state.workspaceGitStatus.truncated) {
    statusRows.push(Text({ key: "workspace-review-truncated", content: "Only the first bounded set of changes is shown.", variant: "caption", color: "warning" }))
  }
  const diffRows: View[] = state.workspaceGitDiff === null
    ? []
    : state.workspaceGitDiff.state === "available"
      ? [Card({ key: "workspace-review-diff", padding: "3", radius: "lg", style: { width: "full", surface: "glass" } }, [
          Text({ key: "workspace-review-diff-path", content: state.workspaceGitDiff.path, variant: "caption", color: "textMuted" }),
          Text({ key: "workspace-review-diff-content", content: state.workspaceGitDiff.content, variant: "body", color: "textPrimary" }),
        ])]
      : [Text({ key: "workspace-review-diff-unavailable", content: state.workspaceGitDiff.message, variant: "body", color: "warning" })]
  return Stack(
    { key: "workspace-review-panel", direction: "column", gap: "3", style: { width: "full", minWidth: 0, flex: 1, minHeight: 0 } },
    [Text({ key: "workspace-review-title", content: "Changes", variant: "heading", color: "textPrimary" }), ...statusRows, ...diffRows],
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
        borderColor: "border",
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
const shellComposer = (state: DesktopShellState): View =>
  Card(
    {
      key: "shell-composer",
      padding: "2",
      radius: "lg",
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
          gap: "1",
          align: "center",
          style: { width: "full" },
        },
        [
          Button({
            key: "shell-harness-fable",
            label: "Fable",
            variant: state.selectedHarness === "fable" ? "secondary" : "ghost",
            disabled: state.pending || !state.harnessLanes.fable.available,
            onPress: IntentRef("DesktopHarnessSelected", StaticPayload("fable")),
            a11y: {
              label: !state.harnessLanes.fable.available
                ? state.harnessLanes.fable.reason ?? "Fable — unavailable"
                : state.selectedHarness === "fable" ? "Fable harness selected" : "Target new turns at Fable",
            },
          }),
          Button({
            key: "shell-harness-codex",
            label: "Codex",
            variant: state.selectedHarness === "codex" ? "secondary" : "ghost",
            disabled: state.pending || !state.harnessLanes.codex.available,
            onPress: IntentRef("DesktopHarnessSelected", StaticPayload("codex")),
            a11y: {
              label: !state.harnessLanes.codex.available
                ? state.harnessLanes.codex.reason ?? "Codex — unavailable"
                : state.selectedHarness === "codex" ? "Codex harness selected" : "Target new turns at Codex",
            },
          }),
        ],
      ),
      Stack(
        {
          key: "shell-composer-row",
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          TextField({
            key: "shell-input",
            value: state.input,
            placeholder: "Message",
            disabled: state.pending,
            clearOnSubmit: true,
            a11y: { label: "Message" },
            onChange: IntentRef("DesktopInputChanged", ComponentValueBinding()),
            onSubmit: IntentRef("DesktopNoteSubmitted", ComponentValueBinding()),
            style: { flex: 1 },
          }),
          Icon({ key: "shell-send-icon", name: "Plane", size: "sm", color: "accent" }),
          Button({
            key: "shell-note",
            label: "Send",
            variant: "primary",
            disabled: state.pending || !state.harnessLanes[state.selectedHarness].available,
            onPress: IntentRef("DesktopNoteSubmitted"),
            a11y: {
              label: state.harnessLanes[state.selectedHarness].available
                ? "Send the typed message"
                : state.harnessLanes[state.selectedHarness].reason ?? "Send unavailable: selected lane cannot act",
            },
          }),
        ],
      ),
    ],
  )

const commandPalette = (): View =>
  Card(
    {
      key: "desktop-command-palette",
      padding: "3",
      radius: "lg",
      style: {
        width: "full",
        maxWidth: 420,
        surface: "glass",
        borderColor: "border",
        borderWidth: 1,
      },
    },
    [
      Stack({ key: "desktop-command-palette-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "desktop-command-palette-title", content: "Commands", variant: "heading", color: "textPrimary" }),
        Spacer({ key: "desktop-command-palette-heading-fill", flex: true }),
        Button({
          key: "desktop-command-palette-close",
          label: "Close",
          variant: "ghost",
          onPress: IntentRef("DesktopCommandPaletteDismissed"),
          a11y: { label: "Close command palette" },
        }),
      ]),
      ...desktopCommandRegistry.map((command) => Button({
        key: `desktop-command-${command.id}`,
        label: command.label,
        variant: "ghost",
        onPress: command.payload === null
          ? IntentRef(command.intentName)
          : IntentRef(command.intentName, StaticPayload(command.payload)),
        a11y: { label: command.label },
      })),
    ],
  )

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
        onPress: IntentRef("DesktopMessageSelected", StaticPayload("")),
        a11y: { label: "Close message details" },
      }),
      Text({ key: "chat-message-inspector-title", content: "Message details", variant: "heading", color: "textPrimary" }),
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
      gap: "5",
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
          ...(state.commandPaletteOpen ? [commandPalette()] : []),
          ...(state.workspace === "chat" && state.history.catalog.roots.length === 0 && state.threads.length === 0 ? [shellWelcome()] : []),
          ...(state.workspace === "chat" && state.history.page !== null ? [historyWorkspaceView(state.history)] : state.workspace === "chat" ? chatTranscriptArea(state) : state.workspace === "files" ? [workspaceFiles(state)] : state.workspace === "review" ? [workspaceReview(state)] : state.workspace === "settings" ? [settingsView(state.settings)] : state.workspace === "fleet" ? [fleetWorkspaceView(state.fleet)] : [projectHome(state)]),
        ],
      ),
    ],
  )],
  )
