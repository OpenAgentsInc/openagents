/**
 * Archivist Schema
 *
 * Types for the Archivist subagent that reviews trajectories
 * and extracts reusable patterns into the skill/memory library.
 *
 * The Archivist runs periodically (or on-demand) to:
 * 1. Review completed task trajectories
 * 2. Identify successful patterns worth preserving
 * 3. Extract skills from repeated solutions
 * 4. Build semantic memories from lessons learned
 * 5. Prune low-value or outdated entries
 */

// --- Trajectory Types ---

/**
 * A recorded action in a trajectory.
 */
export interface TrajectoryAction {
  /** Action type (tool call, thinking, etc.) */
  type: "tool_call" | "thinking" | "output" | "error";
  /** Tool name if tool_call */
  tool?: string;
  /** Input/content */
  content: string;
  /** Result if any */
  result?: string;
  /** Success flag */
  success?: boolean;
  /** Duration in ms */
  durationMs?: number;
  /** Timestamp */
  timestamp: string;
}

/**
 * A complete trajectory of a task attempt.
 */
export interface Trajectory {
  /** Unique trajectory ID */
  id: string;
  /** Task that was attempted */
  taskId: string;
  /** Task description */
  taskDescription: string;
  /** All actions taken */
  actions: TrajectoryAction[];
  /** Overall outcome */
  outcome: "success" | "failure" | "partial" | "timeout";
  /** Error message if failed */
  errorMessage?: string;
  /** Skills that were used */
  skillsUsed: string[];
  /** Files that were modified */
  filesModified: string[];
  /** Total duration */
  totalDurationMs: number;
  /** Model used */
  model: string;
  /** Token usage */
  tokens: {
    input: number;
    output: number;
    total: number;
  };
  /** Timestamp */
  timestamp: string;
  /** Project ID */
  projectId?: string;
  /** Whether this trajectory has been archived (processed by Archivist) */
  archived: boolean;
}

// --- Pattern Types ---

/**
 * A pattern identified from trajectories.
 */
export interface ExtractedPattern {
  /** Pattern ID */
  id: string;
  /** Pattern type */
  type: "skill" | "convention" | "antipattern" | "optimization";
  /** Name for the pattern */
  name: string;
  /** Description of what this pattern does */
  description: string;
  /** The pattern/code/approach */
  content: string;
  /** When to use this pattern */
  triggerContext: string[];
  /** Success rate from trajectories */
  successRate: number;
  /** Number of trajectories this was seen in */
  occurrences: number;
  /** Source trajectory IDs */
  sourceTrajectoryIds: string[];
  /** Confidence in this pattern (0-1) */
  confidence: number;
  /** Category for organization */
  category: string;
  /** Tags */
  tags: string[];
  /** Timestamp */
  extractedAt: string;
}

// --- Lesson Types ---

/**
 * A lesson learned from task executions.
 * Lessons are higher-level insights extracted from patterns and trajectories.
 */
export interface ArchivistLesson {
  /** Unique lesson ID */
  id: string;
  /** Source of the lesson */
  source: "terminal-bench" | "mechacoder" | "manual";
  /** Related task ID (if from a specific task) */
  taskId?: string;
  /** Suite name (if from TB) */
  suite?: string;
  /** Model that generated this lesson */
  model: string;
  /** Human-readable summary of the lesson */
  summary: string;
  /** Patterns that lead to failure */
  failurePatterns?: string[];
  /** Patterns that lead to success */
  successPatterns?: string[];
  /** Skills mentioned or used in this lesson */
  skillsMentioned?: string[];
  /** Confidence score (0-1) */
  confidence: number;
  /** Tags for filtering */
  tags: string[];
  /** Creation timestamp */
  createdAt: string;
}

/**
 * Generate a unique lesson ID.
 */
export const generateLessonId = (): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `lesson-${timestamp}-${random}`;
};

/**
 * Create a lesson from task execution data.
 */
export const createLesson = (
  summary: string,
  data: {
    source: ArchivistLesson["source"];
    model: string;
    taskId?: string;
    suite?: string;
    failurePatterns?: string[];
    successPatterns?: string[];
    skillsMentioned?: string[];
    confidence?: number;
    tags?: string[];
  },
): ArchivistLesson => ({
  id: generateLessonId(),
  source: data.source,
  ...(data.taskId ? { taskId: data.taskId } : {}),
  ...(data.suite ? { suite: data.suite } : {}),
  model: data.model,
  summary,
  ...(data.failurePatterns ? { failurePatterns: data.failurePatterns } : {}),
  ...(data.successPatterns ? { successPatterns: data.successPatterns } : {}),
  ...(data.skillsMentioned ? { skillsMentioned: data.skillsMentioned } : {}),
  confidence: data.confidence ?? 0.5,
  tags: data.tags ?? [],
  createdAt: new Date().toISOString(),
});

// --- Archive Types ---

/**
 * Result of an archiving run.
 */
export interface ArchiveResult {
  /** Archive run ID */
  id: string;
  /** Trajectories processed */
  trajectoriesProcessed: number;
  /** Patterns extracted */
  patternsExtracted: number;
  /** Skills created */
  skillsCreated: number;
  /** Memories created */
  memoriesCreated: number;
  /** Items pruned (low quality) */
  itemsPruned: number;
  /** Duration of archive run */
  durationMs: number;
  /** Timestamp */
  timestamp: string;
}

/**
 * Configuration for archive runs.
 */
export interface ArchiveConfig {
  /** Minimum success rate for skill extraction */
  minSuccessRate: number;
  /** Minimum occurrences before extracting pattern */
  minOccurrences: number;
  /** Maximum age in days for trajectories to process */
  maxTrajectoryAgeDays: number;
  /** Whether to auto-prune low-performing skills */
  autoPrune: boolean;
  /** Minimum success rate before pruning */
  pruneThreshold: number;
  /** Project root for file operations */
  projectRoot: string;
}

