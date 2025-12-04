/**
 * ATIF (Agent Trajectory Interchange Format) Module
 *
 * Provides complete support for ATIF v1.4 trajectory capture and storage:
 * - Schema definitions
 * - Validation
 * - Collection during agent execution
 * - Persistent storage
 * - Event adapters
 *
 * @module atif
 */

// Schema exports
export {
  // Version constant
  ATIF_SCHEMA_VERSION,
  // Schema types
  type Agent,
  type FinalMetrics,
  type Metrics,
  type Observation,
  type ObservationResult,
  type Step,
  type StepSource,
  type SubagentTrajectoryRef,
  type ToolCall,
  type Trajectory,
  // Schema objects (for validation)
  Agent as AgentSchema,
  FinalMetrics as FinalMetricsSchema,
  Metrics as MetricsSchema,
  Observation as ObservationSchema,
  ObservationResult as ObservationResultSchema,
  Step as StepSchema,
  StepSource as StepSourceSchema,
  SubagentTrajectoryRef as SubagentTrajectoryRefSchema,
  ToolCall as ToolCallSchema,
  Trajectory as TrajectorySchema,
  // Decode/encode helpers
  decodeAgent,
  decodeStep,
  decodeTrajectory,
  encodeAgent,
  encodeStep,
  encodeTrajectory,
  // Type guards and helpers
  extractStepText,
  extractSubagentSessionIds,
  extractToolCallIds,
  generateSessionId,
  generateToolCallId,
  getTotalTokens,
  hasObservation,
  hasSubagentRefs,
  hasToolCalls,
  isAgentStep,
  isSystemStep,
  isUserStep,
  timestamp,
} from "./schema.js";

// Validation exports
export {
  type ValidationErrorReason,
  TrajectoryValidationError,
  collectValidationErrors,
  isValidTrajectory,
  validateTrajectory,
  validateTrajectorySync,
} from "./validation.js";

// Collector exports
export {
  type ActiveTrajectory,
  type TrajectoryCollector,
  StandaloneTrajectoryCollector,
  TrajectoryCollectorError,
  TrajectoryCollectorLive,
  TrajectoryCollectorTag,
} from "./collector.js";

// Service exports
export {
  type TrajectoryMetadata,
  type TrajectoryService,
  type TrajectoryServiceConfig,
  DEFAULT_TRAJECTORIES_DIR,
  TrajectoryServiceError,
  TrajectoryServiceLive,
  TrajectoryServiceTag,
  formatTrajectoryJson,
  makeTrajectoryService,
} from "./service.js";

// Adapter exports
export {
  // Agent factories
  createAgent,
  createClaudeCodeAgent,
  createMechaCoderAgent,
  createMinimalSubagent,
  // Session entry adapters
  assistantMessageEntryToStep,
  sessionEntriesToSteps,
  sessionEntriesToTrajectory,
  toolResultEntryToStep,
  userMessageEntryToStep,
  // Orchestrator event adapters
  orchestratorEventToStep,
  orchestratorEventsToSteps,
  // SubagentResult adapters
  subagentResultToMetrics,
  subagentResultToObservation,
  // Utility functions
  createEmptyTrajectory,
  mergeTrajectories,
} from "./adapter.js";
