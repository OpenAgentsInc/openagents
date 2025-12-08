import { Database } from "bun:sqlite";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { DatabaseError } from "./database.js";

/**
 * Initial schema SQL - used for new database initialization
 */
export const INITIAL_SCHEMA_SQL = `-- Schema version tracking
CREATE TABLE _schema_version (
  version TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO _schema_version (version) VALUES ('1.0.0');

-- Tasks table (matches Task schema)
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open', 'in_progress', 'blocked', 'closed', 'commit_pending')),
  priority INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 4),
  type TEXT NOT NULL CHECK (type IN ('bug', 'feature', 'task', 'epic', 'chore')),
  assignee TEXT,
  close_reason TEXT,

  -- JSON fields
  labels JSON,
  commits JSON,
  comments JSON,
  pending_commit JSON,

  -- Extended fields
  design TEXT,
  acceptance_criteria TEXT,
  notes TEXT,
  estimated_minutes REAL,

  -- Source tracking
  source_repo TEXT,
  source_discovered_from TEXT,
  source_external_ref TEXT,

  -- Timestamps
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT,
  deleted_at TEXT
);

-- Dependencies (many-to-many)
CREATE TABLE task_dependencies (
  task_id TEXT NOT NULL,
  depends_on_task_id TEXT NOT NULL,
  dependency_type TEXT NOT NULL CHECK (
    dependency_type IN ('blocks', 'related', 'parent-child', 'discovered-from')
  ),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),

  PRIMARY KEY (task_id, depends_on_task_id),
  FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
  FOREIGN KEY (depends_on_task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- Deletion tombstones
CREATE TABLE task_deletions (
  task_id TEXT PRIMARY KEY,
  deleted_at TEXT NOT NULL,
  deleted_by TEXT,
  reason TEXT
);

-- Indexes for performance
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_status_priority ON tasks(status, priority);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
CREATE INDEX idx_tasks_deleted_at ON tasks(deleted_at);

-- Composite index for ready task query (CRITICAL)
CREATE INDEX idx_tasks_ready ON tasks(status, priority, created_at)
  WHERE deleted_at IS NULL;

-- Full-text search
CREATE VIRTUAL TABLE tasks_fts USING fts5(
  id UNINDEXED,
  title,
  description,
  content=tasks,
  content_rowid=rowid
);

-- FTS sync triggers
CREATE TRIGGER tasks_fts_insert AFTER INSERT ON tasks BEGIN
  INSERT INTO tasks_fts(rowid, id, title, description)
  VALUES (NEW.rowid, NEW.id, NEW.title, NEW.description);
END;

CREATE TRIGGER tasks_fts_update AFTER UPDATE ON tasks BEGIN
  UPDATE tasks_fts SET title = NEW.title, description = NEW.description
  WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER tasks_fts_delete AFTER DELETE ON tasks BEGIN
  DELETE FROM tasks_fts WHERE rowid = OLD.rowid;
END;
`;

/**
 * Migration metadata
 */
export interface Migration {
  version: string;
  filename: string;
  sql: string;
}

/**
 * Get list of migration files from migrations directory
 */
export const listMigrationFiles = (
  migrationsDir: string,
): Effect.Effect<
  string[],
  DatabaseError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    // Check if migrations directory exists
    const exists = yield* fs.exists(migrationsDir).pipe(
      Effect.mapError(
        (e) =>
          new DatabaseError(
            "migration",
            `Failed to check migrations directory: ${e.message}`,
          ),
      ),
    );

    if (!exists) {
      return [];
    }

    // Read directory
    const entries = yield* fs.readDirectory(migrationsDir).pipe(
      Effect.mapError(
        (e) =>
          new DatabaseError(
            "migration",
            `Failed to read migrations directory: ${e.message}`,
          ),
      ),
    );

    // Filter for .sql files and sort
    const sqlFiles = entries.filter((e) => e.endsWith(".sql")).sort();

    return sqlFiles;
  });

/**
 * Load migration file content
 */
export const loadMigration = (
  migrationsDir: string,
  filename: string,
): Effect.Effect<
  Migration,
  DatabaseError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    const filepath = path.join(migrationsDir, filename);

    const sql = yield* fs.readFileString(filepath).pipe(
      Effect.mapError(
        (e) =>
          new DatabaseError(
            "migration",
            `Failed to read migration file ${filename}: ${e.message}`,
          ),
      ),
    );

    // Extract version from filename (e.g., "001_initial_schema.sql" -> "1.0.0")
    // For now, use filename as version
    const version = filename.replace(".sql", "");

    return {
      version,
      filename,
      sql,
    };
  });

