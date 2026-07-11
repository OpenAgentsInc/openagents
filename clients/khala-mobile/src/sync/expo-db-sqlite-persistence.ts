import {
  ClientGroupId,
  ClientId,
  isDeviceLocalScope,
  type LocalAccountLink,
  type LocalIdentityRecord,
  LocalIdentityRef,
  LocalRevision,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncSchemaVersion,
  type SyncScope,
  SyncVersion
} from "@openagentsinc/khala-sync"
import {
  KHALA_SYNC_STORE_SCHEMA,
  KhalaSyncClientStoreError,
  type ClientIdentity,
  type KhalaSyncLocalStore
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

export const KHALA_MOBILE_SYNC_DB_NAME = "khala-mobile-sync.db"
export const KHALA_MOBILE_PERSISTENCE_KIND = "expo-db-sqlite-persistence"
export const KHALA_MOBILE_SYNC_STORE_KIND = "expo-sqlite-khala-sync-store"

export type ExpoSqliteDatabase = Readonly<{
  execAsync: (statement: string) => Promise<void>
  getAllAsync: <T>(statement: string, ...params: ReadonlyArray<unknown>) => Promise<ReadonlyArray<T>>
  getFirstAsync: <T>(statement: string, ...params: ReadonlyArray<unknown>) => Promise<T | null>
  runAsync: (statement: string, ...params: ReadonlyArray<unknown>) => Promise<unknown>
  withExclusiveTransactionAsync?: <A>(task: () => Promise<A>) => Promise<A>
  withTransactionAsync?: <A>(task: () => Promise<A>) => Promise<A>
  closeAsync?: () => Promise<void>
}>

export type ExpoSqliteModule = Readonly<{
  openDatabaseAsync: (name: string) => Promise<ExpoSqliteDatabase>
}>

export type KhalaMobileSyncCheckpoint = Readonly<{
  scope: string
  cursor: number
  updatedAt: string
}>

export type KhalaMobileProjectionEntity = Readonly<{
  scope: string
  entityType: string
  entityId: string
  postImageJson: string
  updatedAt: string
}>

export type KhalaMobileSqlitePersistence = Readonly<{
  kind: typeof KHALA_MOBILE_PERSISTENCE_KIND
  databaseName: string
  db: ExpoSqliteDatabase
  readCheckpoint: (scope: string) => Promise<KhalaMobileSyncCheckpoint | null>
  readProjectionEntities: (
    scope: string,
    entityType?: string,
  ) => Promise<ReadonlyArray<KhalaMobileProjectionEntity>>
  saveCheckpoint: (checkpoint: KhalaMobileSyncCheckpoint) => Promise<void>
  saveProjectionEntities: (
    scope: string,
    entities: ReadonlyArray<Omit<KhalaMobileProjectionEntity, "scope" | "updatedAt">>,
    updatedAt: string,
  ) => Promise<void>
  clearScope: (scope: string) => Promise<void>
}>

export type KhalaMobileSyncStore = KhalaSyncLocalStore & Readonly<{
  kind: typeof KHALA_MOBILE_SYNC_STORE_KIND
  databaseName: string
  db: ExpoSqliteDatabase
  close: () => Effect.Effect<void, KhalaSyncClientStoreError>
}>

const loadExpoSqlite = async (): Promise<ExpoSqliteModule> =>
  (await import("expo-sqlite")) as ExpoSqliteModule

export const KHALA_MOBILE_SQLITE_SCHEMA = `
CREATE TABLE IF NOT EXISTS khala_sync_checkpoints (
  scope TEXT PRIMARY KEY NOT NULL,
  cursor INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS khala_sync_projection_cache (
  scope TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  post_image_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (scope, entity_type, entity_id)
);
`

const toStoreError = (error: unknown): KhalaSyncClientStoreError =>
  error instanceof KhalaSyncClientStoreError
    ? error
    : new KhalaSyncClientStoreError(
        "storage_failure",
        `expo sqlite operation failed: ${error instanceof Error ? error.message : String(error)}`,
        { cause: error }
      )

const tryStore = <A>(
  run: () => Promise<A>,
): Effect.Effect<A, KhalaSyncClientStoreError> =>
  Effect.tryPromise({ try: run, catch: toStoreError })

const transactionQueues = new WeakMap<ExpoSqliteDatabase, Promise<unknown>>()

const enqueueDatabaseTask = async <A>(
  db: ExpoSqliteDatabase,
  run: () => Promise<A>,
): Promise<A> => {
  const previous = transactionQueues.get(db) ?? Promise.resolve()
  const next = previous.catch(() => undefined).then(run)
  transactionQueues.set(db, next.catch(() => undefined))
  return next
}

const runInTransaction = async <A>(
  db: ExpoSqliteDatabase,
  run: () => Promise<A>,
): Promise<A> =>
  enqueueDatabaseTask(db, async () => {
    if (db.withExclusiveTransactionAsync !== undefined) {
      return db.withExclusiveTransactionAsync(run)
    }
    if (db.withTransactionAsync !== undefined) {
      return db.withTransactionAsync(run)
    }
    return run()
  })

const one = async <T>(
  db: ExpoSqliteDatabase,
  statement: string,
  ...params: ReadonlyArray<unknown>
): Promise<T | null> => db.getFirstAsync<T>(statement, ...params)

const getMeta = async (
  db: ExpoSqliteDatabase,
  key: string,
): Promise<string | null> =>
  (await one<{ readonly value: string }>(
    db,
    "SELECT value FROM meta WHERE key = ?",
    key
  ))?.value ?? null

const setMeta = async (
  db: ExpoSqliteDatabase,
  key: string,
  value: string,
): Promise<void> => {
  await db.runAsync(
    `INSERT INTO meta (key, value) VALUES (?, ?)
     ON CONFLICT (key) DO UPDATE SET value = excluded.value`,
    key,
    value
  )
}

const storedCursor = async (
  db: ExpoSqliteDatabase,
  scope: SyncScope,
): Promise<number | null> =>
  (await one<{ readonly version: number }>(
    db,
    "SELECT version FROM cursors WHERE scope = ?",
    String(scope)
  ))?.version ?? null

const upsertCursor = async (
  db: ExpoSqliteDatabase,
  scope: SyncScope,
  version: number,
): Promise<void> => {
  await db.runAsync(
    `INSERT INTO cursors (scope, version) VALUES (?, ?)
     ON CONFLICT (scope) DO UPDATE SET version = excluded.version`,
    String(scope),
    version
  )
}

const upsertEntity = async (
  db: ExpoSqliteDatabase,
  scope: SyncScope,
  entityType: string,
  entityId: string,
  postImageJson: string,
  version: number,
): Promise<void> => {
  await db.runAsync(
    `INSERT INTO entities (scope, entity_type, entity_id, post_image_json, version)
     VALUES (?, ?, ?, ?, ?)
     ON CONFLICT (scope, entity_type, entity_id) DO UPDATE SET
       post_image_json = excluded.post_image_json,
       version = excluded.version
     WHERE excluded.version > entities.version`,
    String(scope),
    entityType,
    entityId,
    postImageJson,
    version
  )
}

const readIdentity = async (
  db: ExpoSqliteDatabase,
): Promise<ClientIdentity | null> => {
  const clientId = await getMeta(db, "client_id")
  const clientGroupId = await getMeta(db, "client_group_id")
  const schemaVersion = await getMeta(db, "schema_version")
  if (clientId === null || clientGroupId === null || schemaVersion === null) {
    return null
  }
  return {
    clientGroupId: ClientGroupId.make(clientGroupId),
    clientId: ClientId.make(clientId),
    schemaVersion: SyncSchemaVersion.make(Number(schemaVersion))
  }
}

// Device-local identity/authority reads (R1-LOCAL #8666). These mirror the
// shared `store-core.ts` semantics used by the greenfield expo-sqlite store,
// against the same `local_identity` / `local_account_link` / `local_entities`
// tables that `KHALA_SYNC_STORE_SCHEMA` already creates in this database.
const readLocalIdentity = async (
  db: ExpoSqliteDatabase,
): Promise<LocalIdentityRecord | null> => {
  const row = await one<{
    readonly identity_ref: string
    readonly created_at: string
  }>(
    db,
    "SELECT identity_ref, created_at FROM local_identity WHERE singleton = 1"
  )
  return row === null
    ? null
    : {
        createdAt: row.created_at,
        identityRef: LocalIdentityRef.make(row.identity_ref),
        schemaVersion: 1
      }
}

const readLocalLink = async (
  db: ExpoSqliteDatabase,
): Promise<LocalAccountLink | null> => {
  const row = await one<{
    readonly identity_ref: string
    readonly owner_user_id: string
    readonly linked_at: string
    readonly link_receipt_ref: string
  }>(
    db,
    `SELECT identity_ref, owner_user_id, linked_at, link_receipt_ref
     FROM local_account_link WHERE singleton = 1`
  )
  return row === null
    ? null
    : {
        identityRef: LocalIdentityRef.make(row.identity_ref),
        linkReceiptRef: row.link_receipt_ref,
        linkedAt: row.linked_at,
        ownerUserId: row.owner_user_id,
        schemaVersion: 1
      }
}

const lastMutationIdRaw = async (db: ExpoSqliteDatabase): Promise<number> => {
  const raw = await getMeta(db, "last_mutation_id")
  return raw === null ? 0 : Number(raw)
}

export const openKhalaMobileSqlitePersistence = async (
  input: {
    readonly databaseName?: string
    readonly sqliteLoader?: () => Promise<ExpoSqliteModule>
  } = {},
): Promise<KhalaMobileSqlitePersistence> => {
  const databaseName = input.databaseName ?? KHALA_MOBILE_SYNC_DB_NAME
  const sqlite = await (input.sqliteLoader ?? loadExpoSqlite)()
  const db = await sqlite.openDatabaseAsync(databaseName)
  await db.execAsync(KHALA_MOBILE_SQLITE_SCHEMA)

  return {
    databaseName,
    db,
    kind: KHALA_MOBILE_PERSISTENCE_KIND,
    clearScope: scope =>
      runInTransaction(db, async () => {
        await db.runAsync(
          "DELETE FROM khala_sync_checkpoints WHERE scope = ?",
          scope
        )
        await db.runAsync(
          "DELETE FROM khala_sync_projection_cache WHERE scope = ?",
          scope
        )
      }).then(() => undefined),
    readCheckpoint: scope =>
      db.getFirstAsync<{
        readonly scope: string
        readonly cursor: number
        readonly updated_at: string
      }>(
        "SELECT scope, cursor, updated_at FROM khala_sync_checkpoints WHERE scope = ?",
        scope
      ).then(row =>
        row === null
          ? null
          : {
              cursor: row.cursor,
              scope: row.scope,
              updatedAt: row.updated_at
            }
      ),
    readProjectionEntities: (scope, entityType) => {
      const query = entityType === undefined
        ? db.getAllAsync<{
            readonly scope: string
            readonly entity_type: string
            readonly entity_id: string
            readonly post_image_json: string
            readonly updated_at: string
          }>(
            `SELECT scope, entity_type, entity_id, post_image_json, updated_at
             FROM khala_sync_projection_cache
             WHERE scope = ?
             ORDER BY entity_type, entity_id`,
            scope
          )
        : db.getAllAsync<{
            readonly scope: string
            readonly entity_type: string
            readonly entity_id: string
            readonly post_image_json: string
            readonly updated_at: string
          }>(
            `SELECT scope, entity_type, entity_id, post_image_json, updated_at
             FROM khala_sync_projection_cache
             WHERE scope = ? AND entity_type = ?
             ORDER BY entity_type, entity_id`,
            scope,
            entityType
          )
      return query.then(rows =>
        rows.map(row => ({
          entityId: row.entity_id,
          entityType: row.entity_type,
          postImageJson: row.post_image_json,
          scope: row.scope,
          updatedAt: row.updated_at
        }))
      )
    },
    saveCheckpoint: checkpoint =>
      db.runAsync(
        `INSERT INTO khala_sync_checkpoints (scope, cursor, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT(scope) DO UPDATE SET
           cursor = excluded.cursor,
           updated_at = excluded.updated_at`,
        checkpoint.scope,
        checkpoint.cursor,
        checkpoint.updatedAt
      ).then(() => undefined),
    saveProjectionEntities: (scope, entities, updatedAt) =>
      runInTransaction(db, async () => {
        for (const entity of entities) {
          await db.runAsync(
            `INSERT INTO khala_sync_projection_cache
               (scope, entity_type, entity_id, post_image_json, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(scope, entity_type, entity_id) DO UPDATE SET
               post_image_json = excluded.post_image_json,
               updated_at = excluded.updated_at`,
            scope,
            entity.entityType,
            entity.entityId,
            entity.postImageJson,
            updatedAt
          )
        }
      }).then(() => undefined)
  }
}

export const openKhalaMobileSyncStore = async (
  input: {
    readonly databaseName?: string
    readonly sqliteLoader?: () => Promise<ExpoSqliteModule>
  } = {},
): Promise<KhalaMobileSyncStore> => {
  const databaseName = input.databaseName ?? KHALA_MOBILE_SYNC_DB_NAME
  const sqlite = await (input.sqliteLoader ?? loadExpoSqlite)()
  const db = await sqlite.openDatabaseAsync(databaseName)
  await db.execAsync(KHALA_SYNC_STORE_SCHEMA)

  const store: KhalaMobileSyncStore = {
    databaseName,
    db,
    kind: KHALA_MOBILE_SYNC_STORE_KIND,
    ackMutations: throughMutationId =>
      tryStore(() =>
        runInTransaction(db, async () => {
          await db.runAsync(
            "DELETE FROM pending_mutations WHERE mutation_id <= ?",
            Number(throughMutationId)
          )
          if (Number(throughMutationId) > await lastMutationIdRaw(db)) {
            await setMeta(db, "last_mutation_id", String(throughMutationId))
          }
        })
      ),
    applyConfirmed: (scope, entries, cursor) =>
      tryStore(() =>
        runInTransaction(db, async () => {
          const stored = await storedCursor(db, scope)
          if (stored !== null && Number(cursor) < stored) {
            throw new KhalaSyncClientStoreError(
              "cursor_regression",
              "applyConfirmed cursor is behind the stored scope cursor"
            )
          }
          for (const entry of entries) {
            if (String(entry.scope) !== String(scope)) {
              throw new KhalaSyncClientStoreError(
                "constraint_violation",
                "changelog entry scope does not match the applied scope"
              )
            }
            if (entry.op === "upsert") {
              if (entry.postImageJson === undefined) {
                throw new KhalaSyncClientStoreError(
                  "constraint_violation",
                  "upsert changelog entry is missing its post-image"
                )
              }
              await upsertEntity(
                db,
                scope,
                String(entry.entityType),
                String(entry.entityId),
                entry.postImageJson,
                Number(entry.version)
              )
            } else {
              await db.runAsync(
                `DELETE FROM entities
                 WHERE scope = ? AND entity_type = ? AND entity_id = ? AND version < ?`,
                String(scope),
                String(entry.entityType),
                String(entry.entityId),
                Number(entry.version)
              )
            }
          }
          await upsertCursor(db, scope, Number(cursor))
        })
      ),
    clearLocalAccountLink: () =>
      tryStore(() =>
        runInTransaction(db, async () => {
          await db.runAsync("DELETE FROM local_account_link WHERE singleton = 1")
        })
      ),
    close: () =>
      tryStore(async () => {
        await db.closeAsync?.()
      }),
    cursor: scope =>
      tryStore(async () => {
        const cursor = await storedCursor(db, scope)
        return cursor === null ? null : SyncVersion.make(cursor)
      }),
    enqueueMutation: mutation =>
      tryStore(() =>
        runInTransaction(db, async () => {
          const last = await lastMutationIdRaw(db)
          if (Number(mutation.mutationId) !== last + 1) {
            throw new KhalaSyncClientStoreError(
              "mutation_id_gap",
              "enqueueMutation received a non-sequential mutation id"
            )
          }
          await db.runAsync(
            `INSERT INTO pending_mutations (mutation_id, name, args_json, created_at)
             VALUES (?, ?, ?, ?)`,
            Number(mutation.mutationId),
            String(mutation.name),
            mutation.argsJson,
            new Date().toISOString()
          )
          await setMeta(db, "last_mutation_id", String(mutation.mutationId))
        })
      ),
    identity: () => tryStore(() => readIdentity(db)),
    lastMutationId: () =>
      tryStore(async () => {
        const last = await lastMutationIdRaw(db)
        return last === 0 ? null : MutationId.make(last)
      }),
    localAccountLink: () => tryStore(() => readLocalLink(db)),
    localIdentity: () => tryStore(() => readLocalIdentity(db)),
    pendingMutations: () =>
      tryStore(async () => {
        const rows = await db.getAllAsync<{
          readonly mutation_id: number
          readonly name: string
          readonly args_json: string
        }>(
          `SELECT mutation_id, name, args_json
           FROM pending_mutations ORDER BY mutation_id ASC`
        )
        return rows.map(row =>
          new MutationEnvelope({
            argsJson: row.args_json,
            mutationId: MutationId.make(row.mutation_id),
            name: MutatorName.make(row.name)
          })
        )
      }),
    readEntities: (scope, entityType) =>
      tryStore(async () => {
        const rows = entityType === undefined
          ? await db.getAllAsync<{
              readonly entity_type: string
              readonly entity_id: string
              readonly post_image_json: string
              readonly version: number
            }>(
              `SELECT entity_type, entity_id, post_image_json, version
               FROM entities WHERE scope = ? ORDER BY entity_type, entity_id`,
              String(scope)
            )
          : await db.getAllAsync<{
              readonly entity_type: string
              readonly entity_id: string
              readonly post_image_json: string
              readonly version: number
            }>(
              `SELECT entity_type, entity_id, post_image_json, version
               FROM entities WHERE scope = ? AND entity_type = ?
               ORDER BY entity_type, entity_id`,
              String(scope),
              entityType
            )
        return rows.map(row => ({
          entityId: row.entity_id,
          entityType: row.entity_type,
          postImageJson: row.post_image_json,
          version: SyncVersion.make(row.version)
        }))
      }),
    readLocalEntities: (scope, entityType) =>
      tryStore(async () => {
        if (!isDeviceLocalScope(scope)) {
          throw new KhalaSyncClientStoreError(
            "constraint_violation",
            "local authority reads require device-local scope"
          )
        }
        const rows = entityType === undefined
          ? await db.getAllAsync<{
              readonly entity_type: string
              readonly entity_id: string
              readonly post_image_json: string
              readonly revision: number
            }>(
              `SELECT entity_type, entity_id, post_image_json, revision
               FROM local_entities WHERE scope = ?
               ORDER BY entity_type, entity_id`,
              String(scope)
            )
          : await db.getAllAsync<{
              readonly entity_type: string
              readonly entity_id: string
              readonly post_image_json: string
              readonly revision: number
            }>(
              `SELECT entity_type, entity_id, post_image_json, revision
               FROM local_entities WHERE scope = ? AND entity_type = ?
               ORDER BY entity_type, entity_id`,
              String(scope),
              entityType
            )
        return rows.map(row => ({
          entityId: row.entity_id,
          entityType: row.entity_type,
          postImageJson: row.post_image_json,
          revision: LocalRevision.make(row.revision)
        }))
      }),
    resetScope: (scope, entities, cursor) =>
      tryStore(() =>
        runInTransaction(db, async () => {
          await db.runAsync("DELETE FROM entities WHERE scope = ?", String(scope))
          for (const entity of entities) {
            await upsertEntity(
              db,
              scope,
              entity.entityType,
              entity.entityId,
              entity.postImageJson,
              Number(entity.version)
            )
          }
          if (Number(cursor) === 0) {
            await db.runAsync("DELETE FROM cursors WHERE scope = ?", String(scope))
          } else {
            await upsertCursor(db, scope, Number(cursor))
          }
        })
      ),
    setIdentity: identity =>
      tryStore(() =>
        runInTransaction(db, async () => {
          const existing = await readIdentity(db)
          if (existing !== null) {
            const conflicting =
              existing.clientId !== identity.clientId ||
              existing.clientGroupId !== identity.clientGroupId ||
              existing.schemaVersion !== identity.schemaVersion
            if (conflicting) {
              throw new KhalaSyncClientStoreError(
                "constraint_violation",
                "client identity is already set and differs; reset the store to change identity"
              )
            }
            return
          }
          await setMeta(db, "client_id", identity.clientId)
          await setMeta(db, "client_group_id", identity.clientGroupId)
          await setMeta(db, "schema_version", String(identity.schemaVersion))
        })
      ),
    setLocalAccountLink: link =>
      tryStore(() =>
        runInTransaction(db, async () => {
          const identity = await readLocalIdentity(db)
          if (identity === null || identity.identityRef !== link.identityRef) {
            throw new KhalaSyncClientStoreError(
              "constraint_violation",
              "account link does not match local identity"
            )
          }
          const existing = await readLocalLink(db)
          if (existing !== null && existing.ownerUserId !== link.ownerUserId) {
            throw new KhalaSyncClientStoreError(
              "constraint_violation",
              "account link owner cannot change without unlink"
            )
          }
          await db.runAsync(
            `INSERT INTO local_account_link
               (singleton, identity_ref, owner_user_id, linked_at, link_receipt_ref, schema_version)
             VALUES (1, ?, ?, ?, ?, 1)
             ON CONFLICT (singleton) DO UPDATE SET
               linked_at = excluded.linked_at,
               link_receipt_ref = excluded.link_receipt_ref`,
            link.identityRef,
            link.ownerUserId,
            link.linkedAt,
            link.linkReceiptRef
          )
        })
      ),
    setLocalIdentity: identity =>
      tryStore(() =>
        runInTransaction(db, async () => {
          const existing = await readLocalIdentity(db)
          if (existing !== null) {
            if (
              existing.identityRef !== identity.identityRef ||
              existing.createdAt !== identity.createdAt
            ) {
              throw new KhalaSyncClientStoreError(
                "constraint_violation",
                "local identity is immutable"
              )
            }
            return
          }
          await db.runAsync(
            `INSERT INTO local_identity (singleton, identity_ref, created_at, schema_version)
             VALUES (1, ?, ?, 1)`,
            identity.identityRef,
            identity.createdAt
          )
        })
      ),
    writeLocalEntities: (scope, entities) =>
      tryStore(() =>
        runInTransaction(db, async () => {
          if (!isDeviceLocalScope(scope)) {
            throw new KhalaSyncClientStoreError(
              "constraint_violation",
              "local authority writes require device-local scope"
            )
          }
          for (const entity of entities) {
            await db.runAsync(
              `INSERT INTO local_entities (scope, entity_type, entity_id, post_image_json, revision)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT (scope, entity_type, entity_id) DO UPDATE SET
                 post_image_json = excluded.post_image_json,
                 revision = excluded.revision
               WHERE excluded.revision > local_entities.revision`,
              String(scope),
              entity.entityType,
              entity.entityId,
              entity.postImageJson,
              Number(entity.revision)
            )
          }
        })
      )
  }

  return store
}
