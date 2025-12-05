/**
 * Effuse Socket Service
 *
 * Effect service wrapping the desktop socket client.
 */

import { Context, Effect, Stream } from "effect"
import type { HudMessage } from "../../hud/protocol.js"
import type {
  TBSuiteInfo,
  TBRunHistoryItem,
  TBRunDetails,
  MCTask,
  UnifiedTrajectory,
} from "../../desktop/protocol.js"

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

/**
 * Options for starting a TB run
 */
export interface StartTBRunOptions {
  suitePath: string
  taskIds?: string[]
  timeout?: number
  maxTurns?: number
  outputDir?: string
}

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

  /** Stream of incoming HUD messages */
  readonly messages: Stream.Stream<HudMessage, never>

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
}

/**
 * Effect Context.Tag for SocketService
 */
export class SocketServiceTag extends Context.Tag("effuse/SocketService")<
  SocketServiceTag,
  SocketService
>() {}
