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
  Button,
  Card,
  ComponentValueBinding,
  IntentRef,
  Spacer,
  Stack,
  Text,
  TextField,
  Transcript,
  defineIntent,
  type IntentHandlers,
  type TextView,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

export type DesktopNoteEntry = Readonly<{
  key: string
  role: "user" | "system"
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
  /** Count of completed button -> intent -> state -> re-render round trips. */
  loopProofs: number
}>

/** "18:04" — display-string timestamps for the typed message contract. */
export const formatShellTimestamp = (date: Date): string =>
  `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`

/**
 * Honest placeholder state: the shell says exactly what it is and what it is
 * not. Sarah conversation and the Fleet cockpit are later #8574 exits.
 */
export const initialDesktopShellState = (
  host: string,
  timestamp: string = formatShellTimestamp(new Date()),
): DesktopShellState => ({
  host,
  input: "",
  pending: false,
  notes: [
    {
      key: "boot-0",
      role: "system",
      text: "OpenAgents Desktop shell online — Effect Native catalog rendered by @effect-native/render-dom inside a hardened Electron window.",
      timestamp,
    },
    {
      key: "boot-1",
      role: "system",
      text: "This is the #8574 greenfield setup exit: typed state, typed intents, one shared catalog. Sarah conversation and the Fleet cockpit land in later exits.",
      timestamp,
    },
  ],
  loopProofs: 0,
})

// ---------------------------------------------------------------------------
// Intents — typed end-to-end: DOM event -> IntentRef -> registry decode ->
// handler -> SubscriptionRef update -> viewStream re-render.
// ---------------------------------------------------------------------------

export const DesktopInputChanged = defineIntent("DesktopInputChanged", Schema.String)
/**
 * Null payload happens on button press (the renderer passes no component
 * value); the handler then falls back to the composer state, mirroring the
 * Sarah `SarahSendText` fallback.
 */
export const DesktopNoteSubmitted = defineIntent(
  "DesktopNoteSubmitted",
  Schema.NullOr(Schema.String),
)
export const DesktopLoopPinged = defineIntent("DesktopLoopPinged", Schema.Null)

export const desktopShellIntents = [
  DesktopInputChanged,
  DesktopNoteSubmitted,
  DesktopLoopPinged,
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
    pending: false,
    notes: [
      ...state.notes,
      { key: `note-${state.notes.length}`, role: "user", text: trimmed, timestamp },
    ],
  }
}

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
): IntentHandlers<typeof desktopShellIntents> => ({
  DesktopInputChanged: (value) =>
    SubscriptionRef.update(state, (current) => withInput(current, value)),
  DesktopNoteSubmitted: (value) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (current.pending) return
      const message =
        typeof value === "string" && value.trim() !== "" ? value : current.input
      yield* SubscriptionRef.set(state, withNote(current, message, now()))
    }),
  DesktopLoopPinged: () =>
    SubscriptionRef.update(state, (current) => withLoopProof(current, now())),
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

/** The centered ChatGPT-grade reading column shared by transcript + composer. */
const columnWidth = 760

/**
 * Real v29 chat rows: sender label and timestamp are typed message data — the
 * renderer draws the meta row and the role treatment (user end-aligned
 * bubble, system muted prose). The body is only the message text.
 */
export const noteMessage = (entry: DesktopNoteEntry): TranscriptMessage => ({
  key: entry.key,
  role: entry.role,
  senderLabel: entry.role === "user" ? "YOU" : "SHELL",
  timestamp: entry.timestamp,
  body: [
    text(
      `${entry.key}-text`,
      entry.text,
      "body",
      entry.role === "user" ? "textPrimary" : "textMuted",
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
        paddingLeft: "4",
        paddingRight: "4",
        paddingTop: "3",
        paddingBottom: "3",
        backgroundColor: "surface",
        borderColor: "border",
        borderWidth: 1,
      },
    },
    [
      Text({ key: "shell-title", content: "OpenAgents", variant: "title", color: "textPrimary" }),
      Badge({
        key: "shell-surface",
        label: "DESKTOP",
        tone: "info",
        a11y: { label: "OpenAgents Desktop surface" },
      }),
      Badge({
        key: "shell-status",
        label: "READY",
        tone: "success",
        a11y: { label: "Shell status: ready" },
      }),
      Spacer({ key: "shell-header-fill", flex: true }),
      Badge({
        key: "shell-host",
        label: `host: ${state.host}`,
        tone: "neutral",
        a11y: { label: `Rendering host: ${state.host}` },
      }),
      Badge({
        key: "shell-ping-count",
        label: `loop proofs: ${state.loopProofs}`,
        tone: state.loopProofs > 0 ? "success" : "neutral",
        a11y: { label: `${state.loopProofs} completed intent loop proofs` },
      }),
    ],
  )

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
        backgroundColor: "surface",
        borderColor: "border",
        borderWidth: 1,
        marginBottom: "4",
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
            placeholder: "Message the shell…",
            disabled: state.pending,
            clearOnSubmit: true,
            a11y: { label: "Message to the shell transcript" },
            onChange: IntentRef("DesktopInputChanged", ComponentValueBinding()),
            onSubmit: IntentRef("DesktopNoteSubmitted", ComponentValueBinding()),
            style: { flex: 1 },
          }),
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
            label: "Ping loop",
            variant: "ghost",
            onPress: IntentRef("DesktopLoopPinged"),
            a11y: { label: "Ping the Effect Native intent loop" },
          }),
        ],
      ),
    ],
  )

export const desktopShellView = (state: DesktopShellState): View =>
  Stack(
    {
      key: "shell-root",
      direction: "column",
      gap: "4",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    [
      shellHeader(state),
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
  )
