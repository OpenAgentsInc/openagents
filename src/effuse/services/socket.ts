/**
 * Effuse Socket Service
 *
 * Effect service wrapping the desktop socket client.
 */

import { Context, Effect, Stream } from "effect"
import type { HudMessage } from "../../hud/protocol.js"
import type { MCTask, UnifiedTrajectory } from "../../desktop/protocol.js"
import type { Trajectory } from "../../atif/schema.js"
import type {
  TBRunOptions,
  TBRunResult,
  TBSuiteInfo,
  TBRunHistoryItem,
  TBRunDetails,
} from "../../shared/tb-types.js"

// Re-export for convenience
export type { TBRunOptions, TBRunResult, TBSuiteInfo, TBRunHistoryItem, TBRunDetails }

/**
 * Error types for socket operations
 */
export class SocketError extends Error {
  readonly _tag = "SocketError"

  constructor(
    readonly reason: "connection_failed" | "timeout" | "disconnected" | "request_failed",
    message: string
  ) {
    super(message)
    this.name = "SocketError"
  }
}

/** @deprecated Use TBRunOptions from shared/tb-types.ts */
export type StartTBRunOptions = TBRunOptions

/**
 * Options for assigning a task to MechaCoder
 */
export interface AssignTaskOptions {
  sandbox?: boolean
}

/**
 * Service interface for socket communication.
 */
export interface SocketService {
  /** Connect to the desktop server */
  readonly connect: () => Effect.Effect<void, SocketError>

  /** Disconnect from the server */
  readonly disconnect: () => Effect.Effect<void, never>

  /** Check if connected */
  readonly isConnected: () => Effect.Effect<boolean, never>

  /** Get stream of incoming HUD messages (creates stream on first call) */
  readonly getMessages: () => Stream.Stream<HudMessage, never>

  // ============================================================================
  // TB Operations
  // ============================================================================

  /** Load a TB suite file */
  readonly loadTBSuite: (suitePath: string) => Effect.Effect<TBSuiteInfo, SocketError>

  /** Start a TB run */
  readonly startTBRun: (options: StartTBRunOptions) => Effect.Effect<{ runId: string }, SocketError>

  /** Stop the active TB run */
  readonly stopTBRun: () => Effect.Effect<{ stopped: boolean }, SocketError>

  /** Load recent TB run history */
  readonly loadRecentTBRuns: (count?: number) => Effect.Effect<TBRunHistoryItem[], SocketError>

  /** Load full TB run details */
  readonly loadTBRunDetails: (runId: string) => Effect.Effect<TBRunDetails | null, SocketError>

  // ============================================================================
  // Task Operations
  // ============================================================================

  /** Load ready tasks from .openagents/tasks.jsonl */
  readonly loadReadyTasks: (limit?: number) => Effect.Effect<MCTask[], SocketError>

  /** Assign a task to MechaCoder */
  readonly assignTaskToMC: (
    taskId: string,
    options?: AssignTaskOptions
  ) => Effect.Effect<{ assigned: boolean }, SocketError>

  // ============================================================================
  // Trajectory Operations
  // ============================================================================

  /** Load unified trajectories */
  readonly loadUnifiedTrajectories: (limit?: number) => Effect.Effect<UnifiedTrajectory[], SocketError>

  /** Get total count of HF trajectories */
  readonly getHFTrajectoryCount: () => Effect.Effect<number, SocketError>

  /** Get page of HF trajectories */
  readonly getHFTrajectories: (offset: number, limit: number) => Effect.Effect<Trajectory[], SocketError>
}

/**
 * Effect Context.Tag for SocketService
 */
export class SocketServiceTag extends Context.Tag("effuse/SocketService")<
  SocketServiceTag,
  SocketService
>() { }
