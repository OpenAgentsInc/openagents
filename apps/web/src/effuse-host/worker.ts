import { routeAgentRequest } from "agents"
import { Effect } from "effect"

import { AuthService } from "../effect/auth"
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext"

import { tryServeAsset } from "./assets"
import { handleAuthRequest } from "./auth"
import { handleCallbackRequest } from "./callback"
import { handleRpcRequest } from "./rpc"
import { handleSsrRequest } from "./ssr"
import { getWorkerRuntime } from "./runtime"
import type { WorkerEnv } from "./env"

export { Chat } from "./do/chat"
export { UserSpaceDO } from "./do/userSpace"

const json = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  })

const handleUserSpaceProxy = async (request: Request, env: WorkerEnv): Promise<Response> => {
  if (!env.UserSpaceDO) {
    return json({ ok: false, error: "userspace_unbound" }, { status: 500 })
  }

  const { runtime } = getWorkerRuntime(env)

  // Read auth from WorkOS cookie (server-side) so we can route to the correct DO
  // and authenticate Convex replication calls.
  const authExit = await runtime.runPromiseExit(
    Effect.gen(function* () {
      const auth = yield* AuthService
      const session = yield* auth.getSession()
      const token = yield* auth.getAccessToken({ forceRefreshToken: false })
      return { session, token }
    }).pipe(Effect.provideService(RequestContextService, makeServerRequestContext(request))),
  )

  if (authExit._tag === "Failure") {
    return json({ ok: false, error: "auth_failed" }, { status: 401 })
  }

  const { session, token } = authExit.value
  const userId = session.userId
  if (!userId) {
    return json({ ok: false, error: "unauthorized" }, { status: 401 })
  }

  const id = env.UserSpaceDO.idFromName(userId)
  const stub = env.UserSpaceDO.get(id)

  const headers = new Headers(request.headers)
  headers.set("x-user-id", userId)
  if (token) {
    headers.set("authorization", `Bearer ${token}`)
  }

  const forwarded = new Request(request.url, {
    method: request.method,
    headers,
    body: request.body,
    redirect: "manual",
  })

  return stub.fetch(forwarded)
}

export default {
  async fetch(request: Request, env: WorkerEnv, ctx: ExecutionContext): Promise<Response> {
    ctx.passThroughOnException()

    const url = new URL(request.url)

    // Agent runtime (Durable Object-backed) for WebSockets + transcript.
    if (url.pathname.startsWith("/agents")) {
      const response = await routeAgentRequest(request, env as any)
      if (response) return response
    }

    // API surfaces.
    if (url.pathname === "/api/rpc") {
      return handleRpcRequest(request, env)
    }

    // WorkOS OAuth callback (legacy path configured as WORKOS_REDIRECT_URI).
    if (url.pathname === "/callback") {
      return handleCallbackRequest(request)
    }

    if (url.pathname.startsWith("/api/auth/")) {
      const response = await handleAuthRequest(request, env)
      if (response) return response
    }

    if (url.pathname.startsWith("/api/user-space")) {
      return handleUserSpaceProxy(request, env)
    }

    // Static assets.
    const asset = await tryServeAsset(request, env)
    if (asset) return asset

    // SSR (GET/HEAD only).
    if (request.method === "GET" || request.method === "HEAD") {
      return handleSsrRequest(request, env)
    }

    return new Response("Not found", { status: 404 })
  },
} satisfies ExportedHandler<WorkerEnv>
