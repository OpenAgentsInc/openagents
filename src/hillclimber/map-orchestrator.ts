/**
 * MAP Orchestrator for HillClimber
 *
 * Modular Agentic Planner architecture inspired by Nature 2025 paper.
 * Coordinates: Task Decomposer, Monitor, Evaluator, and FM Actor.
 *
 * Key difference from legacy orchestrator:
 * - Runs verification DURING execution, not just at end
 * - Provides detailed failure feedback to FM
 * - Supports subtask-based execution with checkpoints
 * - Implements feedback loop: Action → Execute → Evaluate → Adjust
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { readdirSync, existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import type { TerminalBenchTask } from "../bench/terminal-bench.js";
import type { HillClimberConfig } from "./types.js";
import { decomposeTask, type Subtask, type TaskDecomposition } from "./decomposer.js";
import { monitorAction, createActionSignature, type ActionContext } from "./monitor.js";
import { evaluateProgress, evaluateProgressWithDocker, quickEvaluate, formatForPrompt, type EvaluatorResult } from "./evaluator.js";
import { FMService, FMServiceLive } from "../fm/service.js";
import { parseToolCalls } from "../bench/model-adapter.js";
import { runTestGenForTask } from "./testgen-integration.js";
import { runParallelSampling } from "./sampling-orchestrator.js";

// ============================================================================
// Types
// ============================================================================

export interface MAPOrchestratorOptions {
  /** Working directory for task execution */
  workspace: string;
  /** Timeout in seconds */
  timeout: number;
  /** Maximum turns allowed */
  maxTurns: number;
  /** Task description for FM */
  taskDescription: string;
  /** Callback for output streaming */
  onOutput?: (text: string) => void;
  /** Enable detailed logging */
  verbose?: boolean;
  /** HUD emitter for real-time UI updates */
  hudEmitter?: import("./hud-emitter.js").HillClimberHudEmitter;
}

export interface MAPOrchestratorResult {
  /** Whether task passed verification */
  passed: boolean;
  /** Total turns used */
  turns: number;
  /** Duration in milliseconds */
  durationMs: number;
  /** Progress score (0-1) */
  progress: number;
  /** Output log */
  output: string;
  /** Error message if failed */
  error?: string;
  /** Subtask completion status */
  subtaskStatus: SubtaskStatus[];
  /** Final evaluator result */
  evaluation?: EvaluatorResult;
}

export interface SubtaskStatus {
  subtaskId: number;
  name: string;
  status: "pending" | "in_progress" | "completed" | "failed";
  turnsUsed: number;
  progress: number;
  checkpointPassed: boolean;
}

export interface ExecutionState {
  /** Current subtask index */
  currentSubtask: number;
  /** Total turns used */
  totalTurns: number;
  /** Turns used in current subtask */
  subtaskTurns: number;
  /** Files modified in this session */
  modifiedFiles: string[];
  /** Previous action signatures for repetition detection */
  previousActions: string[];
  /** Last evaluation result */
  lastEvaluation: EvaluatorResult | null;
  /** Best progress seen */
  bestProgress: number;
  /** Turns since progress improved */
  turnsSinceImprovement: number;
  /** Subtask status tracking */
  subtaskStatus: SubtaskStatus[];
  /** Output log */
  output: string;
  /** Monitor warning from last action (e.g., "Regex might be too simple") */
  monitorWarning?: string;
}

// ============================================================================
// State Management
// ============================================================================

function createInitialState(decomposition: TaskDecomposition): ExecutionState {
  return {
    currentSubtask: 0,
    totalTurns: 0,
    subtaskTurns: 0,
    modifiedFiles: [],
    previousActions: [],
    lastEvaluation: null,
    bestProgress: 0,
    turnsSinceImprovement: 0,
    subtaskStatus: decomposition.subtasks.map((st) => ({
      subtaskId: st.id,
      name: st.name,
      status: "pending" as const,
      turnsUsed: 0,
      progress: 0,
      checkpointPassed: false,
    })),
    output: "",
  };
}

// ============================================================================
// FM Interface (Placeholder - will integrate with actual FM)
// ============================================================================

interface FMAction {
  toolName: string;
  toolArgs: Record<string, unknown>;
  reasoning?: string;
}

interface FMContext {
  taskDescription: string;
  currentSubtask: Subtask;
  previousActions: string[];
  verificationFeedback?: string;
  hints: string[];
  globalHints: string[];
  /** Current contents of modified files (passed between subtasks) */
  fileContents?: Record<string, string>;
}

/**
 * Build context for FM based on current state.
 * This is what gets injected into the FM prompt.
 *
 * IMPORTANT: Reads modified files from workspace to pass context between subtasks.
 * This prevents context loss when FM advances from one subtask to another.
 */
