/**
 * Test Server Worker
 *
 * Runs in a Worker to provide HTTP + WebSocket server for integration tests.
 * The main process spawns this worker, which then serves:
 * - / - Test HTML page
 * - /ws - WebSocket endpoint for widget communication
 * - /inject - POST endpoint to inject HUD messages
 *
 * Environment Variables:
 * - TEST_HTML: The HTML content to serve
 * - TEST_PORT: Port to listen on (0 for auto-assign)
 */

declare const self: Worker

const TEST_HTML = Bun.env.TEST_HTML || "<html><body>No test HTML provided</body></html>"
const TEST_PORT = parseInt(Bun.env.TEST_PORT || "0", 10)

interface ServerWebSocket {
  send(data: string): void
  subscribe(topic: string): void
}

const server = Bun.serve({
  port: TEST_PORT,

  async fetch(req, server) {
    const url = new URL(req.url)

    // WebSocket upgrade
    if (url.pathname === "/ws") {
      const success = server.upgrade(req)
      if (success) return undefined
      return new Response("WebSocket upgrade failed", { status: 400 })
    }

    // Message injection API
    if (url.pathname === "/inject" && req.method === "POST") {
      try {
        const msg = await req.json()
        // Broadcast to all connected clients
        server.publish("test-channel", JSON.stringify(msg))
        return Response.json({ ok: true })
      } catch (e) {
        return Response.json({ ok: false, error: String(e) }, { status: 400 })
      }
    }

    // Health check
    if (url.pathname === "/health") {
      return Response.json({ ok: true, port: server.port })
    }

    // Serve test HTML
    if (url.pathname === "/" || url.pathname === "/index.html") {
      return new Response(TEST_HTML, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      })
    }

    return new Response("Not found", { status: 404 })
  },

  websocket: {
    open(ws: ServerWebSocket) {
      // Subscribe to test channel for broadcasts
      ws.subscribe("test-channel")
    },
    message(ws: ServerWebSocket, message) {
      // Could record messages from widget if needed for assertions
      // For now, just log for debugging
      if (Bun.env.DEBUG) {
        console.log("[TestServer] Received:", message)
      }
    },
    close(ws: ServerWebSocket) {
      // Cleanup if needed
    },
  },
})

// Report the assigned port to parent process
self.postMessage({ type: "ready", port: server.port })

// Keep worker alive
setInterval(() => {}, 1000)
