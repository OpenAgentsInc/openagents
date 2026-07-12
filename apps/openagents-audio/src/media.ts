import { createHash } from "node:crypto"
import { AUDIO_MEDIA_MAGIC, decodeMediaHeader, MAX_AUDIO_PAYLOAD_BYTES } from "@openagentsinc/audio-contract"

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
