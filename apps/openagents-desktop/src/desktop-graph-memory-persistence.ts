import { chmodSync, mkdirSync } from "node:fs";
import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import path from "node:path";

import { openSqliteDatabase, type SqliteDatabase } from "@openagentsinc/sqlite-runtime";

import type { SafeStorageLike } from "./desktop-session-vault.js";

const GRAPH_MEMORY_DATA_KEY_REF = "graph-memory-data-key.v1";
const GRAPH_MEMORY_SCRUB_REQUIRED_REF = "graph-memory-scrub-required.v1";
const GRAPH_MEMORY_SCHEMA_REF = "graph-memory-schema-version";
const GRAPH_MEMORY_SCHEMA_VERSION = 1;
const GRAPH_MEMORY_AAD_SCHEMA = "openagents.desktop.graph_memory.aad.v1";

export type DesktopGraphMemoryScope = Readonly<{
  ownerScope: string;
  projectScope: string;
}>;

export type DesktopGraphMemoryPersistedState = Readonly<{
  revision: number;
  payload: string;
}>;

export type DesktopGraphMemoryPersistenceStats = Readonly<{
  reads: number;
  writes: number;
}>;

export type DesktopGraphMemoryPersistence = Readonly<{
  enabled: boolean;
  load: (scope: DesktopGraphMemoryScope) => DesktopGraphMemoryPersistedState | null;
  save: (scope: DesktopGraphMemoryScope, state: DesktopGraphMemoryPersistedState) => void;
  compareAndSet: (
    scope: DesktopGraphMemoryScope,
    expectedRevision: number | null,
    state: DesktopGraphMemoryPersistedState,
  ) => boolean;
  remove: (scope: DesktopGraphMemoryScope) => boolean;
  stats: () => DesktopGraphMemoryPersistenceStats;
  close: () => void;
}>;

export type DesktopGraphMemoryPersistenceErrorReason =
  | "encryption_unavailable"
  | "incompatible_version"
  | "invalid_scope"
  | "invalid_state"
  | "storage_unavailable";

export class DesktopGraphMemoryPersistenceError extends Error {
  readonly _tag = "DesktopGraphMemoryPersistenceError";
  override readonly name = "DesktopGraphMemoryPersistenceError";

  constructor(
    readonly reason: DesktopGraphMemoryPersistenceErrorReason,
    message: string,
    options?: { readonly cause?: unknown },
  ) {
    super(message, options);
  }
}

type StoredRow = Readonly<{
  revision: number;
  nonce: Uint8Array;
  ciphertext: Uint8Array;
  auth_tag: Uint8Array;
}>;

type StoredKeyRow = Readonly<{ wrapped_key: string }>;

const publicFailure = (error: unknown): DesktopGraphMemoryPersistenceError =>
  error instanceof DesktopGraphMemoryPersistenceError
    ? error
    : new DesktopGraphMemoryPersistenceError(
        "storage_unavailable",
        "Desktop graph memory storage is unavailable.",
        { cause: error },
      );

const validScopePart = (value: string): boolean =>
  value.length >= 1 && value.length <= 256 && /^[A-Za-z0-9][A-Za-z0-9._:/-]*$/u.test(value);

const requireScope = (scope: DesktopGraphMemoryScope): void => {
  if (!validScopePart(scope.ownerScope) || !validScopePart(scope.projectScope)) {
    throw new DesktopGraphMemoryPersistenceError(
      "invalid_scope",
      "The graph memory scope is invalid.",
    );
  }
};

const requireState = (state: DesktopGraphMemoryPersistedState): void => {
  if (!Number.isSafeInteger(state.revision) || state.revision < 0 || state.payload === "") {
    throw new DesktopGraphMemoryPersistenceError(
      "invalid_state",
      "The graph memory state is invalid.",
    );
  }
};

const requireOsEncryption = (safeStorage: SafeStorageLike): void => {
  let available = false;
  let backend: string | undefined;
  try {
    available = safeStorage.isEncryptionAvailable();
    backend = safeStorage.getSelectedStorageBackend?.();
  } catch {
    available = false;
  }
  if (!available || backend === "basic_text") {
    throw new DesktopGraphMemoryPersistenceError(
      "encryption_unavailable",
      "OS-encrypted graph memory custody is unavailable.",
    );
  }
};

