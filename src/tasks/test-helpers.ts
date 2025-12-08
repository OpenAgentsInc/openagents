import { Database } from "bun:sqlite";
import * as os from "node:os";
import * as fs from "node:fs";
import * as path from "node:path";
import { Effect, Layer } from "effect";
import { BunContext } from "@effect/platform-bun";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import {
  DatabaseService,
  DatabaseError,
  makeDatabaseLive,
} from "../storage/database.js";
import { runMigrations } from "../storage/migrations.js";

const MIGRATIONS_DIR = path.join(process.cwd(), ".openagents", "migrations");

const createDatabaseFile = async (): Promise<{ dbPath: string; cleanup: () => void }> => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "openagents-test-db-"));
  const dbPath = path.join(tmpDir, "test.db");

  // Create and run migrations on a temporary database connection
  const db = new Database(dbPath);
  await Effect.runPromise(
    runMigrations(db, MIGRATIONS_DIR).pipe(Effect.provide(BunContext.layer)),
  );
  db.close(); // Close this temporary connection - the Layer will create its own

  return {
    dbPath,
    cleanup: () => {
      try {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      } catch {
        // ignore cleanup errors
      }
    },
  };
};

const createTestLayer = async (): Promise<{ layer: Layer.Layer<DatabaseService, DatabaseError>; cleanup: () => void }> => {
  const { dbPath, cleanup } = await createDatabaseFile();
  return {
    layer: makeDatabaseLive(dbPath),
    cleanup,
  };
};

export const makeTestDatabaseLayer = (): Effect.Effect<
  { layer: Layer.Layer<DatabaseService, DatabaseError>; cleanup: () => void },
  DatabaseError,
  never
> =>
  Effect.promise(() => createTestLayer()).pipe(
    Effect.mapError((e) =>
      new DatabaseError("connection", `Failed to create test database: ${e}`)
    )
  );

const withTestLayer = <A, E>(program: Effect.Effect<A, E, DatabaseService>) =>
  Effect.scoped(
    Effect.gen(function* () {
      const { layer, cleanup } = yield* Effect.promise(() => createTestLayer());

      try {
        return yield* program.pipe(
          Effect.provide(Layer.mergeAll(layer, BunContext.layer)),
        );
      } finally {
        cleanup();
      }
    }),
  );

export const runWithTestDb = <A, E>(program: Effect.Effect<A, E, DatabaseService>): Promise<A> =>
  withTestLayer(program).pipe(Effect.runPromise);

export const runWithTestContext = <A, E>(
  program: Effect.Effect<A, E, DatabaseService | FileSystem.FileSystem | Path.Path>,
): Promise<A> =>
  Effect.scoped(
    Effect.gen(function* () {
      const { layer, cleanup } = yield* Effect.promise(() => createTestLayer());

      try {
        return yield* program.pipe(
          Effect.provide(
            Layer.mergeAll(
              layer,
              BunContext.layer,
            ),
          ),
        );
      } finally {
        cleanup();
      }
    }),
  ).pipe(Effect.runPromise);
