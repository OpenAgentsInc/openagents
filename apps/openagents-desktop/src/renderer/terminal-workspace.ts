/**
 * Workspace-bounded terminal workspace (CUT-20, #8700).
 *
 * A bounded, honest text terminal: monospace output (a `CodeBlock` over the
 * host's bounded, already-redacted tail) plus a typed input line. The renderer
 * NEVER holds a shell or a process — it dispatches typed intents (create /
 * input / interrupt / restart / close / preview-open) that main binds to the
 * authorized workspace. All host responses are schema-decoded here (mirroring
 * ../terminal-contract.ts), never trusted raw. A full xterm.js pseudo-TTY is
 * the documented enhancement (needs the node-pty backend + a foreign-host
 * canvas seam); the bounded text terminal is the shipped, testable surface.
 *
 * Design-language conformance (apps-sdk chrome oracle): token scales only — no
 * raw colors, no raw dimensions, no raw type sizing.
 */
import {
  Badge,
  Button,
  CodeBlock,
  ComponentValueBinding,
  EmptyMessage,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, SubscriptionRef, Schema } from "@effect-native/core/effect"

import {
  TerminalEventSchema,
  decodeTerminalAckResult,
  decodeTerminalCreateResult,
  decodeTerminalPreviewOpenResult,
  decodeTerminalSnapshot,
  type TerminalEvent,
} from "../terminal-contract.ts"

/** Renderer-side cap on the retained per-session output tail. */
const RENDERER_OUTPUT_CAP = 100_000

export type TerminalPreview = Readonly<{ port: number; url: string; ready: boolean }>

export type TerminalRendererSession = Readonly<{
  sessionRef: string
  cwdLabel: string
  shellLabel: string
  status: "running" | "exited" | "recovered"
  exitCode: number | null
  recovered: boolean
  gap: boolean
  output: string
  previews: ReadonlyArray<TerminalPreview>
}>

export type TerminalWorkspaceState = Readonly<{
  phase: "idle" | "ready"
  activeRef: string | null
  sessions: ReadonlyArray<TerminalRendererSession>
  input: string
  notice: string | null
}>

export const emptyTerminalWorkspaceState = (): TerminalWorkspaceState => ({
  phase: "idle",
  activeRef: null,
  sessions: [],
  input: "",
  notice: null,
})

// ---------------------------------------------------------------------------
// Pure transitions.
// ---------------------------------------------------------------------------

const cap = (text: string): string =>
  text.length > RENDERER_OUTPUT_CAP ? text.slice(text.length - RENDERER_OUTPUT_CAP) : text

const upsert = (
  sessions: ReadonlyArray<TerminalRendererSession>,
  next: TerminalRendererSession,
): ReadonlyArray<TerminalRendererSession> =>
  sessions.some((session) => session.sessionRef === next.sessionRef)
    ? sessions.map((session) => (session.sessionRef === next.sessionRef ? next : session))
    : [...sessions, next]

const patch = (
  state: TerminalWorkspaceState,
  sessionRef: string,
  change: (session: TerminalRendererSession) => TerminalRendererSession,
): TerminalWorkspaceState => ({
  ...state,
  sessions: state.sessions.map((session) =>
    session.sessionRef === sessionRef ? change(session) : session),
})

export const withTerminalInput = (
  state: TerminalWorkspaceState,
  input: string,
): TerminalWorkspaceState => ({ ...state, input })

export const withTerminalActive = (
  state: TerminalWorkspaceState,
  sessionRef: string,
): TerminalWorkspaceState =>
  state.sessions.some((session) => session.sessionRef === sessionRef)
    ? { ...state, activeRef: sessionRef }
    : state

