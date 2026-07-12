import { Schema } from "@effect-native/core/effect"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"

export const DesktopVoiceHostProtocolVersion = 1 as const

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256))
export const DesktopVoiceCommandSchema = Schema.Union([
  Schema.Struct({ protocolVersion: Schema.Literal(1), id: Schema.Literal("voice.start"), threadRef: Ref, sessionRef: Ref, disclosureRef: Ref }),
  Schema.Struct({ protocolVersion: Schema.Literal(1), id: Schema.Literals(["voice.stop", "voice.mute", "voice.unmute", "voice.suspend", "voice.resume", "voice.revoke"]) }),
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
  proposal?: Readonly<{ proposalRef: string; targetRef: string; state: "proposed" | "applied" | "refused" }>
  reason?: "permission_denied" | "network_lost" | "gateway_revoked" | "helper_crashed" | "stale_generation" | "backpressure" | "device_changed"
}>

export type VoiceMediaPacket = Readonly<{ sequence: number; generation: number; payloadLength: number; sha256: string }>
export type VoiceNativeMediaSession = Readonly<{
  setCaptureEnabled: (enabled: boolean) => void
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
      | { kind: "proposal"; proposalRef: string; targetRef: string }
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
}>): DesktopVoiceHost => {
  let current: DesktopVoiceState = { protocolVersion: 1, phase: "idle", generation: 0, nextSequence: 0, acknowledgedSequence: 0, capture: false, egress: false, playback: false, retainedAudio: false, activity: "stopped" }
  let session: VoiceNativeMediaSession | null = null
  let captureWanted = false
  let disposed = false
  const listeners = new Set<(state: DesktopVoiceState) => void>()
  const publish = (next: Omit<DesktopVoiceState, "capture" | "egress" | "playback"> & Partial<Pick<DesktopVoiceState, "capture" | "egress" | "playback">>) => {
    const live = next.phase === "live"; const muted = next.phase === "muted"
    current = { ...next, capture: live && captureWanted, egress: live && captureWanted, playback: live || muted }
    for (const listener of listeners) listener(current)
  }
  const close = (reason: "stop" | "revoke" | "replace" | "suspend" | "shutdown") => { const owned = session; session = null; owned?.close(reason) }
  const typedFailure = (phase: DesktopVoiceState["phase"], reason: DesktopVoiceState["reason"]) => publish({ ...current, phase, ...(reason === undefined ? {} : { reason }) })

  const command = async (command: DesktopVoiceCommand): Promise<DesktopVoiceState> => {
    if (disposed) return current
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
          publish({ ...current, nextSequence: Math.max(current.nextSequence, packet.sequence + 1) })
        },
        onAck: (sequence, ackGeneration) => {
          if (ackGeneration !== ownedGeneration || current.generation !== ownedGeneration) { typedFailure("failed", "stale_generation"); return }
          publish({ ...current, acknowledgedSequence: Math.max(current.acknowledgedSequence, sequence) })
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
          else publish({ ...current, activity: "awaiting_confirmation", proposal: { proposalRef: control.proposalRef, targetRef: control.targetRef, state: "proposed" } })
        },
      }) } catch { typedFailure("failed", "helper_crashed") }
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