const aad = (scope: DesktopGraphMemoryScope, revision: number): Buffer =>
  Buffer.from(
    JSON.stringify({
      schema: GRAPH_MEMORY_AAD_SCHEMA,
      ownerScope: scope.ownerScope,
      projectScope: scope.projectScope,
      revision,
    }),
    "utf8",
  );

const seal = (
  dataKey: Buffer,
  scope: DesktopGraphMemoryScope,
  state: DesktopGraphMemoryPersistedState,
): Omit<StoredRow, "revision"> => {
  const nonce = randomBytes(12);
  const cipher = createCipheriv("aes-256-gcm", dataKey, nonce);
  cipher.setAAD(aad(scope, state.revision));
  const ciphertext = Buffer.concat([cipher.update(state.payload, "utf8"), cipher.final()]);
  return { nonce, ciphertext, auth_tag: cipher.getAuthTag() };
};

const open = (
  dataKey: Buffer,
  scope: DesktopGraphMemoryScope,
  row: StoredRow,
): DesktopGraphMemoryPersistedState => {
  try {
    const decipher = createDecipheriv("aes-256-gcm", dataKey, row.nonce);
    decipher.setAAD(aad(scope, row.revision));
    decipher.setAuthTag(row.auth_tag);
    const payload = Buffer.concat([decipher.update(row.ciphertext), decipher.final()]).toString(
      "utf8",
    );
    const state = { revision: row.revision, payload };
    requireState(state);
    return state;
  } catch (error) {
    throw new DesktopGraphMemoryPersistenceError(
      "invalid_state",
      "The encrypted graph memory state is invalid.",
      { cause: error },
    );
  }
};

const initialize = (database: SqliteDatabase): void => {
  database.exec("PRAGMA journal_mode = WAL;");
  database.exec("PRAGMA secure_delete = ON;");
  const secureDelete = database.all<{ secure_delete: number }>("PRAGMA secure_delete")[0]
    ?.secure_delete;
  if (secureDelete !== 1) {
    throw new DesktopGraphMemoryPersistenceError(
      "storage_unavailable",
      "Secure deletion is unavailable for graph memory storage.",
    );
  }
  database.exec("PRAGMA foreign_keys = ON;");
  database.exec(`
    CREATE TABLE IF NOT EXISTS graph_memory_metadata (
      key_ref TEXT PRIMARY KEY,
      wrapped_key TEXT NOT NULL
    );
  `);
  const versionValue = database.all<StoredKeyRow>(
    "SELECT wrapped_key FROM graph_memory_metadata WHERE key_ref = ?",
    [GRAPH_MEMORY_SCHEMA_REF],
  )[0]?.wrapped_key;
  if (versionValue !== undefined && Number(versionValue) !== GRAPH_MEMORY_SCHEMA_VERSION) {
    throw new DesktopGraphMemoryPersistenceError(
      "incompatible_version",
      "The graph memory database version is not supported.",
    );
  }
  database.transaction(() => {
    database.exec(`
    CREATE TABLE IF NOT EXISTS graph_memory_scopes (
      owner_scope TEXT NOT NULL,
      project_scope TEXT NOT NULL,
      revision INTEGER NOT NULL CHECK (revision >= 0),
      nonce BLOB NOT NULL,
      ciphertext BLOB NOT NULL,
      auth_tag BLOB NOT NULL,
      PRIMARY KEY (owner_scope, project_scope)
    );
    `);
    if (versionValue === undefined) {
      database.run("INSERT INTO graph_memory_metadata (key_ref, wrapped_key) VALUES (?, ?)", [
        GRAPH_MEMORY_SCHEMA_REF,
        String(GRAPH_MEMORY_SCHEMA_VERSION),
      ]);
    }
  });
};

