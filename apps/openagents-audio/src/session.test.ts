import { expect, test } from "bun:test"
import { AudioSession, ROTATE_AFTER_AUDIO_BYTES } from "./session"
import { FakeSttAdapter } from "./stt"
import { identity, mediaFrame } from "./test-support"
test("session ACK/gap/generation/interim/final/cancel behavior", () => {
  const adapter = new FakeSttAdapter(); const sent: any[] = []; const closed: unknown[] = []
  const session = new AudioSession(identity, adapter, { sendText: (x) => { sent.push(x); return true }, close: (...x) => { closed.push(x) } })
  session.receive(mediaFrame(1)); session.receive(mediaFrame(1)); session.receive(mediaFrame(3))
  expect(sent.map((x) => x._tag)).toEqual(["ack", "ack", "gap"])
  adapter.streams[0]!.events({ _tag: "interim", text: "hel", stability: 0.5 }); adapter.streams[0]!.events({ _tag: "interim", text: "hello", stability: 0.9 }); adapter.streams[0]!.events({ _tag: "final", text: "hello" })
  expect(sent.slice(-3).map((x) => [x._tag, x.utteranceRef])).toEqual([["transcript_interim", "utterance:session:1:1"], ["transcript_interim", "utterance:session:1:1"], ["transcript_final", "utterance:session:1:1"]])
  expect(() => session.receive(mediaFrame(2, undefined, { ...identity, generation: 2 }))).toThrow("identity_or_generation")
  expect(adapter.streams[0]!.cancelled).toBeTrue(); expect(closed).toHaveLength(1)
})
test("provider quota/deadline are public-safe and stop has cancellation semantics", () => {
  const adapter = new FakeSttAdapter(); const sent: any[] = []; const session = new AudioSession(identity, adapter, { sendText: (x) => { sent.push(x); return true }, close: () => {} })
  adapter.streams[0]!.events({ _tag: "error", code: "quota" }); expect(sent.at(-1)).toMatchObject({ _tag: "close", reason: "stt_quota" })
  session.stop()
})
test("provider stream rotates before its bound without losing sequence", () => {
  const adapter = new FakeSttAdapter(); const sent: any[] = []
  const session = new AudioSession(identity, adapter, { sendText: (x) => { sent.push(x); return true }, close: () => {} })
  const payload = new Uint8Array(15_000)
  const count = Math.ceil(ROTATE_AFTER_AUDIO_BYTES / payload.byteLength)
  for (let sequence = 1; sequence <= count; sequence++) session.receive(mediaFrame(sequence, payload))
  expect(adapter.streams).toHaveLength(2)
  expect(sent.at(-1)).toMatchObject({ _tag: "ack", acknowledgedClientSequence: count })
  adapter.streams[1]!.events({ _tag: "final", text: "one final" })
  expect(sent.filter((frame) => frame._tag === "transcript_final")).toHaveLength(1)
})
