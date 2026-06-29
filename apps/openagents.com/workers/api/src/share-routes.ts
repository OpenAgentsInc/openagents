import { Effect, Layer, Match as M } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { Env as OpenAgentsEnv } from './index'
import { isRecord, optionalString, readJsonObject } from './json-boundary'
import { type AgentRunBundle } from './omni-runs'
import type { OperatorTargetUser } from './operator-targets'
import { openAgentsDatabase } from './runtime'
import { currentEpochMillis, currentIsoTimestamp } from './runtime-primitives'
import {
  ShareAccessService,
  ShareProjectionBuilder,
  type ShareProjectionError,
  ShareProjectionForbidden,
  ShareProjectionMalformed,
  ShareProjectionNotFound,
  ShareProjectionRepository,
  ShareProjectionStorageError,
  ShareReceiptService,
  ShareUrlService,
  type ShareViewer,
  addReceiptRef,
  audienceLabel,
  decodeShareCreateRequest,
  decodeShareUpdateRequest,
  makeShareId,
  sourceProjectId,
  sourceTeamId,
} from './share-projections'
import { type TeamChatMessage, listTeamChatMessages } from './team-chat'
import { requireAuthorizedAgentRunBundle } from './thread-access'

type ShareSession = Readonly<{
  user: Readonly<{
    email: string
    name: string
    userId: string
  }>
}>

type ShareHttpResponse = Response
type ShareRouteEffect = Effect.Effect<ShareHttpResponse>
type ShareRouteEnv = OpenAgentsEnv

