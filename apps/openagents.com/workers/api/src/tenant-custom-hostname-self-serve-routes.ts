import { Effect, Match as M } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
  unauthorized,
} from './http/responses'
import { recordFromUnknown } from './json-boundary'
import { type TeamRole } from './team-repository'
import {
  type ClaimHostnameInput,
  HostnameManagerRoles,
  makeTenantCustomHostnameSelfServe,
  type TenantCustomHostnameSelfServeConfig,
} from './tenant-custom-hostname-self-serve'

type HttpResponse = globalThis.Response

// CUSTOMER SELF-SERVE custom-hostname routes (OpenAgents #4988 follow-up)
//
//   GET  /api/tenant/hostnames?teamId=...   -> list this team's claimed
//                                              hostnames + DNS instructions
//   POST /api/tenant/hostnames              -> claim a hostname for a team
//        body: { teamId, hostname }
//
// Two gates, both required:
//   1. Browser session — caller must be a signed-in user.
//   2. Team role — caller must have an ACTIVE membership in the named team.
//      - GET (list): any active membership (owner/admin/member/viewer) may
//        view their team's hostnames.
//      - POST (claim): only owner/admin (HostnameManagerRoles) may claim.
//
// This route NEVER touches live DNS, SSL, origin binding, or spend. It only
// creates/reads rows in tenant_custom_hostnames; a claimed hostname stays
// `pending` and does not serve until the owner-gated provisioning core
// (itself default-OFF behind Cloudflare secrets) drives it to active. See
// tenant-custom-hostname-self-serve.ts for the full safety boundary.

type SelfServeSession = Readonly<{
  user: Readonly<{ userId: string }>
}>

export type TenantHostnameSelfServeRouteDependencies<
  Session extends SelfServeSession,
  Bindings,
> = Readonly<{
  database: (env: Bindings) => D1Database
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  // Authorization seam: the caller's ACTIVE role in the named team, or
  // undefined when they have no active membership. Wired to
  // readActiveTeamMembershipRole(db, teamId, userId) in the Worker entry.
  readTeamRole: (
    db: D1Database,
    teamId: string,
    userId: string,
  ) => Promise<TeamRole | undefined>
  // Owner-armed config; defaults to INERT (no live DNS, servingLive=false).
  config?: TenantCustomHostnameSelfServeConfig
}>

const HOSTNAMES_PATH = '/api/tenant/hostnames'

// Parse + bounded-field-extract the claim body. We read the JSON record via the
// shared json-boundary helper and pull the two string fields deterministically;
// this is bounded extraction of structured fields, not intent routing.
const parseBody = (
  request: Request,
): Effect.Effect<ClaimHostnameInput | undefined> =>
  Effect.tryPromise(() => request.json()).pipe(
    Effect.map((raw): ClaimHostnameInput | undefined => {
      const record = recordFromUnknown(raw)

      if (record === undefined) {
        return undefined
      }

      const teamId = record['teamId']
      const hostname = record['hostname']

      if (typeof teamId !== 'string' || typeof hostname !== 'string') {
        return undefined
      }

      return { teamId, hostname }
    }),
    Effect.orElseSucceed((): ClaimHostnameInput | undefined => undefined),
  )

