/**
 * Verify Progress Tool
 *
 * A tool that allows FM to check verification status mid-execution.
 * This is key to the MAP architecture - FM gets feedback during execution,
 * not just at the end.
 *
 * Usage in FM prompt:
 *   Call verify_progress to see how many tests are passing and what's failing.
 */

import type { TerminalBenchTask } from "../../bench/terminal-bench.js";
import { quickEvaluate } from "../../hillclimber/evaluator.js";

// ============================================================================
// Types
// ============================================================================

export interface VerifyProgressResult {
  /** Whether all tests are passing */
  passed: boolean;
  /** Progress score 0-1 */
  progress: number;
  /** Human-readable message */
  message: string;
  /** Tests passing count */
  testsPassing: number;
  /** Total tests count */
  testsTotal: number;
  /** Detailed failure messages (first 3) */
  failureMessages: string[];
  /** Suggestion for what to try next */
  suggestion?: string;
}

export interface VerifyProgressOptions {
  /** The task being verified */
  task: TerminalBenchTask;
  /** Workspace directory */
  workspace: string;
  /** Include detailed failure info */
  detailed?: boolean;
}

// ============================================================================
// Tool Definition
// ============================================================================

export const VERIFY_PROGRESS_TOOL = {
  name: "verify_progress",
  description: "Run verification tests and see detailed results. Use this to check your progress and see which tests are failing.",
  parameters: {
    type: "object",
    properties: {},
    required: [],
  },
};

// ============================================================================
// Tool Execution
// ============================================================================

/**
 * Execute the verify_progress tool.
 *
 * @param options Verification options
 * @returns Structured verification result
 */
export async function executeVerifyProgress(
  options: VerifyProgressOptions,
): Promise<VerifyProgressResult> {
  const { task, workspace } = options;

  try {
    const evalResult = await quickEvaluate(task, workspace);

    const result: VerifyProgressResult = {
      passed: evalResult.passed,
      progress: evalResult.progress,
      message: evalResult.message,
      testsPassing: 0,
      testsTotal: 0,
      failureMessages: [],
    };

    // Parse passing/total from message (format: "X/Y tests passing")
    const match = evalResult.message.match(/(\d+)\/(\d+)/);
    if (match) {
      result.testsPassing = parseInt(match[1], 10);
      result.testsTotal = parseInt(match[2], 10);
    } else if (evalResult.passed) {
      result.testsPassing = 1;
      result.testsTotal = 1;
    }

    return result;
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    return {
      passed: false,
      progress: 0,
      message: `Verification error: ${errMsg}`,
      testsPassing: 0,
      testsTotal: 0,
      failureMessages: [errMsg],
    };
  }
}

/**
 * Format verification result for injection into FM prompt.
 *
 * @param result Verification result
 * @returns Formatted string for FM context
 */
export function formatVerifyProgressForPrompt(result: VerifyProgressResult): string {
  const lines: string[] = [];

  if (result.passed) {
    lines.push(`✓ VERIFICATION PASSED (${result.testsPassing}/${result.testsTotal} tests)`);
  } else {
    lines.push(`✗ VERIFICATION FAILED (${result.testsPassing}/${result.testsTotal} tests)`);
    lines.push(`Progress: ${(result.progress * 100).toFixed(1)}%`);

    if (result.failureMessages.length > 0) {
      lines.push("Failures:");
      for (const msg of result.failureMessages) {
        lines.push(`  - ${msg}`);
      }
    }

    if (result.suggestion) {
      lines.push(`Suggestion: ${result.suggestion}`);
    }
  }

  return lines.join("\n");
}

/**
 * Get tool result as condensed string for history.
 *
 * @param result Verification result
 * @returns Condensed string for action history
 */
export function condenseVerifyProgress(result: VerifyProgressResult): string {
  if (result.passed) {
    return `verify_progress: PASSED (${result.testsPassing}/${result.testsTotal})`;
  }
  return `verify_progress: ${result.testsPassing}/${result.testsTotal} tests passing`;
}
