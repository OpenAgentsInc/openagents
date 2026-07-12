export type VoicePhase = "stopped" | "starting" | "connecting" | "listening" | "speech" | "transcribing" | "awaiting_confirmation" | "executing" | "speaking" | "muted" | "reconnecting" | "degraded" | "revoked"
export type VoiceModel = Readonly<{
  phase: VoicePhase; generation: number | null; capture: boolean; egress: boolean;
  retention: boolean; playback: boolean; lastClientSeq: number; ackedClientSeq: number;
  finals: ReadonlySet<string>; sideEffects: number; disclosureRef: string | null; receiptRef: string | null;
}>
export const stoppedVoiceModel = (): VoiceModel => ({ phase: "stopped", generation: null, capture: false, egress: false, retention: false, playback: false, lastClientSeq: 0, ackedClientSeq: 0, finals: new Set(), sideEffects: 0, disclosureRef: null, receiptRef: null })
const fail = (reason: string): never => { throw new Error(reason) }
export const start = (m: VoiceModel, generation: number, disclosureRef: string): VoiceModel => {
  if (!(m.phase === "stopped" || m.phase === "revoked")) fail("active generation")
  if (!disclosureRef || generation <= (m.generation ?? 0)) fail("disclosure/new generation required")
  return { ...m, phase: "starting", generation, capture: false, egress: false, retention: false, playback: false, lastClientSeq: 0, ackedClientSeq: 0, finals: new Set(), disclosureRef, receiptRef: null }
}
export const connected = (m: VoiceModel, generation: number): VoiceModel => generation === m.generation && m.phase === "starting" ? { ...m, phase: "listening", capture: true, egress: true } : fail("stale connection")
export const acceptAudio = (m: VoiceModel, generation: number, sequence: number): VoiceModel => {
  if (generation !== m.generation || !m.capture || !m.egress) return fail("audio fenced")
  if (sequence !== m.lastClientSeq + 1) return fail(sequence <= m.lastClientSeq ? "duplicate/stale audio" : "sequence gap")
  return { ...m, phase: "speech", lastClientSeq: sequence, sideEffects: m.sideEffects + 1 }
}
export const acknowledge = (m: VoiceModel, sequence: number): VoiceModel => sequence < m.ackedClientSeq || sequence > m.lastClientSeq ? fail("invalid ack") : { ...m, ackedClientSeq: sequence }
export const publishFinal = (m: VoiceModel, ref: string, replay = false): VoiceModel => {
  if (m.finals.has(ref)) return replay ? m : fail("duplicate final")
  if (replay) return fail("replay cannot create final")
  return { ...m, phase: "transcribing", finals: new Set([...m.finals, ref]), sideEffects: m.sideEffects + 1 }
}
export const acceptRetentionReceipt = (m: VoiceModel, generation: number, disclosureRef: string, receiptRef: string): VoiceModel => generation === m.generation && disclosureRef === m.disclosureRef ? { ...m, retention: true, receiptRef } : fail("receipt identity mismatch")
export const mute = (m: VoiceModel): VoiceModel => ({ ...m, phase: "muted", capture: false, egress: false, playback: false })
export const stop = (m: VoiceModel, revoked = false): VoiceModel => ({ ...m, phase: revoked ? "revoked" : "stopped", capture: false, egress: false, retention: false, playback: false, receiptRef: null })
export const replayDelivery = (m: VoiceModel): VoiceModel => ({ ...m })
export const observeProse = (m: VoiceModel): VoiceModel => ({ ...m })
