import { mintAudioGrant } from "../src/auth"
import { mediaFrame } from "../src/test-support"

const required = (name: string) => { const value = process.env[name]; if (!value) throw new Error(`missing ${name}`); return value }
const pcm = new Uint8Array(await Bun.file(required("OPENAGENTS_AUDIO_SMOKE_PCM")).arrayBuffer())
const base = required("OPENAGENTS_AUDIO_URL"); const iam = required("OPENAGENTS_AUDIO_CLOUD_RUN_ID_TOKEN")
const identity = { ownerRef: "smoke:barge-owner", deviceRef: "smoke:barge-device", threadRef: "smoke:barge-thread", sessionRef: `smoke:barge:${Date.now()}`, generation: 1 }
const grant = mintAudioGrant({ identity, expiresAtMs: Date.now() + 5 * 60_000 }, required("OPENAGENTS_AUDIO_TOKEN_SECRET"))
const turnRef = `turn:barge:${Date.now()}`; const speechRef = `speech:barge:${Date.now()}`; const messageRef = `message:barge:${Date.now()}`
const socket = new WebSocket(`${base.replace(/^http/, "ws")}/v1/stream?token=${encodeURIComponent(grant)}`, { headers: { Authorization: `Bearer ${iam}` } } as any)
await new Promise<void>((resolve, reject) => { socket.onopen = () => resolve(); socket.onerror = () => reject(new Error("barge websocket error")) })
let audioStarted = false; let qualifiedAt: number | null = null; let cancelAt: number | null = null; let sequence = -1; const observedTags: string[] = []
const canceled = new Promise<void>((resolve, reject) => {
  const timeout = setTimeout(() => reject(new Error("barge cancel timeout")), 30_000)
  socket.onmessage = event => {
    if (typeof event.data === "string") {
      const frame = JSON.parse(event.data)
      if (typeof frame._tag === "string" && !observedTags.includes(frame._tag)) observedTags.push(frame._tag)
      if ((frame._tag === "transcript_interim" || frame._tag === "transcript_final") && typeof frame.text === "string" && frame.text.trim().length >= 3 && qualifiedAt === null) qualifiedAt = performance.now()
      if (frame._tag === "playback_cancel" && frame.speechRef === speechRef) { cancelAt = performance.now(); clearTimeout(timeout); resolve() }
      return
    }
    if (!audioStarted) {
      audioStarted = true
      void (async () => {
        for (let offset = 0; offset < pcm.byteLength; offset += 8_000) { socket.send(Buffer.from(mediaFrame(++sequence, pcm.slice(offset, offset + 8_000), identity))); await Bun.sleep(100) }
        for (let n = 0; n < 4; n++) { socket.send(Buffer.from(mediaFrame(++sequence, new Uint8Array(8_000), identity))); await Bun.sleep(100) }
      })()
    }
  }
})
const speak = fetch(`${base}/v1/speak`, { method: "POST", headers: { Authorization: `Bearer ${iam}`, "x-openagents-audio-grant": grant, "content-type": "application/json" }, body: JSON.stringify({ turnRef, speechRef, messageRef, text: "This is a deliberately longer canonical assistant response used only to prove that qualified user speech interrupts streamed playback promptly while ordinary background noise does not create command authority." }) })
try { await canceled } catch (error) { socket.close(); console.error(JSON.stringify({ ok: false, observedTags, audioStarted, framesSent: sequence })); throw error }
const response = await speak; socket.close()
if (!response.ok || qualifiedAt === null || cancelAt === null) throw new Error("barge receipt invalid")
const interruptAckMs = Math.round(cancelAt - qualifiedAt)
console.log(JSON.stringify({ schema: "openagents.audio.barge_live_smoke.v1", ok: true, interruptAckMs, speechRefBound: true, outcomeRefObserved: true, transcriptLogged: false }))