/** Apply one host snapshot (restart recovery + initial hydration). */
export const withTerminalSnapshot = (
  state: TerminalWorkspaceState,
  value: unknown,
): TerminalWorkspaceState => {
  const snapshot = decodeTerminalSnapshot(value)
  if (snapshot === null) return state
  const sessions: TerminalRendererSession[] = snapshot.sessions.map((session) => ({
    sessionRef: session.sessionRef,
    cwdLabel: session.cwdLabel,
    shellLabel: session.shellLabel,
    status: session.status,
    exitCode: session.exitCode,
    recovered: session.recovered,
    gap: session.gap,
    output: session.tail,
    previews: session.previews.map((preview) => ({ ...preview })),
  }))
  const activeRef = state.activeRef !== null && sessions.some((session) => session.sessionRef === state.activeRef)
    ? state.activeRef
    : sessions.find((session) => session.status === "running")?.sessionRef
      ?? sessions[0]?.sessionRef
      ?? null
  return { ...state, phase: "ready", sessions, activeRef }
}

/** Apply one streamed host event. */
export const withTerminalEvent = (
  state: TerminalWorkspaceState,
  event: TerminalEvent,
): TerminalWorkspaceState => {
  switch (event.kind) {
    case "ready": {
      const next: TerminalRendererSession = {
        sessionRef: event.sessionRef,
        cwdLabel: event.cwdLabel,
        shellLabel: event.shellLabel,
        status: "running",
        exitCode: null,
        recovered: false,
        gap: false,
        output: state.sessions.find((session) => session.sessionRef === event.sessionRef)?.output ?? "",
        previews: [],
      }
      return {
        ...state,
        phase: "ready",
        sessions: upsert(state.sessions, next),
        activeRef: state.activeRef ?? event.sessionRef,
      }
    }
    case "output":
      return patch(state, event.sessionRef, (session) => ({
        ...session,
        output: cap(session.output + event.chunk),
      }))
    case "exit":
      return patch(state, event.sessionRef, (session) => ({
        ...session,
        status: "exited",
        exitCode: event.exitCode,
      }))
    case "preview":
      return patch(state, event.sessionRef, (session) => ({
        ...session,
        previews: session.previews.some((preview) => preview.port === event.port)
          ? session.previews
          : [...session.previews, { port: event.port, url: event.url, ready: event.ready }],
      }))
    case "closed": {
      const sessions = state.sessions.filter((session) => session.sessionRef !== event.sessionRef)
      return {
        ...state,
        sessions,
        activeRef: state.activeRef === event.sessionRef
          ? sessions[sessions.length - 1]?.sessionRef ?? null
          : state.activeRef,
      }
    }
    case "error":
      return { ...state, notice: event.message.slice(0, 200) }
    default:
      return state
  }
}

/** Fold a renderer-frame batch with one store publication. */
export const withTerminalEvents = (
  state: TerminalWorkspaceState,
  events: ReadonlyArray<TerminalEvent>,
): TerminalWorkspaceState => events.reduce(withTerminalEvent, state)

// ---------------------------------------------------------------------------
// Intents.
// ---------------------------------------------------------------------------

export const TerminalCreateRequested = defineIntent("TerminalCreateRequested", Schema.Null)
export const TerminalSelected = defineIntent("TerminalSelected", Schema.String)
export const TerminalInputChanged = defineIntent("TerminalInputChanged", Schema.String)
export const TerminalInputSubmitted = defineIntent("TerminalInputSubmitted", Schema.Null)
export const TerminalInterruptRequested = defineIntent("TerminalInterruptRequested", Schema.Null)
export const TerminalRestartRequested = defineIntent("TerminalRestartRequested", Schema.Null)
export const TerminalCloseRequested = defineIntent("TerminalCloseRequested", Schema.String)
export const TerminalPreviewOpenRequested = defineIntent("TerminalPreviewOpenRequested", Schema.Number)
export const TerminalRefreshRequested = defineIntent("TerminalRefreshRequested", Schema.Null)
export const TerminalEventReceived = defineIntent("TerminalEventReceived", TerminalEventSchema)
export const TerminalEventsReceived = defineIntent("TerminalEventsReceived", Schema.Array(TerminalEventSchema))

