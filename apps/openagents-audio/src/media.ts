import { createHash } from "node:crypto"
import { AUDIO_MEDIA_MAGIC, decodeMediaHeader, MAX_AUDIO_PAYLOAD_BYTES } from "@openagentsinc/audio-contract"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"

export const GOOGLE_STREAM_AUDIO_LIMIT = 15_360
export type DecodedMediaFrame = Readonly<{ header: ReturnType<typeof decodeMediaHeader>; payload: Uint8Array }>
export const decodeBinaryMediaFrame = (input: Uint8Array): DecodedMediaFrame => {
  if (input.byteLength < 8 || Buffer.from(input.subarray(0, 4)).toString("ascii") !== AUDIO_MEDIA_MAGIC) throw new Error("audio_frame_magic")
  const headerLength = new DataView(input.buffer, input.byteOffset + 4, 4).getUint32(0)
  if (headerLength < 2 || headerLength > 8_192 || 8 + headerLength > input.byteLength) throw new Error("audio_frame_header_length")
  const header = decodeMediaHeader(JSON.parse(Buffer.from(input.subarray(8, 8 + headerLength)).toString("utf8")))
  const payload = input.subarray(8 + headerLength)
  if (header.kind !== "client_audio" || payload.byteLength !== header.payloadLength || payload.byteLength > Math.min(MAX_AUDIO_PAYLOAD_BYTES, GOOGLE_STREAM_AUDIO_LIMIT)) throw new Error("audio_frame_payload_length")
  if (createHash("sha256").update(payload).digest("hex") !== header.sha256) throw new Error("audio_frame_digest")
  return { header, payload }
}

export const encodeServerTtsMediaFrame = (input: Readonly<{
  identity: VoiceIdentity; sequence: number; turnRef: string; speechRef: string; payload: Uint8Array
}>): Uint8Array => {
  if (input.payload.byteLength === 0 || input.payload.byteLength > MAX_AUDIO_PAYLOAD_BYTES || input.payload.byteLength % 2 !== 0) throw new Error("tts_payload_length")
  const header = Buffer.from(JSON.stringify({
    schema: "openagents.audio.v1", kind: "server_tts", identity: input.identity,
    sequence: input.sequence, turnRef: input.turnRef, speechRef: input.speechRef,
    codec: "pcm_s16le", sampleRateHz: 24_000, channels: 1,
    payloadLength: input.payload.byteLength,
    sha256: createHash("sha256").update(input.payload).digest("hex"),
  }))
  const output = Buffer.allocUnsafe(8 + header.byteLength + input.payload.byteLength)
  output.write(AUDIO_MEDIA_MAGIC, 0, "ascii"); output.writeUInt32BE(header.byteLength, 4)
  header.copy(output, 8); Buffer.from(input.payload).copy(output, 8 + header.byteLength)
  return output
}
