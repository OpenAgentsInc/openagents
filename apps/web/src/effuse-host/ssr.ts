import { Effect } from "effect"
import {
  cachePolicyToCacheControlDirectives,
  escapeHtml,
  escapeJsonForHtmlScript,
  renderToString,
  runRoute,
} from "@openagentsinc/effuse"

import { appRoutes } from "../effuse-app/routes"
import type { AppServices } from "../effect/layer"
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext"
import { TelemetryService } from "../effect/telemetry"
import { readE2eTokenFromRequest } from "../auth/e2eAuth"

import { getWorkerAppConfig, getWorkerRuntime } from "./runtime"
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId"
import type { Route, RouteContext, RouteMatch, RouteRun } from "@openagentsinc/effuse"
import type { WorkerEnv } from "./env"

type AnyRoute = Route<any, AppServices>
type AnyRun = RouteRun<unknown>

class RequestAbortedError extends Error {
  readonly _tag = "RequestAbortedError"
  constructor() {
    super("Request aborted")
  }
}

const MAX_SSR_HTML_BYTES = 1_500_000

const byteLengthUtf8 = (text: string): number => new TextEncoder().encode(text).byteLength

const matchRoute = (
  routes: ReadonlyArray<AnyRoute>,
  url: URL,
): { readonly route: AnyRoute; readonly match: RouteMatch } | null => {
  for (const route of routes) {
    try {
      const match = route.match(url)
      if (match) return { route, match }
    } catch {
      // Treat match defects as no match.
    }
  }
  return null
}

const defaultNotFoundHtml = (url: URL): string =>
  `<div data-effuse-error="not-found"><h1>Not found</h1><p>${escapeHtml(
    url.pathname,
  )}</p></div>`

const defaultFailHtml = (url: URL, error: unknown): string =>
  `<div data-effuse-error="fail"><h1>Error</h1><p>${escapeHtml(
    url.pathname,
  )}</p><pre>${escapeHtml(String(error))}</pre></div>`

const htmlHeadersNoStore = (): Headers =>
  new Headers({
    "content-type": "text/html; charset=utf-8",
    "cache-control": "no-store",
  })

const applyCookieMutations = (
  headers: Headers,
  cookies: ReadonlyArray<any> | undefined,
): void => {
  if (!cookies || cookies.length === 0) return

  for (const mutation of cookies) {
    if (!mutation || typeof mutation !== "object") continue
    switch (mutation._tag) {
      case "Set": {
        const attrs = typeof mutation.attributes === "string" ? `; ${mutation.attributes}` : ""
        headers.append("Set-Cookie", `${mutation.name}=${mutation.value}${attrs}`)
        break
      }
      case "Delete": {
        const attrs = typeof mutation.attributes === "string" ? `; ${mutation.attributes}` : ""
        headers.append(
          "Set-Cookie",
          `${mutation.name}=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT${attrs}`,
        )
        break
      }
      default:
        break
    }
  }
}

const applyCachePolicy = (
  headers: Headers,
  input: {
    readonly cache?: any
    readonly cookies?: ReadonlyArray<any>
  },
): void => {
  // If the route explicitly set Cache-Control, do not override.
  if (headers.has("cache-control")) return

  // Conservative default: never cache HTML when cookies are mutated.
  if (input.cookies && input.cookies.length > 0) {
    headers.set("Cache-Control", "no-store")
    return
  }

  if (!input.cache) {
    headers.set("Cache-Control", "no-store")
    return
  }

  const directives = cachePolicyToCacheControlDirectives(input.cache)
  if (!directives) {
    headers.set("Cache-Control", "no-store")
    return
  }

  if (directives === "no-store") {
    headers.set("Cache-Control", "no-store")
    return
  }

  headers.set("Cache-Control", `private, ${directives}`)
}