function buildFMContext(
  task: TerminalBenchTask,
  decomposition: TaskDecomposition,
  state: ExecutionState,
  workspacePath: string,
): FMContext {
  const currentSubtask = decomposition.subtasks[state.currentSubtask];

  // Build verification feedback from last evaluation
  let verificationFeedback: string | undefined;
  if (state.lastEvaluation) {
    verificationFeedback = formatForPrompt(state.lastEvaluation);
  }

  // Read modified files to pass context forward between subtasks
  // This is CRITICAL: without this, FM loses context when advancing subtasks
  const fileContents: Record<string, string> = {};
  for (const filePath of state.modifiedFiles) {
    // Normalize /app/ paths to workspace (TerminalBench convention)
    let normalizedPath = filePath;
    if (filePath.startsWith("/app/")) {
      normalizedPath = filePath.replace("/app/", "");
    } else if (filePath.startsWith("/app")) {
      normalizedPath = filePath.replace("/app", "");
    }

    const fullPath = resolve(workspacePath, normalizedPath);
    if (existsSync(fullPath)) {
      try {
        const content = readFileSync(fullPath, "utf-8");
        // Use /app/ prefix for consistency with FM's view of the workspace
        const appPath = filePath.startsWith("/app") ? filePath : `/app/${normalizedPath}`;
        fileContents[appPath] = content;
      } catch {
        // Skip unreadable files
      }
    }
  }

  // Combine subtask hints with decomposition strategy hints
  const hints = [
    ...currentSubtask.hints,
    ...(state.turnsSinceImprovement > 3 ? ["Try a different approach - current strategy not improving"] : []),
    ...(state.lastEvaluation && state.lastEvaluation.suggestion ? [state.lastEvaluation.suggestion] : []),
    // Add monitor warning if present (e.g., "Regex might be too simple. Need lookahead...")
    ...(state.monitorWarning ? [`⚠️ ${state.monitorWarning}`] : []),
  ];

  const result: FMContext = {
    taskDescription: task.description,
    currentSubtask,
    previousActions: state.previousActions.slice(-3), // Last 3 actions
    hints,
    globalHints: decomposition.globalHints,
  };

  if (verificationFeedback !== undefined) {
    result.verificationFeedback = verificationFeedback;
  }

  // Add file contents if any files were modified
  if (Object.keys(fileContents).length > 0) {
    result.fileContents = fileContents;
  }

  return result;
}

/**
 * Format FM context as a prompt string for injection.
 * Optimized for ~3000 token limit.
 */
export function formatFMPrompt(context: FMContext): string {
  const lines: string[] = [];

  // System instruction (compressed)
  lines.push(`You are solving a coding task. Use tools to complete it.`);
  lines.push("");

  // Task description (ALWAYS include full - it's already in the prompt, no need to read files)
  lines.push(`## Task`);
  lines.push(context.taskDescription);
  lines.push("");
  lines.push(`⚠️ IMPORTANT: The task description is provided above. DO NOT try to read task.md or any other file for the task description.`);
  lines.push(`The task description is already in this prompt. Start working on the task directly.`);
  lines.push("");

  // Current subtask (most important)
  lines.push(`## Current Goal`);
  lines.push(`${context.currentSubtask.goal}`);
  lines.push("");

  // Checkpoint to reach
  lines.push(`## Success Checkpoint`);
  lines.push(`${context.currentSubtask.checkpoint}`);
  lines.push("");

  // Explicit action guidance based on subtask
  if (context.currentSubtask.name === "write-initial-regex" || context.currentSubtask.name === "write-ipv4-aware-regex") {
    lines.push(`⚠️ ACTION REQUIRED: Write the regex file now. Do NOT read files first.`);
    lines.push(`Use write_file to create /app/regex.txt with your regex pattern.`);
    lines.push(``);
    lines.push(`IMPORTANT: Write ONLY the plain regex pattern to the file.`);
    lines.push(`- Do NOT include r"..." or r'...' Python string wrappers`);
    lines.push(`- Do NOT include quotes around the pattern`);
    lines.push(`- Example correct content: \\d{4}-\\d{2}-\\d{2}`);
    lines.push(`- Example WRONG: r"\\d{4}-\\d{2}-\\d{2}" or "\\d{4}-\\d{2}-\\d{2}"`);
    lines.push("");
  } else if (context.currentSubtask.name === "test-and-iterate") {
    lines.push(`⚠️ ACTION REQUIRED: Call verify_progress to see test results.`);
    lines.push(`After seeing results, fix the regex if needed.`);
    lines.push("");
  }

  // Verification feedback (CRITICAL - specific failure details)
  if (context.verificationFeedback) {
    lines.push(`## Verification Status`);
    lines.push(context.verificationFeedback);
    lines.push("");
  }

  // Current file contents (CRITICAL for subtask transitions - prevents context loss)
  if (context.fileContents && Object.keys(context.fileContents).length > 0) {
    lines.push(`## Current File Contents`);
    lines.push(`These files were created/modified. BUILD ON these - do NOT start from scratch.`);
    for (const [filePath, content] of Object.entries(context.fileContents)) {
      lines.push(`### ${filePath}`);
      lines.push("```");
      lines.push(content.trim());
      lines.push("```");
    }
    lines.push("");
  }

  // Hints (prioritize current subtask hints)
  if (context.hints.length > 0) {
    lines.push(`## Hints`);
    for (const hint of context.hints.slice(0, 3)) { // Limit to 3 most relevant
      lines.push(`- ${hint}`);
    }
    lines.push("");
  }

  // Previous actions (compressed - last 2 only)
  if (context.previousActions.length > 0) {
    lines.push(`## Recent Actions`);
    for (const action of context.previousActions.slice(-2)) {
      lines.push(`- ${action}`);
    }
    lines.push("");
  }

  // Available tools
  lines.push(`## Available Tools`);
  lines.push(`- read_file(path): Read a file`);
  lines.push(`- write_file(path, content): Write to a file`);
  lines.push(`- edit_file(path, old_text, new_text): Replace text in a file`);
  lines.push(`- run_command(command): Execute a shell command`);
  lines.push(`- verify_progress: Check test results (IMPORTANT: Call this after writing files to see if tests pass)`);
  lines.push(`- task_complete: Signal task is done`);
  lines.push("");
  lines.push(`## CRITICAL: Tool Call Format`);
  lines.push(`You MUST respond with ONLY a tool call in this exact format:`);
  lines.push(`<tool_call>{"name":"TOOL_NAME","arguments":{"key":"value"}}</tool_call>`);
  lines.push(``);
  lines.push(`Examples:`);
  lines.push(`<tool_call>{"name":"read_file","arguments":{"path":"regex.txt"}}</tool_call>`);
  lines.push(`<tool_call>{"name":"write_file","arguments":{"path":"regex.txt","content":"pattern"}}</tool_call>`);
  lines.push(`<tool_call>{"name":"verify_progress","arguments":{}}</tool_call>`);
  lines.push(``);
  lines.push(`IMPORTANT: After writing or editing files, call verify_progress to see test results.`);
  lines.push(`DO NOT write explanations, code blocks, or any other text. ONLY the tool call.`);

  return lines.join("\n");
}

