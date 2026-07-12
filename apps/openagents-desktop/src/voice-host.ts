import { Schema } from "@effect-native/core/effect"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"

export const DesktopVoiceHostProtocolVersion = 1 as const

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256))
export const DesktopVoiceCommandSchema = Schema.Union([
  Schema.Struct({ protocolVersion: Schema.Literal(1), id: Schema.Literal("voice.start"), identity: Schema.declare<VoiceIdentity>((value): value is VoiceIdentity => typeof value === "object" && value !== null), disclosureRef: Ref }),
  Schema.Struct({ protocolVersion: Schema.Literal(1), id: Schema.Literals(["voice.stop", "voice.mute", "voice.unmute", "voice.suspend", "voice.resume", "voice.revoke"]) }),
])
export type DesktopVoiceCommand = typeof DesktopVoiceCommandSchema.Type

export type DesktopVoiceState = Readonly<{
  protocolVersion: 1
  phase: "idle" | "requesting_permission" | "connecting" | "live" | "muted" | "suspended" | "denied" | "offline" | "backpressured" | "revoked" | "failed"
  generation: number
  nextSequence: number
  acknowledgedSequence: number
  reason?: "permission_denied" | "network_lost" | "gateway_revoked" | "helper_crashed" | "stale_generation" | "backpressure"
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
    onState: (state: "offline" | "backpressured" | "live" | "revoked" | "crashed") => void
  }>) => VoiceNativeMediaSession
}>

export type DesktopVoiceHost = Readonly<{
  command: (command: DesktopVoiceCommand) => Promise<DesktopVoiceState>
  state: () => DesktopVoiceState
  subscribe: (listener: (state: DesktopVoiceState) => void) => () => void
  dispose: () => void
}>

export const createDesktopVoiceHost = (input: Readonly<{
  permission: () => "granted" | "denied" | "not_determined" | Promise<"granted" | "denied" | "not_determined">
  requestPermission: () => "granted" | "denied" | Promise<"granted" | "denied">
  media: VoiceNativeMedia
}>): DesktopVoiceHost => {
  let current: DesktopVoiceState = { protocolVersion: 1, phase: "idle", generation: 0, nextSequence: 0, acknowledgedSequence: 0 }
  let session: VoiceNativeMediaSession | null = null
  let disposed = false
  const listeners = new Set<(state: DesktopVoiceState) => void>()
  const publish = (next: DesktopVoiceState) => { current = next; for (const listener of listeners) listener(next) }
  const close = (reason: "stop" | "revoke" | "replace" | "suspend" | "shutdown") => { const owned = session; session = null; owned?.close(reason) }
  const typedFailure = (phase: DesktopVoiceState["phase"], reason: DesktopVoiceState["reason"]) => publish({ ...current, phase, ...(reason === undefined ? {} : { reason }) })

  const command = async (command: DesktopVoiceCommand): Promise<DesktopVoiceState> => {
    if (disposed) return current
    if (command.id === "voice.start") {
      close("replace")
      const generation = Math.max(current.generation + 1, command.identity.generation)
      publish({ protocolVersion: 1, phase: "requesting_permission", generation, nextSequence: 0, acknowledgedSequence: 0 })
      const observedPermission = await input.permission()
      const permission = observedPermission === "not_determined" ? await input.requestPermission() : observedPermission
      if (permission !== "granted") { typedFailure("denied", "permission_denied"); return current }
      publish({ ...current, phase: "connecting" })
      const ownedGeneration = generation
      try { session = input.media.open({
        identity: { ...command.identity, generation }, disclosureRef: command.disclosureRef,
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
          if (state === "live") publish({ ...current, phase: "live", reason: undefined })
          else if (state === "offline") typedFailure("offline", "network_lost")
          else if (state === "backpressured") typedFailure("backpressured", "backpressure")
          else if (state === "revoked") { close("revoke"); typedFailure("revoked", "gateway_revoked") }
          else { close("shutdown"); typedFailure("failed", "helper_crashed") }
        },
      }) } catch { typedFailure("failed", "helper_crashed") }
      return current
    }
    if (command.id === "voice.mute") { session?.setCaptureEnabled(false); publish({ ...current, phase: "muted" }); return current }
    if (command.id === "voice.unmute") { if (session !== null) { session.setCaptureEnabled(true); publish({ ...current, phase: "live", reason: undefined }) }; return current }
    if (command.id === "voice.suspend") { session?.setCaptureEnabled(false); publish({ ...current, phase: "suspended" }); return current }
    if (command.id === "voice.resume") { if (session !== null) { session.setCaptureEnabled(true); publish({ ...current, phase: "live", reason: undefined }) }; return current }
    close(command.id === "voice.revoke" ? "revoke" : "stop")
    publish({ ...current, phase: command.id === "voice.revoke" ? "revoked" : "idle", ...(command.id === "voice.revoke" ? { reason: "gateway_revoked" as const } : { reason: undefined }) })
    return current
  }
  return { command, state: () => current, subscribe: listener => { listeners.add(listener); return () => listeners.delete(listener) }, dispose: () => { if (disposed) return; disposed = true; close("shutdown"); listeners.clear(); publish({ ...current, phase: "idle", reason: undefined }) } }
}
