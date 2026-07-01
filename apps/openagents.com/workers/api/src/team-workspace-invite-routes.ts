import { readRequestJsonEffect } from '@openagentsinc/effect-boundary'
import { badRequest } from '@openagentsinc/sync-worker'
import { Effect, Match as M, Schema as S } from 'effect'

import type { ResendEmailConfig } from './config'
import { PrivateWorkspaceInviteEmailInput } from './email'
import type { EmailLedgerSendResult, EmailServiceError } from './email'
import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  redirectResponse,
  serverError,
  unauthorized,
} from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TeamWorkspaceInviteAcceptResult,
  TeamWorkspaceInviteRole,
  type TeamWorkspaceInviteStore,
  safeTeamWorkspaceInviteProjection,
} from './team-workspace-invites'

type HttpResponse = globalThis.Response

type TeamWorkspaceInviteEmailDelivery =
  | Readonly<{
      emailMessageId: string
      providerMessageId: string | null
      status: 'accepted'
    }>
  | Readonly<{
      emailMessageId: string | null
      errorMessage: string
      errorName: string
      status: 'failed'
    }>
  | Readonly<{
      errorMessage: string
      errorName: string
      status: 'missing_config'
    }>
  | Readonly<{ status: 'disabled' }>

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
  getResendEmailConfig?: (env: Bindings) => ResendEmailConfig | undefined
  makeStore: (env: Bindings) => TeamWorkspaceInviteStore
  nowIso?: () => string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  sendInviteEmailWithLedger?: (
    env: Bindings,
    config: ResendEmailConfig,
    input: PrivateWorkspaceInviteEmailInput,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
}>

const CreateTeamWorkspaceInviteRequest = S.Struct({
  email: S.String,
  expiresAt: S.optionalKey(S.String),
  expiresInHours: S.optionalKey(S.Number),
  metadataJson: S.optionalKey(S.String),
  projectId: S.optionalKey(S.Union([S.String, S.Null])),
  recipientDisplayName: S.optionalKey(S.String),
  role: S.optionalKey(TeamWorkspaceInviteRole),
  sendEmail: S.optionalKey(S.Boolean),
  teamId: S.String,
  workspaceLabel: S.optionalKey(S.String),
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
  readRequestJsonEffect(
    schema,
    request,
    'team_workspace_invite.body',
  ).pipe(
    Effect.mapError(error =>
      badRequest(
        error.reasonRef === 'boundary.json.malformed'
          ? 'Malformed JSON request body.'
          : 'Team workspace invite request did not match the expected schema.',
      ),
    ),
  )

const dependencyPromise = <A>(
  tryPromise: () => Promise<A>,
): Effect.Effect<A, HttpResponse> =>
  Effect.tryPromise({
    catch: () => serverError(),
    try: tryPromise,
  })

const recordInviteEmailAttempt = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  env: Bindings,
  input: Readonly<{
    attemptedAt: string
    emailMessageId: string
    inviteId: string
  }>,
): Effect.Effect<void> => {
  const store = dependencies.makeStore(env)

  return Effect.tryPromise({
    catch: () => undefined,
    try: () => store.recordEmailAttempt(input),
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )
}

const acceptUrl = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  env: Bindings,
  token: string,
): string =>
  `${dependencies.appOrigin(env)}/api/team-workspace-invites/accept?token=${encodeURIComponent(token)}`

const privateInviteEmailIdempotencyKey = (
  inviteId: string,
  sendCount: number,
): string => `team_workspace_invite:${inviteId}:${sendCount + 1}`

const privateInviteEmailInput = (
  body: CreateTeamWorkspaceInviteRequest,
  invite: Readonly<{
    expiresAt: string
    id: string
    inviteeEmail: string
    projectId: string | null
    sendCount: number
    teamId: string
  }>,
  inviteAcceptUrl: string,
): PrivateWorkspaceInviteEmailInput =>
  new PrivateWorkspaceInviteEmailInput({
    acceptUrl: inviteAcceptUrl,
    displayName: body.recipientDisplayName ?? 'there',
    expiresAt: invite.expiresAt,
    idempotencyKey: privateInviteEmailIdempotencyKey(
      invite.id,
      invite.sendCount,
    ),
    inviteId: invite.id,
    projectId: invite.projectId,
    teamId: invite.teamId,
    to: invite.inviteeEmail,
    workspaceLabel: body.workspaceLabel ?? 'your private OpenAgents workspace',
  })

const safeInviteEmailFailure = (
  error: EmailServiceError,
): TeamWorkspaceInviteEmailDelivery => ({
  emailMessageId: null,
  errorMessage: error.message,
  errorName: 'email_service_error',
  status: 'failed',
})

const sendInviteEmail = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  env: Bindings,
  body: CreateTeamWorkspaceInviteRequest,
  invite: Readonly<{
    expiresAt: string
    id: string
    inviteeEmail: string
    projectId: string | null
    sendCount: number
    teamId: string
  }>,
  inviteAcceptUrl: string,
): Effect.Effect<TeamWorkspaceInviteEmailDelivery> => {
  if (body.sendEmail === false) {
    return Effect.succeed({ status: 'disabled' })
  }

  const config = dependencies.getResendEmailConfig?.(env)

  if (
    config === undefined ||
    dependencies.sendInviteEmailWithLedger === undefined
  ) {
    return Effect.succeed({
      errorMessage: 'Resend email configuration is not set.',
      errorName: 'email_config_missing',
      status: 'missing_config',
    })
  }

  return dependencies
    .sendInviteEmailWithLedger(
      env,
      config,
      privateInviteEmailInput(body, invite, inviteAcceptUrl),
    )
    .pipe(
      Effect.flatMap(result =>
        Effect.gen(function* () {
          yield* recordInviteEmailAttempt(dependencies, env, {
            attemptedAt: routeNowIso(dependencies),
            emailMessageId: result.emailMessageId,
            inviteId: invite.id,
          })

          return result.ok
            ? {
                emailMessageId: result.emailMessageId,
                providerMessageId: result.providerMessageId,
                status: 'accepted' as const,
              }
            : {
                emailMessageId: result.emailMessageId,
                errorMessage: result.errorMessage,
                errorName: result.errorName ?? 'resend_error',
                status: 'failed' as const,
              }
        }),
      ),
      Effect.catch(error => Effect.succeed(safeInviteEmailFailure(error))),
    )
}

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

