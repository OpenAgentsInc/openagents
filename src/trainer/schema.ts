/**
 * Trainer Schema
 *
 * Types for the Trainer/Gym system.
 * Manages training runs on Terminal-Bench or custom task sets.
 *
 * The Trainer provides a controlled environment for:
 * 1. Running benchmark tasks with skill/memory integration
 * 2. Recording trajectories for the Archivist
 * 3. Progressive difficulty scaling
 * 4. Performance tracking and analysis
 */

// --- Task Types ---

/**
 * A training task to execute.
 */
export interface TrainingTask {
  /** Unique task ID */
  id: string;
  /** Task description/prompt */
  prompt: string;
  /** Expected behavior or success criteria */
  expectedBehavior?: string;
  /** Difficulty level (1-5) */
  difficulty: number;
  /** Category for grouping */
  category: string;
  /** Tags for filtering */
  tags: string[];
  /** Maximum duration in ms */
  timeoutMs: number;
  /** Source (e.g., "terminal-bench", "custom") */
  source: string;
  /** Files needed for the task (if any) */
  setupFiles?: Record<string, string>;
}

/**
 * Result of executing a training task.
 */
export interface TaskResult {
  /** Task that was executed */
  taskId: string;
  /** Outcome of the task */
  outcome: "success" | "failure" | "partial" | "timeout";
  /** Score (0-1) if applicable */
  score?: number;
  /** Error message if failed */
  errorMessage?: string;
  /** Output produced */
  output?: string;
  /** Duration in ms */
  durationMs: number;
  /** Model used */
  model: string;
  /** Token usage */
  tokens: {
    input: number;
    output: number;
    total: number;
  };
  /** Skills that were used */
  skillsUsed: string[];
  /** Whether reflexion was applied */
  usedReflexion: boolean;
  /** Attempt number (for retries) */
  attemptNumber: number;
  /** Timestamp */
  timestamp: string;
}

// --- Training Run Types ---

/**
 * Configuration for a training run.
 */
export interface TrainingConfig {
  /** Maximum tasks to run */
  maxTasks: number;
  /** Maximum retries per task */
  maxRetries: number;
  /** Whether to use skills */
  useSkills: boolean;
  /** Whether to use memory */
  useMemory: boolean;
  /** Whether to use reflexion on failures */
  useReflexion: boolean;
  /** Whether to record trajectories */
  recordTrajectories: boolean;
  /** Difficulty filter (1-5, null for all) */
  difficultyFilter?: number;
  /** Category filter (null for all) */
  categoryFilter?: string;
  /** Timeout per task in ms */
  taskTimeoutMs: number;
  /** Project root for file operations */
  projectRoot: string;
  /** Model to use */
  model: "foundation-models" | "openrouter";
  /** Optional HUD message callback for real-time UI updates */
  onHudMessage?: (msg: TrainerHudMessage) => void;
}

export const DEFAULT_TRAINING_CONFIG: TrainingConfig = {
  maxTasks: 10,
  maxRetries: 2,
  useSkills: true,
  useMemory: true,
  useReflexion: true,
  recordTrajectories: true,
  taskTimeoutMs: 120000, // 2 minutes
  projectRoot: process.cwd(),
  model: "foundation-models",
};

/**
 * A complete training run.
 */
export interface TrainingRun {
  /** Unique run ID */
  id: string;
  /** Configuration used */
  config: TrainingConfig;
  /** Tasks executed */
  tasks: TrainingTask[];
  /** Results for each task */
  results: TaskResult[];
  /** Overall statistics */
  stats: TrainingStats;
  /** Start timestamp */
  startedAt: string;
  /** End timestamp */
  completedAt?: string;
  /** Status */
  status: "running" | "completed" | "cancelled" | "failed";
}

/**
 * Statistics for a training run.
 */
