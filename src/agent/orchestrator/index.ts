/**
 * Orchestrator/Subagent Architecture
 * 
 * Following Anthropic's "Effective Harnesses for Long-Running Agents" pattern:
 * - Orchestrator: Manages task selection, decomposition, verification, session coordination
 * - Subagent: Minimal coding agent that implements one subtask at a time
 * 
 * @see https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
 * @see docs/mechacoder/PI-MONO-INTEGRATION.md
 * @see docs/mechacoder/GOLDEN-LOOP-v2.md
 */

// Types
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
} from "./types.js";

export {
  SUBAGENT_SYSTEM_PROMPT,
  buildSubagentPrompt,
  getSubtasksPath,
  getProgressPath,
  getInitScriptPath,
} from "./types.js";

// Subagent
export { runSubagent, createSubagentConfig } from "./subagent.js";

// Orchestrator
export { runOrchestrator } from "./orchestrator.js";
