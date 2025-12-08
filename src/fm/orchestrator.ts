/**
 * Micro-Task Orchestrator for FM
 *
 * Manages state, executes micro-steps, handles errors.
 * FM never sees state - only the orchestrator does.
 */

import { resolve, dirname, basename, isAbsolute } from "path";
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "fs";
import type {
  MicroPlan,
  MicroStep,
  TaskState,
  ToolResult,
  WorkerInput,
} from "./micro-task-types.js";
import {
  condenseSummary,
  condenseError,
  createMicroStep,
  truncate,
  MAX_ACTION_CHARS,
  MAX_CONTEXT_CHARS,
  MAX_PREVIOUS_CHARS,
} from "./micro-task-types.js";
import { callFMWorker, type FMClientLike } from "./worker.js";

// --- State Management ---

export function createTaskState(plan: MicroPlan, workspace: string): TaskState {
  return {
    plan,
    currentStep: 0,
    files: new Map(),
    workspace,
    history: [],
    startTime: Date.now(),
    totalTokens: 0,
  };
}

// --- Worker Input Building ---

export function buildWorkerInput(step: MicroStep, state: TaskState): WorkerInput {
  const action = truncate(step.action, MAX_ACTION_CHARS);

  let context = "";
  switch (step.kind) {
    case "READ_FILE_RANGE":
      context = `Read ${basename(step.params.path as string)}`;
      break;
    case "WRITE_FILE":
      context = `Create ${basename(step.params.path as string)}`;
      break;
    case "EDIT_FILE":
      context = `Edit ${basename(step.params.path as string)}`;
      break;
    case "COMPILE":
      context = `Compile code`;
      break;
    case "RUN_COMMAND":
      context = `Run command`;
      break;
    case "FIX_ERROR":
      context = step.params.errorSummary as string ?? "Fix error";
      break;
    case "CHECK_OUTPUT":
      context = `Verify output`;
      break;
  }
  context = truncate(context, MAX_CONTEXT_CHARS);

  const previous = state.history.length > 0
    ? truncate(state.history[state.history.length - 1], MAX_PREVIOUS_CHARS)
    : "none";

  return { action, context, previous };
}

// --- Tool Execution ---

