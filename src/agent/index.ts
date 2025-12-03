export { agentLoop, AgentLoopError } from "./loop.js";
export type { AgentConfig, AgentTurn, AgentResult } from "./loop.js";
export { GIT_CONVENTIONS, BASE_SYSTEM_PROMPT, buildSystemPrompt } from "./prompts.js";
export {
  createSession,
  loadSession,
  listSessions,
  getSessionPath,
  writeSessionStart,
  writeUserMessage,
  writeTurn,
  writeMessage,
  writeSessionEnd,
  SessionError,
} from "./session.js";
export type { Session } from "./session.js";
export type { AgentTransport, AgentRunConfig, AgentEvent, QueuedMessage } from "./transport.js";
export { createProviderTransport } from "./transport.js";

// Orchestrator/Subagent Architecture
export {
  runOrchestrator,
  runSubagent,
  createSubagentConfig,
  SUBAGENT_SYSTEM_PROMPT,
  buildSubagentPrompt,
  getSubtasksPath,
  getProgressPath,
  getInitScriptPath,
} from "./orchestrator/index.js";
export type {
  Subtask,
  SubtaskStatus,
  SubtaskList,
  SessionProgress,
  SubagentConfig,
  SubagentResult,
  OrchestratorConfig,
  OrchestratorState,
  OrchestratorPhase,
  OrchestratorEvent,
} from "./orchestrator/index.js";
