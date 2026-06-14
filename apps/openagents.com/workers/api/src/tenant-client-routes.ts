import { Effect, Match as M } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { type TenantRef } from './tenant-custom-hostnames'
import {
  type TenantClientWorkroomViewError,
  tenantClientWorkroomView,
} from './tenant-client-views'

type HttpResponse = globalThis.Response

// WS-I tenant client scoped workroom routes (#4991)
//
// GET /api/tenant/client/workrooms/:workroomId
//
// Returns the CUSTOMER-scoped projection of a workroom for the signed-in client
// on a branded tenant subdomain. Two gates, both required:
//
//   1. Browser session — the caller must be a signed-in user (we reuse the same
//      `requireBrowserSession` dependency every other browser route uses).
//   2. Tenant scope — the request must resolve to a tenant (branded host). The
//      `resolveTenant` dependency is host-resolution: today it should call
//      `makeTenantCustomHostnames(db).resolveTenantByHostname(host)` against the
//      inbound Host header. If no tenant resolves, the route does not apply
//      (returns undefined) — these routes only exist on branded subdomains.
//
// Authorization (membership + workroom-in-tenant + visibility) and projection
// live in `tenant-client-views.ts`. This module only does HTTP shape + the two
// gates. It does NOT weaken visibility rules.

type TenantClientSession = Readonly<{
  user: Readonly<{ userId: string }>
}>

export type TenantClientRouteDependencies<
  Session extends TenantClientSession,
  Bindings,
> = Readonly<{
  // Host resolution: yields the tenant for this request, or undefined when the
  // host is not a provisioned branded tenant. Separate follow-up wires this to
  // resolveTenantByHostname over the Host header in the Worker entry.
  resolveTenant: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<TenantRef | undefined>
  // Database accessor for the scoped-view core.
  database: (env: Bindings) => D1Database
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

const WORKROOM_PATH = /^\/api\/tenant\/client\/workrooms\/([^/]+)$/

const workroomIdFromPath = (pathname: string): string | undefined => {
  const match = WORKROOM_PATH.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const denialStatus = (error: TenantClientWorkroomViewError): number => {
  if (error._tag === 'TenantClientWorkroomViewStorageError') {
    return 500
  }

  switch (error.reason) {
    case 'not_authorized_for_tenant':
      // Do not reveal whether the workroom exists to a non-member.
      return 403
    case 'workroom_not_found':
      return 404
    case 'workroom_not_in_tenant':
      // Cross-tenant request: respond 404 so tenant A cannot probe tenant B's
      // workroom ids by status-code differences.
      return 404
    case 'workroom_not_client_visible':
      return 404
  }
}

const denialBody = (
  error: TenantClientWorkroomViewError,
): Readonly<Record<string, string>> =>
  error._tag === 'TenantClientWorkroomViewStorageError'
    ? { error: 'tenant_client_view_storage_error' }
    : { error: `tenant_client_view_${error.reason}` }

const getClientWorkroomView = <
  Session extends TenantClientSession,
  Bindings,
>(
  dependencies: TenantClientRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workroomId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const session = yield* Effect.promise(() =>
      dependencies.requireBrowserSession(request, env, ctx),
    )

    if (session === undefined) {
      return unauthorized()
    }

    const tenant = yield* Effect.promise(() =>
      dependencies.resolveTenant(request, env, ctx),
    )

    // No tenant => not a branded subdomain. The route effectively does not
    // apply; we answer 404 rather than leak that the path exists globally.
    if (tenant === undefined) {
      return noStoreJsonResponse(
        { error: 'tenant_client_view_no_tenant' },
        { status: 404 },
      )
    }

    const result = yield* tenantClientWorkroomView(
      dependencies.database(env),
      {
        clientUserId: session.user.userId,
        tenant,
        workroomId,
      },
    ).pipe(
      Effect.map(view => ({ view })),
      Effect.catch(error => Effect.succeed({ error })),
    )

    if ('error' in result) {
      return noStoreJsonResponse(denialBody(result.error), {
        status: denialStatus(result.error),
      })
    }

    return noStoreJsonResponse({
      surface: result.view.surface,
      teamId: result.view.teamId,
      workroom: result.view.projection.workroom,
      economics: result.view.projection.economics,
      evidenceBundles: result.view.projection.evidenceBundles,
      lifecycleDecisions: result.view.projection.lifecycleDecisions,
      routeScorecards: result.view.projection.routeScorecards,
      workroomId: result.view.workroomId,
    })
  })

export const makeTenantClientRoutes = <
  Session extends TenantClientSession,
  Bindings,
>(
  dependencies: TenantClientRouteDependencies<Session, Bindings>,
) => ({
  routeTenantClientRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const workroomId = workroomIdFromPath(url.pathname)

    if (workroomId === undefined) {
      return undefined
    }

    return M.value(request.method).pipe(
      M.when('GET', () =>
        getClientWorkroomView(dependencies, request, env, ctx, workroomId),
      ),
      M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
    )
  },
})

// ---------------------------------------------------------------------------
// COORDINATOR WIRING (#4991) — INTEGRATION DEFERRED, do not wire from here.
//
// Construct (alongside the other route factories in the Worker entry):
//
//   const tenantClientRoutes = makeTenantClientRoutes<BrowserSession, Env>({
//     database: env => openAgentsDatabase(env),       // existing accessor
//     requireBrowserSession,                           // SAME helper the other
//                                                      // browser routes already
//                                                      // pass (see
//                                                      // autopilot-decision-routes
//                                                      // / agent-proposal-routes)
//     resolveTenant: async (request, env) => {
//       // Host-resolution. DEPENDS ON the host-resolution middleware as a
//       // SEPARATE FOLLOW-UP. Today this can call the existing resolver
//       // directly off the Host header:
//       const host = request.headers.get('Host') ?? ''
//       const tenant = await runEffectProgram(
//         makeTenantCustomHostnames(openAgentsDatabase(env))
//           .resolveTenantByHostname(host),
//       )
//       return tenant ?? undefined
//     },
//   })
//
// Chain (in the request router, before the catch-all; order with the other
// `route*Request(...)` factories — first non-undefined wins):
//
//   const tenantClient = tenantClientRoutes.routeTenantClientRequest(
//     request, env, ctx,
//   )
//   if (tenantClient !== undefined) return runEffectProgram(tenantClient)
//
// Notes:
//   - The route only "applies" when the path matches
//     /api/tenant/client/workrooms/:id; on any other path it returns undefined
//     so the router falls through unchanged.
//   - Once a shared host-resolution middleware exists, prefer passing the
//     already-resolved TenantRef into `resolveTenant` (e.g. read it off a
//     request-scoped context) instead of re-querying per route. The
//     `resolveTenant` seam is intentionally a function so that swap is local.
//   - Visibility/authorization is NOT configurable here: it lives in
//     tenant-client-views.ts and must not be relaxed at the wiring layer.
// ---------------------------------------------------------------------------
