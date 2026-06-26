import { currentIsoTimestamp } from '../../runtime-primitives'

export type HarborFullTraceArchiveRecord = Readonly<{
  archiveRef: string
  runRef: string
  jobRef: string
  sourceKind: 'harbor_job_tarball'
  artifactR2Key: string
  artifactSha256: string
  artifactBytes: number
  contentType: string
  captureStartedAt: string | null
  captureCompletedAt: string
  visibility: 'operator_only'
  containsRawPrompts: true
  containsRawLogs: true
  containsPrivateMaterial: true
  demandKind: 'internal'
  demandSource: 'harbor_terminal_bench'
  createdAt: string
  updatedAt: string
}>

export type HarborFullTraceArchivePutInput = Readonly<{
  archiveRef: string
  runRef: string
  jobRef: string
  artifactSha256: string
  artifactBytes: number
  contentType: string
  captureStartedAt: string | null
  captureCompletedAt: string
  body: ReadableStream
}>

export type HarborFullTraceArchivePutResult = Readonly<{
  created: boolean
  record: HarborFullTraceArchiveRecord
}>

export type HarborFullTraceArchiveObject = Readonly<{
  body: ReadableStream
  contentType: string
  record: HarborFullTraceArchiveRecord
  size: number
}>

export type HarborFullTraceArchiveStore = Readonly<{
  listArchives: (
    input?: Readonly<{ limit?: number; runRef?: string }>,
  ) => Promise<ReadonlyArray<HarborFullTraceArchiveRecord>>
  putArchive: (
    input: HarborFullTraceArchivePutInput,
  ) => Promise<HarborFullTraceArchivePutResult>
  readArchive: (
    archiveRef: string,
  ) => Promise<HarborFullTraceArchiveRecord | undefined>
  readArchiveObject: (
    archiveRef: string,
  ) => Promise<HarborFullTraceArchiveObject | undefined>
}>

type HarborFullTraceArchiveRow = Readonly<{
  archive_ref: string
  run_ref: string
  job_ref: string
  source_kind: string
  artifact_r2_key: string
  artifact_sha256: string
  artifact_bytes: number
  content_type: string
  capture_started_at: string | null
  capture_completed_at: string
  visibility: string
  contains_raw_prompts: number
  contains_raw_logs: number
  contains_private_material: number
  demand_kind: string
  demand_source: string
  created_at: string
  updated_at: string
}>

const rowToRecord = (
  row: HarborFullTraceArchiveRow,
): HarborFullTraceArchiveRecord => ({
  archiveRef: row.archive_ref,
  runRef: row.run_ref,
  jobRef: row.job_ref,
  sourceKind: 'harbor_job_tarball',
  artifactR2Key: row.artifact_r2_key,
  artifactSha256: row.artifact_sha256,
  artifactBytes: Number(row.artifact_bytes),
  contentType: row.content_type,
  captureStartedAt: row.capture_started_at,
  captureCompletedAt: row.capture_completed_at,
  visibility: 'operator_only',
  containsRawPrompts: true,
  containsRawLogs: true,
  containsPrivateMaterial: true,
  demandKind: 'internal',
  demandSource: 'harbor_terminal_bench',
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const boundedLimit = (limit: number | undefined): number =>
  Math.min(Math.max(Math.floor(limit ?? 50), 1), 200)

const safeR2Segment = (value: string): string => {
  const safe = value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '')
  return safe === '' ? 'unknown' : safe.slice(0, 160)
}

export const harborFullTraceArchiveR2Key = (
  input: Pick<
    HarborFullTraceArchivePutInput,
    'archiveRef' | 'artifactSha256' | 'runRef'
  >,
): string =>
  [
    'private',
    'gym',
    'harbor-full-trace-archives',
    safeR2Segment(input.runRef),
    `${safeR2Segment(input.archiveRef)}-${input.artifactSha256.slice(
      0,
      16,
    )}.tar.gz`,
  ].join('/')

const selectRowByArchiveRef = async (
  db: D1Database,
  archiveRef: string,
): Promise<HarborFullTraceArchiveRecord | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM gym_harbor_full_trace_archives
        WHERE archive_ref = ?
        LIMIT 1
      `,
    )
    .bind(archiveRef)
    .first<HarborFullTraceArchiveRow>()

  return row === null ? undefined : rowToRecord(row)
}

const selectRowByDigest = async (
  db: D1Database,
  artifactSha256: string,
): Promise<HarborFullTraceArchiveRecord | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM gym_harbor_full_trace_archives
        WHERE artifact_sha256 = ?
        LIMIT 1
      `,
    )
    .bind(artifactSha256)
    .first<HarborFullTraceArchiveRow>()

  return row === null ? undefined : rowToRecord(row)
}

