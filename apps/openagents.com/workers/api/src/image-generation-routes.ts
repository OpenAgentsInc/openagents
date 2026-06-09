import { Effect, Match as M } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import {
  type ImageGenerationError,
  ImageGenerationService,
  decodeGenerateImageRequest,
} from './image-generation'
import { readJsonObject } from './json-boundary'
import { logWorkerRouteError, logWorkerRouteWarning } from './observability'

type ImageGenerationRouteSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type ImageGenerationRouteEnv = Readonly<{
  ARTIFACTS: R2Bucket
  GEMINI_API_KEY?: string
}>

type ImageGenerationRouteDependencies<
  Session extends ImageGenerationRouteSession,
  Bindings extends ImageGenerationRouteEnv,
> = Readonly<{
  appUrl: (env: Bindings) => string | undefined
  appendRefreshedSessionCookies: (
    response: globalThis.Response,
    session: Session,
  ) => globalThis.Response
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  requireOperatorAccess: (env: Bindings, session: Session) => Promise<boolean>
}>

const runImageGenerationRoute = (
  env: ImageGenerationRouteEnv,
  appUrl: string | undefined,
  effect: Effect.Effect<globalThis.Response, never, ImageGenerationService>,
) =>
  effect.pipe(
    Effect.provide(
      ImageGenerationService.layer({
        ...env,
        ...(appUrl === undefined ? {} : { appUrl }),
      }),
    ),
  )

const imageGenerationErrorResponse = (error: ImageGenerationError) =>
  M.value(error).pipe(
    M.tagsExhaustive({
      ImageGenerationInvalidRequest: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'invalid_image_generation_request', reason },
          { status: 400 },
        ),
      ProviderAuthFailed: () =>
        noStoreJsonResponse(
          { error: 'image_provider_auth_failed' },
          { status: 503 },
        ),
      ProviderInvalidRequest: () =>
        noStoreJsonResponse(
          { error: 'image_provider_invalid_request' },
          { status: 400 },
        ),
      ProviderNoImageReturned: () =>
        noStoreJsonResponse(
          { error: 'image_provider_no_image_returned' },
          { status: 502 },
        ),
      ProviderRateLimited: () =>
        noStoreJsonResponse(
          { error: 'image_provider_rate_limited' },
          { status: 429 },
        ),
      ProviderRejectedPrompt: () =>
        noStoreJsonResponse(
          { error: 'image_provider_rejected_prompt' },
          { status: 422 },
        ),
      ProviderUnavailable: () =>
        noStoreJsonResponse(
          { error: 'image_provider_unavailable' },
          { status: 502 },
        ),
      StorageFailed: () =>
        noStoreJsonResponse({ error: 'image_storage_failed' }, { status: 500 }),
      UnknownImageGenerationError: () =>
        noStoreJsonResponse(
          { error: 'image_generation_failed' },
          { status: 500 },
        ),
    }),
  )

const imageGenerationErrorFields = (error: ImageGenerationError) =>
  M.value(error).pipe(
    M.tagsExhaustive({
      ImageGenerationInvalidRequest: ({ reason }) => ({
        reason,
        tag: 'ImageGenerationInvalidRequest',
      }),
      ProviderAuthFailed: ({ status }) => ({
        status,
        tag: 'ProviderAuthFailed',
      }),
      ProviderInvalidRequest: ({ status }) => ({
        status,
        tag: 'ProviderInvalidRequest',
      }),
      ProviderNoImageReturned: () => ({
        tag: 'ProviderNoImageReturned',
      }),
      ProviderRateLimited: ({ status }) => ({
        status,
        tag: 'ProviderRateLimited',
      }),
      ProviderRejectedPrompt: ({ status }) => ({
        status,
        tag: 'ProviderRejectedPrompt',
      }),
      ProviderUnavailable: ({ status }) => ({
        status,
        tag: 'ProviderUnavailable',
      }),
      StorageFailed: () => ({
        tag: 'StorageFailed',
      }),
      UnknownImageGenerationError: () => ({
        tag: 'UnknownImageGenerationError',
      }),
    }),
  )

