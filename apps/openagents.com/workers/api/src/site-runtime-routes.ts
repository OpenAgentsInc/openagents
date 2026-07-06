import { Effect } from 'effect'

import { methodNotAllowed, redirectResponse } from './http/responses'
import {
  SiteRuntimeService,
  type SiteRuntimeStaticAsset,
  SiteRuntimeStorageError,
  type SiteRuntimeWorkerDeployment,
} from './site-runtime'
import {
  TenantCustomHostnameStorageError,
  type TenantRef,
} from './tenant-custom-hostnames'

type SiteRuntimeRouteEnv = Readonly<{
  // Optional since #8516 (account-level R2 disabled); site-runtime resolves
  // the bucket through `artifactsBucketForEnv`.
  ARTIFACTS?: R2Bucket | undefined
  OPENAGENTS_DB: D1Database
  SITES_DISPATCH: DispatchNamespace
}>
type HttpResponse = globalThis.Response

type SiteRuntimeRoutesConfig = Readonly<{
  reservedHosts?: ReadonlySet<string>
  resolveCustomHostname?: (
    hostname: string,
    env: SiteRuntimeRouteEnv,
  ) => Effect.Effect<
    TenantRef | null,
    SiteRuntimeStorageError | TenantCustomHostnameStorageError
  >
  sitesHost: string
}>

type ParsedSiteRuntimePath = Readonly<{
  candidatePaths: ReadonlyArray<string>
  dispatchPath: string
  slug: string
  versionId: string | null
}>

const SITE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{0,62}$/
const SITE_VERSION_ID_PATTERN = /^site_version_[A-Za-z0-9_-]{1,160}$/

const publicSiteNotFound = (): HttpResponse =>
  new Response('Not found', {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/plain; charset=utf-8',
    },
    status: 404,
  })

const publicSiteServerError = (): HttpResponse =>
  new Response('Site unavailable', {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/plain; charset=utf-8',
    },
    status: 503,
  })

const hasUnsafePathSegment = (segments: ReadonlyArray<string>): boolean =>
  segments.some(segment => segment === '..' || segment.includes('\\'))

const pathLooksLikeFile = (assetPath: string): boolean =>
  /(?:^|\/)[^/]+\.[^/]+$/.test(assetPath)

const candidateAssetPaths = (
  restPath: string,
  pathname: string,
): ReadonlyArray<string> => {
  if (restPath === '') {
    return ['index.html']
  }

  if (pathname.endsWith('/')) {
    return [`${restPath}/index.html`]
  }

  return pathLooksLikeFile(restPath)
    ? [restPath]
    : [restPath, `${restPath}/index.html`]
}

const parseSiteRuntimePath = (url: URL): ParsedSiteRuntimePath | null => {
  const segments = url.pathname.split('/').filter(segment => segment !== '')
  const slug = segments[0]

  if (
    slug === undefined ||
    !SITE_SLUG_PATTERN.test(slug) ||
    hasUnsafePathSegment(segments)
  ) {
    return null
  }

  if (segments[1] === 'versions') {
    const versionId = segments[2]

    if (versionId === undefined || !SITE_VERSION_ID_PATTERN.test(versionId)) {
      return null
    }

    const restPath = segments.slice(3).join('/')
    const versionPrefix = `/${slug}/versions/${versionId}`
    const dispatchPath =
      url.pathname.slice(versionPrefix.length) === ''
        ? '/'
        : url.pathname.slice(versionPrefix.length)

    return {
      candidatePaths: candidateAssetPaths(restPath, url.pathname),
      dispatchPath,
      slug,
      versionId,
    }
  }

  return {
    candidatePaths: candidateAssetPaths(
      segments.slice(1).join('/'),
      url.pathname,
    ),
    dispatchPath:
      url.pathname.slice(`/${slug}`.length) === ''
        ? '/'
        : url.pathname.slice(`/${slug}`.length),
    slug,
    versionId: null,
  }
}

const parseCustomHostnameSiteRuntimePath = (
  url: URL,
): Omit<ParsedSiteRuntimePath, 'slug' | 'versionId'> | null => {
  const segments = url.pathname.split('/').filter(segment => segment !== '')

  if (hasUnsafePathSegment(segments)) {
    return null
  }

  return {
    candidatePaths: candidateAssetPaths(segments.join('/'), url.pathname),
    dispatchPath: url.pathname === '' ? '/' : url.pathname,
  }
}

const artifactHeaders = (
  object: R2ObjectBody,
  asset: SiteRuntimeStaticAsset,
): Headers => {
  const headers = new Headers()
  object.writeHttpMetadata(headers)
  headers.set('cache-control', asset.cacheControl ?? 'public, max-age=60')
  headers.set('etag', object.httpEtag)

  if (asset.contentType !== null) {
    headers.set('content-type', asset.contentType)
  }

  return headers
}