export interface TrainingStats {
  /** Total tasks */
  totalTasks: number;
  /** Completed tasks */
  completedTasks: number;
  /** Successful tasks */
  successfulTasks: number;
  /** Failed tasks */
  failedTasks: number;
  /** Partial success tasks */
  partialTasks: number;
  /** Timed out tasks */
  timedOutTasks: number;
  /** Success rate */
  successRate: number;
  /** Average score */
  averageScore: number;
  /** Total duration */
  totalDurationMs: number;
  /** Average duration per task */
  averageDurationMs: number;
  /** Total tokens used */
  totalTokens: number;
  /** Skills used count */
  skillsUsedCount: number;
  /** Reflexion applied count */
  reflexionAppliedCount: number;
}

// --- Benchmark Types ---

/**
 * A benchmark suite (e.g., Terminal-Bench subset).
 */
export interface BenchmarkSuite {
  /** Unique suite ID */
  id: string;
  /** Suite name */
  name: string;
  /** Description */
  description: string;
  /** Tasks in the suite */
  tasks: TrainingTask[];
  /** Version */
  version: string;
  /** Source (e.g., "terminal-bench-10") */
  source: string;
}

/**
 * Benchmark result (historical record).
 */
export interface BenchmarkResult {
  /** Unique result ID */
  id: string;
  /** Suite that was run */
  suiteId: string;
  /** Training run ID */
  runId: string;
  /** Final stats */
  stats: TrainingStats;
  /** Model used */
  model: string;
  /** Timestamp */
  timestamp: string;
}

// --- Helper Functions ---

/**
 * Generate a unique training run ID.
 */
export const generateRunId = (): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 6);
  return `run-${timestamp}-${random}`;
};

/**
 * Generate a unique task ID.
 */
export const generateTaskId = (source: string): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 6);
  return `task-${source.slice(0, 4)}-${timestamp}-${random}`;
};

/**
 * Calculate stats from task results.
 */
export const calculateStats = (results: TaskResult[]): TrainingStats => {
  const totalTasks = results.length;
  if (totalTasks === 0) {
    return {
      totalTasks: 0,
      completedTasks: 0,
      successfulTasks: 0,
      failedTasks: 0,
      partialTasks: 0,
      timedOutTasks: 0,
      successRate: 0,
      averageScore: 0,
      totalDurationMs: 0,
      averageDurationMs: 0,
      totalTokens: 0,
      skillsUsedCount: 0,
      reflexionAppliedCount: 0,
    };
  }

  const successfulTasks = results.filter((r) => r.outcome === "success").length;
  const failedTasks = results.filter((r) => r.outcome === "failure").length;
  const partialTasks = results.filter((r) => r.outcome === "partial").length;
  const timedOutTasks = results.filter((r) => r.outcome === "timeout").length;

  const totalDurationMs = results.reduce((sum, r) => sum + r.durationMs, 0);
  const totalTokens = results.reduce((sum, r) => sum + r.tokens.total, 0);

  const scoresWithValues = results.filter((r) => r.score !== undefined);
  const averageScore =
    scoresWithValues.length > 0
      ? scoresWithValues.reduce((sum, r) => sum + (r.score ?? 0), 0) / scoresWithValues.length
      : 0;

  const skillsUsed = new Set<string>();
  for (const r of results) {
    for (const s of r.skillsUsed) {
      skillsUsed.add(s);
    }
  }

  return {
    totalTasks,
    completedTasks: totalTasks,
    successfulTasks,
    failedTasks,
    partialTasks,
    timedOutTasks,
    successRate: successfulTasks / totalTasks,
    averageScore,
    totalDurationMs,
    averageDurationMs: totalDurationMs / totalTasks,
    totalTokens,
    skillsUsedCount: skillsUsed.size,
    reflexionAppliedCount: results.filter((r) => r.usedReflexion).length,
  };
};

/**
 * Create a training task from a prompt.
 */