const logImageGenerationFailure = (error: ImageGenerationError): void =>
  M.value(error).pipe(
    M.tagsExhaustive({
      ImageGenerationInvalidRequest: () =>
        logWorkerRouteWarning(
          'image_generation_failed',
          imageGenerationErrorFields(error),
        ),
      ProviderAuthFailed: () =>
        logWorkerRouteWarning(
          'image_generation_failed',
          imageGenerationErrorFields(error),
        ),
      ProviderInvalidRequest: () =>
        logWorkerRouteWarning(
          'image_generation_failed',
          imageGenerationErrorFields(error),
        ),
      ProviderNoImageReturned: () =>
        logWorkerRouteWarning(
          'image_generation_failed',
          imageGenerationErrorFields(error),
        ),
      ProviderRateLimited: () =>
        logWorkerRouteWarning(
          'image_generation_failed',
          imageGenerationErrorFields(error),
        ),
      ProviderRejectedPrompt: () =>
        logWorkerRouteWarning(
          'image_generation_failed',
          imageGenerationErrorFields(error),
        ),
      ProviderUnavailable: () =>
        logWorkerRouteWarning(
          'image_generation_failed',
          imageGenerationErrorFields(error),
        ),
      StorageFailed: ({ error: cause }) =>
        logWorkerRouteError(
          'image_generation_failed',
          cause,
          imageGenerationErrorFields(error),
        ),
      UnknownImageGenerationError: ({ error: cause }) =>
        logWorkerRouteError(
          'image_generation_failed',
          cause,
          imageGenerationErrorFields(error),
        ),
    }),
  )

const requireOperatorAccess = <
  Session extends ImageGenerationRouteSession,
  Bindings extends ImageGenerationRouteEnv,
>(
  dependencies: ImageGenerationRouteDependencies<Session, Bindings>,
  env: Bindings,
  session: Session,
) =>
  Effect.tryPromise({
    catch: () => false,
    try: () => dependencies.requireOperatorAccess(env, session),
  }).pipe(Effect.catch(() => Effect.succeed(false)))

const decodeImageKeyPath = (pathname: string): string | undefined => {
  const match = /^\/api\/images\/(.+)$/.exec(pathname)
  const encodedKey = match?.[1]

  if (encodedKey === undefined || encodedKey.trim() === '') {
    return undefined
  }

  try {
    return decodeURIComponent(encodedKey)
  } catch {
    return undefined
  }
}

export const makeImageGenerationRoutes = <
  Session extends ImageGenerationRouteSession,
  Bindings extends ImageGenerationRouteEnv,
>(
  dependencies: ImageGenerationRouteDependencies<Session, Bindings>,
) => ({
  routeImageGenerationRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => {
    const url = new URL(request.url)

    if (url.pathname === '/api/images/generate') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return runImageGenerationRoute(
        env,
        dependencies.appUrl(env),
        Effect.gen(function* () {
          const session = yield* Effect.promise(() =>
            dependencies
              .requireBrowserSession(request, env, ctx)
              .catch(() => undefined),
          )

          if (session === undefined) {
            return noStoreJsonResponse(
              { error: 'unauthorized' },
              { status: 401 },
            )
          }

          const hasOperatorAccess = yield* requireOperatorAccess(
            dependencies,
            env,
            session,
          )

          if (!hasOperatorAccess) {
            return forbidden()
          }

          const body = yield* Effect.promise(() =>
            readJsonObject(request).catch(() => undefined),
          )

          if (body === undefined) {
            return noStoreJsonResponse(
              {
                error: 'invalid_image_generation_request',
                reason: 'invalid_json',
              },
              { status: 400 },
            )
          }

          const input = yield* Effect.sync(() => {
            try {
              return decodeGenerateImageRequest(body)
            } catch {
              return undefined
            }
          })

          if (input === undefined) {
            return noStoreJsonResponse(
              {
                error: 'invalid_image_generation_request',
                reason: 'schema',
              },
              { status: 400 },
            )
          }

          const service = yield* ImageGenerationService
          const response = yield* service.generate(input).pipe(
            Effect.match({
              onFailure: error => {
                logImageGenerationFailure(error)

                return imageGenerationErrorResponse(error)
              },
              onSuccess: payload => noStoreJsonResponse(payload),
            }),
          )

          return dependencies.appendRefreshedSessionCookies(response, session)
        }),
      )
    }

    const imageKey = decodeImageKeyPath(url.pathname)

    if (imageKey === undefined) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    return Effect.gen(function* () {
      const session = yield* Effect.promise(() =>
        dependencies
          .requireBrowserSession(request, env, ctx)
          .catch(() => undefined),
      )

      if (session === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const hasOperatorAccess = yield* requireOperatorAccess(
        dependencies,
        env,
        session,
      )

      if (!hasOperatorAccess) {
        return forbidden()
      }

      if (!imageKey.startsWith('generated-images/')) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      const object = yield* Effect.promise(() =>
        env.ARTIFACTS.get(imageKey).catch(() => undefined),
      )

      if (object === undefined) {
        return serverError()
      }

      if (object === null) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      const headers = new Headers()
      object.writeHttpMetadata(headers)
      headers.set('cache-control', 'private, max-age=3600')
      headers.set('etag', object.httpEtag)

      return dependencies.appendRefreshedSessionCookies(
        new Response(object.body, { headers }),
        session,
      )
    })
  },
})
