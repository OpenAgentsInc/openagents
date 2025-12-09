/**
 * HillClimber Evaluator Module
 *
 * Real-time progress scoring during task execution.
 * Runs verification, parses test output, returns structured feedback.
 *
 * Part of the MAP-inspired architecture for 10x better HillClimber.
 */

import { Effect } from "effect";
import { existsSync } from "node:fs";
import { join } from "node:path";
import type { TerminalBenchTask } from "../bench/terminal-bench.js";
import { runTB2InDocker } from "../bench/tb2-docker-runner.js";

// ============================================================================
// Types
// ============================================================================

export interface FailureDetail {
  /** Test case identifier (e.g., "test_regex_matches_dates") */
  testName: string;
  /** Line number in test file, if available */
  lineNumber?: number;
  /** What was expected */
  expected?: string;
  /** What was actually received */
  actual?: string;
  /** Raw error message */
  message: string;
}

export interface EvaluatorResult {
  /** Overall pass/fail status */
  passed: boolean;
  /** Progress score 0-1 (tests passing / total tests) */
  progress: number;
  /** Total number of tests */
  testsTotal: number;
  /** Number of tests passing */
  testsPassing: number;
  /** Detailed failure information */
  failures: FailureDetail[];
  /** Suggestion for what to try next */
  suggestion?: string;
  /** Raw verification output */
  rawOutput: string;
  /** Duration in ms */
  durationMs: number;
}

// ============================================================================
// Verification Runner
// ============================================================================

/**
 * Run verification command and return raw output.
 */
export const runVerification = (
  task: TerminalBenchTask,
  workspace: string
): Effect.Effect<{ exitCode: number; output: string }, Error> =>
  Effect.tryPromise({
    try: async () => {
      const startTime = Date.now();
      const cmd = task.verification.command ?? task.verification.script ?? "exit 1";

      // Change to workspace directory
      const originalCwd = process.cwd();
      process.chdir(workspace);

      try {
        const proc = Bun.spawn(["sh", "-c", cmd], {
          stdout: "pipe",
          stderr: "pipe",
          cwd: workspace,
          env: {
            ...process.env,
            PYTHONUNBUFFERED: "1",
          },
        });

        const exitCode = await proc.exited;
        const stdout = await new Response(proc.stdout).text();
        const stderr = await new Response(proc.stderr).text();

        const durationMs = Date.now() - startTime;

        return {
          exitCode,
          output: stdout + (stderr ? `\nSTDERR:\n${stderr}` : ""),
          durationMs,
        };
      } finally {
        process.chdir(originalCwd);
      }
    },
    catch: (e) => new Error(`Verification failed: ${e}`),
  });

/**
 * Run verification using Docker for TB2 tasks.
 *
 * This is the proper way to run TB2 tests - in the Docker environment
 * where /app/ exists as expected. Uses blind verification (pass/fail only).
 *
 * @param task - TB2 task with source_path pointing to task directory
 * @param workspace - Local workspace with the solution
 */
export const runVerificationWithDocker = (
  task: TerminalBenchTask,
  workspace: string
): Effect.Effect<{ exitCode: number; output: string; passed: boolean; progress: number; testsPassing: number; testsTotal: number }, Error> =>
  Effect.tryPromise({
    try: async () => {
      // Check if task has source_path with tests
      const taskDir = task.source_path;
      if (!taskDir || !existsSync(taskDir)) {
        throw new Error(`Task source_path not found: ${taskDir}. Cannot run Docker verification.`);
      }

      const testsDir = join(taskDir, "tests");
      if (!existsSync(testsDir)) {
        throw new Error(`Tests directory not found: ${testsDir}. Cannot run Docker verification.`);
      }

      // Run verification in Docker
      const result = await runTB2InDocker({
        taskId: task.id,
        taskDir,
        workspace,
        timeout: 120000,
        captureDetails: false, // Blind verification - no expected values
      });

      // Format output for compatibility with existing parsers
      const output = result.passed
        ? `${result.testsPassing} passed in ${result.durationMs}ms`
        : `${result.testsPassing} passed, ${result.testsTotal - result.testsPassing} failed in ${result.durationMs}ms${result.feedback ? `\n${result.feedback}` : ""}`;

      return {
        exitCode: result.exitCode,
        output,
        passed: result.passed,
        progress: result.progress,
        testsPassing: result.testsPassing,
        testsTotal: result.testsTotal,
      };
    },
    catch: (e) => new Error(`Docker verification failed: ${e}`),
  });