// ============================================================================
// Action Execution
// ============================================================================

/**
 * Execute a tool action in the workspace.
 * This is a simplified version - full implementation will use FM tools.
 */
async function executeAction(
  action: FMAction,
  workspace: string,
  log: (text: string) => void,
): Promise<{ success: boolean; output: string; modifiedFile?: string }> {
  const { toolName, toolArgs } = action;

  log(`[MAP] Executing: ${toolName}`);

  try {
    switch (toolName) {
      case "write_file": {
        const path = toolArgs.path as string || toolArgs.file_path as string;
        if (!path) {
        return {
          success: false,
          output: `Missing path argument for write_file`,
        };
        }
        const content = String(toolArgs.content || "");

        // Normalize /app/ paths to workspace (TerminalBench convention)
        let normalizedPath = path;
        if (path.startsWith("/app/")) {
          normalizedPath = path.replace("/app/", "");
        } else if (path.startsWith("/app")) {
          normalizedPath = path.replace("/app", "");
        }

        const fullPath = resolve(workspace, normalizedPath);
        const dir = dirname(fullPath);

        if (!existsSync(dir)) {
          mkdirSync(dir, { recursive: true });
        }
        writeFileSync(fullPath, content);

        return {
          success: true,
          output: `Wrote ${content.length} bytes to ${normalizedPath}`,
          modifiedFile: normalizedPath,
        };
      }

      case "read_file": {
        const path = toolArgs.path as string || toolArgs.file_path as string;

        // Normalize /app/ paths to workspace (TerminalBench convention)
        let normalizedPath = path;
        if (path.startsWith("/app/")) {
          normalizedPath = path.replace("/app/", "");
        } else if (path.startsWith("/app")) {
          normalizedPath = path.replace("/app", "");
        }

        const fullPath = resolve(workspace, normalizedPath);

        if (!existsSync(fullPath)) {
          // List available files to help FM
          const files = readdirSync(workspace, { withFileTypes: true });
          const fileList = files
            .filter(f => f.isFile())
            .map(f => f.name)
            .join(", ");
          return {
            success: false,
            output: `File not found: ${path}. Available files in workspace: ${fileList || "none"}`,
          };
        }
        const content = readFileSync(fullPath, "utf-8");
        return {
          success: true,
          output: content.slice(0, 1000) + (content.length > 1000 ? "..." : ""),
        };
      }

      case "run_command": {
        const command = toolArgs.command as string || "";
        const proc = Bun.spawn(["sh", "-c", command], {
          cwd: workspace,
          stdout: "pipe",
          stderr: "pipe",
        });
        const exitCode = await proc.exited;
        const stdout = await new Response(proc.stdout).text();
        const stderr = await new Response(proc.stderr).text();

        return {
          success: exitCode === 0,
          output: stdout + (stderr ? `\nSTDERR:\n${stderr}` : ""),
        };
      }

      case "verify_progress": {
        // Special tool - triggers evaluation
        return {
          success: true,
          output: "VERIFY_PROGRESS_REQUESTED",
        };
      }

      case "task_complete": {
        return {
          success: true,
          output: "TASK_COMPLETE",
        };
      }

      default:
        return {
          success: false,
          output: `Unknown tool: ${toolName}`,
        };
    }
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    return {
      success: false,
      output: `Tool error: ${errMsg}`,
    };
  }
}

