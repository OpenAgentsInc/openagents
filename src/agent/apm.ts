/**
 * APM (Actions Per Minute) Measurement for MechaCoder
 *
 * Ports the original Rust APM analyzer to Effect-TS, tracking:
 * - Messages (user + assistant)
 * - Tool calls (from assistant messages)
 * - Time windows: 1h, 6h, 1d, 1w, 1m, lifetime
 *
 * Formula: APM = (message_count + tool_count) / duration_minutes
 *
 * @see docs/apm.md for specification
 * @see docs/transcripts/oa-186-actions-per-minute.md for context
 */

import * as S from "effect/Schema";

// --- Tool Categories (from original Rust implementation) ---

export type ToolCategory =
  | "Code Generation"
  | "File Operations"
  | "System Operations"
  | "Search"
  | "Planning"
  | "Other";

/**
 * Categorize a tool by its name.
 * Matches the original Rust implementation.
 */
export const getToolCategory = (toolName: string): ToolCategory => {
  switch (toolName) {
    case "Edit":
    case "MultiEdit":
    case "Write":
    case "NotebookEdit":
      return "Code Generation";
    case "Read":
    case "LS":
    case "Glob":
      return "File Operations";
    case "Bash":
    case "BashOutput":
    case "KillShell":
      return "System Operations";
    case "Grep":
    case "WebSearch":
    case "WebFetch":
      return "Search";
    case "TodoWrite":
    case "TodoRead":
    case "Task":
      return "Planning";
    default:
      return "Other";
  }
};

// --- Schema Definitions (Effect-TS style, like metrics.ts) ---

export const ToolUsage = S.Struct({
  name: S.String,
  count: S.Number,
  percentage: S.Number,
  category: S.String,
});
export type ToolUsage = S.Schema.Type<typeof ToolUsage>;

export const ProductivityByTime = S.Struct({
  morning: S.Number, // 6am-12pm
  afternoon: S.Number, // 12pm-6pm
  evening: S.Number, // 6pm-12am
  night: S.Number, // 12am-6am
});
export type ProductivityByTime = S.Schema.Type<typeof ProductivityByTime>;

export const APMSession = S.Struct({
  id: S.String,
  project: S.String,
  apm: S.Number,
  durationMinutes: S.Number,
  messageCount: S.Number,
  toolCount: S.Number,
  timestamp: S.String, // ISO date
  source: S.Literal("claude-code", "mechacoder"),
});
export type APMSession = S.Schema.Type<typeof APMSession>;

export const APMStats = S.Struct({
  apm1h: S.Number,
  apm6h: S.Number,
  apm1d: S.Number,
  apm1w: S.Number,
  apm1m: S.Number,
  apmLifetime: S.Number,
  totalSessions: S.Number,
  totalMessages: S.Number,
  totalToolCalls: S.Number,
  totalDurationMinutes: S.Number,
  toolUsage: S.Array(ToolUsage),
  recentSessions: S.Array(APMSession),
  productivityByTime: ProductivityByTime,
});
export type APMStats = S.Schema.Type<typeof APMStats>;

export const APMBySource = S.Struct({
  claudeCode: APMStats,
  mechaCoder: APMStats,
  combined: APMStats,
  comparison: S.Struct({
    apmDelta: S.Number, // mechaCoder.apmLifetime - claudeCode.apmLifetime
    efficiencyRatio: S.Number, // mechaCoder.apmLifetime / claudeCode.apmLifetime
  }),
});
export type APMBySource = S.Schema.Type<typeof APMBySource>;

export const APMCache = S.Struct({
  lastUpdated: S.String, // ISO timestamp
  lastSessionIds: S.Array(S.String), // Most recent sessions processed
  stats: APMBySource,
});
export type APMCache = S.Schema.Type<typeof APMCache>;

// --- Utility Functions ---

/**
 * Calculate APM from action counts and duration.
 * Returns 0 if duration is <= 0.
 */
export const calculateAPM = (
  messageCount: number,
  toolCount: number,
  durationMinutes: number,
): number => {
  if (durationMinutes <= 0) return 0;
  return (messageCount + toolCount) / durationMinutes;
};

/**
 * Get time slot (0-3) from hour of day.
 * 0 = morning (6-12), 1 = afternoon (12-18), 2 = evening (18-24), 3 = night (0-6)
 */
