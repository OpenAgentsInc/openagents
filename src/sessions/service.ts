/**
 * SessionService - Append-only JSONL session storage
 *
 * Provides full replayability of agent sessions like Claude Code's ~/.claude/
 * Each session is stored as a JSONL file with one entry per line.
 */
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Context, Effect, Layer } from "effect";
import {
  type AssistantMessageEntry,
  type SessionEndEntry,
  type SessionEntry,
  type SessionMetadata,
  type SessionStartEntry,
  type ToolResultEntry,
  type UsageMetrics,
  type UserMessageEntry,
  decodeSessionEntry,
  extractText,
  generateSessionId,
  generateUuid,
  timestamp,
} from "./schema.js";

export class SessionServiceError extends Error {
  readonly _tag = "SessionServiceError";
  constructor(
    readonly reason: "not_found" | "parse_error" | "write_error" | "invalid_state",
    message: string,
  ) {
    super(message);
    this.name = "SessionServiceError";
  }
}

export interface SessionServiceConfig {
  sessionsDir: string;
}

export interface ActiveSession {
  sessionId: string;
  filePath: string;
  taskId: string | undefined;
  startedAt: string;
  lastUuid: string | null;
  turnCount: number;
  cumulativeUsage: UsageMetrics;
  filesModified: Set<string>;
}

/**
 * SessionService interface
 */
export interface SessionService {
  /**
   * Start a new session
   */
  startSession(options: {
    taskId?: string;
    model?: string;
    provider?: string;
    cwd?: string;
    gitBranch?: string;
    sessionId?: string;
  }): Effect.Effect<ActiveSession, SessionServiceError>;

  /**
   * Log a user message
   */
  logUserMessage(
    session: ActiveSession,
    content: string | unknown[],
    userType?: string,
  ): Effect.Effect<ActiveSession, SessionServiceError>;

  /**
   * Log an assistant message
   */
  logAssistantMessage(
    session: ActiveSession,
    content: string | unknown[],
    options?: {
      model?: string;
      messageId?: string;
      usage?: UsageMetrics;
      requestId?: string;
      stopReason?: string | null;
    },
  ): Effect.Effect<ActiveSession, SessionServiceError>;

  /**
   * Log a tool result
   */
  logToolResult(
    session: ActiveSession,
    toolUseId: string,
    result: unknown,
    isError?: boolean,
  ): Effect.Effect<ActiveSession, SessionServiceError>;

  /**
   * End a session
   */
  endSession(
    session: ActiveSession,
    outcome: "success" | "failure" | "blocked" | "cancelled",
    options?: {
      reason?: string;
      commits?: string[];
    },
  ): Effect.Effect<void, SessionServiceError>;

  /**
   * Load a session by ID
   */
  loadSession(sessionId: string): Effect.Effect<SessionEntry[], SessionServiceError>;

  /**
   * List all sessions
   */
  listSessions(): Effect.Effect<string[], SessionServiceError>;

  /**
   * Get session metadata without loading full content
   */
  getSessionMetadata(sessionId: string): Effect.Effect<SessionMetadata, SessionServiceError>;

  /**
   * Search sessions by text content
   */
  searchSessions(term: string): Effect.Effect<SessionMetadata[], SessionServiceError>;

  /**
   * Find sessions for a specific task
   */
  findSessionsByTask(taskId: string): Effect.Effect<SessionMetadata[], SessionServiceError>;

  /**
   * Get the sessions directory
   */
  getSessionsDir(): string;

  /**
   * Track a modified file
   */
  trackFileModified(session: ActiveSession, filePath: string): ActiveSession;
}

export class SessionServiceTag extends Context.Tag("SessionService")<
  SessionServiceTag,
  SessionService
>() {}

/**
 * Create a live SessionService layer
 */