// ============================================================================
// Feedback Loop
// ============================================================================

/**
 * Decide whether to continue current subtask, advance, or backtrack.
 */
function decideNextStep(
  state: ExecutionState,
  decomposition: TaskDecomposition,
  evaluation: EvaluatorResult | null,
): "continue" | "advance" | "backtrack" | "complete" | "no_progress" {
  const currentSubtask = decomposition.subtasks[state.currentSubtask];

  // Task complete?
  if (evaluation?.passed) {
    return "complete";
  }

  // Check if current subtask reached its checkpoint
  if (evaluation && evaluation.progress >= 0.5 && state.subtaskTurns >= 2) {
    // Made progress - consider advancing
    if (state.currentSubtask < decomposition.subtasks.length - 1) {
      // More subtasks to do - check if we have the expected artifacts
      const hasArtifacts = currentSubtask.expectedArtifacts.length === 0 ||
        currentSubtask.expectedArtifacts.some((a) => state.modifiedFiles.includes(a));

      if (hasArtifacts) {
        return "advance";
      }
    }
  }

  // No progress detection
  if (state.turnsSinceImprovement > 5) {
    return "no_progress";
  }

  // Max turns for subtask
  if (state.subtaskTurns >= currentSubtask.maxTurns) {
    if (state.currentSubtask < decomposition.subtasks.length - 1) {
      return "advance"; // Force advance even if not complete
    }
    return "no_progress";
  }

  return "continue";
}

/**
 * Update state after evaluation.
 */
function updateStateWithEvaluation(
  state: ExecutionState,
  evaluation: EvaluatorResult,
): void {
  state.lastEvaluation = evaluation;

  if (evaluation.progress > state.bestProgress) {
    state.bestProgress = evaluation.progress;
    state.turnsSinceImprovement = 0;
  } else {
    state.turnsSinceImprovement++;
  }

  // Update subtask status
  if (state.currentSubtask < state.subtaskStatus.length) {
    state.subtaskStatus[state.currentSubtask].progress = evaluation.progress;
    state.subtaskStatus[state.currentSubtask].turnsUsed = state.subtaskTurns;
  }
}

/**
 * Advance to next subtask.
 */
function advanceSubtask(state: ExecutionState): void {
  // Mark current as completed
  if (state.currentSubtask < state.subtaskStatus.length) {
    state.subtaskStatus[state.currentSubtask].status = "completed";
    state.subtaskStatus[state.currentSubtask].checkpointPassed = true;
  }

  // Move to next
  state.currentSubtask++;
  state.subtaskTurns = 0;

  // Mark new subtask as in progress
  if (state.currentSubtask < state.subtaskStatus.length) {
    state.subtaskStatus[state.currentSubtask].status = "in_progress";
  }
}

// ============================================================================
// Main Orchestrator
// ============================================================================

/**
 * Run a task using the MAP architecture.
 *
 * Key features:
 * 1. Decomposes task into subtasks with checkpoints
 * 2. Monitors actions before execution
 * 3. Evaluates progress after file modifications
 * 4. Provides detailed feedback to FM
 * 5. Adapts strategy based on progress
 */
export async function runMAPOrchestrator(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  options: MAPOrchestratorOptions,
): Promise<MAPOrchestratorResult> {
  const startTime = Date.now();
  const log = (text: string): void => {
    options.onOutput?.(text + "\n");
  };

  log(`[MAP] Starting MAP orchestrator for task: ${task.id}`);

  // Step 0: Generate comprehensive test suite using testgen
  log(`[MAP] Running testgen to generate comprehensive tests...`);
  try {
    const testgenOptions: { model: "local"; verbose?: boolean } = {
      model: "local",
    };
    if (options.verbose) {
      testgenOptions.verbose = options.verbose;
    }
    const testgenResult = await runTestGenForTask(task, options.workspace, testgenOptions);
    log(`[MAP] Generated ${testgenResult.tests.length} tests (score: ${testgenResult.comprehensivenessScore}/10)`);
    log(`[MAP] Tests written to: ${testgenResult.testFilePath}`);
  } catch (error) {
    log(`[MAP] Warning: TestGen failed: ${error instanceof Error ? error.message : String(error)}`);
    log(`[MAP] Continuing with standard TB2 tests...`);
  }

  // Step 1: Decompose task
  const decomposition = decomposeTask(task);

  if (!decomposition) {
    log(`[MAP] No decomposition found for task: ${task.id}, using generic single-step`);
    // Create a generic single-step decomposition
    const genericDecomposition: TaskDecomposition = {
      taskId: task.id,
      subtaskCount: 1,
      subtasks: [{
        id: 1,
        name: "complete-task",
        goal: task.description,
        checkpoint: "Task passes verification",
        expectedArtifacts: [],
        dependsOn: [],
        hints: [],
        maxTurns: options.maxTurns,
      }],
      globalHints: [],
      filesToRead: [],
      requiredOutputs: [],
    };
    return runMAPOrchestratorWithDecomposition(task, config, options, genericDecomposition, startTime, log);
  }

  log(`[MAP] Decomposed into ${decomposition.subtasks.length} subtasks`);
  for (const st of decomposition.subtasks) {
    log(`[MAP]   ${st.id}. ${st.name}: ${st.goal}`);
  }

  return runMAPOrchestratorWithDecomposition(task, config, options, decomposition, startTime, log);
}

