import { notFound } from '@openagentsinc/sync-worker'
import { Effect, Layer, Option } from 'effect'
import { WorkerEnvironment } from 'effect-cf'

import { artifactsBucketForEnv } from './artifacts-binding'

import { type OpenAgentsWorkerEnv, ThreadFileArtifacts } from './bindings'
import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  optionalBoolean,
  optionalString,
  readJsonObject,
} from './json-boundary'
import { khalaCodeProductStateDatabaseForEnv } from './khala-code-product-state-store'
import { openAgentsDatabase } from './runtime'
import { randomUuid } from './runtime-primitives'
import {
  type PublicThreadFile,
  type ReadActiveTeamMembershipRole,
  ThreadFileRepository,
  type ThreadFileRepositoryError,
  type ThreadFileScope,
  authorizeThreadFileManage,
  authorizeThreadFileRead,
  sha256Hex,
  threadFileObjectKey,
} from './thread-files'

const MAX_THREAD_FILE_BYTES = 50 * 1024 * 1024

type SyncNotificationContext = Pick<ExecutionContext, 'waitUntil'>

type BrowserSessionShape = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type ThreadFileRouteDependencies<Session extends BrowserSessionShape> =
  Readonly<{
    appendRefreshedSessionCookies: (
      response: Response,
      session: Session,
    ) => Response
    publishTeamThreadFileSync: (
      env: Pick<OpenAgentsWorkerEnv, 'OPENAGENTS_DB' | 'SYNC_ROOM'>,
      ctx: SyncNotificationContext,
      file: PublicThreadFile,
      actorId: string,
    ) => Promise<void>
    makeThreadFileId?: () => string
    readActiveTeamMembershipRole: ReadActiveTeamMembershipRole
    requireBrowserSession: (
      request: Request,
      env: OpenAgentsWorkerEnv,
      ctx: ExecutionContext,
    ) => Promise<Session | undefined>
  }>

