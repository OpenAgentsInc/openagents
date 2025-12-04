export {
  // Schemas
  Status,
  IssueType,
  DependencyType,
  Dependency,
  TaskSource,
  Comment,
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
  searchAllTasks,
  getTaskStats,
  getStaleTasks,
  getTaskWithDeps,
  hasConflictMarkers,
  type CreateTaskOptions,
  type UpdateTaskOptions,
  type CloseTaskOptions,
  type ReopenTaskOptions,
  type ArchiveOptions,
  type ArchiveResult,
  type SearchAllTasksOptions,
  type TaskStats,
  type StaleTasksOptions,
  type TaskWithDeps,
} from "./service.js";

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
