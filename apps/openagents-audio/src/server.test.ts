import { afterEach, expect, test } from "bun:test"
import { mintAudioGrant } from "./auth"
import { startAudioServer } from "./server"
import { FakeSttAdapter } from "./stt"
import { identity, mediaFrame } from "./test-support"
import { FakeTtsAdapter } from "./tts"
import { decodeMediaHeader } from "@openagentsinc/audio-contract"
let stop: (() => Promise<void>) | undefined; afterEach(async () => { await stop?.(); stop = undefined })
test("real WebSocket refuses auth and accepts bounded binary AUDIO-1 frames", async () => {
  const secret = "z".repeat(32); const running = startAudioServer({ tokenSecret: secret, adapter: new FakeSttAdapter(), port: 0 }); stop = running.stop
  expect((await fetch(`http://127.0.0.1:${running.port}/v1/stream`)).status).toBe(426)
  const token = mintAudioGrant({ identity, expiresAtMs: Date.now() + 60_000 }, secret)
  const socket = new WebSocket(`ws://127.0.0.1:${running.port}/v1/stream?token=${token}`)
  const result = await new Promise<any>((resolve, reject) => { socket.onopen = () => socket.send(Buffer.from(mediaFrame(0))); socket.onmessage = (e) => resolve(JSON.parse(String(e.data))); socket.onerror = reject })
  expect(result).toMatchObject({ _tag: "ack", acknowledgedClientSequence: 0, identity }); socket.close()
})
test("real WebSocket refuses an invalid application grant", async () => {
  const running = startAudioServer({ tokenSecret: "q".repeat(32), adapter: new FakeSttAdapter(), port: 0 }); stop = running.stop
  const socket = new WebSocket(`ws://127.0.0.1:${running.port}/v1/stream?token=invalid`)
  const refused = await new Promise<boolean>((resolve) => { socket.onopen = () => resolve(false); socket.onerror = () => resolve(true); socket.onclose = () => resolve(true) })
  expect(refused).toBeTrue()
})
test("canonical speak route streams identity/turn/speech-bound PCM and returns text-free receipt", async () => {
  const secret = "t".repeat(32); const running = startAudioServer({ tokenSecret: secret, adapter: new FakeSttAdapter(), tts: new FakeTtsAdapter([new Uint8Array([100, 0, 101, 0])]), port: 0 }); stop = running.stop
  const token = mintAudioGrant({ identity, expiresAtMs: Date.now() + 60_000 }, secret)
  const socket = new WebSocket(`ws://127.0.0.1:${running.port}/v1/stream?token=${token}`)
  const received: Array<string | Uint8Array> = []
  await new Promise<void>((resolve, reject) => { socket.onopen = () => resolve(); socket.onerror = reject })
  socket.onmessage = event => received.push(typeof event.data === "string" ? event.data : new Uint8Array(event.data as ArrayBuffer))
  const response = await fetch(`http://127.0.0.1:${running.port}/v1/speak`, { method: "POST", headers: { "content-type": "application/json", "x-openagents-audio-grant": token }, body: JSON.stringify({ turnRef: "turn.1", speechRef: "speech.1", messageRef: "message.1", text: "Canonical visible reply" }) })
  expect(response.status).toBe(200); const receipt = await response.json() as any
  expect(receipt).toMatchObject({ outcome: "completed", chunksOut: 1, charsIn: 23 }); expect(JSON.stringify(receipt)).not.toContain("Canonical")
  await Bun.sleep(10)
  const binary = received.find(value => value instanceof Uint8Array) as Uint8Array
  const headerLength = new DataView(binary.buffer, binary.byteOffset + 4, 4).getUint32(0)
  expect(decodeMediaHeader(JSON.parse(Buffer.from(binary.subarray(8, 8 + headerLength)).toString("utf8")))).toMatchObject({ identity, turnRef: "turn.1", speechRef: "speech.1" })
  expect(received.filter(value => typeof value === "string").map(value => JSON.parse(value as string)._tag)).toContain("assistant_text")
  socket.close()
})
test("retained sessions announce policy and ACK only after the durable accept hook", async () => {
  const secret = "r".repeat(32); let release!: () => void; const durable = new Promise<void>(resolve => { release = resolve }); let accepted = 0
  const running = startAudioServer({ tokenSecret: secret, adapter: new FakeSttAdapter(), port: 0, retention: {
    admit: async () => ({ receipt: { receiptRef: "retained.1", expiresAtMs: Date.now() + 60_000 }, accept: async () => { accepted++; await durable }, gap: async () => {}, stop: async () => {} }),
    reconcile: async () => ({}), exportSession: async () => ({}), deleteSession: async () => ({}), close: async () => {},
  } }); stop = running.stop
  const token = mintAudioGrant({ identity, expiresAtMs: Date.now() + 60_000 }, secret)
  const socket = new WebSocket(`ws://127.0.0.1:${running.port}/v1/stream?token=${token}`); const frames: any[] = []
  await new Promise<void>((resolve, reject) => { socket.onopen = () => resolve(); socket.onerror = reject })
  socket.onmessage = event => frames.push(JSON.parse(String(event.data)))
  await Bun.sleep(5); socket.send(Buffer.from(mediaFrame(0))); await Bun.sleep(5)
  expect(frames[0]).toMatchObject({ _tag: "retention_receipt", receipt: { receiptRef: "retained.1" } }); expect(accepted).toBe(1); expect(frames.some(frame => frame._tag === "ack")).toBeFalse()
  release(); await Bun.sleep(5); expect(frames.at(-1)).toMatchObject({ _tag: "ack", acknowledgedClientSequence: 0 }); socket.close()
})