const artifactResponse = (
  request: Request,
  object: R2ObjectBody,
  asset: SiteRuntimeStaticAsset,
): HttpResponse =>
  new Response(request.method === 'HEAD' ? null : object.body, {
    headers: artifactHeaders(object, asset),
    status: 200,
  })

const workerDispatchRequest = (
  request: Request,
  dispatchPath: string,
): Request => {
  const url = new URL(request.url)
  url.pathname = dispatchPath

  return new Request(url, request)
}

const workerDispatchResponse = (
  dispatchNamespace: DispatchNamespace,
  deployment: SiteRuntimeWorkerDeployment,
  request: Request,
  dispatchPath: string,
): Effect.Effect<HttpResponse, SiteRuntimeStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new SiteRuntimeStorageError({
        error,
        operation: 'siteRuntime.dispatch.fetch',
      }),
    try: () =>
      dispatchNamespace
        .get(deployment.runtimeScriptName)
        .fetch(workerDispatchRequest(request, dispatchPath)),
  })

const runSiteRuntimeRoute = (
  env: SiteRuntimeRouteEnv,
  effect: Effect.Effect<
    HttpResponse,
    SiteRuntimeStorageError | TenantCustomHostnameStorageError,
    SiteRuntimeService
  >,
): Effect.Effect<HttpResponse> =>
  effect.pipe(
    Effect.provide(SiteRuntimeService.layer(env)),
    Effect.catchTag('SiteRuntimeStorageError', () =>
      Effect.succeed(publicSiteServerError()),
    ),
    Effect.catchTag('TenantCustomHostnameStorageError', () =>
      Effect.succeed(publicSiteServerError()),
    ),
  )

export const makeSiteRuntimeRoutes = (config: SiteRuntimeRoutesConfig) => ({
  routeSiteRuntimeRequest: (
    request: Request,
    env: SiteRuntimeRouteEnv,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (
      url.hostname !== config.sitesHost &&
      (config.resolveCustomHostname === undefined ||
        config.reservedHosts?.has(url.hostname) === true)
    ) {
      return undefined
    }

    if (url.search !== '') {
      return Effect.succeed(redirectResponse(`${url.origin}${url.pathname}`))
    }

    if (url.hostname !== config.sitesHost) {
      const parsed = parseCustomHostnameSiteRuntimePath(url)

      if (parsed === null) {
        return Effect.succeed(publicSiteNotFound())
      }

      return runSiteRuntimeRoute(
        env,
        Effect.gen(function* () {
          const tenant = yield* config.resolveCustomHostname!(url.hostname, env)

          if (tenant === null) {
            return publicSiteNotFound()
          }

          const runtime = yield* SiteRuntimeService
          const target = yield* runtime.resolveRuntimeTargetForTeam(
            tenant.teamId,
            parsed.candidatePaths,
          )

          if (target === null) {
            return publicSiteNotFound()
          }

          if (target._tag === 'worker') {
            return yield* workerDispatchResponse(
              env.SITES_DISPATCH,
              target,
              request,
              parsed.dispatchPath,
            )
          }

          if (request.method !== 'GET' && request.method !== 'HEAD') {
            return methodNotAllowed(['GET', 'HEAD'])
          }

          const object = yield* runtime.readArtifactObject(target)

          return object === null
            ? publicSiteNotFound()
            : artifactResponse(request, object, target)
        }),
      )
    }

    const parsed = parseSiteRuntimePath(url)

    if (parsed === null) {
      return Effect.succeed(publicSiteNotFound())
    }

    return runSiteRuntimeRoute(
      env,
      Effect.gen(function* () {
        const runtime = yield* SiteRuntimeService
        const target =
          parsed.versionId === null
            ? yield* runtime.resolveRuntimeTarget(
                parsed.slug,
                parsed.candidatePaths,
              )
            : yield* runtime.resolveVersionRuntimeTarget(
                parsed.slug,
                parsed.versionId,
                parsed.candidatePaths,
              )

        if (target === null) {
          return publicSiteNotFound()
        }

        if (target._tag === 'worker') {
          return yield* workerDispatchResponse(
            env.SITES_DISPATCH,
            target,
            request,
            parsed.dispatchPath,
          )
        }

        if (request.method !== 'GET' && request.method !== 'HEAD') {
          return methodNotAllowed(['GET', 'HEAD'])
        }

        const object = yield* runtime.readArtifactObject(target)

        return object === null
          ? publicSiteNotFound()
          : artifactResponse(request, object, target)
      }),
    )
  },
})
