import type {
  ForgeGitPackfileObjectFormat,
} from '@openagentsinc/forge-protocol'

import type { ForgeGitPackfileRefUpdate } from './forge-git-packfile-archive-store'

export type ForgeGitCanonicalRefState = 'active' | 'deleted'

export type ForgeGitCanonicalRefRow = Readonly<{
  tenant_ref: string
  repository_ref: string
  ref_name: string
  object_id: string | null
  previous_object_id: string | null
  object_format: ForgeGitPackfileObjectFormat
  state: ForgeGitCanonicalRefState
  updated_by_change_ref: string
  updated_by_packfile_ref: string
  updated_by_receive_pack_ref: string
  source_refs_json: string
  created_at: string
  updated_at: string
}>

export type ForgeGitCanonicalObjectRow = Readonly<{
  tenant_ref: string
  repository_ref: string
  object_id: string
  object_format: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>
  packfile_ref: string
  packfile_sha256: string
  first_seen_at: string
  latest_seen_at: string
  source_refs_json: string
}>

export type ForgeGitReceivePackIntakeRow = Readonly<{
  tenant_ref: string
  receive_pack_ref: string
  repository_ref: string
  token_ref: string
  subject_ref: string
  change_ref: string | null
  packfile_ref: string | null
  packfile_sha256: string | null
  packfile_bytes: number
  object_format: ForgeGitPackfileObjectFormat
  state: 'accepted' | 'rejected'
  command_count: number
  ref_updates_json: string
  source_refs_json: string
  rejection_code: string | null
  rejection_reason: string | null
  created_at: string
  updated_at: string
}>

export type ForgeGitCanonicalReceivePackInput = Readonly<{
  tenantRef: string
  repositoryRef: string
  receivePackRef: string
  tokenRef: string
  subjectRef: string
  changeRef: string
  packfileRef: string
  packfileSha256: string
  packfileBytes: number
  objectFormat: ForgeGitPackfileObjectFormat
  refUpdates: ReadonlyArray<ForgeGitPackfileRefUpdate>
  sourceRefs: ReadonlyArray<string>
  nowIso: string
}>

export type ForgeGitCanonicalExternalRefImportInput = Readonly<{
  tenantRef: string
  repositoryRef: string
  refName: string
  objectId: string
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>
  changeRef: string
  packfileRef: string
  receivePackRef: string
  sourceDigestSha256: string
  sourceRefs: ReadonlyArray<string>
  nowIso: string
}>

export type ForgeGitCanonicalExternalRefImportResult = Readonly<{
  changed: boolean
  ref: ForgeGitCanonicalRefRow
  object: ForgeGitCanonicalObjectRow
}>

export type ForgeGitCanonicalPreflightInput = Pick<
  ForgeGitCanonicalReceivePackInput,
  | 'tenantRef'
  | 'repositoryRef'
  | 'packfileSha256'
  | 'packfileBytes'
  | 'objectFormat'
  | 'refUpdates'
>

export type ForgeGitCanonicalApplyResult = Readonly<{
  intake: ForgeGitReceivePackIntakeRow
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>
  refs: ReadonlyArray<ForgeGitCanonicalRefRow>
  objects: ReadonlyArray<ForgeGitCanonicalObjectRow>
}>

export type ForgeGitCanonicalStore = Readonly<{
  preflightReceivePack: (
    input: ForgeGitCanonicalPreflightInput,
  ) => Promise<Exclude<ForgeGitPackfileObjectFormat, 'unknown'>>
  applyReceivePack: (
    input: ForgeGitCanonicalReceivePackInput,
  ) => Promise<ForgeGitCanonicalApplyResult>
  importExternalRef: (
    input: ForgeGitCanonicalExternalRefImportInput,
  ) => Promise<ForgeGitCanonicalExternalRefImportResult>
  readRef: (
    tenantRef: string,
    repositoryRef: string,
    refName: string,
  ) => Promise<ForgeGitCanonicalRefRow | undefined>
  listRefs: (
    tenantRef: string,
    repositoryRef: string,
    input?: Readonly<{
      state?: ForgeGitCanonicalRefState
      limit?: number
    }>,
  ) => Promise<ReadonlyArray<ForgeGitCanonicalRefRow>>
  readObject: (
    tenantRef: string,
    repositoryRef: string,
    objectId: string,
  ) => Promise<ForgeGitCanonicalObjectRow | undefined>
}>

