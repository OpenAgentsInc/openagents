import { Runtime, type RuntimeServerWebSocket } from "@openagentsinc/runtime-platform"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"
import { verifyAudioGrant } from "./auth"
import { AudioSession, type AcceptedAudioFrame } from "./session"
import type { SttAdapter } from "./stt"
import type { TtsAdapter, TtsReceipt } from "./tts"

export type AudioRetentionSession = Readonly<{
  receipt: Readonly<{ receiptRef: string; expiresAtMs: number }>
  accept: (frame: AcceptedAudioFrame) => Promise<void>
  gap: (firstSequence: number, lastSequence: number) => Promise<void>
  stop: () => Promise<void>
}>
export type AudioRetentionRuntime = Readonly<{
  admit: (identity: VoiceIdentity) => Promise<AudioRetentionSession>
  reconcile: (identity: VoiceIdentity) => Promise<unknown>
  exportSession: (identity: VoiceIdentity) => Promise<unknown>
  deleteSession: (identity: VoiceIdentity) => Promise<unknown>
  close: () => Promise<void>
}>
type SocketData = { identity: VoiceIdentity; retention?: AudioRetentionSession; session?: AudioSession; receiveTail?: Promise<void> }
export type AudioServerConfig = Readonly<{ tokenSecret: string; adapter: SttAdapter; retention?: AudioRetentionRuntime; tts?: TtsAdapter; ttsVoiceRef?: string; port?: number; log?: (event: Readonly<Record<string, string | number | boolean>>) => void; onBargeIn?: (input: Readonly<{ identity: VoiceIdentity; turnRef: string; speechRef: string }>) => Promise<string> }>
export const startAudioServer = (config: AudioServerConfig) => {
  if (config.tokenSecret.length < 32) throw new Error("audio_token_secret_too_short")
  const log = config.log ?? (() => {})
  const sessions = new Map<string, AudioSession>()
  const sessionKey = (identity: VoiceIdentity) => `${identity.ownerRef}\u0000${identity.deviceRef}\u0000${identity.threadRef}\u0000${identity.sessionRef}\u0000${identity.generation}`
  const server = Runtime.serve<SocketData>({
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
      if (["/v1/retention/reconcile", "/v1/retention/export", "/v1/retention/delete"].includes(url.pathname) && request.method === "POST") {
        const grant = verifyAudioGrant(request.headers.get("x-openagents-audio-grant") ?? "", config.tokenSecret)
        if (!grant) return Response.json({ error: "unauthorized" }, { status: 401 })
        if (!config.retention) return Response.json({ error: "retention_unavailable" }, { status: 503 })
        try {
          const result = url.pathname.endsWith("/reconcile") ? await config.retention.reconcile(grant.identity)
            : url.pathname.endsWith("/export") ? await config.retention.exportSession(grant.identity)
            : await config.retention.deleteSession(grant.identity)
          return Response.json(result)
        } catch { return Response.json({ error: "retention_operation_failed" }, { status: 503 }) }
      }
      if (url.pathname !== "/v1/stream") return Response.json({ error: "not_found" }, { status: 404 })
      if (request.headers.get("upgrade")?.toLowerCase() !== "websocket") return Response.json({ error: "upgrade_required" }, { status: 426 })
      // Cloud Run IAM consumes the Authorization identity token but still
      // forwards it. The application voice grant therefore uses its own
      // header (or the WebSocket-compatible query fallback), never IAM's JWT.
      const bearer = request.headers.get("x-openagents-audio-grant") ?? url.searchParams.get("token") ?? ""
      const grant = verifyAudioGrant(bearer, config.tokenSecret)
      if (!grant) return Response.json({ error: "unauthorized" }, { status: 401 })
      let retention: AudioRetentionSession | undefined
      try { retention = await config.retention?.admit(grant.identity) }
      catch { return Response.json({ error: "retention_admission_failed" }, { status: 503 }) }
      if (!srv.upgrade(request, { data: { identity: grant.identity, ...(retention === undefined ? {} : { retention }) } })) return Response.json({ error: "upgrade_failed" }, { status: 500 })
      return undefined as unknown as Response
    },
    websocket: {
      open(ws) {
        ws.data.session = new AudioSession(ws.data.identity, config.adapter, { sendText: (value) => ws.send(JSON.stringify(value)) > 0, sendBinary: value => ws.send(value) > 0, close: (code, reason) => ws.close(code, reason) }, config.tts === undefined ? undefined : { adapter: config.tts, ...(config.ttsVoiceRef === undefined ? {} : { voiceRef: config.ttsVoiceRef }), ...(config.onBargeIn === undefined ? {} : { onBargeIn: config.onBargeIn }), receipt: receipt => log({ event: "audio_tts_receipt", charsIn: receipt.charsIn, chunksOut: receipt.chunksOut, bytesOut: receipt.bytesOut, totalMs: receipt.totalMs, synthTtfbMs: receipt.synthTtfbMs ?? -1, outcome: receipt.outcome }) }, ws.data.retention)
        sessions.set(sessionKey(ws.data.identity), ws.data.session)
        if (ws.data.retention) ws.data.session.announceRetention({ schema: "openagents.audio.v1", _tag: "retention_receipt", identity: ws.data.identity, receiptRef: ws.data.retention.receipt.receiptRef, disclosureRef: "audio-retention.mvp.v1", policyRef: "audio-retention.mvp.v1", expiresAtMs: ws.data.retention.receipt.expiresAtMs, maxRetentionSeconds: 2_592_000 })
        log({ event: "audio_session_open", generation: ws.data.identity.generation })
      },
      message(ws: RuntimeServerWebSocket<SocketData>, message) {
        if (typeof message === "string") { ws.close(4003, "binary_required"); return }
        const bytes = new Uint8Array(message).slice()
        ws.data.receiveTail = (ws.data.receiveTail ?? Promise.resolve()).then(() => ws.data.session?.receive(bytes)).catch(() => { ws.close(4011, "retention_or_media_failed") })
      },
      close(ws) { ws.data.session?.stop(); void ws.data.retention?.stop(); sessions.delete(sessionKey(ws.data.identity)); log({ event: "audio_session_close", generation: ws.data.identity.generation }) },
    },
  })
  return {
    server,
    get port() { return server.port ?? config.port ?? 8080 },
    ready: server.ready,
    speak: async (input: Readonly<{ identity: VoiceIdentity; turnRef: string; speechRef: string; messageRef: string; text: string }>): Promise<TtsReceipt | null> => sessions.get(sessionKey(input.identity))?.speak(input) ?? null,
    stop: async () => { for (const session of sessions.values()) session.stop(); sessions.clear(); await server.stop(true); await config.retention?.close() },
  }
}
