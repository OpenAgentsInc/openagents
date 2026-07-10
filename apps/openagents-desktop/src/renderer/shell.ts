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
  Spacer,
  Stack,
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
import type { DesktopThread } from "../chat-contract.ts"

export type DesktopNoteEntry = Readonly<{
  key: string
  role: "user" | "assistant" | "system"
  text: string
  /** Preformatted display timestamp (the catalog ships no date formatting). */
  timestamp: string
}>

export type DesktopShellState = Readonly<{
  /** Host identity decoded from the preload bridge ("electron/darwin" etc.). */
  host: string
  input: string
  /** True while a submission is in flight; the composer disables itself. */
  pending: boolean
  notes: ReadonlyArray<DesktopNoteEntry>
  threads: ReadonlyArray<DesktopThread>
  activeThreadId: string | null
  /** The desktop-only planning deck; it has no deployment authority itself. */
  fleetDeskOpen: boolean
  /** The current, explicitly unsubmitted FleetRun objective draft. */
  fleetObjective: string
  /** Honest deployment posture: local UI cannot invent a FleetRun receipt. */
  fleetDeployment: "not_requested" | "dispatching" | "accepted" | "rejected" | "unavailable"
  /** Count of completed button -> intent -> state -> re-render round trips. */
  loopProofs: number
}>

/** "18:04" — display-string timestamps for the typed message contract. */
export const formatShellTimestamp = (date: Date): string =>
  `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`

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
  fleetDeskOpen: false,
  fleetObjective: "",
  fleetDeployment: "not_requested",
  loopProofs: 0,
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

export const desktopShellIntents = [
  DesktopInputChanged,
  DesktopNoteSubmitted,
  DesktopLoopPinged,
  DesktopFleetDeskToggled,
  DesktopFleetObjectiveChanged,
  DesktopFleetDeploymentRequested,
  DesktopNewChat,
  DesktopChatSelected,
] as const

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
  fleetDeskOpen: false,
  fleetObjective: "",
  fleetDeployment: "not_requested",
})

export const withChatSelected = (state: DesktopShellState, thread: DesktopThread): DesktopShellState => ({
  ...state,
  notes: thread.notes,
  activeThreadId: thread.id,
  fleetDeskOpen: false,
})

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
  sendMessage: (input: Readonly<{ id: string; message: string }>) => Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>>
}>

export const withThreads = (state: DesktopShellState, threads: ReadonlyArray<DesktopThread>): DesktopShellState => {
  const active = state.activeThreadId === null ? threads[0] : threads.find((thread) => thread.id === state.activeThreadId)
  return { ...state, threads: threads.slice(0, 5), activeThreadId: active?.id ?? null, notes: active?.notes ?? state.notes }
}