export type ForgeGitCanonicalErrorCode =
  | 'forge_git_delete_only_push_rejected'
  | 'forge_git_duplicate_ref_update'
  | 'forge_git_invalid_object_id'
  | 'forge_git_invalid_packfile'
  | 'forge_git_ref_lock_conflict'
  | 'forge_git_unsafe_ref_update'

export class ForgeGitCanonicalStoreError extends Error {
  constructor(
    readonly errorCode: ForgeGitCanonicalErrorCode,
    message: string,
    readonly status: number,
  ) {
    super(message)
    this.name = 'ForgeGitCanonicalStoreError'
  }
}

const sha1Pattern = /^[0-9a-f]{40}$/i
const sha256Pattern = /^[0-9a-f]{64}$/i
const sha1Zero = '0'.repeat(40)
const sha256Zero = '0'.repeat(64)

// Exported: reused verbatim by the Postgres FOR-UPDATE ref-lock port
// (`forge-git-canonical-postgres-store.ts`, MIGRATION_PLAN §3.13) so the
// same pure validation/normalization logic runs identically on both
// engines — only the storage/locking primitives differ.
export const jsonArray = (values: ReadonlyArray<unknown>): string =>
  JSON.stringify([...values])

export const boundedLimit = (limit: number | undefined): number =>
  Math.min(Math.max(Math.floor(limit ?? 100), 1), 500)

const safeRefSegment = (value: string): string => {
  const safe = value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '')
  return safe === '' ? 'ref' : safe.slice(0, 120)
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

const stringField = (row: Record<string, unknown>, name: string): string => {
  const value = row[name]
  if (typeof value !== 'string') {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      `forge git canonical row field ${name} must be a string`,
      500,
    )
  }
  return value
}

const numberField = (row: Record<string, unknown>, name: string): number => {
  const value = row[name]
  if (typeof value !== 'number') {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      `forge git canonical row field ${name} must be a number`,
      500,
    )
  }
  return value
}

const nullableStringField = (
  row: Record<string, unknown>,
  name: string,
): string | null => {
  const value = row[name]
  if (value === null) {
    return null
  }
  if (typeof value !== 'string') {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      `forge git canonical row field ${name} must be a nullable string`,
      500,
    )
  }
  return value
}

const decodeObjectFormat = (value: string): ForgeGitPackfileObjectFormat => {
  if (value === 'sha1' || value === 'sha256' || value === 'unknown') {
    return value
  }
  throw new ForgeGitCanonicalStoreError(
    'forge_git_invalid_object_id',
    'forge git canonical row object format is invalid',
    500,
  )
}

const decodeNonUnknownObjectFormat = (
  value: string,
): Exclude<ForgeGitPackfileObjectFormat, 'unknown'> => {
  if (value === 'sha1' || value === 'sha256') {
    return value
  }
  throw new ForgeGitCanonicalStoreError(
    'forge_git_invalid_object_id',
    'forge git object row format cannot be unknown',
    500,
  )
}

const decodeRefState = (value: string): ForgeGitCanonicalRefState => {
  if (value === 'active' || value === 'deleted') {
    return value
  }
  throw new ForgeGitCanonicalStoreError(
    'forge_git_invalid_object_id',
    'forge git ref row state is invalid',
    500,
  )
}

const decodeRefRow = (row: unknown): ForgeGitCanonicalRefRow => {
  if (!isRecord(row)) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      'forge git canonical ref row must be an object',
      500,
    )
  }
  return {
    tenant_ref: stringField(row, 'tenant_ref'),
    repository_ref: stringField(row, 'repository_ref'),
    ref_name: stringField(row, 'ref_name'),
    object_id: nullableStringField(row, 'object_id'),
    previous_object_id: nullableStringField(row, 'previous_object_id'),
    object_format: decodeObjectFormat(stringField(row, 'object_format')),
    state: decodeRefState(stringField(row, 'state')),
    updated_by_change_ref: stringField(row, 'updated_by_change_ref'),
    updated_by_packfile_ref: stringField(row, 'updated_by_packfile_ref'),
    updated_by_receive_pack_ref: stringField(row, 'updated_by_receive_pack_ref'),
    source_refs_json: stringField(row, 'source_refs_json'),
    created_at: stringField(row, 'created_at'),
    updated_at: stringField(row, 'updated_at'),
  }
}