const truncateWriteAheadLog = (database: SqliteDatabase): void => {
  const result = database.all<{ busy: number; log: number; checkpointed: number }>(
    "PRAGMA wal_checkpoint(TRUNCATE)",
  )[0];
  if (result?.busy !== 0 || result.log !== 0 || result.checkpointed !== 0) {
    throw new DesktopGraphMemoryPersistenceError(
      "storage_unavailable",
      "Graph memory history cleanup did not complete.",
    );
  }
};

const scrubRequired = (database: SqliteDatabase): boolean =>
  database.all<StoredKeyRow>("SELECT wrapped_key FROM graph_memory_metadata WHERE key_ref = ?", [
    GRAPH_MEMORY_SCRUB_REQUIRED_REF,
  ])[0]?.wrapped_key === "1";

const markScrubRequired = (database: SqliteDatabase): void => {
  database.run(
    `INSERT INTO graph_memory_metadata (key_ref, wrapped_key) VALUES (?, ?)
     ON CONFLICT(key_ref) DO UPDATE SET wrapped_key = excluded.wrapped_key`,
    [GRAPH_MEMORY_SCRUB_REQUIRED_REF, "1"],
  );
};

const finishPendingScrub = (
  database: SqliteDatabase,
  checkpoint: (database: SqliteDatabase) => void,
): void => {
  checkpoint(database);
  if (!scrubRequired(database)) return;
  database.run("DELETE FROM graph_memory_metadata WHERE key_ref = ?", [
    GRAPH_MEMORY_SCRUB_REQUIRED_REF,
  ]);
  checkpoint(database);
};

const dataKeyFor = (database: SqliteDatabase, safeStorage: SafeStorageLike): Buffer => {
  const stored = database.all<StoredKeyRow>(
    "SELECT wrapped_key FROM graph_memory_metadata WHERE key_ref = ?",
    [GRAPH_MEMORY_DATA_KEY_REF],
  )[0];
  if (stored !== undefined) {
    try {
      const encoded = safeStorage.decryptString(Buffer.from(stored.wrapped_key, "base64"));
      const key = Buffer.from(encoded, "base64");
      if (key.byteLength !== 32) throw new Error("invalid data-key length");
      return key;
    } catch (error) {
      throw new DesktopGraphMemoryPersistenceError(
        "encryption_unavailable",
        "The graph memory data key cannot be recovered.",
        { cause: error },
      );
    }
  }

  const key = randomBytes(32);
  const wrapped = safeStorage.encryptString(key.toString("base64")).toString("base64");
  database.run("INSERT INTO graph_memory_metadata (key_ref, wrapped_key) VALUES (?, ?)", [
    GRAPH_MEMORY_DATA_KEY_REF,
    wrapped,
  ]);
  return key;
};

const disabledPersistence = (): DesktopGraphMemoryPersistence => ({
  enabled: false,
  load: () => null,
  save: () => undefined,
  compareAndSet: () => false,
  remove: () => false,
  stats: () => ({ reads: 0, writes: 0 }),
  close: () => undefined,
});

/**
 * Open the owner-local encrypted graph-memory state driver.
 *
 * The disabled return happens before path, SQLite, safe-storage, or crypto
 * access. The enabled driver stores one atomically replaced sealed state per
 * owner and project. SQLite only contains scope refs, revision metadata, and
 * AES-GCM ciphertext. Electron safeStorage wraps only the random data key.
 */
