import { Schema } from "@effect-native/core/effect"

import {
  fixtureSessions,
  isFixtureSessionRef,
  type NativeWorkspace,
  type SpikeState,
} from "./program.ts"
import {
  assertNativeProductionCommandBindings,
  resolveNativeDeferredCommand,
} from "./production-command-parity.ts"

export const bridgePayloadLimit = 8 * 1024

const NativeIntentSchema = Schema.Struct({
  protocol: Schema.Literal(1),
  sequence: Schema.Number,
  intent: Schema.Union([
    Schema.Struct({ _tag: Schema.Literal("DeferredCommand"), command: Schema.Unknown }),
    Schema.Struct({ _tag: Schema.Literal("RendererReloadRequested"), commandId: Schema.Literal("openagents.spike.reload-effect") }),
    Schema.Struct({ _tag: Schema.Literal("NewChatRequested"), commandId: Schema.Literal("chat.new") }),
    Schema.Struct({ _tag: Schema.Literal("WorkspaceSelected"), workspace: Schema.Literal("chat"), commandId: Schema.Literal("chat.open") }),
    Schema.Struct({ _tag: Schema.Literal("WorkspaceSelected"), workspace: Schema.Literal("home"), commandId: Schema.Literal("workspace.home") }),
    Schema.Struct({ _tag: Schema.Literal("WorkspaceSelected"), workspace: Schema.Literal("settings"), commandId: Schema.Literal("settings.open") }),
    Schema.Struct({ _tag: Schema.Literal("SessionSelected"), sessionRef: Schema.String, commandId: Schema.Null }),
  ]),
})

export type NativeIntentEnvelope = typeof NativeIntentSchema.Type
const decodeNativeIntentSchema = Schema.decodeUnknownSync(NativeIntentSchema)

export type NativeAppliedCommand = Readonly<{
  sequence: number
  commandId: "chat.new"
  intentName: "DesktopNewChat"
  source: "native_menu"
}>

type PendingAppliedCommand = Omit<NativeAppliedCommand, "sequence">

export type NativeDispatch = Readonly<{
  intentName: string
  payload: null | string
  appliedCommand: PendingAppliedCommand | null
}>

export const resolveNativeDispatch = (envelope: NativeIntentEnvelope): NativeDispatch => {
  const action = envelope.intent
  if (action._tag === "RendererReloadRequested") {
    throw new Error("native_renderer_reload_is_host_lifecycle")
  }
  if (action._tag === "DeferredCommand") {
    const resolved = resolveNativeDeferredCommand(action.command)
    if (
      resolved.command.commandId !== "chat.new" ||
      resolved.command.source !== "native_menu" ||
      resolved.intentName !== "DesktopNewChat" ||
      resolved.payload !== null
    ) {
      throw new Error("native_deferred_command_out_of_scope")
    }
    return {
      intentName: resolved.intentName,
      payload: null,
      appliedCommand: {
        commandId: "chat.new",
        intentName: "DesktopNewChat",
        source: "native_menu",
      },
    }
  }
  if (action._tag === "NewChatRequested") {
    return { intentName: "DesktopNewChat", payload: null, appliedCommand: null }
  }
  if (action._tag === "WorkspaceSelected") {
    return {
      intentName: "DesktopWorkspaceSelected",
      payload: action.workspace,
      appliedCommand: null,
    }
  }
  return { intentName: "DesktopChatSelected", payload: action.sessionRef, appliedCommand: null }
}

export const decodeNativeIntent = (candidate: unknown): NativeIntentEnvelope => {
  assertNativeProductionCommandBindings()
  const decoded = decodeNativeIntentSchema(candidate, { onExcessProperty: "error" })
  if (!Number.isSafeInteger(decoded.sequence) || decoded.sequence <= 0) {
    throw new Error("native_intent_sequence_invalid")
  }
  if (decoded.intent._tag === "SessionSelected" && !isFixtureSessionRef(decoded.intent.sessionRef)) {
    throw new Error("native_intent_session_unknown")
  }
  if (decoded.intent._tag !== "RendererReloadRequested") resolveNativeDispatch(decoded)
  return decoded
}

