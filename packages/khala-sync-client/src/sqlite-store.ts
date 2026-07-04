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
} from "@openagentsinc/khala-sync"
import { Database } from "bun:sqlite"
import { Effect } from "effect"
import {
  type ClientIdentity,
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
  type KhalaSyncLocalStore,
} from "./store.js"

/**
 * `bun:sqlite` implementation of {@link KhalaSyncLocalStore} (KS-5.1) for
 * Khala Code desktop. WAL journaling on file databases; every multi-row
 * semantic ({@link KhalaSyncLocalStore.applyConfirmed},
 * {@link KhalaSyncLocalStore.resetScope},
 * {@link KhalaSyncLocalStore.enqueueMutation}) runs in ONE SQLite
 * transaction — a failure mid-batch leaves the store byte-for-byte
 * unchanged.
 *
 * Invariant 2 (SPEC §7) lives at this seam: the tables below hold
 * **server-confirmed state only**. The optimistic overlay (KS-5.2) is a
 * separate in-memory structure and must never write here.
 *
 * `bun:sqlite` is synchronous, so the Effect surface wraps direct calls
 * with `Effect.try` — the Promise/Effect split used by the
 * khala-sync-server outbox writer is not needed at this seam.
 */

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

const SCHEMA = `
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
// Store
// ---------------------------------------------------------------------------

export interface KhalaSyncSqliteStore extends KhalaSyncLocalStore {
  /** Close the underlying database handle; later calls fail typed. */
  readonly close: () => Effect.Effect<void, KhalaSyncClientStoreError>
}

const storeError = (error: unknown): KhalaSyncClientStoreError =>
  error instanceof KhalaSyncClientStoreError
    ? error
    : new KhalaSyncClientStoreError(
        "storage_failure",
        `sqlite operation failed: ${error instanceof Error ? error.message : String(error)}`,
        { cause: error },
      )

/**
 * Open (or create) a Khala Sync local store at `path` — a filesystem path
 * or `":memory:"`. Synchronous by nature of `bun:sqlite`; throws
 * {@link KhalaSyncClientStoreError} if the database cannot be opened or
 * migrated.
 */
export const openKhalaSyncStore = (path: string): KhalaSyncSqliteStore => {
  let db: Database
  try {
    db = new Database(path, { create: true })
    // WAL for durable file stores (readers don't block the writer). On
    // :memory: databases SQLite reports "memory" and that is fine.
    db.exec("PRAGMA journal_mode = WAL;")
    db.exec("PRAGMA foreign_keys = ON;")
    db.exec(SCHEMA)
  } catch (error) {
    throw new KhalaSyncClientStoreError(
      "storage_failure",
      "failed to open khala-sync local store",
      { cause: error },
    )
  }

  // -- prepared statements --------------------------------------------------

  const selectCursor = db.query<{ version: number }, [string]>(
    "SELECT version FROM cursors WHERE scope = ?",
  )
  const upsertCursor = db.query(
    `INSERT INTO cursors (scope, version) VALUES (?, ?)
     ON CONFLICT (scope) DO UPDATE SET version = excluded.version`,
  )
  // Skip-stale upsert: only newer versions overwrite (SPEC §7 invariant 4).
  const upsertEntity = db.query(
    `INSERT INTO entities (scope, entity_type, entity_id, post_image_json, version)
     VALUES (?, ?, ?, ?, ?)
     ON CONFLICT (scope, entity_type, entity_id) DO UPDATE SET
       post_image_json = excluded.post_image_json,
       version = excluded.version
     WHERE excluded.version > entities.version`,
  )
  // Skip-stale delete: a tombstone only removes strictly-older state.
  const deleteEntity = db.query(
    `DELETE FROM entities
     WHERE scope = ? AND entity_type = ? AND entity_id = ? AND version < ?`,
  )
  const deleteScopeEntities = db.query("DELETE FROM entities WHERE scope = ?")
  const selectEntities = db.query<EntityRow, [string]>(
    `SELECT entity_type, entity_id, post_image_json, version
     FROM entities WHERE scope = ? ORDER BY entity_type, entity_id`,
  )
  const selectEntitiesOfType = db.query<EntityRow, [string, string]>(
    `SELECT entity_type, entity_id, post_image_json, version
     FROM entities WHERE scope = ? AND entity_type = ?
     ORDER BY entity_type, entity_id`,
  )
  const insertPending = db.query(
    `INSERT INTO pending_mutations (mutation_id, name, args_json, created_at)
     VALUES (?, ?, ?, ?)`,
  )
  const selectPending = db.query<PendingRow, []>(
    `SELECT mutation_id, name, args_json
     FROM pending_mutations ORDER BY mutation_id ASC`,
  )
  const deleteAcked = db.query(
    "DELETE FROM pending_mutations WHERE mutation_id <= ?",
  )
  const selectMeta = db.query<{ value: string }, [string]>(
    "SELECT value FROM meta WHERE key = ?",
  )
  const upsertMeta = db.query(
    `INSERT INTO meta (key, value) VALUES (?, ?)
     ON CONFLICT (key) DO UPDATE SET value = excluded.value`,
  )

  const getMeta = (key: string): string | null =>
    selectMeta.get(key)?.value ?? null

  const lastMutationId = (): number => {
    const raw = getMeta(META_LAST_MUTATION_ID)
    return raw === null ? 0 : Number(raw)
  }

  // -- transactional cores (throw KhalaSyncClientStoreError to roll back) ---

  const applyConfirmedTx = db.transaction(
    (
      scope: SyncScope,
      entries: ReadonlyArray<ChangelogEntry>,
      cursor: SyncVersion,
    ): void => {
      const stored = selectCursor.get(scope)?.version ?? null
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
          upsertEntity.run(
            scope,
            entry.entityType,
            entry.entityId,
            entry.postImageJson,
            entry.version,
          )
        } else {
          deleteEntity.run(scope, entry.entityType, entry.entityId, entry.version)
        }
      }
      upsertCursor.run(scope, cursor)
    },
  )

  const resetScopeTx = db.transaction(
    (
      scope: SyncScope,
      entities: ReadonlyArray<ConfirmedEntity>,
      cursor: SyncVersion,
    ): void => {
      deleteScopeEntities.run(scope)
      for (const entity of entities) {
        upsertEntity.run(
          scope,
          entity.entityType,
          entity.entityId,
          entity.postImageJson,
          entity.version,
        )
      }
      upsertCursor.run(scope, cursor)
    },
  )

  const enqueueMutationTx = db.transaction(
    (mutation: MutationEnvelope): void => {
      const last = lastMutationId()
      if (mutation.mutationId !== last + 1) {
        throw new KhalaSyncClientStoreError(
          "mutation_id_gap",
          `enqueueMutation expected mutationId ${last + 1}, got ${mutation.mutationId}`,
        )
      }
      insertPending.run(
        mutation.mutationId,
        mutation.name,
        mutation.argsJson,
        new Date().toISOString(),
      )
      upsertMeta.run(META_LAST_MUTATION_ID, String(mutation.mutationId))
    },
  )

  const ackMutationsTx = db.transaction((throughMutationId: MutationId): void => {
    deleteAcked.run(throughMutationId)
    // A server ack can run ahead of local enqueues (rejections also advance
    // lastMutationId); keep "next id = last acked + 1" true either way.
    if (throughMutationId > lastMutationId()) {
      upsertMeta.run(META_LAST_MUTATION_ID, String(throughMutationId))
    }
  })

  const setIdentityTx = db.transaction((identity: ClientIdentity): void => {
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
    upsertMeta.run(META_CLIENT_ID, identity.clientId)
    upsertMeta.run(META_CLIENT_GROUP_ID, identity.clientGroupId)
    upsertMeta.run(META_SCHEMA_VERSION, String(identity.schemaVersion))
  })

  // -- reads -----------------------------------------------------------------

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

  // -- Effect surface ---------------------------------------------------------

  const tryStore = <A>(run: () => A): Effect.Effect<A, KhalaSyncClientStoreError> =>
    Effect.try({ try: run, catch: storeError })

  return {
    cursor: (scope) =>
      tryStore(() => {
        const row = selectCursor.get(scope)
        return row === null ? null : SyncVersion.make(row.version)
      }),
    applyConfirmed: (scope, entries, cursor) =>
      tryStore(() => applyConfirmedTx(scope, entries, cursor)),
    resetScope: (scope, entities, cursor) =>
      tryStore(() => resetScopeTx(scope, entities, cursor)),
    readEntities: (scope, entityType) =>
      tryStore(() =>
        (entityType === undefined
          ? selectEntities.all(scope)
          : selectEntitiesOfType.all(scope, entityType)
        ).map(rowToEntity),
      ),
    enqueueMutation: (mutation) => tryStore(() => enqueueMutationTx(mutation)),
    pendingMutations: () =>
      tryStore(() =>
        selectPending.all().map(
          (row) =>
            new MutationEnvelope({
              mutationId: MutationId.make(row.mutation_id),
              name: MutatorName.make(row.name),
              argsJson: row.args_json,
            }),
        ),
      ),
    ackMutations: (throughMutationId) =>
      tryStore(() => ackMutationsTx(throughMutationId)),
    identity: () => tryStore(readIdentity),
    setIdentity: (identity) => tryStore(() => setIdentityTx(identity)),
    close: () => tryStore(() => db.close()),
  }
}