const decodeObjectRow = (row: unknown): ForgeGitCanonicalObjectRow => {
  if (!isRecord(row)) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      'forge git canonical object row must be an object',
      500,
    )
  }
  return {
    tenant_ref: stringField(row, 'tenant_ref'),
    repository_ref: stringField(row, 'repository_ref'),
    object_id: stringField(row, 'object_id'),
    object_format: decodeNonUnknownObjectFormat(
      stringField(row, 'object_format'),
    ),
    packfile_ref: stringField(row, 'packfile_ref'),
    packfile_sha256: stringField(row, 'packfile_sha256'),
    first_seen_at: stringField(row, 'first_seen_at'),
    latest_seen_at: stringField(row, 'latest_seen_at'),
    source_refs_json: stringField(row, 'source_refs_json'),
  }
}

const decodeIntakeRow = (row: unknown): ForgeGitReceivePackIntakeRow => {
  if (!isRecord(row)) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      'forge git receive-pack intake row must be an object',
      500,
    )
  }
  return {
    tenant_ref: stringField(row, 'tenant_ref'),
    receive_pack_ref: stringField(row, 'receive_pack_ref'),
    repository_ref: stringField(row, 'repository_ref'),
    token_ref: stringField(row, 'token_ref'),
    subject_ref: stringField(row, 'subject_ref'),
    change_ref: nullableStringField(row, 'change_ref'),
    packfile_ref: nullableStringField(row, 'packfile_ref'),
    packfile_sha256: nullableStringField(row, 'packfile_sha256'),
    packfile_bytes: numberField(row, 'packfile_bytes'),
    object_format: decodeObjectFormat(stringField(row, 'object_format')),
    state: stringField(row, 'state') === 'accepted' ? 'accepted' : 'rejected',
    command_count: numberField(row, 'command_count'),
    ref_updates_json: stringField(row, 'ref_updates_json'),
    source_refs_json: stringField(row, 'source_refs_json'),
    rejection_code: nullableStringField(row, 'rejection_code'),
    rejection_reason: nullableStringField(row, 'rejection_reason'),
    created_at: stringField(row, 'created_at'),
    updated_at: stringField(row, 'updated_at'),
  }
}

export const isZeroObjectId = (value: string): boolean =>
  value === sha1Zero || value === sha256Zero

export const objectFormatForObjectId = (
  value: string,
): Exclude<ForgeGitPackfileObjectFormat, 'unknown'> => {
  if (sha1Pattern.test(value)) {
    return 'sha1'
  }
  if (sha256Pattern.test(value)) {
    return 'sha256'
  }
  throw new ForgeGitCanonicalStoreError(
    'forge_git_invalid_object_id',
    'git object id must be a SHA-1 or SHA-256 hex string',
    400,
  )
}

export const validateSha256 = (value: string): string => {
  if (!sha256Pattern.test(value)) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_packfile',
      'packfile digest must be a SHA-256 hex string',
      400,
    )
  }
  return value.toLowerCase()
}

export const normalizeObjectId = (value: string): string =>
  objectFormatForObjectId(value) === 'sha1'
    ? value.toLowerCase()
    : value.toLowerCase()

export const resolveObjectFormat = (
  declaredFormat: ForgeGitPackfileObjectFormat,
  refUpdates: ReadonlyArray<ForgeGitPackfileRefUpdate>,
): Exclude<ForgeGitPackfileObjectFormat, 'unknown'> => {
  const formats = new Set(
    refUpdates.flatMap(update => [
      objectFormatForObjectId(update.oldObjectId),
      objectFormatForObjectId(update.newObjectId),
    ]),
  )
  if (formats.size !== 1) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      'all receive-pack commands must use one object id width',
      400,
    )
  }
  const inferred = [...formats][0]!
  if (declaredFormat !== 'unknown' && declaredFormat !== inferred) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      'receive-pack object-format capability does not match command ids',
      400,
    )
  }
  return inferred
}

