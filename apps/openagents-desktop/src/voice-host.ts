import { Schema } from "@effect-native/core/effect"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"

export const DesktopVoiceHostProtocolVersion = 1 as const

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256))
const SpokenText = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(16_384))
export const DesktopVoiceCommandSchema = Schema.Union([
  Schema.Struct({ protocolVersion: Schema.Literal(1), id: Schema.Literal("voice.start"), threadRef: Ref, sessionRef: Ref, disclosureRef: Ref }),
  Schema.Struct({ protocolVersion: Schema.Literal(1), id: Schema.Literals(["voice.stop", "voice.mute", "voice.unmute", "voice.suspend", "voice.resume", "voice.revoke"]) }),
  Schema.Struct({ protocolVersion: Schema.Literal(1), id: Schema.Literal("voice.speak"), turnRef: Ref, speechRef: Ref, messageRef: Ref, text: SpokenText }),
])
export type DesktopVoiceCommand = typeof DesktopVoiceCommandSchema.Type

export type DesktopVoiceState = Readonly<{
  protocolVersion: 1
  phase: "idle" | "requesting_permission" | "connecting" | "live" | "muted" | "suspended" | "denied" | "offline" | "backpressured" | "device_changed" | "revoked" | "failed"
  generation: number
  nextSequence: number
  acknowledgedSequence: number
  capture: boolean
  egress: boolean
  playback: boolean
  retainedAudio: boolean
  activity: "stopped" | "permission" | "connecting" | "listening" | "speech_detected" | "transcribing" | "awaiting_confirmation" | "executing" | "speaking" | "muted" | "reconnecting" | "degraded" | "revoked"
  transcript?: Readonly<{ utteranceRef: string; text: string; final: boolean }>
  proposal?: Readonly<{ proposalRef: string; utteranceRef: string; turnRef: string; targetRef: string; commandId: string; expiresAtMs: number; state: "proposed" | "applied" | "refused" }>
  playbackOutcomeRef?: string
  reason?: "permission_denied" | "network_lost" | "gateway_revoked" | "helper_crashed" | "stale_generation" | "backpressure" | "device_changed"
}>

export type VoiceMediaPacket = Readonly<{ sequence: number; generation: number; payloadLength: number; sha256: string }>
export type VoiceNativeMediaSession = Readonly<{
  setCaptureEnabled: (enabled: boolean) => void
  speak: (input: Readonly<{ turnRef: string; speechRef: string; messageRef: string; text: string }>) => Promise<boolean>
  close: (reason: "stop" | "revoke" | "replace" | "suspend" | "shutdown") => void
}>
export type VoiceNativeMedia = Readonly<{
  open: (input: Readonly<{
    identity: VoiceIdentity
    disclosureRef: string
    onPacket: (packet: VoiceMediaPacket) => void
    onAck: (sequence: number, generation: number) => void
    onState: (state: "offline" | "backpressured" | "device_changed" | "live" | "revoked" | "crashed") => void
    onControl: (control: Readonly<
      | { kind: "transcript"; utteranceRef: string; text: string; final: boolean }
      | { kind: "activity"; activity: "speech_detected" | "transcribing" | "awaiting_confirmation" | "executing" | "speaking" | "listening" }
      | { kind: "proposal"; proposalRef: string; utteranceRef: string; turnRef: string; targetRef: string; commandId: string; expiresAtMs: number }
      | { kind: "playback"; speechRef: string; state: "speaking" | "canceled"; outcomeRef?: string }
    >) => void
  }>) => VoiceNativeMediaSession | Promise<VoiceNativeMediaSession>
}>

export type DesktopVoiceHost = Readonly<{
  command: (command: DesktopVoiceCommand) => Promise<DesktopVoiceState>
  state: () => DesktopVoiceState
  subscribe: (listener: (state: DesktopVoiceState) => void) => () => void
  dispose: () => void
}>

