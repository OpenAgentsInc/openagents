/**
 * Dashboard Schema
 *
 * Types for the learning system dashboard.
 * Provides visibility into training progress and system health.
 */

import type { LoopState, LearningStats } from "../learning/index.js";
import type { TBSubset } from "../trainer/schema.js";

// --- Display Types ---

/**
 * Dashboard display mode.
 */
export type DisplayMode = "compact" | "detailed" | "json";

/**
 * Dashboard update interval in ms.
 */
export const UPDATE_INTERVALS = {
  fast: 1000,
  normal: 5000,
  slow: 15000,
} as const;

export type UpdateInterval = keyof typeof UPDATE_INTERVALS;

// --- Dashboard State ---

/**
 * Current state of the dashboard display.
 */
export interface DashboardState {
  /** Display mode */
  mode: DisplayMode;
  /** Update interval */
  interval: UpdateInterval;
  /** Last update timestamp */
  lastUpdate: string;
  /** Whether live updates are enabled */
  liveUpdates: boolean;
  /** Current view/tab */
  currentView: DashboardView;
  /** Filter settings */
  filters: DashboardFilters;
}

/**
 * Available dashboard views.
 */
export type DashboardView =
  | "overview"
  | "skills"
  | "memory"
  | "training"
  | "archivist"
  | "reflexion"
  | "history";

/**
 * Dashboard filter settings.
 */
export interface DashboardFilters {
  /** Time range filter */
  timeRange: "1h" | "24h" | "7d" | "30d" | "all";
  /** Benchmark subset filter */
  subset?: TBSubset;
  /** Status filter */
  status?: "running" | "completed" | "failed";
  /** Category filter */
  category?: string;
}

// --- Metrics ---

/**
 * Real-time metrics for the dashboard.
 */
export interface DashboardMetrics {
  /** Learning system stats */
  stats: LearningStats;
  /** Loop state */
  loop: LoopState | null;
  /** Trends */
  trends: MetricTrends;
  /** Health indicators */
  health: SystemHealth;
  /** Timestamp */
  timestamp: string;
}

/**
 * Metric trends over time.
 */
export interface MetricTrends {
  /** Success rate trend (last N runs) */
  successRateTrend: number[];
  /** Skills learned trend */
  skillsLearnedTrend: number[];
  /** Duration trend */
  durationTrend: number[];
  /** Token usage trend */
  tokenUsageTrend: number[];
}

/**
 * System health indicators.
 */
export interface SystemHealth {
  /** Overall health status */
  status: "healthy" | "degraded" | "unhealthy";
  /** FM service available */
  fmAvailable: boolean;
  /** Skill library size OK */
  skillLibraryOk: boolean;
  /** Memory system OK */
  memorySystemOk: boolean;
  /** Archivist running */
  archivistRunning: boolean;
  /** Issues if any */
  issues: string[];
}

// --- Progress Display ---

/**
 * Progress bar configuration.
 */
export interface ProgressConfig {
  /** Total expected tasks */
  total: number;
  /** Completed tasks */
  completed: number;
  /** Width of progress bar (chars) */
  width: number;
  /** Fill character */
  fillChar: string;
  /** Empty character */
  emptyChar: string;
  /** Show percentage */
  showPercent: boolean;
}

export const DEFAULT_PROGRESS_CONFIG: ProgressConfig = {
  total: 100,
  completed: 0,
  width: 40,
  fillChar: "█",
  emptyChar: "░",
  showPercent: true,
};

// --- History ---

/**
 * Historical run record for display.
 */
export interface RunRecord {
  /** Run ID */
  id: string;
  /** Benchmark subset */
  subset: TBSubset;
  /** Success rate */
  successRate: number;
  /** Tasks completed */
  tasksCompleted: number;
  /** Duration */
  durationMs: number;
  /** Skills used */
  skillsUsed: number;
  /** Patterns extracted */
  patternsExtracted: number;
  /** Timestamp */
  timestamp: string;
}

// --- Helper Functions ---

/**
 * Format duration for display.
 */
export const formatDuration = (ms: number): string => {
  if (ms < 1000) {
    return `${ms}ms`;
  }
  if (ms < 60000) {
    return `${(ms / 1000).toFixed(1)}s`;
  }
  if (ms < 3600000) {
    return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
  }
  return `${Math.floor(ms / 3600000)}h ${Math.floor((ms % 3600000) / 60000)}m`;
};

/**
 * Format percentage for display.
 */
export const formatPercent = (value: number): string => {
  return `${(value * 100).toFixed(1)}%`;
};

/**
 * Format number with K/M suffix.
 */
export const formatNumber = (n: number): string => {
  if (n < 1000) {
    return n.toString();
  }
  if (n < 1000000) {
    return `${(n / 1000).toFixed(1)}K`;
  }
  return `${(n / 1000000).toFixed(1)}M`;
};

/**
 * Build a progress bar string.
 */
export const buildProgressBar = (config: Partial<ProgressConfig> = {}): string => {
  const c = { ...DEFAULT_PROGRESS_CONFIG, ...config };
  const percent = Math.min(c.completed / c.total, 1);
  const filled = Math.round(percent * c.width);
  const empty = c.width - filled;

  const bar = c.fillChar.repeat(filled) + c.emptyChar.repeat(empty);
  const percentStr = c.showPercent ? ` ${formatPercent(percent)}` : "";

  return `[${bar}]${percentStr}`;
};

/**
 * Get health status color (for terminal).
 */
export const getHealthColor = (status: SystemHealth["status"]): string => {
  switch (status) {
    case "healthy":
      return "\x1b[32m"; // Green
    case "degraded":
      return "\x1b[33m"; // Yellow
    case "unhealthy":
      return "\x1b[31m"; // Red
    default:
      return "\x1b[0m"; // Reset
  }
};

/**
 * Get reset color code.
 */
export const resetColor = (): string => "\x1b[0m";

/**
 * Calculate system health from stats.
 */
export const calculateHealth = (stats: LearningStats): SystemHealth => {
  const issues: string[] = [];

  // Check skill library
  const skillLibraryOk = stats.skills.total >= 10;
  if (!skillLibraryOk) {
    issues.push("Skill library has fewer than 10 skills");
  }

  // Check memory system
  const memorySystemOk = true; // Assume OK if we got this far

  // Check archivist
  const archivistRunning = stats.archivist.totalTrajectories > 0;
  if (!archivistRunning && stats.training.totalRuns > 0) {
    issues.push("Archivist has not recorded any trajectories");
  }

  // FM availability (assume true if we got stats)
  const fmAvailable = true;

  // Determine overall status
  let status: SystemHealth["status"] = "healthy";
  if (issues.length > 0) {
    status = issues.length > 2 ? "unhealthy" : "degraded";
  }

  return {
    status,
    fmAvailable,
    skillLibraryOk,
    memorySystemOk,
    archivistRunning,
    issues,
  };
};

/**
 * Create initial dashboard state.
 */
export const createDashboardState = (
  overrides?: Partial<DashboardState>,
): DashboardState => ({
  mode: "compact",
  interval: "normal",
  lastUpdate: new Date().toISOString(),
  liveUpdates: false,
  currentView: "overview",
  filters: {
    timeRange: "24h",
  },
  ...overrides,
});
