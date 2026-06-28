import {
  decodeForgeGitPackfileArchiveRow,
  type ForgeGitPackfileArchiveRow,
  type ForgeGitPackfileObjectFormat,
} from '@openagentsinc/forge-protocol'

export const FORGE_GIT_PACKFILE_CONTENT_TYPE =
  'application/x-git-packed-objects'

export type ForgeGitPackfileRefUpdate = Readonly<{
  oldObjectId: string
  newObjectId: string
  refName: string
  action: 'create' | 'update' | 'delete'
}>

export type ForgeGitPackfileArchiveInput = Readonly<{
  tenantRef: string
  packfileRef: string
  repositoryRef: string
  changeRef?: string | null
  receivePackRef?: string | null
  packfileSha256: string
  packfileBytes: number
  objectFormat: ForgeGitPackfileObjectFormat
  capabilities: ReadonlyArray<string>
  refUpdates: ReadonlyArray<ForgeGitPackfileRefUpdate>
  sourceRefs: ReadonlyArray<string>
  body: ReadableStream | ArrayBuffer | string | Blob
  nowIso: string
}>

export type ForgeGitPackfileArchivePutResult = Readonly<{
  created: boolean
  record: ForgeGitPackfileArchiveRow
}>

export type ForgeGitPackfileArchiveObject = Readonly<{
  body: ReadableStream
  contentType: string
  record: ForgeGitPackfileArchiveRow
  size: number
}>

export type ForgeGitPackfileArchiveStore = Readonly<{
  putPackfile: (
    input: ForgeGitPackfileArchiveInput,
  ) => Promise<ForgeGitPackfileArchivePutResult>
  readPackfile: (
    tenantRef: string,
    packfileRef: string,
  ) => Promise<ForgeGitPackfileArchiveRow | undefined>
  readPackfileByDigest: (
    tenantRef: string,
    packfileSha256: string,
  ) => Promise<ForgeGitPackfileArchiveRow | undefined>
  readPackfileObject: (
    tenantRef: string,
    packfileRef: string,
  ) => Promise<ForgeGitPackfileArchiveObject | undefined>
  listPackfiles: (
    tenantRef: string,
    input?: Readonly<{ repositoryRef?: string; limit?: number }>,
  ) => Promise<ReadonlyArray<ForgeGitPackfileArchiveRow>>
}>

class ForgeGitPackfileArchiveStoreError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForgeGitPackfileArchiveStoreError'
  }
}

const sha256Pattern = /^[0-9a-f]{64}$/i

const jsonArray = (values: ReadonlyArray<unknown>): string =>
  JSON.stringify([...values])

const boundedLimit = (limit: number | undefined): number =>
  Math.min(Math.max(Math.floor(limit ?? 50), 1), 200)

const safeR2Segment = (value: string): string => {
  const safe = value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '')
  return safe === '' ? 'unknown' : safe.slice(0, 160)
}

const assertSha256 = (value: string): string => {
  if (!sha256Pattern.test(value)) {
    throw new ForgeGitPackfileArchiveStoreError(
      'forge git packfile digest must be a lowercase or uppercase SHA-256 hex string',
    )
  }
  return value.toLowerCase()
}

const assertPackfileBytes = (value: number): number => {
  if (!Number.isInteger(value) || value < 0) {
    throw new ForgeGitPackfileArchiveStoreError(
      'forge git packfile byte count must be a non-negative integer',
    )
  }
  return value
}

const rowOrFail = <T>(row: T | null, label: string): T => {
  if (row === null) {
    throw new ForgeGitPackfileArchiveStoreError(`${label} was not persisted`)
  }
  return row
}

export const forgeGitPackfileArchiveR2Key = (
  input: Pick<
    ForgeGitPackfileArchiveInput,
    'tenantRef' | 'repositoryRef' | 'packfileRef' | 'packfileSha256'
  >,
): string =>
  [
    'private',
    'forge',
    'git-packfiles',
    safeR2Segment(input.tenantRef),
    safeR2Segment(input.repositoryRef),
    `${safeR2Segment(input.packfileRef)}-${assertSha256(input.packfileSha256).slice(
      0,
      16,
    )}.pack`,
  ].join('/')

export const forgeGitPackfileObjectFormatForCapabilities = (
  capabilities: ReadonlyArray<string>,
): ForgeGitPackfileObjectFormat => {
  const objectFormat = capabilities
    .find(capability => capability.startsWith('object-format='))
    ?.slice('object-format='.length)
  return objectFormat === 'sha1' || objectFormat === 'sha256'
    ? objectFormat
    : 'unknown'
}

const selectRowByRef = async (
  db: D1Database,
  tenantRef: string,
  packfileRef: string,
): Promise<ForgeGitPackfileArchiveRow | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_git_packfile_archives
        WHERE tenant_ref = ? AND packfile_ref = ?
        LIMIT 1
      `,
    )
    .bind(tenantRef, packfileRef)
    .first()

  return row === null ? undefined : decodeForgeGitPackfileArchiveRow(row)
}

const selectRowByDigest = async (
  db: D1Database,
  tenantRef: string,
  packfileSha256: string,
): Promise<ForgeGitPackfileArchiveRow | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_git_packfile_archives
        WHERE tenant_ref = ? AND packfile_sha256 = ?
        LIMIT 1
      `,
    )
    .bind(tenantRef, assertSha256(packfileSha256))
    .first()

  return row === null ? undefined : decodeForgeGitPackfileArchiveRow(row)
}