export const safeRefUpdateTarget = (refName: string): boolean =>
  refName.startsWith('refs/heads/') || refName.startsWith('refs/tags/')

export const zeroObjectIdForFormat = (
  format: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
): string => (format === 'sha1' ? sha1Zero : sha256Zero)

const runChanges = (result: unknown): number | undefined => {
  if (!isRecord(result) || !isRecord(result.meta)) {
    return undefined
  }
  const changes = result.meta.changes
  return typeof changes === 'number' ? changes : undefined
}

const rowOrFail = <T>(row: T | null | undefined, label: string): T => {
  if (row === null || row === undefined) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      `${label} was not persisted`,
      500,
    )
  }
  return row
}

const validatePackfile = (input: ForgeGitCanonicalPreflightInput): void => {
  validateSha256(input.packfileSha256)
  if (!Number.isInteger(input.packfileBytes) || input.packfileBytes <= 0) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_packfile',
      'object-writing receive-pack request must include a non-empty packfile',
      400,
    )
  }
}

export const validateReceivePackShape = (
  input: ForgeGitCanonicalPreflightInput,
): Exclude<ForgeGitPackfileObjectFormat, 'unknown'> => {
  if (input.refUpdates.length === 0) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_packfile',
      'receive-pack request must include at least one ref update',
      400,
    )
  }
  if (input.refUpdates.every(update => update.action === 'delete')) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_delete_only_push_rejected',
      'delete-only receive-pack pushes are rejected by Forge intake',
      400,
    )
  }
  validatePackfile(input)

  const seenRefs = new Set<string>()
  for (const update of input.refUpdates) {
    const normalizedOld = normalizeObjectId(update.oldObjectId)
    const normalizedNew = normalizeObjectId(update.newObjectId)
    if (
      normalizedOld !== update.oldObjectId ||
      normalizedNew !== update.newObjectId
    ) {
      throw new ForgeGitCanonicalStoreError(
        'forge_git_invalid_object_id',
        'receive-pack object ids must be normalized lowercase hex',
        400,
      )
    }
    if (!safeRefUpdateTarget(update.refName)) {
      throw new ForgeGitCanonicalStoreError(
        'forge_git_unsafe_ref_update',
        'Forge receive-pack only accepts refs/heads/* and refs/tags/* updates',
        409,
      )
    }
    if (seenRefs.has(update.refName)) {
      throw new ForgeGitCanonicalStoreError(
        'forge_git_duplicate_ref_update',
        'receive-pack request updates the same ref more than once',
        409,
      )
    }
    seenRefs.add(update.refName)
  }

  return resolveObjectFormat(input.objectFormat, input.refUpdates)
}

