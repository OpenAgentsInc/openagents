import type { Server, ServerWebSocket } from "bun"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"
import { verifyAudioGrant } from "./auth"
import { AudioSession } from "./session"
import type { SttAdapter } from "./stt"

type SocketData = { identity: VoiceIdentity; session?: AudioSession }
export type AudioServerConfig = Readonly<{ tokenSecret: string; adapter: SttAdapter; port?: number; log?: (event: Readonly<Record<string, string | number | boolean>>) => void }>
export const startAudioServer = (config: AudioServerConfig) => {
  if (config.tokenSecret.length < 32) throw new Error("audio_token_secret_too_short")
  const log = config.log ?? (() => {})
  const server = Bun.serve<SocketData>({
    port: config.port ?? 8080, idleTimeout: 240,
    fetch(request, srv) {
      const url = new URL(request.url)
      if (url.pathname === "/health") return Response.json({ ok: true, service: "openagents-audio" })
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
        ws.data.session = new AudioSession(ws.data.identity, config.adapter, { sendText: (value) => ws.send(JSON.stringify(value)) > 0, close: (code, reason) => ws.close(code, reason) })
        log({ event: "audio_session_open", generation: ws.data.identity.generation })
      },
      message(ws: ServerWebSocket<SocketData>, message) {
        if (typeof message === "string") { ws.close(4003, "binary_required"); return }
        try { ws.data.session?.receive(new Uint8Array(message)) } catch { /* session closes with public-safe reason */ }
      },
      close(ws) { ws.data.session?.stop(); log({ event: "audio_session_close", generation: ws.data.identity.generation }) },
    },
  })
  return { server, port: server.port ?? config.port ?? 8080, stop: async () => { await server.stop(true) } }
}
