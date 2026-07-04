import { Data, Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const KHALA_CODE_DOWNLOAD_COUNTS_ENDPOINT =
  '/api/public/khala-code/download-counts' as const
export const KHALA_CODE_DOWNLOAD_COUNTS_TABLE =
  'khala_code_download_events' as const

export const KhalaCodeDownloadArtifactKind = S.Literals([
  'desktop_dmg',
  'npm_cli',
  'source_build',
])
export type KhalaCodeDownloadArtifactKind =
  typeof KhalaCodeDownloadArtifactKind.Type

export const KhalaCodeDownloadCountRow = S.Struct({
  artifactKind: KhalaCodeDownloadArtifactKind,
  channel: S.String,
  exactRows: S.Int,
  lastCountedAt: S.NullOr(S.String),
})
export type KhalaCodeDownloadCountRow =
  typeof KhalaCodeDownloadCountRow.Type

export const PublicKhalaCodeDownloadCounts = S.Struct({
  schemaVersion: S.Literal('openagents.khala_code.public_download_counts.v1'),
  product: S.Literal('khala-code'),
  promiseId: S.Literal('khala_code.desktop_codex_wrapper.v1'),
  generatedAt: S.String,
  counts: S.Array(KhalaCodeDownloadCountRow),
  blockerRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaCodeDownloadCounts =
  typeof PublicKhalaCodeDownloadCounts.Type

export type KhalaCodeDownloadCountsStore = Readonly<{
  readCounts: () => Effect.Effect<ReadonlyArray<KhalaCodeDownloadCountRow>, unknown>
}>

type KhalaCodeDownloadCountsRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: (() => string) | undefined
  store?: KhalaCodeDownloadCountsStore | undefined
}>

type DownloadCountSqlRow = Readonly<{
  artifact_kind: unknown
  channel: unknown
  download_count: unknown
  last_counted_at: unknown
}>

export class KhalaCodeDownloadCountsReadError extends Data.TaggedError(
  'KhalaCodeDownloadCountsReadError',
)<{ readonly reason: string }> {}

const artifactKindOrder: ReadonlyArray<KhalaCodeDownloadArtifactKind> = [
  'desktop_dmg',
  'npm_cli',
  'source_build',
]

const artifactKindRank = (kind: KhalaCodeDownloadArtifactKind): number =>
  artifactKindOrder.indexOf(kind)

const safeString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const safeCount = (value: unknown): number | undefined => {
  const parsed =
    typeof value === 'number'
      ? value
      : typeof value === 'string'
        ? Number.parseInt(value, 10)
        : Number.NaN

  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : undefined
}

export const khalaCodeDownloadCountRowFromSql = (
  row: DownloadCountSqlRow,
): KhalaCodeDownloadCountRow | undefined => {
  const artifactKind = safeString(row.artifact_kind)
  const channel = safeString(row.channel)
  const exactRows = safeCount(row.download_count)
  const lastCountedAt =
    row.last_counted_at === null ? null : safeString(row.last_counted_at)

  if (
    artifactKind !== 'desktop_dmg' &&
    artifactKind !== 'npm_cli' &&
    artifactKind !== 'source_build'
  ) {
    return undefined
  }
  if (channel === undefined || exactRows === undefined) {
    return undefined
  }

  return {
    artifactKind,
    channel,
    exactRows,
    lastCountedAt: lastCountedAt ?? null,
  }
}

export const makeD1KhalaCodeDownloadCountsStore = (
  db: D1Database | undefined,
): KhalaCodeDownloadCountsStore => ({
  readCounts: () =>
    db === undefined
      ? Effect.succeed([])
      : Effect.tryPromise({
          try: async () => {
            const rows = await db
              .prepare(
                `
                  SELECT
                    artifact_kind,
                    channel,
                    COUNT(*) AS download_count,
                    MAX(occurred_at) AS last_counted_at
                  FROM khala_code_download_events
                  WHERE product = ?
                    AND public_countable = 1
                  GROUP BY artifact_kind, channel
                  ORDER BY artifact_kind ASC, channel ASC
                `,
              )
              .bind('khala-code')
              .all<DownloadCountSqlRow>()

            return (rows.results ?? [])
              .flatMap(row => {
                const count = khalaCodeDownloadCountRowFromSql(row)
                return count === undefined ? [] : [count]
              })
              .sort((left, right) => {
                const kindCompare =
                  artifactKindRank(left.artifactKind) -
                  artifactKindRank(right.artifactKind)
                return kindCompare === 0
                  ? left.channel.localeCompare(right.channel)
                  : kindCompare
              })
          },
          catch: error =>
            new KhalaCodeDownloadCountsReadError({
              reason: missingTableMessage(error),
            }),
        }),
})

const missingTableMessage = (error: unknown): string =>
  error instanceof KhalaCodeDownloadCountsReadError
    ? error.reason
    : error instanceof Error ? error.message : String(error)

const isMissingDownloadCountTable = (error: unknown): boolean => {
  const message = missingTableMessage(error).toLowerCase()
  return (
    message.includes(KHALA_CODE_DOWNLOAD_COUNTS_TABLE) &&
    (message.includes('no such table') ||
      message.includes('not found') ||
      message.includes('no such object'))
  )
}

const responsePayload = (
  input: Readonly<{
    blockerRefs: ReadonlyArray<string>
    counts: ReadonlyArray<KhalaCodeDownloadCountRow>
    generatedAt: string
    sourceRefs: ReadonlyArray<string>
  }>,
): PublicKhalaCodeDownloadCounts => ({
  schemaVersion: 'openagents.khala_code.public_download_counts.v1',
  product: 'khala-code',
  promiseId: 'khala_code.desktop_codex_wrapper.v1',
  generatedAt: input.generatedAt,
  counts: [...input.counts],
  blockerRefs: [...input.blockerRefs],
  sourceRefs: [...input.sourceRefs],
  staleness: liveAtReadStaleness([KHALA_CODE_DOWNLOAD_COUNTS_TABLE]),
})

export const handlePublicKhalaCodeDownloadCountsApi = (
  request: Request,
  input: KhalaCodeDownloadCountsRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp
  const store =
    input.store ?? makeD1KhalaCodeDownloadCountsStore(input.OPENAGENTS_DB)

  return store.readCounts().pipe(
    Effect.map(counts =>
      noStoreJsonResponse(
        responsePayload({
          blockerRefs:
            counts.length === 0
              ? ['blocker.public.khala_code_download_counts.no_rows']
              : [],
          counts,
          generatedAt: nowIso(),
          sourceRefs: [`table:${KHALA_CODE_DOWNLOAD_COUNTS_TABLE}`],
        }),
      ),
    ),
    Effect.catch(error => {
      if (isMissingDownloadCountTable(error)) {
        return Effect.succeed(
          noStoreJsonResponse(
            responsePayload({
              blockerRefs: [
                'blocker.public.khala_code_download_counts.table_missing',
              ],
              counts: [],
              generatedAt: nowIso(),
              sourceRefs: [`table:${KHALA_CODE_DOWNLOAD_COUNTS_TABLE}:missing`],
            }),
          ),
        )
      }

      return Effect.succeed(
        noStoreJsonResponse(
          { error: 'khala_code_download_counts_unavailable' },
          { status: 503 },
        ),
      )
    }),
  )
}
