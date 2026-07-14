import {
  type QaSwarmRunProjection,
  assertResolverBackedQaSwarmProjection,
} from '@openagentsinc/qa-swarm-contract'
import { Data, Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'

export const QA_SWARM_PROJECTION_OPERATOR_PREFIX =
  '/api/operator/qa-swarm/runs/' as const
export const QA_SWARM_PROJECTION_PUBLIC_PREFIX =
  '/api/public/qa-swarm/runs/' as const

const runRefPattern = /^qa-run\.[a-z0-9][a-z0-9._-]{0,183}$/i
const privateMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|customer[_-]?(email|phone|prompt|record)|gh[op]_[A-Za-z0-9_]+|invoice[_-]?(id|raw)|macaroon|mnemonic|oauth|payment[_-]?(hash|invoice|preimage|secret)|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(credential|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|log|payment|payload|prompt|provider|runner|source|state|telemetry|trace)|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(key|material|mnemonic|secret|seed))/i

export type QaSwarmProjectionStore = Readonly<{
  read: (
    runRef: string,
  ) => Effect.Effect<QaSwarmRunProjection | null, QaSwarmProjectionStoreUnavailable>
  write: (
    projection: QaSwarmRunProjection,
  ) => Effect.Effect<void, QaSwarmProjectionStoreUnavailable>
}>

export class QaSwarmProjectionStoreUnavailable extends Data.TaggedError(
  'QaSwarmProjectionStoreUnavailable',
)<{ readonly reason: string }> {}

const objectKey = (runRef: string): string =>
  `public/qa-swarm/run-projections/${encodeURIComponent(runRef)}.json`

export const makeArtifactQaSwarmProjectionStore = (
  bucket: R2Bucket,
): QaSwarmProjectionStore => ({
  read: runRef =>
    Effect.tryPromise({
      try: async () => {
        const object = await bucket.get(objectKey(runRef))
        if (object === null) return null
        return assertPublicQaSwarmProjection(JSON.parse(await object.text()))
      },
      catch: error =>
        new QaSwarmProjectionStoreUnavailable({ reason: String(error) }),
    }),
  write: projection =>
    Effect.tryPromise({
      try: async () => {
        await bucket.put(objectKey(projection.runRef), JSON.stringify(projection), {
          httpMetadata: { contentType: 'application/json' },
        })
      },
      catch: error =>
        new QaSwarmProjectionStoreUnavailable({ reason: String(error) }),
    }),
})

export const assertPublicQaSwarmProjection = (
  value: unknown,
): QaSwarmRunProjection => {
  const projection = assertResolverBackedQaSwarmProjection(value)
  if (!runRefPattern.test(projection.runRef)) {
    throw new Error('QA Swarm projection has an invalid runRef')
  }
  if (privateMaterialPattern.test(JSON.stringify(projection))) {
    throw new Error('QA Swarm projection contains private material')
  }
  return projection
}

const runRefFromPath = (pathname: string, prefix: string): string | null => {
  if (!pathname.startsWith(prefix)) return null
  const encoded = pathname.slice(prefix.length)
  if (encoded === '' || encoded.includes('/')) return null
  try {
    const decoded = decodeURIComponent(encoded)
    return runRefPattern.test(decoded) ? decoded : null
  } catch {
    return null
  }
}

const readProjection = (
  request: Request,
  runRef: string,
  store: QaSwarmProjectionStore,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') return Effect.succeed(methodNotAllowed(['GET']))
  return store.read(runRef).pipe(
    Effect.map(projection =>
      projection === null
        ? noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
        : noStoreJsonResponse({ projection }),
    ),
    Effect.catchTag('QaSwarmProjectionStoreUnavailable', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'qa_swarm_projection_unavailable' },
          { status: 503 },
        ),
      ),
    ),
  )
}

const publishProjection = (
  request: Request,
  runRef: string,
  store: QaSwarmProjectionStore,
  requireAdminApiToken: (request: Request) => Promise<boolean>,
): Effect.Effect<Response> => {
  if (request.method !== 'PUT') return Effect.succeed(methodNotAllowed(['PUT']))
  return Effect.gen(function* () {
    if (!(yield* Effect.promise(() => requireAdminApiToken(request)))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }
    const bodyText = yield* Effect.tryPromise(() => request.text())
    const unknown = parseJsonUnknown(bodyText)
    const projection = yield* S.decodeUnknownEffect(S.Unknown)(unknown).pipe(
      Effect.flatMap(value => Effect.try(() => assertPublicQaSwarmProjection(value))),
      Effect.catch(() => Effect.succeed(null)),
    )
    if (projection === null || projection.runRef !== runRef) {
      return noStoreJsonResponse(
        { error: 'invalid_qa_swarm_projection' },
        { status: 422 },
      )
    }
    yield* store.write(projection)
    return noStoreJsonResponse({
      ok: true,
      projection,
      shareUrl: `/qa/${encodeURIComponent(runRef)}`,
    })
  }).pipe(
    Effect.catchTag('QaSwarmProjectionStoreUnavailable', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'qa_swarm_projection_unavailable' },
          { status: 503 },
        ),
      ),
    ),
    Effect.catch(() =>
      Effect.succeed(noStoreJsonResponse({ error: 'bad_request' }, { status: 400 })),
    ),
  )
}

export const makeQaSwarmProjectionRoutes = <Env>(input: Readonly<{
  makeStore: (env: Env) => QaSwarmProjectionStore
  requireAdminApiToken: (request: Request, env: Env) => Promise<boolean>
}>) => ({
  routeQaSwarmProjectionRequest: (
    request: Request,
    env: Env,
    _ctx: ExecutionContext,
  ): Effect.Effect<Response> | undefined => {
    const pathname = new URL(request.url).pathname
    const publicRunRef = runRefFromPath(pathname, QA_SWARM_PROJECTION_PUBLIC_PREFIX)
    if (publicRunRef !== null) {
      return readProjection(request, publicRunRef, input.makeStore(env))
    }
    const operatorRunRef = runRefFromPath(
      pathname,
      QA_SWARM_PROJECTION_OPERATOR_PREFIX,
    )
    if (operatorRunRef !== null) {
      return publishProjection(
        request,
        operatorRunRef,
        input.makeStore(env),
        authRequest => input.requireAdminApiToken(authRequest, env),
      )
    }
    return undefined
  },
})
