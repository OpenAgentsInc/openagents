import { Runtime } from "@openagentsinc/runtime-platform"
import { mintAudioGrant } from "../src/auth"
import { decodeMediaHeader } from "@openagentsinc/audio-contract"

const required = (name: string) => { const value = process.env[name]; if (!value) throw new Error(`missing ${name}`); return value }
const identity = { ownerRef: "smoke:tts-owner", deviceRef: "smoke:tts-device", threadRef: "smoke:tts-thread", sessionRef: `smoke:tts:${Date.now()}`, generation: 1 }
const grant = mintAudioGrant({ identity, expiresAtMs: Date.now() + 5 * 60_000 }, required("OPENAGENTS_AUDIO_TOKEN_SECRET"))
const base = required("OPENAGENTS_AUDIO_URL"); const iam = required("OPENAGENTS_AUDIO_CLOUD_RUN_ID_TOKEN")
const turnRef = `turn:tts:${Date.now()}`; const speechRef = `speech:tts:${Date.now()}`; const messageRef = `message:tts:${Date.now()}`
const socket = new WebSocket(`${base.replace(/^http/, "ws")}/v1/stream?token=${encodeURIComponent(grant)}`, { headers: { Authorization: `Bearer ${iam}` } } as any)
const headers: Array<ReturnType<typeof decodeMediaHeader>> = []; let assistantText = false
await new Promise<void>((resolve, reject) => { socket.onopen = () => resolve(); socket.onerror = () => reject(new Error("tts websocket error")) })
socket.onmessage = event => {
  if (typeof event.data === "string") { const frame = JSON.parse(event.data); if (frame._tag === "assistant_text" && frame.turnRef === turnRef && frame.speechRef === speechRef) assistantText = true; return }
  void (async () => {
    const bytes = new Uint8Array(await new Response(event.data).arrayBuffer()); const size = new DataView(bytes.buffer, bytes.byteOffset + 4, 4).getUint32(0)
    headers.push(decodeMediaHeader(JSON.parse(Buffer.from(bytes.subarray(8, 8 + size)).toString("utf8"))))
  })()
}
const response = await fetch(`${base}/v1/speak`, { method: "POST", headers: { Authorization: `Bearer ${iam}`, "x-openagents-audio-grant": grant, "content-type": "application/json" }, body: JSON.stringify({ turnRef, speechRef, messageRef, text: "OpenAgents streaming speech receipt." }) })
if (!response.ok) throw new Error(`tts speak failed ${response.status}`)
const receipt = await response.json() as Record<string, unknown>; await Runtime.sleep(250); socket.close()
if (!assistantText || headers.length === 0 || headers.some(header => header.kind !== "server_tts" || header.identity.generation !== 1 || header.turnRef !== turnRef || header.speechRef !== speechRef)) throw new Error("tts binding receipt invalid")
console.log(JSON.stringify({ schema: "openagents.audio.tts_live_smoke.v1", ok: true, assistantText, mediaFrames: headers.length, adapterRef: receipt.adapterRef, voiceRef: receipt.voiceRef, charsIn: receipt.charsIn, synthTtfbMs: receipt.synthTtfbMs, totalMs: receipt.totalMs, bytesOut: receipt.bytesOut, chunksOut: receipt.chunksOut, outcome: receipt.outcome, transcriptLogged: false }))