/**
 * Check if migration has been applied
 */
export const isMigrationApplied = (
  db: Database,
  version: string,
): Effect.Effect<boolean, DatabaseError> =>
  Effect.try({
    try: () => {
      // Check if schema version table exists
      const tableExists = db
        .prepare(
          "SELECT name FROM sqlite_master WHERE type='table' AND name='_schema_version'",
        )
        .get();

      if (!tableExists) {
        return false;
      }

      // Check if this version exists
      const stmt = db.prepare("SELECT version FROM _schema_version WHERE version = ?");
      const row = stmt.get(version);

      return !!row;
    },
    catch: (e) =>
      new DatabaseError("query", `Failed to check migration status: ${e}`),
  });

/**
 * Apply a single migration
 */
export const applyMigration = (
  db: Database,
  migration: Migration,
): Effect.Effect<void, DatabaseError> =>
  Effect.gen(function* () {
    // Check if already applied
    const applied = yield* isMigrationApplied(db, migration.version);

    if (applied) {
      console.log(`Migration ${migration.filename} already applied, skipping`);
      return;
    }

    console.log(`Applying migration ${migration.filename}...`);

    // Execute SQL in a transaction
    yield* Effect.try({
      try: () => {
        db.transaction(() => {
          db.exec(migration.sql);
          // Record migration version
          const stmt = db.prepare("INSERT INTO _schema_version (version) VALUES (?)");
          stmt.run(migration.version);
        })();
      },
      catch: (e) =>
        new DatabaseError(
          "migration",
          `Failed to apply migration ${migration.filename}: ${e}`,
          e,
        ),
    });

    console.log(`✓ Migration ${migration.filename} applied successfully`);
  });

/**
 * Run all pending migrations
 */
export const runMigrations = (
  db: Database,
  migrationsDir: string = ".openagents/migrations",
): Effect.Effect<
  void,
  DatabaseError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    console.log(`Running migrations from ${migrationsDir}...`);

    // List migration files
    const files = yield* listMigrationFiles(migrationsDir);

    if (files.length === 0) {
      console.log("No migration files found");
      return;
    }

    console.log(`Found ${files.length} migration file(s)`);

    // Load and apply each migration in order
    for (const filename of files) {
      const migration = yield* loadMigration(migrationsDir, filename);
      yield* applyMigration(db, migration);
    }

    console.log("All migrations applied successfully");
  });

/**
 * Get current schema version
 */
export const getCurrentSchemaVersion = (
  db: Database,
): Effect.Effect<string | null, DatabaseError> =>
  Effect.try({
    try: () => {
      // Check if schema version table exists
      const tableExists = db
        .prepare(
          "SELECT name FROM sqlite_master WHERE type='table' AND name='_schema_version'",
        )
        .get();

      if (!tableExists) {
        return null;
      }

      // Get latest version
      const stmt = db.prepare(
        "SELECT version FROM _schema_version ORDER BY applied_at DESC LIMIT 1",
      );
      const row = stmt.get() as { version: string } | null;

      return row?.version ?? null;
    },
    catch: (e) =>
      new DatabaseError("query", `Failed to get schema version: ${e}`),
  });

/**
 * Check database integrity
 */
export const checkIntegrity = (
  db: Database,
): Effect.Effect<boolean, DatabaseError> =>
  Effect.try({
    try: () => {
      const stmt = db.prepare("PRAGMA integrity_check");
      const result = stmt.get() as { integrity_check: string };

      return result.integrity_check === "ok";
    },
    catch: (e) =>
      new DatabaseError("query", `Integrity check failed: ${e}`),
  });

/**
 * Vacuum database (compact and optimize)
 */
export const vacuum = (
  db: Database,
): Effect.Effect<void, DatabaseError> =>
  Effect.try({
    try: () => {
      db.exec("VACUUM");
    },
    catch: (e) =>
      new DatabaseError("query", `Vacuum failed: ${e}`),
  });

/**
 * Analyze database (update query planner statistics)
 */
export const analyze = (
  db: Database,
): Effect.Effect<void, DatabaseError> =>
  Effect.try({
    try: () => {
      db.exec("ANALYZE");
    },
    catch: (e) =>
      new DatabaseError("query", `Analyze failed: ${e}`),
  });
