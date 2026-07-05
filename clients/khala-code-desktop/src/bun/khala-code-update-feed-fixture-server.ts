import {
  khalaCodeDesktopUpdaterPlatformPrefix,
  type KhalaCodeDesktopUpdaterPlatform,
} from "../shared/updater.js"

/**
 * Fixture update-server for #8440 in-app updater plumbing tests. Serves the
 * exact wire shape `fetchKhalaCodeDesktopUpdateFeedInfo` (and, in real
 * builds, Electrobun's own `Updater`) fetch from a release `baseUrl`:
 * `{baseUrl}/{channel}-{os}-{arch}-update.json`.
 */

export type KhalaCodeUpdateFeedFixtureRoute =
  | { readonly kind: "json"; readonly body: unknown }
  | { readonly kind: "malformed" }
  | { readonly kind: "status"; readonly status: number }

export type KhalaCodeUpdateFeedFixtureServer = {
  readonly baseUrl: string
  readonly requestCount: () => number
  readonly stop: () => void
}

export const khalaCodeUpdateFeedFixtureRouteKey = (
  channel: string,
  platform: KhalaCodeDesktopUpdaterPlatform,
): string => `${khalaCodeDesktopUpdaterPlatformPrefix(channel, platform)}-update.json`

export function startKhalaCodeUpdateFeedFixtureServer(input: {
  readonly routes: ReadonlyMap<string, KhalaCodeUpdateFeedFixtureRoute>
}): KhalaCodeUpdateFeedFixtureServer {
  let requestCount = 0
  const server = Bun.serve({
    port: 0,
    fetch(request) {
      requestCount += 1
      const url = new URL(request.url)
      const key = url.pathname.replace(/^\/+/, "")
      const route = input.routes.get(key)
      if (route === undefined) {
        return new Response(`fixture: no route for ${key}`, { status: 404 })
      }
      if (route.kind === "status") {
        return new Response("fixture error", { status: route.status })
      }
      if (route.kind === "malformed") {
        return new Response("{not valid json", {
          headers: { "content-type": "application/json" },
          status: 200,
        })
      }
      return new Response(JSON.stringify(route.body), {
        headers: { "content-type": "application/json" },
        status: 200,
      })
    },
  })
  return {
    baseUrl: `http://127.0.0.1:${server.port}`,
    requestCount: () => requestCount,
    stop: () => server.stop(true),
  }
}