type ShareRouteDependencies<Session extends ShareSession> = Readonly<{
  appendRefreshedSessionCookies: (
    response: ShareHttpResponse,
    session: Session,
  ) => ShareHttpResponse
  appOrigin: (env: ShareRouteEnv) => string
  authenticateRequestActor: (
    request: Request,
    env: ShareRouteEnv,
    ctx: ExecutionContext,
  ) => Promise<ShareAuthenticatedActor | undefined>
  isAdminEmail: (email: string) => boolean
  readSelectedOperatorTargetUser: (
    db: D1Database,
    selector: Record<string, unknown>,
  ) => Promise<OperatorTargetUser | undefined>
  requireAdminApiToken: (
    request: Request,
    env: ShareRouteEnv,
  ) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: ShareRouteEnv,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

type ShareAuthenticatedActor =
  | Readonly<{
      kind: 'agent'
      agent: Readonly<{
        user: Readonly<{
          displayName: string
          id: string
          primaryEmail: string | null
        }>
      }>
    }>
  | Readonly<{
      kind: 'human'
      user: Readonly<{
        email: string
        name: string
        userId: string
      }>
    }>

type ShareRouteActor<Session extends ShareSession> =
  | Readonly<{
      _tag: 'Api'
      viewer: ShareViewer
    }>
  | Readonly<{
      _tag: 'Browser'
      session: Session
      viewer: ShareViewer
    }>

type ShareCreatorResolution<Session extends ShareSession> =
  | Readonly<{
      _tag: 'Creator'
      actor: ShareRouteActor<Session>
    }>
  | Readonly<{
      _tag: 'Response'
      response: ShareHttpResponse
    }>

type SourceBundle =
  | Readonly<{
      _tag: 'AgentRun'
      bundle: AgentRunBundle
      ownerUserId: string
      projectId: string | null
      teamId: string | null
    }>
  | Readonly<{
      _tag: 'TeamThread'
      messages: ReadonlyArray<TeamChatMessage>
      ownerUserId: string
      projectId: string | null
      teamId: string
    }>

const shareUuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

const optionalShareUuid = (value: string | undefined): string | undefined =>
  value !== undefined && shareUuidPattern.test(value)
    ? value.toLowerCase()
    : undefined

const errorStatus = (error: ShareProjectionError): number =>
  M.value(error).pipe(
    M.tags({
      ShareProjectionAuthenticationRequired: () => 401,
      ShareProjectionForbidden: () => 403,
      ShareProjectionMalformed: () => 400,
      ShareProjectionNotFound: () => 404,
      ShareProjectionStorageError: () => 500,
      ShareProjectionUnsafe: () => 422,
    }),
    M.exhaustive,
  )

const errorCode = (error: ShareProjectionError): string =>
  M.value(error).pipe(
    M.tags({
      ShareProjectionAuthenticationRequired: () => 'authentication_required',
      ShareProjectionForbidden: () => 'forbidden',
      ShareProjectionMalformed: () => 'bad_request',
      ShareProjectionNotFound: () => 'not_found',
      ShareProjectionStorageError: () => 'share_projection_unavailable',
      ShareProjectionUnsafe: () => 'share_projection_unsafe',
    }),
    M.exhaustive,
  )

const errorResponse = (error: ShareProjectionError): ShareHttpResponse =>
  noStoreJsonResponse(
    {
      error: errorCode(error),
      ...(error._tag === 'ShareProjectionMalformed'
        ? { reason: error.reason }
        : {}),
    },
    { status: errorStatus(error) },
  )

const viewerFromSession = (
  session: ShareSession,
  dependencies: Pick<ShareRouteDependencies<ShareSession>, 'isAdminEmail'>,
): ShareViewer => ({
  email: session.user.email,
  isAdmin: dependencies.isAdminEmail(session.user.email),
  name: session.user.name,
  userId: session.user.userId,
})

const viewerFromOperatorTargetUser = (
  targetUser: OperatorTargetUser,
  dependencies: Pick<ShareRouteDependencies<ShareSession>, 'isAdminEmail'>,
): ShareViewer => ({
  email: targetUser.email ?? '',
  isAdmin:
    targetUser.email === null
      ? false
      : dependencies.isAdminEmail(targetUser.email),
  name: targetUser.displayName,
  userId: targetUser.userId,
})

const viewerFromAuthenticatedActor = (
  actor: ShareAuthenticatedActor,
  dependencies: Pick<ShareRouteDependencies<ShareSession>, 'isAdminEmail'>,
): ShareViewer =>
  actor.kind === 'human'
    ? {
        email: actor.user.email,
        isAdmin: dependencies.isAdminEmail(actor.user.email),
        name: actor.user.name,
        userId: actor.user.userId,
      }
    : {
        email: actor.agent.user.primaryEmail ?? '',
        isAdmin: false,
        name: actor.agent.user.displayName,
        userId: actor.agent.user.id,
      }

const adminApiViewer: ShareViewer = {
  email: 'admin-api@openagents.local',
  isAdmin: true,
  name: 'OpenAgents Admin API',
  userId: 'openagents:admin-api',
}

const appendActorCookies = <Session extends ShareSession>(
  response: ShareHttpResponse,
  actor: ShareRouteActor<Session>,
  dependencies: ShareRouteDependencies<Session>,
): ShareHttpResponse =>
  actor._tag === 'Browser'
    ? dependencies.appendRefreshedSessionCookies(response, actor.session)
    : response

const targetSelectorFromShareBody = (
  body: Record<string, unknown>,
): Record<string, unknown> => {
  const nestedTarget =
    isRecord(body.targetUser) || isRecord(body.target)
      ? isRecord(body.targetUser)
        ? body.targetUser
        : body.target
      : undefined

  return nestedTarget === undefined ? body : { ...body, ...nestedTarget }
}

const hasExplicitTargetSelector = (
  selector: Record<string, unknown>,
): boolean =>
  optionalString(selector.userId) !== undefined ||
  optionalString(selector.email) !== undefined ||
  optionalString(selector.githubLogin) !== undefined ||
  optionalString(selector.login) !== undefined

const resolveOptionalActor = <Session extends ShareSession>(
  request: Request,
  env: ShareRouteEnv,
  ctx: ExecutionContext,
  dependencies: ShareRouteDependencies<Session>,
): Effect.Effect<
  ShareRouteActor<Session> | undefined,
  ShareProjectionStorageError
> =>
  Effect.tryPromise({
    try: async () => {
      const session = await dependencies.requireBrowserSession(
        request,
        env,
        ctx,
      )

      if (session !== undefined) {
        return {
          _tag: 'Browser' as const,
          session,
          viewer: viewerFromSession(session, dependencies),
        }
      }

      if (await dependencies.requireAdminApiToken(request, env)) {
        return {
          _tag: 'Api' as const,
          viewer: adminApiViewer,
        }
      }

      const actor = await dependencies.authenticateRequestActor(
        request,
        env,
        ctx,
      )

      return actor === undefined
        ? undefined
        : {
            _tag: 'Api' as const,
            viewer: viewerFromAuthenticatedActor(actor, dependencies),
          }
    },
    catch: error =>
      new ShareProjectionStorageError({
        error,
        operation: 'share.actor',
      }),
  })

const resolveRequiredActor = <Session extends ShareSession>(
  request: Request,
  env: ShareRouteEnv,
  ctx: ExecutionContext,
  dependencies: ShareRouteDependencies<Session>,
): Effect.Effect<
  ShareCreatorResolution<Session>,
  ShareProjectionStorageError
> =>
  resolveOptionalActor(request, env, ctx, dependencies).pipe(
    Effect.map(actor =>
      actor === undefined
        ? {
            _tag: 'Response' as const,
            response: noStoreJsonResponse(
              { error: 'unauthorized' },
              { status: 401 },
            ),
          }
        : { _tag: 'Creator' as const, actor },
    ),
  )

const resolveCreateActor = <Session extends ShareSession>(
  request: Request,
  env: ShareRouteEnv,
  ctx: ExecutionContext,
  body: Record<string, unknown>,
  dependencies: ShareRouteDependencies<Session>,
): Effect.Effect<
  ShareCreatorResolution<Session>,
  ShareProjectionStorageError
> =>
  Effect.tryPromise({
    try: async () => {
      const session = await dependencies.requireBrowserSession(
        request,
        env,
        ctx,
      )

      if (session !== undefined) {
        return {
          _tag: 'Creator' as const,
          actor: {
            _tag: 'Browser' as const,
            session,
            viewer: viewerFromSession(session, dependencies),
          },
        }
      }

      if (await dependencies.requireAdminApiToken(request, env)) {
        const selector = targetSelectorFromShareBody(body)

        if (!hasExplicitTargetSelector(selector)) {
          return {
            _tag: 'Response' as const,
            response: noStoreJsonResponse(
              {
                error: 'bad_request',
                reason:
                  'admin share creation requires email, login, githubLogin, or userId',
              },
              { status: 400 },
            ),
          }
        }

        const targetUser = await dependencies.readSelectedOperatorTargetUser(
          openAgentsDatabase(env),
          selector,
        )

        if (targetUser === undefined) {
          return {
            _tag: 'Response' as const,
            response: noStoreJsonResponse(
              { error: 'target_user_not_found' },
              { status: 404 },
            ),
          }
        }

        return {
          _tag: 'Creator' as const,
          actor: {
            _tag: 'Api' as const,
            viewer: viewerFromOperatorTargetUser(targetUser, dependencies),
          },
        }
      }

      const actor = await dependencies.authenticateRequestActor(
        request,
        env,
        ctx,
      )

      return actor === undefined
        ? {
            _tag: 'Response' as const,
            response: noStoreJsonResponse(
              { error: 'unauthorized' },
              { status: 401 },
            ),
          }
        : {
            _tag: 'Creator' as const,
            actor: {
              _tag: 'Api' as const,
              viewer: viewerFromAuthenticatedActor(actor, dependencies),
            },
          }
    },
    catch: error =>
      new ShareProjectionStorageError({
        error,
        operation: 'share.create_actor',
      }),
  })

const shareLayer = (env: ShareRouteEnv, origin: string) =>
  Layer.mergeAll(
    ShareProjectionRepository.layer(openAgentsDatabase(env)),
    ShareUrlService.layer(origin),
    ShareReceiptService.layer,
    ShareAccessService.layer,
    ShareProjectionBuilder.layer,
  )

const loadSourceBundle = (
  env: ShareRouteEnv,
  viewer: ShareViewer,
  source: Parameters<typeof sourceTeamId>[0],
): Effect.Effect<SourceBundle, ShareProjectionError> =>
  source.kind === 'agent-run'
    ? Effect.tryPromise({
        try: () =>
          requireAuthorizedAgentRunBundle(env, viewer.userId, source.id),
        catch: error =>
          error instanceof Error && error.name.includes('RouteAccess')
            ? new ShareProjectionForbidden({ shareId: source.id })
            : new ShareProjectionStorageError({
                error,
                operation: 'share.source.agent_run',
              }),
      }).pipe(
        Effect.map(bundle => ({
          _tag: 'AgentRun' as const,
          bundle,
          ownerUserId: bundle.run.userId,
          projectId: bundle.run.projectId,
          teamId: bundle.run.teamId,
        })),
      )
    : Effect.tryPromise({
        try: () => {
          const teamId = sourceTeamId(source)
          const projectId = sourceProjectId(source)

          if (teamId === null) {
            return Promise.reject(
              new ShareProjectionMalformed({
                reason: 'team source is missing teamId',
              }),
            )
          }

          return listTeamChatMessages(
            openAgentsDatabase(env),
            teamId,
            120,
            undefined,
            undefined,
            projectId,
          ).then(messages => ({
            _tag: 'TeamThread' as const,
            messages,
            ownerUserId: viewer.userId,
            projectId,
            teamId,
          }))
        },
        catch: error =>
          error instanceof ShareProjectionMalformed
            ? error
            : new ShareProjectionStorageError({
                error,
                operation: 'share.source.team_thread',
              }),
      })

const handleCreateShare = <Session extends ShareSession>(
  request: Request,
  env: ShareRouteEnv,
  ctx: ExecutionContext,
  dependencies: ShareRouteDependencies<Session>,
): ShareRouteEffect => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }

  return Effect.gen(function* () {
    const body = yield* Effect.tryPromise({
      try: () => readJsonObject(request),
      catch: error =>
        new ShareProjectionStorageError({
          error,
          operation: 'share.request_json',
        }),
    })
    const actorResolution = yield* resolveCreateActor(
      request,
      env,
      ctx,
      body,
      dependencies,
    )

    if (actorResolution._tag === 'Response') {
      return actorResolution.response
    }

    const { actor } = actorResolution
    const { viewer } = actor
    const create = yield* decodeShareCreateRequest(body)
    const source = yield* loadSourceBundle(env, viewer, create.source)
    const access = yield* ShareAccessService

    yield* access.authorizeCreate({
      audience: create.audience,
      db: openAgentsDatabase(env),
      sourceTeamId: source.teamId,
      viewer,
    })

    const repository = yield* ShareProjectionRepository
    const urlService = yield* ShareUrlService
    const receipts = yield* ShareReceiptService
    const builder = yield* ShareProjectionBuilder
    const shareId = makeShareId()
    const now = currentIsoTimestamp()
    const canonicalUrl = urlService.canonicalUrlForShareId(shareId)
    const receiptRefs = [receipts.createdRef(shareId)]
    const projection = yield* builder.build(
      source._tag === 'AgentRun'
        ? {
            _tag: 'AgentRun',
            audience: create.audience,
            bundle: source.bundle,
            canonicalUrl,
            createdAt: now,
            receiptRefs,
            shareId,
            ...(create.expiresAt === undefined
              ? {}
              : { expiresAt: create.expiresAt }),
            ...(create.title === undefined ? {} : { title: create.title }),
          }
        : {
            _tag: 'TeamThread',
            audience: create.audience,
            canonicalUrl,
            createdAt: now,
            messages: source.messages,
            projectId: source.projectId,
            receiptRefs,
            shareId,
            teamId: source.teamId,
            ...(create.title === undefined ? {} : { title: create.title }),
          },
    )
    const record = yield* repository.create({
      audience: create.audience,
      canonicalUrl,
      ownerUserId: source.ownerUserId,
      projectId: source.projectId,
      projection,
      redactionPolicyId:
        create.redactionPolicyId?.trim() === ''
          ? 'default'
          : (create.redactionPolicyId ?? 'default'),
      shareId,
      source: projection.source,
      teamId: source.teamId,
      title: projection.title,
      expiresAt: create.expiresAt ?? null,
    })
    const response = noStoreJsonResponse(
      {
        id: record.id,
        url: record.canonicalUrl,
        audienceLabel: record.projection.audienceLabel,
        status: record.projection.status,
      },
      { status: 201 },
    )

    return appendActorCookies(response, actor, dependencies)
  }).pipe(
    Effect.provide(shareLayer(env, dependencies.appOrigin(env))),
    Effect.catch(error => Effect.succeed(errorResponse(error))),
  )
}

