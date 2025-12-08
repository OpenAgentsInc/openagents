/**
 * Desktop Socket Protocol
 *
 * Extends the HUD protocol with bidirectional request/response support.
 * This replaces Electrobun RPC with a unified WebSocket protocol.
 *
 * Architecture:
 * - All messages flow over a single WebSocket connection
 * - Events (HudMessage) flow server -> client (one-way)
 * - Requests flow client -> server with correlation IDs
 * - Responses flow server -> client with matching correlation IDs
 */

import type { HudMessage } from "../hud/protocol.js";
import type { TBRunOptions, TBSuiteInfo, TBRunHistoryItem, TBRunDetails } from "../shared/tb-types.js";

// Re-export shared types for convenience
export type { TBRunOptions, TBSuiteInfo, TBRunHistoryItem, TBRunDetails } from "../shared/tb-types.js";

// ============================================================================
// Correlation ID Support
// ============================================================================

/**
 * Generate a unique correlation ID for request/response matching
 */
export function generateCorrelationId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

// ============================================================================
// Request Types (Client -> Server)
// ============================================================================

/**
 * Base interface for all requests
 */
interface BaseRequest {
  /** Unique ID for correlating response */
  correlationId: string;
}

/**
 * Load TB suite information
 */
export interface LoadTBSuiteRequest extends BaseRequest {
  type: "request:loadTBSuite";
  suitePath: string;
}

/**
 * Start a TB run
 */
export interface StartTBRunRequest extends BaseRequest, TBRunOptions {
  type: "request:startTBRun";
}

/**
 * Stop the active TB run
 */
export interface StopTBRunRequest extends BaseRequest {
  type: "request:stopTBRun";
}

/**
 * Load recent TB run history
 */
export interface LoadRecentTBRunsRequest extends BaseRequest {
  type: "request:loadRecentTBRuns";
  count?: number;
}

/**
 * Load full TB run details
 */
export interface LoadTBRunDetailsRequest extends BaseRequest {
  type: "request:loadTBRunDetails";
  runId: string;
}

/**
 * Load ready tasks from .openagents/tasks.jsonl
 */
export interface LoadReadyTasksRequest extends BaseRequest {
  type: "request:loadReadyTasks";
  limit?: number;
}

export interface AssignTaskToMCRequest extends BaseRequest {
  type: "request:assignTaskToMC";
  taskId: string;
  options?: {
    sandbox?: boolean;
  };
}

/**
 * Load unified trajectories (TB runs + ATIF trajectories merged)
 */
export interface LoadUnifiedTrajectoriesRequest extends BaseRequest {
  type: "request:loadUnifiedTrajectories";
  limit?: number;
}

/**
 * Get total count of HF trajectories
 */
export interface GetHFTrajectoryCountRequest extends BaseRequest {
  type: "request:getHFTrajectoryCount";
}

/**
 * Get paginated HF trajectories
 */
export interface GetHFTrajectoriesRequest extends BaseRequest {
  type: "request:getHFTrajectories";
  offset?: number;
  limit?: number;
}

/**
 * Get a single HF trajectory by index
 */
export interface GetHFTrajectoryRequest extends BaseRequest {
  type: "request:getHFTrajectory";
  index: number;
}

/**
 * Union of all request types
 */
export type SocketRequest =
  | LoadTBSuiteRequest
  | StartTBRunRequest
  | StopTBRunRequest
  | LoadRecentTBRunsRequest
  | LoadTBRunDetailsRequest
  | LoadReadyTasksRequest
  | AssignTaskToMCRequest
  | LoadUnifiedTrajectoriesRequest
  | GetHFTrajectoryCountRequest
  | GetHFTrajectoriesRequest
  | GetHFTrajectoryRequest;

// ============================================================================
// Response Types (Server -> Client)
// ============================================================================

/**
 * Base interface for all responses
 */
interface BaseResponse {
  /** Correlation ID matching the request */
  correlationId: string;
  /** Whether the request succeeded */
  success: boolean;
  /** Error message if success is false */
  error?: string;
}

/**
 * Response to LoadTBSuiteRequest
 */
export interface LoadTBSuiteResponse extends BaseResponse {
  type: "response:loadTBSuite";
  data?: TBSuiteInfo;
}

/**
 * Response to StartTBRunRequest
 */
export interface StartTBRunResponse extends BaseResponse {
  type: "response:startTBRun";
  data?: {
    runId: string;
  };
}