export interface NativeProjection {
  readonly protocol: 1
  readonly revision: number
  readonly workspace: NativeWorkspace
  readonly selectedSessionRef: string | null
  readonly messageCount: number
  readonly pending: boolean
  readonly status: string
  readonly lastAppliedCommand: NativeAppliedCommand | null
}

const projectWorkspace = (workspace: SpikeState["workspace"]): NativeWorkspace => {
  if (workspace === "chat" || workspace === "home" || workspace === "settings") return workspace
  throw new Error(`native_projection_workspace_unsupported:${workspace}`)
}

export const projectNativeState = (
  state: SpikeState,
  revision: number,
  lastAppliedCommand: NativeAppliedCommand | null = null,
): NativeProjection => ({
  protocol: 1,
  revision,
  workspace: projectWorkspace(state.workspace),
  selectedSessionRef: isFixtureSessionRef(state.activeThreadId) ? state.activeThreadId : null,
  messageCount: state.notes.length,
  pending: state.pending,
  status: state.pending ? "Codex is working" : "Production Desktop shell synchronized",
  lastAppliedCommand,
})

type ZeroBridge = {
  readonly invoke: (command: string, payload: unknown) => Promise<unknown>
}

const zeroBridge = (): ZeroBridge | undefined =>
  (globalThis as typeof globalThis & { zero?: ZeroBridge }).zero

export const publishNativeProjection = async (
  state: SpikeState,
  revision: number,
  acknowledgedNativeSequence = 0,
  lastAppliedCommand: NativeAppliedCommand | null = null,
): Promise<NativeIntentEnvelope | null> => {
  const bridge = zeroBridge()
  if (bridge === undefined) return null
  const request = { ...projectNativeState(state, revision, lastAppliedCommand), acknowledgedNativeSequence }
  if (new TextEncoder().encode(JSON.stringify(request)).length > bridgePayloadLimit) return null
  const response = await bridge.invoke("openagents.spike.projection.v1", request)
  if (response === null || typeof response !== "object" || !("intent" in response)) return null
  const intent = (response as Readonly<{ intent?: unknown }>).intent
  if (intent === null || intent === undefined) return null
  try {
    return decodeNativeIntent(intent)
  } catch {
    return null
  }
}

export const startNativeBridgeSync = (
  readState: () => Promise<SpikeState>,
  handler: (intent: NativeIntentEnvelope) => Promise<PendingAppliedCommand | null>,
  options: Readonly<{
    initialRevision: number
    initialAcknowledgedSequence?: number
    onProjection?: (state: SpikeState, revision: number) => void
    onAcknowledged?: (state: SpikeState, revision: number, sequence: number) => void
  }>,
): (() => void) => {
  let acknowledgedSequence = options.initialAcknowledgedSequence ?? 0
  let revision = options.initialRevision
  let lastState: SpikeState | null = null
  let lastAppliedCommand: NativeAppliedCommand | null = null
  let inFlight = false
  const tick = (): void => {
    if (inFlight) return
    inFlight = true
    void readState()
      .then(async (state) => {
        if (lastState !== null && state !== lastState) revision += 1
        lastState = state
        options.onProjection?.(state, revision)
        const envelope = await publishNativeProjection(state, revision, acknowledgedSequence, lastAppliedCommand)
        if (envelope === null || envelope.sequence <= acknowledgedSequence) return
        const applied = await handler(envelope)
        acknowledgedSequence = envelope.sequence
        options.onAcknowledged?.(state, revision, acknowledgedSequence)
        if (applied !== null) lastAppliedCommand = { ...applied, sequence: envelope.sequence }
      })
      .catch(() => undefined)
      .finally(() => { inFlight = false })
  }
  tick()
  const timer = globalThis.setInterval(tick, 120)
  return () => globalThis.clearInterval(timer)
}

export const nativeFixtureSessionCount = fixtureSessions.length
