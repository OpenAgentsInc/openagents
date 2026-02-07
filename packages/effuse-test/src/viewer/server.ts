import type { ServerWebSocket } from "bun"

import { Effect, Scope } from "effect"

import type { TestEvent } from "../spec.ts"
import { VIEWER_HTML } from "./assets.ts"

export type ViewerServer = {
  readonly url: string
  readonly broadcast: (event: TestEvent) => void
}

type ViewerServerInternal = ViewerServer & { readonly _server: ReturnType<typeof Bun.serve> }

export const startViewerServer = (port: number): Effect.Effect<ViewerServer, never, Scope.Scope> =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const clients = new Set<ServerWebSocket<{ readonly kind: "events" }>>()
      const server = Bun.serve<{ readonly kind: "events" }>({
        port,
        hostname: "127.0.0.1",
        fetch(req, bunServer) {
          const url = new URL(req.url)
          if (url.pathname === "/ws") {
            const ok = bunServer.upgrade(req, { data: { kind: "events" } })
            return ok ? new Response(null, { status: 101 }) : new Response("upgrade failed", { status: 400 })
          }
          if (url.pathname === "/") {
            return new Response(VIEWER_HTML, {
              headers: { "content-type": "text/html; charset=utf-8" },
            })
          }
          return new Response("not found", { status: 404 })
        },
        websocket: {
          open(ws) {
            clients.add(ws)
          },
          close(ws) {
            clients.delete(ws)
          },
          message() {
            // viewer is write-only (runner -> viewer)
          },
        },
      })

      const broadcast = (event: TestEvent) => {
        const payload = JSON.stringify(event)
        for (const ws of clients) ws.send(payload)
      }

      const internal: ViewerServerInternal = { url: `http://127.0.0.1:${server.port}`, broadcast, _server: server }
      return internal
    }),
    ({ _server }) =>
      Effect.sync(() => {
        try {
          _server.stop()
        } catch {
          // ignore
        }
      }),
  ).pipe(Effect.map(({ url, broadcast }) => ({ url, broadcast })))
