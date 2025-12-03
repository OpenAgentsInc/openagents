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

// Decomposition
export {
  analyzeTask,
  decomposeTask,
  decomposeByRules,
  generateSubtaskId,
  readSubtasks,
  writeSubtasks,
  updateSubtaskStatus,
  createSubtaskList,
  getPendingSubtasks,
  getNextSubtask,
  isAllSubtasksComplete,
  hasFailedSubtasks,
} from "./decompose.js";
export type { DecompositionHeuristics } from "./decompose.js";

// Progress Files
export {
  writeProgress,
  readProgress,
  formatProgressMarkdown,
  parseProgressMarkdown,
  progressExists,
  getPreviousSessionSummary,
  createEmptyProgress,
} from "./progress.js";

// Orchestrator
export { runOrchestrator } from "./orchestrator.js";