export const terminalWorkspaceIntents = [
  TerminalCreateRequested,
  TerminalSelected,
  TerminalInputChanged,
  TerminalInputSubmitted,
  TerminalInterruptRequested,
  TerminalRestartRequested,
  TerminalCloseRequested,
  TerminalPreviewOpenRequested,
  TerminalRefreshRequested,
  TerminalEventReceived,
  TerminalEventsReceived,
] as const

// ---------------------------------------------------------------------------
// Bridge + handlers (generic in the shell state shape, like ./fleet-workspace).
// ---------------------------------------------------------------------------

export type TerminalRendererBridge = Readonly<{
  create: (value: unknown) => Promise<unknown>
  input: (value: unknown) => Promise<unknown>
  interrupt: (value: unknown) => Promise<unknown>
  restart: (value: unknown) => Promise<unknown>
  close: (value: unknown) => Promise<unknown>
  snapshot: () => Promise<unknown>
  openPreview: (value: unknown) => Promise<unknown>
}>

export const unavailableTerminalBridge: TerminalRendererBridge = {
  create: async () => ({ ok: false, reason: "unavailable", message: "Terminals are unavailable." }),
  input: async () => ({ ok: false, reason: "not_found" }),
  interrupt: async () => ({ ok: false, reason: "not_found" }),
  restart: async () => ({ ok: false, reason: "not_found" }),
  close: async () => ({ ok: false, reason: "not_found" }),
  snapshot: async () => ({ sessions: [] }),
  openPreview: async () => ({ ok: false, reason: "unavailable" }),
}

export type TerminalCapableState = Readonly<{ terminal: TerminalWorkspaceState }>

const activeRefOf = <S extends TerminalCapableState>(state: S): string | null =>
  state.terminal.activeRef

export const makeTerminalWorkspaceHandlers = <S extends TerminalCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: TerminalRendererBridge = unavailableTerminalBridge,
) => ({
  TerminalRefreshRequested: () =>
    Effect.gen(function* () {
      const snapshot = yield* Effect.promise(() => bridge.snapshot().catch(() => null))
      yield* SubscriptionRef.update(state, (current) => ({
        ...current,
        terminal: withTerminalSnapshot(current.terminal, snapshot),
      }))
    }),
  TerminalCreateRequested: () =>
    Effect.gen(function* () {
      const result = decodeTerminalCreateResult(
        yield* Effect.promise(() => bridge.create({}).catch(() => null)),
      )
      if (result.ok) {
        const created = result
        yield* SubscriptionRef.update(state, (current) => ({
          ...current,
          terminal: {
            ...current.terminal,
            phase: "ready",
            activeRef: created.sessionRef,
            sessions: upsert(current.terminal.sessions, {
              sessionRef: created.sessionRef,
              cwdLabel: created.cwdLabel,
              shellLabel: created.shellLabel,
              status: "running",
              exitCode: null,
              recovered: false,
              gap: false,
              output: "",
              previews: [],
            }),
          },
        }))
      } else {
        const message = "message" in result ? result.message : "The terminal could not be started."
        yield* SubscriptionRef.update(state, (current) => ({
          ...current,
          terminal: { ...current.terminal, notice: message },
        }))
      }
    }),
  TerminalSelected: (sessionRef: string) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      terminal: withTerminalActive(current.terminal, sessionRef),
    })),
  TerminalInputChanged: (input: string) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      terminal: withTerminalInput(current.terminal, input.slice(0, 8_000)),
    })),
  TerminalInputSubmitted: () =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const sessionRef = activeRefOf(current)
      if (sessionRef === null) return
      const line = current.terminal.input
      yield* SubscriptionRef.update(state, (next) => ({
        ...next,
        terminal: withTerminalInput(next.terminal, ""),
      }))
      // A newline commits the line to the shell's STDIN (never an argv).
      decodeTerminalAckResult(
        yield* Effect.promise(() =>
          bridge.input({ sessionRef, data: `${line}\n` }).catch(() => null)),
      )
    }),
  TerminalInterruptRequested: () =>
    Effect.gen(function* () {
      const sessionRef = activeRefOf(yield* SubscriptionRef.get(state))
      if (sessionRef === null) return
      // Ctrl+C — a typed interrupt (SIGINT to the owned process group in main).
      yield* Effect.promise(() => bridge.interrupt({ sessionRef }).catch(() => null))
    }),
  TerminalRestartRequested: () =>
    Effect.gen(function* () {
      const sessionRef = activeRefOf(yield* SubscriptionRef.get(state))
      if (sessionRef === null) return
      yield* SubscriptionRef.update(state, (current) => ({
        ...current,
        terminal: patch(current.terminal, sessionRef, (session) => ({ ...session, output: "" })),
      }))
      yield* Effect.promise(() => bridge.restart({ sessionRef }).catch(() => null))
    }),
  TerminalCloseRequested: (sessionRef: string) =>
    Effect.gen(function* () {
      yield* Effect.promise(() => bridge.close({ sessionRef }).catch(() => null))
    }),
  TerminalPreviewOpenRequested: (port: number) =>
    Effect.gen(function* () {
      const sessionRef = activeRefOf(yield* SubscriptionRef.get(state))
      if (sessionRef === null) return
      const result = decodeTerminalPreviewOpenResult(
        yield* Effect.promise(() => bridge.openPreview({ sessionRef, port }).catch(() => null)),
      )
      if (!result.ok) {
        yield* SubscriptionRef.update(state, (current) => ({
          ...current,
          terminal: { ...current.terminal, notice: `Preview could not be opened (${result.reason}).` },
        }))
      }
    }),
  TerminalEventReceived: (event: TerminalEvent) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      terminal: withTerminalEvent(current.terminal, event),
    })),
  TerminalEventsReceived: (events: ReadonlyArray<TerminalEvent>) =>
    SubscriptionRef.update(state, current => ({
      ...current,
      terminal: withTerminalEvents(current.terminal, events),
    })),
})

