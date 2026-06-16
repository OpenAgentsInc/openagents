import { badRequest } from '@openagentsinc/sync-worker'
import { Effect, Match as M, Schema as S } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  redirectResponse,
  unauthorized,
} from './http/responses'
import { readJsonObject } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TeamWorkspaceInviteAcceptResult,
  TeamWorkspaceInviteRole,
  type TeamWorkspaceInviteStore,
  safeTeamWorkspaceInviteProjection,
} from './team-workspace-invites'

type HttpResponse = globalThis.Response

type TeamWorkspaceInviteSession = Readonly<{
  tokens?: unknown
  user: Readonly<{
    email: string
    name: string
    userId: string
  }>
}>

type TeamWorkspaceInviteRouteDependencies<
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  appOrigin: (env: Bindings) => string
  makeStore: (env: Bindings) => TeamWorkspaceInviteStore
  nowIso?: () => string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

const CreateTeamWorkspaceInviteRequest = S.Struct({
  email: S.String,
  expiresAt: S.optionalKey(S.String),
  expiresInHours: S.optionalKey(S.Number),
  metadataJson: S.optionalKey(S.String),
  projectId: S.optionalKey(S.Union([S.String, S.Null])),
  role: S.optionalKey(TeamWorkspaceInviteRole),
  teamId: S.String,
})
type CreateTeamWorkspaceInviteRequest =
  typeof CreateTeamWorkspaceInviteRequest.Type

const AcceptTeamWorkspaceInviteRequest = S.Struct({
  token: S.String,
})
type AcceptTeamWorkspaceInviteRequest =
  typeof AcceptTeamWorkspaceInviteRequest.Type

const routeNowIso = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const decodeBody = <A>(
  schema: S.Decoder<A>,
  request: Request,
): Effect.Effect<A, HttpResponse> =>
  Effect.tryPromise({
    catch: error =>
      badRequest(error instanceof Error ? error.message : String(error)),
    try: async () => S.decodeUnknownSync(schema)(await readJsonObject(request)),
  })

const acceptUrl = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  env: Bindings,
  token: string,
): string =>
  `${dependencies.appOrigin(env)}/api/team-workspace-invites/accept?token=${encodeURIComponent(token)}`

const projectRedirectUrl = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  env: Bindings,
  result: Extract<
    TeamWorkspaceInviteAcceptResult,
    { _tag: 'Accepted' | 'AlreadyAccepted' }
  >,
): string =>
  result.invite.projectId === null
    ? `${dependencies.appOrigin(env)}/teams/${encodeURIComponent(result.invite.teamId)}/chat`
    : `${dependencies.appOrigin(env)}/teams/${encodeURIComponent(result.invite.teamId)}/projects/${encodeURIComponent(result.invite.projectId)}/chat`

const safeAcceptInviteView = (
  result: Extract<
    TeamWorkspaceInviteAcceptResult,
    { _tag: 'Accepted' | 'AlreadyAccepted' }
  >,
) => ({
  invite: safeTeamWorkspaceInviteProjection(result.invite),
  membership: {
    role: result.invite.role,
    status: 'active',
    teamId: result.invite.teamId,
  },
})