/**
 * Internal function that runs with a known decomposition.
 */
async function runMAPOrchestratorWithDecomposition(
  task: TerminalBenchTask,
  _config: HillClimberConfig,
  options: MAPOrchestratorOptions,
  decomposition: TaskDecomposition,
  startTime: number,
  log: (text: string) => void,
): Promise<MAPOrchestratorResult> {
  // Step 2: Initialize state
  const state = createInitialState(decomposition);
  state.subtaskStatus[0].status = "in_progress";

  // Start heartbeat for visibility (every 30 seconds)
  const heartbeatInterval = setInterval(() => {
    const elapsed = Math.floor((Date.now() - startTime) / 1000);
    const currentSubtask = decomposition.subtasks[state.currentSubtask];
    log(`[HEARTBEAT] Turn ${state.totalTurns}/${options.maxTurns} | Subtask: ${currentSubtask?.name ?? 'unknown'} | Progress: ${(state.bestProgress * 100).toFixed(1)}% | Elapsed: ${elapsed}s`);
    // Emit to HUD for real-time UI updates
    options.hudEmitter?.onHeartbeat(
      state.totalTurns,
      options.maxTurns,
      state.lastEvaluation?.progress ?? 0,
      state.bestProgress,
      elapsed * 1000
    );
  }, 30_000);

  // Helper to clean up heartbeat
  const cleanup = () => clearInterval(heartbeatInterval);

  // Step 3: Main execution loop
  while (state.totalTurns < options.maxTurns) {
    // Check timeout
    if (Date.now() - startTime > options.timeout * 1000) {
      log(`[MAP] Timeout reached`);
      break;
    }

    state.totalTurns++;
    state.subtaskTurns++;

    const currentSubtask = decomposition.subtasks[state.currentSubtask];
    log(`\n[MAP] === Turn ${state.totalTurns} (Subtask ${state.currentSubtask + 1}: ${currentSubtask.name}) ===`);

    // Emit turn start to HUD
    options.hudEmitter?.onTurnStart(state.totalTurns, options.maxTurns, currentSubtask.name);

    // Step 3a: Build FM context with verification feedback
    const fmContext = buildFMContext(task, decomposition, state, options.workspace);
    const promptInjection = formatFMPrompt(fmContext);

    if (options.verbose) {
      log(`[MAP] FM Context:\n${promptInjection}`);
    }

    // Step 3b: Get action from FM with proper error handling
    // Use parallel sampling for write-initial-regex and test-and-iterate subtasks
    const useSampling = currentSubtask.name === "write-initial-regex" || currentSubtask.name === "test-and-iterate";
    const action = useSampling
      ? await getNextActionWithSampling(task, fmContext, options.workspace, log, {
          numSamples: 3,
          currentBestProgress: state.bestProgress,
        })
      : await getNextAction(task, fmContext, options.workspace, log);

    if (!action) {
      // Error was already logged by getNextAction with specific reason
      // Check if we should advance subtask due to repeated failures
      if (state.subtaskTurns > 5 && state.currentSubtask < decomposition.subtasks.length - 1) {
        log(`[MAP] Advancing to next subtask after ${state.subtaskTurns} turns with FM errors`);
        state.currentSubtask++;
        state.subtaskTurns = 0;
      }
      continue;
    }

    // Step 3c: Monitor action before execution
    const monitorCtx: ActionContext = {
      toolName: action.toolName,
      args: action.toolArgs,
      workspace: options.workspace,
      taskId: task.id,
      modifiedFiles: state.modifiedFiles,
      turnNumber: state.totalTurns,
      previousActions: state.previousActions,
    };

    const monitorDecision = monitorAction(monitorCtx);

    // Special case: If trying to read a file that doesn't exist and current subtask is write-initial-regex,
    // provide more specific feedback
    if (!monitorDecision.allowed && action.toolName === "read_file" && currentSubtask.name === "write-initial-regex") {
      const path = action.toolArgs.path as string || action.toolArgs.file_path as string;
      const normalizedPath = path?.replace(/^\/app\//, "").replace(/^\/app/, "") || path;
      const fullPath = resolve(options.workspace, normalizedPath || "");
      if (!existsSync(fullPath)) {
        monitorDecision.suggestion = `The file ${path} doesn't exist yet. Write it using write_file instead of reading it.`;
      }
    }

    if (!monitorDecision.allowed) {
      log(`[MAP] Monitor REJECTED: ${monitorDecision.reason}`);
      if (monitorDecision.suggestion) {
        log(`[MAP] Suggestion: ${monitorDecision.suggestion}`);
        // Add rejection feedback to state so FM sees it next turn
        if (!state.lastEvaluation) {
          state.lastEvaluation = {
            passed: false,
            progress: state.bestProgress,
            testsTotal: 0,
            testsPassing: 0,
            failures: [],
            rawOutput: "",
            durationMs: 0,
            suggestion: `Action rejected: ${monitorDecision.reason}. ${monitorDecision.suggestion}`,
          };
        } else {
          state.lastEvaluation.suggestion = `Action rejected: ${monitorDecision.reason}. ${monitorDecision.suggestion}. ${state.lastEvaluation.suggestion || ""}`;
        }
      }
      // Advance to next subtask if monitor rejects actions repeatedly
      if (state.subtaskTurns > 5 && state.currentSubtask < decomposition.subtasks.length - 1) {
        log(`[MAP] Advancing to next subtask after ${state.subtaskTurns} turns with monitor rejections`);
        state.currentSubtask++;
        state.subtaskTurns = 0;
      }
      continue;
    }

    if (monitorDecision.warning) {
      log(`[MAP] Monitor WARNING: ${monitorDecision.warning}`);
      // Store warning so FM sees it in next turn's context
      state.monitorWarning = monitorDecision.warning;
    }

    // Step 3d: Execute action
    // Emit FM action to HUD
    options.hudEmitter?.onFMAction("tool_call", action.toolName);

    const actionResult = await executeAction(action, options.workspace, log);
    log(`[MAP] Result: ${actionResult.success ? "SUCCESS" : "FAILED"} - ${actionResult.output.slice(0, 100)}`);

    // If action failed, add error to state so FM sees it next turn
    if (!actionResult.success) {
      if (!state.lastEvaluation) {
        state.lastEvaluation = {
          passed: false,
          progress: state.bestProgress,
          testsTotal: 0,
          testsPassing: 0,
          failures: [],
          rawOutput: "",
          durationMs: 0,
          suggestion: `Action failed: ${actionResult.output}`,
        };
      } else {
        state.lastEvaluation.suggestion = `Action failed: ${actionResult.output}. ${state.lastEvaluation.suggestion || ""}`;
      }
    }

    // Track action
    state.previousActions.push(createActionSignature(action.toolName, action.toolArgs));
    if (state.previousActions.length > 10) {
      state.previousActions = state.previousActions.slice(-10);
    }

    if (actionResult.modifiedFile) {
      state.modifiedFiles.push(actionResult.modifiedFile);
    }

    // Step 3e: Handle special actions
    if (actionResult.output === "TASK_COMPLETE") {
      log(`[MAP] FM signaled task complete - running final verification`);
      const finalEval = await quickEvaluate(task, options.workspace);

      if (finalEval.passed) {
        log(`[MAP] TASK PASSED!`);
        return {
          passed: true,
          turns: state.totalTurns,
          durationMs: Date.now() - startTime,
          progress: 1.0,
          output: state.output,
          subtaskStatus: state.subtaskStatus,
        };
      } else {
        log(`[MAP] Verification failed: ${finalEval.message}`);
        // Continue - FM will get feedback
      }
    }

    // Step 3f: Evaluate progress after file modifications
    if (actionResult.modifiedFile || actionResult.output === "VERIFY_PROGRESS_REQUESTED") {
      log(`[MAP] Running verification...`);
      // Emit verify start to HUD
      options.hudEmitter?.onVerifyStart();

      try {
        // Use Docker-based verification for TB2 tasks with source_path
        const useDocker = task.source_path && existsSync(task.source_path) && existsSync(join(task.source_path, "tests"));
        const evaluation = await Effect.runPromise(
          useDocker
            ? evaluateProgressWithDocker(task, options.workspace).pipe(Effect.provide(BunContext.layer))
            : evaluateProgress(task, options.workspace).pipe(Effect.provide(BunContext.layer)),
        );

        log(`[MAP] Progress: ${(evaluation.progress * 100).toFixed(1)}% (${evaluation.testsPassing}/${evaluation.testsTotal} tests)`);
        // Emit verify complete to HUD
        options.hudEmitter?.onVerifyComplete(evaluation.testsPassing, evaluation.testsTotal, evaluation.progress);

        updateStateWithEvaluation(state, evaluation);

        // Step 3g: Decide next step
        const decision = decideNextStep(state, decomposition, evaluation);
        log(`[MAP] Decision: ${decision}`);

        switch (decision) {
          case "complete":
            // Emit run complete to HUD
            options.hudEmitter?.onRunComplete(true, evaluation.progress);
            cleanup();
            return {
              passed: true,
              turns: state.totalTurns,
              durationMs: Date.now() - startTime,
              progress: evaluation.progress,
              output: state.output,
              subtaskStatus: state.subtaskStatus,
              evaluation,
            };

          case "advance":
            log(`[MAP] Advancing to next subtask`);
            // Emit subtask completion to HUD
            options.hudEmitter?.onSubtaskChange(currentSubtask.name, "completed");
            advanceSubtask(state);
            // Emit new subtask start to HUD
            const nextSubtask = decomposition.subtasks[state.currentSubtask];
            if (nextSubtask) {
              options.hudEmitter?.onSubtaskChange(nextSubtask.name, "active");
            }
            break;

          case "no_progress":
            log(`[MAP] No progress detected - trying different approach`);
            // Reset improvement counter but continue
            state.turnsSinceImprovement = 0;
            break;

          case "backtrack":
            log(`[MAP] Backtracking to previous checkpoint`);
            // For now, just reset improvement counter
            state.turnsSinceImprovement = 0;
            break;

          case "continue":
          default:
            // Keep going with current subtask
            break;
        }
      } catch (e) {
        log(`[MAP] Evaluation error: ${e}`);
      }
    }
  }

  // Max turns reached
  log(`[MAP] Max turns (${options.maxTurns}) reached`);

  // Use last evaluation result instead of running quickEvaluate again
  // (quickEvaluate has outdated regex parsing that can report wrong progress)
  const finalProgress = state.lastEvaluation?.progress ?? state.bestProgress;
  const finalPassed = state.lastEvaluation?.passed ?? false;

  // Emit run complete to HUD
  options.hudEmitter?.onRunComplete(finalPassed, finalProgress);

  cleanup();
  return {
    passed: finalPassed,
    turns: state.totalTurns,
    durationMs: Date.now() - startTime,
    progress: finalProgress,
    output: state.output,
    subtaskStatus: state.subtaskStatus,
    error: `Max turns reached with ${(finalProgress * 100).toFixed(1)}% progress`,
  };
}

// ============================================================================
// FM Action Interface
// ============================================================================

/**
 * Extract regex pattern from write_file action content.
 * Handles various formats: quoted strings, raw patterns, etc.
 */
function extractRegexPattern(content: string): string | null {
  let cleaned = content.trim();

  // Handle Python raw string literals: r"pattern" or r'pattern'
  if (cleaned.startsWith('r"') || cleaned.startsWith("r'")) {
    // Remove r" or r' prefix
    cleaned = cleaned.substring(2);
    // Remove trailing quote (matching the opening quote)
    if (cleaned.endsWith('"') || cleaned.endsWith("'")) {
      cleaned = cleaned.substring(0, cleaned.length - 1);
    }
  }
  // Handle regular quoted strings: "pattern" or 'pattern'
  else if ((cleaned.startsWith('"') && cleaned.endsWith('"')) ||
           (cleaned.startsWith("'") && cleaned.endsWith("'"))) {
    cleaned = cleaned.substring(1, cleaned.length - 1);
  }

  // Remove any markdown code block markers
  cleaned = cleaned
    .replace(/^```.*\n/, "")
    .replace(/\n```$/, "")
    .trim();

  return cleaned || null;
}

/**
 * Get next action from FM.
 * Calls the actual FM service with formatted prompt and parses response.
 */
async function getNextAction(
  task: TerminalBenchTask,
  context: FMContext,
  workspace: string,
  log: (text: string) => void,
  temperature: number = 0.3,
): Promise<FMAction | null> {
  try {
    const prompt = formatFMPrompt(context);

    log(`[MAP-FM] Calling FM with prompt (${prompt.length} chars, temp=${temperature.toFixed(2)})`);

    // Call FM service
    const response = await Effect.runPromise(
      Effect.gen(function* () {
        const fm = yield* FMService;
        yield* fm.ensureRunning();

        const chatResponse = yield* fm.chat({
          messages: [{ role: "user", content: prompt }],
          temperature,
          maxTokens: 512, // Short responses for tool calls
        });

        return chatResponse;
      }).pipe(Effect.provide(FMServiceLive))
    );

    const content = response.choices[0]?.message?.content ?? "";

    if (!content) {
      log(`[MAP-FM] Empty response from FM`);
      return null;
    }

    log(`[MAP-FM] FM response: ${content.slice(0, 200)}...`);

    // Clean content - remove markdown code blocks if present
    let cleanedContent = content.trim();
    if (cleanedContent.includes("```")) {
      // Extract content from code blocks
      const codeBlockMatch = cleanedContent.match(/```(?:json|text|python)?\s*([\s\S]*?)```/);
      if (codeBlockMatch) {
        cleanedContent = codeBlockMatch[1].trim();
      }
    }

    // Parse tool calls from response
    const toolCalls = parseToolCalls(cleanedContent);

    if (toolCalls.length === 0) {
      log(`[MAP-FM] No tool call parsed from response. Raw: ${content.slice(0, 300)}`);
      // Try to extract tool name from text if format is different
      const toolMatch = content.match(/(?:tool|action|call)[:\s]+(\w+)/i);
      if (toolMatch) {
        const toolName = toolMatch[1];
        log(`[MAP-FM] Extracted tool name from text: ${toolName}`);
        return {
          toolName,
          toolArgs: {},
          reasoning: content.slice(0, 100),
        };
      }
      // Last resort: if content mentions a tool name, try to use it
      const toolNames = ["read_file", "write_file", "edit_file", "run_command", "verify_progress", "task_complete"];
      for (const toolName of toolNames) {
        if (content.toLowerCase().includes(toolName.toLowerCase())) {
          log(`[MAP-FM] Inferred tool from content: ${toolName}`);
          return {
            toolName,
            toolArgs: {},
            reasoning: content.slice(0, 100),
          };
        }
      }
      return null;
    }

    // When multiple tool calls present, prefer write_file over read_file
    // FM often outputs read_file + write_file together, but we should execute write_file
    let selectedCall = toolCalls[0];
    if (toolCalls.length > 1) {
      // Priority order: write_file > edit_file > verify_progress > run_command > read_file
      const priorityOrder = ["write_file", "edit_file", "verify_progress", "run_command", "read_file", "task_complete"];
      for (const toolName of priorityOrder) {
        const found = toolCalls.find(tc => tc.name === toolName);
        if (found) {
          selectedCall = found;
          if (selectedCall !== toolCalls[0]) {
            log(`[MAP-FM] Selected ${toolName} over ${toolCalls[0].name} (multiple tool calls present)`);
          }
          break;
        }
      }
    }
    log(`[MAP-FM] Parsed tool call: ${selectedCall.name} with args: ${JSON.stringify(selectedCall.arguments)}`);

    return {
      toolName: selectedCall.name,
      toolArgs: selectedCall.arguments,
      reasoning: content.slice(0, 100), // First 100 chars as reasoning
    };
  } catch (error) {
    log(`[MAP-FM] Error calling FM: ${error instanceof Error ? error.message : String(error)}`);
    return null;
  }
}

/**
 * Get next action using parallel sampling (TTC).
 *
 * For write_file actions in critical subtasks (like write-initial-regex),
 * we sample multiple candidates and pick the best based on test progress.
 */
async function getNextActionWithSampling(
  task: TerminalBenchTask,
  context: FMContext,
  workspace: string,
  log: (text: string) => void,
  options: {
    numSamples?: number;
    currentBestProgress?: number;
  } = {}
): Promise<FMAction | null> {
  const { numSamples = 3, currentBestProgress = 0 } = options;

  log(`[MAP-SAMPLING] Using parallel sampling with N=${numSamples}`);

  try {
    // Run parallel sampling
    const result = await runParallelSampling(
      async (variation, temp, index) => {
        // Add variation hint to context
        const samplingContext: FMContext = {
          ...context,
          hints: [...context.hints, variation].filter(h => h),
        };

        // Get action with this variation and temperature
        const action = await getNextAction(task, samplingContext, workspace, log, temp);

        // Extract regex pattern from write_file action
        if (action && action.toolName === "write_file") {
          const content = action.toolArgs.content as string || "";
          return extractRegexPattern(content);
        }

        return null;
      },
      {
        numSamples,
        baseWorkspace: workspace,
        task,
        currentBestProgress,
        solutionFilename: "regex.txt",
      }
    );

    log(`[MAP-SAMPLING] Best candidate: ${result.best.testsPassing}/${result.best.testsTotal} tests`);

    // Return write_file action for best candidate
    if (result.best.solution) {
      return {
        toolName: "write_file",
        toolArgs: {
          path: "/app/regex.txt",
          content: result.best.solution,
        },
        reasoning: `Best of ${numSamples} candidates (${result.best.testsPassing}/${result.best.testsTotal} tests)`,
      };
    }

    return null;
  } catch (error) {
    log(`[MAP-SAMPLING] Error: ${error instanceof Error ? error.message : String(error)}`);
    // Fallback to single sample
    return getNextAction(task, context, workspace, log);
  }
}

// ============================================================================
// Integration with HillClimber
// ============================================================================

/**
 * Run a task using MAP orchestrator (for HillClimber integration).
 * This wraps runMAPOrchestrator with the expected interface.
 */
export async function runTaskWithMAP(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  workspace: string,
  timeout: number,
  maxTurns: number,
  onOutput?: (text: string) => void,
): Promise<{
  passed: boolean;
  turns: number;
  durationMs: number;
  progress: number;
  error?: string;
}> {
  const options: MAPOrchestratorOptions = {
    workspace,
    timeout,
    maxTurns,
    taskDescription: task.description,
    verbose: false,
  };

  if (onOutput !== undefined) {
    options.onOutput = onOutput;
  }

  const result = await runMAPOrchestrator(task, config, options);

  const output: {
    passed: boolean;
    turns: number;
    durationMs: number;
    progress: number;
    error?: string;
  } = {
    passed: result.passed,
    turns: result.turns,
    durationMs: result.durationMs,
    progress: result.progress,
  };

  if (result.error !== undefined) {
    output.error = result.error;
  }

  return output;
}
