import * as nodePath from "node:path";
import { Effect } from "effect";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import {
  addComment,
  closeTask,
  createTask,
  listComments,
  listTasks,
  pickNextTask,
  readyTasks,
  reopenTask,
  updateTask,
  type AddCommentOptions,
  type CloseTaskOptions,
  type CreateTaskOptions,
  type TaskServiceError,
  type UpdateTaskOptions,
} from "./service.js";
import type { Comment, Task, TaskFilter, TaskUpdate } from "./schema.js";

export interface TaskRepositoryConfig {
  tasksPath: string;
}

export interface TaskRepositoryFromRootOptions {
  rootDir?: string;
  openagentsDir?: string;
  tasksFile?: string;
  tasksPath?: string;
}

const DEFAULT_OPENAGENTS_DIR = ".openagents";
const DEFAULT_TASKS_FILE = "tasks.jsonl";

export class TaskRepository {
  readonly tasksPath: string;

  constructor(config: TaskRepositoryConfig) {
    this.tasksPath = config.tasksPath;
  }

  static fromRoot(options: TaskRepositoryFromRootOptions = {}): TaskRepository {
    if (options.tasksPath) return new TaskRepository({ tasksPath: options.tasksPath });

    const rootDir = options.rootDir ?? process.cwd();
    const openagentsDir = options.openagentsDir ?? DEFAULT_OPENAGENTS_DIR;
    const tasksFile = options.tasksFile ?? DEFAULT_TASKS_FILE;

    const openagentsPath = nodePath.isAbsolute(openagentsDir)
      ? openagentsDir
      : nodePath.join(rootDir, openagentsDir);

    return new TaskRepository({ tasksPath: nodePath.join(openagentsPath, tasksFile) });
  }

  listTasks(
    filter?: TaskFilter,
  ): Effect.Effect<Task[], TaskServiceError, FileSystem.FileSystem | Path.Path> {
    return listTasks(this.tasksPath, filter);
  }

  readyTasks(
    filter?: TaskFilter,
  ): Effect.Effect<Task[], TaskServiceError, FileSystem.FileSystem> {
    return readyTasks(this.tasksPath, filter);
  }

  pickNextTask(
    filter?: TaskFilter,
  ): Effect.Effect<Task | null, TaskServiceError, FileSystem.FileSystem> {
    return pickNextTask(this.tasksPath, filter);
  }

  claimNext(
    filter?: TaskFilter,
    options: { status?: Task["status"]; timestamp?: Date } = {},
  ): Effect.Effect<Task | null, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    const status = options.status ?? "in_progress";

    return this.pickNextTask(filter).pipe(
      Effect.flatMap((task) => {
        if (!task) return Effect.succeed<Task | null>(null);

        return this.updateTask({
          id: task.id,
          update: { status },
          ...(options.timestamp ? { timestamp: options.timestamp } : {}),
        }).pipe(Effect.catchAll(() => Effect.succeed(task)));
      }),
    );
  }

  createTask(
    options: Omit<CreateTaskOptions, "tasksPath">,
  ): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    return createTask({ ...options, tasksPath: this.tasksPath });
  }

  updateTask(
    options: Omit<UpdateTaskOptions, "tasksPath">,
  ): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    return updateTask({ ...options, tasksPath: this.tasksPath });
  }

  update(
    id: string,
    update: TaskUpdate,
    options: { appendCommits?: string[]; timestamp?: Date } = {},
  ): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    return this.updateTask({
      id,
      update,
      appendCommits: options.appendCommits ?? [],
      ...(options.timestamp ? { timestamp: options.timestamp } : {}),
    });
  }

  closeTask(
    options: Omit<CloseTaskOptions, "tasksPath">,
  ): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    return closeTask({ ...options, tasksPath: this.tasksPath });
  }

  close(
    id: string,
    options: { reason?: string; commits?: string[]; timestamp?: Date } = {},
  ): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    const reasonPatch = options.reason !== undefined ? { reason: options.reason } : {};
    return this.closeTask({
      id,
      ...reasonPatch,
      commits: options.commits ?? [],
      ...(options.timestamp ? { timestamp: options.timestamp } : {}),
    });
  }

  reopenTask(
    id: string,
    timestamp?: Date,
  ): Effect.Effect<Task, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    const options = timestamp
      ? { tasksPath: this.tasksPath, id, timestamp }
      : { tasksPath: this.tasksPath, id };
    return reopenTask(options);
  }

  addComment(
    options: Omit<AddCommentOptions, "tasksPath">,
  ): Effect.Effect<{ task: Task; comment: Comment }, TaskServiceError, FileSystem.FileSystem | Path.Path> {
    return addComment({ ...options, tasksPath: this.tasksPath });
  }

  listComments(taskId: string): Effect.Effect<Comment[], TaskServiceError, FileSystem.FileSystem | Path.Path> {
    return listComments({ tasksPath: this.tasksPath, taskId });
  }

  // Backward-compatible helpers for earlier API surface
  ready(filter?: TaskFilter) {
    return this.readyTasks(filter);
  }

  pickNext(filter?: TaskFilter) {
    return this.pickNextTask(filter);
  }
}

export const createTaskRepository = (options: TaskRepositoryFromRootOptions = {}): TaskRepository =>
  TaskRepository.fromRoot(options);
