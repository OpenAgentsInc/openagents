import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import { listTasks } from "../tasks/service.js";
import { defaultProjectConfig, loadProjectConfig } from "../tasks/project.js";
import type { Task } from "../tasks/schema.js";
import {
  buildTaskRollup,
  type MechaCoderState,
  type RunLogInfo,
  type TaskDependencyInfo,
  type TaskInfo,
} from "./mechacoder-map.js";

export class MechaCoderStateError extends Error {
  readonly _tag = "MechaCoderStateError";
  constructor(
    readonly reason: "read_error" | "parse_error",
    message: string,
  ) {
    super(message);
    this.name = "MechaCoderStateError";
  }
}

const RunLogMetadata = S.Struct({
  id: S.String,
  taskId: S.optional(S.String),
  taskTitle: S.optional(S.String),
  status: S.Literal("success", "incomplete", "failed", "no_tasks"),
  startedAt: S.String,
  finishedAt: S.String,
  workDir: S.String,
  logFilePath: S.optional(S.NullOr(S.String)),
  sessionFilePath: S.optional(S.NullOr(S.String)),
  commits: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  totalTurns: S.optionalWith(S.Number, { default: () => 0 }),
  finalMessage: S.optionalWith(S.String, { default: () => "" }),
  error: S.optional(S.NullOr(S.String)),
});

type RunLogMetadata = S.Schema.Type<typeof RunLogMetadata>;
const decodeRunLogMetadata = S.decodeUnknownSync(RunLogMetadata);

export interface LoadMechaCoderStateOptions {
  readonly rootDir?: string;
  readonly maxRunLogs?: number;
}

const toTaskInfo = (task: Task, tasksById: Map<string, Task>): TaskInfo => ({
  id: task.id,
  title: task.title,
  status: task.status,
  priority: task.priority,
  type: task.type,
  labels: task.labels ?? [],
  deps: (task.deps ?? []).map((dep): TaskDependencyInfo => {
    const status = tasksById.get(dep.id)?.status;
    return status !== undefined ? { id: dep.id, type: dep.type, status } : { id: dep.id, type: dep.type };
  }),
  createdAt: task.createdAt,
  updatedAt: task.updatedAt,
});

const parseRunLogFile = (
  filePath: string,
): Effect.Effect<RunLogInfo, MechaCoderStateError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const content = yield* fs.readFileString(filePath).pipe(
      Effect.mapError(
        (e) =>
          new MechaCoderStateError(
            "read_error",
            `Failed to read run log ${filePath}: ${e.message}`,
          ),
      ),
    );

    const parsed = yield* Effect.try({
      try: () => JSON.parse(content) as unknown,
      catch: (error) =>
        new MechaCoderStateError(
          "parse_error",
          `Invalid JSON in run log ${filePath}: ${error}`,
        ),
    });

    const metadata = yield* Effect.try({
      try: () => decodeRunLogMetadata(parsed),
      catch: (error) =>
        new MechaCoderStateError(
          "parse_error",
          `Invalid run log ${filePath}: ${(error as Error).message}`,
        ),
    });

    return {
      id: metadata.id,
      taskId: metadata.taskId ?? null,
      status: metadata.status,
      startedAt: metadata.startedAt,
      finishedAt: metadata.finishedAt,
      totalTurns: metadata.totalTurns ?? 0,
    };
  });

const loadRunLogs = (
  runLogDir: string,
  maxRunLogs: number,
): Effect.Effect<RunLogInfo[], MechaCoderStateError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    const exists = yield* fs.exists(runLogDir).pipe(
      Effect.mapError(
        (e) =>
          new MechaCoderStateError(
            "read_error",
            `Failed to check run-logs directory: ${e.message}`,
          ),
      ),
    );
    if (!exists) return [];

    const dayEntries = yield* fs.readDirectory(runLogDir).pipe(
      Effect.mapError(
        (e) =>
          new MechaCoderStateError(
            "read_error",
            `Failed to read run-logs directory: ${e.message}`,
          ),
      ),
    );

    const files: string[] = [];
    for (const entry of dayEntries) {
      const dayPath = path.join(runLogDir, entry);
      const stat = yield* fs.stat(dayPath).pipe(
        Effect.mapError(
          (e) =>
            new MechaCoderStateError(
              "read_error",
              `Failed to stat ${dayPath}: ${e.message}`,
            ),
        ),
        Effect.catchAll(() => Effect.succeed(null)),
      );
      if (!stat || stat.type !== "Directory") continue;

      const dayFiles = yield* fs.readDirectory(dayPath).pipe(
        Effect.mapError(
          (e) =>
            new MechaCoderStateError(
              "read_error",
              `Failed to read ${dayPath}: ${e.message}`,
            ),
        ),
      );

      for (const file of dayFiles) {
        if (!file.endsWith(".json")) continue;
        files.push(path.join(dayPath, file));
      }
    }

    const runLogs = yield* Effect.forEach(files, parseRunLogFile, {
      concurrency: "unbounded",
    }).pipe(
      Effect.mapError(
        (e) =>
          new MechaCoderStateError(
            e.reason,
            e.message,
          ),
      ),
    );

    return runLogs
      .sort(
        (a, b) =>
          new Date(b.finishedAt).getTime() - new Date(a.finishedAt).getTime(),
      )
      .slice(0, maxRunLogs);
  });

const resolveRunLogDir = (
  path: Path.Path,
  rootDir: string,
  configured: string | undefined,
): string => {
  if (configured && configured.startsWith("/")) return configured;
  return path.join(rootDir, configured ?? ".openagents/run-logs");
};

export const loadMechaCoderState = (
  options?: LoadMechaCoderStateOptions,
): Effect.Effect<MechaCoderState, MechaCoderStateError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const path = yield* Path.Path;
    const rootDir = options?.rootDir ?? ".";

    const projectConfig = yield* loadProjectConfig(rootDir).pipe(
      Effect.catchAll(() => Effect.succeed(null)),
    );

    const resolvedProject =
      projectConfig ?? defaultProjectConfig(path.basename(rootDir));

    const projectRoot = path.join(rootDir, resolvedProject.rootDir ?? ".");
    const tasksPath = path.join(projectRoot, ".openagents", "tasks.jsonl");
    const runLogDir = resolveRunLogDir(
      path,
      projectRoot,
      resolvedProject.runLogDir,
    );

    const tasks = yield* listTasks(tasksPath).pipe(
      Effect.mapError(
        (e) =>
          new MechaCoderStateError(
            "read_error",
            `Failed to load tasks: ${e.message}`,
          ),
      ),
    );

    const tasksById = new Map(tasks.map((t) => [t.id, t]));
    const taskInfos = tasks.map((task) => toTaskInfo(task, tasksById));
    const rollup = buildTaskRollup(taskInfos);

    const recentRuns = yield* loadRunLogs(
      runLogDir,
      options?.maxRunLogs ?? 10,
    );
    const activeTaskId = taskInfos.find((t) => t.status === "in_progress")?.id ?? null;

    return {
      repos: [
        {
          name: resolvedProject.projectId,
          path: projectRoot,
          tasks: taskInfos,
          rollup,
        },
      ],
      currentPhase: activeTaskId ? "edit" : "idle",
      activeTaskId,
      recentRuns,
      rollup,
    };
  });
