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
  InitScriptResult,
  OrchestratorConfig,
  OrchestratorState,
  OrchestratorPhase,
  OrchestratorEvent,
  ClaudeCodeSettings,
  ClaudeCodePermissionMode,
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
export { runBestAvailableSubagent, shouldUseClaudeCode } from "./subagent-router.js";

// Init Script
export { runInitScript } from "./init-script.js";

// Claude Code integration
export { detectClaudeCode } from "./claude-code-detector.js";
export { runClaudeCodeSubagent } from "./claude-code-subagent.js";

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

// Agent Lock
export {
  acquireLock,
  releaseLock,
  checkLock,
  readLock,
  forceRemoveLock,
  createLockGuard,
  getLockPath,
  isPidRunning,
} from "./agent-lock.js";
export type { AgentLock, AcquireLockResult, CheckLockResult } from "./agent-lock.js";

// Sandbox Runner
export {
  runCommand,
  runCommandString,
  runVerificationWithSandbox,
  checkSandboxAvailable,
  buildContainerConfig,
} from "./sandbox-runner.js";
export type {
  SandboxRunnerConfig,
  SandboxRunnerEvent,
  CommandResult,
  VerificationResult,
} from "./sandbox-runner.js";