const handleReadShare = <Session extends ShareSession>(
  request: Request,
  env: ShareRouteEnv,
  ctx: ExecutionContext,
  shareId: string,
  dependencies: ShareRouteDependencies<Session>,
): ShareRouteEffect => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.gen(function* () {
    const repository = yield* ShareProjectionRepository
    const access = yield* ShareAccessService
    const record = yield* repository.readById(shareId)

    if (record === undefined) {
      return yield* new ShareProjectionNotFound({ shareId })
    }

    if (record.status === 'revoked' || record.revokedAt !== null) {
      return noStoreJsonResponse({ error: 'share_revoked' }, { status: 410 })
    }

    if (
      record.expiresAt !== null &&
      Date.parse(record.expiresAt) <= currentEpochMillis()
    ) {
      return noStoreJsonResponse({ error: 'share_expired' }, { status: 410 })
    }

    const actor = yield* resolveOptionalActor(request, env, ctx, dependencies)
    const projection = yield* access.authorizeView(
      actor === undefined
        ? {
            db: openAgentsDatabase(env),
            record,
          }
        : {
            db: openAgentsDatabase(env),
            record,
            viewer: actor.viewer,
          },
    )
    const response = noStoreJsonResponse({ projection })

    return actor === undefined
      ? response
      : appendActorCookies(response, actor, dependencies)
  }).pipe(
    Effect.provide(shareLayer(env, dependencies.appOrigin(env))),
    Effect.catch(error => Effect.succeed(errorResponse(error))),
  )
}

