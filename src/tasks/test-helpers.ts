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

const createDatabaseFile = (): { dbPath: string; cleanup: () => void } => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "openagents-test-db-"));
  const dbPath = path.join(tmpDir, "test.db");

  const db = new Database(dbPath);
  try {
    Effect.runPromise(
      runMigrations(db, MIGRATIONS_DIR).pipe(Effect.provide(BunContext.layer)),
    );
  } finally {
    db.close();
  }

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

const createTestLayer = (): { layer: Layer.Layer<DatabaseService, DatabaseError>; cleanup: () => void } => {
  const { dbPath, cleanup } = createDatabaseFile();
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
  Effect.try({
    try: () => createTestLayer(),
    catch: (e) =>
      new DatabaseError("connection", `Failed to create test database: ${e}`),
  });

const withTestLayer = <A, E>(program: Effect.Effect<A, E, DatabaseService>) =>
  Effect.scoped(
    Effect.gen(function* () {
      const { layer, cleanup } = createTestLayer();

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
      const { layer, cleanup } = createTestLayer();

      try {
        return yield* program.pipe(
          Effect.provide(
            Layer.mergeAll(
              layer,
              BunContext.layer,
              FileSystem.FileSystem,
              Path.Path,
            ),
          ),
        );
      } finally {
        cleanup();
      }
    }),
  ).pipe(Effect.runPromise);