export const openDesktopGraphMemoryPersistence = (
  input: Readonly<{
    enabled: boolean;
    databasePath: string;
    safeStorage: SafeStorageLike;
    openDatabase?: (databasePath: string) => SqliteDatabase;
    checkpoint?: (database: SqliteDatabase) => void;
  }>,
): DesktopGraphMemoryPersistence => {
  if (!input.enabled) return disabledPersistence();

  let database: SqliteDatabase | undefined;
  try {
    requireOsEncryption(input.safeStorage);
    const databasePath = path.resolve(input.databasePath);
    const parent = path.dirname(databasePath);
    mkdirSync(parent, { recursive: true, mode: 0o700 });
    if (process.platform !== "win32") chmodSync(parent, 0o700);
    database = (input.openDatabase ?? openSqliteDatabase)(databasePath);
    initialize(database);
    const checkpoint = input.checkpoint ?? truncateWriteAheadLog;
    finishPendingScrub(database, checkpoint);
    const dataKey = dataKeyFor(database, input.safeStorage);
    if (process.platform !== "win32") chmodSync(databasePath, 0o600);
    let reads = 0;
    let writes = 0;

    const completeMutation = (): void => finishPendingScrub(database!, checkpoint);

    const save = (
      scope: DesktopGraphMemoryScope,
      state: DesktopGraphMemoryPersistedState,
    ): void => {
      requireScope(scope);
      requireState(state);
      const sealed = seal(dataKey, scope, state);
      database!.run(
        `INSERT INTO graph_memory_scopes
           (owner_scope, project_scope, revision, nonce, ciphertext, auth_tag)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(owner_scope, project_scope) DO UPDATE SET
           revision = excluded.revision,
           nonce = excluded.nonce,
           ciphertext = excluded.ciphertext,
           auth_tag = excluded.auth_tag`,
        [
          scope.ownerScope,
          scope.projectScope,
          state.revision,
          sealed.nonce,
          sealed.ciphertext,
          sealed.auth_tag,
        ],
      );
    };

    return {
      enabled: true,
      load: (scope) => {
        requireScope(scope);
        reads += 1;
        try {
          if (scrubRequired(database!)) completeMutation();
          const row = database!.all<StoredRow>(
            `SELECT revision, nonce, ciphertext, auth_tag
               FROM graph_memory_scopes
              WHERE owner_scope = ? AND project_scope = ?`,
            [scope.ownerScope, scope.projectScope],
          )[0];
          return row === undefined ? null : open(dataKey, scope, row);
        } catch (error) {
          throw publicFailure(error);
        }
      },
      save: (scope, state) => {
        try {
          database!.transaction(() => {
            save(scope, state);
            markScrubRequired(database!);
          });
          completeMutation();
          writes += 1;
        } catch (error) {
          throw publicFailure(error);
        }
      },
      compareAndSet: (scope, expectedRevision, state) => {
        requireScope(scope);
        requireState(state);
        if (
          expectedRevision !== null &&
          (!Number.isSafeInteger(expectedRevision) || expectedRevision < 0)
        ) {
          throw new DesktopGraphMemoryPersistenceError(
            "invalid_state",
            "The expected graph memory revision is invalid.",
          );
        }
        try {
          const changed = database!.transaction(() => {
            const current = database!.all<{ revision: number }>(
              `SELECT revision FROM graph_memory_scopes
                WHERE owner_scope = ? AND project_scope = ?`,
              [scope.ownerScope, scope.projectScope],
            )[0];
            if (
              (expectedRevision === null && current !== undefined) ||
              (expectedRevision !== null && current?.revision !== expectedRevision)
            ) {
              return false;
            }
            save(scope, state);
            markScrubRequired(database!);
            return true;
          });
          if (changed) {
            completeMutation();
            writes += 1;
          }
          return changed;
        } catch (error) {
          throw publicFailure(error);
        }
      },
      remove: (scope) => {
        requireScope(scope);
        try {
          const before =
            database!.all<{ count: number }>(
              `SELECT COUNT(*) AS count FROM graph_memory_scopes
              WHERE owner_scope = ? AND project_scope = ?`,
              [scope.ownerScope, scope.projectScope],
            )[0]?.count ?? 0;
          database!.transaction(() => {
            database!.run(
              `DELETE FROM graph_memory_scopes
                WHERE owner_scope = ? AND project_scope = ?`,
              [scope.ownerScope, scope.projectScope],
            );
            markScrubRequired(database!);
          });
          completeMutation();
          writes += 1;
          return before > 0;
        } catch (error) {
          throw publicFailure(error);
        }
      },
      stats: () => ({ reads, writes }),
      close: () => {
        try {
          database!.close();
        } catch (error) {
          throw publicFailure(error);
        }
      },
    };
  } catch (error) {
    try {
      database?.close();
    } catch {
      // Keep the open or migration error as the actionable failure.
    }
    throw publicFailure(error);
  }
};
