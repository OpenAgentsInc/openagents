import { describe, expect, test } from "vite-plus/test"
import { decodeClientFrame, decodeMediaHeader, MAX_AUDIO_PAYLOAD_BYTES } from "./index"
const identity = { ownerRef: "owner:1", deviceRef: "device:1", threadRef: "thread:1", sessionRef: "session:1", generation: 1 }
const digest = "0".repeat(64)
describe("openagents audio v1 schema", () => {
  test("accepts bounded exact frames", () => expect(decodeClientFrame({ schema: "openagents.audio.v1", _tag: "audio_chunk", identity, sequence: 1, payloadLength: MAX_AUDIO_PAYLOAD_BYTES, sha256: digest })._tag).toBe("audio_chunk"))
  test("rejects unknown version, tag, excess fields, and oversized media", () => {
    for (const value of [
      { schema: "openagents.audio.v2", _tag: "mute", identity, sequence: 1 },
      { schema: "openagents.audio.v1", _tag: "model_says_execute", identity, sequence: 1 },
      { schema: "openagents.audio.v1", _tag: "mute", identity, sequence: 1, token: "secret" },
    ]) expect(() => decodeClientFrame(value)).toThrow()
    expect(() => decodeMediaHeader({ schema: "openagents.audio.v1", kind: "client_audio", identity, sequence: 1, codec: "pcm_s16le", sampleRateHz: 16_000, channels: 1, payloadLength: MAX_AUDIO_PAYLOAD_BYTES + 1, sha256: digest })).toThrow()
  })
})