const renderDocument = (input: {
  readonly title: string
  readonly meta: ReadonlyArray<readonly [string, string]>
  readonly bodyHtml: string
  readonly dehydrateJson: string | null
  readonly prelaunch: boolean
}): string => {
  const metaTags = input.meta
    .map(
      ([name, content]) =>
        `<meta name="${escapeHtml(name)}" content="${escapeHtml(content)}" data-effuse-meta="1" />`,
    )
    .join("")

  // Client runtime needs prelaunch mode to match SSR, otherwise CSR navigations can
  // accidentally "turn off" prelaunch-only UI like the countdown.
  const prelaunchMeta = `<meta name="oa-prelaunch" content="${input.prelaunch ? "1" : "0"}" />`

  const dehydrateScript =
    input.dehydrateJson != null
      ? `<script id="effuse-dehydrate" type="application/json">${escapeJsonForHtmlScript(
        input.dehydrateJson,
      )}</script>`
      : ""

  // NOTE: In Phase 5 we serve a dedicated Effuse client bundle (no React/TanStack).
  // These filenames are intentionally stable so the Worker host doesn't need a Vite manifest yet.
  const cssHref = "/effuse-client.css"
  const jsSrc = "/effuse-client.js"

  return [
    "<!doctype html>",
    "<html lang=\"en\">",
    "<head>",
    "<meta charset=\"utf-8\" />",
    "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />",
    `<title>${escapeHtml(input.title)}</title>`,
    prelaunchMeta,
    metaTags,
    `<link rel="stylesheet" href="${cssHref}" />`,
    dehydrateScript,
    `<script type="module" src="${jsSrc}"></script>`,
    "</head>",
    "<body>",
    `<div data-effuse-shell><div data-effuse-outlet>${input.bodyHtml}</div></div>`,
    "</body>",
    "</html>",
  ].join("")
}

const PRELAUNCH_BYPASS_COOKIE = "prelaunch_bypass=1"
const PRELAUNCH_BYPASS_COOKIE_MAX_AGE = 604800 // 7 days

/** When prelaunch is on, only these pathnames are allowed without ?key= or cookie. */
const PRELAUNCH_ALLOWED_PATHNAMES = new Set(["/", "/deck"])
/** Even with bypass (cookie or ?key=), these routes stay blocked in prelaunch mode. */
const PRELAUNCH_ALWAYS_BLOCKED_PATHNAMES = new Set<string>([])

function hasPrelaunchBypass(
  request: Request,
  url: URL,
  bypassKey: string | null,
): boolean {
  if (!bypassKey) return false
  const cookie = request.headers.get("Cookie") ?? ""
  if (cookie.includes(PRELAUNCH_BYPASS_COOKIE)) return true
  return url.searchParams.get("key") === bypassKey
}

function normalizePathname(pathname: string): string {
  const s = pathname.endsWith("/") && pathname.length > 1 ? pathname.slice(0, -1) : pathname
  return s || "/"
}

const isLocalHost = (hostname: string): boolean =>
  hostname === "localhost" ||
  hostname === "127.0.0.1" ||
  hostname === "::1" ||
  hostname === "0.0.0.0"

/** True when request is from local dev (localhost / 127.0.0.1 / 0.0.0.0). Uses Host header as fallback. */
function isLocalDevRequest(request: Request, url: URL): boolean {
  if (isLocalHost(url.hostname)) return true
  const host = request.headers.get("Host") ?? ""
  const hostname = host.split(":")[0] ?? host
  return isLocalHost(hostname)
}

/**
 * Returns a 302 redirect to / when prelaunch is on and the request is not allowed.
 * Redirect is skipped on localhost for local dev ergonomics.
 */
export function getPrelaunchRedirectIfRequired(
  request: Request,
  url: URL,
  env: WorkerEnv,
): Response | null {
  const config = getWorkerAppConfig(env)
  if (!config.prelaunch) return null
  if (isLocalDevRequest(request, url)) return null
  const pathname = normalizePathname(url.pathname)
  // If the prelaunch bypass key is present on the homepage URL, mint bypass cookie and stay on home.
  // Important: this response must mint the bypass cookie because SSR is skipped on redirects.
  if (
    pathname === "/" &&
    !!config.prelaunchBypassKey &&
    url.searchParams.get("key") === config.prelaunchBypassKey
  ) {
    const headers = new Headers({
      location: "/",
      "cache-control": "no-store, no-cache, must-revalidate",
      pragma: "no-cache",
      expires: "0",
      "x-oa-prelaunch": "bypass",
    })
    headers.append(
      "Set-Cookie",
      `prelaunch_bypass=1; Path=/; Max-Age=${PRELAUNCH_BYPASS_COOKIE_MAX_AGE}; Secure; SameSite=Lax`,
    )
    return new Response(null, { status: 302, headers })
  }
  if (PRELAUNCH_ALWAYS_BLOCKED_PATHNAMES.has(pathname)) return new Response(null, {
    status: 302,
    headers: new Headers({
      location: "/",
      "cache-control": "no-store, no-cache, must-revalidate",
      pragma: "no-cache",
      expires: "0",
      "x-oa-prelaunch": "redirect",
    }),
  })
  const bypassGranted = hasPrelaunchBypass(request, url, config.prelaunchBypassKey)
  if (bypassGranted) return null
  if (PRELAUNCH_ALLOWED_PATHNAMES.has(pathname)) return null
  return new Response(null, {
    status: 302,
    headers: new Headers({
      location: "/",
      "cache-control": "no-store, no-cache, must-revalidate",
      pragma: "no-cache",
      expires: "0",
      "x-oa-prelaunch": "redirect",
    }),
  })
}

