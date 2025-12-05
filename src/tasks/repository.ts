import * as nodePath from "node:path";
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
  type ReopenTaskOptions,
  type UpdateTaskOptions,
} from "./service.js";
import type { TaskFilter } from "./schema.js";

export interface TaskRepositoryPaths {
  /** Absolute path to the repo root (or provided rootDir) */
  rootDir: string;
  /** Relative .openagents directory (default: .openagents) */
  openagentsDir: string;
  /** Absolute path to tasks.jsonl */
  tasksPath: string;
  /** Absolute path to project.json */
  projectPath: string;
}

export interface TaskRepositoryOptions {
  rootDir?: string;
  openagentsDir?: string;
  tasksFilename?: string;
  projectFilename?: string;
  tasksPath?: string;
  projectPath?: string;
}

export interface TaskRepository {
  paths: TaskRepositoryPaths;
  list: (filter?: TaskFilter) => ReturnType<typeof listTasks>;
  ready: (filter?: TaskFilter) => ReturnType<typeof readyTasks>;
  pickNext: (filter?: TaskFilter) => ReturnType<typeof pickNextTask>;
  create: (
    options: Omit<CreateTaskOptions, "tasksPath">
  ) => ReturnType<typeof createTask>;
  update: (
    options: Omit<UpdateTaskOptions, "tasksPath">
  ) => ReturnType<typeof updateTask>;
  close: (
    options: Omit<CloseTaskOptions, "tasksPath">
  ) => ReturnType<typeof closeTask>;
  reopen: (
    options: Omit<ReopenTaskOptions, "tasksPath">
  ) => ReturnType<typeof reopenTask>;
  addComment: (
    options: Omit<AddCommentOptions, "tasksPath">
  ) => ReturnType<typeof addComment>;
  listComments: (taskId: string) => ReturnType<typeof listComments>;
}

export const resolveTaskRepositoryPaths = (
  options: TaskRepositoryOptions = {},
): TaskRepositoryPaths => {
  const rootDir = options.rootDir ? nodePath.resolve(options.rootDir) : process.cwd();
  const openagentsDir = options.openagentsDir ?? ".openagents";
  const tasksFilename = options.tasksFilename ?? "tasks.jsonl";
  const projectFilename = options.projectFilename ?? "project.json";

  const tasksPath =
    options.tasksPath ??
    nodePath.resolve(rootDir, openagentsDir, tasksFilename);
  const projectPath =
    options.projectPath ??
    nodePath.resolve(rootDir, openagentsDir, projectFilename);

  return {
    rootDir,
    openagentsDir,
    tasksPath,
    projectPath,
  };
};

export const createTaskRepository = (
  options: TaskRepositoryOptions = {},
): TaskRepository => {
  const paths = resolveTaskRepositoryPaths(options);

  return {
    paths,
    list: (filter) => listTasks(paths.tasksPath, filter),
    ready: (filter) => readyTasks(paths.tasksPath, filter),
    pickNext: (filter) => pickNextTask(paths.tasksPath, filter),
    create: (opts) => createTask({ ...opts, tasksPath: paths.tasksPath }),
    update: (opts) => updateTask({ ...opts, tasksPath: paths.tasksPath }),
    close: (opts) => closeTask({ ...opts, tasksPath: paths.tasksPath }),
    reopen: (opts) => reopenTask({ ...opts, tasksPath: paths.tasksPath }),
    addComment: (opts) => addComment({ ...opts, tasksPath: paths.tasksPath }),
    listComments: (taskId) => listComments({ tasksPath: paths.tasksPath, taskId }),
  };
};