const handlePatchShare = <Session extends ShareSession>(
  request: Request,
  env: ShareRouteEnv,
  ctx: ExecutionContext,
  shareId: string,
  dependencies: ShareRouteDependencies<Session>,
): ShareRouteEffect => {
  if (request.method !== 'PATCH') {
    return Effect.succeed(methodNotAllowed(['PATCH']))
  }

  return Effect.gen(function* () {
    const actorResolution = yield* resolveRequiredActor(
      request,
      env,
      ctx,
      dependencies,
    )

    if (actorResolution._tag === 'Response') {
      return actorResolution.response
    }

    const { actor } = actorResolution
    const repository = yield* ShareProjectionRepository
    const access = yield* ShareAccessService
    const receipts = yield* ShareReceiptService
    const current = yield* repository.readById(shareId)

    if (current === undefined) {
      return yield* new ShareProjectionNotFound({ shareId })
    }

    const { viewer } = actor
    yield* access.authorizeManage({
      db: openAgentsDatabase(env),
      record: current,
      viewer,
    })

    const body = yield* Effect.tryPromise({
      try: () => readJsonObject(request),
      catch: error =>
        new ShareProjectionStorageError({
          error,
          operation: 'share.patch_json',
        }),
    })
    const update = yield* decodeShareUpdateRequest(body)
    const now = currentIsoTimestamp()
    const nextAudience = update.audience ?? current.audience
    const nextProjection = addReceiptRef(
      {
        ...current.projection,
        audience: nextAudience,
        audienceLabel: audienceLabel(nextAudience, viewer),
        title: update.title ?? current.projection.title,
      },
      receipts.audienceChangedRef(shareId),
      now,
    )
    const updated = yield* repository.update(shareId, {
      audience: nextAudience,
      projection: {
        ...nextProjection,
        audienceLabel: audienceLabel(nextAudience, viewer),
      },
      updatedAt: now,
      ...(Object.hasOwn(update, 'expiresAt')
        ? { expiresAt: update.expiresAt }
        : {}),
      ...(update.title === undefined ? {} : { title: update.title }),
    })

    if (updated === undefined) {
      return yield* new ShareProjectionNotFound({ shareId })
    }

    return appendActorCookies(
      noStoreJsonResponse({
        id: updated.id,
        url: updated.canonicalUrl,
        audienceLabel: updated.projection.audienceLabel,
        status: updated.projection.status,
      }),
      actor,
      dependencies,
    )
  }).pipe(
    Effect.provide(shareLayer(env, dependencies.appOrigin(env))),
    Effect.catch(error => Effect.succeed(errorResponse(error))),
  )
}

