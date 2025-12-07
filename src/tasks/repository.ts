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
} from "./service.js";
import type { TaskCreate, TaskFilter, TaskUpdate } from "./schema.js";

type CreateTaskOptions = Parameters<typeof createTask>[0];
type UpdateTaskOptions = Parameters<typeof updateTask>[0];
type CloseTaskOptions = Parameters<typeof closeTask>[0];
type ReopenTaskOptions = Parameters<typeof reopenTask>[0];
type AddCommentOptions = Parameters<typeof addComment>[0];

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
    task: TaskCreate,
    options?: Omit<CreateTaskOptions, "task" | "tasksPath">,
  ) => ReturnType<typeof createTask>;
  update: (
    update: { id: string } & TaskUpdate,
    options?: Omit<UpdateTaskOptions, "tasksPath" | "id" | "update">,
  ) => ReturnType<typeof updateTask>;
  close: (options: Omit<CloseTaskOptions, "tasksPath">) => ReturnType<typeof closeTask>;
  reopen: (
    id: string,
    options?: Omit<ReopenTaskOptions, "tasksPath" | "id">,
  ) => ReturnType<typeof reopenTask>;
  addComment: (options: Omit<AddCommentOptions, "tasksPath">) => ReturnType<typeof addComment>;
  listComments: (taskId: string) => ReturnType<typeof listComments>;
}

export const resolveTaskRepositoryPaths = (
  options: TaskRepositoryOptions | string = {},
): TaskRepositoryPaths => {
  if (typeof options === "string") {
    const tasksPath = nodePath.resolve(options);
    const openagentsDir = nodePath.basename(nodePath.dirname(tasksPath));
    const rootDir = nodePath.resolve(nodePath.dirname(tasksPath), "..");
    const projectPath = nodePath.join(nodePath.dirname(tasksPath), "project.json");
    return { rootDir, openagentsDir, tasksPath, projectPath };
  }

  const rootDir = options.rootDir ? nodePath.resolve(options.rootDir) : process.cwd();
  const openagentsDir = options.openagentsDir ?? ".openagents";
  const tasksFilename = options.tasksFilename ?? "tasks.jsonl";
  const projectFilename = options.projectFilename ?? "project.json";

  const tasksPath = options.tasksPath ?? nodePath.resolve(rootDir, openagentsDir, tasksFilename);
  const projectPath = options.projectPath ?? nodePath.resolve(rootDir, openagentsDir, projectFilename);

  return { rootDir, openagentsDir, tasksPath, projectPath };
};

export const createTaskRepository = (
  options: TaskRepositoryOptions | string = {},
): TaskRepository => {
  const paths = resolveTaskRepositoryPaths(options);

  return {
    paths,
    list: (filter) => listTasks(paths.tasksPath, filter),
    ready: (filter) => readyTasks(paths.tasksPath, filter),
    pickNext: (filter) => pickNextTask(paths.tasksPath, filter),
    create: (task, opts) => createTask({ tasksPath: paths.tasksPath, task, ...opts }),
    update: ({ id, ...update }, opts) =>
      updateTask({ tasksPath: paths.tasksPath, id, update, ...opts }),
    close: (opts) => closeTask({ tasksPath: paths.tasksPath, ...opts }),
    reopen: (id, opts) => reopenTask({ tasksPath: paths.tasksPath, id, ...opts }),
    addComment: (opts) => addComment({ tasksPath: paths.tasksPath, ...opts }),
    listComments: (taskId) => listComments({ tasksPath: paths.tasksPath, taskId }),
  };
};