export const createTask = (
  prompt: string,
  options?: {
    id?: string;
    expectedBehavior?: string;
    difficulty?: number;
    category?: string;
    tags?: string[];
    timeoutMs?: number;
    source?: string;
    setupFiles?: Record<string, string>;
  },
): TrainingTask => {
  const task: TrainingTask = {
    id: options?.id ?? generateTaskId(options?.source ?? "custom"),
    prompt,
    difficulty: options?.difficulty ?? 3,
    category: options?.category ?? "general",
    tags: options?.tags ?? [],
    timeoutMs: options?.timeoutMs ?? 120000,
    source: options?.source ?? "custom",
  };
  if (options?.expectedBehavior !== undefined) {
    task.expectedBehavior = options.expectedBehavior;
  }
  if (options?.setupFiles !== undefined) {
    task.setupFiles = options.setupFiles;
  }
  return task;
};

/**
 * Create a task result.
 */
export const createTaskResult = (
  taskId: string,
  data: {
    outcome: TaskResult["outcome"];
    score?: number;
    errorMessage?: string;
    output?: string;
    durationMs: number;
    model: string;
    tokens: TaskResult["tokens"];
    skillsUsed?: string[];
    usedReflexion?: boolean;
    attemptNumber?: number;
  },
): TaskResult => {
  const result: TaskResult = {
    taskId,
    outcome: data.outcome,
    durationMs: data.durationMs,
    model: data.model,
    tokens: data.tokens,
    skillsUsed: data.skillsUsed ?? [],
    usedReflexion: data.usedReflexion ?? false,
    attemptNumber: data.attemptNumber ?? 1,
    timestamp: new Date().toISOString(),
  };
  if (data.score !== undefined) {
    result.score = data.score;
  }
  if (data.errorMessage !== undefined) {
    result.errorMessage = data.errorMessage;
  }
  if (data.output !== undefined) {
    result.output = data.output;
  }
  return result;
};

/**
 * Create an empty training run.
 */
export const createTrainingRun = (config: TrainingConfig): TrainingRun => ({
  id: generateRunId(),
  config,
  tasks: [],
  results: [],
  stats: calculateStats([]),
  startedAt: new Date().toISOString(),
  status: "running",
});

// --- Terminal-Bench Integration ---

/**
 * Terminal-Bench task subset definitions.
 */
export const TB_SUBSETS = {
  TB_10: {
    name: "Terminal-Bench 10",
    description: "First 10 tasks for quick validation",
    count: 10,
  },
  TB_30: {
    name: "Terminal-Bench 30",
    description: "Extended subset for thorough testing",
    count: 30,
  },
  TB_89: {
    name: "Terminal-Bench Full",
    description: "Complete 89-task benchmark",
    count: 89,
  },
} as const;

export type TBSubset = keyof typeof TB_SUBSETS;

// --- HUD Integration ---

/** Trainer HUD message types */
export type TrainerHudMessage =
  | { type: "trainer_run_start"; runId: string; totalTasks: number; config: { model: string; maxRetries: number; useSkills: boolean; useMemory: boolean; useReflection: boolean }; timestamp: string }
  | { type: "trainer_task_start"; runId: string; taskId: string; taskPrompt: string; taskIndex: number; totalTasks: number }
  | { type: "trainer_task_complete"; runId: string; taskId: string; outcome: "success" | "failure" | "timeout"; durationMs: number; turns: number; tokens: number; retriesUsed: number }
  | { type: "trainer_run_complete"; runId: string; stats: { totalTasks: number; successRate: number; averageDurationMs: number; totalTokens: number }; durationMs: number }
  | { type: "trainer_evolution_generation_start"; runId: string; generation: number; populationSize: number; topPerformers: string[] }
  | { type: "trainer_evolution_profile_evaluated"; runId: string; profileId: string; profileName: string; generation: number; fitness: number; successRate: number }
  | { type: "trainer_evolution_ab_result"; runId: string; profileA: string; profileB: string; winner: "A" | "B" | "tie"; effectSize: number; confidence: number }
  | { type: "trainer_evolution_complete"; runId: string; generations: number; bestProfileId: string; bestProfileName: string; bestFitness: number; fitnessImprovement: number };
