import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import {
  decodeTask,
  decodeTaskCreate,
  decodeTaskUpdate,
  isTaskReady,
  type Task,
  type TaskCreate,
  type TaskFilter,
  type TaskUpdate,
} from "./schema.js";
import {
  canHaveChildren,
  findNextChildNumber,
  generateChildId,
  generateHashId,
} from "./id.js";

export class TaskServiceError extends Error {
  readonly _tag = "TaskServiceError";
  constructor(
    readonly reason:
      | "not_found"
      | "read_error"
      | "write_error"
      | "parse_error"
      | "validation_error"
      | "conflict",
    message: string,
  ) {
    super(message);
    this.name = "TaskServiceError";
  }
}

const nowIso = (timestamp?: Date) => (timestamp ?? new Date()).toISOString();

const parseJsonLine = (
  line: string,
): Effect.Effect<unknown, TaskServiceError, never> =>
  Effect.try({
    try: () => JSON.parse(line),
    catch: (error) =>
      new TaskServiceError("parse_error", `Invalid JSON: ${error}`),
  });

const ensureDir = (
  filePath: string,
): Effect.Effect<void, TaskServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const dir = path.dirname(filePath);
    yield* fs.makeDirectory(dir, { recursive: true }).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "write_error",
            `Failed to create directory ${dir}: ${e.message}`,
          ),
      ),
    );
  });

const deriveUniqueId = (
  prefix: string,
  hash: string,
  existingIds: string[],
): string => {
  for (let length = 6; length <= hash.length; length += 1) {
    const candidate = `${prefix}-${hash.slice(0, length)}`;
    if (!existingIds.includes(candidate)) return candidate;
  }

  let counter = 1;
  while (true) {
    const candidate = `${prefix}-${hash.slice(0, 6)}-${counter}`;
    if (!existingIds.includes(candidate)) return candidate;
    counter += 1;
  }
};

const filterTasks = (tasks: Task[], filter?: TaskFilter): Task[] => {
  if (!filter) return tasks;

  return tasks.filter((task) => {
    if (filter.status && task.status !== filter.status) return false;
    if (filter.priority !== undefined && task.priority !== filter.priority)
      return false;
    if (filter.type && task.type !== filter.type) return false;
    if (filter.assignee && task.assignee !== filter.assignee) return false;
    if (filter.labels) {
      const labels = filter.labels;
      if (!labels.every((label) => task.labels?.includes(label))) return false;
    }
    return true;
  });
};

const sortReadyTasks = (tasks: Task[]): Task[] => {
  const dateValue = (value: string) => {
    const time = new Date(value).getTime();
    return Number.isNaN(time) ? Number.MAX_SAFE_INTEGER : time;
  };

  return [...tasks].sort((a, b) => {
    const priorityDiff = a.priority - b.priority;
    if (priorityDiff !== 0) return priorityDiff;
    const timeDiff = dateValue(a.createdAt) - dateValue(b.createdAt);
    if (timeDiff !== 0) return timeDiff;
    return a.id.localeCompare(b.id);
  });
};

export const readTasks = (
  tasksPath: string,
): Effect.Effect<Task[], TaskServiceError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const exists = yield* fs.exists(tasksPath).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to check tasks file: ${e.message}`,
          ),
      ),
    );
    if (!exists) return [];

    const content = yield* fs.readFileString(tasksPath).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to read tasks file: ${e.message}`,
          ),
      ),
    );

    const lines = content
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

    if (lines.length === 0) return [];

    const tasks: Task[] = [];
    for (const line of lines) {
      const parsed = yield* parseJsonLine(line);
      const task = yield* Effect.try({
        try: () => decodeTask(parsed),
        catch: (error) =>
          new TaskServiceError(
            "validation_error",
            `Invalid task entry: ${(error as Error).message}`,
          ),
      });
      tasks.push(task);
    }

    return tasks;
  });

export const writeTasks = (
  tasksPath: string,
  tasks: Task[],
): Effect.Effect<void, TaskServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    yield* ensureDir(tasksPath);

    const payload = tasks.map((task) => JSON.stringify(task)).join("\n") + "\n";

    yield* fs.writeFile(tasksPath, new TextEncoder().encode(payload)).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "write_error",
            `Failed to write tasks file: ${e.message}`,
          ),
      ),
    );
  });

export interface CreateTaskOptions {
  tasksPath: string;
  task: TaskCreate;
  idPrefix?: string;
  parentId?: string;
  workspaceId?: string;
  timestamp?: Date;
}

export const createTask = ({
  tasksPath,
  task,
  idPrefix = "oa",
  parentId,
  workspaceId = "",
  timestamp,
}: CreateTaskOptions): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const now = timestamp ?? new Date();
    const existing = yield* readTasks(tasksPath);
    const validated = yield* Effect.try({
      try: () => decodeTaskCreate(task),
      catch: (error) =>
        new TaskServiceError(
          "validation_error",
          `Invalid task: ${(error as Error).message}`,
        ),
    });

    const existingIds = existing.map((t) => t.id);
    let id: string;

    if (parentId) {
      const parent = existing.find((t) => t.id === parentId);
      if (!parent) {
        return yield* Effect.fail(
          new TaskServiceError("not_found", `Parent task not found: ${parentId}`),
        );
      }
      if (!canHaveChildren(parentId)) {
        return yield* Effect.fail(
          new TaskServiceError(
            "conflict",
            `Parent id ${parentId} cannot have more children`,
          ),
        );
      }
      const childNumber = findNextChildNumber(parentId, existingIds);
      id = generateChildId(parentId, childNumber);
    } else {
      const hash = yield* generateHashId(
        idPrefix,
        validated.title,
        validated.description ?? "",
        now,
        workspaceId,
      );
      id = deriveUniqueId(idPrefix, hash, existingIds);
    }

    const created: Task = {
      ...validated,
      id,
      createdAt: nowIso(now),
      updatedAt: nowIso(now),
      closedAt: validated.status === "closed" ? nowIso(now) : null,
      commits: [],
      labels: validated.labels ?? [],
      deps: validated.deps ?? [],
    };

    const decoded = yield* Effect.try({
      try: () => decodeTask(created),
      catch: (error) =>
        new TaskServiceError(
          "validation_error",
          `Failed to validate new task: ${(error as Error).message}`,
        ),
    });

    yield* writeTasks(tasksPath, [...existing, decoded]);
    return decoded;
  });