export async function executeTool(
  toolName: string,
  toolArgs: Record<string, unknown>,
  workspace: string,
): Promise<ToolResult> {
  const normalizePath = (inputPath: string): string => {
    if (isAbsolute(inputPath)) {
      return resolve(workspace, basename(inputPath));
    }
    return resolve(workspace, inputPath);
  };

  try {
    switch (toolName) {
      case "read_file": {
        const path = normalizePath(toolArgs.path as string ?? toolArgs.p as string);
        if (!existsSync(path)) {
          return {
            success: false,
            output: `File not found: ${path}`,
            condensed: condenseError(`File not found: ${basename(path)}`),
          };
        }
        let content = readFileSync(path, "utf-8");
        const start = toolArgs.start as number ?? toolArgs.s as number;
        const end = toolArgs.end as number ?? toolArgs.e as number;
        if (start !== undefined && end !== undefined) {
          const lines = content.split("\n");
          content = lines.slice(start - 1, end).join("\n");
        }
        // Include actual content in condensed so FM can use it (truncate if long)
        const preview = content.length > 500 ? content.slice(0, 500) + "..." : content;
        return {
          success: true,
          output: content,
          condensed: `${basename(path)} contains: ${preview}`,
        };
      }

      case "write_file": {
        const path = normalizePath(toolArgs.path as string ?? toolArgs.p as string);
        // Convert content to string in case FM returns a number or other type
        const rawContent = toolArgs.content ?? toolArgs.c ?? "";
        const content = String(rawContent);
        const dir = dirname(path);
        if (!existsSync(dir)) {
          mkdirSync(dir, { recursive: true });
        }
        writeFileSync(path, content);
        return {
          success: true,
          output: `Wrote ${content.length} bytes to ${path}`,
          condensed: condenseSummary(`Created ${basename(path)}`),
        };
      }

      case "edit_file": {
        const path = normalizePath(toolArgs.path as string ?? toolArgs.p as string);
        const oldText = toolArgs.old_text as string ?? toolArgs.o as string ?? "";
        const newText = toolArgs.new_text as string ?? toolArgs.n as string ?? "";
        if (!existsSync(path)) {
          return {
            success: false,
            output: `File not found: ${path}`,
            condensed: condenseError(`File not found: ${basename(path)}`),
          };
        }
        let content = readFileSync(path, "utf-8");
        if (!content.includes(oldText)) {
          return {
            success: false,
            output: `Text not found in file`,
            condensed: condenseError(`Text not found in ${basename(path)}`),
          };
        }
        content = content.replace(oldText, newText);
        writeFileSync(path, content);
        return {
          success: true,
          output: `Edited ${path}`,
          condensed: condenseSummary(`Edited ${basename(path)}`),
        };
      }

      case "run_command": {
        const command = toolArgs.command as string ?? toolArgs.c as string ?? "";
        const proc = Bun.spawn(["sh", "-c", command], {
          cwd: workspace,
          stdout: "pipe",
          stderr: "pipe",
        });
        const exitCode = await proc.exited;
        const stdout = await new Response(proc.stdout).text();
        const stderr = await new Response(proc.stderr).text();
        const output = stdout + (stderr ? `\nSTDERR:\n${stderr}` : "");

        if (exitCode === 0) {
          // Include stdout in condensed so FM can use the output (500 char limit)
          const preview = stdout.trim().length > 500 
            ? stdout.trim().slice(0, 500) + "..." 
            : stdout.trim();
          const condensed = preview 
            ? `Command output: ${preview}`
            : "Command succeeded (no output)";
          return {
            success: true,
            output: `Exit 0\n${output}`,
            condensed,
          };
        } else {
          return {
            success: false,
            output: `Exit ${exitCode}\n${output}`,
            condensed: condenseError(stderr || output || `Exit ${exitCode}`),
          };
        }
      }

      case "task_complete":
        return {
          success: true,
          output: "Task marked as complete",
          condensed: "TASK_COMPLETE",
        };

      default:
        return {
          success: false,
          output: `Unknown tool: ${toolName}`,
          condensed: condenseError(`Unknown tool: ${toolName}`),
        };
    }
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    return {
      success: false,
      output: `Tool error: ${errMsg}`,
      condensed: condenseError(errMsg),
    };
  }
}

// --- Step Update ---

export function updateStateFromResult(
  state: TaskState,
  step: MicroStep,
  result: ToolResult,
): void {
  if (result.success) {
    step.status = "done";
    step.resultSummary = result.condensed;
  } else {
    step.status = "failed";
    step.errorSummary = result.condensed;
  }
  state.history.push(result.condensed);
}

// --- Error Recovery ---

export function createFixStep(failedStep: MicroStep, result: ToolResult): MicroStep {
  const errorSummary = result.condensed;
  return createMicroStep(
    failedStep.id + 0.5,
    "FIX_ERROR",
    `Fix: ${errorSummary}`,
    {
      originalStep: failedStep.id,
      errorSummary,
      failedKind: failedStep.kind,
    },
  );
}

// --- Main Orchestrator ---

export interface OrchestratorOptions {
  workspace: string;
  timeout: number;
  maxTurns: number;
  taskDescription?: string | undefined;
  onOutput?: ((text: string) => void) | undefined;
}

export interface OrchestratorResult {
  success: boolean;
  turns: number;
  tokens: number;
  durationMs: number;
  output: string;
  error?: string;
}

