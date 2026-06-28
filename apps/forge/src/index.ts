import { Effect } from "effect"

import {
  normalizeForgeRoute,
  renderForgeShellEffect,
} from "./shell"

type ForgeEnv = Readonly<{
  FORGE_ENV?: string
}>

const json = (value: unknown, init: ResponseInit = {}): Response =>
  new Response(JSON.stringify(value, null, 2), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...init.headers,
    },
  })

const html = (body: string, init: ResponseInit = {}): Response =>
  new Response(body, {
    ...init,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "public, max-age=60",
      ...init.headers,
    },
  })

const notFound = (pathname: string): Response =>
  json(
    {
      ok: false,
      error: "not_found",
      pathname,
    },
    { status: 404 },
  )

const handleRequest = (
  request: Request,
  env: ForgeEnv,
): Effect.Effect<Response> =>
  Effect.gen(function* () {
    const url = new URL(request.url)

    if (request.method !== "GET" && request.method !== "HEAD") {
      return json(
        {
          ok: false,
          error: "method_not_allowed",
          method: request.method,
        },
        {
          status: 405,
          headers: {
            allow: "GET, HEAD",
          },
        },
      )
    }

    if (url.pathname === "/health") {
      return json({
        ok: true,
        service: "openagents-forge",
        env: env.FORGE_ENV ?? "development",
        canonicalHost: "forge.openagents.com",
      })
    }

    if (url.pathname === "/version") {
      return json({
        service: "openagents-forge",
        appBoundary: "apps/forge",
        canonicalHost: "forge.openagents.com",
        apiDependency: "forge.public_safe_contract.pending",
      })
    }

    const route = normalizeForgeRoute(url.pathname)
    const knownShellPath =
      url.pathname === "/" || url.pathname === `/${route}`
    if (!knownShellPath) {
      return notFound(url.pathname)
    }

    const body = yield* renderForgeShellEffect({
      route,
      generatedAt: new Date().toISOString(),
    })
    return html(body)
  })

export default {
  fetch(request, env): Promise<Response> {
    return Effect.runPromise(handleRequest(request, env))
  },
} satisfies ExportedHandler<ForgeEnv>