export const handleSsrRequest = async (
  request: Request,
  env: WorkerEnv,
): Promise<Response> => {
  const url = new URL(request.url)
  const config = getWorkerAppConfig(env)
  const bypassGranted = hasPrelaunchBypass(request, url, config.prelaunchBypassKey)
  const pathname = normalizePathname(url.pathname)
  if (config.prelaunch && !isLocalDevRequest(request, url) && PRELAUNCH_ALWAYS_BLOCKED_PATHNAMES.has(pathname)) {
    return new Response(null, {
      status: 302,
      headers: new Headers({ location: "/", "cache-control": "no-store" }),
    })
  }
  const allowedWithoutBypass = PRELAUNCH_ALLOWED_PATHNAMES.has(pathname)
  if (config.prelaunch && !isLocalDevRequest(request, url) && !bypassGranted && !allowedWithoutBypass) {
    return new Response(null, {
      status: 302,
      headers: new Headers({ location: "/", "cache-control": "no-store" }),
    })
  }
  const setBypassCookie =
    !!config.prelaunchBypassKey && url.searchParams.get("key") === config.prelaunchBypassKey
  const matched = matchRoute(appRoutes as ReadonlyArray<AnyRoute>, url)

  const { runtime } = getWorkerRuntime(env)
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing"
  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService
    }),
  )
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: request.method,
    pathname: url.pathname,
  })

  const awaitRequestAbort = Effect.async<never, RequestAbortedError>((resume, fiberSignal) => {
    if (request.signal.aborted) {
      resume(Effect.fail(new RequestAbortedError()))
      return
    }

    const onAbort = () => resume(Effect.fail(new RequestAbortedError()))
    request.signal.addEventListener("abort", onAbort, { once: true })

    const cleanup = () => request.signal.removeEventListener("abort", onAbort)
    fiberSignal.addEventListener("abort", cleanup, { once: true })

    return Effect.sync(() => {
      cleanup()
      fiberSignal.removeEventListener("abort", cleanup)
    })
  })

  const effect = Effect.gen(function* () {
    // Unmatched document routes (e.g. /hatchery) redirect to home; assets/API are handled earlier.
    if (!matched) {
      const headers = new Headers({
        location: "/",
        "cache-control": "no-store, no-cache, must-revalidate",
      })
      if (setBypassCookie) {
        // Preserve prelaunch bypass when a legacy path (or typo) includes ?key=.
        headers.append(
          "Set-Cookie",
          `prelaunch_bypass=1; Path=/; Max-Age=${PRELAUNCH_BYPASS_COOKIE_MAX_AGE}; Secure; SameSite=Lax`,
        )
      }
      return new Response(null, {
        status: 302,
        headers,
      })
    }

    const ctx: RouteContext = { _tag: "Server", url, match: matched.match, request }
    const run: AnyRun = yield* runRoute(matched.route, ctx)

    switch (run._tag) {
      case "Redirect": {
        const location = new URL(run.href, url).toString()
        const headers = new Headers({ location, "cache-control": "no-store" })
        if (setBypassCookie) {
          // Important: the prelaunch bypass key is often used on routes that immediately redirect
          // (e.g. auth endpoints or legacy routes that redirect to /). Mint the bypass cookie here so
          // a redirect chain doesn't lose bypass and fall back to the countdown page.
          headers.append(
            "Set-Cookie",
            `prelaunch_bypass=1; Path=/; Max-Age=${PRELAUNCH_BYPASS_COOKIE_MAX_AGE}; Secure; SameSite=Lax`,
          )
        }
        return new Response(null, {
          status: run.status ?? 302,
          headers,
        })
      }
      case "NotFound": {
        const html = renderDocument({
          title: "Not found",
          meta: [],
          bodyHtml: defaultNotFoundHtml(url),
          dehydrateJson: null,
          prelaunch: config.prelaunch,
        })
        if (byteLengthUtf8(html) > MAX_SSR_HTML_BYTES) {
          return new Response("<!doctype html><h1>SSR output too large</h1>", {
            status: 500,
            headers: htmlHeadersNoStore(),
          })
        }
        return new Response(html, {
          status: 404,
          headers: htmlHeadersNoStore(),
        })
      }
      case "Fail": {
        const html = renderDocument({
          title: "Error",
          meta: [],
          bodyHtml: defaultFailHtml(url, run.error),
          dehydrateJson: null,
          prelaunch: config.prelaunch,
        })
        if (byteLengthUtf8(html) > MAX_SSR_HTML_BYTES) {
          return new Response("<!doctype html><h1>SSR output too large</h1>", {
            status: 500,
            headers: htmlHeadersNoStore(),
          })
        }
        return new Response(html, {
          status: run.status ?? 500,
          headers: htmlHeadersNoStore(),
        })
      }
      case "Ok": {
        const bodyHtml = renderToString(run.template)
        const title = run.head?.title ?? "OpenAgents"
        const metaBase = run.head?.meta ?? []
        const hasE2e = !!readE2eTokenFromRequest(request)
        const meta = hasE2e ? [...metaBase, ["oa-e2e", "1"] as const] : metaBase

        // v1 dehydrate payload: route-scoped fragment only (namespaced by routeId).
        const dehydrate =
          run.hints?.dehydrate !== undefined
            ? JSON.stringify({ [run.routeId]: run.hints.dehydrate })
            : null

        const html = renderDocument({
          title,
          meta,
          bodyHtml,
          dehydrateJson: dehydrate,
          prelaunch: config.prelaunch,
        })
        if (byteLengthUtf8(html) > MAX_SSR_HTML_BYTES) {
          return new Response("<!doctype html><h1>SSR output too large</h1>", {
            status: 500,
            headers: htmlHeadersNoStore(),
          })
        }

        const headers = new Headers({ "content-type": "text/html; charset=utf-8" })
        if (run.hints?.headers) {
          for (const [k, v] of run.hints.headers) headers.append(k, v)
        }
        applyCachePolicy(headers, { cache: run.hints?.cache, cookies: run.hints?.cookies })
        applyCookieMutations(headers, run.hints?.cookies)
        if (setBypassCookie) {
          headers.append(
            "Set-Cookie",
            `prelaunch_bypass=1; Path=/; Max-Age=${PRELAUNCH_BYPASS_COOKIE_MAX_AGE}; Secure; SameSite=Lax`,
          )
        }

        return new Response(html, { status: 200, headers })
      }
    }
  }).pipe(
    Effect.raceFirst(awaitRequestAbort),
    // Provide request-scoped context for services (AuthService, ConvexService, etc.).
    Effect.provideService(RequestContextService, makeServerRequestContext(request)),
    Effect.provideService(TelemetryService, requestTelemetry),
    Effect.catchAll((error) => {
      if (error instanceof RequestAbortedError) {
        return Effect.succeed(
          new Response(null, { status: 499, headers: { "cache-control": "no-store" } }),
        )
      }

      console.error(`[ssr] ${formatRequestIdLogToken(requestId)}`, error)

      const html = renderDocument({
        title: "Error",
        meta: [],
        bodyHtml: defaultFailHtml(url, error),
        dehydrateJson: null,
        prelaunch: config.prelaunch,
      })

      if (byteLengthUtf8(html) > MAX_SSR_HTML_BYTES) {
        return Effect.succeed(
          new Response("<!doctype html><h1>SSR output too large</h1>", {
            status: 500,
            headers: htmlHeadersNoStore(),
          }),
        )
      }

      return Effect.succeed(
        new Response(html, {
          status: 500,
          headers: htmlHeadersNoStore(),
        }),
      )
    }),
  )

  return runtime.runPromise(effect)
}