// Resolve the browser session as an Effect. The dependency reference is held in
// a local thunk so the route bodies do not embed Promise dependency adapters
// inline (zero-debt architecture budget).
const sessionEffect = <Session extends SelfServeSession, Bindings>(
  dependencies: TenantHostnameSelfServeRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<Session | undefined> => {
  const resolve = () =>
    dependencies.requireBrowserSession(request, env, ctx)

  return Effect.promise(resolve)
}

// Resolve the caller's active team role as an Effect (same local-thunk pattern).
const roleEffect = <Session extends SelfServeSession, Bindings>(
  dependencies: TenantHostnameSelfServeRouteDependencies<Session, Bindings>,
  db: D1Database,
  teamId: string,
  userId: string,
): Effect.Effect<TeamRole | undefined> => {
  const resolve = () => dependencies.readTeamRole(db, teamId, userId)

  return Effect.promise(resolve)
}

const listHostnames = <Session extends SelfServeSession, Bindings>(
  dependencies: TenantHostnameSelfServeRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  teamId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const session = yield* sessionEffect(dependencies, request, env, ctx)

    if (session === undefined) {
      return unauthorized()
    }

    if (teamId.trim() === '') {
      return noStoreJsonResponse(
        { error: 'tenant_hostname_team_required' },
        { status: 400 },
      )
    }

    const db = dependencies.database(env)
    const role = yield* roleEffect(
      dependencies,
      db,
      teamId,
      session.user.userId,
    )

    // No active membership: do not reveal whether the team or its hostnames
    // exist; answer 403 uniformly.
    if (role === undefined) {
      return forbidden()
    }

    const selfServe = makeTenantCustomHostnameSelfServe(
      db,
      dependencies.config,
    )

    return yield* selfServe.listForTeam(teamId).pipe(
      Effect.map(hostnames =>
        noStoreJsonResponse({ teamId, hostnames }),
      ),
      Effect.catch(() => Effect.succeed(serverError())),
    )
  })

const claimHostname = <Session extends SelfServeSession, Bindings>(
  dependencies: TenantHostnameSelfServeRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const session = yield* sessionEffect(dependencies, request, env, ctx)

    if (session === undefined) {
      return unauthorized()
    }

    const body = yield* parseBody(request)

    if (body === undefined || body.teamId.trim() === '') {
      return noStoreJsonResponse(
        { error: 'tenant_hostname_invalid_request' },
        { status: 400 },
      )
    }

    const db = dependencies.database(env)
    const role = yield* roleEffect(
      dependencies,
      db,
      body.teamId,
      session.user.userId,
    )

    if (role === undefined) {
      return forbidden()
    }

    // Claiming (a write) requires a management role; viewers/members may not.
    if (!HostnameManagerRoles.has(role)) {
      return noStoreJsonResponse(
        { error: 'tenant_hostname_insufficient_role' },
        { status: 403 },
      )
    }

    const selfServe = makeTenantCustomHostnameSelfServe(
      db,
      dependencies.config,
    )

    return yield* selfServe.claim(body).pipe(
      Effect.map(hostname =>
        noStoreJsonResponse({ hostname }, { status: 201 }),
      ),
      Effect.catchTags({
        TenantCustomHostnameValidationError: error =>
          Effect.succeed(
            noStoreJsonResponse(
              {
                error: 'tenant_hostname_invalid',
                reason: error.reason,
              },
              { status: 400 },
            ),
          ),
        TenantCustomHostnameConflictError: () =>
          Effect.succeed(
            noStoreJsonResponse(
              { error: 'tenant_hostname_taken' },
              { status: 409 },
            ),
          ),
        TenantCustomHostnameStorageError: error =>
          Effect.succeed(
            // claim() maps the "taken by another team" case onto a storage
            // error tagged hostname_taken; surface it as a 409, not a 500.
            error.error instanceof Error && error.error.message === 'hostname_taken'
              ? noStoreJsonResponse(
                  { error: 'tenant_hostname_taken' },
                  { status: 409 },
                )
              : serverError(),
          ),
      }),
      Effect.catch(() => Effect.succeed(serverError())),
    )
  })

export const makeTenantHostnameSelfServeRoutes = <
  Session extends SelfServeSession,
  Bindings,
>(
  dependencies: TenantHostnameSelfServeRouteDependencies<Session, Bindings>,
) => ({
  routeTenantHostnameSelfServeRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname !== HOSTNAMES_PATH) {
      return undefined
    }

    return M.value(request.method).pipe(
      M.when('GET', () =>
        listHostnames(
          dependencies,
          request,
          env,
          ctx,
          url.searchParams.get('teamId') ?? '',
        ),
      ),
      M.when('POST', () => claimHostname(dependencies, request, env, ctx)),
      M.orElse(() => Effect.succeed(methodNotAllowed(['GET', 'POST']))),
    )
  },
})