// ============================================================================
// Output Parsers
// ============================================================================

/**
 * Parse pytest output to extract test results.
 */
export function parsePytestOutput(output: string): {
  total: number;
  passed: number;
  failed: number;
  failures: FailureDetail[];
} {
  const failures: FailureDetail[] = [];

  // Parse pytest summary line (e.g., "1 passed, 2 failed in 0.05s")
  const summaryMatch = output.match(/(\d+)\s+passed.*?(\d+)\s+failed/i);
  const passedOnlyMatch = output.match(/(\d+)\s+passed/i);
  const failedOnlyMatch = output.match(/(\d+)\s+failed/i);

  let passed = 0;
  let failed = 0;

  if (summaryMatch) {
    passed = parseInt(summaryMatch[1], 10);
    failed = parseInt(summaryMatch[2], 10);
  } else if (passedOnlyMatch && !failedOnlyMatch) {
    passed = parseInt(passedOnlyMatch[1], 10);
    failed = 0;
  } else if (failedOnlyMatch) {
    failed = parseInt(failedOnlyMatch[1], 10);
    passed = passedOnlyMatch ? parseInt(passedOnlyMatch[1], 10) : 0;
  }

  // Parse individual test failures
  // Pattern: FAILED tests/test_*.py::test_name - AssertionError: message
  const failurePattern = /FAILED\s+(\S+)::(\w+)\s*[-–]\s*(.+?)(?=\n(?:FAILED|PASSED|===|$))/gs;
  let match;

  while ((match = failurePattern.exec(output)) !== null) {
    failures.push({
      testName: match[2],
      message: match[3].trim(),
    });
  }

  // Also parse assertion details from verbose output
  // Pattern: AssertionError: Expected [...], but got [...]
  const assertPattern = /AssertionError:\s*Expected\s+(.+?),\s+but\s+got\s+(.+?)(?=\n|$)/gi;

  while ((match = assertPattern.exec(output)) !== null) {
    // Find the corresponding failure and add details
    if (failures.length > 0) {
      const lastFailure = failures[failures.length - 1];
      if (!lastFailure.expected) {
        lastFailure.expected = match[1].trim();
        lastFailure.actual = match[2].trim();
      }
    }
  }

  // Parse specific pattern for regex-log task
  // "Expected ['date1', 'date2'], but got ['date1', 'date3']"
  const listAssertPattern = /Expected\s+(\[.+?\]),\s+but\s+got\s+(\[.+?\])/gi;

  while ((match = listAssertPattern.exec(output)) !== null) {
    if (failures.length > 0) {
      const lastFailure = failures[failures.length - 1];
      if (!lastFailure.expected) {
        lastFailure.expected = match[1];
        lastFailure.actual = match[2];
      }
    }
  }

  const total = passed + failed;

  return { total, passed, failed, failures };
}

/**
 * Parse generic test output (exit code based).
 */
export function parseGenericOutput(output: string, exitCode: number): {
  total: number;
  passed: number;
  failed: number;
  failures: FailureDetail[];
} {
  // Simple: if exit code is 0, all tests passed
  if (exitCode === 0) {
    return { total: 1, passed: 1, failed: 0, failures: [] };
  }

  // Try to extract error information from output
  const failures: FailureDetail[] = [];

  // Look for common error patterns
  const errorPatterns = [
    /error:\s*(.+)/gi,
    /failed:\s*(.+)/gi,
    /assertion.*?:\s*(.+)/gi,
    /expected\s+(.+?)\s+but\s+got\s+(.+)/gi,
  ];

  for (const pattern of errorPatterns) {
    let match;
    while ((match = pattern.exec(output)) !== null) {
      failures.push({
        testName: "verification",
        message: match[0],
        expected: match[1],
        actual: match[2],
      });
    }
  }

  // If no failures parsed, create a generic one
  if (failures.length === 0) {
    failures.push({
      testName: "verification",
      message: output.slice(0, 500) + (output.length > 500 ? "..." : ""),
    });
  }

  return { total: 1, passed: 0, failed: 1, failures };
}

