// KS-8.16 follow-up (#8358): the Forge ref-lock protocol PORT onto real
// Postgres `SELECT ... FOR UPDATE` transactions (MIGRATION_PLAN.md §3.13).
//
// THIS FILE IS NOT WIRED TO PRODUCTION AUTHORITY. It is the tested
// mechanism the read/write cutover will adopt once the compare-mode soak
// (`docs/khala-sync/RUNBOOK.md` "Forge domain cutover" step 5) is silent
// over a representative window — that flip, plus re-adding the six
// deliberately-unported uniques (active-lease-per-work,
// held-lock-per-ref [made moot below], token-hash, packfile digest,
// mirror-destination tuple, github-issue-number/change_ref) and moving
// write authority, is tracked as remaining work on this issue. Landing the
// mechanism now, fully tested and NOT live, lets the correctness-critical
// locking design get reviewed and proven in isolation before it is ever on
// the write path for a real git ref.
//
// THE D1 DANCE THIS REPLACES: `forge-git-canonical-store.ts`
// (`makeD1ForgeGitCanonicalStore`) inserts a `forge_git_ref_locks` row
// with `state='held'` per ref (a partial UNIQUE index on
// `(tenant_ref, repository_ref, ref_name) WHERE state='held'` is the
// actual mutex), then applies the ref CAS update, then flips the lock row
// to `applied`/`rejected`. That is a manually-bookkept advisory lock
// implemented as data — appropriate for D1 (no cross-statement
// transactions), wrong for Postgres, which has real locking primitives.
//
// THE PORT: one Postgres transaction per `applyReceivePack` call.
//
//   1. For every ref in the receive-pack, in the SAME sorted order the D1
//      lane already uses (`ref_name` ascending — the collision-avoidance
//      rule that keeps concurrent multi-ref pushes from deadlocking each
//      other), take `pg_advisory_xact_lock(hashtextextended(<ref key>, 0))`.
//      This is the REAL mutex: two concurrent `applyReceivePack` calls
//      touching the same (tenant, repository, ref) serialize HERE, and the
//      lock releases itself at COMMIT or ROLLBACK — no lock row, no
//      manual release, no leaked "held forever" state after a crash.
//      Advisory locks are the correct primitive (rather than a plain
//      `SELECT ... FOR UPDATE`) for the 'create' case specifically:
//      a not-yet-existing ref has no row to select, so nothing to hold a
//      row lock on, but the advisory lock still serializes two concurrent
//      creates of the same brand-new ref.
//   2. THEN, for refs that already have a `forge_git_refs` row (update /
//      delete), the precondition check runs a REAL
//      `SELECT * FROM forge_git_refs WHERE ... FOR UPDATE` — the literal
//      mechanism MIGRATION_PLAN §3.13 names — which both locks the row
//      for the rest of the transaction and returns its current state for
//      the compare-and-swap check, atomically. Combined with step 1, a
//      concurrent second pusher can never observe a stale `old_object_id`
//      between the read and the write, for either a brand-new ref or an
//      existing one.
//   3. Ref/object/intake writes happen with `RETURNING *`, so the applied
//      row comes back from the same statement that changed it — no
//      separate read-after-write race, no `runChanges` row-count parsing.
//   4. On ANY error, the transaction rolls back — the advisory locks and
//      the row lock both release automatically; there is no `rejected`
//      lock row to write because there is no lock row at all.
//
// Every other rule (delete-only-push rejection, one object-id width per
// push, refs/heads|refs/tags-only targets, duplicate-ref-in-one-push
// rejection, the R2-only packfile-bytes rule) is the SAME pure validation
// already proven on D1 — reused verbatim via the exports added to
// `forge-git-canonical-store.ts` rather than re-implemented, so the two
// engines can never drift on what a "safe" ref update means.

import type { ForgeGitPackfileObjectFormat } from '@openagentsinc/forge-protocol'
import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'

import {
  boundedLimit,
  isZeroObjectId,
  jsonArray,
  safeRefUpdateTarget,
  validateReceivePackShape,
  validateSha256,
  zeroObjectIdForFormat,
  ForgeGitCanonicalStoreError,
  type ForgeGitCanonicalApplyResult,
  type ForgeGitCanonicalExternalRefImportInput,
  type ForgeGitCanonicalExternalRefImportResult,
  type ForgeGitCanonicalObjectRow,
  type ForgeGitCanonicalPreflightInput,
  type ForgeGitCanonicalReceivePackInput,
  type ForgeGitCanonicalRefRow,
  type ForgeGitCanonicalRefState,
  type ForgeGitCanonicalStore,
  type ForgeGitReceivePackIntakeRow,
} from './forge-git-canonical-store'
import type { ForgeGitPackfileRefUpdate } from './forge-git-packfile-archive-store'

