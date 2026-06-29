// COORDINATOR WIRING (#4994): `src/bun/index.ts` (a shared file this lane must
// NOT edit) wires this core into the live app as follows:
//   1. import { makeLoopbackPreview } from "./loopback-preview"
//   2. construct a real `listen` that wraps Bun.serve bound to 127.0.0.1:
//        const listen: LoopbackListener = ({ handle }) => {
//          const server = Bun.serve({
//            hostname: "127.0.0.1", port: 0,
//            fetch(req) {
//              const r = handle(new URL(req.url).pathname)
//              return new Response(r.body, { status: r.status,
//                headers: { "content-type": r.contentType } })
//            },
//          })
//          return { url: `http://127.0.0.1:${server.port}`, stop: () => server.stop() }
//        }
//      The LIVE loopback bind is owner/runtime-gated (only on operator action,
//      loopback-only — never 0.0.0.0).
//   3. construct a real `deployToCloud` that calls the cloud site-runtime (e.g.
//      reuse the pylon-control `deployToCloud` /command seam, or the Sites cloud
//      host API). The LIVE cloud deploy is owner/runtime-gated; the cloud HOSTS
//      and OWNS the public record — the desktop only previews locally.
//   4. expose an Electrobun RPC method (e.g. `sites.preview.start` / `.deploy`)
//      on DesktopRPCSchema in ../shared/rpc and route it through this preview's
//      `start()`/`deploy()`. The webview gets only the public-safe handle url +
//      CloudDeployResult; no token/socket crosses to the webview.
//
// #4994 — local loopback Sites preview core (desktop).
//
// Goal: serve a generated site bundle on a loopback HTTP server for an instant
// local preview, then hand the saved bundle off to the cloud site-runtime for
// real hosting. The desktop only previews locally; the cloud HOSTS and OWNS the
// public record. Nothing here owns the site.
//
// Design: the request-routing core (`servePreviewRequest`) is a PURE function
// of (bundle, requestPath) → response, so it is unit-testable with a fake
// bundle and no real socket. The server BIND is INJECTED via `listen`, so tests
// pass a fake listener; the LIVE `Bun.serve` loopback bind is owner/runtime-
// gated and wired in `src/bun/index.ts` (see COORDINATOR WIRING there).

// A single file in the generated site bundle. Bodies may be a string (text/
// html/css/js) or raw bytes (images, fonts, wasm), keyed by their site-relative
// path (e.g. "/index.html", "/assets/app.css").
export type SiteBundleEntry = {
  readonly contentType: string
  readonly body: string | Uint8Array
}

export type SiteBundle = {
  // Path → entry. Paths are site-relative and begin with "/".
  readonly files: Readonly<Record<string, SiteBundleEntry>>
  // When set, unknown paths resolve to this entry's path (typically
  // "/index.html") so client-side-routed SPAs render. When absent, an unknown
  // path is a 404.
  readonly indexFallback?: string
}

export type PreviewResponse = {
  readonly status: 200 | 404
  readonly contentType: string
  readonly body: string | Uint8Array
  // The bundle path that was actually served (after fallback resolution), or
  // null on a 404. Useful for tests/telemetry without re-deriving routing.
  readonly servedPath: string | null
}

const NOT_FOUND_BODY = "Not found"

// Normalize a request path to a bundle key:
//   - strip query/hash,
//   - ensure a leading "/",
//   - treat "/" and "" as the index fallback path when one exists.
function normalizeRequestPath(requestPath: string): string {
  let p = requestPath
  const q = p.search(/[?#]/)
  if (q >= 0) p = p.slice(0, q)
  if (p.length === 0) p = "/"
  if (!p.startsWith("/")) p = `/${p}`
  return p
}

// Pure request router. No I/O. Deterministic for a given (bundle, requestPath).
//
//   - known path        → 200 with the entry's content-type and body
//   - unknown path       → indexFallback entry (200) when configured
//   - unknown, no fallback (or fallback target missing) → 404
export function servePreviewRequest(bundle: SiteBundle, requestPath: string): PreviewResponse {
  const path = normalizeRequestPath(requestPath)

  const direct = bundle.files[path]
  if (direct) {
    return { status: 200, contentType: direct.contentType, body: direct.body, servedPath: path }
  }

  // A request for "/" with no explicit "/" entry resolves through the fallback.
  if (bundle.indexFallback) {
    const fallback = bundle.files[bundle.indexFallback]
    if (fallback) {
      return {
        status: 200,
        contentType: fallback.contentType,
        body: fallback.body,
        servedPath: bundle.indexFallback,
      }
    }
  }

  return { status: 404, contentType: "text/plain; charset=utf-8", body: NOT_FOUND_BODY, servedPath: null }
}

// Injected loopback listener. The real implementation (owner/runtime-gated)
// wraps `Bun.serve` bound to 127.0.0.1 and forwards each request through
// `handle`. Tests pass a fake that records the handler and lets them drive
// requests synchronously without opening a socket.
export type LoopbackListener = (input: {
  readonly handle: (requestPath: string) => PreviewResponse
}) => LoopbackPreviewHandle

export type LoopbackPreviewHandle = {
  // Loopback origin the preview is reachable at, e.g. "http://127.0.0.1:PORT".
  readonly url: string
  // Tear the preview server down (stop listening).
  readonly stop: () => void
}

// Cloud handoff seam. Represents handing the SAVED bundle to the cloud site-
// runtime for hosting. Injected so the core stays pure and testable; the LIVE
// call (a real deploy to the cloud site-runtime that owns the hosted record) is
// owner/runtime-gated in `src/bun/index.ts`.
export type DeployToCloud = (input: {
  readonly bundle: SiteBundle
}) => Promise<CloudDeployResult>

export type CloudDeployResult = {
  readonly accepted: boolean
  // The cloud-hosted URL once accepted; null until the cloud assigns one.
  readonly url: string | null
  readonly reason: string
}

export type LoopbackPreview = {
  // The previewed bundle (what the loopback server serves).
  readonly bundle: SiteBundle
  // Start the loopback preview via the injected listener; returns its handle.
  readonly start: () => LoopbackPreviewHandle
  // Route a request without a live server — the pure core, exposed for the
  // webview/RPC and for deterministic tests.
  readonly serve: (requestPath: string) => PreviewResponse
  // Hand the saved bundle to the cloud site-runtime for hosting.
  readonly deploy: () => Promise<CloudDeployResult>
}

// Wire the pure router behind an INJECTED listener and an INJECTED cloud-deploy
// seam. No socket is opened and no cloud call is made until `start()`/`deploy()`
// are invoked, and even then only through the injected dependencies.
export function makeLoopbackPreview(input: {
  readonly bundle: SiteBundle
  readonly listen: LoopbackListener
  readonly deployToCloud: DeployToCloud
}): LoopbackPreview {
  const { bundle, listen, deployToCloud } = input
  const serve = (requestPath: string) => servePreviewRequest(bundle, requestPath)
  return {
    bundle,
    serve,
    start: () => listen({ handle: serve }),
    deploy: () => deployToCloud({ bundle }),
  }
}
