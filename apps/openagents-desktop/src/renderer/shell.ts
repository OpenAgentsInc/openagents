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
  IconButton,
  List,
  Spacer,
  Stack,
  Text,
  TextField,
  Transcript,
  StaticPayload,
  defineIntent,
  type IntentHandlers,
  type KeyedView,
  type TextView,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"
import type { DesktopThread } from "../chat-contract.ts"
import type {
  DesktopWorkspaceFile,
  DesktopWorkspaceGitDiff,
  DesktopWorkspaceGitStatus,
  DesktopWorkspaceSaveResult,
  DesktopWorkspaceSnapshot,
} from "../workspace-contract.ts"
import { desktopCommandRegistry } from "./command-registry.ts"
import { emptyHistoryWorkspaceState, historyCatalogPageSize, historyWorkspaceIntents, historyWorkspaceView, type HistoryWorkspaceState } from "./history-workspace.ts"
import type { CodexHistoryCatalog, CodexHistoryPage } from "../codex-history-contract.ts"

import {
  initialSettingsState,
  makeSettingsHandlers,
  settingsIntents,
  settingsView,
  unavailableCodexSettingsBridge,
  unavailableOpenAgentsSessionSettingsBridge,
  type CodexSettingsBridge,
  type OpenAgentsSessionSettingsBridge,
  type SettingsState,
} from "./settings.ts"

export type DesktopNoteEntry = Readonly<{
  key: string
  role: "user" | "assistant" | "system"
  text: string
  /** Preformatted display timestamp (the catalog ships no date formatting). */
  timestamp: string
}>

export const desktopWorkspaceNames = ["chat", "home", "files", "review", "terminal", "inbox", "settings"] as const
export type DesktopWorkspaceName = (typeof desktopWorkspaceNames)[number]

export type DesktopShellState = Readonly<{
  /** Host identity decoded from the preload bridge ("electron/darwin" etc.). */
  host: string
  input: string
  /** True while a submission is in flight; the composer disables itself. */
  pending: boolean
  notes: ReadonlyArray<DesktopNoteEntry>
  threads: ReadonlyArray<DesktopThread>
  activeThreadId: string | null
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
  threads: [],
  activeThreadId: null,
  workspace: "chat",
  workspaceSnapshot: null,
  workspaceFile: null,
  workspaceDraft: "",
  workspaceBaseRevision: null,
  workspaceSave: "idle",
  workspaceGitStatus: { state: "unavailable" },
  workspaceGitDiff: null,
  commandPaletteOpen: false,
  fleetDeskOpen: false,
  fleetObjective: "",
  fleetDeployment: "not_requested",
  loopProofs: 0,
  settings: initialSettingsState(),
  history: emptyHistoryWorkspaceState(),
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
export const DesktopChatSelected = defineIntent("DesktopChatSelected", Schema.String)
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

export const desktopShellIntents = [
  DesktopInputChanged,
  DesktopNoteSubmitted,
  DesktopLoopPinged,
  DesktopFleetDeskToggled,
  DesktopFleetObjectiveChanged,
  DesktopFleetDeploymentRequested,
  DesktopNewChat,
  DesktopChatSelected,
  DesktopWorkspaceSelected,
  DesktopWorkspacePickerRequested,
  DesktopWorkspaceFileSelected,
  DesktopWorkspaceDraftChanged,
  DesktopWorkspaceSaveRequested,
  DesktopWorkspaceReloadRequested,
  DesktopWorkspaceGitDiffSelected,
  DesktopCommandPaletteToggled,
  DesktopCommandPaletteDismissed,
  ...settingsIntents,
  ...historyWorkspaceIntents,
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
  workspace: "chat",
  fleetDeskOpen: false,
  fleetObjective: "",
  fleetDeployment: "not_requested",
  commandPaletteOpen: false,
})

export const withChatSelected = (state: DesktopShellState, thread: DesktopThread): DesktopShellState => ({
  ...state,
  notes: thread.notes,
  activeThreadId: thread.id,
  fleetDeskOpen: false,
  workspace: "chat",
  commandPaletteOpen: false,
})

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

export type ChatHost = Readonly<{
  listThreads: () => Promise<ReadonlyArray<DesktopThread>>
  newThread: () => Promise<DesktopThread | null>
  openThread: (id: string) => Promise<DesktopThread | null>
  hydrateThread?: (id: string) => Promise<DesktopThread | null>
  sendMessage: (input: Readonly<{ id: string; message: string }>) => Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>>
}>

export type WorkspaceHost = Readonly<{
  summary: () => Promise<DesktopWorkspaceSnapshot | null>
  choose: () => Promise<DesktopWorkspaceSnapshot | null>
  readFile: (path: string) => Promise<DesktopWorkspaceFile | null>
  saveFile: (input: Readonly<{ path: string; content: string; expectedRevision: string }>) => Promise<DesktopWorkspaceSaveResult>
  gitStatus: () => Promise<DesktopWorkspaceGitStatus>
  gitDiff: (path: string) => Promise<DesktopWorkspaceGitDiff>
}>

