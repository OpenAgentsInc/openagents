// Operator-only Harbor full trace archive route (epic #6253).
//
// This is intentionally NOT the public ATIF trace store. A Harbor job directory
// tarball can contain raw prompts, model responses, shell output, local paths,
// endpoint hints, and other private operator material. The route stores bytes in
// the private ARTIFACTS R2 bucket and metadata in D1, behind the admin API token.
// Public `/gym` continues to use only the public-safe run-progress projection.
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { currentIsoTimestamp } from '../../runtime-primitives'
import type {
  HarborFullTraceArchiveRecord,
  HarborFullTraceArchiveStore,
} from './harbor-full-trace-archive-store'

type HttpResponse = globalThis.Response

const MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
const DEFAULT_CONTENT_TYPE = 'application/gzip'
const ALLOWED_CONTENT_TYPES = new Set([
  'application/gzip',
  'application/octet-stream',
  'application/x-gzip',
])
const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,239}$/
const SHA256_PATTERN = /^[a-fA-F0-9]{64}$/

export type HarborFullTraceArchiveRouteInput = Readonly<{
  requireAdminApiToken: (request: Request) => Promise<boolean>
  store?: HarborFullTraceArchiveStore
  nowIso?: () => string
}>

type ParsedUploadHeaders = Readonly<{
  archiveRef: string
  artifactBytes: number
  artifactSha256: string
  captureCompletedAt: string
  captureStartedAt: string | null
  contentType: string
  jobRef: string
  runRef: string
}>

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const badRequest = (reason: string) =>
  noStoreJsonResponse(
    { error: 'harbor_full_trace_archive_rejected', reason },
    { status: 400 },
  )

const unavailable = () =>
  noStoreJsonResponse(
    {
      error: 'harbor_full_trace_archive_unavailable',
      reason: 'No writable Harbor full trace archive store is configured.',
    },
    { status: 503 },
  )

const archiveNotFound = () =>
  noStoreJsonResponse(
    { error: 'harbor_full_trace_archive_not_found' },
    { status: 404 },
  )

const headerValue = (request: Request, name: string): string | undefined => {
  const value = request.headers.get(name)?.trim()
  return value === undefined || value === '' ? undefined : value
}

const parsePositiveInteger = (
  value: string | undefined,
): number | undefined => {
  if (value === undefined || !/^[0-9]+$/.test(value)) {
    return undefined
  }
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined
}

const requireSafeRef = (
  request: Request,
  headerName: string,
): string | Readonly<{ error: string }> => {
  const value = headerValue(request, headerName)
  if (value === undefined) {
    return { error: `Missing ${headerName}.` }
  }
  if (!SAFE_REF_PATTERN.test(value)) {
    return { error: `${headerName} is not a bounded ref token.` }
  }
  return value
}

const optionalIso = (
  request: Request,
  headerName: string,
): string | null | Readonly<{ error: string }> => {
  const value = headerValue(request, headerName)
  if (value === undefined) {
    return null
  }
  if (Number.isNaN(Date.parse(value))) {
    return { error: `${headerName} must be an ISO timestamp.` }
  }
  return value
}

const normalizeContentType = (request: Request): string | undefined => {
  const value = request.headers.get('content-type')?.split(';')[0]?.trim()
  return value === undefined || value === '' ? undefined : value.toLowerCase()
}

const archiveRefFromDigest = (artifactSha256: string): string =>
  `archive.gym.harbor_full_trace.${artifactSha256.slice(0, 32)}`