// ---------------------------------------------------------------------------
// View — pure state -> View.
// ---------------------------------------------------------------------------

const statusTone = (status: TerminalRendererSession["status"]): "success" | "neutral" | "warn" =>
  status === "running" ? "success" : status === "recovered" ? "warn" : "neutral"

const sessionTab = (state: TerminalWorkspaceState, session: TerminalRendererSession): View =>
  Button({
    key: `terminal-tab-${session.sessionRef}`,
    label: `${session.cwdLabel} · ${session.status}`,
    variant: "ghost",
    selected: session.sessionRef === state.activeRef,
    onPress: IntentRef("TerminalSelected", StaticPayload(session.sessionRef)),
    a11y: { label: `Select terminal ${session.sessionRef} (${session.status})` },
  })

const previewRow = (session: TerminalRendererSession): ReadonlyArray<View> =>
  session.previews.map((preview) =>
    Stack(
      { key: `terminal-preview-${session.sessionRef}-${preview.port}`, direction: "row", gap: "2", align: "center" },
      [
        Badge({
          key: `terminal-preview-badge-${preview.port}`,
          label: `preview :${preview.port}`,
          tone: preview.ready ? "success" : "neutral",
          a11y: { label: `Local preview on port ${preview.port}` },
        }),
        Text({
          key: `terminal-preview-url-${preview.port}`,
          content: preview.url,
          variant: "caption",
          color: "textMuted",
        }),
        Button({
          key: `terminal-preview-open-${preview.port}`,
          label: "Open in browser",
          variant: "ghost",
          onPress: IntentRef("TerminalPreviewOpenRequested", StaticPayload(preview.port)),
          a11y: { label: `Open local preview on port ${preview.port} in the browser` },
        }),
      ],
    ))

const activeSession = (state: TerminalWorkspaceState): TerminalRendererSession | null =>
  state.sessions.find((session) => session.sessionRef === state.activeRef) ?? null