const readRefRow = async (
  db: D1Database,
  tenantRef: string,
  repositoryRef: string,
  refName: string,
): Promise<ForgeGitCanonicalRefRow | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_git_refs
        WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?
        LIMIT 1
      `,
    )
    .bind(tenantRef, repositoryRef, refName)
    .first()
  return row === null ? undefined : decodeRefRow(row)
}

const validateRefPreconditions = async (
  db: D1Database,
  input: ForgeGitCanonicalPreflightInput,
): Promise<Exclude<ForgeGitPackfileObjectFormat, 'unknown'>> => {
  const objectFormat = validateReceivePackShape(input)
  const zeroObjectId = zeroObjectIdForFormat(objectFormat)

  for (const update of input.refUpdates) {
    const current = await readRefRow(
      db,
      input.tenantRef,
      input.repositoryRef,
      update.refName,
    )
    const currentObjectId =
      current === undefined || current.state === 'deleted'
        ? zeroObjectId
        : current.object_id

    if (update.action === 'create') {
      if (update.oldObjectId !== zeroObjectId || currentObjectId !== zeroObjectId) {
        throw new ForgeGitCanonicalStoreError(
          'forge_git_unsafe_ref_update',
          `unsafe create for ${update.refName}: expected an empty ref`,
          409,
        )
      }
      continue
    }

    if (currentObjectId !== update.oldObjectId) {
      throw new ForgeGitCanonicalStoreError(
        'forge_git_unsafe_ref_update',
        `unsafe ${update.action} for ${update.refName}: old object id does not match the canonical ref`,
        409,
      )
    }
  }

  return objectFormat
}

const insertHeldLock = async (
  db: D1Database,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
) => {
  const lockRef = `${input.receivePackRef}.${safeRefSegment(update.refName)}`
  try {
    await db
      .prepare(
        `
          INSERT INTO forge_git_ref_locks (
            tenant_ref,
            lock_ref,
            repository_ref,
            ref_name,
            receive_pack_ref,
            expected_old_object_id,
            new_object_id,
            action,
            state,
            acquired_at,
            released_at,
            source_refs_json
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'held', ?, NULL, ?)
        `,
      )
      .bind(
        input.tenantRef,
        lockRef,
        input.repositoryRef,
        update.refName,
        input.receivePackRef,
        update.oldObjectId,
        update.newObjectId,
        update.action,
        input.nowIso,
        jsonArray(input.sourceRefs),
      )
      .run()
  } catch {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_ref_lock_conflict',
      `canonical ref lock is already held for ${update.refName}`,
      409,
    )
  }
  return lockRef
}

const releaseLocks = async (
  db: D1Database,
  tenantRef: string,
  lockRefs: ReadonlyArray<string>,
  state: 'applied' | 'rejected',
  nowIso: string,
) => {
  for (const lockRef of lockRefs) {
    await db
      .prepare(
        `
          UPDATE forge_git_ref_locks
          SET state = ?, released_at = ?
          WHERE tenant_ref = ? AND lock_ref = ? AND state = 'held'
        `,
      )
      .bind(state, nowIso, tenantRef, lockRef)
      .run()
  }
}

const insertObjectTip = async (
  db: D1Database,
  input: ForgeGitCanonicalReceivePackInput,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
  objectId: string,
) => {
  await db
    .prepare(
      `
        INSERT INTO forge_git_objects (
          tenant_ref,
          repository_ref,
          object_id,
          object_format,
          packfile_ref,
          packfile_sha256,
          first_seen_at,
          latest_seen_at,
          source_refs_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (tenant_ref, repository_ref, object_id) DO UPDATE SET
          packfile_ref = excluded.packfile_ref,
          packfile_sha256 = excluded.packfile_sha256,
          latest_seen_at = excluded.latest_seen_at,
          source_refs_json = excluded.source_refs_json
      `,
    )
    .bind(
      input.tenantRef,
      input.repositoryRef,
      objectId,
      objectFormat,
      input.packfileRef,
      validateSha256(input.packfileSha256),
      input.nowIso,
      input.nowIso,
      jsonArray(input.sourceRefs),
    )
    .run()
}

const applyCreate = async (
  db: D1Database,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
) => {
  const result = await db
    .prepare(
      `
        INSERT INTO forge_git_refs (
          tenant_ref,
          repository_ref,
          ref_name,
          object_id,
          previous_object_id,
          object_format,
          state,
          updated_by_change_ref,
          updated_by_packfile_ref,
          updated_by_receive_pack_ref,
          source_refs_json,
          created_at,
          updated_at
        ) VALUES (?, ?, ?, ?, NULL, ?, 'active', ?, ?, ?, ?, ?, ?)
        ON CONFLICT (tenant_ref, repository_ref, ref_name) DO UPDATE SET
          object_id = excluded.object_id,
          previous_object_id = NULL,
          object_format = excluded.object_format,
          state = 'active',
          updated_by_change_ref = excluded.updated_by_change_ref,
          updated_by_packfile_ref = excluded.updated_by_packfile_ref,
          updated_by_receive_pack_ref = excluded.updated_by_receive_pack_ref,
          source_refs_json = excluded.source_refs_json,
          updated_at = excluded.updated_at
        WHERE forge_git_refs.state = 'deleted'
      `,
    )
    .bind(
      input.tenantRef,
      input.repositoryRef,
      update.refName,
      update.newObjectId,
      objectFormat,
      input.changeRef,
      input.packfileRef,
      input.receivePackRef,
      jsonArray(input.sourceRefs),
      input.nowIso,
      input.nowIso,
    )
    .run()

  if ((runChanges(result) ?? 1) < 1) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      `unsafe create for ${update.refName}: canonical ref changed before apply`,
      409,
    )
  }
}

const applyUpdate = async (
  db: D1Database,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
) => {
  const result = await db
    .prepare(
      `
        UPDATE forge_git_refs
        SET
          previous_object_id = object_id,
          object_id = ?,
          object_format = ?,
          state = 'active',
          updated_by_change_ref = ?,
          updated_by_packfile_ref = ?,
          updated_by_receive_pack_ref = ?,
          source_refs_json = ?,
          updated_at = ?
        WHERE tenant_ref = ?
          AND repository_ref = ?
          AND ref_name = ?
          AND object_id = ?
          AND state = 'active'
      `,
    )
    .bind(
      update.newObjectId,
      objectFormat,
      input.changeRef,
      input.packfileRef,
      input.receivePackRef,
      jsonArray(input.sourceRefs),
      input.nowIso,
      input.tenantRef,
      input.repositoryRef,
      update.refName,
      update.oldObjectId,
    )
    .run()

  if ((runChanges(result) ?? 1) < 1) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      `unsafe update for ${update.refName}: canonical ref changed before apply`,
      409,
    )
  }
}

const applyDelete = async (
  db: D1Database,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
) => {
  const result = await db
    .prepare(
      `
        UPDATE forge_git_refs
        SET
          previous_object_id = object_id,
          object_id = NULL,
          object_format = ?,
          state = 'deleted',
          updated_by_change_ref = ?,
          updated_by_packfile_ref = ?,
          updated_by_receive_pack_ref = ?,
          source_refs_json = ?,
          updated_at = ?
        WHERE tenant_ref = ?
          AND repository_ref = ?
          AND ref_name = ?
          AND object_id = ?
          AND state = 'active'
      `,
    )
    .bind(
      objectFormat,
      input.changeRef,
      input.packfileRef,
      input.receivePackRef,
      jsonArray(input.sourceRefs),
      input.nowIso,
      input.tenantRef,
      input.repositoryRef,
      update.refName,
      update.oldObjectId,
    )
    .run()

  if ((runChanges(result) ?? 1) < 1) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      `unsafe delete for ${update.refName}: canonical ref changed before apply`,
      409,
    )
  }
}

const applyRefUpdate = async (
  db: D1Database,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
) => {
  if (update.action === 'create') {
    await applyCreate(db, input, update, objectFormat)
    return
  }
  if (update.action === 'update') {
    await applyUpdate(db, input, update, objectFormat)
    return
  }
  await applyDelete(db, input, update, objectFormat)
}

const recordAcceptedIntake = async (
  db: D1Database,
  input: ForgeGitCanonicalReceivePackInput,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
) => {
  await db
    .prepare(
      `
        INSERT INTO forge_git_receive_pack_intakes (
          tenant_ref,
          receive_pack_ref,
          repository_ref,
          token_ref,
          subject_ref,
          change_ref,
          packfile_ref,
          packfile_sha256,
          packfile_bytes,
          object_format,
          state,
          command_count,
          ref_updates_json,
          source_refs_json,
          rejection_code,
          rejection_reason,
          created_at,
          updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'accepted', ?, ?, ?, NULL, NULL, ?, ?)
        ON CONFLICT (tenant_ref, receive_pack_ref) DO UPDATE SET
          repository_ref = excluded.repository_ref,
          token_ref = excluded.token_ref,
          subject_ref = excluded.subject_ref,
          change_ref = excluded.change_ref,
          packfile_ref = excluded.packfile_ref,
          packfile_sha256 = excluded.packfile_sha256,
          packfile_bytes = excluded.packfile_bytes,
          object_format = excluded.object_format,
          state = 'accepted',
          command_count = excluded.command_count,
          ref_updates_json = excluded.ref_updates_json,
          source_refs_json = excluded.source_refs_json,
          rejection_code = NULL,
          rejection_reason = NULL,
          updated_at = excluded.updated_at
      `,
    )
    .bind(
      input.tenantRef,
      input.receivePackRef,
      input.repositoryRef,
      input.tokenRef,
      input.subjectRef,
      input.changeRef,
      input.packfileRef,
      validateSha256(input.packfileSha256),
      input.packfileBytes,
      objectFormat,
      input.refUpdates.length,
      jsonArray(input.refUpdates),
      jsonArray(input.sourceRefs),
      input.nowIso,
      input.nowIso,
    )
    .run()
}

const insertExternalObjectTip = async (
  db: D1Database,
  input: ForgeGitCanonicalExternalRefImportInput,
) => {
  await db
    .prepare(
      `
        INSERT INTO forge_git_objects (
          tenant_ref,
          repository_ref,
          object_id,
          object_format,
          packfile_ref,
          packfile_sha256,
          first_seen_at,
          latest_seen_at,
          source_refs_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (tenant_ref, repository_ref, object_id) DO UPDATE SET
          packfile_ref = excluded.packfile_ref,
          packfile_sha256 = excluded.packfile_sha256,
          latest_seen_at = excluded.latest_seen_at,
          source_refs_json = excluded.source_refs_json
      `,
    )
    .bind(
      input.tenantRef,
      input.repositoryRef,
      input.objectId,
      input.objectFormat,
      input.packfileRef,
      validateSha256(input.sourceDigestSha256),
      input.nowIso,
      input.nowIso,
      jsonArray(input.sourceRefs),
    )
    .run()
}

const importExternalRef = async (
  db: D1Database,
  input: ForgeGitCanonicalExternalRefImportInput,
): Promise<ForgeGitCanonicalExternalRefImportResult> => {
  if (!safeRefUpdateTarget(input.refName)) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      'Forge external imports only accept refs/heads/* and refs/tags/* targets',
      409,
    )
  }
  if (objectFormatForObjectId(input.objectId) !== input.objectFormat) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      'external import object id does not match its declared format',
      400,
    )
  }
  if (normalizeObjectId(input.objectId) !== input.objectId) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      'external import object id must be normalized lowercase hex',
      400,
    )
  }

  await insertExternalObjectTip(db, input)

  const result = await db
    .prepare(
      `
        INSERT INTO forge_git_refs (
          tenant_ref,
          repository_ref,
          ref_name,
          object_id,
          previous_object_id,
          object_format,
          state,
          updated_by_change_ref,
          updated_by_packfile_ref,
          updated_by_receive_pack_ref,
          source_refs_json,
          created_at,
          updated_at
        ) VALUES (?, ?, ?, ?, NULL, ?, 'active', ?, ?, ?, ?, ?, ?)
        ON CONFLICT (tenant_ref, repository_ref, ref_name) DO UPDATE SET
          previous_object_id = CASE
            WHEN forge_git_refs.object_id = excluded.object_id THEN forge_git_refs.previous_object_id
            ELSE forge_git_refs.object_id
          END,
          object_id = excluded.object_id,
          object_format = excluded.object_format,
          state = 'active',
          updated_by_change_ref = excluded.updated_by_change_ref,
          updated_by_packfile_ref = excluded.updated_by_packfile_ref,
          updated_by_receive_pack_ref = excluded.updated_by_receive_pack_ref,
          source_refs_json = excluded.source_refs_json,
          updated_at = excluded.updated_at
        WHERE forge_git_refs.state = 'deleted'
          OR forge_git_refs.object_id IS NOT excluded.object_id
          OR forge_git_refs.object_format IS NOT excluded.object_format
      `,
    )
    .bind(
      input.tenantRef,
      input.repositoryRef,
      input.refName,
      input.objectId,
      input.objectFormat,
      input.changeRef,
      input.packfileRef,
      input.receivePackRef,
      jsonArray(input.sourceRefs),
      input.nowIso,
      input.nowIso,
    )
    .run()

  return {
    changed: (runChanges(result) ?? 1) > 0,
    ref: rowOrFail(
      await readRefRow(db, input.tenantRef, input.repositoryRef, input.refName),
      'forge git external import ref',
    ),
    object: rowOrFail(
      await readObjectRow(db, input.tenantRef, input.repositoryRef, input.objectId),
      'forge git external import object',
    ),
  }
}

const readIntake = async (
  db: D1Database,
  tenantRef: string,
  receivePackRef: string,
): Promise<ForgeGitReceivePackIntakeRow> =>
  decodeIntakeRow(
    rowOrFail(
      await db
        .prepare(
          `
            SELECT *
            FROM forge_git_receive_pack_intakes
            WHERE tenant_ref = ? AND receive_pack_ref = ?
            LIMIT 1
          `,
        )
        .bind(tenantRef, receivePackRef)
        .first(),
      'forge git receive-pack intake',
    ),
  )

const readObjectRow = async (
  db: D1Database,
  tenantRef: string,
  repositoryRef: string,
  objectId: string,
): Promise<ForgeGitCanonicalObjectRow | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_git_objects
        WHERE tenant_ref = ? AND repository_ref = ? AND object_id = ?
        LIMIT 1
      `,
    )
    .bind(tenantRef, repositoryRef, objectId)
    .first()
  return row === null ? undefined : decodeObjectRow(row)
}