const parseUploadHeaders = (
  request: Request,
  nowIso: () => string,
): ParsedUploadHeaders | Readonly<{ error: string }> => {
  const runRef = requireSafeRef(request, 'x-openagents-run-ref')
  if (typeof runRef !== 'string') {
    return runRef
  }

  const jobRef = requireSafeRef(request, 'x-openagents-job-ref')
  if (typeof jobRef !== 'string') {
    return jobRef
  }

  const artifactSha256 = headerValue(request, 'x-openagents-archive-sha256')
  if (artifactSha256 === undefined || !SHA256_PATTERN.test(artifactSha256)) {
    return {
      error:
        'x-openagents-archive-sha256 must be a lowercase or uppercase SHA-256 hex digest.',
    }
  }

  const archiveRef =
    headerValue(request, 'x-openagents-archive-ref') ??
    archiveRefFromDigest(artifactSha256.toLowerCase())
  if (!SAFE_REF_PATTERN.test(archiveRef)) {
    return { error: 'x-openagents-archive-ref is not a bounded ref token.' }
  }

  const declaredBytes = parsePositiveInteger(
    headerValue(request, 'x-openagents-archive-bytes'),
  )
  const contentLength = parsePositiveInteger(
    request.headers.get('content-length') ?? undefined,
  )
  const artifactBytes = declaredBytes ?? contentLength
  if (artifactBytes === undefined) {
    return {
      error:
        'Missing byte length. Send content-length or x-openagents-archive-bytes.',
    }
  }
  if (
    contentLength !== undefined &&
    declaredBytes !== undefined &&
    contentLength !== declaredBytes
  ) {
    return {
      error:
        'content-length and x-openagents-archive-bytes must agree when both are present.',
    }
  }
  if (artifactBytes > MAX_ARCHIVE_BYTES) {
    return {
      error: `Archive exceeds the ${MAX_ARCHIVE_BYTES} byte operator upload cap.`,
    }
  }

  const contentType = normalizeContentType(request) ?? DEFAULT_CONTENT_TYPE
  if (!ALLOWED_CONTENT_TYPES.has(contentType)) {
    return {
      error:
        'Archive upload content-type must be application/gzip, application/x-gzip, or application/octet-stream.',
    }
  }

  const captureStartedAt = optionalIso(
    request,
    'x-openagents-capture-started-at',
  )
  if (captureStartedAt !== null && typeof captureStartedAt === 'object') {
    return captureStartedAt
  }

  const captureCompletedAt =
    headerValue(request, 'x-openagents-capture-completed-at') ?? nowIso()
  if (Number.isNaN(Date.parse(captureCompletedAt))) {
    return {
      error: 'x-openagents-capture-completed-at must be an ISO timestamp.',
    }
  }

  return {
    archiveRef,
    artifactBytes,
    artifactSha256: artifactSha256.toLowerCase(),
    captureCompletedAt,
    captureStartedAt,
    contentType,
    jobRef,
    runRef,
  }
}

const archiveSummary = (record: HarborFullTraceArchiveRecord) => ({
  archiveRef: record.archiveRef,
  runRef: record.runRef,
  jobRef: record.jobRef,
  sourceKind: record.sourceKind,
  artifactR2Key: record.artifactR2Key,
  artifactSha256: record.artifactSha256,
  artifactBytes: record.artifactBytes,
  contentType: record.contentType,
  captureStartedAt: record.captureStartedAt,
  captureCompletedAt: record.captureCompletedAt,
  visibility: record.visibility,
  containsRawPrompts: record.containsRawPrompts,
  containsRawLogs: record.containsRawLogs,
  containsPrivateMaterial: record.containsPrivateMaterial,
  demandKind: record.demandKind,
  demandSource: record.demandSource,
  createdAt: record.createdAt,
  updatedAt: record.updatedAt,
  downloadUrl: `/api/operator/gym/full-trace-archives?archive_ref=${encodeURIComponent(
    record.archiveRef,
  )}&download=1`,
  authority: {
    acceptedWorkAuthority: false,
    payoutAuthority: false,
    publicClaimAuthority: false,
    trainingConsentAuthority: false,
  },
})

