/**
 * APM Parser - Reads Claude Code conversation JSONL files and extracts APM metrics.
 *
 * Data source: ~/.claude/projects/<project-name>/*.jsonl
 *
 * MechaCoder sessions are identified by:
 * - userType: "external" (vs interactive user)
 * - Message content containing "## Subtask: oa-*" patterns
 */

import * as Effect from "effect/Effect";
import * as FileSystem from "@effect/platform/FileSystem";
import * as os from "node:os";
import * as path from "node:path";
import {
  type APMSession,
  type APMStats,
  type APMBySource,
  type ToolUsage,
  type ProductivityByTime,
  calculateAPM,
  getTimeSlot,
  cleanProjectName,
  emptyAPMStats,
  emptyAPMBySource,
} from "./apm.js";

// --- JSONL Entry Types (matching Claude Code format) ---

interface ConversationEntry {
  type: string;
  sessionId?: string;
  timestamp?: string;
  message?: {
    role?: string;
    content?: unknown[];
  };
  userType?: string;
  cwd?: string;
}

interface ToolUseContent {
  type: "tool_use";
  name: string;
  input?: unknown;
}

// --- Parser Functions ---

/**
 * Check if a session is from MechaCoder based on markers.
 */
const isMechaCoderSession = (entries: ConversationEntry[]): boolean => {
  // Check for userType: "external"
  const hasExternalUser = entries.some((e) => e.userType === "external");
  if (!hasExternalUser) return false;

  // Check for subtask pattern in message content
  for (const entry of entries) {
    if (entry.message?.content) {
      const content = JSON.stringify(entry.message.content);
      if (content.includes("## Subtask: oa-") || content.includes("Subtask:")) {
        return true;
      }
    }
  }
  return false;
};

/**
 * Extract tool calls from an assistant message.
 */
const extractToolCalls = (entry: ConversationEntry): string[] => {
  const tools: string[] = [];
  if (entry.message?.role === "assistant" && Array.isArray(entry.message.content)) {
    for (const item of entry.message.content) {
      if (
        typeof item === "object" &&
        item !== null &&
        "type" in item &&
        (item as ToolUseContent).type === "tool_use"
      ) {
        const toolUse = item as ToolUseContent;
        if (toolUse.name) {
          tools.push(toolUse.name);
        }
      }
    }
  }
  return tools;
};

/**
 * Parse a single JSONL file and extract session data.
 */
const parseJSONLFile = (
  content: string,
  projectName: string,
): APMSession | null => {
  const lines = content.split("\n").filter((line) => line.trim());
  const entries: ConversationEntry[] = [];

  for (const line of lines) {
    try {
      const entry = JSON.parse(line) as ConversationEntry;
      entries.push(entry);
    } catch {
      // Skip invalid JSON lines
    }
  }

  if (entries.length < 2) return null;

  // Get session ID
  const sessionId = entries.find((e) => e.sessionId)?.sessionId;
  if (!sessionId) return null;

  // Count messages and tools
  let messageCount = 0;
  let toolCount = 0;
  const timestamps: Date[] = [];
  const toolNames: string[] = [];

  for (const entry of entries) {
    // Count messages
    if (entry.message?.role === "user" || entry.message?.role === "assistant") {
      messageCount++;
    }

    // Extract tool calls
    const tools = extractToolCalls(entry);
    toolCount += tools.length;
    toolNames.push(...tools);

    // Collect timestamps
    if (entry.timestamp) {
      try {
        timestamps.push(new Date(entry.timestamp));
      } catch {
        // Skip invalid timestamps
      }
    }
  }

  if (timestamps.length < 2) return null;

  // Calculate duration
  timestamps.sort((a, b) => a.getTime() - b.getTime());
  const startTime = timestamps[0];
  const endTime = timestamps[timestamps.length - 1];
  const durationMinutes = (endTime.getTime() - startTime.getTime()) / 60000;

  if (durationMinutes <= 0) return null;

  // Determine source
  const source = isMechaCoderSession(entries) ? "mechacoder" : "claude-code";

  return {
    id: sessionId,
    project: cleanProjectName(projectName),
    apm: calculateAPM(messageCount, toolCount, durationMinutes),
    durationMinutes,
    messageCount,
    toolCount,
    timestamp: startTime.toISOString(),
    source,
  };
};

