import { setTimeout as sleep } from "node:timers/promises"
import { expect, test } from "vite-plus/test"
import { FakeSttAdapter } from "./stt"
import { AudioSession } from "./session"
import { decodeMediaHeader } from "@openagentsinc/audio-contract"
import { FakeTtsAdapter, normalizeSpoken, trimLeadingSilencePcm, type TtsAdapter, type TtsChunkStream } from "./tts"

const identity = { ownerRef: "owner", deviceRef: "device", threadRef: "thread", sessionRef: "session", generation: 3 }
const decodeBinary = (frame: Uint8Array) => {
  const headerLength = new DataView(frame.buffer, frame.byteOffset + 4, 4).getUint32(0)
  return { header: decodeMediaHeader(JSON.parse(Buffer.from(frame.subarray(8, 8 + headerLength)).toString("utf8"))), payload: frame.subarray(8 + headerLength) }
}

test("spoken normalization is conservative and leading PCM silence is removed", () => {
  expect(normalizeSpoken("Open the AI API at openagents.com")).toBe("Open the A.I. A.P.I. at open agents dot com")
  expect([...trimLeadingSilencePcm(new Uint8Array([0, 0, 1, 0, 40, 0]))]).toEqual([40, 0])
})

test("TTS binds canonical text, turn, speech and generation; chunks/order/receipt stay text-free", async () => {
  const stt = new FakeSttAdapter(); const textFrames: any[] = []; const binary: Uint8Array[] = []; const receipts: any[] = []
  const tts = new FakeTtsAdapter([new Uint8Array([0, 0, 100, 0]), new Uint8Array(30_000).fill(1)])
  let now = 1_000
  const session = new AudioSession(identity, stt, { sendText: value => { textFrames.push(value); return true }, sendBinary: value => { binary.push(value); now += 10; return true }, close: () => {} }, { adapter: tts, now: () => now, receipt: value => receipts.push(value) })
  const receipt = await session.speak({ turnRef: "turn.1", speechRef: "speech.1", messageRef: "message.1", text: "Canonical AI answer" })
  expect(textFrames[0]).toMatchObject({ _tag: "assistant_text", turnRef: "turn.1", speechRef: "speech.1", text: "Canonical AI answer" })
  expect(binary).toHaveLength(3)
  for (const frame of binary) expect(decodeBinary(frame).header).toMatchObject({ kind: "server_tts", identity, turnRef: "turn.1", speechRef: "speech.1" })
  expect(receipt).toMatchObject({ outcome: "completed", chunksOut: 3, charsIn: 19, synthTtfbMs: 0 })
  expect(JSON.stringify(receipts)).not.toContain("Canonical")
})

test("provider failure and absent TTS preserve assistant text and return typed fallback", async () => {
  const frames: any[] = []
  const failing: TtsAdapter = { adapterRef: "failing", synthesize: () => Object.assign((async function* () { throw new Error("provider") })(), { cancel: () => {} }) }
  const session = new AudioSession(identity, new FakeSttAdapter(), { sendText: value => { frames.push(value); return true }, sendBinary: () => true, close: () => {} }, { adapter: failing })
  const receipt = await session.speak({ turnRef: "turn.1", speechRef: "speech.error", messageRef: "message.1", text: "Visible even without audio" })
  expect(frames[0]).toMatchObject({ _tag: "assistant_text", text: "Visible even without audio" })
  expect(receipt.outcome).toBe("provider_error")
})

test("qualified speech cancels synthesis, emits outcome ref, while noise/backchannel does not", async () => {
  const stt = new FakeSttAdapter(); const frames: any[] = []; let canceled = false; let release!: () => void
  const gate = new Promise<void>(resolve => { release = resolve })
  const stream = Object.assign((async function* () { yield new Uint8Array([1, 0]); await gate; if (!canceled) yield new Uint8Array([2, 0]) })(), { cancel: () => { canceled = true; release() } }) satisfies TtsChunkStream
  const tts: TtsAdapter = { adapterRef: "controlled", synthesize: () => stream }
  const session = new AudioSession(identity, stt, { sendText: value => { frames.push(value); return true }, sendBinary: () => true, close: () => {} }, { adapter: tts, now: () => 1_000, onBargeIn: async () => "outcome.interrupt.1" })
  const speaking = session.speak({ turnRef: "turn.9", speechRef: "speech.9", messageRef: "message.9", text: "long answer" })
  await sleep(1)
  stt.streams[0]!.events({ _tag: "speech_begin" }); stt.streams[0]!.events({ _tag: "interim", text: "uh", stability: 0.1 })
  expect(canceled).toBe(false)
  stt.streams[0]!.events({ _tag: "interim", text: "stop now", stability: 0.8 })
  const receipt = await speaking
  expect(canceled).toBe(true); expect(receipt.outcome).toBe("canceled")
  expect(frames).toContainEqual(expect.objectContaining({ _tag: "playback_cancel", turnRef: "turn.9", speechRef: "speech.9", outcomeRef: "outcome.interrupt.1" }))
})

test("a newer utterance cancels the old stream and stale chunks cannot resume", async () => {
  const stt = new FakeSttAdapter(); const binary: Uint8Array[] = []; const requests: Array<{ canceled: boolean; release: () => void }> = []
  const tts: TtsAdapter = { adapterRef: "ordered", synthesize: () => {
    let release!: () => void; const gate = new Promise<void>(resolve => { release = resolve }); const request = { canceled: false, release }; requests.push(request)
    return Object.assign((async function* () { await gate; if (!request.canceled) yield new Uint8Array([1, 0]) })(), { cancel: () => { request.canceled = true; release() } })
  } }
  const session = new AudioSession(identity, stt, { sendText: () => true, sendBinary: value => { binary.push(value); return true }, close: () => {} }, { adapter: tts })
  const old = session.speak({ turnRef: "turn.1", speechRef: "speech.old", messageRef: "m1", text: "old" }); await sleep(1)
  const fresh = session.speak({ turnRef: "turn.2", speechRef: "speech.new", messageRef: "m2", text: "new" }); requests[1]!.release()
  expect((await old).outcome).toBe("canceled"); expect((await fresh).outcome).toBe("completed")
  expect(binary.map(frame => decodeBinary(frame).header).every(header => header.kind === "server_tts" && header.speechRef === "speech.new")).toBe(true)
})