/**
 * Response to StopTBRunRequest
 */
export interface StopTBRunResponse extends BaseResponse {
  type: "response:stopTBRun";
  data?: {
    stopped: boolean;
  };
}

/**
 * Response to LoadRecentTBRunsRequest
 */
export interface LoadRecentTBRunsResponse extends BaseResponse {
  type: "response:loadRecentTBRuns";
  data?: TBRunHistoryItem[];
}

/**
 * Response to LoadTBRunDetailsRequest
 */
export interface LoadTBRunDetailsResponse extends BaseResponse {
  type: "response:loadTBRunDetails";
  data?: TBRunDetails | null;
}

/**
 * MechaCoder task item (simplified Task for UI)
 */
export interface MCTask {
  id: string;
  title: string;
  description: string;
  status: string;
  priority: number;
  type: string;
  labels: string[];
  createdAt: string;
  updatedAt: string;
}

/**
 * Response to LoadReadyTasksRequest
 */
export interface LoadReadyTasksResponse extends BaseResponse {
  type: "response:loadReadyTasks";
  data?: MCTask[];
}

/**
 * Response to AssignTaskToMCRequest
 */
export interface AssignTaskToMCResponse extends BaseResponse {
  type: "response:assignTaskToMC";
  data?: { assigned: boolean };
}

/**
 * Unified trajectory item (TB run or ATIF trajectory)
 */
export interface UnifiedTrajectory {
  /** Run ID or session ID */
  id: string;
  /** Source type */
  type: "tb-run" | "atif";
  /** ISO 8601 timestamp */
  timestamp: string;
  /** Display label (e.g., "TB: 85% (34/40)" or "MC: 45 steps") */
  label: string;

  // TB-specific (optional)
  suiteName?: string;
  passRate?: number;
  passed?: number;
  failed?: number;
  taskCount?: number;

  // ATIF-specific (optional)
  agentName?: string;
  totalSteps?: number;
  modelName?: string;
}

/**
 * Response to LoadUnifiedTrajectoriesRequest
 */
export interface LoadUnifiedTrajectoriesResponse extends BaseResponse {
  type: "response:loadUnifiedTrajectories";
  data?: UnifiedTrajectory[];
}

/**
 * Response to GetHFTrajectoryCountRequest
 */
export interface GetHFTrajectoryCountResponse extends BaseResponse {
  type: "response:getHFTrajectoryCount";
  data?: { count: number };
}

/**
 * Response to GetHFTrajectoriesRequest
 * Note: Trajectory type from @openagents/atif
 */
export interface GetHFTrajectoriesResponse extends BaseResponse {
  type: "response:getHFTrajectories";
  data?: unknown[]; // Trajectory[] from atif/schema.js
}

/**
 * Response to GetHFTrajectoryRequest
 */
export interface GetHFTrajectoryResponse extends BaseResponse {
  type: "response:getHFTrajectory";
  data?: unknown | null; // Trajectory from atif/schema.js
}

/**
 * Union of all response types
 */
export type SocketResponse =
  | LoadTBSuiteResponse
  | StartTBRunResponse
  | StopTBRunResponse
  | LoadRecentTBRunsResponse
  | LoadTBRunDetailsResponse
  | LoadReadyTasksResponse
  | AssignTaskToMCResponse
  | LoadUnifiedTrajectoriesResponse
  | GetHFTrajectoryCountResponse
  | GetHFTrajectoriesResponse
  | GetHFTrajectoryResponse;

// ============================================================================
// Unified Socket Message Type
// ============================================================================

/**
 * All messages that can flow over the socket
 *
 * - HudMessage: Events from server (agents) to client (UI)
 * - SocketRequest: Requests from client (UI) to server
 * - SocketResponse: Responses from server to client
 */
export type SocketMessage = HudMessage | SocketRequest | SocketResponse;

// ============================================================================
// Type Guards
// ============================================================================

/**
 * Check if a message is a request
 */
export const isSocketRequest = (msg: unknown): msg is SocketRequest => {
  if (typeof msg !== "object" || msg === null) return false;
  const obj = msg as Record<string, unknown>;
  return typeof obj.type === "string" && obj.type.startsWith("request:");
};

/**
 * Check if a message is a response
 */
export const isSocketResponse = (msg: unknown): msg is SocketResponse => {
  if (typeof msg !== "object" || msg === null) return false;
  const obj = msg as Record<string, unknown>;
  return typeof obj.type === "string" && obj.type.startsWith("response:");
};