const handleDeleteShare = <Session extends ShareSession>(
  request: Request,
  env: ShareRouteEnv,
  ctx: ExecutionContext,
  shareId: string,
  dependencies: ShareRouteDependencies<Session>,
): ShareRouteEffect => {
  if (request.method !== 'DELETE') {
    return Effect.succeed(methodNotAllowed(['DELETE']))
  }

  return Effect.gen(function* () {
    const actorResolution = yield* resolveRequiredActor(
      request,
      env,
      ctx,
      dependencies,
    )

    if (actorResolution._tag === 'Response') {
      return actorResolution.response
    }

    const { actor } = actorResolution
    const repository = yield* ShareProjectionRepository
    const access = yield* ShareAccessService
    const current = yield* repository.readById(shareId)

    if (current === undefined) {
      return yield* new ShareProjectionNotFound({ shareId })
    }

    yield* access.authorizeManage({
      db: openAgentsDatabase(env),
      record: current,
      viewer: actor.viewer,
    })

    const revoked = yield* repository.revoke(shareId, currentIsoTimestamp())

    if (revoked === undefined) {
      return yield* new ShareProjectionNotFound({ shareId })
    }

    return appendActorCookies(
      noStoreJsonResponse({
        id: revoked.id,
        status: 'revoked',
        url: revoked.canonicalUrl,
      }),
      actor,
      dependencies,
    )
  }).pipe(
    Effect.provide(shareLayer(env, dependencies.appOrigin(env))),
    Effect.catch(error => Effect.succeed(errorResponse(error))),
  )
}