export const withTurnResult = (state: DesktopShellState, result: Awaited<ReturnType<ChatHost["sendMessage"]>>, timestamp: string): DesktopShellState => {
  if (result.ok && result.thread) return { ...withChatSelected(state, result.thread), pending: false, threads: [result.thread, ...state.threads.filter((thread) => thread.id !== result.thread!.id)].slice(0, 5) }
  return { ...state, pending: false, notes: [...state.notes, { key: `error-${state.notes.length}`, role: "system", text: result.error ?? "The model request failed.", timestamp }] }
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
): IntentHandlers<typeof desktopShellIntents> => ({
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
  DesktopChatSelected: (id) => Effect.gen(function* () { const thread = yield* Effect.promise(() => chat.openThread(id)); if (thread) yield* SubscriptionRef.update(state, (current) => withChatSelected(current, thread)) }),
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
    entry.role === "user" ? "YOU" : entry.role === "assistant" ? "ASSISTANT" : "SYSTEM",
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

const shellHeader = (state: DesktopShellState): View =>
  Stack(
    {
      key: "shell-header",
      direction: "row",
      gap: "3",
      align: "center",
      style: {
        width: "full",
        paddingLeft: "3",
        paddingRight: "3",
        paddingTop: "2",
        paddingBottom: "2",
        surface: "glass",
      },
    },
    [
      Text({ key: "shell-title", content: state.fleetDeskOpen ? "Fleet" : state.threads.find((thread) => thread.id === state.activeThreadId)?.title ?? "New chat", variant: "title", color: "textPrimary" }),
      Badge({
        key: "shell-surface",
        label: state.fleetDeskOpen ? "Planning" : "Chat",
        tone: "info",
        a11y: { label: state.fleetDeskOpen ? "Fleet planning workspace" : "Chat workspace" },
      }),
      Button({
        key: "shell-fleet-toggle",
        label: state.fleetDeskOpen ? "Back to chat" : "Open Fleet",
        variant: "ghost",
        onPress: IntentRef("DesktopFleetDeskToggled"),
        a11y: { label: state.fleetDeskOpen ? "Close Fleet desk" : "Open Fleet desk" },
      }),
      Badge({
        key: "shell-status",
        label: "Local workspace",
        tone: "success",
        a11y: { label: "Local workspace ready" },
      }),
      Spacer({ key: "shell-header-fill", flex: true }),
      Badge({
        key: "shell-host",
        label: state.host,
        tone: "neutral",
        a11y: { label: `Rendering host: ${state.host}` },
      }),
      Badge({
        key: "shell-ping-count",
        label: `proofs ${state.loopProofs}`,
        tone: state.loopProofs > 0 ? "success" : "neutral",
        a11y: { label: `${state.loopProofs} completed intent loop proofs` },
      }),
    ],
  )

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
      Stack({ key: "sidebar-action-new-chat", direction: "row", gap: "2", align: "center" }, [
        Icon({ key: "sidebar-new-chat-icon", name: "ChatCompose", size: "sm", color: "accent" }),
        Button({
          key: "sidebar-new-chat",
          label: "New chat",
          variant: "secondary",
          onPress: IntentRef("DesktopNewChat"),
          a11y: { label: "Start a new chat" },
        }),
      ]),
      Text({ key: "sidebar-chats-label", content: "Chats", variant: "caption", color: "textMuted" }),
      ...state.threads.map((thread) => Stack({ key: `sidebar-action-thread-${thread.id}`, direction: "row", gap: "2", align: "center" }, [
        Icon({ key: `sidebar-thread-icon-${thread.id}`, name: "Chats", size: "sm", color: "textMuted" }),
        Button({
          key: `sidebar-thread-${thread.id}`,
          label: thread.title,
          variant: "ghost",
          onPress: IntentRef("DesktopChatSelected", StaticPayload(thread.id)),
          a11y: { label: `Open chat ${thread.title}` },
        }),
      ])),
      Text({ key: "sidebar-workspace-label", content: "Workspace", variant: "caption", color: "textMuted" }),
      Stack({ key: "sidebar-action-fleet", direction: "row", gap: "2", align: "center" }, [
        Icon({ key: "sidebar-fleet-icon", name: "Agent", size: "sm", color: "textMuted" }),
        Button({
          key: "sidebar-fleet",
          label: "Fleet",
          variant: "ghost",
          onPress: IntentRef("DesktopFleetDeskToggled"),
          a11y: { label: state.fleetDeskOpen ? "Close Fleet" : "Open Fleet" },
        }),
      ]),
      Spacer({ key: "sidebar-fill", flex: true }),
      Badge({
        key: "sidebar-pylon-status",
        label: "Pylon dispatch",
        tone: "neutral",
        a11y: { label: "Local Pylon dispatch capability" },
      }),
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
        content: "What would you like to move today?",
        variant: "heading",
        color: "textPrimary",
      }),
      Text({
        key: "shell-welcome-body",
        content: "Start with a question, a task, or a clear objective for your local fleet.",
        variant: "body",
        color: "textMuted",
      }),
    ],
  )

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
          Button({
            key: "shell-ping",
            label: "Proof",
            variant: "ghost",
            onPress: IntentRef("DesktopLoopPinged"),
            a11y: { label: "Ping the Effect Native intent loop" },
          }),
        ],
      ),
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
          shellHeader(state),
          ...(state.fleetDeskOpen ? [fleetDesk(state)] : []),
          ...(state.notes.length === 0 && !state.fleetDeskOpen ? [shellWelcome()] : []),
          Transcript({
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
          }),
          shellComposer(state),
        ],
      ),
    ],
  )],
  )
