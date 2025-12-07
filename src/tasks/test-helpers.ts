/**
 * Test utilities for task system with SQLite database
 */

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { BunContext } from "@effect/platform-bun";
import { Effect, Context, Layer } from "effect";
import { Database } from "bun:sqlite";
import { runMigrations } from "../storage/migrations.js";
import { DatabaseService, type DatabaseError, makeDatabaseLive } from "../storage/database.js";
import * as path from "node:path";

/**
 * Creates an in-memory SQLite database for testing
 *
 * @returns Effect with database, path, and cleanup function
 */
export const makeTestDatabase = (): Effect.Effect<
  { db: Database; dbPath: string; cleanup: () => void },
  DatabaseError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const pathService = yield* Path.Path;

    // Create temporary directory for test database
    const tmpDir = yield* fs.makeTempDirectoryScoped({ prefix: "test-db-" });
    const dbPath = path.join(tmpDir, "test.db");

    // Create database
    const db = new Database(dbPath);

    // Run migrations to set up schema
    const migrationsDir = path.join(process.cwd(), ".openagents", "migrations");
    yield* runMigrations(db, migrationsDir);

    // Return database handle and cleanup function
    return {
      db,
      dbPath,
      cleanup: () => {
        try {
          db.close();
        } catch (e) {
          // Database might already be closed
          console.error("Error closing test database:", e);
        }
      }
    };
  });

/**
 * Creates a DatabaseService layer from a database path
 *
 * This uses the actual DatabaseLive implementation with a custom path,
 * which is useful for tests that need a fully functional database.
 *
 * @param dbPath - Path to the SQLite database file
 * @returns Layer providing DatabaseService
 */
export const makeDatabaseLayerFromPath = (dbPath: string): Layer.Layer<DatabaseService> => {
  return makeDatabaseLive(dbPath);
};

/**
 * Creates a test database and returns a Layer with DatabaseService
 *
 * This combines makeTestDatabase + DatabaseService layer for convenience
 * in tests that need a fully functional database.
 *
 * Usage:
 * ```typescript
 * const result = await Effect.gen(function* () {
 *   const db = yield* DatabaseService;
 *   return yield* db.getTask("some-id");
 * }).pipe(
 *   Effect.provide(makeTestDatabaseLayer()),
 *   Effect.provide(BunContext.layer),
 *   Effect.runPromise
 * );
 * ```
 */
export const makeTestDatabaseLayer = (): Effect.Effect<
  {
    layer: Layer.Layer<DatabaseService>;
    cleanup: () => void;
  },
  DatabaseError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const { db, dbPath, cleanup } = yield* makeTestDatabase();

    return {
      layer: makeDatabaseLayerFromPath(dbPath),
      cleanup,
    };
  });

/**
 * Helper to run an Effect with a test database
 *
 * This automatically creates a test database, provides it to the program,
 * and cleans up afterwards.
 *
 * Usage:
 * ```typescript
 * const result = await runWithTestDb(Effect.gen(function* () {
 *   const db = yield* DatabaseService;
 *   return yield* db.getTask("some-id");
 * }));
 * ```
 */
export const runWithTestDb = <A, E>(
  program: Effect.Effect<A, E, DatabaseService>
): Promise<A> =>
  Effect.gen(function* () {
    const { layer, cleanup } = yield* makeTestDatabaseLayer();

    try {
      return yield* program.pipe(Effect.provide(layer));
    } finally {
      cleanup();
    }
  }).pipe(
    Effect.provide(FileSystem.FileSystem),
    Effect.provide(Path.Path),
    Effect.runPromise
  );

/**
 * A convenience helper that provides both DatabaseService and BunContext
 * for tests that need both.
 *
 * Usage:
 * ```typescript
 * const result = await runWithTestContext(Effect.gen(function* () {
 *   const db = yield* DatabaseService;
 *   const fs = yield* FileSystem.FileSystem;
 *   // ... test code ...
 * }));
 * ```
 */
export const runWithTestContext = <A, E>(
  program: Effect.Effect<A, E, any>
): Promise<A> =>
  Effect.gen(function* () {
    const { layer, cleanup } = yield* makeTestDatabaseLayer();

    try {
      return yield* program.pipe(
        Effect.provide(layer),
        Effect.provide(BunContext.layer)
      );
    } finally {
      cleanup();
    }
  }).pipe(
    Effect.provide(BunContext.layer),
    Effect.runPromise
  );
