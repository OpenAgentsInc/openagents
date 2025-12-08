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
        return {
          success: true,
          output: content,
          condensed: condenseSummary(`Read ${basename(path)}: ${content.length} chars`),
        };
      }

      case "write_file": {
        const path = normalizePath(toolArgs.path as string ?? toolArgs.p as string);
        const content = toolArgs.content as string ?? toolArgs.c as string ?? "";
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
          return {
            success: true,
            output: `Exit 0\n${output}`,
            condensed: condenseSummary(`Command succeeded`),
          };
        } else {
          return {
            success: false,
            output: `Exit ${exitCode}\n${output}`,
            condensed: condenseError(stderr || output || `Exit ${exitCode}`),
          };
        }
      }

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

  const log = (text: string): void => {
    outputText += text + "\n";
    options.onOutput?.(text + "\n");
  };

  log(`[Orchestrator] Starting plan with ${plan.steps.length} steps`);

  const timeoutMs = options.timeout * 1000;
  const maxTurns = options.maxTurns;

  let stepIndex = 0;
  while (stepIndex < plan.steps.length) {
    if (Date.now() - state.startTime > timeoutMs) {
      log(`[Orchestrator] Timeout reached`);
      return {
        success: false,
        turns,
        tokens: state.totalTokens,
        durationMs: Date.now() - state.startTime,
        output: outputText,
        error: "Task timed out",
      };
    }

    if (turns >= maxTurns) {
      log(`[Orchestrator] Max turns reached`);
      return {
        success: false,
        turns,
        tokens: state.totalTokens,
        durationMs: Date.now() - state.startTime,
        output: outputText,
        error: `Reached max turns (${maxTurns})`,
      };
    }

    const step = plan.steps[stepIndex];
    step.status = "in_progress";
    state.currentStep = step.id;
    turns++;

    log(`\n--- Step ${step.id}: ${step.action} ---`);

    const workerInput = buildWorkerInput(step, state);
    log(`[Worker] Input: action="${workerInput.action}", context="${workerInput.context}", previous="${workerInput.previous}"`);

    try {
      const workerOutput = await callFMWorker(client, workerInput);
      log(`[Worker] Output: tool=${workerOutput.toolName}, args=${JSON.stringify(workerOutput.toolArgs)}`);

      if (!workerOutput.toolName) {
        log(`[Worker] No tool call parsed, raw: ${workerOutput.raw.slice(0, 100)}`);
        step.status = "failed";
        step.errorSummary = "No tool call";
        state.history.push("Step failed: no tool call");

        const fixStep = createFixStep(step, {
          success: false,
          output: "No tool call",
          condensed: "No tool call",
        });
        plan.steps.splice(stepIndex + 1, 0, fixStep);
        stepIndex++;
        continue;
      }

      const result = await executeTool(
        workerOutput.toolName,
        workerOutput.toolArgs,
        options.workspace,
      );
      log(`[Tool] ${workerOutput.toolName}: ${result.success ? "success" : "failed"} - ${result.condensed}`);

      updateStateFromResult(state, step, result);

      if (!result.success && stepIndex < plan.steps.length - 1) {
        const fixStep = createFixStep(step, result);
        plan.steps.splice(stepIndex + 1, 0, fixStep);
        log(`[Orchestrator] Inserted fix step for error: ${result.condensed}`);
      }

      stepIndex++;
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      log(`[Orchestrator] Error: ${errMsg}`);
      step.status = "failed";
      step.errorSummary = condenseError(errMsg);
      state.history.push(`Error: ${step.errorSummary}`);
      stepIndex++;
    }
  }

  const allDone = plan.steps.every((s) => s.status === "done");
  log(`\n[Orchestrator] Completed. Success: ${allDone}`);

  const result: OrchestratorResult = {
    success: allDone,
    turns,
    tokens: state.totalTokens,
    durationMs: Date.now() - state.startTime,
    output: outputText,
  };

  if (!allDone) {
    result.error = "Some steps failed";
  }

  return result;
}
