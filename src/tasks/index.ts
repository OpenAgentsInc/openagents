import type { Effect } from "effect";
import {
  addComment,
  archiveTasks,
  closeTask,
  compactTasks,
  createTask,
  getStaleTasks,
  getTaskStats,
  getTaskWithDeps,
  mergeTasksById,
  recordDeletion,
  renameTaskPrefix,
  reopenTask,
  searchAllTasks,
  updateTask,
} from "./service.js";

export {
  // Schemas
  Status,
  IssueType,
  DependencyType,
  Dependency,
  TaskSource,
  Comment,
  DeletionEntry,
  Task,
  TaskCreate,
  TaskUpdate,
  ProjectConfig,
  TaskFilter,
  SandboxConfig,
  MergeStrategy,
  ParallelExecutionConfig,
  ReflexionConfig,
  // Types
  type Status as StatusType,
  type IssueType as IssueTypeType,
  type DependencyType as DependencyTypeType,
  type Dependency as DependencyT,
  type TaskSource as TaskSourceT,
  type Comment as CommentT,
  type DeletionEntry as DeletionEntryT,
  type Task as TaskT,
  type TaskCreate as TaskCreateT,
  type TaskUpdate as TaskUpdateT,
  type ProjectConfig as ProjectConfigT,
  type TaskFilter as TaskFilterT,
  type SandboxConfig as SandboxConfigT,
  type MergeStrategy as MergeStrategyT,
  type ParallelExecutionConfig as ParallelExecutionConfigT,
  type ReflexionConfig as ReflexionConfigT,
  // Helpers
  isTaskReady,
  decodeTask,
  decodeTaskCreate,
  decodeTaskUpdate,
  decodeDeletionEntry,
  decodeProjectConfig,
} from "./schema.js";

export {
  // ID Generation
  generateHashId,
  generateShortId,
  generateRandomId,
  generateChildId,
  // ID Parsing
  parseHierarchicalId,
  isChildOf,
  getParentId,
  canHaveChildren,
  findNextChildNumber,
  MAX_HIERARCHY_DEPTH,
} from "./id.js";

export {
  TaskServiceError,
  readTasks,
  writeTasks,
  createTask,
  updateTask,
  closeTask,
  reopenTask,
  listTasks,
  readyTasks,
  pickNextTask,
  archiveTasks,
  readArchivedTasks,
  compactTasks,
  searchAllTasks,
  getTaskStats,
  getStaleTasks,
  getTaskWithDeps,
  hasConflictMarkers,
  addComment,
  listComments,
  renameTaskPrefix,
  mergeTasksById,
  readDeletions,
  writeDeletions,
  recordDeletion,
} from "./service.js";

export {
  createTaskRepository,
  resolveTaskRepositoryPaths,
  type TaskRepository,
  type TaskRepositoryOptions,
  type TaskRepositoryPaths,
} from "./repository.js";

export {
  ProjectServiceError,
  projectConfigPath,
  defaultProjectConfig,
  loadProjectConfig,
  saveProjectConfig,
} from "./project.js";

export {
  initOpenAgentsProject,
  InitProjectError,
  type InitProjectOptions,
} from "./init.js";

export {
  importBeadsIssues,
  BeadsImportError,
} from "./beads.js";

export {
  mergeTasks,
  mergeTaskFiles,
  ensureMergeDriverConfig,
  TaskMergeError,
  type MergeResult,
  type MergeFilesOptions,
} from "./merge.js";

export {
  installHooks,
  uninstallHooks,
  HooksError,
  type HooksConfig,
  type HooksResult,
} from "./hooks.js";

type ServiceOptions<T extends (...args: any) => any> = Parameters<T>[0];
type ServiceEffectResult<T extends (...args: any) => any> =
  ReturnType<T> extends Effect.Effect<infer A, any, any> ? A : never;

type CreateTaskOptions = ServiceOptions<typeof createTask>;
type UpdateTaskOptions = ServiceOptions<typeof updateTask>;
type CloseTaskOptions = ServiceOptions<typeof closeTask>;
type ReopenTaskOptions = ServiceOptions<typeof reopenTask>;
type ArchiveOptions = ServiceOptions<typeof archiveTasks>;
type ArchiveResult = ServiceEffectResult<typeof archiveTasks>;
type CompactOptions = ServiceOptions<typeof compactTasks>;
type CompactResult = ServiceEffectResult<typeof compactTasks>;
type SearchAllTasksOptions = ServiceOptions<typeof searchAllTasks>;
type TaskStats = ServiceEffectResult<typeof getTaskStats>;
type StaleTasksOptions = ServiceOptions<typeof getStaleTasks>;
type TaskWithDeps = ServiceEffectResult<typeof getTaskWithDeps>;
type AddCommentOptions = ServiceOptions<typeof addComment>;
type RenamePrefixOptions = ServiceOptions<typeof renameTaskPrefix>;
type RenamePrefixResult = ServiceEffectResult<typeof renameTaskPrefix>;
type MergeTasksOptions = ServiceOptions<typeof mergeTasksById>;
type MergeTasksResult = ServiceEffectResult<typeof mergeTasksById>;
type RecordDeletionOptions = ServiceOptions<typeof recordDeletion>;

export type {
  CreateTaskOptions,
  UpdateTaskOptions,
  CloseTaskOptions,
  ReopenTaskOptions,
  ArchiveOptions,
  ArchiveResult,
  CompactOptions,
  CompactResult,
  SearchAllTasksOptions,
  TaskStats,
  StaleTasksOptions,
  TaskWithDeps,
  AddCommentOptions,
  RenamePrefixOptions,
  RenamePrefixResult,
  MergeTasksOptions,
  MergeTasksResult,
  RecordDeletionOptions,
};
