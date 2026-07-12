const STREAM = "/v1/stream"
const HTTP_PATHS = new Set(["/v1/speak", "/v1/retention/reconcile", "/v1/retention/export", "/v1/retention/delete"])
const origin = (process.env.OPENAGENTS_AUDIO_CLOUD_RUN_URL ?? "").replace(/\/$/u, "")
if (!origin.startsWith("https://")) throw new Error("missing OPENAGENTS_AUDIO_CLOUD_RUN_URL")

let cached: { token: string; refreshAt: number } | undefined
const identityToken = async (): Promise<string> => {
  if (cached && cached.refreshAt > Date.now()) return cached.token
  const url = new URL("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity")
  url.searchParams.set("audience", origin); url.searchParams.set("format", "full")
  const response = await fetch(url, { headers: { "metadata-flavor": "Google" } })
  if (!response.ok) throw new Error("edge_identity_unavailable")
  const token = await response.text()
  if (token.length < 100 || token.length > 8_192) throw new Error("edge_identity_invalid")
  cached = { token, refreshAt: Date.now() + 45 * 60_000 }
  return token
}

type SocketData = {
  grant: string
  upstream?: WebSocket
  pending: Array<string | Uint8Array>
  pendingBytes: number
}

const server = Bun.serve<SocketData>({
  port: Number(process.env.PORT ?? 8080), idleTimeout: 240,
  async fetch(request, server) {
    const url = new URL(request.url)
    if (url.pathname === "/health") return Response.json({ ok: true, service: "openagents-audio-edge" })
    const grant = request.headers.get("x-openagents-audio-grant") ?? ""
    if (grant.length < 16 || grant.length > 4_096 || url.search !== "") return Response.json({ error: "invalid_audio_edge_request" }, { status: 400 })
    if (url.pathname === STREAM) {
      if (request.method !== "GET" || request.headers.get("upgrade")?.toLowerCase() !== "websocket") return Response.json({ error: "upgrade_required" }, { status: 426 })
      return server.upgrade(request, { data: { grant, pending: [], pendingBytes: 0 } }) ? undefined as unknown as Response : Response.json({ error: "upgrade_failed" }, { status: 500 })
    }
    if (!HTTP_PATHS.has(url.pathname) || request.method !== "POST") return Response.json({ error: "not_found" }, { status: 404 })
    try {
      const response = await fetch(`${origin}${url.pathname}`, { method: "POST", headers: { authorization: `Bearer ${await identityToken()}`, "content-type": "application/json", "x-openagents-audio-grant": grant }, body: await request.text() })
      return new Response(response.body, { status: response.status, headers: { "content-type": response.headers.get("content-type") ?? "application/json", "cache-control": "no-store" } })
    } catch { return Response.json({ error: "audio_gateway_unavailable" }, { status: 502 }) }
  },
  websocket: {
    async open(client) {
      try {
        const upstream = new WebSocket(origin.replace(/^http/u, "ws") + STREAM, { headers: { authorization: `Bearer ${await identityToken()}`, "x-openagents-audio-grant": client.data.grant } } as never)
        upstream.binaryType = "arraybuffer"
        client.data.upstream = upstream
        upstream.onopen = () => { for (const item of client.data.pending) upstream.send(item); client.data.pending = []; client.data.pendingBytes = 0 }
        upstream.onmessage = event => { if (client.readyState === WebSocket.OPEN) client.send(typeof event.data === "string" ? event.data : new Uint8Array(event.data as ArrayBuffer)) }
        upstream.onerror = () => client.close(1011, "audio_gateway_unavailable")
        upstream.onclose = event => client.close(event.code === 1000 ? 1000 : 1011, "audio_gateway_closed")
      } catch { client.close(1011, "audio_gateway_unavailable") }
    },
    message(client, message) {
      const item = typeof message === "string" ? message : new Uint8Array(message).slice()
      if (client.data.upstream?.readyState === WebSocket.OPEN) { client.data.upstream.send(item); return }
      const size = typeof item === "string" ? item.length : item.byteLength
      client.data.pendingBytes += size
      if (client.data.pendingBytes > 256 * 1024) { client.close(1009, "edge_backpressure"); return }
      client.data.pending.push(item)
    },
    close(client) { client.data.upstream?.close(1000, "client_closed") },
  },
})

console.log(JSON.stringify({ event: "audio_edge_started", port: server.port }))