const createInvite = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const authorized = yield* Effect.tryPromise({
      catch: () => unauthorized(),
      try: () => dependencies.requireAdminApiToken(request, env),
    })

    if (!authorized) {
      return unauthorized()
    }

    const body = yield* decodeBody(CreateTeamWorkspaceInviteRequest, request)
    const result = yield* Effect.promise(() =>
      dependencies.makeStore(env).createOrRefreshInvite({
        email: body.email,
        expiresAt: body.expiresAt,
        expiresInHours: body.expiresInHours,
        invitedByActorRef: 'operator:admin_api',
        metadataJson: body.metadataJson,
        projectId: body.projectId,
        role: body.role,
        teamId: body.teamId,
      }),
    )

    return M.value(result).pipe(
      M.tags({
        Created: ({ invite, token }) =>
          noStoreJsonResponse(
            {
              acceptUrl: acceptUrl(dependencies, env, token),
              generatedAt: routeNowIso(dependencies),
              invite: safeTeamWorkspaceInviteProjection(invite),
            },
            { status: 201 },
          ),
        InvalidEmail: () => badRequest('A valid invite email is required.'),
        ProjectNotFound: () =>
          noStoreJsonResponse(
            {
              error: 'team_workspace_invite_project_not_found',
              reason: 'Project not found for the requested team.',
            },
            { status: 404 },
          ),
        Refreshed: ({ invite, token }) =>
          noStoreJsonResponse({
            acceptUrl: acceptUrl(dependencies, env, token),
            generatedAt: routeNowIso(dependencies),
            invite: safeTeamWorkspaceInviteProjection(invite),
          }),
        TeamNotFound: () =>
          noStoreJsonResponse(
            {
              error: 'team_workspace_invite_team_not_found',
              reason: 'Team not found.',
            },
            { status: 404 },
          ),
      }),
      M.exhaustive,
    )
  }).pipe(Effect.catch(error => Effect.succeed(error)))

const tokenFromAcceptRequest = (
  request: Request,
): Effect.Effect<string, HttpResponse> => {
  if (request.method === 'GET') {
    const token = new URL(request.url).searchParams.get('token')?.trim()

    return token === undefined || token === ''
      ? Effect.fail(badRequest('Invite token is required.'))
      : Effect.succeed(token)
  }

  return decodeBody(AcceptTeamWorkspaceInviteRequest, request).pipe(
    Effect.map(body => body.token),
  )
}

const acceptInvite = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST' && request.method !== 'GET') {
      return methodNotAllowed(['GET', 'POST'])
    }

    const token = yield* tokenFromAcceptRequest(request)
    const session = yield* Effect.tryPromise({
      catch: () => unauthorized(),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return unauthorized()
    }

    const result = yield* Effect.promise(() =>
      dependencies.makeStore(env).acceptInvite({
        sessionEmail: session.user.email,
        token,
        userId: session.user.userId,
      }),
    )

    const response = M.value(result).pipe(
      M.tags({
        Accepted: result =>
          request.method === 'GET'
            ? redirectResponse(projectRedirectUrl(dependencies, env, result))
            : noStoreJsonResponse({
                generatedAt: routeNowIso(dependencies),
                ...safeAcceptInviteView(result),
              }),
        AlreadyAccepted: result =>
          request.method === 'GET'
            ? redirectResponse(projectRedirectUrl(dependencies, env, result))
            : noStoreJsonResponse({
                generatedAt: routeNowIso(dependencies),
                ...safeAcceptInviteView(result),
              }),
        Expired: ({ invite }) =>
          noStoreJsonResponse(
            {
              error: 'team_workspace_invite_expired',
              invite: safeTeamWorkspaceInviteProjection(invite),
            },
            { status: 410 },
          ),
        InviteUnavailable: ({ status }) =>
          noStoreJsonResponse(
            {
              error: 'team_workspace_invite_unavailable',
              status,
            },
            { status: 409 },
          ),
        NotFound: () =>
          noStoreJsonResponse(
            { error: 'team_workspace_invite_not_found' },
            { status: 404 },
          ),
        WrongUser: () => forbidden(),
      }),
      M.exhaustive,
    )

    return dependencies.appendRefreshedSessionCookies(response, session)
  }).pipe(Effect.catch(error => Effect.succeed(error)))

export const makeTeamWorkspaceInviteRoutes = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
) => ({
  routeTeamWorkspaceInviteRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/team-workspace-invites') {
      return createInvite(dependencies, request, env)
    }

    if (url.pathname === '/api/team-workspace-invites/accept') {
      return acceptInvite(dependencies, request, env, ctx)
    }

    return undefined
  },
})