const acceptReturnPath = (request: Request): string => {
  const url = new URL(request.url)

  return `${url.pathname}${url.search}`
}

const acceptLoginRedirect = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
): HttpResponse =>
  redirectResponse(
    `${dependencies.appOrigin(env)}/login/email?returnTo=${encodeURIComponent(
      acceptReturnPath(request),
    )}`,
  )

const acceptWrongAccountRedirect = <
  Bindings,
  Session extends TeamWorkspaceInviteSession = TeamWorkspaceInviteSession,
>(
  dependencies: TeamWorkspaceInviteRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
): HttpResponse =>
  redirectResponse(
    `${dependencies.appOrigin(env)}/auth/logout?returnTo=${encodeURIComponent(
      acceptReturnPath(request),
    )}`,
  )

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
    const result = yield* dependencyPromise(() =>
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

    return yield* M.value(result).pipe(
      M.tags({
        Created: ({ invite, token }) =>
          Effect.gen(function* () {
            const inviteAcceptUrl = acceptUrl(dependencies, env, token)
            const email = yield* sendInviteEmail(
              dependencies,
              env,
              body,
              invite,
              inviteAcceptUrl,
            )

            return noStoreJsonResponse(
              {
                acceptUrl: inviteAcceptUrl,
                email,
                generatedAt: routeNowIso(dependencies),
                invite: safeTeamWorkspaceInviteProjection(invite),
              },
              { status: 201 },
            )
          }),
        InvalidEmail: () =>
          Effect.succeed(badRequest('A valid invite email is required.')),
        ProjectNotFound: () =>
          Effect.succeed(
            noStoreJsonResponse(
              {
                error: 'team_workspace_invite_project_not_found',
                reason: 'Project not found for the requested team.',
              },
              { status: 404 },
            ),
          ),
        Refreshed: ({ invite, token }) =>
          Effect.gen(function* () {
            const inviteAcceptUrl = acceptUrl(dependencies, env, token)
            const email = yield* sendInviteEmail(
              dependencies,
              env,
              body,
              invite,
              inviteAcceptUrl,
            )

            return noStoreJsonResponse({
              acceptUrl: inviteAcceptUrl,
              email,
              generatedAt: routeNowIso(dependencies),
              invite: safeTeamWorkspaceInviteProjection(invite),
            })
          }),
        TeamNotFound: () =>
          Effect.succeed(
            noStoreJsonResponse(
              {
                error: 'team_workspace_invite_team_not_found',
                reason: 'Team not found.',
              },
              { status: 404 },
            ),
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
      return request.method === 'GET'
        ? acceptLoginRedirect(dependencies, request, env)
        : unauthorized()
    }

    const result = yield* dependencyPromise(() =>
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
        WrongUser: () =>
          request.method === 'GET'
            ? acceptWrongAccountRedirect(dependencies, request, env)
            : forbidden(),
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