export const getTimeSlot = (hour: number): 0 | 1 | 2 | 3 => {
  if (hour >= 6 && hour < 12) return 0; // morning
  if (hour >= 12 && hour < 18) return 1; // afternoon
  if (hour >= 18 && hour < 24) return 2; // evening
  return 3; // night (0-6)
};

/**
 * Clean project name for display.
 * Converts "-Users-john-projects-myapp" to "~/john/projects/myapp"
 */
export const cleanProjectName = (projectName: string): string => {
  return projectName
    .replace("-Users-", "~/")
    .replace(/-/g, "/")
    .replace(/^~\//, "");
};

// --- Empty/Default Stats ---

export const emptyProductivityByTime = (): ProductivityByTime => ({
  morning: 0,
  afternoon: 0,
  evening: 0,
  night: 0,
});

export const emptyAPMStats = (): APMStats => ({
  apm1h: 0,
  apm6h: 0,
  apm1d: 0,
  apm1w: 0,
  apm1m: 0,
  apmLifetime: 0,
  totalSessions: 0,
  totalMessages: 0,
  totalToolCalls: 0,
  totalDurationMinutes: 0,
  toolUsage: [],
  recentSessions: [],
  productivityByTime: emptyProductivityByTime(),
});

export const emptyAPMBySource = (): APMBySource => ({
  claudeCode: emptyAPMStats(),
  mechaCoder: emptyAPMStats(),
  combined: emptyAPMStats(),
  comparison: {
    apmDelta: 0,
    efficiencyRatio: 0,
  },
});

// --- APM Collector Class (for real-time tracking during MechaCoder runs) ---

export interface APMAction {
  type: "message" | "tool_call";
  toolName?: string | undefined;
  timestamp: number; // Date.now()
}

/**
 * Collects APM metrics during a MechaCoder session.
 * Similar pattern to MetricsCollector in bench/metrics.ts.
 */
export class APMCollector {
  private sessionId: string;
  private project: string;
  private startMs: number;
  private actions: APMAction[] = [];
  private toolCounts: Map<string, number> = new Map();

  constructor(sessionId: string, project: string) {
    this.sessionId = sessionId;
    this.project = project;
    this.startMs = Date.now();
  }

  /**
   * Record an action (message or tool call).
   */
  recordAction(type: "message" | "tool_call", toolName?: string): void {
    this.actions.push({
      type,
      toolName,
      timestamp: Date.now(),
    });

    if (type === "tool_call" && toolName) {
      this.toolCounts.set(toolName, (this.toolCounts.get(toolName) ?? 0) + 1);
    }
  }

  /**
   * Get current session APM.
   */
  getSessionAPM(): number {
    const durationMinutes = (Date.now() - this.startMs) / 60000;
    const messageCount = this.actions.filter((a) => a.type === "message").length;
    const toolCount = this.actions.filter((a) => a.type === "tool_call").length;
    return calculateAPM(messageCount, toolCount, durationMinutes);
  }

  /**
   * Get action count for the last N minutes.
   */
  getRecentAPM(minutes: number): number {
    const cutoff = Date.now() - minutes * 60000;
    const recentActions = this.actions.filter((a) => a.timestamp >= cutoff);
    const messageCount = recentActions.filter((a) => a.type === "message").length;
    const toolCount = recentActions.filter((a) => a.type === "tool_call").length;
    return calculateAPM(messageCount, toolCount, minutes);
  }

  /**
   * Finalize the session and return APMSession data.
   */
  finalize(): APMSession {
    const endMs = Date.now();
    const durationMinutes = (endMs - this.startMs) / 60000;
    const messageCount = this.actions.filter((a) => a.type === "message").length;
    const toolCount = this.actions.filter((a) => a.type === "tool_call").length;
    const startDate = new Date(this.startMs);

    return {
      id: this.sessionId,
      project: this.project,
      apm: calculateAPM(messageCount, toolCount, durationMinutes),
      durationMinutes,
      messageCount,
      toolCount,
      timestamp: startDate.toISOString(),
      source: "mechacoder",
    };
  }

  /**
   * Get tool usage breakdown.
   */
  getToolUsage(): ToolUsage[] {
    const total = Array.from(this.toolCounts.values()).reduce((a, b) => a + b, 0);
    return Array.from(this.toolCounts.entries())
      .map(([name, count]) => ({
        name,
        count,
        percentage: total > 0 ? (count / total) * 100 : 0,
        category: getToolCategory(name),
      }))
      .sort((a, b) => b.count - a.count);
  }
}