// ---------------------------------------------------------------------------
// Row decoding (postgres.js/Bun-SQL row shapes — text columns need no
// bigint normalization on this table family; `forge_git_receive_pack_intakes`
// carries the two bigint columns, `packfile_bytes`/`command_count`).
// ---------------------------------------------------------------------------

const decodeRefRow = (row: Record<string, unknown>): ForgeGitCanonicalRefRow => ({
  created_at: String(row['created_at']),
  object_format: row['object_format'] as ForgeGitPackfileObjectFormat,
  object_id: row['object_id'] === null ? null : String(row['object_id']),
  previous_object_id:
    row['previous_object_id'] === null ? null : String(row['previous_object_id']),
  ref_name: String(row['ref_name']),
  repository_ref: String(row['repository_ref']),
  source_refs_json: String(row['source_refs_json']),
  state: row['state'] as ForgeGitCanonicalRefState,
  tenant_ref: String(row['tenant_ref']),
  updated_at: String(row['updated_at']),
  updated_by_change_ref: String(row['updated_by_change_ref']),
  updated_by_packfile_ref: String(row['updated_by_packfile_ref']),
  updated_by_receive_pack_ref: String(row['updated_by_receive_pack_ref']),
})

const decodeObjectRow = (
  row: Record<string, unknown>,
): ForgeGitCanonicalObjectRow => ({
  first_seen_at: String(row['first_seen_at']),
  latest_seen_at: String(row['latest_seen_at']),
  object_format: row['object_format'] as Exclude<
    ForgeGitPackfileObjectFormat,
    'unknown'
  >,
  object_id: String(row['object_id']),
  packfile_ref: String(row['packfile_ref']),
  packfile_sha256: String(row['packfile_sha256']),
  repository_ref: String(row['repository_ref']),
  source_refs_json: String(row['source_refs_json']),
  tenant_ref: String(row['tenant_ref']),
})

const decodeIntakeRow = (
  row: Record<string, unknown>,
): ForgeGitReceivePackIntakeRow => ({
  change_ref: row['change_ref'] === null ? null : String(row['change_ref']),
  command_count: Number(row['command_count']),
  created_at: String(row['created_at']),
  object_format: row['object_format'] as ForgeGitPackfileObjectFormat,
  packfile_bytes: Number(row['packfile_bytes']),
  packfile_ref: row['packfile_ref'] === null ? null : String(row['packfile_ref']),
  packfile_sha256:
    row['packfile_sha256'] === null ? null : String(row['packfile_sha256']),
  receive_pack_ref: String(row['receive_pack_ref']),
  ref_updates_json: String(row['ref_updates_json']),
  rejection_code:
    row['rejection_code'] === null ? null : String(row['rejection_code']),
  rejection_reason:
    row['rejection_reason'] === null ? null : String(row['rejection_reason']),
  repository_ref: String(row['repository_ref']),
  source_refs_json: String(row['source_refs_json']),
  state: row['state'] === 'accepted' ? 'accepted' : 'rejected',
  subject_ref: String(row['subject_ref']),
  tenant_ref: String(row['tenant_ref']),
  token_ref: String(row['token_ref']),
  updated_at: String(row['updated_at']),
})

const rowOrFail = <T>(rows: ReadonlyArray<T>, label: string): T => {
  const row = rows[0]
  if (row === undefined) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_invalid_object_id',
      `${label} was not persisted`,
      500,
    )
  }
  return row
}

// ---------------------------------------------------------------------------
// The advisory-lock key — a real Postgres transaction-scoped mutex over a
// ref that may not have a row yet (the 'create' case).
// ---------------------------------------------------------------------------

const refLockKey = (
  tenantRef: string,
  repositoryRef: string,
  refName: string,
): string => `forge_git_ref:${tenantRef}:${repositoryRef}:${refName}`