export const makeShareRoutes = <Session extends ShareSession>(
  dependencies: ShareRouteDependencies<Session>,
) => ({
  routeShareRequest: (
    request: Request,
    env: ShareRouteEnv,
    ctx: ExecutionContext,
  ): ShareRouteEffect | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/share') {
      return handleCreateShare(request, env, ctx, dependencies)
    }

    const dataMatch = /^\/api\/share\/([^/]+)\/v1\/data$/.exec(url.pathname)

    if (dataMatch !== null) {
      const shareId = optionalShareUuid(dataMatch[1])

      return shareId === undefined
        ? Effect.succeed(
            noStoreJsonResponse(
              { error: 'bad_request', reason: 'share id is malformed' },
              { status: 400 },
            ),
          )
        : handleReadShare(request, env, ctx, shareId, dependencies)
    }

    const mutationMatch = /^\/api\/share\/([^/]+)$/.exec(url.pathname)

    if (mutationMatch !== null) {
      const shareId = optionalShareUuid(mutationMatch[1])

      if (shareId === undefined) {
        return Effect.succeed(
          noStoreJsonResponse(
            { error: 'bad_request', reason: 'share id is malformed' },
            { status: 400 },
          ),
        )
      }

      if (request.method === 'PATCH') {
        return handlePatchShare(request, env, ctx, shareId, dependencies)
      }

      if (request.method === 'DELETE') {
        return handleDeleteShare(request, env, ctx, shareId, dependencies)
      }

      return Effect.succeed(methodNotAllowed(['PATCH', 'DELETE']))
    }

    return undefined
  },
})