export const withThreads = (state: DesktopShellState, threads: ReadonlyArray<DesktopThread>): DesktopShellState => {
  const active = state.activeThreadId === null ? threads[0] : threads.find((thread) => thread.id === state.activeThreadId)
  return { ...state, threads: threads.slice(0, 5), activeThreadId: active?.id ?? null, notes: active?.notes ?? state.notes }
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
): IntentHandlers<typeof desktopShellIntents> => ({
  ...makeSettingsHandlers(state, codexBridge, openAgentsBridge, settingsSleep),
  DesktopInputChanged: (value) =>
    SubscriptionRef.update(state, (current) => withInput(current, value)),
  DesktopNoteSubmitted: (value) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (current.pending || current.activeThreadId === null) return
      const message =
        typeof value === "string" && value.trim() !== "" ? value : current.input
      yield* SubscriptionRef.set(state, withNote(current, message, now()))
      const result = yield* Effect.promise(() => chat.sendMessage({ id: current.activeThreadId!, message }))
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
  DesktopChatSelected: (id) => Effect.gen(function* () {
    yield* SubscriptionRef.update(state, current => ({ ...current, activeThreadId: id, notes: [] }))
    const thread = yield* Effect.promise(() => chat.openThread(id)); if (!thread) return
    yield* SubscriptionRef.update(state, (current) => withChatSelected(current, thread))
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
  HistoryItemSelected: (id) => SubscriptionRef.update(state, current => { const selectedItemRef=id===""?null:id; const page=current.history.page;if(page)historyHost.save?.({rootThreadRef:page.rootThreadRef,selectedThreadRef:page.selectedThreadRef,offset:page.offset,selectedItemRef,railCollapsed:current.history.railCollapsed,anchorItemRef:page.items[0]?.itemRef??null,expandedThreadRefs:current.history.expandedThreadRefs});return { ...current, history: { ...current.history, selectedItemRef } } }),
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
      if (workspace === "home" || workspace === "files" || workspace === "review") {
        const snapshot = yield* Effect.promise(workspaceHost.summary)
        yield* SubscriptionRef.update(state, (current) => withWorkspaceSnapshot(current, snapshot))
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
})

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
 * bubble, system muted prose). The body is only the message text.
 */
export const noteMessage = (entry: DesktopNoteEntry): TranscriptMessage => ({
  key: entry.key,
  role: entry.role,
  senderLabel:
    entry.role === "user"
      ? entry.key.startsWith("pending-") ? "YOU · PENDING" : "YOU"
      : entry.role === "assistant" ? "ASSISTANT" : "SYSTEM",
  timestamp: entry.timestamp,
  body: [
    text(
      `${entry.key}-text`,
      entry.text,
      "body",
      entry.role === "system" ? "textMuted" : "textPrimary",
    ),
  ],
})

const historySidebarItems = (state: DesktopShellState): ReadonlyArray<KeyedView> => {
  const roots=state.history.catalog.roots.slice(0,state.history.visibleRootCount)
  const rows=roots.map((thread) => Stack({ key: `sidebar-action-thread-${thread.threadRef}`, direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
    Button({key:`sidebar-thread-${thread.threadRef}`,label:thread.title,variant:"ghost",onPress:IntentRef("HistoryConversationSelected",StaticPayload(thread.threadRef)),a11y:{label:`Open historical chat ${thread.title}, ${thread.descendantCount} descendant agents`,selected:(state.history.pendingThreadRef??state.history.page?.rootThreadRef)===thread.threadRef},style:{flex:1,padding:"0",borderWidth:0,borderRadius:"none",color:"textPrimary",textAlign:"left"}}),
    Text({key:`sidebar-thread-time-${thread.threadRef}`,content:formatRelativeTimestamp(thread.updatedAt),variant:"caption",color:"textMuted",style:{textAlign:"right"}}),
  ]) as KeyedView)
  return state.history.visibleRootCount>=state.history.catalog.roots.length?rows:[...rows,Button({key:"sidebar-history-load-more",label:`Load ${Math.min(historyCatalogPageSize,state.history.catalog.roots.length-state.history.visibleRootCount)} more`,variant:"ghost",onPress:IntentRef("HistoryCatalogMoreRequested"),a11y:{label:`Load older Codex conversations, ${state.history.visibleRootCount} of ${state.history.catalog.roots.length} shown`},style:{width:"full",textAlign:"left"}}) as KeyedView]
}

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
      Stack({ key: "sidebar-workspace-dock", direction: "row", gap: "1", align: "center" }, [
        IconButton({
          key: "workspace-chat",
          icon: "Chats",
          accessibilityLabel: "Chat",
          onPress: IntentRef("DesktopWorkspaceSelected", StaticPayload("chat")),
          surface: "glass",
          style: state.workspace === "chat" ? { backgroundColor: "accent" } : {},
        }),
        IconButton({
          key: "workspace-files",
          icon: "Folder",
          accessibilityLabel: "Files",
          onPress: IntentRef("DesktopWorkspaceSelected", StaticPayload("files")),
          surface: "glass",
          style: state.workspace === "files" ? { backgroundColor: "accent" } : {},
        }),
        IconButton({
          key: "workspace-home",
          icon: "Home",
          accessibilityLabel: "Project home",
          onPress: IntentRef("DesktopWorkspaceSelected", StaticPayload("home")),
          surface: "glass",
          style: state.workspace === "home" ? { backgroundColor: "accent" } : {},
        }),
        IconButton({
          key: "shell-command-palette-toggle",
          icon: "Menu",
          accessibilityLabel: "Open command palette",
          onPress: IntentRef("DesktopCommandPaletteToggled"),
          surface: "glass",
        }),
        IconButton({
          key: "shell-settings-toggle",
          icon: "Settings",
          accessibilityLabel: state.workspace === "settings" ? "Close Settings" : "Open Settings",
          onPress: IntentRef("DesktopSettingsToggled"),
          surface: "glass",
          style: state.workspace === "settings" ? { backgroundColor: "accent" } : {},
        }),
      ]),
      Text({ key: "sidebar-chats-label", content: "Codex history · all time", variant: "caption", color: "textMuted" }),
      ...(state.history.catalog.roots.length === 0 && state.threads.length === 0 ? [Text({ key: "sidebar-chats-empty", content: "No local Codex history found.", variant: "body", color: "textMuted" })] : []),
      ...(state.history.catalog.roots.length > 0 ? [List({ key:"sidebar-history-list", virtualize:false, estimatedItemSize:28, style:{flex:1,minHeight:0,width:"full"}, a11y:{role:"list",label:`${Math.min(state.history.visibleRootCount,state.history.catalog.roots.length)} of ${state.history.catalog.roots.length} Codex conversations`} }, historySidebarItems(state))] : []),
      ...(state.history.catalog.roots.length === 0 && state.threads.length > 0 ? [List({key:"sidebar-chat-list",virtualize:state.threads.length>40,estimatedItemSize:28,style:{flex:1,minHeight:0,width:"full"},a11y:{role:"list",label:`${state.threads.length} conversations`}},state.threads.map((thread) => Stack({ key: `sidebar-action-thread-${thread.id}`, direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
        Button({ key: `sidebar-thread-${thread.id}`, label: thread.title, variant: "ghost", onPress: IntentRef("DesktopChatSelected", StaticPayload(thread.id)), a11y: { label: `Open chat ${thread.title}` }, style: { flex: 1, padding: "0", borderWidth: 0, borderRadius: "none", color: "textPrimary", textAlign: "left" } }),
        Text({ key: `sidebar-thread-time-${thread.id}`, content: formatRelativeTimestamp(thread.updatedAt), variant: "caption", color: "textMuted", style: { textAlign: "right" } }),
      ]) as KeyedView))] : []),
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

const projectHome = (state: DesktopShellState): View =>
  Stack(
    {
      key: "workspace-home-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", maxWidth: columnWidth, alignSelf: "center", flex: 1, minHeight: 0 },
    },
    [
      Text({ key: "workspace-home-title", content: "Recent conversations", variant: "heading", color: "textPrimary" }),
      Text({ key: "workspace-home-copy", content: "Pick up work where you left it.", variant: "body", color: "textMuted" }),
      Button({
        key: "workspace-home-open-folder",
        label: state.workspaceSnapshot === null ? "Choose folder" : `Open ${state.workspaceSnapshot.label}`,
        variant: "secondary",
        onPress: IntentRef("DesktopWorkspacePickerRequested"),
        a11y: { label: "Choose local workspace folder" },
      }),
      ...state.threads.map((thread) => Card(
        {
          key: `workspace-home-thread-${thread.id}`,
          padding: "3",
          radius: "lg",
          style: { width: "full", surface: "glass" },
        },
        [
          Text({ key: `workspace-home-thread-title-${thread.id}`, content: thread.title, variant: "body", color: "textPrimary" }),
          Text({ key: `workspace-home-thread-time-${thread.id}`, content: `Updated ${thread.updatedAt.slice(0, 16).replace("T", " ")}`, variant: "caption", color: "textMuted" }),
          Button({
            key: `workspace-home-thread-open-${thread.id}`,
            label: "Open",
            variant: "ghost",
            onPress: IntentRef("DesktopChatSelected", StaticPayload(thread.id)),
            a11y: { label: `Open chat ${thread.title}` },
          }),
        ],
      )),
    ],
  )

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
            disabled: state.pending,
            onPress: IntentRef("DesktopNoteSubmitted"),
            a11y: { label: "Send the typed message" },
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
          ...(state.workspace === "chat" && state.history.page !== null ? [historyWorkspaceView(state.history)] : state.workspace === "chat" ? [Transcript({
            key: "shell-transcript",
            pinToEnd: true,
            messages: state.notes.map(noteMessage),
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
          })] : state.workspace === "files" ? [workspaceFiles(state)] : state.workspace === "review" ? [workspaceReview(state)] : state.workspace === "settings" ? [settingsView(state.settings)] : [projectHome(state)]),
          ...(state.workspace === "chat" && state.history.page === null ? [shellComposer(state)] : []),
        ],
      ),
    ],
  )],
  )