const acquireRefLock = async (
  tx: SyncTransactionSql,
  tenantRef: string,
  repositoryRef: string,
  refName: string,
): Promise<void> => {
  await tx`SELECT pg_advisory_xact_lock(hashtextextended(${refLockKey(tenantRef, repositoryRef, refName)}, 0))`
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

const readRefRow = async (
  sql: SyncSql | SyncTransactionSql,
  tenantRef: string,
  repositoryRef: string,
  refName: string,
): Promise<ForgeGitCanonicalRefRow | undefined> => {
  const rows: Array<Record<string, unknown>> = await sql`
    SELECT * FROM forge_git_refs
     WHERE tenant_ref = ${tenantRef}
       AND repository_ref = ${repositoryRef}
       AND ref_name = ${refName}
     LIMIT 1
  `
  return rows[0] === undefined ? undefined : decodeRefRow(rows[0])
}

/** The literal §3.13 mechanism: locks the row (if it exists) AND returns
 * its current state in one atomic statement, inside the caller's
 * transaction — the CAS precondition check below reads a state that
 * cannot change out from under it until this transaction ends. */
const readRefRowForUpdate = async (
  tx: SyncTransactionSql,
  tenantRef: string,
  repositoryRef: string,
  refName: string,
): Promise<ForgeGitCanonicalRefRow | undefined> => {
  const rows: Array<Record<string, unknown>> = await tx`
    SELECT * FROM forge_git_refs
     WHERE tenant_ref = ${tenantRef}
       AND repository_ref = ${repositoryRef}
       AND ref_name = ${refName}
     LIMIT 1
     FOR UPDATE
  `
  return rows[0] === undefined ? undefined : decodeRefRow(rows[0])
}

const readObjectRow = async (
  sql: SyncSql | SyncTransactionSql,
  tenantRef: string,
  repositoryRef: string,
  objectId: string,
): Promise<ForgeGitCanonicalObjectRow | undefined> => {
  const rows: Array<Record<string, unknown>> = await sql`
    SELECT * FROM forge_git_objects
     WHERE tenant_ref = ${tenantRef}
       AND repository_ref = ${repositoryRef}
       AND object_id = ${objectId}
     LIMIT 1
  `
  return rows[0] === undefined ? undefined : decodeObjectRow(rows[0])
}

const readIntake = async (
  sql: SyncSql | SyncTransactionSql,
  tenantRef: string,
  receivePackRef: string,
): Promise<ForgeGitReceivePackIntakeRow> => {
  const rows: Array<Record<string, unknown>> = await sql`
    SELECT * FROM forge_git_receive_pack_intakes
     WHERE tenant_ref = ${tenantRef} AND receive_pack_ref = ${receivePackRef}
     LIMIT 1
  `
  return decodeIntakeRow(
    rowOrFail(rows, 'forge git receive-pack intake'),
  )
}

// ---------------------------------------------------------------------------
// Preflight (no lock — same as the D1 lane: an early, best-effort
// precondition signal before the packfile even uploads; `applyReceivePack`
// re-validates for real, under lock, below)
// ---------------------------------------------------------------------------

const validateRefPreconditions = async (
  sql: SyncSql,
  input: ForgeGitCanonicalPreflightInput,
): Promise<Exclude<ForgeGitPackfileObjectFormat, 'unknown'>> => {
  const objectFormat = validateReceivePackShape(input)
  const zeroObjectId = zeroObjectIdForFormat(objectFormat)

  for (const update of input.refUpdates) {
    const current = await readRefRow(
      sql,
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

/** The SAME check, but every read is `SELECT ... FOR UPDATE` inside the
 * caller's transaction (after the advisory locks are already held) — the
 * real compare-and-swap re-validation `applyReceivePack` runs under lock. */
const validateRefPreconditionsForUpdate = async (
  tx: SyncTransactionSql,
  input: ForgeGitCanonicalPreflightInput,
): Promise<Exclude<ForgeGitPackfileObjectFormat, 'unknown'>> => {
  const objectFormat = validateReceivePackShape(input)
  const zeroObjectId = zeroObjectIdForFormat(objectFormat)

  for (const update of input.refUpdates) {
    const current = await readRefRowForUpdate(
      tx,
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

// ---------------------------------------------------------------------------
// Apply (writes use RETURNING * so the applied row comes back from the
// SAME statement that changed it — no read-after-write race window)
// ---------------------------------------------------------------------------

const applyCreate = async (
  tx: SyncTransactionSql,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
): Promise<void> => {
  const rows: Array<Record<string, unknown>> = await tx`
    INSERT INTO forge_git_refs (
      tenant_ref, repository_ref, ref_name, object_id, previous_object_id,
      object_format, state, updated_by_change_ref, updated_by_packfile_ref,
      updated_by_receive_pack_ref, source_refs_json, created_at, updated_at
    ) VALUES (
      ${input.tenantRef}, ${input.repositoryRef}, ${update.refName},
      ${update.newObjectId}, NULL, ${objectFormat}, 'active',
      ${input.changeRef}, ${input.packfileRef}, ${input.receivePackRef},
      ${jsonArray(input.sourceRefs)}, ${input.nowIso}, ${input.nowIso}
    )
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
    RETURNING ref_name
  `
  if (rows.length < 1) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      `unsafe create for ${update.refName}: canonical ref changed before apply`,
      409,
    )
  }
}

const applyUpdate = async (
  tx: SyncTransactionSql,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
): Promise<void> => {
  const rows: Array<Record<string, unknown>> = await tx`
    UPDATE forge_git_refs
       SET previous_object_id = object_id,
           object_id = ${update.newObjectId},
           object_format = ${objectFormat},
           state = 'active',
           updated_by_change_ref = ${input.changeRef},
           updated_by_packfile_ref = ${input.packfileRef},
           updated_by_receive_pack_ref = ${input.receivePackRef},
           source_refs_json = ${jsonArray(input.sourceRefs)},
           updated_at = ${input.nowIso}
     WHERE tenant_ref = ${input.tenantRef}
       AND repository_ref = ${input.repositoryRef}
       AND ref_name = ${update.refName}
       AND object_id = ${update.oldObjectId}
       AND state = 'active'
    RETURNING ref_name
  `
  if (rows.length < 1) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      `unsafe update for ${update.refName}: canonical ref changed before apply`,
      409,
    )
  }
}

const applyDelete = async (
  tx: SyncTransactionSql,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
): Promise<void> => {
  const rows: Array<Record<string, unknown>> = await tx`
    UPDATE forge_git_refs
       SET previous_object_id = object_id,
           object_id = NULL,
           object_format = ${objectFormat},
           state = 'deleted',
           updated_by_change_ref = ${input.changeRef},
           updated_by_packfile_ref = ${input.packfileRef},
           updated_by_receive_pack_ref = ${input.receivePackRef},
           source_refs_json = ${jsonArray(input.sourceRefs)},
           updated_at = ${input.nowIso}
     WHERE tenant_ref = ${input.tenantRef}
       AND repository_ref = ${input.repositoryRef}
       AND ref_name = ${update.refName}
       AND object_id = ${update.oldObjectId}
       AND state = 'active'
    RETURNING ref_name
  `
  if (rows.length < 1) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      `unsafe delete for ${update.refName}: canonical ref changed before apply`,
      409,
    )
  }
}

const applyRefUpdate = async (
  tx: SyncTransactionSql,
  input: ForgeGitCanonicalReceivePackInput,
  update: ForgeGitPackfileRefUpdate,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
): Promise<void> => {
  if (update.action === 'create') {
    await applyCreate(tx, input, update, objectFormat)
    return
  }
  if (update.action === 'update') {
    await applyUpdate(tx, input, update, objectFormat)
    return
  }
  await applyDelete(tx, input, update, objectFormat)
}

const insertObjectTip = async (
  tx: SyncTransactionSql,
  input: ForgeGitCanonicalReceivePackInput,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
  objectId: string,
): Promise<void> => {
  await tx`
    INSERT INTO forge_git_objects (
      tenant_ref, repository_ref, object_id, object_format, packfile_ref,
      packfile_sha256, first_seen_at, latest_seen_at, source_refs_json
    ) VALUES (
      ${input.tenantRef}, ${input.repositoryRef}, ${objectId}, ${objectFormat},
      ${input.packfileRef}, ${validateSha256(input.packfileSha256)},
      ${input.nowIso}, ${input.nowIso}, ${jsonArray(input.sourceRefs)}
    )
    ON CONFLICT (tenant_ref, repository_ref, object_id) DO UPDATE SET
      packfile_ref = excluded.packfile_ref,
      packfile_sha256 = excluded.packfile_sha256,
      latest_seen_at = excluded.latest_seen_at,
      source_refs_json = excluded.source_refs_json
  `
}

const recordAcceptedIntake = async (
  tx: SyncTransactionSql,
  input: ForgeGitCanonicalReceivePackInput,
  objectFormat: Exclude<ForgeGitPackfileObjectFormat, 'unknown'>,
): Promise<void> => {
  await tx`
    INSERT INTO forge_git_receive_pack_intakes (
      tenant_ref, receive_pack_ref, repository_ref, token_ref, subject_ref,
      change_ref, packfile_ref, packfile_sha256, packfile_bytes,
      object_format, state, command_count, ref_updates_json,
      source_refs_json, rejection_code, rejection_reason, created_at,
      updated_at
    ) VALUES (
      ${input.tenantRef}, ${input.receivePackRef}, ${input.repositoryRef},
      ${input.tokenRef}, ${input.subjectRef}, ${input.changeRef},
      ${input.packfileRef}, ${validateSha256(input.packfileSha256)},
      ${input.packfileBytes}, ${objectFormat}, 'accepted',
      ${input.refUpdates.length}, ${jsonArray(input.refUpdates)},
      ${jsonArray(input.sourceRefs)}, NULL, NULL, ${input.nowIso}, ${input.nowIso}
    )
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
  `
}

const insertExternalObjectTip = async (
  sql: SyncSql | SyncTransactionSql,
  input: ForgeGitCanonicalExternalRefImportInput,
): Promise<void> => {
  await sql`
    INSERT INTO forge_git_objects (
      tenant_ref, repository_ref, object_id, object_format, packfile_ref,
      packfile_sha256, first_seen_at, latest_seen_at, source_refs_json
    ) VALUES (
      ${input.tenantRef}, ${input.repositoryRef}, ${input.objectId},
      ${input.objectFormat}, ${input.packfileRef},
      ${validateSha256(input.sourceDigestSha256)}, ${input.nowIso},
      ${input.nowIso}, ${jsonArray(input.sourceRefs)}
    )
    ON CONFLICT (tenant_ref, repository_ref, object_id) DO UPDATE SET
      packfile_ref = excluded.packfile_ref,
      packfile_sha256 = excluded.packfile_sha256,
      latest_seen_at = excluded.latest_seen_at,
      source_refs_json = excluded.source_refs_json
  `
}

const importExternalRef = async (
  sql: SyncSql,
  input: ForgeGitCanonicalExternalRefImportInput,
): Promise<ForgeGitCanonicalExternalRefImportResult> => {
  if (!safeRefUpdateTarget(input.refName)) {
    throw new ForgeGitCanonicalStoreError(
      'forge_git_unsafe_ref_update',
      'Forge external imports only accept refs/heads/* and refs/tags/* targets',
      409,
    )
  }

  return await sql.begin(async tx => {
    // The advisory lock closes the same not-yet-existing-row gap as the
    // receive-pack path — two concurrent imports of the same brand-new
    // ref serialize here.
    await acquireRefLock(tx, input.tenantRef, input.repositoryRef, input.refName)

    await insertExternalObjectTip(tx, input)

    // The WHERE guard mirrors the D1 lane exactly: a re-import that
    // resolves to the SAME object id/format is a no-op UPDATE (nothing
    // fires, `changed` is false) — an idempotent replay never bumps
    // `updated_at`/`previous_object_id` for identical content.
    const upserted: Array<Record<string, unknown>> = await tx`
      INSERT INTO forge_git_refs (
        tenant_ref, repository_ref, ref_name, object_id, previous_object_id,
        object_format, state, updated_by_change_ref, updated_by_packfile_ref,
        updated_by_receive_pack_ref, source_refs_json, created_at, updated_at
      ) VALUES (
        ${input.tenantRef}, ${input.repositoryRef}, ${input.refName},
        ${input.objectId}, NULL, ${input.objectFormat}, 'active',
        ${input.changeRef}, ${input.packfileRef}, ${input.receivePackRef},
        ${jsonArray(input.sourceRefs)}, ${input.nowIso}, ${input.nowIso}
      )
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
         OR forge_git_refs.object_id IS DISTINCT FROM excluded.object_id
         OR forge_git_refs.object_format IS DISTINCT FROM excluded.object_format
      RETURNING ref_name
    `

    const objectRows: Array<Record<string, unknown>> = await tx`
      SELECT * FROM forge_git_objects
       WHERE tenant_ref = ${input.tenantRef}
         AND repository_ref = ${input.repositoryRef}
         AND object_id = ${input.objectId}
       LIMIT 1
    `
    const refRows: Array<Record<string, unknown>> = await tx`
      SELECT * FROM forge_git_refs
       WHERE tenant_ref = ${input.tenantRef}
         AND repository_ref = ${input.repositoryRef}
         AND ref_name = ${input.refName}
       LIMIT 1
    `

    return {
      changed: upserted.length > 0,
      object: decodeObjectRow(
        rowOrFail(objectRows, 'forge git external import object'),
      ),
      ref: decodeRefRow(rowOrFail(refRows, 'forge git external import ref')),
    }
  })
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/**
 * The Postgres-authoritative canonical Forge git store — real
 * `pg_advisory_xact_lock` + `SELECT ... FOR UPDATE` transactions instead
 * of the D1 held/applied/rejected lock-row dance. NOT wired to any
 * production call site: this is the tested mechanism for the eventual
 * read/write cutover (MIGRATION_PLAN §3.13), landed in isolation so its
 * locking design can be reviewed and proven before it is ever on the
 * write path for a real git ref.
 */
export const makePostgresForgeGitCanonicalStore = (
  sql: SyncSql,
): ForgeGitCanonicalStore => ({
  preflightReceivePack: input => validateRefPreconditions(sql, input),

  importExternalRef: input => importExternalRef(sql, input),

  async applyReceivePack(input): Promise<ForgeGitCanonicalApplyResult> {
    return await sql.begin(async tx => {
      // Sorted lock acquisition order — the same deadlock-avoidance rule
      // as the D1 lane (a multi-ref push always locks refs in the same
      // order regardless of which order the commands were sent in).
      const sorted = [...input.refUpdates].sort((left, right) =>
        left.refName.localeCompare(right.refName),
      )
      for (const update of sorted) {
        await acquireRefLock(
          tx,
          input.tenantRef,
          input.repositoryRef,
          update.refName,
        )
      }

      const objectFormat = await validateRefPreconditionsForUpdate(tx, input)

      for (const update of sorted) {
        await applyRefUpdate(tx, input, update, objectFormat)
      }

      for (const update of input.refUpdates) {
        if (!isZeroObjectId(update.newObjectId)) {
          await insertObjectTip(tx, input, objectFormat, update.newObjectId)
        }
      }

      await recordAcceptedIntake(tx, input, objectFormat)

      const refs = (
        await Promise.all(
          input.refUpdates.map(update =>
            readRefRow(tx, input.tenantRef, input.repositoryRef, update.refName),
          ),
        )
      ).filter((ref): ref is ForgeGitCanonicalRefRow => ref !== undefined)

      const objects = (
        await Promise.all(
          input.refUpdates
            .filter(update => !isZeroObjectId(update.newObjectId))
            .map(update =>
              readObjectRow(
                tx,
                input.tenantRef,
                input.repositoryRef,
                update.newObjectId,
              ),
            ),
        )
      ).filter((object): object is ForgeGitCanonicalObjectRow => object !== undefined)

      return {
        intake: await readIntake(tx, input.tenantRef, input.receivePackRef),
        objectFormat,
        objects,
        refs,
      }
    })
  },

  readRef: (tenantRef, repositoryRef, refName) =>
    readRefRow(sql, tenantRef, repositoryRef, refName),

  async listRefs(tenantRef, repositoryRef, input) {
    const limit = boundedLimit(input?.limit)
    const rows: Array<Record<string, unknown>> =
      input?.state === undefined
        ? await sql`
            SELECT * FROM forge_git_refs
             WHERE tenant_ref = ${tenantRef} AND repository_ref = ${repositoryRef}
             ORDER BY ref_name ASC
             LIMIT ${limit}
          `
        : await sql`
            SELECT * FROM forge_git_refs
             WHERE tenant_ref = ${tenantRef} AND repository_ref = ${repositoryRef}
               AND state = ${input.state}
             ORDER BY ref_name ASC
             LIMIT ${limit}
          `
    return rows.map(row => decodeRefRow(row))
  },

  readObject: (tenantRef, repositoryRef, objectId) =>
    readObjectRow(sql, tenantRef, repositoryRef, objectId),
})
