import * as S from "effect/Schema";

// Status represents the current state of a task
export const Status = S.Literal("open", "in_progress", "blocked", "closed");
export type Status = S.Schema.Type<typeof Status>;

// IssueType categorizes the kind of work
export const IssueType = S.Literal("bug", "feature", "task", "epic", "chore");
export type IssueType = S.Schema.Type<typeof IssueType>;

// DependencyType categorizes relationships between tasks
export const DependencyType = S.Literal("blocks", "related", "parent-child", "discovered-from");
export type DependencyType = S.Schema.Type<typeof DependencyType>;

// Dependency represents a relationship to another task
export const Dependency = S.Struct({
  id: S.String,
  type: DependencyType,
});
export type Dependency = S.Schema.Type<typeof Dependency>;

// Source tracks where the task came from
export const TaskSource = S.Struct({
  repo: S.optional(S.String),
  discoveredFrom: S.optional(S.String),
  externalRef: S.optional(S.NullOr(S.String)),
});
export type TaskSource = S.Schema.Type<typeof TaskSource>;

// Task represents a trackable work item (matches beads Issue schema)
export const Task = S.Struct({
  id: S.String,
  title: S.String.pipe(S.minLength(1), S.maxLength(500)),
  description: S.optionalWith(S.String, { default: () => "" }),
  status: Status,
  priority: S.Number.pipe(S.int(), S.greaterThanOrEqualTo(0), S.lessThanOrEqualTo(4)),
  type: IssueType,
  assignee: S.optional(S.String),
  labels: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  deps: S.optionalWith(S.Array(Dependency), { default: () => [] as Dependency[] }),
  commits: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  createdAt: S.String, // ISO 8601 timestamp
  updatedAt: S.String, // ISO 8601 timestamp
  closedAt: S.optional(S.NullOr(S.String)),
  closeReason: S.optional(S.String),
  source: S.optional(TaskSource),
  // Extended fields from beads (optional for compatibility)
  design: S.optional(S.String),
  acceptanceCriteria: S.optional(S.String),
  notes: S.optional(S.String),
  estimatedMinutes: S.optional(S.NullOr(S.Number)),
});
export type Task = S.Schema.Type<typeof Task>;

// TaskCreate is for creating new tasks (id and timestamps are generated)
export const TaskCreate = S.Struct({
  title: S.String.pipe(S.minLength(1), S.maxLength(500)),
  description: S.optionalWith(S.String, { default: () => "" }),
  status: S.optionalWith(Status, { default: () => "open" as const }),
  priority: S.optionalWith(S.Number.pipe(S.int(), S.greaterThanOrEqualTo(0), S.lessThanOrEqualTo(4)), {
    default: () => 2,
  }),
  type: S.optionalWith(IssueType, { default: () => "task" as const }),
  assignee: S.optional(S.String),
  labels: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  deps: S.optionalWith(S.Array(Dependency), { default: () => [] as Dependency[] }),
  source: S.optional(TaskSource),
  design: S.optional(S.String),
  acceptanceCriteria: S.optional(S.String),
  notes: S.optional(S.String),
  estimatedMinutes: S.optional(S.NullOr(S.Number)),
});
export type TaskCreate = S.Schema.Type<typeof TaskCreate>;

// TaskUpdate is for updating existing tasks (partial updates)
export const TaskUpdate = S.Struct({
  title: S.optional(S.String.pipe(S.minLength(1), S.maxLength(500))),
  description: S.optional(S.String),
  status: S.optional(Status),
  priority: S.optional(S.Number.pipe(S.int(), S.greaterThanOrEqualTo(0), S.lessThanOrEqualTo(4))),
  type: S.optional(IssueType),
  assignee: S.optional(S.NullOr(S.String)),
  labels: S.optional(S.Array(S.String)),
  deps: S.optional(S.Array(Dependency)),
  commits: S.optional(S.Array(S.String)),
  closeReason: S.optional(S.String),
  design: S.optional(S.String),
  acceptanceCriteria: S.optional(S.String),
  notes: S.optional(S.String),
  estimatedMinutes: S.optional(S.NullOr(S.Number)),
});
export type TaskUpdate = S.Schema.Type<typeof TaskUpdate>;

// ProjectConfig matches .openagents/project.json
export const ProjectConfig = S.Struct({
  version: S.optionalWith(S.Number, { default: () => 1 }),
  projectId: S.String,
  defaultBranch: S.optionalWith(S.String, { default: () => "main" }),
  defaultModel: S.optionalWith(S.String, { default: () => "x-ai/grok-4.1-fast" }),
  rootDir: S.optionalWith(S.String, { default: () => "." }),
  testCommands: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  e2eCommands: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  allowPush: S.optionalWith(S.Boolean, { default: () => true }),
  allowForcePush: S.optionalWith(S.Boolean, { default: () => false }),
  maxTasksPerRun: S.optionalWith(S.Number, { default: () => 3 }),
  maxRuntimeMinutes: S.optionalWith(S.Number, { default: () => 240 }),
  idPrefix: S.optionalWith(S.String, { default: () => "oa" }),
  cloud: S.optional(
    S.Struct({
      useGateway: S.optionalWith(S.Boolean, { default: () => false }),
      sendTelemetry: S.optionalWith(S.Boolean, { default: () => false }),
      relayUrl: S.optional(S.NullOr(S.String)),
    }),
  ),
});
export type ProjectConfig = S.Schema.Type<typeof ProjectConfig>;

// TaskFilter for querying tasks
export const TaskFilter = S.Struct({
  status: S.optional(Status),
  priority: S.optional(S.Number),
  type: S.optional(IssueType),
  assignee: S.optional(S.String),
  labels: S.optional(S.Array(S.String)),
  limit: S.optional(S.Number),
});
export type TaskFilter = S.Schema.Type<typeof TaskFilter>;

// Helper to check if a task is ready (open, no blocking deps)
export const isTaskReady = (task: Task, allTasks: Task[]): boolean => {
  if (task.status !== "open") return false;
  if (task.type === "epic") return false;

  const blockingDeps = task.deps?.filter((d) => d.type === "blocks" || d.type === "parent-child");
  if (!blockingDeps || blockingDeps.length === 0) return true;

  return blockingDeps.every((dep) => {
    const depTask = allTasks.find((t) => t.id === dep.id);
    return depTask?.status === "closed";
  });
};

// Helper to decode a task from unknown input
export const decodeTask = S.decodeUnknownSync(Task);
export const decodeTaskCreate = S.decodeUnknownSync(TaskCreate);
export const decodeTaskUpdate = S.decodeUnknownSync(TaskUpdate);
export const decodeProjectConfig = S.decodeUnknownSync(ProjectConfig);