export const makeD1R2ForgeGitPackfileArchiveStore = (
  db: D1Database,
  bucket: R2Bucket,
): ForgeGitPackfileArchiveStore => ({
  async putPackfile(input) {
    const packfileSha256 = assertSha256(input.packfileSha256)
    const packfileBytes = assertPackfileBytes(input.packfileBytes)
    const existingByRef = await selectRowByRef(db, input.tenantRef, input.packfileRef)
    if (existingByRef !== undefined) {
      return { created: false, record: existingByRef }
    }

    const existingByDigest = await selectRowByDigest(
      db,
      input.tenantRef,
      packfileSha256,
    )
    if (existingByDigest !== undefined) {
      return { created: false, record: existingByDigest }
    }

    const artifactR2Key = forgeGitPackfileArchiveR2Key({
      ...input,
      packfileSha256,
    })
    const existingObject = await bucket.head(artifactR2Key)
    if (existingObject === null) {
      const stored = await bucket.put(artifactR2Key, input.body, {
        customMetadata: {
          packfileRef: input.packfileRef,
          packfileSha256,
          repositoryRef: input.repositoryRef,
          tenantRef: input.tenantRef,
          visibility: 'operator_only',
        },
        httpMetadata: {
          contentType: FORGE_GIT_PACKFILE_CONTENT_TYPE,
        },
        sha256: packfileSha256,
      })
      if (stored === null) {
        throw new ForgeGitPackfileArchiveStoreError(
          'forge git packfile R2 put returned null',
        )
      }
    }

    await db
      .prepare(
        `
          INSERT INTO forge_git_packfile_archives (
            tenant_ref,
            packfile_ref,
            repository_ref,
            change_ref,
            receive_pack_ref,
            artifact_r2_key,
            packfile_sha256,
            packfile_bytes,
            object_format,
            command_count,
            capabilities_json,
            ref_updates_json,
            source_refs_json,
            content_type,
            visibility,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `,
      )
      .bind(
        input.tenantRef,
        input.packfileRef,
        input.repositoryRef,
        input.changeRef ?? null,
        input.receivePackRef ?? null,
        artifactR2Key,
        packfileSha256,
        packfileBytes,
        input.objectFormat,
        input.refUpdates.length,
        jsonArray(input.capabilities),
        jsonArray(input.refUpdates),
        jsonArray(input.sourceRefs),
        FORGE_GIT_PACKFILE_CONTENT_TYPE,
        'operator_only',
        input.nowIso,
        input.nowIso,
      )
      .run()

    return {
      created: true,
      record: decodeForgeGitPackfileArchiveRow(
        rowOrFail(
          await db
            .prepare(
              `
                SELECT *
                FROM forge_git_packfile_archives
                WHERE tenant_ref = ? AND packfile_ref = ?
              `,
            )
            .bind(input.tenantRef, input.packfileRef)
            .first(),
          'forge git packfile archive',
        ),
      ),
    }
  },

  readPackfile: (tenantRef, packfileRef) =>
    selectRowByRef(db, tenantRef, packfileRef),

  readPackfileByDigest: (tenantRef, packfileSha256) =>
    selectRowByDigest(db, tenantRef, packfileSha256),

  async readPackfileObject(tenantRef, packfileRef) {
    const record = await selectRowByRef(db, tenantRef, packfileRef)
    if (record === undefined) {
      return undefined
    }

    const object = await bucket.get(record.artifact_r2_key)
    if (object === null) {
      return undefined
    }

    return {
      body: object.body,
      contentType:
        object.httpMetadata?.contentType ??
        record.content_type ??
        FORGE_GIT_PACKFILE_CONTENT_TYPE,
      record,
      size: object.size,
    }
  },

  async listPackfiles(tenantRef, input) {
    const limit = boundedLimit(input?.limit)
    if (input?.repositoryRef !== undefined) {
      const rows = await db
        .prepare(
          `
            SELECT *
            FROM forge_git_packfile_archives
            WHERE tenant_ref = ? AND repository_ref = ?
            ORDER BY created_at DESC, packfile_ref ASC
            LIMIT ?
          `,
        )
        .bind(tenantRef, input.repositoryRef, limit)
        .all()
      return (rows.results ?? []).map(row =>
        decodeForgeGitPackfileArchiveRow(row),
      )
    }

    const rows = await db
      .prepare(
        `
          SELECT *
          FROM forge_git_packfile_archives
          WHERE tenant_ref = ?
          ORDER BY created_at DESC, packfile_ref ASC
          LIMIT ?
        `,
      )
      .bind(tenantRef, limit)
      .all()
    return (rows.results ?? []).map(row =>
      decodeForgeGitPackfileArchiveRow(row),
    )
  },
})
