import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import {
  decodeProjectConfig,
  type ProjectConfig,
} from "./schema.js";

export class ProjectServiceError extends Error {
  readonly _tag = "ProjectServiceError";
  constructor(
    readonly reason:
      | "not_found"
      | "read_error"
      | "write_error"
      | "parse_error"
      | "validation_error",
    message: string,
  ) {
    super(message);
    this.name = "ProjectServiceError";
  }
}

const projectDirName = ".openagents";
const projectFileName = "project.json";

export const projectConfigPath = (
  rootDir: string,
): Effect.Effect<string, never, Path.Path> =>
  Effect.gen(function* () {
    const path = yield* Path.Path;
    return path.join(rootDir, projectDirName, projectFileName);
  });

export const defaultProjectConfig = (
  projectId: string,
): ProjectConfig =>
  decodeProjectConfig({ projectId });

const ensureDir = (
  filePath: string,
): Effect.Effect<void, ProjectServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const dir = path.dirname(filePath);
    yield* fs.makeDirectory(dir, { recursive: true }).pipe(
      Effect.mapError(
        (e) =>
          new ProjectServiceError(
            "write_error",
            `Failed to create directory ${dir}: ${e.message}`,
          ),
      ),
    );
  });

export const loadProjectConfig = (
  rootDir: string,
): Effect.Effect<ProjectConfig | null, ProjectServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const configPath = yield* projectConfigPath(rootDir);

    const exists = yield* fs.exists(configPath).pipe(
      Effect.mapError(
        (e) =>
          new ProjectServiceError(
            "read_error",
            `Failed to check project config: ${e.message}`,
          ),
      ),
    );
    if (!exists) return null;

    const content = yield* fs.readFileString(configPath).pipe(
      Effect.mapError(
        (e) =>
          new ProjectServiceError(
            "read_error",
            `Failed to read project config: ${e.message}`,
          ),
      ),
    );

    const parsed = yield* Effect.try({
      try: () => JSON.parse(content) as unknown,
      catch: (error) =>
        new ProjectServiceError(
          "parse_error",
          `Invalid JSON in project config: ${error}`,
        ),
    });

    return yield* Effect.try({
      try: () => decodeProjectConfig(parsed),
      catch: (error) =>
        new ProjectServiceError(
          "validation_error",
          `Invalid project config: ${(error as Error).message}`,
        ),
    });
  });

export const saveProjectConfig = (
  rootDir: string,
  config: ProjectConfig,
): Effect.Effect<void, ProjectServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const configPath = yield* projectConfigPath(rootDir);
    yield* ensureDir(configPath);

    const payload = JSON.stringify(config, null, 2) + "\n";

    yield* fs.writeFile(configPath, new TextEncoder().encode(payload)).pipe(
      Effect.mapError(
        (e) =>
          new ProjectServiceError(
            "write_error",
            `Failed to write project config: ${e.message}`,
          ),
      ),
    );
  });
