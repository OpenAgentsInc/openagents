import { expect, test } from "bun:test"
import { decodeBinaryMediaFrame, GOOGLE_STREAM_AUDIO_LIMIT } from "./media"
import { mediaFrame } from "./test-support"
test("bounded binary media validates framing, digest, and Google limit", () => {
  expect(decodeBinaryMediaFrame(mediaFrame(1)).header.sequence).toBe(1)
  const corrupt = mediaFrame(1); corrupt[corrupt.length - 1] ^= 1; expect(() => decodeBinaryMediaFrame(corrupt)).toThrow("digest")
  expect(() => decodeBinaryMediaFrame(mediaFrame(1, new Uint8Array(GOOGLE_STREAM_AUDIO_LIMIT + 1)))).toThrow("payload_length")
})
