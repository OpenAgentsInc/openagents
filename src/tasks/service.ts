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

const matchesFilter = (task: Task, filter: TaskFilter): boolean => {
  const allowedStatuses = filter.status ? [filter.status] : ["open", "in_progress"];
  if (!allowedStatuses.includes(task.status)) return false;

  if (filter.priority !== undefined && task.priority !== filter.priority) return false;
  if (filter.type && task.type !== filter.type) return false;

  if (filter.unassigned) {
    if (task.assignee && task.assignee.trim().length > 0) return false;
  } else if (filter.assignee && task.assignee !== filter.assignee) {
    return false;
  }

  if (filter.labels && filter.labels.length > 0) {
    const labels = filter.labels;
    if (!labels.every((label) => task.labels?.includes(label))) return false;
  }

  if (filter.labelsAny && filter.labelsAny.length > 0) {
    const labelsAny = filter.labelsAny;
    if (!task.labels?.some((label) => labelsAny.includes(label))) return false;
  }

  return true;
};

const matchesListFilter = (task: Task, filter?: TaskFilter): boolean => {
  if (!filter) return true;

  if (filter.status && task.status !== filter.status) return false;
  if (filter.priority !== undefined && task.priority !== filter.priority) return false;
  if (filter.type && task.type !== filter.type) return false;

  if (filter.unassigned) {
    if (task.assignee && task.assignee.trim().length > 0) return false;
  } else if (filter.assignee && task.assignee !== filter.assignee) {
    return false;
  }

  if (filter.labels && filter.labels.length > 0) {
    if (!filter.labels.every((label) => task.labels?.includes(label))) return false;
  }

  if (filter.labelsAny && filter.labelsAny.length > 0) {
    if (!task.labels?.some((label) => filter.labelsAny?.includes(label))) return false;
  }

  return true;
};

const parseDate = (value: string): number => {
  const time = new Date(value).getTime();
  return Number.isNaN(time) ? Number.MAX_SAFE_INTEGER : time;
};

const sortReadyTasks = (tasks: Task[], sortPolicy: string | undefined): Task[] => {
  const policy = sortPolicy ?? "hybrid";
  const now = Date.now();
  const fortyEightHours = 48 * 60 * 60 * 1000;

  const isRecent = (task: Task) => {
    const created = parseDate(task.createdAt);
    return created >= now - fortyEightHours;
  };

  const byPriorityThenAge = (a: Task, b: Task) => {
    const priorityDiff = a.priority - b.priority;
    if (priorityDiff !== 0) return priorityDiff;
    const timeDiff = parseDate(a.createdAt) - parseDate(b.createdAt);
    if (timeDiff !== 0) return timeDiff;
    return a.id.localeCompare(b.id);
  };

  const byOldest = (a: Task, b: Task) => {
    const timeDiff = parseDate(a.createdAt) - parseDate(b.createdAt);
    if (timeDiff !== 0) return timeDiff;
    return a.id.localeCompare(b.id);
  };

  if (policy === "priority") {
    return [...tasks].sort(byPriorityThenAge);
  }

  if (policy === "oldest") {
    return [...tasks].sort(byOldest);
  }

  // Hybrid: recent issues (<48h) sorted by priority, older by age
  return [...tasks].sort((a, b) => {
    const aRecent = isRecent(a);
    const bRecent = isRecent(b);

    if (aRecent && bRecent) return byPriorityThenAge(a, b);
    if (aRecent && !bRecent) return -1;
    if (!aRecent && bRecent) return 1;
    return byOldest(a, b);
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
    const filtered = tasks.filter((task) => matchesListFilter(task, filter));
    if (filter?.limit && filter.limit > 0) {
      return sortReadyTasks(filtered, filter.sortPolicy).slice(0, filter.limit);
    }
    return sortReadyTasks(filtered, filter?.sortPolicy);
  });

export const readyTasks = (
  tasksPath: string,
  filter?: TaskFilter,
): Effect.Effect<Task[], TaskServiceError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const tasks = yield* readTasks(tasksPath);
    const appliedFilter: TaskFilter = filter ?? {};
    const ready = tasks.filter((task) => isTaskReady(task, tasks)).filter((task) => matchesFilter(task, appliedFilter));
    const sorted = sortReadyTasks(ready, appliedFilter.sortPolicy);
    if (appliedFilter.limit && appliedFilter.limit > 0) {
      return sorted.slice(0, appliedFilter.limit);
    }
    return sorted;
  });

export const pickNextTask = (
  tasksPath: string,
  filter?: TaskFilter,
): Effect.Effect<Task | null, TaskServiceError, FileSystem.FileSystem> =>
  readyTasks(tasksPath, filter).pipe(Effect.map((tasks) => tasks[0] ?? null));
