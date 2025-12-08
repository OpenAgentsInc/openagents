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
  MAX_HISTORY_ENTRY_CHARS,
} from "./micro-task-types.js";
import { callFMWorker, type FMClientLike } from "./worker.js";
import { buildHint, type SuiteMode } from "./hints.js";
import {
  summarizeToolResult,
  buildPreviousField,
  type StepSummary
} from "./step-summary.js";

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

      case "verify_progress":
        // This tool allows FM to check verification status mid-execution
        // The actual verification will be handled by the orchestrator loop
        return {
          success: true,
          output: "Verification requested",
          condensed: "VERIFY_PROGRESS_REQUESTED",
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
  state.history.push(truncate(result.condensed, MAX_HISTORY_ENTRY_CHARS));
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
  skills?: import("../skills/schema.js").Skill[] | undefined;
  onOutput?: ((text: string) => void) | undefined;
  suiteMode?: SuiteMode;
  verifyTask?: () => Promise<boolean>;
  maxRetryAfterFailedVerify?: number; // Default: 2
}

export interface OrchestratorResult {
  success: boolean;
  turns: number;
  tokens: number;
  durationMs: number;
  output: string;
  error?: string;
}

type CompletionReason =
  | "task_complete"
  | "repeat_same_action"
  | "repeat_failures";

/**
 * Check if task is done. If verifier exists, run it.
 * Returns result if done, undefined if should continue.
 */
