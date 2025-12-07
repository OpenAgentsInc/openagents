import { Database } from "bun:sqlite";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { DatabaseError } from "./database.js";

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
    const path = yield* Path.Path;

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
