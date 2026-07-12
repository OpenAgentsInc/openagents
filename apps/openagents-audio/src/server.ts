import type { Server, ServerWebSocket } from "bun"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"
import { verifyAudioGrant } from "./auth"
import { AudioSession } from "./session"
import type { SttAdapter } from "./stt"
import type { TtsAdapter, TtsReceipt } from "./tts"

type SocketData = { identity: VoiceIdentity; session?: AudioSession }
export type AudioServerConfig = Readonly<{ tokenSecret: string; adapter: SttAdapter; tts?: TtsAdapter; ttsVoiceRef?: string; port?: number; log?: (event: Readonly<Record<string, string | number | boolean>>) => void; onBargeIn?: (input: Readonly<{ identity: VoiceIdentity; turnRef: string; speechRef: string }>) => Promise<string> }>
export const startAudioServer = (config: AudioServerConfig) => {
  if (config.tokenSecret.length < 32) throw new Error("audio_token_secret_too_short")
  const log = config.log ?? (() => {})
  const sessions = new Map<string, AudioSession>()
  const sessionKey = (identity: VoiceIdentity) => `${identity.ownerRef}\u0000${identity.deviceRef}\u0000${identity.threadRef}\u0000${identity.sessionRef}\u0000${identity.generation}`
  const server = Bun.serve<SocketData>({
    port: config.port ?? 8080, idleTimeout: 240,
    async fetch(request, srv) {
      const url = new URL(request.url)
      if (url.pathname === "/health") return Response.json({ ok: true, service: "openagents-audio" })
      if (url.pathname === "/v1/speak" && request.method === "POST") {
        const bearer = request.headers.get("x-openagents-audio-grant") ?? ""
        const grant = verifyAudioGrant(bearer, config.tokenSecret)
        if (!grant) return Response.json({ error: "unauthorized" }, { status: 401 })
        let value: unknown
        try { value = await request.json() } catch { return Response.json({ error: "invalid_request" }, { status: 400 }) }
        if (typeof value !== "object" || value === null) return Response.json({ error: "invalid_request" }, { status: 400 })
        const body = value as Record<string, unknown>
        if (![body.turnRef, body.speechRef, body.messageRef].every(ref => typeof ref === "string" && ref.length > 0 && ref.length <= 256) || typeof body.text !== "string" || body.text.length > 16_384) return Response.json({ error: "invalid_request" }, { status: 400 })
        const session = sessions.get(sessionKey(grant.identity))
        if (session === undefined) return Response.json({ error: "session_not_live" }, { status: 409 })
        const receipt = await session.speak({ turnRef: String(body.turnRef), speechRef: String(body.speechRef), messageRef: String(body.messageRef), text: body.text })
        return Response.json(receipt)
      }
      if (url.pathname !== "/v1/stream") return Response.json({ error: "not_found" }, { status: 404 })
      if (request.headers.get("upgrade")?.toLowerCase() !== "websocket") return Response.json({ error: "upgrade_required" }, { status: 426 })
      // Cloud Run IAM consumes the Authorization identity token but still
      // forwards it. The application voice grant therefore uses its own
      // header (or the WebSocket-compatible query fallback), never IAM's JWT.
      const bearer = request.headers.get("x-openagents-audio-grant") ?? url.searchParams.get("token") ?? ""
      const grant = verifyAudioGrant(bearer, config.tokenSecret)
      if (!grant) return Response.json({ error: "unauthorized" }, { status: 401 })
      if (!srv.upgrade(request, { data: { identity: grant.identity } })) return Response.json({ error: "upgrade_failed" }, { status: 500 })
      return undefined as unknown as Response
    },
    websocket: {
      open(ws) {
        ws.data.session = new AudioSession(ws.data.identity, config.adapter, { sendText: (value) => ws.send(JSON.stringify(value)) > 0, sendBinary: value => ws.send(value) > 0, close: (code, reason) => ws.close(code, reason) }, config.tts === undefined ? undefined : { adapter: config.tts, ...(config.ttsVoiceRef === undefined ? {} : { voiceRef: config.ttsVoiceRef }), ...(config.onBargeIn === undefined ? {} : { onBargeIn: config.onBargeIn }), receipt: receipt => log({ event: "audio_tts_receipt", charsIn: receipt.charsIn, chunksOut: receipt.chunksOut, bytesOut: receipt.bytesOut, totalMs: receipt.totalMs, synthTtfbMs: receipt.synthTtfbMs ?? -1, outcome: receipt.outcome }) })
        sessions.set(sessionKey(ws.data.identity), ws.data.session)
        log({ event: "audio_session_open", generation: ws.data.identity.generation })
      },
      message(ws: ServerWebSocket<SocketData>, message) {
        if (typeof message === "string") { ws.close(4003, "binary_required"); return }
        try { ws.data.session?.receive(new Uint8Array(message)) } catch { /* session closes with public-safe reason */ }
      },
      close(ws) { ws.data.session?.stop(); sessions.delete(sessionKey(ws.data.identity)); log({ event: "audio_session_close", generation: ws.data.identity.generation }) },
    },
  })
  return {
    server, port: server.port ?? config.port ?? 8080,
    speak: async (input: Readonly<{ identity: VoiceIdentity; turnRef: string; speechRef: string; messageRef: string; text: string }>): Promise<TtsReceipt | null> => sessions.get(sessionKey(input.identity))?.speak(input) ?? null,
    stop: async () => { for (const session of sessions.values()) session.stop(); sessions.clear(); await server.stop(true) },
  }
}