// ============================================================================
// Suggestion Generator
// ============================================================================

/**
 * Generate a suggestion based on failures.
 */
export function generateSuggestion(
  taskId: string,
  failures: FailureDetail[]
): string | undefined {
  if (failures.length === 0) {
    return undefined;
  }

  // Task-specific suggestions
  if (taskId === "regex-log") {
    const firstFailure = failures[0];
    if (firstFailure.expected && firstFailure.actual) {
      // Analyze the difference (with error handling)
      let expected: unknown[] = [];
      let actual: unknown[] = [];
      try {
        expected = JSON.parse(firstFailure.expected || "[]");
        actual = JSON.parse(firstFailure.actual || "[]");
      } catch (e) {
        // If JSON parsing fails, try to extract arrays from string representation
        const expectedMatch = firstFailure.expected.match(/\[(.*?)\]/);
        const actualMatch = firstFailure.actual.match(/\[(.*?)\]/);
        if (expectedMatch) {
          expected = expectedMatch[1].split(",").map(s => s.trim().replace(/['"]/g, ""));
        }
        if (actualMatch) {
          actual = actualMatch[1].split(",").map(s => s.trim().replace(/['"]/g, ""));
        }
      }

      if (expected.length > actual.length) {
        return `Missing ${expected.length - actual.length} matches. Check if regex is too restrictive.`;
      } else if (actual.length > expected.length) {
        return `${actual.length - expected.length} false positives. Check boundary conditions and IP validation.`;
      } else {
        // Same length but different values
        const wrongMatches = actual.filter((a: string) => !expected.includes(a));
        if (wrongMatches.length > 0) {
          return `Wrong matches: ${wrongMatches.join(", ")}. Check date validation or IP boundary conditions.`;
        }
      }
    }
    return "Check regex for: 1) Valid IPv4 (0-255 per octet), 2) Valid dates (month 1-12, correct days), 3) Boundary assertions.";
  }

  if (taskId === "path-tracing") {
    return "Check: 1) PPM format (P6 header), 2) Image dimensions match reference, 3) Path tracing algorithm correctness.";
  }

  if (taskId === "model-extraction-relu-logits") {
    return "Check: 1) Query strategy (try unit vectors), 2) ReLU activation handling, 3) Row matching tolerance.";
  }

  if (taskId === "video-processing") {
    return "Check: 1) Frame difference calculation, 2) Motion threshold tuning, 3) TOML output format.";
  }

  if (taskId === "dna-assembly") {
    return "Check: 1) BsaI site structure, 2) Melting temperature calculation, 3) Overhang uniqueness.";
  }

  // Generic suggestion
  return `Fix: ${failures[0].message.slice(0, 100)}`;
}

// ============================================================================
// Main Evaluator
// ============================================================================

/**
 * Evaluate progress on a task by running verification and parsing results.
 *
 * @param task Terminal-Bench task definition
 * @param workspace Working directory containing task files
 * @returns Structured evaluation result with progress score and failures
 */
export const evaluateProgress = (
  task: TerminalBenchTask,
  workspace: string
): Effect.Effect<EvaluatorResult, Error, never> =>
  Effect.gen(function* () {
    const startTime = Date.now();

    // Run verification
    const { exitCode, output } = yield* runVerification(task, workspace);

    const durationMs = Date.now() - startTime;

    // Parse output based on verification type
    let parseResult: { total: number; passed: number; failed: number; failures: FailureDetail[] };

    if (task.verification.type === "test" || output.includes("pytest") || output.includes("PASSED") || output.includes("FAILED")) {
      parseResult = parsePytestOutput(output);
    } else {
      parseResult = parseGenericOutput(output, exitCode);
    }

    // Calculate progress
    const progress = parseResult.total > 0 ? parseResult.passed / parseResult.total : 0;
    const passed = exitCode === 0;

    // Generate suggestion
    const suggestion = generateSuggestion(task.id, parseResult.failures);

    const result: EvaluatorResult = {
      passed,
      progress,
      testsTotal: parseResult.total,
      testsPassing: parseResult.passed,
      failures: parseResult.failures,
      rawOutput: output,
      durationMs,
    };

    if (suggestion !== undefined) {
      result.suggestion = suggestion;
    }

    return result;
  });

/**
 * Evaluate progress using Docker-based verification for TB2 tasks.
 *
 * This uses the proper Docker environment where /app/ exists.
 * Returns blind verification results (pass/fail + progress, no expected values).
 *
 * @param task Terminal-Bench task definition (must have source_path)
 * @param workspace Working directory containing solution files
 * @returns Structured evaluation result with progress score
 */
export const evaluateProgressWithDocker = (
  task: TerminalBenchTask,
  workspace: string
): Effect.Effect<EvaluatorResult, Error, never> =>
  Effect.gen(function* () {
    const result = yield* runVerificationWithDocker(task, workspace);

    // For blind verification, we don't parse individual failures
    // We only know pass/fail and generic progress
    const evalResult: EvaluatorResult = {
      passed: result.passed,
      progress: result.progress,
      testsTotal: result.testsTotal,
      testsPassing: result.testsPassing,
      failures: [], // Blind - no individual failure details
      rawOutput: result.output,
      durationMs: 0, // Not tracked in Docker result
    };

    // Add generic suggestion for failures (no expected values)
    if (!result.passed && result.testsTotal > 0) {
      const failCount = result.testsTotal - result.testsPassing;
      evalResult.suggestion = `${failCount} test${failCount > 1 ? "s" : ""} failing. Review edge cases and constraints.`;
    }

    return evalResult;
  });

/**
 * Quick evaluation that doesn't parse detailed failures.
 * Use for frequent checks during execution.
 */
export const quickEvaluate = async (
  task: TerminalBenchTask,
  workspace: string
): Promise<{ passed: boolean; progress: number; message: string }> => {
  try {
    const cmd = task.verification.command ?? task.verification.script ?? "exit 1";

    const originalCwd = process.cwd();
    process.chdir(workspace);

    try {
      const proc = Bun.spawn(["sh", "-c", cmd], {
        stdout: "pipe",
        stderr: "pipe",
        cwd: workspace,
      });

      const exitCode = await proc.exited;
      const stdout = await new Response(proc.stdout).text();

      // Quick parse for pass count
      const passedMatch = stdout.match(/(\d+)\s+passed/i);
      const failedMatch = stdout.match(/(\d+)\s+failed/i);

      const passed = passedMatch ? parseInt(passedMatch[1], 10) : 0;
      const failed = failedMatch ? parseInt(failedMatch[1], 10) : 0;
      const total = passed + failed || 1;

      const progress = exitCode === 0 ? 1 : passed / total;

      return {
        passed: exitCode === 0,
        progress,
        message: exitCode === 0 ? "All tests passing" : `${passed}/${total} tests passing`,
      };
    } finally {
      process.chdir(originalCwd);
    }
  } catch (e) {
    return {
      passed: false,
      progress: 0,
      message: `Evaluation error: ${e}`,
    };
  }
};

/**
 * Format evaluation result for injection into FM prompt.
 */
export function formatForPrompt(result: EvaluatorResult): string {
  const lines: string[] = [];

  if (result.passed) {
    lines.push(`Verification: PASSED (${result.testsPassing}/${result.testsTotal} tests)`);
  } else {
    lines.push(`Verification: FAILED (${result.testsPassing}/${result.testsTotal} tests)`);

    // Add first 3 failures
    for (const failure of result.failures.slice(0, 3)) {
      if (failure.expected && failure.actual) {
        lines.push(`  - ${failure.testName}: expected ${failure.expected}, got ${failure.actual}`);
      } else {
        lines.push(`  - ${failure.testName}: ${failure.message.slice(0, 100)}`);
      }
    }

    if (result.failures.length > 3) {
      lines.push(`  ... and ${result.failures.length - 3} more failures`);
    }

    if (result.suggestion) {
      lines.push(`Suggestion: ${result.suggestion}`);
    }
  }

  return lines.join("\n");
}