export interface UpdateTaskOptions {
  tasksPath: string;
  id: string;
  update: TaskUpdate;
  appendCommits?: string[];
  timestamp?: Date;
}

export const updateTask = ({
  tasksPath,
  id,
  update,
  appendCommits = [],
  timestamp,
}: UpdateTaskOptions): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const now = timestamp ?? new Date();
    const tasks = yield* readTasks(tasksPath);
    const index = tasks.findIndex((t) => t.id === id);
    if (index === -1) {
      return yield* Effect.fail(
        new TaskServiceError("not_found", `Task not found: ${id}`),
      );
    }

    const validatedUpdate = yield* Effect.try({
      try: () => decodeTaskUpdate(update),
      catch: (error) =>
        new TaskServiceError(
          "validation_error",
          `Invalid update: ${(error as Error).message}`,
        ),
    });

    const current = { ...tasks[index] };

    if (validatedUpdate.title !== undefined) current.title = validatedUpdate.title;
    if (validatedUpdate.description !== undefined)
      current.description = validatedUpdate.description;
    let closeReason = current.closeReason;
    if (validatedUpdate.closeReason !== undefined) {
      closeReason = validatedUpdate.closeReason;
    }

    if (validatedUpdate.status !== undefined) {
      current.status = validatedUpdate.status;
      current.closedAt =
        validatedUpdate.status === "closed" ? nowIso(now) : null;
      if (validatedUpdate.status !== "closed" && validatedUpdate.closeReason === undefined) {
        closeReason = undefined;
      }
    }
    if (validatedUpdate.priority !== undefined)
      current.priority = validatedUpdate.priority;
    if (validatedUpdate.type !== undefined) current.type = validatedUpdate.type;
    if (validatedUpdate.assignee !== undefined) {
      if (validatedUpdate.assignee === null) {
        delete current.assignee;
      } else {
        current.assignee = validatedUpdate.assignee;
      }
    }
    if (validatedUpdate.labels !== undefined)
      current.labels = [...validatedUpdate.labels];
    if (validatedUpdate.deps !== undefined) current.deps = [...validatedUpdate.deps];
    if (validatedUpdate.commits !== undefined)
      current.commits = [...validatedUpdate.commits];
    current.closeReason = closeReason;
    if (validatedUpdate.design !== undefined) current.design = validatedUpdate.design;
    if (validatedUpdate.acceptanceCriteria !== undefined)
      current.acceptanceCriteria = validatedUpdate.acceptanceCriteria;
    if (validatedUpdate.notes !== undefined) current.notes = validatedUpdate.notes;
    if (validatedUpdate.estimatedMinutes !== undefined)
      current.estimatedMinutes = validatedUpdate.estimatedMinutes;

    if (appendCommits.length > 0) {
      const nextCommits = new Set(current.commits ?? []);
      appendCommits.forEach((commit) => nextCommits.add(commit));
      current.commits = Array.from(nextCommits);
    }

    current.updatedAt = nowIso(now);

    const validatedTask = yield* Effect.try({
      try: () => decodeTask(current),
      catch: (error) =>
        new TaskServiceError(
          "validation_error",
          `Invalid task after update: ${(error as Error).message}`,
        ),
    });

    tasks[index] = validatedTask;
    yield* writeTasks(tasksPath, tasks);
    return validatedTask;
  });

export interface CloseTaskOptions {
  tasksPath: string;
  id: string;
  reason?: string;
  commits?: string[];
  timestamp?: Date;
}

export const closeTask = ({
  tasksPath,
  id,
  reason,
  commits = [],
  timestamp,
}: CloseTaskOptions): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> => {
  const updateOptions: UpdateTaskOptions = {
    tasksPath,
    id,
    update: { status: "closed", closeReason: reason },
    appendCommits: commits,
  };

  if (timestamp) {
    updateOptions.timestamp = timestamp;
  }

  return updateTask(updateOptions);
};

export const listTasks = (
  tasksPath: string,
  filter?: TaskFilter,
): Effect.Effect<Task[], TaskServiceError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const tasks = yield* readTasks(tasksPath);
    const filtered = filterTasks(tasks, filter);
    if (filter?.limit && filter.limit > 0) {
      return filtered.slice(0, filter.limit);
    }
    return filtered;
  });

export const readyTasks = (
  tasksPath: string,
): Effect.Effect<Task[], TaskServiceError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const tasks = yield* readTasks(tasksPath);
    const ready = tasks.filter((task) => isTaskReady(task, tasks));
    return sortReadyTasks(ready);
  });

export const pickNextTask = (
  tasksPath: string,
): Effect.Effect<Task | null, TaskServiceError, FileSystem.FileSystem> =>
  readyTasks(tasksPath).pipe(Effect.map((tasks) => tasks[0] ?? null));
