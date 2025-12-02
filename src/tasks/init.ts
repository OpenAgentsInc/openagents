import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { defaultProjectConfig, saveProjectConfig } from "./project.js";
import { writeTasks } from "./service.js";

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
`;

export const initOpenAgentsProject = ({
  rootDir,
  projectId,
  allowExisting = false,
}: InitProjectOptions): Effect.Effect<
  { projectId: string; projectPath: string; tasksPath: string },
  InitProjectError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const resolvedRoot = path.resolve(rootDir);
    const openagentsDir = path.join(resolvedRoot, ".openagents");
    const projectPath = path.join(openagentsDir, "project.json");
    const tasksPath = path.join(openagentsDir, "tasks.jsonl");
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

    yield* saveProjectConfig(resolvedRoot, config).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to write project.json: ${e.message}`,
          ),
      ),
    );

    yield* writeTasks(tasksPath, []).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to write tasks.jsonl: ${e.message}`,
          ),
      ),
    );

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

    return { projectId: id, projectPath, tasksPath };
  });
