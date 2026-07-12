import { mintAudioGrant } from "../src/auth"
import { mediaFrame } from "../src/test-support"

const required = (name: string) => { const value = process.env[name]; if (!value) throw new Error(`missing ${name}`); return value }
const pcm = new Uint8Array(await Bun.file(required("OPENAGENTS_AUDIO_SMOKE_PCM")).arrayBuffer())
const identity = { ownerRef: "smoke:owner", deviceRef: "smoke:device", threadRef: "smoke:thread", sessionRef: `smoke:${Date.now()}`, generation: 1 }
const grant = mintAudioGrant({ identity, expiresAtMs: Date.now() + 5 * 60_000 }, required("OPENAGENTS_AUDIO_TOKEN_SECRET"))
const base = required("OPENAGENTS_AUDIO_URL").replace(/^http/, "ws")
const started = Date.now(); let sequence = 0; let finalCount = 0; let gapCount = 0
const socket = new WebSocket(`${base}/v1/stream?token=${encodeURIComponent(grant)}`, { headers: { Authorization: `Bearer ${required("OPENAGENTS_AUDIO_CLOUD_RUN_ID_TOKEN")}` } } as any)
await new Promise<void>((resolve, reject) => {
  const timeout = setTimeout(() => reject(new Error("live smoke final timeout")), 45_000)
  socket.onerror = () => reject(new Error("live smoke websocket error"))
  socket.onopen = async () => {
    for (let offset = 0; offset < pcm.byteLength; offset += 8_000) {
      socket.send(Buffer.from(mediaFrame(++sequence, pcm.slice(offset, offset + 8_000), identity)))
      await Bun.sleep(125)
    }
    for (let n = 0; n < 4; n++) { socket.send(Buffer.from(mediaFrame(++sequence, new Uint8Array(8_000), identity))); await Bun.sleep(125) }
  }
  socket.onmessage = (message) => {
    const frame = JSON.parse(String(message.data)) as { _tag?: string }
    if (frame._tag === "gap") gapCount++
    if (frame._tag === "transcript_final") {
      finalCount++
      clearTimeout(timeout); socket.close(); resolve()
    }
  }
})
if (finalCount !== 1 || gapCount !== 0) throw new Error("live smoke receipt invalid")
console.log(JSON.stringify({ schema: "openagents.audio.stt_smoke.v1", finalCount, gapCount, audioBytes: pcm.byteLength, latencyMs: Date.now() - started, transcriptLogged: false }))