/**
 * Check if a message is a HUD event (not request/response)
 */
export const isHudEvent = (msg: unknown): msg is HudMessage => {
  if (typeof msg !== "object" || msg === null) return false;
  const obj = msg as Record<string, unknown>;
  if (typeof obj.type !== "string") return false;
  return !obj.type.startsWith("request:") && !obj.type.startsWith("response:");
};

// ============================================================================
// Request Type Guards
// ============================================================================

export const isLoadTBSuiteRequest = (msg: SocketRequest): msg is LoadTBSuiteRequest =>
  msg.type === "request:loadTBSuite";

export const isStartTBRunRequest = (msg: SocketRequest): msg is StartTBRunRequest =>
  msg.type === "request:startTBRun";

export const isStopTBRunRequest = (msg: SocketRequest): msg is StopTBRunRequest =>
  msg.type === "request:stopTBRun";

export const isLoadRecentTBRunsRequest = (msg: SocketRequest): msg is LoadRecentTBRunsRequest =>
  msg.type === "request:loadRecentTBRuns";

export const isLoadTBRunDetailsRequest = (msg: SocketRequest): msg is LoadTBRunDetailsRequest =>
  msg.type === "request:loadTBRunDetails";

export const isLoadReadyTasksRequest = (msg: SocketRequest): msg is LoadReadyTasksRequest =>
  msg.type === "request:loadReadyTasks";

export const isAssignTaskToMCRequest = (msg: SocketRequest): msg is AssignTaskToMCRequest =>
  msg.type === "request:assignTaskToMC";

export const isLoadUnifiedTrajectoriesRequest = (msg: SocketRequest): msg is LoadUnifiedTrajectoriesRequest =>
  msg.type === "request:loadUnifiedTrajectories";

export const isGetHFTrajectoryCountRequest = (msg: SocketRequest): msg is GetHFTrajectoryCountRequest =>
  msg.type === "request:getHFTrajectoryCount";

export const isGetHFTrajectoriesRequest = (msg: SocketRequest): msg is GetHFTrajectoriesRequest =>
  msg.type === "request:getHFTrajectories";

export const isGetHFTrajectoryRequest = (msg: SocketRequest): msg is GetHFTrajectoryRequest =>
  msg.type === "request:getHFTrajectory";

// ============================================================================
// Serialization Helpers
// ============================================================================

export const serializeSocketMessage = (msg: SocketMessage): string =>
  JSON.stringify(msg);

export const parseSocketMessage = (data: string): SocketMessage | null => {
  try {
    const parsed = JSON.parse(data);
    if (typeof parsed !== "object" || parsed === null) return null;
    if (typeof parsed.type !== "string") return null;
    return parsed as SocketMessage;
  } catch {
    return null;
  }
};

// ============================================================================
// Request/Response Helpers
// ============================================================================

/**
 * Create a request message with auto-generated correlation ID
 */
export function createRequest<T extends SocketRequest["type"]>(
  type: T,
  params: Omit<Extract<SocketRequest, { type: T }>, "type" | "correlationId">
): Extract<SocketRequest, { type: T }> {
  return {
    type,
    correlationId: generateCorrelationId(),
    ...params,
  } as Extract<SocketRequest, { type: T }>;
}

/**
 * Create a success response
 */
export function createSuccessResponse<T extends SocketResponse["type"]>(
  type: T,
  correlationId: string,
  data: Extract<SocketResponse, { type: T }>["data"]
): Extract<SocketResponse, { type: T }> {
  return {
    type,
    correlationId,
    success: true,
    data,
  } as Extract<SocketResponse, { type: T }>;
}

/**
 * Create an error response
 */
export function createErrorResponse<T extends SocketResponse["type"]>(
  type: T,
  correlationId: string,
  error: string
): Extract<SocketResponse, { type: T }> {
  return {
    type,
    correlationId,
    success: false,
    error,
  } as Extract<SocketResponse, { type: T }>;
}

// ============================================================================
// Protocol Constants
// ============================================================================

/** HTTP port for static file serving */
export const DESKTOP_HTTP_PORT = 8080;

/** WebSocket path on the HTTP server */
export const DESKTOP_WS_PATH = "/ws";

/** Full WebSocket URL for desktop */
export const DESKTOP_WS_URL = `ws://localhost:${DESKTOP_HTTP_PORT}${DESKTOP_WS_PATH}`;
