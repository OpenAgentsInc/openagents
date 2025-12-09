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
import { loadTaskConfig } from "./tb2-config.js";
import { ensureTaskImage } from "./tb2-image-manager.js";

export interface TB2DockerRunnerOptions {
  /** Task ID (e.g., "regex-log") */
  taskId: string;
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
  /** Names of failed tests */
  failedTests?: string[];
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
 * Then runs pytest in a container with the task-specific Docker image.
 */
export async function runTB2InDocker(
  options: TB2DockerRunnerOptions
): Promise<TB2DockerResult> {
  const { taskId, taskDir, workspace, timeout = 120000, captureDetails = false } = options;
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

  // Load task configuration from task.toml
  const taskConfig = await loadTaskConfig(taskDir);
  const envConfig = taskConfig.environment || {};

  // Ensure proper Docker image is available
  const imageOptions: { timeout?: number } = {};
  if (envConfig.build_timeout_sec) {
    imageOptions.timeout = envConfig.build_timeout_sec * 1000;
  }
  const dockerImage = await ensureTaskImage(taskId, taskDir, envConfig, imageOptions);

  console.log(`[TB2] Running verification for ${taskId} with image: ${dockerImage}`);

  // Create temp directory for Docker context
  const dockerContext = mkdtempSync(join(tmpdir(), "tb2-docker-"));

  try {
    // Copy workspace to docker context (this becomes /app/)
    cpSync(workspace, dockerContext, { recursive: true });

    // Check if testgen-generated tests exist in workspace
    const workspaceTestsDir = join(workspace, "tests");
    const hasTestgenTests = existsSync(workspaceTestsDir) && existsSync(join(workspaceTestsDir, "test_outputs.py"));

    if (hasTestgenTests) {
      console.log(`[TB2] Using testgen-generated tests from workspace`);
      // Tests already copied with workspace - use those
    } else {
      console.log(`[TB2] Using TB2 reference tests from ${testsDir}`);
      // Copy TB2 tests to docker context
      const testsDestDir = join(dockerContext, "tests");
      cpSync(testsDir, testsDestDir, { recursive: true });
    }

    // Run pytest in Docker container with task-specific image
    const dockerArgs = [
      "run",
      "--rm",
      "-v", `${dockerContext}:/app`,
      "-w", "/app",
    ];

    // Add resource limits if specified
    if (envConfig.memory) {
      dockerArgs.push("--memory", envConfig.memory);
    }
    if (envConfig.cpus) {
      dockerArgs.push("--cpus", String(envConfig.cpus));
    }

    dockerArgs.push(
      dockerImage,
      "sh", "-c",
      // Robust pytest installation and execution
      // 1. Check if pytest already available
      "if command -v pytest >/dev/null 2>&1 || python3 -m pytest --version >/dev/null 2>&1; then " +
      "  echo '[PYTEST] Already installed' >&2; " +
      "else " +
      // 2. Try to install pytest using pip
      "  echo '[PYTEST] Installing pytest...' >&2; " +
      "  if command -v python3 >/dev/null 2>&1; then " +
      // Try with --break-system-packages (for Debian 12+)
      "    python3 -m pip install --break-system-packages pytest >/dev/null 2>&1 || " +
      // Fallback: try without --break-system-packages
      "    python3 -m pip install --user pytest >/dev/null 2>&1 || " +
      // Fallback: try with apt-get if pip fails
      "    (apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq python3-pytest >/dev/null 2>&1); " +
      "  else " +
      // No python3 - install it first
      "    apt-get update -qq >/dev/null 2>&1 && " +
      "    apt-get install -y -qq python3 python3-pytest >/dev/null 2>&1; " +
      "  fi; " +
      "fi && " +
      // 3. Verify pytest is now available
      "if ! (command -v pytest >/dev/null 2>&1 || python3 -m pytest --version >/dev/null 2>&1); then " +
      "  echo '[PYTEST] ERROR: pytest not available after installation' >&2; " +
      "  exit 127; " +
      "fi && " +
      // 4. Run tests
      "echo '=== PYTEST OUTPUT START ===' && " +
      "python3 -m pytest tests/ -v 2>&1"
    );

    const result = await runDockerCommand(dockerArgs, timeout);

    // Debug: log raw output
    const fullOutput = result.stdout + result.stderr;
    console.log(`[TB2] Docker exitCode: ${result.exitCode}`);
    console.log(`[TB2] Docker output length: ${fullOutput.length} chars`);
    if (fullOutput.length > 0) {
      console.log(`[TB2] Docker output (first 500 chars):\n${fullOutput.substring(0, 500)}`);
    }

    // Parse pytest output
    const parsed = parsePytestSummary(fullOutput);

    const durationMs = Date.now() - startTime;

    // For blind verification, provide only generic feedback
    let feedback: string | undefined;
    if (!parsed.passed && !captureDetails) {
      // Generic feedback without revealing expected values
      if (parsed.failed > 0) {
        const testList = parsed.failedTests.length > 0
          ? `\nFailing tests: ${parsed.failedTests.slice(0, 5).join(', ')}${parsed.failedTests.length > 5 ? '...' : ''}`
          : '';
        feedback = `${parsed.failed} test${parsed.failed > 1 ? 's' : ''} failing.${testList}`;
      }
    }

    const dockerResult: TB2DockerResult = {
      passed: parsed.passed,
      progress: parsed.total > 0 ? parsed.passing / parsed.total : 0,
      testsPassing: parsed.passing,
      testsTotal: parsed.total,
      exitCode: result.exitCode,
      durationMs,
    };
    if (feedback) {
      dockerResult.feedback = feedback;
    }
    if (parsed.failedTests.length > 0) {
      dockerResult.failedTests = parsed.failedTests;
    }
    return dockerResult;

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
 * Parse failed test names from pytest verbose output.
 *
 * Looks for patterns like:
 * - "FAILED tests/test_outputs.py::test_anti_cheat_1 - AssertionError"
 * - "tests/test_example.py::test_boundary_2 FAILED"
 */
function parseFailedTests(output: string): string[] {
  const failedTests: string[] = [];

  // Pattern 1: FAILED path::test_name
  const pattern1 = /FAILED\s+[^:]+::(\w+)/g;
  let match;
  while ((match = pattern1.exec(output)) !== null) {
    if (match[1] && !failedTests.includes(match[1])) {
      failedTests.push(match[1]);
    }
  }

  // Pattern 2: path::test_name FAILED
  const pattern2 = /[^:]+::(\w+)\s+FAILED/g;
  while ((match = pattern2.exec(output)) !== null) {
    if (match[1] && !failedTests.includes(match[1])) {
      failedTests.push(match[1]);
    }
  }

  return failedTests;
}

/**
 * Parse pytest summary line to extract pass/fail counts and failed test names.
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
  failedTests: string[];
} {
  // Parse failed test names
  const failedTests = parseFailedTests(output);

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
      failedTests,
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
      failedTests,
    };
  }

  // Check for "no tests ran" or error
  if (output.includes("no tests ran") || output.includes("error")) {
    return {
      passing: 0,
      failed: 0,
      total: 0,
      passed: false,
      failedTests: [],
    };
  }

  // Default: couldn't parse
  return {
    passing: 0,
    failed: 0,
    total: 0,
    passed: false,
    failedTests: [],
  };
}
