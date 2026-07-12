import { createHash } from "node:crypto"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"
export const identity: VoiceIdentity = { ownerRef: "owner:1", deviceRef: "device:1", threadRef: "thread:1", sessionRef: "session:1", generation: 1 }
export const mediaFrame = (sequence: number, payload = Uint8Array.of(1, 2, 3, 4), useIdentity = identity): Uint8Array => {
  const header = { schema: "openagents.audio.v1", kind: "client_audio", identity: useIdentity, sequence, codec: "pcm_s16le", sampleRateHz: 16000, channels: 1, payloadLength: payload.byteLength, sha256: createHash("sha256").update(payload).digest("hex") }
  const json = Buffer.from(JSON.stringify(header)); const out = Buffer.alloc(8 + json.length + payload.length)
  out.write("OAA1", 0, "ascii"); out.writeUInt32BE(json.length, 4); json.copy(out, 8); Buffer.from(payload).copy(out, 8 + json.length); return out
}