export const DEFAULT_ARCHIVE_CONFIG: ArchiveConfig = {
  minSuccessRate: 0.7,
  minOccurrences: 2,
  maxTrajectoryAgeDays: 30,
  autoPrune: true,
  pruneThreshold: 0.3,
  projectRoot: process.cwd(),
};

// --- Helper Functions ---

/**
 * Generate a unique trajectory ID.
 */
export const generateTrajectoryId = (): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `traj-${timestamp}-${random}`;
};

/**
 * Generate a unique pattern ID.
 */
export const generatePatternId = (type: string): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `pat-${type.slice(0, 3)}-${timestamp}-${random}`;
};

/**
 * Generate a unique archive ID.
 */
export const generateArchiveId = (): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 6);
  return `arch-${timestamp}-${random}`;
};

/**
 * Create a trajectory from task execution data.
 */
export const createTrajectory = (
  taskId: string,
  taskDescription: string,
  data: {
    actions: TrajectoryAction[];
    outcome: Trajectory["outcome"];
    errorMessage?: string;
    skillsUsed?: string[];
    filesModified?: string[];
    totalDurationMs: number;
    model: string;
    tokens: Trajectory["tokens"];
    projectId?: string;
  },
): Trajectory => ({
  id: generateTrajectoryId(),
  taskId,
  taskDescription,
  actions: data.actions,
  outcome: data.outcome,
  errorMessage: data.errorMessage,
  skillsUsed: data.skillsUsed ?? [],
  filesModified: data.filesModified ?? [],
  totalDurationMs: data.totalDurationMs,
  model: data.model,
  tokens: data.tokens,
  timestamp: new Date().toISOString(),
  projectId: data.projectId,
  archived: false,
});

/**
 * Build a prompt for extracting patterns from trajectories.
 */
export const buildPatternExtractionPrompt = (trajectories: Trajectory[]): string => {
  const successfulTrajs = trajectories.filter((t) => t.outcome === "success");
  const failedTrajs = trajectories.filter((t) => t.outcome === "failure");

  const parts = [
    "You are an Archivist analyzing task trajectories to extract reusable patterns.",
    "",
    "## Successful Trajectories",
    "",
  ];

  for (const traj of successfulTrajs.slice(0, 5)) {
    parts.push(`### Task: ${traj.taskDescription}`);
    parts.push(`Duration: ${traj.totalDurationMs}ms, Skills: ${traj.skillsUsed.join(", ") || "none"}`);
    parts.push("Actions:");
    for (const action of traj.actions.slice(0, 10)) {
      if (action.type === "tool_call") {
        parts.push(`  - ${action.tool}: ${action.content.slice(0, 100)}...`);
      }
    }
    parts.push("");
  }

  if (failedTrajs.length > 0) {
    parts.push("## Failed Trajectories (antipatterns to avoid)");
    parts.push("");
    for (const traj of failedTrajs.slice(0, 3)) {
      parts.push(`### Task: ${traj.taskDescription}`);
      parts.push(`Error: ${traj.errorMessage?.slice(0, 200) ?? "unknown"}`);
      parts.push("");
    }
  }

  parts.push(
    "## Extract Patterns",
    "",
    "Identify reusable patterns from these trajectories. For each pattern, provide:",
    "1. **name**: Short descriptive name",
    "2. **type**: skill | convention | antipattern | optimization",
    "3. **description**: What the pattern does",
    "4. **content**: The code/approach to use",
    "5. **triggerContext**: When to use this pattern",
    "6. **category**: Category for organization",
    "",
    "Output as a JSON array of patterns.",
  );

  return parts.join("\n");
};

/**
 * Parse patterns from FM response.
 */
export const parsePatternsFromResponse = (
  response: string,
  sourceTrajectoryIds: string[],
): ExtractedPattern[] => {
  try {
    // Extract JSON array from response
    const jsonMatch = response.match(/\[[\s\S]*\]/);
    if (!jsonMatch) {
      return [];
    }

    const parsed = JSON.parse(jsonMatch[0]);
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed.map((p: any) => ({
      id: generatePatternId(p.type ?? "skill"),
      type: p.type ?? "skill",
      name: p.name ?? "Unnamed Pattern",
      description: p.description ?? "",
      content: p.content ?? "",
      triggerContext: p.triggerContext ?? [],
      successRate: 1.0, // Will be updated from trajectory stats
      occurrences: sourceTrajectoryIds.length,
      sourceTrajectoryIds,
      confidence: 0.7,
      category: p.category ?? "general",
      tags: p.tags ?? [],
      extractedAt: new Date().toISOString(),
    }));
  } catch {
    return [];
  }
};

/**
 * Calculate success rate from trajectories.
 */
export const calculateSuccessRate = (trajectories: Trajectory[]): number => {
  if (trajectories.length === 0) return 0;
  const successful = trajectories.filter((t) => t.outcome === "success").length;
  return successful / trajectories.length;
};

/**
 * Group trajectories by similarity (for pattern detection).
 */
export const groupSimilarTrajectories = (
  trajectories: Trajectory[],
): Map<string, Trajectory[]> => {
  const groups = new Map<string, Trajectory[]>();

  for (const traj of trajectories) {
    // Simple grouping by first tool used + outcome
    const firstTool = traj.actions.find((a) => a.type === "tool_call")?.tool ?? "unknown";
    const key = `${firstTool}-${traj.outcome}`;

    const existing = groups.get(key) ?? [];
    existing.push(traj);
    groups.set(key, existing);
  }

  return groups;
};