/**
 * Calculate APM for a time window.
 */
const calculateWindowAPM = (
  sessions: APMSession[],
  hoursBack: number,
  windowMinutes: number,
): number => {
  const cutoff = Date.now() - hoursBack * 60 * 60 * 1000;
  let windowMessages = 0;
  let windowTools = 0;

  for (const session of sessions) {
    const sessionTime = new Date(session.timestamp).getTime();
    if (sessionTime >= cutoff) {
      windowMessages += session.messageCount;
      windowTools += session.toolCount;
    }
  }

  return calculateAPM(windowMessages, windowTools, windowMinutes);
};

/**
 * Aggregate sessions into APMStats.
 */
const aggregateSessions = (sessions: APMSession[]): APMStats => {
  if (sessions.length === 0) return emptyAPMStats();

  // Sort by timestamp
  sessions.sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
  );

  // Calculate totals
  let totalMessages = 0;
  let totalToolCalls = 0;
  let totalDuration = 0;
  const productivitySlots: [number[], number[], number[], number[]] = [
    [],
    [],
    [],
    [],
  ];

  // Track timestamps for lifetime calculation
  let earliestTimestamp: Date | null = null;
  let latestTimestamp: Date | null = null;

  for (const session of sessions) {
    totalMessages += session.messageCount;
    totalToolCalls += session.toolCount;
    totalDuration += session.durationMinutes;

    // Track time range
    const sessionDate = new Date(session.timestamp);
    if (!earliestTimestamp || sessionDate < earliestTimestamp) {
      earliestTimestamp = sessionDate;
    }
    if (!latestTimestamp || sessionDate > latestTimestamp) {
      latestTimestamp = sessionDate;
    }

    // Track productivity by time of day
    const hour = sessionDate.getHours();
    const slot = getTimeSlot(hour);
    productivitySlots[slot].push(session.apm);
  }

  // Calculate time window APMs
  const apm1h = calculateWindowAPM(sessions, 1, 60);
  const apm6h = calculateWindowAPM(sessions, 6, 360);
  const apm1d = calculateWindowAPM(sessions, 24, 1440);
  const apm1w = calculateWindowAPM(sessions, 168, 10080);
  const apm1m = calculateWindowAPM(sessions, 720, 43200);

  // Lifetime APM: wall-clock time from first to last conversation
  let apmLifetime = 0;
  if (earliestTimestamp && latestTimestamp) {
    const lifetimeMinutes =
      (latestTimestamp.getTime() - earliestTimestamp.getTime()) / 60000;
    if (lifetimeMinutes > 0) {
      apmLifetime = (totalMessages + totalToolCalls) / lifetimeMinutes;
    }
  }

  // Calculate productivity by time
  const avgSlot = (values: number[]) =>
    values.length > 0
      ? values.reduce((a, b) => a + b, 0) / values.length
      : 0;

  const productivityByTime: ProductivityByTime = {
    morning: avgSlot(productivitySlots[0]),
    afternoon: avgSlot(productivitySlots[1]),
    evening: avgSlot(productivitySlots[2]),
    night: avgSlot(productivitySlots[3]),
  };

  // Build tool usage (would need to re-parse files for accurate counts)
  // For now, return empty - would need to track during parsing
  const toolUsage: ToolUsage[] = [];

  return {
    apm1h,
    apm6h,
    apm1d,
    apm1w,
    apm1m,
    apmLifetime,
    totalSessions: sessions.length,
    totalMessages,
    totalToolCalls,
    totalDurationMinutes: totalDuration,
    toolUsage,
    recentSessions: sessions.slice(-20), // Last 20 sessions
    productivityByTime,
  };
};

// --- Main Parser Effect ---

/**
 * Parse all Claude Code conversations and return APM stats by source.
 */
