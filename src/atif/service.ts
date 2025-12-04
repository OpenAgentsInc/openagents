/**
 * TrajectoryService - ATIF Trajectory Storage and Retrieval
 *
 * Persists trajectories to disk in date-organized folders:
 * .openagents/trajectories/YYYYMMDD/<session-id>.atif.json
 *
 * Follows the SessionService pattern from src/sessions/service.ts
 */
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Context, Effect, Layer } from "effect";
import {
  type Trajectory,
  decodeTrajectory,
  extractSubagentSessionIds,
} from "./schema.js";
import {
  validateTrajectory,
  type TrajectoryValidationError,
} from "./validation.js";

// ============================================================================
// Error Types
// ============================================================================

export class TrajectoryServiceError extends Error {
  readonly _tag = "TrajectoryServiceError";

  constructor(
    readonly reason:
      | "not_found"
      | "parse_error"
      | "write_error"
      | "validation_failed"
      | "invalid_path",
    message: string,
  ) {
    super(message);
    this.name = "TrajectoryServiceError";
  }
}

// ============================================================================
// Configuration
// ============================================================================

export interface TrajectoryServiceConfig {
  /** Base directory for trajectories (default: .openagents/trajectories) */
  trajectoriesDir: string;
  /** Whether to validate before saving (default: true) */
  validateOnSave?: boolean;
}

export const DEFAULT_TRAJECTORIES_DIR = ".openagents/trajectories";

// ============================================================================
// Metadata Types
// ============================================================================

export interface TrajectoryMetadata {
  sessionId: string;
  agentName: string;
  agentVersion: string;
  modelName: string;
  totalSteps: number;
  totalCostUsd: number | undefined;
  parentSessionId: string | undefined;
  childSessionIds: string[];
  filePath: string;
  createdAt: string;
}

// ============================================================================
// Service Interface
// ============================================================================

export interface TrajectoryService {
  /**
   * Save a trajectory to disk
   */
  saveTrajectory(
    trajectory: Trajectory,
  ): Effect.Effect<
    string,
    TrajectoryServiceError | TrajectoryValidationError
  >;

  /**
   * Load a trajectory by session ID
   */
  loadTrajectory(
    sessionId: string,
  ): Effect.Effect<Trajectory, TrajectoryServiceError>;

  /**
   * List all trajectory session IDs
   */
  listTrajectories(): Effect.Effect<string[], TrajectoryServiceError>;

  /**
   * List trajectories for a specific date (YYYYMMDD)
   */
  listTrajectoriesForDate(
    date: string,
  ): Effect.Effect<string[], TrajectoryServiceError>;

  /**
   * Get trajectory metadata without loading full content
   */
  getTrajectoryMetadata(
    sessionId: string,
  ): Effect.Effect<TrajectoryMetadata, TrajectoryServiceError>;

  /**
   * Find child trajectories of a parent session
   */
  findChildTrajectories(
    parentSessionId: string,
  ): Effect.Effect<TrajectoryMetadata[], TrajectoryServiceError>;

  /**
   * Get the full trajectory tree (parent + all descendants)
   */
  getTrajectoryTree(
    sessionId: string,
  ): Effect.Effect<Trajectory[], TrajectoryServiceError>;

  /**
   * Find trajectories by agent name
   */
  findByAgent(
    agentName: string,
  ): Effect.Effect<TrajectoryMetadata[], TrajectoryServiceError>;

  /**
   * Delete a trajectory
   */
  deleteTrajectory(
    sessionId: string,
  ): Effect.Effect<void, TrajectoryServiceError>;

  /**
   * Get the trajectories directory path
   */
  getTrajectoriesDir(): string;

  /**
   * Get the file path for a trajectory
   */
  getTrajectoryPath(sessionId: string): string | null;
}

// ============================================================================
// Service Tag
// ============================================================================

export class TrajectoryServiceTag extends Context.Tag("TrajectoryService")<
  TrajectoryServiceTag,
  TrajectoryService
>() {}

// ============================================================================
// Service Implementation
// ============================================================================

