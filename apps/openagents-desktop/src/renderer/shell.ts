/**
 * OpenAgents Desktop shell (#8574 initial greenfield exit).
 *
 * The whole first screen is typed Effect Native data: state, intents, and a
 * pure `state -> View` projection over the shared vendored catalog
 * (`@effect-native/core`, catalog v26). No React, no local UI primitives —
 * catalog gaps go to `docs/effect-native/DEMAND_REGISTER.md` (D-DESK-01),
 * never local code.
 *
 * Component-sharing proof: the transcript-message and composer compositions
 * are structured identically to the Sarah EN surface
 * (`apps/sarah/src/ui/main.ts` — `transcriptMessage`, `composerView`) so one
 * catalog serves many hosts with the same composition shapes. Desktop must
 * not import Sarah's app modules (separate hosts, no cross-app coupling), so
 * the shape parity is asserted here and in `shell.test.ts` instead.
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
}>

export type DesktopShellState = Readonly<{
  /** Host identity decoded from the preload bridge ("electron/darwin" etc.). */
  host: string
  input: string
  notes: ReadonlyArray<DesktopNoteEntry>
  /** Count of completed button -> intent -> state -> re-render round trips. */
  loopProofs: number
}>

/**
 * Honest placeholder state: the shell says exactly what it is and what it is
 * not. Sarah conversation and the Fleet cockpit are later #8574 exits.
 */
export const initialDesktopShellState = (host: string): DesktopShellState => ({
  host,
  input: "",
  notes: [
    {
      key: "boot-0",
      role: "system",
      text: "OpenAgents Desktop shell online — Effect Native catalog rendered by @effect-native/render-dom inside a hardened Electron window.",
    },
    {
      key: "boot-1",
      role: "system",
      text: "This is the #8574 greenfield setup exit: typed state, typed intents, one shared catalog. Sarah conversation and the Fleet cockpit land in later exits.",
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

export const withNote = (state: DesktopShellState, text: string): DesktopShellState => {
  const trimmed = text.trim()
  if (trimmed === "") return state
  return {
    ...state,
    input: "",
    notes: [
      ...state.notes,
      { key: `note-${state.notes.length}`, role: "user", text: trimmed },
    ],
  }
}

export const withLoopProof = (state: DesktopShellState): DesktopShellState => {
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
      },
    ],
  }
}

export const makeDesktopShellHandlers = (
  state: SubscriptionRef.SubscriptionRef<DesktopShellState>,
): IntentHandlers<typeof desktopShellIntents> => ({
  DesktopInputChanged: (value) =>
    SubscriptionRef.update(state, (current) => withInput(current, value)),
  DesktopNoteSubmitted: (value) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const message =
        typeof value === "string" && value.trim() !== "" ? value : current.input
      yield* SubscriptionRef.set(state, withNote(current, message))
    }),
  DesktopLoopPinged: () => SubscriptionRef.update(state, withLoopProof),
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

/**
 * Same keyed role-tagged Card body shape as Sarah's `transcriptMessage`
 * (apps/sarah/src/ui/main.ts) — the shared-composition proof for this exit.
 */
export const noteMessage = (entry: DesktopNoteEntry): TranscriptMessage => ({
  key: entry.key,
  role: entry.role,
  body: [
    Card(
      {
        key: `${entry.key}-card`,
        padding: "3",
        radius: "lg",
        style: {
          backgroundColor: entry.role === "user" ? "surfaceRaised" : "surface",
          borderColor: "border",
          borderWidth: 1,
          width: "full",
        },
      },
      [
        text(`${entry.key}-role`, entry.role === "user" ? "YOU" : "SHELL", "caption", "textMuted"),
        text(`${entry.key}-text`, entry.text, "body"),
      ],
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
      style: { width: "full" },
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

/** Same row composition as Sarah's `composerView` (shared-composition proof). */
const shellComposer = (state: DesktopShellState): View =>
  Stack(
    {
      key: "shell-composer",
      direction: "row",
      gap: "3",
      align: "center",
      style: { width: "full" },
    },
    [
      TextField({
        key: "shell-input",
        value: state.input,
        placeholder: "Append a note to the shell transcript…",
        a11y: { label: "Note to the shell transcript" },
        onChange: IntentRef("DesktopInputChanged", ComponentValueBinding()),
        onSubmit: IntentRef("DesktopNoteSubmitted", ComponentValueBinding()),
        style: { flex: 1 },
      }),
      Button({
        key: "shell-note",
        label: "Note",
        variant: "primary",
        onPress: IntentRef("DesktopNoteSubmitted"),
        a11y: { label: "Append the typed note" },
      }),
      Button({
        key: "shell-ping",
        label: "Ping loop",
        variant: "secondary",
        onPress: IntentRef("DesktopLoopPinged"),
        a11y: { label: "Ping the Effect Native intent loop" },
      }),
    ],
  )

export const desktopShellView = (state: DesktopShellState): View =>
  Stack(
    {
      key: "shell-root",
      direction: "column",
      gap: "4",
      style: { width: "full", height: "full", minHeight: 0, padding: "4" },
    },
    [
      shellHeader(state),
      Transcript({
        key: "shell-transcript",
        pinToEnd: true,
        messages: state.notes.map(noteMessage),
        style: { width: "full", flex: 1, minHeight: 0 },
      }),
      shellComposer(state),
    ],
  )
