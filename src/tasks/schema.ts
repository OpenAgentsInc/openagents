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

const ClaudeCodeConfig = S.Struct({
  enabled: S.optionalWith(S.Boolean, { default: () => true }),
  preferForComplexTasks: S.optionalWith(S.Boolean, { default: () => true }),
  maxTurnsPerSubtask: S.optionalWith(S.Number, { default: () => 300 }),
  permissionMode: S.optionalWith(
    S.Literal("default", "acceptEdits", "bypassPermissions", "plan", "dontAsk"),
    { default: () => "bypassPermissions" },
  ),
  fallbackToMinimal: S.optionalWith(S.Boolean, { default: () => true }),
});
export type ClaudeCodeConfig = S.Schema.Type<typeof ClaudeCodeConfig>;

export const SandboxConfig = S.Struct({
  /** Enable sandboxed execution (default: true if available) */
  enabled: S.optionalWith(S.Boolean, { default: () => true }),
  /** Backend to use: auto-detect, macos-container, docker, seatbelt, or none */
  backend: S.optionalWith(
    S.Literal("auto", "macos-container", "docker", "seatbelt", "none"),
    { default: () => "auto" as const },
  ),
  /** Container image to use (only for container backends) */
  image: S.optional(S.String),
  /** Memory limit with suffix K/M/G (e.g., "4G") */
  memoryLimit: S.optional(S.String),
  /** Number of CPUs to allocate */
  cpuLimit: S.optional(S.Number),
  /** Timeout in milliseconds for sandboxed operations */
  timeoutMs: S.optionalWith(S.Number, { default: () => 300_000 }),
});
export type SandboxConfig = S.Schema.Type<typeof SandboxConfig>;

/** Merge strategy for parallel agent execution */
export const MergeStrategy = S.Literal("auto", "direct", "queue", "pr");
export type MergeStrategy = S.Schema.Type<typeof MergeStrategy>;

/** Configuration for parallel agent execution using git worktrees */
export const ParallelExecutionConfig = S.Struct({
  /** Enable parallel agent execution (default: false) */
  enabled: S.optionalWith(S.Boolean, { default: () => false }),
  /** Maximum number of agents to run in parallel */
  maxAgents: S.optionalWith(S.Number, { default: () => 4 }),
  /** Timeout for each worktree in milliseconds */
  worktreeTimeout: S.optionalWith(S.Number, { default: () => 30 * 60 * 1000 }),
  /** Timeout for dependency installation in milliseconds (default: 15 minutes) */
  installTimeoutMs: S.optionalWith(S.Number, { default: () => 15 * 60 * 1000 }),
  /** Extra args to pass to bun install (e.g., --frozen-lockfile) */
  installArgs: S.optionalWith(S.Array(S.String), {
    default: () => ["--frozen-lockfile"] as string[],
  }),
  /** Run agents in containers for additional isolation */
  useContainers: S.optionalWith(S.Boolean, { default: () => false }),
  /** Merge strategy: auto (select based on count), direct, queue, or pr */
  mergeStrategy: S.optionalWith(MergeStrategy, { default: () => "auto" as const }),
  /** Number of agents before switching from direct to queue (when auto) */
  mergeThreshold: S.optionalWith(S.Number, { default: () => 4 }),
  /** Number of agents before switching from queue to PR (when auto) */
  prThreshold: S.optionalWith(S.Number, { default: () => 50 }),
});
export type ParallelExecutionConfig = S.Schema.Type<typeof ParallelExecutionConfig>;

/** Configuration for ATIF trajectory capture and storage */
export const TrajectoryConfig = S.Struct({
  /** Enable trajectory capture (default: true) */
  enabled: S.optionalWith(S.Boolean, { default: () => true }),
  /** Number of days to retain trajectories (default: 30) */
  retentionDays: S.optionalWith(S.Number, { default: () => 30 }),
  /** Maximum storage size in GB before pruning old trajectories (default: 5) */
  maxSizeGB: S.optionalWith(S.Number, { default: () => 5 }),
  /** Include full tool arguments in trajectories (default: true) */
  includeToolArgs: S.optionalWith(S.Boolean, { default: () => true }),
  /** Include tool result content in trajectories (default: true) */
  includeToolResults: S.optionalWith(S.Boolean, { default: () => true }),
  /** Custom trajectories directory (relative to .openagents/) */
  directory: S.optionalWith(S.String, { default: () => "trajectories" }),
});
export type TrajectoryConfig = S.Schema.Type<typeof TrajectoryConfig>;

/** Healer mode determines aggressiveness of recovery attempts */
export const HealerMode = S.Literal("conservative", "aggressive");
export type HealerMode = S.Schema.Type<typeof HealerMode>;