const handleListArchives = (
  request: Request,
  store: HarborFullTraceArchiveStore,
): Effect.Effect<HttpResponse> =>
  Effect.tryPromise({
    catch: error => (error instanceof Error ? error.message : String(error)),
    try: async () => {
      const url = new URL(request.url)
      const limit = parsePositiveInteger(
        url.searchParams.get('limit') ?? undefined,
      )
      const runRef = url.searchParams.get('run_ref')?.trim() || undefined
      const archives = await store.listArchives({
        ...(limit === undefined ? {} : { limit }),
        ...(runRef === undefined ? {} : { runRef }),
      })
      return noStoreJsonResponse({
        schemaVersion: 'openagents.gym.harbor_full_trace_archives.v1',
        scope: 'operator',
        archives: archives.map(archiveSummary),
      })
    },
  }).pipe(
    Effect.catch(reason =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'harbor_full_trace_archive_list_failed', reason },
          { status: 500 },
        ),
      ),
    ),
  )

const handleDownloadArchive = (
  archiveRef: string,
  store: HarborFullTraceArchiveStore,
): Effect.Effect<HttpResponse> =>
  Effect.tryPromise({
    catch: error => (error instanceof Error ? error.message : String(error)),
    try: async () => store.readArchiveObject(archiveRef),
  }).pipe(
    Effect.map(object => {
      if (object === undefined) {
        return archiveNotFound()
      }

      const headers = new Headers({
        'cache-control': 'no-store',
        'content-disposition': `attachment; filename="${object.record.archiveRef}.tar.gz"`,
        'content-type': object.contentType,
        'x-openagents-archive-ref': object.record.archiveRef,
        'x-openagents-archive-sha256': object.record.artifactSha256,
      })
      headers.set('content-length', String(object.size))

      return new Response(object.body, { headers, status: 200 })
    }),
    Effect.catch(reason =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'harbor_full_trace_archive_download_failed', reason },
          { status: 500 },
        ),
      ),
    ),
  )

const handleGetArchives = (
  request: Request,
  store: HarborFullTraceArchiveStore,
): Effect.Effect<HttpResponse> => {
  const url = new URL(request.url)
  const archiveRef = url.searchParams.get('archive_ref')?.trim()
  const download = url.searchParams.get('download') === '1'
  if (download) {
    if (archiveRef === undefined || archiveRef === '') {
      return Effect.succeed(badRequest('download=1 requires archive_ref.'))
    }
    if (!SAFE_REF_PATTERN.test(archiveRef)) {
      return Effect.succeed(
        badRequest('archive_ref is not a bounded ref token.'),
      )
    }
    return handleDownloadArchive(archiveRef, store)
  }
  return handleListArchives(request, store)
}

const handleUploadArchive = (
  request: Request,
  input: HarborFullTraceArchiveRouteInput,
  store: HarborFullTraceArchiveStore,
): Effect.Effect<HttpResponse> => {
  const body = request.body
  if (body === null) {
    return Effect.succeed(badRequest('Archive upload body is required.'))
  }

  const parsed = parseUploadHeaders(
    request,
    input.nowIso ?? currentIsoTimestamp,
  )
  if ('error' in parsed) {
    return Effect.succeed(badRequest(parsed.error))
  }

  return Effect.tryPromise({
    catch: error => (error instanceof Error ? error.message : String(error)),
    try: async () =>
      store.putArchive({
        ...parsed,
        body,
      }),
  }).pipe(
    Effect.map(result =>
      noStoreJsonResponse(
        {
          schemaVersion: 'openagents.gym.harbor_full_trace_archives.v1',
          kind: 'harbor_full_trace_archive_stored',
          archive: archiveSummary(result.record),
          created: result.created,
        },
        { status: result.created ? 201 : 200 },
      ),
    ),
    Effect.catch(reason =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'harbor_full_trace_archive_store_failed', reason },
          { status: 500 },
        ),
      ),
    ),
  )
}

export const handleOperatorHarborFullTraceArchivesApi = (
  request: Request,
  input: HarborFullTraceArchiveRouteInput,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }

  const store = input.store
  if (store === undefined) {
    return Effect.succeed(unavailable())
  }

  return Effect.gen(function* () {
    const authorized = yield* Effect.promise(() =>
      input.requireAdminApiToken(request),
    )
    if (!authorized) {
      return unauthorized()
    }
    if (request.method === 'POST') {
      return yield* handleUploadArchive(request, input, store)
    }
    return yield* handleGetArchives(request, store)
  })
}
