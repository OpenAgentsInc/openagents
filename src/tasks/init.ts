import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { Database } from "bun:sqlite";
import { defaultProjectConfig, saveProjectConfig } from "./project.js";
import { runMigrations } from "../storage/migrations.js";

export class InitProjectError extends Error {
  readonly _tag = "InitProjectError";
  constructor(
    readonly reason: "exists" | "write_error",
    message: string,
  ) {
    super(message);
    this.name = "InitProjectError";
  }
}

export interface InitProjectOptions {
  rootDir: string;
  projectId?: string;
  allowExisting?: boolean;
}

const OPENAGENTS_GITIGNORE = `# MechaCoder session and run logs (local only)
sessions/
run-logs/
usage.jsonl
`;

export const initOpenAgentsProject = ({
  rootDir,
  projectId,
  allowExisting = false,
}: InitProjectOptions): Effect.Effect<
  { projectId: string; projectPath: string; dbPath: string },
  InitProjectError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const resolvedRoot = path.resolve(rootDir);
    const openagentsDir = path.join(resolvedRoot, ".openagents");
    const projectPath = path.join(openagentsDir, "project.json");
    const dbPath = path.join(openagentsDir, "openagents.db");
    const migrationsDir = path.join(openagentsDir, "migrations");
    const sessionsDir = path.join(openagentsDir, "sessions");
    const runLogsDir = path.join(openagentsDir, "run-logs");
    const gitignorePath = path.join(openagentsDir, ".gitignore");

    const projectExists = yield* fs.exists(projectPath).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to check project.json: ${e.message}`,
          ),
      ),
    );

    if (projectExists && !allowExisting) {
      return yield* Effect.fail(
        new InitProjectError(
          "exists",
          `.openagents/project.json already exists at ${projectPath}`,
        ),
      );
    }

    // Create main .openagents directory
    yield* fs.makeDirectory(openagentsDir, { recursive: true }).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to create .openagents directory: ${e.message}`,
          ),
      ),
    );

    // Create migrations directory
    yield* fs.makeDirectory(migrationsDir, { recursive: true }).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to create migrations directory: ${e.message}`,
          ),
      ),
    );

    // Create sessions directory
    yield* fs.makeDirectory(sessionsDir, { recursive: true }).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to create sessions directory: ${e.message}`,
          ),
      ),
    );

    // Create run-logs directory
    yield* fs.makeDirectory(runLogsDir, { recursive: true }).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to create run-logs directory: ${e.message}`,
          ),
      ),
    );

    const id = projectId ?? path.basename(resolvedRoot);
    const config = defaultProjectConfig(id);

    // Save project config (stays as JSON)
    yield* saveProjectConfig(resolvedRoot, config).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to write project.json: ${e.message}`,
          ),
      ),
    );

    // Create SQLite database and run migrations
    yield* Effect.try({
      try: () => {
        const db = new Database(dbPath);

        // Run migrations to set up schema
        Effect.runSync(runMigrations(db, migrationsDir).pipe(
          Effect.provide({
            FileSystem: fs,
            Path: path,
          } as any),
        ));

        db.close();
      },
      catch: (e) =>
        new InitProjectError(
          "write_error",
          `Failed to create database: ${e}`,
        ),
    });

    // Create .gitignore for sessions and run-logs
    yield* fs.writeFile(gitignorePath, new TextEncoder().encode(OPENAGENTS_GITIGNORE)).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to write .gitignore: ${e.message}`,
          ),
      ),
    );

    return { projectId: id, projectPath, dbPath };
  });