export async function runMicroTaskPlan(
  client: FMClientLike,
  plan: MicroPlan,
  options: OrchestratorOptions,
): Promise<OrchestratorResult> {
  const state = createTaskState(plan, options.workspace);
  let outputText = "";
  let turns = 0;
  let consecutiveFailures = 0;
  let hadAnySuccess = false;
  let lastActionSignature = "";
  let repeatCount = 0;
  const MAX_CONSECUTIVE_FAILURES = 3;
  const MAX_REPEAT_ACTIONS = 2; // Stop if same action repeated this many times

  const log = (text: string): void => {
    outputText += text + "\n";
    options.onOutput?.(text + "\n");
  };

  log(`[Orchestrator] Starting dynamic execution for task`);

  const timeoutMs = options.timeout * 1000;
  const maxTurns = options.maxTurns;

  // Simple loop: keep calling FM until we hit max turns or succeed
  while (turns < maxTurns) {
    if (Date.now() - state.startTime > timeoutMs) {
      log(`[Orchestrator] Timeout reached`);
      return {
        success: hadAnySuccess,
        turns,
        tokens: state.totalTokens,
        durationMs: Date.now() - state.startTime,
        output: outputText,
        error: "Task timed out",
      };
    }

    // Stop if we've had too many consecutive failures after initial success
    if (hadAnySuccess && consecutiveFailures >= MAX_CONSECUTIVE_FAILURES) {
      log(`\n[Orchestrator] ${MAX_CONSECUTIVE_FAILURES} consecutive failures after success - task likely complete`);
      return {
        success: true,
        turns,
        tokens: state.totalTokens,
        durationMs: Date.now() - state.startTime,
        output: outputText,
      };
    }

    turns++;
    log(`\n--- Turn ${turns} ---`);

    // Build worker input with context from previous actions (keep last 5 for more context)
    const previous = state.history.length > 0
      ? state.history.slice(-5).join("; ")
      : "none";
    
    const workerInputWithTask = {
      action: "Complete the task using the appropriate tool",
      context: hadAnySuccess ? "Previous action succeeded" : "Start or continue the task",
      previous,
      taskDescription: options.taskDescription,
    };

    log(`[Worker] Previous: ${previous.slice(0, 100)}`);

    try {
      const workerOutput = await callFMWorker(client, workerInputWithTask, log);
      log(`[Worker] Output: tool=${workerOutput.toolName}, args=${JSON.stringify(workerOutput.toolArgs)}`);

      if (!workerOutput.toolName) {
        log(`[Worker] No tool call parsed, raw: ${workerOutput.raw.slice(0, 100)}`);
        state.history.push("No tool call - retrying");
        consecutiveFailures++;
        continue;
      }

      // Check for repeated identical actions (FM doing same thing over and over)
      const actionSignature = `${workerOutput.toolName}:${JSON.stringify(workerOutput.toolArgs)}`;
      if (actionSignature === lastActionSignature) {
        repeatCount++;
        if (repeatCount >= MAX_REPEAT_ACTIONS) {
          log(`\n[Orchestrator] Same action repeated ${repeatCount} times - task complete`);
          return {
            success: hadAnySuccess,
            turns,
            tokens: state.totalTokens,
            durationMs: Date.now() - state.startTime,
            output: outputText,
          };
        }
      } else {
        lastActionSignature = actionSignature;
        repeatCount = 1;
      }

      // Check if FM signaled task complete
      if (workerOutput.toolName === "task_complete") {
        log(`\n[Orchestrator] FM signaled task complete`);
        return {
          success: hadAnySuccess,
          turns,
          tokens: state.totalTokens,
          durationMs: Date.now() - state.startTime,
          output: outputText,
        };
      }

      const result = await executeTool(
        workerOutput.toolName,
        workerOutput.toolArgs,
        options.workspace,
      );
      log(`[Tool] ${workerOutput.toolName}: ${result.success ? "success" : "failed"} - ${result.condensed}`);

      state.history.push(result.condensed);

      if (result.success) {
        hadAnySuccess = true;
        consecutiveFailures = 0;
      } else {
        consecutiveFailures++;
      }

    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      log(`[Orchestrator] Error: ${errMsg}`);
      state.history.push(`Error: ${condenseError(errMsg)}`);
      consecutiveFailures++;
    }
  }

  log(`\n[Orchestrator] Max turns reached`);
  
  const result: OrchestratorResult = {
    success: hadAnySuccess,
    turns,
    tokens: state.totalTokens,
    durationMs: Date.now() - state.startTime,
    output: outputText,
  };

  if (!hadAnySuccess) {
    result.error = `Reached max turns (${maxTurns}) with no success`;
  }

  return result;
}