export const makeD1R2HarborFullTraceArchiveStore = (
  db: D1Database,
  bucket: R2Bucket,
): HarborFullTraceArchiveStore => ({
  listArchives: async input => {
    const limit = boundedLimit(input?.limit)
    if (input?.runRef !== undefined) {
      const rows = await db
        .prepare(
          `
            SELECT *
            FROM gym_harbor_full_trace_archives
            WHERE run_ref = ?
            ORDER BY capture_completed_at DESC, created_at DESC, archive_ref ASC
            LIMIT ?
          `,
        )
        .bind(input.runRef, limit)
        .all<HarborFullTraceArchiveRow>()
      return (rows.results ?? []).map(rowToRecord)
    }

    const rows = await db
      .prepare(
        `
          SELECT *
          FROM gym_harbor_full_trace_archives
          ORDER BY capture_completed_at DESC, created_at DESC, archive_ref ASC
          LIMIT ?
        `,
      )
      .bind(limit)
      .all<HarborFullTraceArchiveRow>()
    return (rows.results ?? []).map(rowToRecord)
  },
  putArchive: async input => {
    const existingByRef = await selectRowByArchiveRef(db, input.archiveRef)
    if (existingByRef !== undefined) {
      return { created: false, record: existingByRef }
    }

    const existingByDigest = await selectRowByDigest(db, input.artifactSha256)
    if (existingByDigest !== undefined) {
      return { created: false, record: existingByDigest }
    }

    const artifactR2Key = harborFullTraceArchiveR2Key(input)
    const existingObject = await bucket.head(artifactR2Key)
    if (existingObject === null) {
      await bucket.put(artifactR2Key, input.body, {
        customMetadata: {
          archiveRef: input.archiveRef,
          demandKind: 'internal',
          demandSource: 'harbor_terminal_bench',
          jobRef: input.jobRef,
          runRef: input.runRef,
          visibility: 'operator_only',
        },
        httpMetadata: {
          contentType: input.contentType,
        },
      })
    }

    const nowIso = currentIsoTimestamp()
    await db
      .prepare(
        `
          INSERT INTO gym_harbor_full_trace_archives (
            archive_ref,
            run_ref,
            job_ref,
            source_kind,
            artifact_r2_key,
            artifact_sha256,
            artifact_bytes,
            content_type,
            capture_started_at,
            capture_completed_at,
            visibility,
            contains_raw_prompts,
            contains_raw_logs,
            contains_private_material,
            demand_kind,
            demand_source,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `,
      )
      .bind(
        input.archiveRef,
        input.runRef,
        input.jobRef,
        'harbor_job_tarball',
        artifactR2Key,
        input.artifactSha256,
        input.artifactBytes,
        input.contentType,
        input.captureStartedAt,
        input.captureCompletedAt,
        'operator_only',
        1,
        1,
        1,
        'internal',
        'harbor_terminal_bench',
        nowIso,
        nowIso,
      )
      .run()

    return {
      created: true,
      record: {
        archiveRef: input.archiveRef,
        runRef: input.runRef,
        jobRef: input.jobRef,
        sourceKind: 'harbor_job_tarball',
        artifactR2Key,
        artifactSha256: input.artifactSha256,
        artifactBytes: input.artifactBytes,
        contentType: input.contentType,
        captureStartedAt: input.captureStartedAt,
        captureCompletedAt: input.captureCompletedAt,
        visibility: 'operator_only',
        containsRawPrompts: true,
        containsRawLogs: true,
        containsPrivateMaterial: true,
        demandKind: 'internal',
        demandSource: 'harbor_terminal_bench',
        createdAt: nowIso,
        updatedAt: nowIso,
      },
    }
  },
  readArchive: archiveRef => selectRowByArchiveRef(db, archiveRef),
  readArchiveObject: async archiveRef => {
    const record = await selectRowByArchiveRef(db, archiveRef)
    if (record === undefined) {
      return undefined
    }

    const object = await bucket.get(record.artifactR2Key)
    if (object === null) {
      return undefined
    }

    return {
      body: object.body,
      contentType: object.httpMetadata?.contentType ?? record.contentType,
      record,
      size: object.size,
    }
  },
})
