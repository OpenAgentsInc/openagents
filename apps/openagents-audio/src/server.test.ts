import { afterEach, expect, test } from "bun:test"
import { mintAudioGrant } from "./auth"
import { startAudioServer } from "./server"
import { FakeSttAdapter } from "./stt"
import { identity, mediaFrame } from "./test-support"
let stop: (() => Promise<void>) | undefined; afterEach(async () => { await stop?.(); stop = undefined })
test("real WebSocket refuses auth and accepts bounded binary AUDIO-1 frames", async () => {
  const secret = "z".repeat(32); const running = startAudioServer({ tokenSecret: secret, adapter: new FakeSttAdapter(), port: 0 }); stop = running.stop
  expect((await fetch(`http://127.0.0.1:${running.port}/v1/stream`)).status).toBe(426)
  const token = mintAudioGrant({ identity, expiresAtMs: Date.now() + 60_000 }, secret)
  const socket = new WebSocket(`ws://127.0.0.1:${running.port}/v1/stream?token=${token}`)
  const result = await new Promise<any>((resolve, reject) => { socket.onopen = () => socket.send(Buffer.from(mediaFrame(1))); socket.onmessage = (e) => resolve(JSON.parse(String(e.data))); socket.onerror = reject })
  expect(result).toMatchObject({ _tag: "ack", acknowledgedClientSequence: 1, identity }); socket.close()
})
test("real WebSocket refuses an invalid application grant", async () => {
  const running = startAudioServer({ tokenSecret: "q".repeat(32), adapter: new FakeSttAdapter(), port: 0 }); stop = running.stop
  const socket = new WebSocket(`ws://127.0.0.1:${running.port}/v1/stream?token=invalid`)
  const refused = await new Promise<boolean>((resolve) => { socket.onopen = () => resolve(false); socket.onerror = () => resolve(true); socket.onclose = () => resolve(true) })
  expect(refused).toBeTrue()
})