async function finalizeIfDone(
  reason: CompletionReason,
  options: OrchestratorOptions,
  state: {
    step: number;
    history: StepSummary[];
    verifyRetryCount: number;
    maxVerifyRetries: number;
    totalTokens: number;
    durationMs: number;
    output: string;
    hadAnySuccess: boolean;
  },
  log: (text: string) => void,
  logParseErrors: () => void
): Promise<OrchestratorResult | undefined> {
  // If no verifier, trust the signal (backward compat for fm-mini)
  if (!options.verifyTask) {
    log(`\n[Orchestrator] FM signaled completion (${reason}) - no verification`);
    logParseErrors();
    return {
      success: state.hadAnySuccess,
      turns: state.step,
      tokens: state.totalTokens,
      durationMs: state.durationMs,
      output: state.output,
    };
  }

  // Run verification
  log(`\n[Orchestrator] ${reason} - running verification`);
  const passed = await options.verifyTask();

  if (passed) {
    log(`\n[Orchestrator] Verification passed`);
    logParseErrors();
    return {
      success: true,
      turns: state.step,
      tokens: state.totalTokens,
      durationMs: state.durationMs,
      output: state.output,
    };
  }

  // Verification failed
  state.verifyRetryCount++;
  log(`\n[Orchestrator] Verification failed (attempt ${state.verifyRetryCount}/${state.maxVerifyRetries})`);

  if (state.verifyRetryCount >= state.maxVerifyRetries) {
    log(`\n[Orchestrator] Verification failed after ${state.verifyRetryCount} attempts`);
    logParseErrors();
    return {
      success: false,
      turns: state.step,
      tokens: state.totalTokens,
      durationMs: state.durationMs,
      output: state.output,
      error: `Verification failed after ${state.verifyRetryCount} attempts`,
    };
  }

  // Add feedback to history and continue
  state.history.push(summarizeToolResult(
    state.step,
    "verification",
    false,
    "Verification failed: output does not meet spec. Fix and try again.",
    {}
  ));

  // Return undefined = not done, keep looping
  return undefined;
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
  const MAX_REPEAT_ACTIONS = 3; // Stop if same action repeated this many times
  // Track tool names for hint system
  const toolHistory: string[] = [];
  // Track step summaries for context management
  const stepHistory: StepSummary[] = [];
  // Track verification retry count
  let verifyRetryCount = 0;
  const maxVerifyRetries = options.maxRetryAfterFailedVerify ?? 2;
  // Track parse errors for metrics
  let parseErrorCount = 0;
  // Track verification feedback for next turn (MAP architecture key feature)
  let lastVerificationFeedback: string | undefined;

  const log = (text: string): void => {
    outputText += text + "\n";
    options.onOutput?.(text + "\n");
  };

  // Helper to log parse errors before returning
  const logParseErrors = (): void => {
    if (parseErrorCount > 0) {
      log(`[Orchestrator] Parse errors in this task: ${parseErrorCount}`);
    }
  };

  log(`[Orchestrator] Starting dynamic execution for task`);

  const timeoutMs = options.timeout * 1000;
  const maxTurns = options.maxTurns;

  // Simple loop: keep calling FM until we hit max turns or succeed
  while (turns < maxTurns) {
    if (Date.now() - state.startTime > timeoutMs) {
      log(`[Orchestrator] Timeout reached`);
      logParseErrors();
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
    // Now goes through verification
    if (hadAnySuccess && consecutiveFailures >= MAX_CONSECUTIVE_FAILURES) {
      const result = await finalizeIfDone(
        "repeat_failures",
        options,
        {
          step: turns,
          history: stepHistory,
          verifyRetryCount,
          maxVerifyRetries,
          totalTokens: state.totalTokens,
          durationMs: Date.now() - state.startTime,
          output: outputText,
          hadAnySuccess,
        },
        log,
        logParseErrors
      );
      if (result) {
        return result;
      }
      // Verification failed but we're retrying - reset counters
      consecutiveFailures = 0;
      repeatCount = 0;
      lastActionSignature = "";
      continue;
    }

    turns++;
    log(`\n--- Turn ${turns} ---`);

    // Build previous field from step summaries (keeps last 3 entries)
    const previous = buildPreviousField(stepHistory);

    // Build hint using tool names (not full outputs)
    const hint = buildHint(
      options.taskDescription ?? "",
      toolHistory,
      options.suiteMode ?? "unknown"
    );

    const workerInputWithTask = {
      action: "Complete the task using the appropriate tool",
      context: hadAnySuccess ? "Previous action succeeded" : "Start or continue the task",
      previous,
      taskDescription: options.taskDescription,
      skills: options.skills,
      hint,
      verificationFeedback: lastVerificationFeedback,
    };

    log(`[Worker] Previous: ${previous}`);

    try {
      const workerOutput = await callFMWorker(client, workerInputWithTask, log);
      log(`[Worker] Output: tool=${workerOutput.toolName}, args=${JSON.stringify(workerOutput.toolArgs)}`);

      if (!workerOutput.toolName) {
        log(`[Worker] No tool call parsed, raw: ${workerOutput.raw.slice(0, 100)}`);
        parseErrorCount++;
        // Add summary for failed tool call parsing
        stepHistory.push({
          step: turns,
          tool: "parse_error",
          success: false,
          message: "No tool call parsed - retrying",
        });
        state.history.push(truncate("No tool call - retrying", MAX_HISTORY_ENTRY_CHARS));
        consecutiveFailures++;
        continue;
      }

      // Check for repeated actions (FM doing same thing over and over)
      // For write/edit, just check tool + path (content may vary)
      const toolPath = workerOutput.toolArgs.path as string ?? workerOutput.toolArgs.p as string ?? "";
      const actionSignature = (workerOutput.toolName === "write_file" || workerOutput.toolName === "edit_file")
        ? `${workerOutput.toolName}:${toolPath}`
        : `${workerOutput.toolName}:${JSON.stringify(workerOutput.toolArgs)}`;

      if (actionSignature === lastActionSignature) {
        repeatCount++;
        // Don't return here - let the verification check below handle it
      } else {
        lastActionSignature = actionSignature;
        repeatCount = 1;
      }

      // Safety: exit after too many turns with success (task likely done)
      if (hadAnySuccess && turns > 10) {
        log(`\n[Orchestrator] ${turns} turns after success - assuming complete`);
        logParseErrors();
        return {
          success: true,
          turns,
          tokens: state.totalTokens,
          durationMs: Date.now() - state.startTime,
          output: outputText,
        };
      }

      // Check if FM signaled task complete
      if (workerOutput.toolName === "task_complete") {
        const result = await finalizeIfDone(
          "task_complete",
          options,
          {
            step: turns,
            history: stepHistory,
            verifyRetryCount,
            maxVerifyRetries,
            totalTokens: state.totalTokens,
            durationMs: Date.now() - state.startTime,
            output: outputText,
            hadAnySuccess,
          },
          log,
          logParseErrors
        );
        if (result) {
          return result;
        }
        // Verification failed but we're retrying - reset counters
        consecutiveFailures = 0;
        repeatCount = 0;
        lastActionSignature = "";
        continue;
      }

      // Check if same action repeated too many times
      if (repeatCount >= MAX_REPEAT_ACTIONS) {
        const result = await finalizeIfDone(
          "repeat_same_action",
          options,
          {
            step: turns,
            history: stepHistory,
            verifyRetryCount,
            maxVerifyRetries,
            totalTokens: state.totalTokens,
            durationMs: Date.now() - state.startTime,
            output: outputText,
            hadAnySuccess,
          },
          log,
          logParseErrors
        );
        if (result) {
          return result;
        }
        // Verification failed but we're retrying - reset counters
        consecutiveFailures = 0;
        repeatCount = 0;
        lastActionSignature = "";
        continue;
      }

      const result = await executeTool(
        workerOutput.toolName,
        workerOutput.toolArgs,
        options.workspace,
      );
      log(`[Tool] ${workerOutput.toolName}: ${result.success ? "success" : "failed"} - ${result.condensed}`);

      // Handle verify_progress tool - run verification and store feedback
      if (workerOutput.toolName === "verify_progress" && options.verifyTask) {
        try {
          const passed = await options.verifyTask();
          // Format verification feedback for next turn
          lastVerificationFeedback = passed
            ? "All tests passing! You can call task_complete."
            : "Tests failing. Review the output above and fix the issues.";
          log(`[Orchestrator] Verification: ${passed ? "PASSED" : "FAILED"}`);
        } catch (e) {
          lastVerificationFeedback = `Verification error: ${e instanceof Error ? e.message : String(e)}`;
        }
      }

      // Track tool name for hint system
      toolHistory.push(workerOutput.toolName);

      // Create step summary with tool-aware summarization
      // IMPORTANT: Pass toolCall.arguments (the parsed args from FM's response)
      // to summarizeToolResult so it can generate tool-aware summaries.
      const summary = summarizeToolResult(
        turns,
        workerOutput.toolName,
        result.success,
        result.output,
        workerOutput.toolArgs  // The args FM provided (path, command, content, etc.)
      );
      stepHistory.push(summary);

      // Also keep old history for backward compatibility (if needed elsewhere)
      state.history.push(truncate(result.condensed, MAX_HISTORY_ENTRY_CHARS));

      if (result.success) {
        hadAnySuccess = true;
        consecutiveFailures = 0; // Reset on success
      } else {
        consecutiveFailures++; // Increment on failure
      }

    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      log(`[Orchestrator] Error: ${errMsg}`);
      // Add error summary
      stepHistory.push({
        step: turns,
        tool: "error",
        success: false,
        message: `Error: ${condenseError(errMsg)}`,
      });
      state.history.push(truncate(`Error: ${condenseError(errMsg)}`, MAX_HISTORY_ENTRY_CHARS));
      consecutiveFailures++;
    }
  }

  log(`\n[Orchestrator] Max turns reached`);
  logParseErrors();

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