export const parseClaudeConversations = Effect.gen(function* () {
  const fs = yield* FileSystem.FileSystem;

  const homeDir = os.homedir();
  const claudeDir = path.join(homeDir, ".claude", "projects");

  // Check if directory exists
  const exists = yield* fs.exists(claudeDir).pipe(
    Effect.catchAll(() => Effect.succeed(false)),
  );

  if (!exists) {
    return emptyAPMBySource();
  }

  // Read project directories
  const projectDirs = yield* fs.readDirectory(claudeDir).pipe(
    Effect.catchAll(() => Effect.succeed([] as string[])),
  );

  const claudeCodeSessions: APMSession[] = [];
  const mechaCoderSessions: APMSession[] = [];

  // Process each project
  for (const projectName of projectDirs) {
    if (projectName.startsWith(".")) continue;

    const projectPath = path.join(claudeDir, projectName);

    // Check if it's a directory
    const stat = yield* fs.stat(projectPath).pipe(
      Effect.catchAll(() => Effect.succeed(null)),
    );
    if (!stat || stat.type !== "Directory") continue;

    // Read JSONL files
    const files = yield* fs.readDirectory(projectPath).pipe(
      Effect.catchAll(() => Effect.succeed([] as string[])),
    );

    for (const fileName of files) {
      if (!fileName.endsWith(".jsonl")) continue;

      const filePath = path.join(projectPath, fileName);
      const content = yield* fs.readFileString(filePath).pipe(
        Effect.catchAll(() => Effect.succeed("")),
      );

      if (!content) continue;

      const session = parseJSONLFile(content, projectName);
      if (session) {
        if (session.source === "mechacoder") {
          mechaCoderSessions.push(session);
        } else {
          claudeCodeSessions.push(session);
        }
      }
    }
  }

  // Aggregate stats
  const claudeCodeStats = aggregateSessions(claudeCodeSessions);
  const mechaCoderStats = aggregateSessions(mechaCoderSessions);
  const combinedStats = aggregateSessions([
    ...claudeCodeSessions,
    ...mechaCoderSessions,
  ]);

  // Calculate comparison metrics
  const apmDelta = mechaCoderStats.apmLifetime - claudeCodeStats.apmLifetime;
  const efficiencyRatio =
    claudeCodeStats.apmLifetime > 0
      ? mechaCoderStats.apmLifetime / claudeCodeStats.apmLifetime
      : 0;

  return {
    claudeCode: claudeCodeStats,
    mechaCoder: mechaCoderStats,
    combined: combinedStats,
    comparison: {
      apmDelta,
      efficiencyRatio,
    },
  } satisfies APMBySource;
});

/**
 * Get APM stats for a specific project.
 */
export const parseProjectConversations = (projectDir: string) =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const homeDir = os.homedir();
    // Convert /Users/foo/code/project to -Users-foo-code-project
    const projectName = projectDir.replace(/\//g, "-").replace(/^-/, "");
    const claudeProjectDir = path.join(homeDir, ".claude", "projects", projectName);

    const exists = yield* fs.exists(claudeProjectDir).pipe(
      Effect.catchAll(() => Effect.succeed(false)),
    );

    if (!exists) {
      return emptyAPMBySource();
    }

    const files = yield* fs.readDirectory(claudeProjectDir).pipe(
      Effect.catchAll(() => Effect.succeed([] as string[])),
    );

    const claudeCodeSessions: APMSession[] = [];
    const mechaCoderSessions: APMSession[] = [];

    for (const fileName of files) {
      if (!fileName.endsWith(".jsonl")) continue;

      const filePath = path.join(claudeProjectDir, fileName);
      const content = yield* fs.readFileString(filePath).pipe(
        Effect.catchAll(() => Effect.succeed("")),
      );

      if (!content) continue;

      const session = parseJSONLFile(content, projectName);
      if (session) {
        if (session.source === "mechacoder") {
          mechaCoderSessions.push(session);
        } else {
          claudeCodeSessions.push(session);
        }
      }
    }

    const claudeCodeStats = aggregateSessions(claudeCodeSessions);
    const mechaCoderStats = aggregateSessions(mechaCoderSessions);
    const combinedStats = aggregateSessions([
      ...claudeCodeSessions,
      ...mechaCoderSessions,
    ]);

    const apmDelta = mechaCoderStats.apmLifetime - claudeCodeStats.apmLifetime;
    const efficiencyRatio =
      claudeCodeStats.apmLifetime > 0
        ? mechaCoderStats.apmLifetime / claudeCodeStats.apmLifetime
        : 0;

    return {
      claudeCode: claudeCodeStats,
      mechaCoder: mechaCoderStats,
      combined: combinedStats,
      comparison: {
        apmDelta,
        efficiencyRatio,
      },
    } satisfies APMBySource;
  });
