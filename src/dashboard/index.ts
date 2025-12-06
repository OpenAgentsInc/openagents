/**
 * Dashboard Module
 *
 * Provides visibility into MechaCoder's learning system.
 *
 * Features:
 * - Real-time training progress
 * - Skill library statistics
 * - Memory system metrics
 * - System health monitoring
 * - Historical run tracking
 *
 * @module
 */

// Schema exports
export {
  type DisplayMode,
  type UpdateInterval,
  type DashboardState,
  type DashboardView,
  type DashboardFilters,
  type DashboardMetrics,
  type MetricTrends,
  type SystemHealth,
  type ProgressConfig,
  type RunRecord,
  UPDATE_INTERVALS,
  DEFAULT_PROGRESS_CONFIG,
  formatDuration,
  formatPercent,
  formatNumber,
  buildProgressBar,
  getHealthColor,
  resetColor,
  calculateHealth,
  createDashboardState,
} from "./schema.js";

// Reporter exports
export {
  Reporter,
  ReporterLive,
  type IReporter,
} from "./reporter.js";
