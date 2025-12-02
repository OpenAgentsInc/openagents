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

    yield* fs.makeDirectory(openagentsDir, { recursive: true }).pipe(
      Effect.mapError(
        (e) =>
          new InitProjectError(
            "write_error",
            `Failed to create .openagents directory: ${e.message}`,
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

    return { projectId: id, projectPath, tasksPath };
  });
