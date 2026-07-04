import {
  canonicalJson,
  type ChangelogEntry,
  decodeFleetAccountEntity,
  decodeFleetAssignmentEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  encodeFleetAccountEntity,
  encodeFleetAssignmentEntity,
  encodeFleetRunEntity,
  encodeFleetWorkerEntity,
  EntityId,
  EntityType,
  FLEET_ACCOUNT_ENTITY_TYPE,
  FLEET_ASSIGNMENT_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  type FleetAccountEntity,
  type FleetAssignmentEntity,
  type FleetRunEntity,
  fleetRunScope,
  type FleetWorkerEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import type { SyncTransactionWriter } from "./outbox-writer.js"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

/**
 * Fleet cockpit scope projection (KS-6.1, #8302; SPEC §2.1
 * `scope.fleet_run.<id>`, §7 invariants 3 and 9).
 *
 * Helpers that append PUBLIC-SAFE fleet entity upserts/deletes to a fleet
 * run's scope inside a `withSyncTransaction` writer, plus:
 *
 * - `ensureScopeOwner` — first-writer-wins scope ownership
 *   (`khala_sync_scope_owners`), written on the scope's first projection
 *   append; the fleet mutators and the v1 read gate consult it.
 * - `canReadScopeV1` — the v1 scope-read gate: a user reads their own
 *   personal scope and the fleet_run scopes they own. Exported so Worker
 *   read routes (KS-4.4 lane) can adopt it; when the shared scope-auth
 *   seam (KS-7) lands, this becomes one arm of it.
 * - `projectFleetEntitiesBestEffort` — the FAIL-SOFT wrapper for the v1
 *   dual-write call site (see below).
 *
 * REDACTION BOUNDARY (invariant 9): every post-image produced here goes
 * through an explicit allowlist mapping (`fleetRunPostImage`,
 * `fleetAssignmentPostImage`, …) and is then DECODED through the fleet
 * entity contracts, whose ref patterns structurally refuse emails,
 * filesystem paths, and whitespace. Raw rows are never spread into a
 * post-image. As defense in depth, `assertFleetPostImageRedacted` rejects
 * any serialized post-image matching the forbidden-material pattern.
 *
 * V1 DUAL-WRITE HONESTY: today the authoritative fleet/assignment business
 * writes live in Worker D1 (and Pylon-local SQLite); this projection is a
 * best-effort SECOND write into Khala Sync Postgres performed AFTER the
 * business write commits. A projection failure must therefore never fail
 * the business write — `projectFleetEntitiesBestEffort` swallows
 * everything into a typed diagnostic. KS-8.1 (#8307) migrates the business
 * write into the SAME Postgres transaction as the changelog append, at
 * which point the fail-soft wrapper is retired and invariant 5 holds for
 * fleet state end to end.
 */

// ---------------------------------------------------------------------------
// Scope ownership (khala_sync_scope_owners)
// ---------------------------------------------------------------------------

/**
 * Claim-or-read the owner of `scope`. First writer wins: when no owner row
 * exists this inserts `(scope, userId)`; when one exists the insert is a
 * no-op and the EXISTING owner is returned. Callers must compare the
 * returned owner against their acting user before writing into the scope.
 * Safe inside a mutator transaction: a foreign-owner outcome performs no
 * write (ON CONFLICT DO NOTHING), preserving reject-before-write.
 */
export const ensureScopeOwner = async (
  sql: SqlTag,
  scope: SyncScope,
  userId: string,
): Promise<string> => {
  const inserted: Array<{ owner_user_id: string }> = await sql`
    INSERT INTO khala_sync_scope_owners (scope, owner_user_id)
    VALUES (${scope}, ${userId})
    ON CONFLICT (scope) DO NOTHING
    RETURNING owner_user_id
  `
  const insertedRow = inserted[0]
  if (insertedRow !== undefined) return insertedRow.owner_user_id
  const existing: Array<{ owner_user_id: string }> = await sql`
    SELECT owner_user_id FROM khala_sync_scope_owners WHERE scope = ${scope}
  `
  const row = existing[0]
  if (row === undefined) {
    // Insert conflicted but the row is gone: only possible under a
    // concurrent delete, which nothing does. Surface it honestly.
    throw new Error(`scope owner row vanished for ${scope}`)
  }
  return row.owner_user_id
}

/** Read the owner of `scope`, or null when the scope is unowned. */
export const readScopeOwner = async (
  sql: SqlTag,
  scope: SyncScope,
): Promise<string | null> => {
  const rows: Array<{ owner_user_id: string }> = await sql`
    SELECT owner_user_id FROM khala_sync_scope_owners WHERE scope = ${scope}
  `
  return rows[0]?.owner_user_id ?? null
}

const PERSONAL_SCOPE_PREFIX = "scope.user."
const FLEET_RUN_SCOPE_PREFIX = "scope.fleet_run."

/**
 * v1 scope-read gate (SPEC §7 invariant 7, v1 slice): a user may read
 * their OWN personal scope, and any `fleet_run` scope they own per
 * `khala_sync_scope_owners`. Everything else is denied — team/thread/
 * public scope auth arrives with KS-7.
 */
export const canReadScopeV1 = async (
  sql: SqlTag,
  userId: string,
  scope: SyncScope,
): Promise<boolean> => {
  if (scope.startsWith(PERSONAL_SCOPE_PREFIX)) {
    return scope === `${PERSONAL_SCOPE_PREFIX}${userId}`
  }
  if (scope.startsWith(FLEET_RUN_SCOPE_PREFIX)) {
    const owner = await readScopeOwner(sql, scope)
    return owner !== null && owner === userId
  }
  return false
}

// ---------------------------------------------------------------------------
// Redaction guard (defense in depth behind the contract patterns)
// ---------------------------------------------------------------------------

/**
 * Material that must NEVER appear in a fleet post-image, checked against
 * the canonical serialization as a last line of defense (the contract ref
 * patterns are the structural first line).
 */
export const FLEET_POST_IMAGE_FORBIDDEN_PATTERN =
  /token|apiKey|authorization|\/Users\//i

export class FleetPostImageRedactionError extends Error {
  readonly _tag = "FleetPostImageRedactionError"
  override readonly name = "FleetPostImageRedactionError"
  constructor(readonly entityType: string) {
    // Deliberately does NOT echo the offending value.
    super(
      `refusing to project ${entityType}: serialized post-image matches the ` +
        "forbidden-material pattern (SPEC §7 invariant 9)",
    )
  }
}

const assertFleetPostImageRedacted = (
  entityType: string,
  postImage: unknown,
): void => {
  if (FLEET_POST_IMAGE_FORBIDDEN_PATTERN.test(canonicalJson(postImage))) {
    throw new FleetPostImageRedactionError(entityType)
  }
}

// ---------------------------------------------------------------------------
// Allowlist redaction mappings (raw row shapes → contract entities)
// ---------------------------------------------------------------------------

const toIso = (raw: Date | string): string =>
  raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()

/**
 * Raw fleet-run row shape (the Pylon orchestration store's `FleetRun` /
 * the desktop supervisor's run record). Only the allowlisted fields are
 * read; anything else on the object is ignored by construction.
 */
export interface RawFleetRunRow {
  readonly runRef: string
  readonly state: string
  readonly targetConcurrency: number
  readonly workerKind: string
  readonly startedAt: string | Date | null
  readonly counters?: {
    readonly workUnitsTotal?: number
    readonly activeAssignments?: number
    readonly completedAssignments?: number
    readonly failedAssignments?: number
    readonly blockedAssignments?: number
  }
  readonly updatedAt: string | Date
}

export const fleetRunPostImage = (row: RawFleetRunRow): FleetRunEntity =>
  decodeFleetRunEntity({
    counters: {
      activeAssignments: row.counters?.activeAssignments ?? 0,
      blockedAssignments: row.counters?.blockedAssignments ?? 0,
      completedAssignments: row.counters?.completedAssignments ?? 0,
      failedAssignments: row.counters?.failedAssignments ?? 0,
      workUnitsTotal: row.counters?.workUnitsTotal ?? 0,
    },
    desiredSlots: row.targetConcurrency,
    runId: row.runRef,
    startedAt: row.startedAt === null ? null : toIso(row.startedAt),
    status: row.state,
    updatedAt: toIso(row.updatedAt),
    workerKind: row.workerKind,
  })

/**
 * Raw worker/dispatch-context row shape. `worktreePath`,
 * `assigneeHandle`, and every other private field simply have no mapping.
 */
export interface RawFleetWorkerRow {
  readonly id: string
  readonly status: string
  readonly currentAssignmentRef?: string | null
  readonly accountRefHash?: string | null
  readonly lastHeartbeatAt?: string | Date | null
  readonly updatedAt: string | Date
}

export const fleetWorkerPostImage = (
  row: RawFleetWorkerRow,
): FleetWorkerEntity =>
  decodeFleetWorkerEntity({
    ...(row.currentAssignmentRef == null
      ? {}
      : { assignmentRef: row.currentAssignmentRef }),
    ...(row.accountRefHash == null
      ? {}
      : { accountRefHash: row.accountRefHash }),
    ...(row.lastHeartbeatAt == null
      ? {}
      : { lastProgressAt: toIso(row.lastHeartbeatAt) }),
    phase: row.status,
    updatedAt: toIso(row.updatedAt),
    workerId: row.id,
  })

/**
 * Raw assignment row shape (the Worker's `PylonApiAssignmentRecord`
 * slice). The coding-assignment payload is deliberately NOT an input: it
 * can carry prompts and workspace paths. Callers pass only the public-safe
 * scalars; `issueRef` must already be the `#N` / `owner/repo#N` form.
 */
export interface RawFleetAssignmentRow {
  readonly assignmentRef: string
  readonly state: string
  readonly issueRef?: string | null
  readonly closeoutClass?: string | null
  readonly updatedAt: string | Date
}

const TERMINAL_CLOSEOUT_STATES: ReadonlySet<string> = new Set([
  "accepted_work",
  "rejected",
  "cancelled",
])

export const fleetAssignmentPostImage = (
  row: RawFleetAssignmentRow,
): FleetAssignmentEntity => {
  const closeoutClass =
    row.closeoutClass ??
    (TERMINAL_CLOSEOUT_STATES.has(row.state) ? row.state : null)
  return decodeFleetAssignmentEntity({
    assignmentRef: row.assignmentRef,
    ...(row.issueRef == null ? {} : { issueRef: row.issueRef }),
    ...(closeoutClass === null ? {} : { closeoutClass }),
    status: row.state,
    updatedAt: toIso(row.updatedAt),
  })
}

/** Raw account-readiness row shape (already hash-ref keyed at the source). */
export interface RawFleetAccountRow {
  readonly accountRefHash: string
  readonly readiness: string
  readonly rateLimitClass?: string | null
  readonly updatedAt: string | Date
}

export const fleetAccountPostImage = (
  row: RawFleetAccountRow,
): FleetAccountEntity =>
  decodeFleetAccountEntity({
    accountRefHash: row.accountRefHash,
    ...(row.rateLimitClass == null
      ? {}
      : { rateLimitClass: row.rateLimitClass }),
    readiness: row.readiness,
    updatedAt: toIso(row.updatedAt),
  })

// ---------------------------------------------------------------------------
// Append helpers (inside a SyncTransactionWriter)
// ---------------------------------------------------------------------------

/**
 * Named system writer ref for dual-write projections (SPEC §7 invariant 3:
 * every changelog entry is attributable to a mutation ref OR a named
 * system writer). Operator-mutator appends use `ctx.mutationRef` instead.
 */
export const FLEET_PROJECTION_SYSTEM_REF =
  "system:fleet_projection.pylon_assignment.v1"

export type FleetEntityChange =
  | { readonly kind: "fleet_run"; readonly op: "upsert"; readonly entity: FleetRunEntity }
  | { readonly kind: "fleet_worker"; readonly op: "upsert"; readonly entity: FleetWorkerEntity }
  | { readonly kind: "fleet_assignment"; readonly op: "upsert"; readonly entity: FleetAssignmentEntity }
  | { readonly kind: "fleet_account"; readonly op: "upsert"; readonly entity: FleetAccountEntity }
  | {
      readonly kind:
        | "fleet_run"
        | "fleet_worker"
        | "fleet_assignment"
        | "fleet_account"
      readonly op: "delete"
      readonly entityId: string
    }

const encodeByKind = {
  [FLEET_ACCOUNT_ENTITY_TYPE]: (entity: FleetAccountEntity) =>
    encodeFleetAccountEntity(entity),
  [FLEET_ASSIGNMENT_ENTITY_TYPE]: (entity: FleetAssignmentEntity) =>
    encodeFleetAssignmentEntity(entity),
  [FLEET_RUN_ENTITY_TYPE]: (entity: FleetRunEntity) =>
    encodeFleetRunEntity(entity),
  [FLEET_WORKER_ENTITY_TYPE]: (entity: FleetWorkerEntity) =>
    encodeFleetWorkerEntity(entity),
}

const entityIdOf = (change: FleetEntityChange): string => {
  if (change.op === "delete") return change.entityId
  switch (change.kind) {
    case "fleet_run":
      return change.entity.runId
    case "fleet_worker":
      return change.entity.workerId
    case "fleet_assignment":
      return change.entity.assignmentRef
    case "fleet_account":
      return change.entity.accountRefHash
  }
}

/**
 * Append one fleet entity change to `scope.fleet_run.<runId>` through the
 * transaction writer. Upsert post-images are the ENCODED contract entities
 * (allowlist-mapped upstream) and pass the forbidden-material guard before
 * anything is written.
 */
export const appendFleetEntityChange = async (
  writer: SyncTransactionWriter,
  runId: string,
  change: FleetEntityChange,
  mutationRef: string = FLEET_PROJECTION_SYSTEM_REF,
): Promise<ChangelogEntry> => {
  const scope = fleetRunScope(runId)
  const entityType = EntityType.make(change.kind)
  const entityId = EntityId.make(entityIdOf(change))
  if (change.op === "delete") {
    return writer.appendChange({
      entityId,
      entityType,
      mutationRef,
      op: "delete",
      scope,
    })
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const postImage = encodeByKind[change.kind](change.entity as any)
  assertFleetPostImageRedacted(change.kind, postImage)
  return writer.appendChange({
    entityId,
    entityType,
    mutationRef,
    op: "upsert",
    postImage,
    scope,
  })
}

// ---------------------------------------------------------------------------
// Fail-soft dual-write wrapper (v1 only — retired by KS-8.1)
// ---------------------------------------------------------------------------

export interface FleetProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason:
    | "storage_failed"
    | "redaction_refused"
    | "scope_owned_by_other_user"
    | "projection_failed"
  readonly messageSafe: string
}

export type FleetProjectionOutcome =
  | { readonly ok: true; readonly entries: ReadonlyArray<ChangelogEntry> }
  | { readonly ok: false; readonly diagnostic: FleetProjectionDiagnostic }

export interface ProjectFleetEntitiesInput {
  readonly sql: SyncSql
  /**
   * The user the scope belongs to (for Worker assignment projections:
   * the assignment's `ownerAgentUserId`). Written to
   * `khala_sync_scope_owners` on the scope's first append; a scope already
   * owned by a DIFFERENT user refuses the projection (no cross-user
   * fan-out) with a diagnostic.
   */
  readonly ownerUserId: string
  readonly runId: string
  readonly changes: ReadonlyArray<FleetEntityChange>
  /** Defaults to the named system writer ref. */
  readonly mutationRef?: string
}

/**
 * Project fleet entity changes into `scope.fleet_run.<runId>` in ONE
 * Postgres transaction (scope-owner claim + version allocation + appends),
 * FAIL-SOFT: this function NEVER throws — any failure (connection,
 * constraint, redaction refusal, foreign owner) rolls back the projection
 * transaction and comes back as a typed diagnostic for the caller to log.
 *
 * v1 dual-write contract: the caller invokes this AFTER its authoritative
 * business write (Worker D1) has committed; a projection failure must
 * never fail that business write. KS-8.1 (#8307) replaces this wrapper by
 * moving the business write into the same transaction (invariant 5).
 */
export const projectFleetEntitiesBestEffort = async (
  input: ProjectFleetEntitiesInput,
): Promise<FleetProjectionOutcome> => {
  try {
    const entries = await withSyncTransaction(input.sql, async (writer) => {
      const scope = fleetRunScope(input.runId)
      const owner = await ensureScopeOwner(
        writer.sql,
        scope,
        input.ownerUserId,
      )
      if (owner !== input.ownerUserId) {
        throw new FleetScopeOwnedByOtherUserError(scope)
      }
      const appended: Array<ChangelogEntry> = []
      for (const change of input.changes) {
        appended.push(
          await appendFleetEntityChange(
            writer,
            input.runId,
            change,
            input.mutationRef ?? FLEET_PROJECTION_SYSTEM_REF,
          ),
        )
      }
      return appended
    })
    return { entries, ok: true }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}

export class FleetScopeOwnedByOtherUserError extends Error {
  readonly _tag = "FleetScopeOwnedByOtherUserError"
  override readonly name = "FleetScopeOwnedByOtherUserError"
  constructor(readonly scope: string) {
    super(`fleet scope ${scope} is owned by a different user`)
  }
}

const diagnosticFromUnknown = (error: unknown): FleetProjectionDiagnostic => {
  if (error instanceof FleetScopeOwnedByOtherUserError) {
    return {
      messageSafe: error.message,
      reason: "scope_owned_by_other_user",
    }
  }
  if (error instanceof FleetPostImageRedactionError) {
    return { messageSafe: error.message, reason: "redaction_refused" }
  }
  const tag = (error as { _tag?: unknown })?._tag
  if (tag === "KhalaSyncStorageError") {
    const messageSafe = (error as { messageSafe?: unknown }).messageSafe
    return {
      messageSafe:
        typeof messageSafe === "string" ? messageSafe : "storage failure",
      reason: "storage_failed",
    }
  }
  // Anything else (driver errors, mapping/decode failures) can embed raw
  // row values or connection strings — never echo them.
  return {
    messageSafe: "fleet projection failed",
    reason: "projection_failed",
  }
}