export const createDesktopVoiceHost = (input: Readonly<{
  resolveIdentity: (request: Readonly<{ threadRef: string; sessionRef: string; generation: number }>) => VoiceIdentity | null
  permission: () => "granted" | "denied" | "not_determined" | Promise<"granted" | "denied" | "not_determined">
  requestPermission: () => "granted" | "denied" | Promise<"granted" | "denied">
  media: VoiceNativeMedia
  now?: () => number
}>): DesktopVoiceHost => {
  let current: DesktopVoiceState = { protocolVersion: 1, phase: "idle", generation: 0, nextSequence: 0, acknowledgedSequence: 0, capture: false, egress: false, playback: false, retainedAudio: false, activity: "stopped" }
  let session: VoiceNativeMediaSession | null = null
  let captureWanted = false
  let playbackActive = false
  let disposed = false
  const listeners = new Set<(state: DesktopVoiceState) => void>()
  const publish = (next: Omit<DesktopVoiceState, "capture" | "egress" | "playback"> & Partial<Pick<DesktopVoiceState, "capture" | "egress" | "playback">>) => {
    const live = next.phase === "live"; const muted = next.phase === "muted"
    current = { ...next, capture: live && captureWanted, egress: live && captureWanted, playback: playbackActive && (live || muted) }
    for (const listener of listeners) listener(current)
  }
  // Transport watermarks are queryable host state, but they are not visible UI
  // state. Publishing every 100 ms packet and ACK forces the whole Desktop shell
  // to reconcile continuously (including hover and disclosure affordances).
  // Keep the exact counters locally and publish only the first retention edge.
  const updateTransportState = (next: DesktopVoiceState) => { current = next }
  const close = (reason: "stop" | "revoke" | "replace" | "suspend" | "shutdown") => { const owned = session; session = null; owned?.close(reason) }
  const typedFailure = (phase: DesktopVoiceState["phase"], reason: DesktopVoiceState["reason"]) => publish({ ...current, phase, ...(reason === undefined ? {} : { reason }) })

  const command = async (command: DesktopVoiceCommand): Promise<DesktopVoiceState> => {
    if (disposed) return current
    if (command.id === "voice.speak") {
      if (session !== null && (current.phase === "live" || current.phase === "muted")) await session.speak({ turnRef: command.turnRef, speechRef: command.speechRef, messageRef: command.messageRef, text: command.text })
      return current
    }
    if (command.id === "voice.start") {
      close("replace")
      const generation = current.generation + 1
      const identity = input.resolveIdentity({ threadRef: command.threadRef, sessionRef: command.sessionRef, generation })
      if (identity === null) { typedFailure("failed", "helper_crashed"); return current }
      captureWanted = false
      publish({ protocolVersion: 1, phase: "requesting_permission", generation, nextSequence: 0, acknowledgedSequence: 0, retainedAudio: false, activity: "permission", transcript: undefined, proposal: undefined })
      const observedPermission = await input.permission()
      const permission = observedPermission === "not_determined" ? await input.requestPermission() : observedPermission
      if (permission !== "granted") { typedFailure("denied", "permission_denied"); return current }
      captureWanted = true
      publish({ ...current, phase: "connecting", activity: "connecting" })
      const ownedGeneration = generation
      try { session = await input.media.open({
        identity, disclosureRef: command.disclosureRef,
        onPacket: packet => {
          if (session === null || packet.generation !== ownedGeneration || current.generation !== ownedGeneration || current.phase !== "live") return
          updateTransportState({ ...current, nextSequence: Math.max(current.nextSequence, packet.sequence + 1) })
        },
        onAck: (sequence, ackGeneration) => {
          if (ackGeneration !== ownedGeneration || current.generation !== ownedGeneration) { typedFailure("failed", "stale_generation"); return }
          const firstRetention = !current.retainedAudio
          const next = { ...current, acknowledgedSequence: Math.max(current.acknowledgedSequence, sequence), retainedAudio: true }
          if (firstRetention) publish(next)
          else updateTransportState(next)
        },
        onState: state => {
          if (current.generation !== ownedGeneration) return
          if (state === "live") { session?.setCaptureEnabled(captureWanted); publish({ ...current, phase: captureWanted ? "live" : "muted", activity: captureWanted ? "listening" : "muted", reason: undefined }) }
          else if (state === "offline") { session?.setCaptureEnabled(false); publish({ ...current, phase: "offline", activity: "reconnecting", reason: "network_lost" }) }
          else if (state === "backpressured") { session?.setCaptureEnabled(false); publish({ ...current, phase: "backpressured", activity: "degraded", reason: "backpressure" }) }
          else if (state === "device_changed") { session?.setCaptureEnabled(false); publish({ ...current, phase: "device_changed", activity: "degraded", reason: "device_changed" }) }
          else if (state === "revoked") { close("revoke"); publish({ ...current, phase: "revoked", activity: "revoked", reason: "gateway_revoked" }) }
          else { close("shutdown"); typedFailure("failed", "helper_crashed") }
        },
        onControl: control => {
          if (current.generation !== ownedGeneration) return
          if (control.kind === "transcript") publish({ ...current, activity: control.final ? "listening" : "transcribing", transcript: { utteranceRef: control.utteranceRef, text: control.text.slice(0, 16_384), final: control.final } })
          else if (control.kind === "activity") publish({ ...current, activity: control.activity })
          else if (control.kind === "proposal") {
            const commandTargets = control.commandId === control.targetRef ||
              ((control.commandId === "conversation.interrupt" || control.commandId === "conversation.followup") && control.targetRef === control.turnRef)
            const state = !commandTargets || control.expiresAtMs <= (input.now?.() ?? Date.now()) ? "refused" as const : "proposed" as const
            publish({ ...current, activity: state === "proposed" ? "awaiting_confirmation" : "listening", proposal: { ...control, state } })
          } else { playbackActive = control.state === "speaking"; publish({ ...current, activity: playbackActive ? "speaking" : "listening", ...(control.outcomeRef === undefined ? {} : { playbackOutcomeRef: control.outcomeRef }) }) }
        },
      }) } catch (error) {
        // Keep credentials and remote payloads opaque while retaining the
        // bounded local failure code needed to diagnose the real Desktop path.
        console.error("[openagents-desktop:voice] media open failed:", error instanceof Error ? error.message : "unknown_error")
        typedFailure("failed", "helper_crashed")
      }
      return current
    }
    if (command.id === "voice.mute") { captureWanted = false; session?.setCaptureEnabled(false); publish({ ...current, phase: "muted", activity: "muted" }); return current }
    if (command.id === "voice.unmute") { captureWanted = true; if (session !== null) { session.setCaptureEnabled(true); publish({ ...current, phase: "live", activity: "listening", reason: undefined }) }; return current }
    if (command.id === "voice.suspend") { captureWanted = false; session?.setCaptureEnabled(false); publish({ ...current, phase: "suspended" }); return current }
    if (command.id === "voice.resume") { captureWanted = true; if (session !== null) { session.setCaptureEnabled(true); publish({ ...current, phase: "live", reason: undefined }) }; return current }
    captureWanted = false
    close(command.id === "voice.revoke" ? "revoke" : "stop")
    publish({ ...current, phase: command.id === "voice.revoke" ? "revoked" : "idle", activity: command.id === "voice.revoke" ? "revoked" : "stopped", ...(command.id === "voice.revoke" ? { reason: "gateway_revoked" as const } : { reason: undefined }) })
    return current
  }
  return { command, state: () => current, subscribe: listener => { listeners.add(listener); return () => listeners.delete(listener) }, dispose: () => { if (disposed) return; disposed = true; close("shutdown"); listeners.clear(); publish({ ...current, phase: "idle", reason: undefined }) } }
}
