import {
  type ChangelogEntry,
  ClientGroupId,
  ClientId,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncSchemaVersion,
  type SyncScope,
  SyncVersion,
  type SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import {
  type ClientIdentity,
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
  type KhalaSyncLocalStore,
} from "./store.js"

/**
 * Driver-agnostic SQL core of the Khala Sync local store (KS-5.1/KS-5.4).
 *
 * ALL store semantics live here — idempotent applyConfirmed, cursor
 * monotonicity, resetScope, the FIFO mutation queue with gap rejection,
 * identity meta, and the typed error taxonomy — parameterized over a
 * minimal synchronous {@link SqlDriver}. Two drivers exist:
 *
 * - `bun:sqlite` (sqlite-store.ts, Khala Code desktop)
 * - SQLite-WASM `oo1.DB` on the `opfs-sahpool` VFS (web/wasm-driver.ts,
 *   runs inside the web storage worker)
 *
 * Because the semantics are driver-agnostic, the full store test suite
 * runs against this core in bun (with `bun:sqlite` as the harness driver)
 * and the WASM worker inherits the exact same behavior by construction.
 */

// ---------------------------------------------------------------------------
// Driver seam
// ---------------------------------------------------------------------------

export type SqlValue = string | number

/**
 * Minimal synchronous SQL surface the store core needs. Implementations
 * must throw on failure; {@link SqlDriver.transaction} must roll back and
 * rethrow when `fn` throws (leaving the database byte-for-byte unchanged).
 */
export interface SqlDriver {
  /** Execute one or more statements without bind parameters (DDL). */
  readonly exec: (sql: string) => void
  /** Execute one statement with positional bind parameters. */
  readonly run: (sql: string, params?: ReadonlyArray<SqlValue>) => void
  /** All result rows of one statement, as objects keyed by column name. */
  readonly all: <Row>(
    sql: string,
    params?: ReadonlyArray<SqlValue>,
  ) => ReadonlyArray<Row>
  /** Run `fn` in ONE transaction; rethrow after rollback on failure. */
  readonly transaction: <A>(fn: () => A) => A
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

export const KHALA_SYNC_STORE_SCHEMA = `
CREATE TABLE IF NOT EXISTS entities (
  scope           TEXT    NOT NULL,
  entity_type     TEXT    NOT NULL,
  entity_id       TEXT    NOT NULL,
  post_image_json TEXT    NOT NULL,
  version         INTEGER NOT NULL,
  PRIMARY KEY (scope, entity_type, entity_id)
);

CREATE TABLE IF NOT EXISTS cursors (
  scope   TEXT    PRIMARY KEY,
  version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS pending_mutations (
  mutation_id INTEGER PRIMARY KEY,
  name        TEXT    NOT NULL,
  args_json   TEXT    NOT NULL,
  created_at  TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
`

const META_CLIENT_ID = "client_id"
const META_CLIENT_GROUP_ID = "client_group_id"
const META_SCHEMA_VERSION = "schema_version"
const META_LAST_MUTATION_ID = "last_mutation_id"

// ---------------------------------------------------------------------------
// Row shapes
// ---------------------------------------------------------------------------

interface EntityRow {
  readonly entity_type: string
  readonly entity_id: string
  readonly post_image_json: string
  readonly version: number
}

interface PendingRow {
  readonly mutation_id: number
  readonly name: string
  readonly args_json: string
}

// ---------------------------------------------------------------------------
// Core
// ---------------------------------------------------------------------------

/**
 * Synchronous store core: the same method set as
 * {@link KhalaSyncLocalStore}, throwing {@link KhalaSyncClientStoreError}
 * instead of failing an Effect. Adapters wrap it for their surface
 * (Effect on desktop, postMessage RPC in the web storage worker).
 */
export interface KhalaSyncStoreCore {
  readonly cursor: (scope: SyncScope) => SyncVersion | null
  readonly applyConfirmed: (
    scope: SyncScope,
    entries: ReadonlyArray<ChangelogEntry>,
    cursor: SyncVersion,
  ) => void
  readonly resetScope: (
    scope: SyncScope,
    entities: ReadonlyArray<ConfirmedEntity>,
    cursor: SyncVersion | SyncVersionWatermark,
  ) => void
  readonly readEntities: (
    scope: SyncScope,
    entityType?: string,
  ) => ReadonlyArray<ConfirmedEntity>
  readonly enqueueMutation: (mutation: MutationEnvelope) => void
  readonly pendingMutations: () => ReadonlyArray<MutationEnvelope>
  readonly lastMutationId: () => MutationId | null
  readonly ackMutations: (throughMutationId: MutationId) => void
  readonly identity: () => ClientIdentity | null
  readonly setIdentity: (identity: ClientIdentity) => void
}

/** Wrap unknown driver failures into the typed store error taxonomy. */
export const toKhalaSyncStoreError = (
  error: unknown,
): KhalaSyncClientStoreError =>
  error instanceof KhalaSyncClientStoreError
    ? error
    : new KhalaSyncClientStoreError(
        "storage_failure",
        `sqlite operation failed: ${error instanceof Error ? error.message : String(error)}`,
        { cause: error },
      )

/**
 * Build the store core over a driver. Runs the schema migration eagerly;
 * throws {@link KhalaSyncClientStoreError} if migration fails.
 */
export const createKhalaSyncStoreCore = (
  driver: SqlDriver,
): KhalaSyncStoreCore => {
  try {
    driver.exec(KHALA_SYNC_STORE_SCHEMA)
  } catch (error) {
    throw new KhalaSyncClientStoreError(
      "storage_failure",
      "failed to migrate khala-sync local store schema",
      { cause: error },
    )
  }

  const one = <Row>(
    sql: string,
    params: ReadonlyArray<SqlValue>,
  ): Row | null => driver.all<Row>(sql, params)[0] ?? null

  const getMeta = (key: string): string | null =>
    one<{ readonly value: string }>(
      "SELECT value FROM meta WHERE key = ?",
      [key],
    )?.value ?? null

  const setMeta = (key: string, value: string): void => {
    driver.run(
      `INSERT INTO meta (key, value) VALUES (?, ?)
       ON CONFLICT (key) DO UPDATE SET value = excluded.value`,
      [key, value],
    )
  }

  const storedCursor = (scope: SyncScope): number | null =>
    one<{ readonly version: number }>(
      "SELECT version FROM cursors WHERE scope = ?",
      [scope],
    )?.version ?? null

  const upsertCursor = (scope: SyncScope, version: number): void => {
    driver.run(
      `INSERT INTO cursors (scope, version) VALUES (?, ?)
       ON CONFLICT (scope) DO UPDATE SET version = excluded.version`,
      [scope, version],
    )
  }

  // Skip-stale upsert: only newer versions overwrite (SPEC §7 invariant 4).
  const upsertEntity = (
    scope: SyncScope,
    entityType: string,
    entityId: string,
    postImageJson: string,
    version: number,
  ): void => {
    driver.run(
      `INSERT INTO entities (scope, entity_type, entity_id, post_image_json, version)
       VALUES (?, ?, ?, ?, ?)
       ON CONFLICT (scope, entity_type, entity_id) DO UPDATE SET
         post_image_json = excluded.post_image_json,
         version = excluded.version
       WHERE excluded.version > entities.version`,
      [scope, entityType, entityId, postImageJson, version],
    )
  }

  const lastMutationIdRaw = (): number => {
    const raw = getMeta(META_LAST_MUTATION_ID)
    return raw === null ? 0 : Number(raw)
  }

  const rowToEntity = (row: EntityRow): ConfirmedEntity => ({
    entityType: row.entity_type,
    entityId: row.entity_id,
    postImageJson: row.post_image_json,
    version: SyncVersion.make(row.version),
  })

  const readIdentity = (): ClientIdentity | null => {
    const clientId = getMeta(META_CLIENT_ID)
    const clientGroupId = getMeta(META_CLIENT_GROUP_ID)
    const schemaVersion = getMeta(META_SCHEMA_VERSION)
    if (clientId === null || clientGroupId === null || schemaVersion === null) {
      return null
    }
    return {
      clientId: ClientId.make(clientId),
      clientGroupId: ClientGroupId.make(clientGroupId),
      schemaVersion: SyncSchemaVersion.make(Number(schemaVersion)),
    }
  }

  return {
    cursor: (scope) => {
      const stored = storedCursor(scope)
      return stored === null ? null : SyncVersion.make(stored)
    },

    applyConfirmed: (scope, entries, cursor) =>
      driver.transaction(() => {
        const stored = storedCursor(scope)
        if (stored !== null && cursor < stored) {
          throw new KhalaSyncClientStoreError(
            "cursor_regression",
            `applyConfirmed cursor ${cursor} is behind stored cursor ${stored} for the scope`,
          )
        }
        for (const entry of entries) {
          if (entry.scope !== scope) {
            throw new KhalaSyncClientStoreError(
              "constraint_violation",
              "changelog entry scope does not match the applied scope",
            )
          }
          if (entry.op === "upsert") {
            if (entry.postImageJson === undefined) {
              throw new KhalaSyncClientStoreError(
                "constraint_violation",
                "upsert changelog entry is missing its post-image (SPEC §2.3)",
              )
            }
            upsertEntity(
              scope,
              entry.entityType,
              entry.entityId,
              entry.postImageJson,
              entry.version,
            )
          } else {
            // Skip-stale delete: a tombstone only removes strictly-older state.
            driver.run(
              `DELETE FROM entities
               WHERE scope = ? AND entity_type = ? AND entity_id = ? AND version < ?`,
              [scope, entry.entityType, entry.entityId, entry.version],
            )
          }
        }
        upsertCursor(scope, cursor)
      }),

    resetScope: (scope, entities, cursor) =>
      driver.transaction(() => {
        driver.run("DELETE FROM entities WHERE scope = ?", [scope])
        for (const entity of entities) {
          upsertEntity(
            scope,
            entity.entityType,
            entity.entityId,
            entity.postImageJson,
            entity.version,
          )
        }
        if (cursor === 0) {
          // Watermark 0 = scope start: clear the cursor (never store 0 — the
          // cursors table holds SyncVersions, which start at 1).
          driver.run("DELETE FROM cursors WHERE scope = ?", [scope])
        } else {
          upsertCursor(scope, cursor)
        }
      }),

    readEntities: (scope, entityType) =>
      (entityType === undefined
        ? driver.all<EntityRow>(
            `SELECT entity_type, entity_id, post_image_json, version
             FROM entities WHERE scope = ? ORDER BY entity_type, entity_id`,
            [scope],
          )
        : driver.all<EntityRow>(
            `SELECT entity_type, entity_id, post_image_json, version
             FROM entities WHERE scope = ? AND entity_type = ?
             ORDER BY entity_type, entity_id`,
            [scope, entityType],
          )
      ).map(rowToEntity),

    enqueueMutation: (mutation) =>
      driver.transaction(() => {
        const last = lastMutationIdRaw()
        if (mutation.mutationId !== last + 1) {
          throw new KhalaSyncClientStoreError(
            "mutation_id_gap",
            `enqueueMutation expected mutationId ${last + 1}, got ${mutation.mutationId}`,
          )
        }
        driver.run(
          `INSERT INTO pending_mutations (mutation_id, name, args_json, created_at)
           VALUES (?, ?, ?, ?)`,
          [
            mutation.mutationId,
            mutation.name,
            mutation.argsJson,
            new Date().toISOString(),
          ],
        )
        setMeta(META_LAST_MUTATION_ID, String(mutation.mutationId))
      }),

    pendingMutations: () =>
      driver
        .all<PendingRow>(
          `SELECT mutation_id, name, args_json
           FROM pending_mutations ORDER BY mutation_id ASC`,
        )
        .map(
          (row) =>
            new MutationEnvelope({
              mutationId: MutationId.make(row.mutation_id),
              name: MutatorName.make(row.name),
              argsJson: row.args_json,
            }),
        ),

    lastMutationId: () => {
      const last = lastMutationIdRaw()
      return last === 0 ? null : MutationId.make(last)
    },

    ackMutations: (throughMutationId) =>
      driver.transaction(() => {
        driver.run("DELETE FROM pending_mutations WHERE mutation_id <= ?", [
          throughMutationId,
        ])
        // A server ack can run ahead of local enqueues (rejections also
        // advance lastMutationId); keep "next id = last acked + 1" true
        // either way.
        if (throughMutationId > lastMutationIdRaw()) {
          setMeta(META_LAST_MUTATION_ID, String(throughMutationId))
        }
      }),

    identity: readIdentity,

    setIdentity: (identity) =>
      driver.transaction(() => {
        const existing = readIdentity()
        if (existing !== null) {
          const conflicting =
            existing.clientId !== identity.clientId ||
            existing.clientGroupId !== identity.clientGroupId ||
            existing.schemaVersion !== identity.schemaVersion
          if (conflicting) {
            throw new KhalaSyncClientStoreError(
              "constraint_violation",
              "client identity is already set and differs; reset the store to change identity",
            )
          }
          return
        }
        setMeta(META_CLIENT_ID, identity.clientId)
        setMeta(META_CLIENT_GROUP_ID, identity.clientGroupId)
        setMeta(META_SCHEMA_VERSION, String(identity.schemaVersion))
      }),
  }
}

// ---------------------------------------------------------------------------
// Effect adapter
// ---------------------------------------------------------------------------

/**
 * Lift a synchronous {@link KhalaSyncStoreCore} into the Effect-surfaced
 * {@link KhalaSyncLocalStore} contract. Used by the `bun:sqlite` desktop
 * store; the web adapter instead crosses a postMessage RPC boundary and
 * wraps promises.
 */
export const localStoreFromCore = (
  core: KhalaSyncStoreCore,
): KhalaSyncLocalStore => {
  const tryStore = <A>(
    run: () => A,
  ): Effect.Effect<A, KhalaSyncClientStoreError> =>
    Effect.try({ try: run, catch: toKhalaSyncStoreError })
  return {
    cursor: (scope) => tryStore(() => core.cursor(scope)),
    applyConfirmed: (scope, entries, cursor) =>
      tryStore(() => core.applyConfirmed(scope, entries, cursor)),
    resetScope: (scope, entities, cursor) =>
      tryStore(() => core.resetScope(scope, entities, cursor)),
    readEntities: (scope, entityType) =>
      tryStore(() => core.readEntities(scope, entityType)),
    enqueueMutation: (mutation) => tryStore(() => core.enqueueMutation(mutation)),
    pendingMutations: () => tryStore(() => core.pendingMutations()),
    lastMutationId: () => tryStore(() => core.lastMutationId()),
    ackMutations: (throughMutationId) =>
      tryStore(() => core.ackMutations(throughMutationId)),
    identity: () => tryStore(() => core.identity()),
    setIdentity: (identity) => tryStore(() => core.setIdentity(identity)),
  }
}