/** Configuration for which scenarios trigger Healer */
export const HealerScenarioConfig = S.Struct({
  /** Trigger on init script failures (typecheck, test) */
  onInitFailure: S.optionalWith(S.Boolean, { default: () => true }),
  /** Trigger on verification failures after work */
  onVerificationFailure: S.optionalWith(S.Boolean, { default: () => true }),
  /** Trigger on subtask execution failures */
  onSubtaskFailure: S.optionalWith(S.Boolean, { default: () => true }),
  /** Trigger on runtime errors in orchestrator */
  onRuntimeError: S.optionalWith(S.Boolean, { default: () => true }),
  /** Trigger on stuck subtasks (experimental) */
  onStuckSubtask: S.optionalWith(S.Boolean, { default: () => false }),
});
export type HealerScenarioConfig = S.Schema.Type<typeof HealerScenarioConfig>;

/** Configuration for allowed/forbidden spells */
export const HealerSpellsConfig = S.Struct({
  /** Spells explicitly allowed (empty = all allowed) */
  allowed: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  /** Spells explicitly forbidden (takes precedence over allowed) */
  forbidden: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
});
export type HealerSpellsConfig = S.Schema.Type<typeof HealerSpellsConfig>;

/** Configuration for Healer self-healing subagent */
export const HealerConfig = S.Struct({
  /** Enable Healer (default: true, replaces safeMode) */
  enabled: S.optionalWith(S.Boolean, { default: () => true }),
  /** Maximum Healer invocations per orchestrator session (default: 2) */
  maxInvocationsPerSession: S.optionalWith(S.Number, { default: () => 2 }),
  /** Maximum Healer invocations per subtask (default: 1) */
  maxInvocationsPerSubtask: S.optionalWith(S.Number, { default: () => 1 }),
  /** Which scenarios trigger Healer */
  scenarios: S.optionalWith(HealerScenarioConfig, {
    default: () => S.decodeUnknownSync(HealerScenarioConfig)({}),
  }),
  /** Spell allow/forbid lists */
  spells: S.optionalWith(HealerSpellsConfig, {
    default: () => S.decodeUnknownSync(HealerSpellsConfig)({}),
  }),
  /** Healer mode: conservative (safe) or aggressive (more attempts) */
  mode: S.optionalWith(HealerMode, { default: () => "conservative" as const }),
  /** Hours before a stuck subtask triggers Healer (default: 2) */
  stuckThresholdHours: S.optionalWith(S.Number, { default: () => 2 }),
});
export type HealerConfig = S.Schema.Type<typeof HealerConfig>;

// ProjectConfig matches .openagents/project.json
export const ProjectConfig = S.Struct({
  version: S.optionalWith(S.Number, { default: () => 1 }),
  projectId: S.String,
  defaultBranch: S.optionalWith(S.String, { default: () => "main" }),
  defaultModel: S.optionalWith(S.String, { default: () => "x-ai/grok-4.1-fast:free" }),
  rootDir: S.optionalWith(S.String, { default: () => "." }),
  typecheckCommands: S.optionalWith(S.Array(S.String), {
    default: () => [] as string[],
  }),
  testCommands: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  e2eCommands: S.optionalWith(S.Array(S.String), { default: () => [] as string[] }),
  allowPush: S.optionalWith(S.Boolean, { default: () => true }),
  allowForcePush: S.optionalWith(S.Boolean, { default: () => false }),
  maxTasksPerRun: S.optionalWith(S.Number, { default: () => 3 }),
  maxRuntimeMinutes: S.optionalWith(S.Number, { default: () => 240 }),
  idPrefix: S.optionalWith(S.String, { default: () => "oa" }),
  // Session and run log directories (relative to .openagents/)
  sessionDir: S.optionalWith(S.String, { default: () => ".openagents/sessions" }),
  runLogDir: S.optionalWith(S.String, { default: () => ".openagents/run-logs" }),
  claudeCode: S.optionalWith(ClaudeCodeConfig, {
    default: () => S.decodeUnknownSync(ClaudeCodeConfig)({}),
  }),
  sandbox: S.optionalWith(SandboxConfig, {
    default: () => S.decodeUnknownSync(SandboxConfig)({}),
  }),
  parallelExecution: S.optionalWith(ParallelExecutionConfig, {
    default: () => S.decodeUnknownSync(ParallelExecutionConfig)({}),
  }),
  trajectory: S.optionalWith(TrajectoryConfig, {
    default: () => S.decodeUnknownSync(TrajectoryConfig)({}),
  }),
  healer: S.optionalWith(HealerConfig, {
    default: () => S.decodeUnknownSync(HealerConfig)({}),
  }),
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
  labelsAny: S.optional(S.Array(S.String)),
  unassigned: S.optional(S.Boolean),
  sortPolicy: S.optional(S.Literal("hybrid", "priority", "oldest")),
  limit: S.optional(S.Number),
});
export type TaskFilter = S.Schema.Type<typeof TaskFilter>;

// Helper to check if a task is ready (open, no blocking deps)
export const isTaskReady = (task: Task, allTasks: Task[]): boolean => {
  if (task.status === "closed" || task.status === "blocked") return false;

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