export const makeTrajectoryService = (config: TrajectoryServiceConfig) =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const pathService = yield* Path.Path;
    const trajectoriesDir = config.trajectoriesDir;
    const validateOnSave = config.validateOnSave ?? true;

    // Cache of session ID -> file path for quick lookups
    const pathCache = new Map<string, string>();

    // Ensure base directory exists
    yield* fs.makeDirectory(trajectoriesDir, { recursive: true }).pipe(
      Effect.catchAll(() => Effect.void),
    );

    /**
     * Get date folder from session ID or current date
     */
    const getDateFolder = (sessionId: string): string => {
      // Try to extract date from session ID (format: session-YYYY-MM-DDTHH-MM-SS-...)
      const match = sessionId.match(/session-(\d{4})-(\d{2})-(\d{2})/);
      if (match) {
        return `${match[1]}${match[2]}${match[3]}`;
      }
      // Fallback to current date
      const now = new Date();
      return now.toISOString().slice(0, 10).replace(/-/g, "");
    };

    /**
     * Get file path for a session
     */
    const getFilePath = (sessionId: string): string => {
      const cached = pathCache.get(sessionId);
      if (cached) return cached;

      const dateFolder = getDateFolder(sessionId);
      return pathService.join(
        trajectoriesDir,
        dateFolder,
        `${sessionId}.atif.json`,
      );
    };

    /**
     * Find file path by searching date folders
     */
    const findFilePath = (
      sessionId: string,
    ): Effect.Effect<string | null, TrajectoryServiceError> =>
      Effect.gen(function* () {
        // Check cache first
        const cached = pathCache.get(sessionId);
        if (cached) return cached;

        // Check expected path first
        const expectedPath = getFilePath(sessionId);
        const exists = yield* fs.exists(expectedPath).pipe(
          Effect.catchAll(() => Effect.succeed(false)),
        );
        if (exists) {
          pathCache.set(sessionId, expectedPath);
          return expectedPath;
        }

        // Search all date folders
        const dirExists = yield* fs.exists(trajectoriesDir).pipe(
          Effect.catchAll(() => Effect.succeed(false)),
        );
        if (!dirExists) return null;

        const dateFolders = yield* fs.readDirectory(trajectoriesDir).pipe(
          Effect.catchAll(() => Effect.succeed([] as string[])),
        );

        for (const dateFolder of dateFolders) {
          const folderPath = pathService.join(trajectoriesDir, dateFolder);
          const files = yield* fs.readDirectory(folderPath).pipe(
            Effect.catchAll(() => Effect.succeed([] as string[])),
          );

          const fileName = `${sessionId}.atif.json`;
          if (files.includes(fileName)) {
            const fullPath = pathService.join(folderPath, fileName);
            pathCache.set(sessionId, fullPath);
            return fullPath;
          }
        }

        return null;
      });

    const service: TrajectoryService = {
      getTrajectoriesDir: () => trajectoriesDir,

      getTrajectoryPath: (sessionId) => pathCache.get(sessionId) ?? null,

      saveTrajectory: (trajectory) =>
        Effect.gen(function* () {
          // Validate if enabled
          if (validateOnSave) {
            yield* validateTrajectory(trajectory);
          }

          const filePath = getFilePath(trajectory.session_id);
          const dir = pathService.dirname(filePath);

          // Ensure date directory exists
          yield* fs.makeDirectory(dir, { recursive: true }).pipe(
            Effect.catchAll(() => Effect.void),
          );

          // Format JSON with 2-space indent
          const content = JSON.stringify(trajectory, null, 2);

          yield* fs
            .writeFileString(filePath, content)
            .pipe(
              Effect.mapError(
                (e) =>
                  new TrajectoryServiceError(
                    "write_error",
                    `Failed to write trajectory: ${e.message}`,
                  ),
              ),
            );

          // Update cache
          pathCache.set(trajectory.session_id, filePath);

          return filePath;
        }),

      loadTrajectory: (sessionId) =>
        Effect.gen(function* () {
          const filePath = yield* findFilePath(sessionId);
          if (!filePath) {
            return yield* Effect.fail(
              new TrajectoryServiceError(
                "not_found",
                `Trajectory not found: ${sessionId}`,
              ),
            );
          }

          const content = yield* fs.readFileString(filePath).pipe(
            Effect.mapError(
              (e) =>
                new TrajectoryServiceError("not_found", `Failed to read: ${e.message}`),
            ),
          );

          try {
            const parsed = JSON.parse(content);
            return decodeTrajectory(parsed);
          } catch (e) {
            return yield* Effect.fail(
              new TrajectoryServiceError(
                "parse_error",
                `Failed to parse trajectory: ${e}`,
              ),
            );
          }
        }),

      listTrajectories: () =>
        Effect.gen(function* () {
          const dirExists = yield* fs.exists(trajectoriesDir).pipe(
            Effect.catchAll(() => Effect.succeed(false)),
          );
          if (!dirExists) return [];

          const dateFolders = yield* fs.readDirectory(trajectoriesDir).pipe(
            Effect.catchAll(() => Effect.succeed([] as string[])),
          );

          const sessionIds: string[] = [];

          for (const dateFolder of dateFolders.sort().reverse()) {
            const folderPath = pathService.join(trajectoriesDir, dateFolder);
            const files = yield* fs.readDirectory(folderPath).pipe(
              Effect.catchAll(() => Effect.succeed([] as string[])),
            );

            for (const file of files) {
              if (file.endsWith(".atif.json")) {
                const sessionId = file.replace(".atif.json", "");
                sessionIds.push(sessionId);
                // Update cache
                pathCache.set(
                  sessionId,
                  pathService.join(folderPath, file),
                );
              }
            }
          }

          return sessionIds;
        }),

      listTrajectoriesForDate: (date) =>
        Effect.gen(function* () {
          const folderPath = pathService.join(trajectoriesDir, date);
          const exists = yield* fs.exists(folderPath).pipe(
            Effect.catchAll(() => Effect.succeed(false)),
          );
          if (!exists) return [];

          const files = yield* fs.readDirectory(folderPath).pipe(
            Effect.catchAll(() => Effect.succeed([] as string[])),
          );

          return files
            .filter((f) => f.endsWith(".atif.json"))
            .map((f) => f.replace(".atif.json", ""));
        }),

      getTrajectoryMetadata: (sessionId) =>
        Effect.gen(function* () {
          const trajectory = yield* service.loadTrajectory(sessionId);
          const filePath = pathCache.get(sessionId) ?? getFilePath(sessionId);

          const childSessionIds = extractSubagentSessionIds(trajectory);
          const parentSessionId =
            trajectory.extra?.parent_session_id as string | undefined;

          return {
            sessionId: trajectory.session_id,
            agentName: trajectory.agent.name,
            agentVersion: trajectory.agent.version,
            modelName: trajectory.agent.model_name,
            totalSteps: trajectory.steps.length,
            totalCostUsd: trajectory.final_metrics?.total_cost_usd,
            parentSessionId,
            childSessionIds,
            filePath,
            createdAt:
              trajectory.steps[0]?.timestamp ?? new Date().toISOString(),
          };
        }),

      findChildTrajectories: (parentSessionId) =>
        Effect.gen(function* () {
          const allIds = yield* service.listTrajectories();
          const children: TrajectoryMetadata[] = [];

          for (const sessionId of allIds) {
            const metadata = yield* service.getTrajectoryMetadata(sessionId).pipe(
              Effect.catchAll(() => Effect.succeed(null)),
            );
            if (metadata?.parentSessionId === parentSessionId) {
              children.push(metadata);
            }
          }

          return children;
        }),

      getTrajectoryTree: (sessionId) =>
        Effect.gen(function* () {
          const trajectories: Trajectory[] = [];
          const visited = new Set<string>();
          const queue = [sessionId];

          while (queue.length > 0) {
            const id = queue.shift()!;
            if (visited.has(id)) continue;
            visited.add(id);

            const trajectory = yield* service.loadTrajectory(id).pipe(
              Effect.catchAll(() => Effect.succeed(null)),
            );

            if (trajectory) {
              trajectories.push(trajectory);
              // Add child session IDs to queue
              const childIds = extractSubagentSessionIds(trajectory);
              queue.push(...childIds);
            }
          }

          return trajectories;
        }),

      findByAgent: (agentName) =>
        Effect.gen(function* () {
          const allIds = yield* service.listTrajectories();
          const results: TrajectoryMetadata[] = [];

          for (const sessionId of allIds) {
            const metadata = yield* service.getTrajectoryMetadata(sessionId).pipe(
              Effect.catchAll(() => Effect.succeed(null)),
            );
            if (metadata?.agentName === agentName) {
              results.push(metadata);
            }
          }

          return results;
        }),

      deleteTrajectory: (sessionId) =>
        Effect.gen(function* () {
          const filePath = yield* findFilePath(sessionId);
          if (!filePath) {
            return yield* Effect.fail(
              new TrajectoryServiceError(
                "not_found",
                `Trajectory not found: ${sessionId}`,
              ),
            );
          }

          yield* fs.remove(filePath).pipe(
            Effect.mapError(
              (e) =>
                new TrajectoryServiceError(
                  "write_error",
                  `Failed to delete: ${e.message}`,
                ),
            ),
          );

          pathCache.delete(sessionId);
        }),
    };

    return service;
  });

// ============================================================================
// Layer
// ============================================================================

export const TrajectoryServiceLive = (config: TrajectoryServiceConfig) =>
  Layer.effect(TrajectoryServiceTag, makeTrajectoryService(config));

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Format a trajectory as pretty JSON (compact arrays)
 */
export const formatTrajectoryJson = (trajectory: Trajectory): string => {
  // First dump with standard formatting
  let json = JSON.stringify(trajectory, null, 2);

  // Compact arrays of numbers (token_ids, logprobs) onto single lines
  const numericArrayPattern =
    /\[\s*\n\s*-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?(?:\s*,\s*\n\s*-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)*\s*\n\s*\]/g;

  json = json.replace(numericArrayPattern, (match) => {
    const numbers = match.match(/-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/g);
    if (!numbers) return match;
    return `[${numbers.join(", ")}]`;
  });

  return json;
};