export const makeThreadFileRoutes = <Session extends BrowserSessionShape>(
  dependencies: ThreadFileRouteDependencies<Session>,
) => {
  const makeThreadFileId = dependencies.makeThreadFileId ?? randomUuid

  const threadFileStorageLayer = (env: OpenAgentsWorkerEnv) => {
    // CFG-8 (#8523): the effect-cf R2 tag reads `env.ARTIFACTS` directly,
    // so hand it an env whose ARTIFACTS slot is already resolved (GCS
    // adapter when configured, rejecting stub otherwise).
    const workerEnvironmentLayer = Layer.succeed(WorkerEnvironment, {
      ...env,
      ARTIFACTS: artifactsBucketForEnv(env),
    })
    const repositoryLayer = ThreadFileRepository.layer(
      khalaCodeProductStateDatabaseForEnv(env),
    )

    return Layer.mergeAll(
      repositoryLayer,
      ThreadFileArtifacts.layer({ binding: 'ARTIFACTS' }),
    ).pipe(Layer.provide(workerEnvironmentLayer))
  }

  const threadFileRepositoryErrorResponse = (
    error: ThreadFileRepositoryError,
  ): Response =>
    noStoreJsonResponse(
      {
        error: 'thread_file_repository_error',
        message: error.message,
      },
      { status: 500 },
    )

  const handleThreadFilesApi = (
    request: Request,
    env: OpenAgentsWorkerEnv,
    ctx: ExecutionContext,
  ): Effect.Effect<Response> =>
    Effect.gen(function* () {
      if (request.method !== 'GET' && request.method !== 'POST') {
        return methodNotAllowed(['GET', 'POST'])
      }

      const session = yield* Effect.promise(() =>
        dependencies.requireBrowserSession(request, env, ctx),
      )

      if (session === undefined) {
        return unauthorized()
      }

      const repository = yield* ThreadFileRepository

      if (request.method === 'GET') {
        const url = new URL(request.url)
        const teamId = optionalString(url.searchParams.get('teamId'))
        const threadId = optionalString(url.searchParams.get('threadId'))

        if (teamId !== undefined) {
          const role = yield* Effect.promise(() =>
            dependencies.readActiveTeamMembershipRole(
              openAgentsDatabase(env),
              teamId,
              session.user.userId,
            ),
          )

          if (role === undefined) {
            return forbidden()
          }

          const files = yield* repository.listTeam({
            teamId,
            ...(threadId === undefined ? {} : { threadId }),
          })

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({ files }),
            session,
          )
        }

        if (threadId === undefined) {
          return noStoreJsonResponse(
            { error: 'bad_request', reason: 'threadId is required' },
            { status: 400 },
          )
        }

        const files = yield* repository.listPersonal({
          ownerUserId: session.user.userId,
          threadId,
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ files }),
          session,
        )
      }

      const form = yield* Effect.promise(() => request.formData())
      const selectedFile = form.get('file')

      if (!(selectedFile instanceof File)) {
        return noStoreJsonResponse(
          { error: 'bad_request', reason: 'file is required' },
          { status: 400 },
        )
      }

      if (selectedFile.size <= 0) {
        return noStoreJsonResponse(
          { error: 'bad_request', reason: 'file must not be empty' },
          { status: 400 },
        )
      }

      if (selectedFile.size > MAX_THREAD_FILE_BYTES) {
        return noStoreJsonResponse(
          { error: 'bad_request', reason: 'file must be 50 MB or smaller' },
          { status: 400 },
        )
      }

      const teamId = optionalString(form.get('teamId'))
      const scope: ThreadFileScope = teamId === undefined ? 'personal' : 'team'
      const fallbackThreadId =
        teamId === undefined
          ? `personal:${session.user.userId}:chat`
          : `team:${teamId}:chat`
      const threadId = optionalString(form.get('threadId')) ?? fallbackThreadId

      if (teamId !== undefined) {
        const role = yield* Effect.promise(() =>
          dependencies.readActiveTeamMembershipRole(
            openAgentsDatabase(env),
            teamId,
            session.user.userId,
          ),
        )

        if (role === undefined) {
          return forbidden()
        }
      }

      const id = makeThreadFileId()
      const filename = selectedFile.name.trim() || 'upload'
      const contentType =
        selectedFile.type.trim() === ''
          ? 'application/octet-stream'
          : selectedFile.type
      const bytes = yield* Effect.promise(() => selectedFile.arrayBuffer())
      const checksumSha256 = yield* Effect.promise(() => sha256Hex(bytes))
      const artifacts = yield* ThreadFileArtifacts
      const objectKey = threadFileObjectKey({
        filename,
        id,
        ownerUserId: session.user.userId,
        scope,
        teamId: teamId ?? null,
        threadId,
      })

      yield* artifacts.put(objectKey, bytes, {
        customMetadata: {
          ownerUserId: session.user.userId,
          scope,
          threadId,
          ...(teamId === undefined ? {} : { teamId }),
        },
        httpMetadata: { contentType },
      })

      const file = yield* repository.insert({
        checksumSha256,
        contentType,
        filename,
        id,
        objectKey,
        ownerUserId: session.user.userId,
        scope,
        sizeBytes: selectedFile.size,
        teamId: teamId ?? null,
        threadId,
      })

      if (teamId !== undefined) {
        yield* Effect.promise(() =>
          dependencies.publishTeamThreadFileSync(
            env,
            ctx,
            file,
            session.user.userId,
          ),
        )
      }

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({ file }, { status: 201 }),
        session,
      )
    }).pipe(
      Effect.withSpan('ThreadFiles.handleThreadFilesApi'),
      Effect.provide(threadFileStorageLayer(env)),
      Effect.catchTag('ThreadFileRepositoryError', error =>
        Effect.succeed(threadFileRepositoryErrorResponse(error)),
      ),
      Effect.catchTag('R2OperationError', error =>
        Effect.succeed(
          noStoreJsonResponse(
            {
              error: 'thread_file_artifact_error',
              message: `R2 ${error.operation} failed for ${error.binding}`,
            },
            { status: 500 },
          ),
        ),
      ),
    )

  const handleTeamFilesApi = (
    request: Request,
    env: OpenAgentsWorkerEnv,
    ctx: ExecutionContext,
    teamId: string,
  ): Effect.Effect<Response> =>
    Effect.gen(function* () {
      if (request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const session = yield* Effect.promise(() =>
        dependencies.requireBrowserSession(request, env, ctx),
      )

      if (session === undefined) {
        return unauthorized()
      }

      const role = yield* Effect.promise(() =>
        dependencies.readActiveTeamMembershipRole(
          openAgentsDatabase(env),
          teamId,
          session.user.userId,
        ),
      )

      if (role === undefined) {
        return forbidden()
      }

      const repository = yield* ThreadFileRepository
      const files = yield* repository.listTeam({ teamId })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          files,
          teamId,
        }),
        session,
      )
    }).pipe(
      Effect.withSpan('ThreadFiles.handleTeamFilesApi'),
      Effect.provide(threadFileStorageLayer(env)),
      Effect.catchTag('ThreadFileRepositoryError', error =>
        Effect.succeed(threadFileRepositoryErrorResponse(error)),
      ),
    )

  const handleThreadFileApi = (
    request: Request,
    env: OpenAgentsWorkerEnv,
    ctx: ExecutionContext,
    fileId: string,
  ): Effect.Effect<Response> =>
    Effect.gen(function* () {
      if (request.method !== 'GET' && request.method !== 'PATCH') {
        return methodNotAllowed(['GET', 'PATCH'])
      }

      const session = yield* Effect.promise(() =>
        dependencies.requireBrowserSession(request, env, ctx),
      )

      if (session === undefined) {
        return unauthorized()
      }

      const repository = yield* ThreadFileRepository
      const row = yield* repository.readById(fileId)

      if (row === undefined) {
        return notFound()
      }

      const canRead = yield* Effect.promise(() =>
        authorizeThreadFileRead(
          openAgentsDatabase(env),
          row,
          session.user.userId,
          dependencies.readActiveTeamMembershipRole,
        ),
      )

      if (!canRead) {
        return forbidden()
      }

      const url = new URL(request.url)
      const requestedTeamId = optionalString(url.searchParams.get('teamId'))

      if (requestedTeamId !== undefined && row.team_id !== requestedTeamId) {
        return notFound()
      }

      if (request.method === 'PATCH') {
        const canManage = yield* Effect.promise(() =>
          authorizeThreadFileManage(
            openAgentsDatabase(env),
            row,
            session.user.userId,
            dependencies.readActiveTeamMembershipRole,
          ),
        )

        if (!canManage) {
          return forbidden()
        }

        const body = yield* Effect.promise(() =>
          readJsonObject(request).catch((): Record<string, unknown> => ({})),
        )
        const downloadEnabled = optionalBoolean(body.downloadEnabled)

        if (downloadEnabled === undefined) {
          return noStoreJsonResponse(
            { error: 'bad_request', reason: 'downloadEnabled is required' },
            { status: 400 },
          )
        }

        yield* repository.setDownloadEnabled({
          downloadEnabled,
          fileId: row.id,
        })
      }

      const latest = yield* repository.readById(fileId)

      if (latest === undefined) {
        return notFound()
      }

      const detail = yield* repository.readDetail({
        readActiveTeamMembershipRole: dependencies.readActiveTeamMembershipRole,
        row: latest,
        userId: session.user.userId,
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({ detail }),
        session,
      )
    }).pipe(
      Effect.withSpan('ThreadFiles.handleThreadFileApi'),
      Effect.provide(threadFileStorageLayer(env)),
      Effect.catchTag('ThreadFileRepositoryError', error =>
        Effect.succeed(threadFileRepositoryErrorResponse(error)),
      ),
    )

  const handleThreadFileDownloadApi = (
    request: Request,
    env: OpenAgentsWorkerEnv,
    ctx: ExecutionContext,
    fileId: string,
  ): Effect.Effect<Response> =>
    Effect.gen(function* () {
      if (request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const session = yield* Effect.promise(() =>
        dependencies.requireBrowserSession(request, env, ctx),
      )

      if (session === undefined) {
        return unauthorized()
      }

      const repository = yield* ThreadFileRepository
      const row = yield* repository.readById(fileId)

      if (row === undefined) {
        return notFound()
      }

      const canRead = yield* Effect.promise(() =>
        authorizeThreadFileRead(
          openAgentsDatabase(env),
          row,
          session.user.userId,
          dependencies.readActiveTeamMembershipRole,
        ),
      )

      if (!canRead) {
        return forbidden()
      }

      if (row.download_enabled !== 1) {
        return forbidden()
      }

      const artifacts = yield* ThreadFileArtifacts
      const maybeObject = yield* artifacts.get(row.object_key)
      const object = Option.getOrUndefined(maybeObject)

      if (object === undefined) {
        return notFound()
      }

      const headers = new Headers()
      headers.set('cache-control', 'private, max-age=60')
      headers.set('content-type', row.content_type)
      headers.set(
        'content-disposition',
        `attachment; filename="${row.filename.replace(/["\\]/g, '_')}"`,
      )

      return dependencies.appendRefreshedSessionCookies(
        new Response(object.body, { headers }),
        session,
      )
    }).pipe(
      Effect.withSpan('ThreadFiles.handleThreadFileDownloadApi'),
      Effect.provide(threadFileStorageLayer(env)),
      Effect.catchTag('ThreadFileRepositoryError', error =>
        Effect.succeed(threadFileRepositoryErrorResponse(error)),
      ),
      Effect.catchTag('R2OperationError', error =>
        Effect.succeed(
          noStoreJsonResponse(
            {
              error: 'thread_file_artifact_error',
              message: `R2 ${error.operation} failed for ${error.binding}`,
            },
            { status: 500 },
          ),
        ),
      ),
    )

  const routeThreadFileRequest = (
    request: Request,
    env: OpenAgentsWorkerEnv,
    ctx: ExecutionContext,
  ): Effect.Effect<Response> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/thread-files') {
      return handleThreadFilesApi(request, env, ctx)
    }

    const threadFileDetailMatch = /^\/api\/thread-files\/([^/]+)$/.exec(
      url.pathname,
    )

    if (threadFileDetailMatch !== null) {
      const fileId = threadFileDetailMatch[1]

      if (fileId !== undefined) {
        return handleThreadFileApi(request, env, ctx, fileId)
      }
    }

    const threadFileDownloadMatch =
      /^\/api\/thread-files\/([^/]+)\/download$/.exec(url.pathname)

    if (threadFileDownloadMatch !== null) {
      const fileId = threadFileDownloadMatch[1]

      if (fileId !== undefined) {
        return handleThreadFileDownloadApi(request, env, ctx, fileId)
      }
    }

    const teamFilesMatch = /^\/api\/teams\/([^/]+)\/files$/.exec(url.pathname)

    if (teamFilesMatch !== null) {
      const encodedTeamId = teamFilesMatch[1]

      if (encodedTeamId !== undefined) {
        let teamId: string

        try {
          teamId = decodeURIComponent(encodedTeamId)
        } catch {
          return Effect.succeed(
            noStoreJsonResponse(
              { error: 'bad_request', reason: 'teamId is malformed' },
              { status: 400 },
            ),
          )
        }

        return handleTeamFilesApi(request, env, ctx, teamId)
      }
    }

    return undefined
  }

  return {
    handleTeamFilesApi,
    handleThreadFileApi,
    handleThreadFileDownloadApi,
    handleThreadFilesApi,
    routeThreadFileRequest,
  }
}
