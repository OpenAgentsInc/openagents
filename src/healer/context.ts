/**
 * Healer Context Builder
 *
 * Builds the HealerContext from available data sources:
 * - OrchestratorState and event
 * - ProjectConfig
 * - Git status
 * - Progress file
 * - ATIF trajectories (optional)
 */
import { Effect } from "effect";
import type {
  OrchestratorEvent,
  OrchestratorState,
} from "../agent/orchestrator/types.js";
import type { ProjectConfig } from "../tasks/schema.js";
import type {
  HealerContext,
  HealerScenario,
  HealerCounters,
  GitStatus,
  HealerHeuristics,
} from "./types.js";
import { getErrorOutput, getInitScriptResult } from "./policy.js";

// ============================================================================
// Git Status
// ============================================================================

/**
 * Get current git status.
 */
export const getGitStatus = async (cwd: string): Promise<GitStatus> => {
  try {
    // Get status
    const statusProc = Bun.spawn(["git", "status", "--porcelain"], { cwd });
    const statusOutput = await new Response(statusProc.stdout).text();
    await statusProc.exited;

    const lines = statusOutput.trim().split("\n").filter(Boolean);
    const modifiedFiles: string[] = [];
    const untrackedFiles: string[] = [];

    for (const line of lines) {
      const status = line.slice(0, 2);
      const file = line.slice(3).trim();
      if (status.includes("?")) {
        untrackedFiles.push(file);
      } else {
        modifiedFiles.push(file);
      }
    }

    // Get current branch
    const branchProc = Bun.spawn(["git", "rev-parse", "--abbrev-ref", "HEAD"], { cwd });
    const branch = (await new Response(branchProc.stdout).text()).trim();
    await branchProc.exited;

    // Get last commit
    const logProc = Bun.spawn(["git", "log", "-1", "--format=%H|%s"], { cwd });
    const logOutput = (await new Response(logProc.stdout).text()).trim();
    await logProc.exited;

    const [sha, message] = logOutput.split("|");

    return {
      isDirty: modifiedFiles.length > 0 || untrackedFiles.length > 0,
      modifiedFiles,
      untrackedFiles,
      currentBranch: branch || "unknown",
      lastCommitSha: sha || "unknown",
      lastCommitMessage: message || "unknown",
    };
  } catch {
    return {
      isDirty: false,
      modifiedFiles: [],
      untrackedFiles: [],
      currentBranch: "unknown",
      lastCommitSha: "unknown",
      lastCommitMessage: "unknown",
    };
  }
};

// ============================================================================
// Progress File
// ============================================================================

/**
 * Read progress.md content if it exists.
 */
export const readProgressFile = async (cwd: string): Promise<string | null> => {
  try {
    const path = `${cwd}/.openagents/progress.md`;
    const file = Bun.file(path);
    if (await file.exists()) {
      return await file.text();
    }
    return null;
  } catch {
    return null;
  }
};

// ============================================================================
// Heuristics
// ============================================================================

/**
 * Detect error patterns in output.
 */
const detectErrorPatterns = (output: string | null): string[] => {
  if (!output) return [];

  const patterns: string[] = [];

  // TypeScript errors
  if (/error TS\d+/.test(output)) {
    patterns.push("TypeScript compilation error");
  }
  if (/cannot find (module|name)/i.test(output)) {
    patterns.push("Missing module or name");
  }
  if (/Property .+ does not exist/i.test(output)) {
    patterns.push("Property access error");
  }
  if (/Type .+ is not assignable/i.test(output)) {
    patterns.push("Type assignment error");
  }

  // Test failures
  if (/\d+ (test|spec)s? failed/i.test(output)) {
    patterns.push("Test failures");
  }
  if (/expect\(.+\)\.(toBe|toEqual)/i.test(output)) {
    patterns.push("Assertion failure");
  }

  // Import/export issues
  if (/import .+ from/i.test(output) && /not found/i.test(output)) {
    patterns.push("Import resolution error");
  }
  if (/export .+ not found/i.test(output)) {
    patterns.push("Export not found");
  }

  // Runtime errors
  if (/TypeError:/i.test(output)) {
    patterns.push("Runtime type error");
  }
  if (/ReferenceError:/i.test(output)) {
    patterns.push("Reference error");
  }
  if (/SyntaxError:/i.test(output)) {
    patterns.push("Syntax error");
  }

  return patterns;
};

/**
 * Build heuristics from context data.
 */
export const buildHeuristics = (
  scenario: HealerScenario,
  errorOutput: string | null,
  failureCount: number
): HealerHeuristics => {
  const patterns = detectErrorPatterns(errorOutput);

  return {
    scenario,
    failureCount,
    isFlaky: false, // TODO: Detect from ATIF history
    hasMissingImports: patterns.some((p) => p.includes("Import") || p.includes("module")),
    hasTypeErrors: patterns.some((p) => p.includes("Type") || p.includes("TypeScript")),
    hasTestAssertions: patterns.some((p) => p.includes("Assertion") || p.includes("Test")),
    errorPatterns: patterns,
    previousAttempts: 0, // TODO: Get from ATIF history
  };
};

// ============================================================================
// Context Builder
// ============================================================================

/**
 * Build full HealerContext from available data.
 */
export const buildHealerContext = (
  trigger: {
    scenario: HealerScenario;
    event: OrchestratorEvent;
  },
  state: OrchestratorState,
  config: ProjectConfig,
  counters: HealerCounters
): Effect.Effect<HealerContext, Error, never> =>
  Effect.gen(function* () {
    const projectRoot = config.rootDir ?? ".";

    // Get git status
    const gitStatus = yield* Effect.tryPromise({
      try: () => getGitStatus(projectRoot),
      catch: (e) => new Error(`Failed to get git status: ${e}`),
    });

    // Read progress file
    const progressMd = yield* Effect.tryPromise({
      try: () => readProgressFile(projectRoot),
      catch: () => null as string | null,
    });

    // Extract error output
    const errorOutput = getErrorOutput(trigger.event);

    // Get failure count from subtask if available
    const subtask = state.subtasks?.subtasks.find(
      (s) => s.status === "failed" || s.status === "in_progress"
    );
    const failureCount = subtask?.failureCount ?? 0;

    // Build heuristics
    const heuristics = buildHeuristics(trigger.scenario, errorOutput, failureCount);

    // Get init failure type if applicable
    const initResult = getInitScriptResult(trigger.event);

    const context: HealerContext = {
      projectRoot,
      projectConfig: config,
      task: state.task ?? undefined,
      subtask,
      sessionId: state.sessionId,
      runId: undefined, // TODO: Get from orchestrator
      trajectory: undefined, // TODO: Load from ATIF service
      relatedTrajectories: [],
      progressMd,
      gitStatus,
      heuristics,
      triggerEvent: trigger.event,
      orchestratorState: state,
      initFailureType: initResult?.failureType,
      errorOutput: errorOutput ?? undefined,
      counters,
    };

    return context;
  });

// Export individual helpers for testing
export { detectErrorPatterns };
