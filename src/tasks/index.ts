export {
  // Schemas
  Status,
  IssueType,
  DependencyType,
  Dependency,
  TaskSource,
  Task,
  TaskCreate,
  TaskUpdate,
  ProjectConfig,
  TaskFilter,
  // Types
  type Status as StatusType,
  type IssueType as IssueTypeType,
  type DependencyType as DependencyTypeType,
  type Dependency as DependencyT,
  type TaskSource as TaskSourceT,
  type Task as TaskT,
  type TaskCreate as TaskCreateT,
  type TaskUpdate as TaskUpdateT,
  type ProjectConfig as ProjectConfigT,
  type TaskFilter as TaskFilterT,
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
  listTasks,
  readyTasks,
  pickNextTask,
  archiveTasks,
  readArchivedTasks,
  searchAllTasks,
  type CreateTaskOptions,
  type UpdateTaskOptions,
  type CloseTaskOptions,
  type ArchiveOptions,
  type ArchiveResult,
  type SearchAllTasksOptions,
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