export const makeD1ForgeGitCanonicalStore = (
  db: D1Database,
): ForgeGitCanonicalStore => ({
  preflightReceivePack: input => validateRefPreconditions(db, input),

  importExternalRef: input => importExternalRef(db, input),

  async applyReceivePack(input) {
    const objectFormat = await validateRefPreconditions(db, input)
    const lockRefs: string[] = []

    try {
      for (const update of [...input.refUpdates].sort((left, right) =>
        left.refName.localeCompare(right.refName),
      )) {
        lockRefs.push(await insertHeldLock(db, input, update))
      }

      for (const update of input.refUpdates) {
        await applyRefUpdate(db, input, update, objectFormat)
      }

      for (const update of input.refUpdates) {
        if (!isZeroObjectId(update.newObjectId)) {
          await insertObjectTip(db, input, objectFormat, update.newObjectId)
        }
      }

      await releaseLocks(db, input.tenantRef, lockRefs, 'applied', input.nowIso)
      await recordAcceptedIntake(db, input, objectFormat)

      const refs = await Promise.all(
        input.refUpdates.map(update =>
          readRefRow(db, input.tenantRef, input.repositoryRef, update.refName),
        ),
      )
      const objects = await Promise.all(
        input.refUpdates
          .filter(update => !isZeroObjectId(update.newObjectId))
          .map(update =>
            readObjectRow(
              db,
              input.tenantRef,
              input.repositoryRef,
              update.newObjectId,
            ),
          ),
      )

      return {
        intake: await readIntake(db, input.tenantRef, input.receivePackRef),
        objectFormat,
        refs: refs.filter(ref => ref !== undefined),
        objects: objects.filter(object => object !== undefined),
      }
    } catch (error) {
      await releaseLocks(db, input.tenantRef, lockRefs, 'rejected', input.nowIso)
      throw error
    }
  },

  readRef: (tenantRef, repositoryRef, refName) =>
    readRefRow(db, tenantRef, repositoryRef, refName),

  async listRefs(tenantRef, repositoryRef, input) {
    const limit = boundedLimit(input?.limit)
    if (input?.state !== undefined) {
      const rows = await db
        .prepare(
          `
            SELECT *
            FROM forge_git_refs
            WHERE tenant_ref = ? AND repository_ref = ? AND state = ?
            ORDER BY ref_name ASC
            LIMIT ?
          `,
        )
        .bind(tenantRef, repositoryRef, input.state, limit)
        .all()
      return (rows.results ?? []).map(row => decodeRefRow(row))
    }

    const rows = await db
      .prepare(
        `
          SELECT *
          FROM forge_git_refs
          WHERE tenant_ref = ? AND repository_ref = ?
          ORDER BY ref_name ASC
          LIMIT ?
        `,
      )
      .bind(tenantRef, repositoryRef, limit)
      .all()
    return (rows.results ?? []).map(row => decodeRefRow(row))
  },

  readObject: (tenantRef, repositoryRef, objectId) =>
    readObjectRow(db, tenantRef, repositoryRef, objectId),
})
