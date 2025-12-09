/**
 * TB2 Docker Runner
 *
 * Runs Terminal-Bench 2 verification in Docker containers.
 * This ensures tests run in the expected environment (/app/) without gaming.
 *
 * For blind verification: returns ONLY pass/fail, not expected values.
 */

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, cpSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

export interface TB2DockerRunnerOptions {
  /** Path to the TB2 task directory (e.g., /path/to/terminal-bench-2/regex-log) */
  taskDir: string;
  /** Workspace directory with the solution (will be mounted to /app/) */
  workspace: string;
  /** Timeout in milliseconds (default: 120000) */
  timeout?: number;
  /** Whether to capture detailed output (default: false for blind verification) */
  captureDetails?: boolean;
}

export interface TB2DockerResult {
  /** Did all tests pass? */
  passed: boolean;
  /** Progress score 0-1 */
  progress: number;
  /** Number of tests passing */
  testsPassing: number;
  /** Total number of tests */
  testsTotal: number;
  /** Generic feedback (no expected values) */
  feedback?: string;
  /** Exit code from pytest */
  exitCode: number;
  /** Duration in ms */
  durationMs: number;
}

/**
 * Run TB2 verification in Docker.
 *
 * This creates a temporary directory with:
 * - The workspace solution files
 * - The TB2 test files
 *
 * Then runs pytest in a container.
 */
export async function runTB2InDocker(
  options: TB2DockerRunnerOptions
): Promise<TB2DockerResult> {
  const { taskDir, workspace, timeout = 120000, captureDetails = false } = options;
  const startTime = Date.now();

  // Validate inputs
  if (!existsSync(taskDir)) {
    throw new Error(`TB2 task directory not found: ${taskDir}`);
  }
  if (!existsSync(workspace)) {
    throw new Error(`Workspace not found: ${workspace}`);
  }

  const testsDir = join(taskDir, "tests");
  if (!existsSync(testsDir)) {
    throw new Error(`Tests directory not found: ${testsDir}`);
  }

  // Create temp directory for Docker context
  const dockerContext = mkdtempSync(join(tmpdir(), "tb2-docker-"));

  try {
    // Copy workspace to docker context (this becomes /app/)
    cpSync(workspace, dockerContext, { recursive: true });

    // Copy TB2 tests to docker context
    const testsDestDir = join(dockerContext, "tests");
    cpSync(testsDir, testsDestDir, { recursive: true });

    // Run pytest in Docker container
    // Use python:3.11 base image with pytest pre-installed
    const dockerArgs = [
      "run",
      "--rm",
      "-v", `${dockerContext}:/app`,
      "-w", "/app",
      "python:3.11-slim",
      "sh", "-c",
      "pip install -q pytest && pytest tests/ -v 2>&1"
    ];

    const result = await runDockerCommand(dockerArgs, timeout);

    // Parse pytest output
    const parsed = parsePytestSummary(result.stdout + result.stderr);

    const durationMs = Date.now() - startTime;

    // For blind verification, provide only generic feedback
    let feedback: string | undefined;
    if (!parsed.passed && !captureDetails) {
      // Generic feedback without revealing expected values
      if (parsed.failed > 0) {
        feedback = `${parsed.failed} test${parsed.failed > 1 ? 's' : ''} failing. Check edge cases.`;
      }
    }

    return {
      passed: parsed.passed,
      progress: parsed.total > 0 ? parsed.passing / parsed.total : 0,
      testsPassing: parsed.passing,
      testsTotal: parsed.total,
      feedback,
      exitCode: result.exitCode,
      durationMs,
    };

  } finally {
    // Cleanup temp directory
    try {
      rmSync(dockerContext, { recursive: true, force: true });
    } catch {
      // Ignore cleanup errors
    }
  }
}

/**
 * Run a Docker command and capture output.
 */
async function runDockerCommand(
  args: string[],
  timeout: number
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const proc = spawn("docker", args, {
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data) => {
      stdout += data.toString();
    });

    proc.stderr.on("data", (data) => {
      stderr += data.toString();
    });

    const timer = setTimeout(() => {
      proc.kill("SIGKILL");
      reject(new Error(`Docker command timed out after ${timeout}ms`));
    }, timeout);

    proc.on("close", (code) => {
      clearTimeout(timer);
      resolve({
        exitCode: code ?? 1,
        stdout,
        stderr,
      });
    });

    proc.on("error", (err) => {
      clearTimeout(timer);
      reject(new Error(`Docker command failed: ${err.message}`));
    });
  });
}

/**
 * Parse pytest summary line to extract pass/fail counts.
 *
 * Examples:
 * - "1 passed in 0.12s"
 * - "3 passed, 2 failed in 0.45s"
 * - "9 passed"
 */
function parsePytestSummary(output: string): {
  passing: number;
  failed: number;
  total: number;
  passed: boolean;
} {
  // Look for pytest summary patterns
  const summaryMatch = output.match(
    /(\d+)\s+passed(?:,\s+(\d+)\s+failed)?/i
  );

  if (summaryMatch) {
    const passing = parseInt(summaryMatch[1], 10);
    const failed = summaryMatch[2] ? parseInt(summaryMatch[2], 10) : 0;
    return {
      passing,
      failed,
      total: passing + failed,
      passed: failed === 0,
    };
  }

  // Try alternative pattern (just failed)
  const failedOnlyMatch = output.match(/(\d+)\s+failed/i);
  if (failedOnlyMatch) {
    const failed = parseInt(failedOnlyMatch[1], 10);
    return {
      passing: 0,
      failed,
      total: failed,
      passed: false,
    };
  }

  // Check for "no tests ran" or error
  if (output.includes("no tests ran") || output.includes("error")) {
    return {
      passing: 0,
      failed: 0,
      total: 0,
      passed: false,
    };
  }

  // Default: couldn't parse
  return {
    passing: 0,
    failed: 0,
    total: 0,
    passed: false,
  };
}