const terminalOutput = (session: TerminalRendererSession | null): View => {
  const text = session?.output ?? ""
  const lines = text === "" ? [""] : text.replace(/\r/g, "").split("\n")
  return Stack(
    {
      key: "terminal-output-well",
      direction: "column",
      gap: "1",
      style: {
        width: "full",
        minWidth: 0,
        flex: 1,
        minHeight: 0,
        backgroundColor: "background",
        borderRadius: "md",
        borderColor: "borderSubtle",
        borderWidth: 1,
        padding: "2",
      },
    },
    [
      CodeBlock({
        key: "terminal-output-code",
        lines: lines.map((line) => ({ tokens: [{ kind: "plain" as const, text: line }] })),
      }),
    ],
  )
}

export const terminalWorkspaceView = (state: TerminalWorkspaceState): View => {
  const session = activeSession(state)
  const canAct = session !== null && session.status === "running"
  return Stack(
    {
      key: "workspace-terminal-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", minWidth: 0, flex: 1, minHeight: 0, paddingRight: "4", paddingTop: "2" },
    },
    [
      Stack({ key: "terminal-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "terminal-title", content: "Terminal", variant: "heading", color: "textPrimary" }),
        ...(session === null
          ? []
          : [Badge({
              key: "terminal-status",
              label: session.status,
              tone: statusTone(session.status),
              a11y: { label: `Active terminal status: ${session.status}` },
            })]),
        Spacer({ key: "terminal-heading-fill", flex: true }),
        Button({
          key: "terminal-new",
          label: "New terminal",
          variant: "secondary",
          onPress: IntentRef("TerminalCreateRequested"),
          a11y: { label: "Open a new workspace terminal" },
        }),
        Button({
          key: "terminal-interrupt",
          label: "Interrupt (Ctrl+C)",
          variant: "ghost",
          disabled: !canAct,
          onPress: IntentRef("TerminalInterruptRequested"),
          a11y: { label: "Send Ctrl+C to the active terminal" },
        }),
        Button({
          key: "terminal-restart",
          label: "Restart",
          variant: "ghost",
          disabled: session === null,
          onPress: IntentRef("TerminalRestartRequested"),
          a11y: { label: "Restart the active terminal" },
        }),
        ...(session === null
          ? []
          : [Button({
              key: "terminal-close",
              label: "Close",
              variant: "ghost",
              onPress: IntentRef("TerminalCloseRequested", StaticPayload(session.sessionRef)),
              a11y: { label: "Close the active terminal" },
            })]),
      ]),
      ...(state.sessions.length > 1
        ? [Stack(
            { key: "terminal-tabs", direction: "row", gap: "1", align: "center", style: { width: "full", minWidth: 0 } },
            state.sessions.map((entry) => sessionTab(state, entry)),
          )]
        : []),
      ...(state.notice === null
        ? []
        : [Text({ key: "terminal-notice", content: state.notice, variant: "caption", color: "warning" })]),
      ...(session === null
        ? [EmptyMessage({
            key: "terminal-empty",
            icon: { name: "Terminal", tone: "secondary" },
            title: "No terminal open. Open one to run build, test, and dev-server commands in this workspace.",
          })]
        : [
            ...(session.recovered
              ? [Text({
                  key: "terminal-recovered-note",
                  content: session.gap
                    ? "Recovered from a previous session — output below is the persisted tail (a gap may precede it)."
                    : "Recovered from a previous session.",
                  variant: "caption",
                  color: "textMuted",
                })]
              : []),
            terminalOutput(session),
            ...previewRow(session),
            Stack({ key: "terminal-input-row", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
              TextField({
                key: "terminal-input-field",
                value: state.input,
                placeholder: canAct ? "Type a command and press Send…" : "Terminal is not running",
                disabled: !canAct,
                onChange: IntentRef("TerminalInputChanged", ComponentValueBinding()),
                a11y: { label: "Terminal input line (sent to the shell's standard input)" },
                style: { width: "full" },
              }),
              Button({
                key: "terminal-send",
                label: "Send",
                variant: "primary",
                disabled: !canAct,
                onPress: IntentRef("TerminalInputSubmitted"),
                a11y: { label: "Send the input line to the terminal" },
              }),
            ]),
          ]),
    ],
  )
}