export const makeSessionService = (config: SessionServiceConfig) =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const pathService = yield* Path.Path;
    const sessionsDir = config.sessionsDir;

    // Ensure sessions directory exists
    yield* fs.makeDirectory(sessionsDir, { recursive: true }).pipe(
      Effect.catchAll(() => Effect.void),
    );

    const appendEntry = (
      filePath: string,
      entry: SessionEntry,
    ): Effect.Effect<void, SessionServiceError> =>
      Effect.gen(function* () {
        const line = JSON.stringify(entry) + "\n";
        yield* fs.writeFile(filePath, new TextEncoder().encode(line), { flag: "a" }).pipe(
          Effect.mapError((e) => new SessionServiceError("write_error", `Failed to append entry: ${e.message}`)),
        );
      });

    const getSessionPath = (sessionId: string): string =>
      pathService.join(sessionsDir, `${sessionId}.jsonl`);

    const service: SessionService = {
      getSessionsDir: () => sessionsDir,

      startSession: (options) =>
        Effect.gen(function* () {
          const sessionId = options.sessionId ?? generateSessionId();
          const filePath = getSessionPath(sessionId);
          const now = timestamp();
          const uuid = generateUuid();

          const entry: SessionStartEntry = {
            type: "session_start",
            uuid,
            timestamp: now,
            sessionId,
            parentUuid: null,
            taskId: options.taskId,
            cwd: options.cwd ?? process.cwd(),
            model: options.model,
            provider: options.provider,
            gitBranch: options.gitBranch,
            version: "1.0.0",
          };

          yield* appendEntry(filePath, entry);

          return {
            sessionId,
            filePath,
            taskId: options.taskId,
            startedAt: now,
            lastUuid: uuid,
            turnCount: 0,
            cumulativeUsage: {},
            filesModified: new Set<string>(),
          };
        }),

      logUserMessage: (session, content, userType) =>
        Effect.gen(function* () {
          const uuid = generateUuid();

          const entry: UserMessageEntry = {
            type: "user",
            uuid,
            timestamp: timestamp(),
            sessionId: session.sessionId,
            parentUuid: session.lastUuid,
            message: {
              role: "user",
              content: typeof content === "string" ? content : content,
            },
            userType,
          };

          yield* appendEntry(session.filePath, entry);

          return {
            ...session,
            lastUuid: uuid,
          };
        }),

      logAssistantMessage: (session, content, options) =>
        Effect.gen(function* () {
          const uuid = generateUuid();

          const entry: AssistantMessageEntry = {
            type: "assistant",
            uuid,
            timestamp: timestamp(),
            sessionId: session.sessionId,
            parentUuid: session.lastUuid,
            message: {
              role: "assistant",
              content: typeof content === "string" ? content : content,
              model: options?.model,
              id: options?.messageId,
              stop_reason: options?.stopReason,
            },
            usage: options?.usage,
            requestId: options?.requestId,
          };

          yield* appendEntry(session.filePath, entry);

          // Update cumulative usage
          const newUsage = { ...session.cumulativeUsage };
          if (options?.usage) {
            newUsage.inputTokens = (newUsage.inputTokens ?? 0) + (options.usage.inputTokens ?? 0);
            newUsage.outputTokens = (newUsage.outputTokens ?? 0) + (options.usage.outputTokens ?? 0);
            newUsage.cacheReadInputTokens =
              (newUsage.cacheReadInputTokens ?? 0) + (options.usage.cacheReadInputTokens ?? 0);
            newUsage.cacheCreationInputTokens =
              (newUsage.cacheCreationInputTokens ?? 0) + (options.usage.cacheCreationInputTokens ?? 0);
            newUsage.totalCostUsd = (newUsage.totalCostUsd ?? 0) + (options.usage.totalCostUsd ?? 0);
          }

          return {
            ...session,
            lastUuid: uuid,
            turnCount: session.turnCount + 1,
            cumulativeUsage: newUsage,
          };
        }),

      logToolResult: (session, toolUseId, result, isError) =>
        Effect.gen(function* () {
          const uuid = generateUuid();

          const entry: ToolResultEntry = {
            type: "tool_result",
            uuid,
            timestamp: timestamp(),
            sessionId: session.sessionId,
            parentUuid: session.lastUuid,
            message: {
              role: "user",
              content: [
                {
                  type: "tool_result",
                  tool_use_id: toolUseId,
                  content: result,
                  is_error: isError,
                },
              ],
            },
            toolUseResult: result,
          };

          yield* appendEntry(session.filePath, entry);

          return {
            ...session,
            lastUuid: uuid,
          };
        }),

      endSession: (session, outcome, options) =>
        Effect.gen(function* () {
          const entry: SessionEndEntry = {
            type: "session_end",
            uuid: generateUuid(),
            timestamp: timestamp(),
            sessionId: session.sessionId,
            parentUuid: session.lastUuid,
            outcome,
            reason: options?.reason,
            totalTurns: session.turnCount,
            totalUsage: session.cumulativeUsage,
            filesModified: Array.from(session.filesModified),
            commits: options?.commits,
          };

          yield* appendEntry(session.filePath, entry);
        }),

      loadSession: (sessionId) =>
        Effect.gen(function* () {
          const filePath = getSessionPath(sessionId);

          const exists = yield* fs.exists(filePath).pipe(
            Effect.mapError((e) => new SessionServiceError("not_found", e.message)),
          );
          if (!exists) {
            return yield* Effect.fail(
              new SessionServiceError("not_found", `Session not found: ${sessionId}`),
            );
          }

          const content = yield* fs.readFileString(filePath).pipe(
            Effect.mapError((e) => new SessionServiceError("not_found", e.message)),
          );

          const lines = content.trim().split("\n").filter((l) => l.length > 0);
          const entries: SessionEntry[] = [];

          for (const line of lines) {
            try {
              const parsed = JSON.parse(line);
              const entry = decodeSessionEntry(parsed);
              entries.push(entry);
            } catch (e) {
              // Skip malformed lines but continue
              console.warn(`Skipping malformed session line: ${line.slice(0, 50)}...`);
            }
          }

          return entries;
        }),

      listSessions: () =>
        Effect.gen(function* () {
          const exists = yield* fs.exists(sessionsDir).pipe(
            Effect.mapError((e) => new SessionServiceError("not_found", e.message)),
          );
          if (!exists) return [];

          const entries = yield* fs.readDirectory(sessionsDir).pipe(
            Effect.mapError((e) => new SessionServiceError("not_found", e.message)),
          );

          return entries
            .filter((e) => e.endsWith(".jsonl"))
            .map((e) => e.replace(".jsonl", ""))
            .sort()
            .reverse();
        }),

      getSessionMetadata: (sessionId) =>
        Effect.gen(function* () {
          const entries = yield* service.loadSession(sessionId);
          if (entries.length === 0) {
            return yield* Effect.fail(
              new SessionServiceError("not_found", `Session empty or not found: ${sessionId}`),
            );
          }

          const startEntry = entries.find((e): e is SessionStartEntry => e.type === "session_start");
          const endEntry = entries.find((e): e is SessionEndEntry => e.type === "session_end");
          const firstUserMessage = entries.find((e): e is UserMessageEntry => e.type === "user");

          if (!startEntry) {
            return yield* Effect.fail(
              new SessionServiceError("parse_error", `No session_start entry found: ${sessionId}`),
            );
          }

          const metadata: SessionMetadata = {
            sessionId,
            taskId: startEntry.taskId,
            startedAt: startEntry.timestamp,
            endedAt: endEntry?.timestamp,
            outcome: endEntry?.outcome,
            totalTurns: endEntry?.totalTurns ?? entries.filter((e) => e.type === "assistant").length,
            totalUsage: endEntry?.totalUsage,
            filesModified: endEntry?.filesModified,
            commits: endEntry?.commits,
            model: startEntry.model,
            cwd: startEntry.cwd,
            firstUserMessage: firstUserMessage
              ? extractText(firstUserMessage.message.content)
              : undefined,
          };

          return metadata;
        }),

      searchSessions: (term) =>
        Effect.gen(function* () {
          const sessionIds = yield* service.listSessions();
          const results: SessionMetadata[] = [];
          const lowerTerm = term.toLowerCase();

          for (const sessionId of sessionIds) {
            const entries = yield* service.loadSession(sessionId).pipe(
              Effect.catchAll(() => Effect.succeed([] as SessionEntry[])),
            );

            // Search through all text content
            let found = false;
            for (const entry of entries) {
              if (entry.type === "user" || entry.type === "assistant") {
                const text = extractText(entry.message.content);
                if (text.toLowerCase().includes(lowerTerm)) {
                  found = true;
                  break;
                }
              }
            }

            if (found) {
              const metadata = yield* service.getSessionMetadata(sessionId).pipe(
                Effect.catchAll(() => Effect.succeed(null)),
              );
              if (metadata) results.push(metadata);
            }
          }

          return results;
        }),

      findSessionsByTask: (taskId) =>
        Effect.gen(function* () {
          const sessionIds = yield* service.listSessions();
          const results: SessionMetadata[] = [];

          for (const sessionId of sessionIds) {
            const metadata = yield* service.getSessionMetadata(sessionId).pipe(
              Effect.catchAll(() => Effect.succeed(null)),
            );
            if (metadata?.taskId === taskId) {
              results.push(metadata);
            }
          }

          return results;
        }),

      trackFileModified: (session, filePath) => ({
        ...session,
        filesModified: new Set([...session.filesModified, filePath]),
      }),
    };

    return service;
  });

/**
 * Create a SessionService layer with config
 */
export const SessionServiceLive = (config: SessionServiceConfig) =>
  Layer.effect(
    SessionServiceTag,
    makeSessionService(config),
  );

/**
 * Default sessions directory
 */
export const DEFAULT_SESSIONS_DIR = ".openagents/sessions";
