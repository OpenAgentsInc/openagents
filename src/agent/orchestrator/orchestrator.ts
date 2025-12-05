/**
 * Orchestrator Agent
 * 
 * Manages the high-level flow for overnight automation:
 * 1. Orient - Read progress files, git log, understand repo state
 * 2. Select Task - Pick highest priority ready task
 * 3. Decompose - Break task into subtasks
 * 4. Execute - Invoke subagent for each subtask
 * 5. Verify - Run tests after changes
 * 6. Commit & Push - If tests pass
 * 7. Update Task - Mark as done
 * 8. Log - Write progress for next session
 */
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect, Option } from "effect";
import { pickNextTask, updateTask } from "../../tasks/index.js";
import { recoverPendingCommits, type RecoveryEvent } from "./recovery.js";
import {
  writeCheckpoint,
  clearCheckpoint,
  maybeResumeCheckpoint,
  captureGitState,
  createCheckpoint,
  updateCheckpointPhase,
  type OrchestratorCheckpoint,
} from "./checkpoint.js";
import type { OpenRouterClient } from "../../llm/openrouter.js";
import { readTool } from "../../tools/read.js";
import { editTool } from "../../tools/edit.js";
import { bashTool } from "../../tools/bash.js";
import { writeTool } from "../../tools/write.js";
import { runBestAvailableSubagent } from "./subagent-router.js";
import { runInitScript } from "./init-script.js";
import {
  readSubtasks,
  writeSubtasks,
  createSubtaskList,
} from "./decompose.js";
import {
  writeProgress,
  getPreviousSessionSummary,
} from "./progress.js";
import {
  type OrchestratorConfig,
  type OrchestratorState,
  type OrchestratorEvent,
  type SessionProgress,
  type SubagentResult,
  type Subtask,
  type InitScriptResult,
  getProgressPath,
} from "./types.js";
import type { FailureContextType, ReflectionType } from "./reflection/index.js";
import { createHealerCounters } from "../../healer/types.js";
import {
  runVerificationWithSandbox,
  type SandboxRunnerConfig,
} from "./sandbox-runner.js";
import { appendUsageRecord, computeUsageIdempotencyKey } from "../../usage/store.js";
import type { UsageRecord } from "../../usage/types.js";
import {
  createStepResultsManager,
  durableStep,
} from "./step-results.js";
import {
  runVerificationOnHost,
  type VerificationRunResult,
} from "./verification-runner.js";
import * as nodePath from "node:path";
import * as fs from "node:fs";

// Minimal tools for subagent (pi-mono pattern)
const SUBAGENT_TOOLS = [readTool, editTool, bashTool, writeTool];

/**
 * Generate a unique session ID
 */
const generateSessionId = (): string => {
  const now = new Date();
  const ts = now.toISOString().replace(/[:.]/g, "-");
  const rand = Math.random().toString(36).substring(2, 8);
  return `session-${ts}-${rand}`;
};

const summarizeOutput = (output?: string, maxLength = 400): string | undefined => {
  if (!output) return undefined;
  const trimmed = output.trim().replace(/\s+/g, " ");
  if (trimmed.length <= maxLength) return trimmed;
  return `${trimmed.slice(0, maxLength)}...`;
};

const buildVerificationCommands = (
  typecheckCommands: string[] | undefined,
  testCommands: string[],
  sandboxTestCommands?: string[],
  useSandbox?: boolean
): string[] => {
  // Use sandbox test commands when sandbox is enabled and they are defined
  const effectiveTestCommands =
    useSandbox && sandboxTestCommands && sandboxTestCommands.length > 0
      ? sandboxTestCommands
      : testCommands;
  return [...(typecheckCommands ?? []), ...effectiveTestCommands];
};

/**
 * Run verification commands (typecheck, tests).
 * Uses sandbox when config.sandbox is enabled and available, otherwise runs on host.
 * Returns structured per-command results with aggregated pass/fail.
 */
const runVerification = (
  commands: string[],
  cwd: string,
  emit: (event: OrchestratorEvent) => void,
  sandboxConfig?: SandboxRunnerConfig
): Effect.Effect<VerificationRunResult, Error, never> => {
  // If no sandbox config or sandbox explicitly disabled, run on host
  if (!sandboxConfig || sandboxConfig.sandboxConfig.enabled === false) {
    return runVerificationOnHost(commands, cwd, emit);
  }

  // Try sandbox execution with automatic fallback to host
  const sandboxEmit = emit as (event: { type: string; [key: string]: any }) => void;
  return runVerificationWithSandbox(commands, sandboxConfig, sandboxEmit).pipe(
    Effect.map((result) => {
      const results = result.outputs.map((output, idx) => ({
        command: commands[idx] ?? `command-${idx + 1}`,
        exitCode: result.passed ? 0 : 1,
        stdout: output,
        stderr: "",
        durationMs: 0,
      }));
      return { passed: result.passed, outputs: result.outputs, results };
    }),
    Effect.catchAll(() => runVerificationOnHost(commands, cwd, emit))
  );
};

/**
 * Run e2e test commands on the host.
 * Similar to runVerificationOnHost but emits e2e-specific events.
 */
const runE2eOnHost = (
  commands: string[],
  cwd: string,
  emit: (event: OrchestratorEvent) => void
): Effect.Effect<{ passed: boolean; outputs: string[] }, Error, never> =>
  Effect.try({
    try: () => {
      const outputs: string[] = [];
      let allPassed = true;

      for (const cmd of commands) {
        emit({ type: "e2e_start", command: cmd });
        try {
          const { execSync } = require("node:child_process") as typeof import("node:child_process");
          const output = execSync(cmd, {
            cwd,
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
            timeout: 300000,
          });
          outputs.push(String(output));
          emit({ type: "e2e_complete", command: cmd, passed: true, output: String(output) });
        } catch (error: any) {
          const output = String(error?.stdout || error?.stderr || error?.message || error);
          outputs.push(output);
          emit({ type: "e2e_complete", command: cmd, passed: false, output });
          allPassed = false;
        }
      }

      return { passed: allPassed, outputs };
    },
    catch: (error: any) => error as Error,
  });

/**
 * Check if a task should run e2e tests based on its labels.
 */
const shouldRunE2e = (taskLabels: readonly string[] = []): boolean => {
  const e2eLabels = ["e2e", "golden-loop", "integration"];
  return taskLabels.some((label) => e2eLabels.includes(label.toLowerCase()));
};

const buildE2eCommands = (commands: string[] | undefined): string[] =>
  commands?.filter((cmd) => cmd.trim().length > 0) ?? [];

const mapReflectionsToNotes = (reflections: ReflectionType[], limit: number): string[] =>
  reflections
    .slice(0, limit)
    .map((r) => `${r.analysis} - Next: ${r.suggestion}`)
    .map((text) => text.slice(0, 400));

const normalizePathsForGit = (paths: readonly string[], cwd: string): string[] => {
  const normalized = new Set<string>();

  for (const raw of paths) {
    if (!raw) continue;
    const trimmed = raw.trim();
    if (!trimmed) continue;

    const resolved = nodePath.isAbsolute(trimmed)
      ? trimmed
      : nodePath.resolve(cwd, trimmed);
    const relative = nodePath.relative(cwd, resolved);

    // Ignore paths outside the repo
    if (relative.startsWith("..")) continue;

    const gitPath = relative === "" ? "." : relative.split(nodePath.sep).join("/");
    normalized.add(gitPath);
  }

  return Array.from(normalized);
};

/**
 * Create a git commit with the task ID.
 * Stages only the provided paths and passes the commit message via stdin to avoid quoting issues.
 */
export const createCommit = (
  taskId: string,
  message: string,
  cwd: string,
  pathsToStage: readonly string[]
): Effect.Effect<string, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      const { execFileSync } = await import("node:child_process");
      const debugCommit = process.env.DEBUG_CREATE_COMMIT === "1";

      const normalizedPaths = normalizePathsForGit(pathsToStage, cwd);
      if (normalizedPaths.length === 0) {
        throw new Error("No paths provided to stage for commit");
      }

      const statusOutput = execFileSync(
        "git",
        ["status", "--porcelain", "--", ...normalizedPaths],
        { cwd, encoding: "utf-8" }
      );
      if (debugCommit) {
        console.log("[createCommit] normalizedPaths:", normalizedPaths);
        console.log("[createCommit] statusOutput:", statusOutput);
      }
      const stageable = Array.from(
        new Set(
          statusOutput
            .split("\n")
            .map((line) => line)
            .filter((line) => line.trim().length > 0)
            .map((line) => {
              const status = line.slice(0, 2);
              const pathPart = line.slice(3);
              const renameParts = pathPart.split(" -> ");
              const candidate = renameParts[renameParts.length - 1]?.trim() ?? "";
              const absolutePath = nodePath.resolve(cwd, candidate);
              const exists = fs.existsSync(absolutePath);
              const isDeletion = status.includes("D");
              return !candidate || (!exists && !isDeletion) ? "" : candidate;
            })
            .filter((line) => line.length > 0)
        )
      );

      if (stageable.length === 0) {
        throw new Error(
          `No matching changes found for provided paths: ${normalizedPaths.join(", ")}`
        );
      }

      if (debugCommit) {
        console.log("[createCommit] stageable:", stageable);
      }

      execFileSync("git", ["add", "--", ...stageable], { cwd, encoding: "utf-8" });

      const fullMessage = `${taskId}: ${message}

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>
`;

      execFileSync("git", ["commit", "-F", "-"], {
        cwd,
        encoding: "utf-8",
        input: fullMessage,
      });

      const sha = execFileSync("git", ["rev-parse", "HEAD"], { cwd, encoding: "utf-8" }).trim();
      return sha;
    },
    catch: (error: any) => new Error(`Failed to create commit: ${error.message}`),
  });

/**
 * Push to remote
 */
const pushToRemote = (
  branch: string,
  cwd: string
): Effect.Effect<void, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      const { execSync } = await import("node:child_process");
      execSync(`git push origin ${branch}`, { cwd, encoding: "utf-8" });
    },
    catch: (error: any) => new Error(`Failed to push: ${error.message}`),
  });

/**
 * Write checkpoint at phase transition.
 * Captures current state for crash recovery.
 */
const saveCheckpoint = (
  openagentsDir: string,
  checkpoint: OrchestratorCheckpoint,
  emit: (event: OrchestratorEvent) => void
): Effect.Effect<void, never, FileSystem.FileSystem> =>
  writeCheckpoint(openagentsDir, checkpoint).pipe(
    Effect.tap(() => {
      emit({ type: "checkpoint_written", phase: checkpoint.phase });
      return Effect.void;
    }),
    Effect.catchAll(() => Effect.void) // Checkpoint failures are non-fatal
  );

/**
 * Main orchestrator loop.
 * 
 * Coordinates the entire task execution flow:
 * Orient → Select → Decompose → Execute → Verify → Commit → Update → Log
 */
export const runOrchestrator = (
  config: OrchestratorConfig,
  emit: (event: OrchestratorEvent) => void = () => {},
  deps?: { runSubagent?: typeof runBestAvailableSubagent }
): Effect.Effect<OrchestratorState, Error, FileSystem.FileSystem | Path.Path | OpenRouterClient> =>
  Effect.gen(function* () {
    let sessionId = generateSessionId();
    const now = new Date().toISOString();
    const subagentRunner = deps?.runSubagent ?? runBestAvailableSubagent;
    const openagentsDir = config.openagentsDir ?? `${config.cwd}/.openagents`;
    const stepResultsManager = yield* createStepResultsManager(openagentsDir, sessionId);
    sessionId = stepResultsManager.sessionId;
    const tasksPath = `${openagentsDir}/tasks.jsonl`;
    const sessionStartedAt = Date.now();
    const usageAccumulator = {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      totalCostUsd: 0,
      subtasks: 0,
      agents: new Set<string>(),
    };
    
    emit({ type: "session_start", sessionId, timestamp: now });

    const state: OrchestratorState = {
      sessionId,
      task: null,
      subtasks: null,
      progress: null,
      phase: "orienting",
    };
    const recordUsage = (): Effect.Effect<void, never, FileSystem.FileSystem> => {
      const agent: UsageRecord["agent"] =
        usageAccumulator.agents.size === 0
          ? config.claudeCode?.enabled
            ? "claude-code"
            : "unknown"
          : usageAccumulator.agents.size === 1
            ? (Array.from(usageAccumulator.agents)[0] as UsageRecord["agent"])
            : "mixed";

      const record = {
        sessionId,
        projectId: config.projectConfig?.projectId ?? "unknown",
        timestamp: new Date().toISOString(),
        inputTokens: usageAccumulator.inputTokens,
        outputTokens: usageAccumulator.outputTokens,
        cacheReadTokens: usageAccumulator.cacheReadTokens,
        cacheCreationTokens: usageAccumulator.cacheCreationTokens,
        totalCostUsd: usageAccumulator.totalCostUsd,
        agent,
        subtasks: usageAccumulator.subtasks,
        durationMs: Date.now() - sessionStartedAt,
      };

      const hasActivity =
        usageAccumulator.subtasks > 0 ||
        usageAccumulator.inputTokens > 0 ||
        usageAccumulator.outputTokens > 0 ||
        usageAccumulator.totalCostUsd > 0;

      if (!hasActivity) {
        return Effect.void;
      }

      const usageRecord: UsageRecord = {
        ...record,
        idempotencyKey: computeUsageIdempotencyKey(record as UsageRecord),
      };

      emit({ type: "usage_recorded", usage: usageRecord });
      return appendUsageRecord({ rootDir: config.cwd, record: usageRecord }).pipe(Effect.catchAll(() => Effect.void));
    };
    const verificationCommands = buildVerificationCommands(
      config.typecheckCommands,
      config.testCommands,
      config.sandboxTestCommands,
      config.sandbox?.enabled
    );

    // Build sandbox runner config if sandbox is enabled
    const sandboxRunnerConfig: SandboxRunnerConfig | undefined = config.sandbox
      ? {
          sandboxConfig: config.sandbox,
          cwd: config.cwd,
          emit: (event) => {
            // Forward sandbox events as orchestrator events
            if (event.type === "sandbox_available") {
              emit({ type: "sandbox_status", status: "available", backend: event.backend } as any);
            } else if (event.type === "sandbox_unavailable") {
              emit({ type: "sandbox_status", status: "unavailable", reason: event.reason } as any);
            } else if (event.type === "sandbox_fallback") {
              emit({ type: "sandbox_fallback", reason: event.reason } as any);
            }
          },
          // Forward HUD messages for container streaming
          emitHud: config.emitHud,
          context: "verification",
        }
      : undefined;

    // Initialize progress
    const progress: SessionProgress = {
      sessionId,
      startedAt: now,
      taskId: "",
      taskTitle: "",
      orientation: {
        repoState: "",
        testsPassingAtStart: false,
      },
      work: {
        subtasksCompleted: [],
        subtasksInProgress: [],
        filesModified: [],
        testsRun: false,
        testsPassingAfterWork: false,
        e2eRun: false,
        e2ePassingAfterWork: false,
      },
      nextSession: {
        suggestedNextSteps: [],
      },
    };

    const maxReflectionsPerRetry = config.reflexionConfig?.maxReflectionsPerRetry ?? 3;
    const loadRecentReflections = (subtaskId: string, limit?: number) =>
      config.reflectionService && config.reflexionConfig?.enabled !== false
        ? config.reflectionService
            .getRecent(subtaskId, limit ?? maxReflectionsPerRetry)
            .pipe(Effect.catchAll(() => Effect.succeed([] as ReflectionType[])))
        : Effect.succeed([] as ReflectionType[]);
    const recordVerificationReflection = (
      failureOutput: string | undefined,
      failureType: FailureContextType["failureType"]
    ) =>
      config.reflectionService &&
      config.reflexionConfig?.enabled !== false &&
      state.task &&
      state.subtasks &&
      state.subtasks.subtasks.length > 0
        ? Effect.gen(function* () {
            const targetSubtask = state.subtasks!.subtasks[state.subtasks!.subtasks.length - 1];
            const previous = yield* loadRecentReflections(targetSubtask.id);
            const failureContext: FailureContextType = {
              id: `vc-${Date.now().toString(36)}`,
              sessionId,
              taskId: state.task!.id,
              subtaskId: targetSubtask.id,
              subtaskDescription: targetSubtask.description.slice(0, 1000),
              attemptNumber: (targetSubtask.failureCount ?? 0) + 1,
              failureType,
              errorOutput: (failureOutput ?? "Verification failed").slice(0, 2000),
              filesModified: progress.work.filesModified ?? [],
              previousReflections: mapReflectionsToNotes(previous, maxReflectionsPerRetry),
              createdAt: new Date().toISOString(),
            };

            const reflection = yield* config.reflectionService!.generate(failureContext).pipe(
              Effect.catchAll(() => Effect.succeed(null))
            );

            if (reflection) {
              yield* config.reflectionService!.save(reflection).pipe(Effect.catchAll(() => Effect.void));
            }

            targetSubtask.status = "failed";
            targetSubtask.failureCount = (targetSubtask.failureCount ?? 0) + 1;
            targetSubtask.lastFailureReason = failureContext.errorOutput;
            writeSubtasks(openagentsDir, state.subtasks!);
          })
        : Effect.void;

    // Track checkpoint for crash recovery
    let currentCheckpoint: OrchestratorCheckpoint | null = null;

    // Check for existing checkpoint from previous crash
    const maybeExistingCheckpoint = yield* maybeResumeCheckpoint(openagentsDir, config.cwd).pipe(
      Effect.catchAll(() => Effect.succeed(Option.none()))
    );

    if (Option.isSome(maybeExistingCheckpoint)) {
      const existingCheckpoint = maybeExistingCheckpoint.value;
      emit({
        type: "checkpoint_found",
        sessionId: existingCheckpoint.sessionId,
        phase: existingCheckpoint.phase,
        taskId: existingCheckpoint.taskId,
      });

      // For now, we emit that we found a checkpoint but proceed normally
      // The subtask file already tracks completed subtasks, and the two-phase
      // commit recovery handles commit_pending states
      // Future enhancement: could skip orientation phase if checkpoint is recent
      emit({
        type: "checkpoint_resuming",
        phase: existingCheckpoint.phase,
        taskId: existingCheckpoint.taskId,
      });
    }

    try {
      // Phase 1: Orient
      state.phase = "orienting";
      const prevSummary = getPreviousSessionSummary(openagentsDir);
      if (prevSummary) {
        progress.orientation.previousSessionSummary = prevSummary;
      }

      let initScriptResult: InitScriptResult;
      if (config.skipInitScript) {
        // Skip init script (e.g., in worktree runs where main repo is already validated)
        initScriptResult = { ran: false, success: true, output: "Skipped (skipInitScript=true)" };
        emit({ type: "init_script_complete", result: initScriptResult });
      } else {
        initScriptResult = yield* durableStep(
          stepResultsManager,
          "init_script",
          () => runInitScript(openagentsDir, config.cwd, emit),
          { inputHash: config.cwd }
        );
      }
      progress.orientation.initScript = initScriptResult;

      // Healer: Attempt self-healing for init script failures
      if (initScriptResult.ran && !initScriptResult.success && config.healerService && config.projectConfig) {
        // Invoke Healer synchronously using yield*
        const healerOutcome = yield* config.healerService.maybeRun(
          { type: "init_script_complete", result: initScriptResult },
          state,
          config.projectConfig,
          config.healerCounters ?? createHealerCounters()
        );

        if (healerOutcome) {
          // Healer was invoked, log the outcome
          if (healerOutcome.status === "resolved") {
            // Healer resolved the issue, re-run init script to verify
            const retryResult = yield* runInitScript(openagentsDir, config.cwd, emit);

            if (retryResult.success) {
              // Self-healing succeeded! Update init script result and continue
              initScriptResult = retryResult;
              progress.orientation.initScript = retryResult;
              yield* stepResultsManager.recordResult(
                "init_script",
                retryResult,
                config.cwd
              );
            }
          } else if (healerOutcome.status === "contained") {
            // Healer contained the issue (e.g., marked task blocked)
            // Continue with failure, but note that it was handled
          }
          // For "unresolved" or "skipped", continue with failure
        }
      }

      // Check if init script still failed after potential self-healing
      if (initScriptResult.ran && !initScriptResult.success) {
        state.phase = "failed";
        const failureInfo = initScriptResult.failureType
          ? ` (${initScriptResult.failureType}${initScriptResult.canSelfHeal ? ", self-heal attempted" : ""})`
          : "";
        state.error = `Init script failed${failureInfo}`;
        progress.orientation.repoState = "init script failed";
        progress.orientation.testsPassingAtStart = false;

        emit({
          type: "orientation_complete",
          repoState: progress.orientation.repoState,
          testsPassingAtStart: progress.orientation.testsPassingAtStart,
          initScript: initScriptResult,
        });

        progress.nextSession.blockers = [
          `Init script failed${failureInfo}`,
          ...(summarizeOutput(initScriptResult.output) ? [summarizeOutput(initScriptResult.output)!] : []),
        ];
        progress.nextSession.suggestedNextSteps = [
          "Inspect .openagents/init.sh output",
          "Fix init script errors before rerunning",
          ...(config.safeMode && initScriptResult.canSelfHeal ? ["Self-healing was attempted but failed"] : []),
        ];

        writeProgress(openagentsDir, progress);
        yield* recordUsage();
        emit({ type: "session_complete", success: false, summary: state.error });
        yield* stepResultsManager.clear();
        return state;
      }

      // Quick test check
      if (verificationCommands.length > 0) {
        const testResult = yield* runVerification(
          [verificationCommands[0]],
          config.cwd,
          emit,
          sandboxRunnerConfig
        ).pipe(Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] })));
        progress.orientation.testsPassingAtStart = testResult.passed;
      } else {
        progress.orientation.testsPassingAtStart = true;
      }

      progress.orientation.repoState = progress.orientation.testsPassingAtStart ? "clean" : "typecheck_failing";
      emit({
        type: "orientation_complete",
        repoState: progress.orientation.repoState,
        testsPassingAtStart: progress.orientation.testsPassingAtStart,
        initScript: initScriptResult,
      });

      // Phase 1.5: Recover any interrupted commits from previous crash
      const recoveryResult = yield* recoverPendingCommits({
        tasksPath,
        cwd: config.cwd,
        emit: (event: RecoveryEvent) => {
          if (event.type === "recovery_start" && event.pendingCount > 0) {
            emit({ type: "recovery_start", pendingCount: event.pendingCount });
          } else if (event.type === "task_closed") {
            emit({ type: "recovery_task_closed", taskId: event.taskId, sha: event.sha });
          } else if (event.type === "task_reset") {
            emit({ type: "recovery_task_reset", taskId: event.taskId });
          }
        },
      }).pipe(Effect.catchAll(() => Effect.succeed({ closed: [], reset: [], failed: [] })));

      if (recoveryResult.closed.length > 0 || recoveryResult.reset.length > 0) {
        emit({
          type: "recovery_complete",
          closedCount: recoveryResult.closed.length,
          resetCount: recoveryResult.reset.length,
          failedCount: recoveryResult.failed.length,
        });
      }

      // Phase 2: Select Task
      state.phase = "selecting_task";

      // Use pre-assigned task if provided (parallel runner), otherwise pick next ready task
      let taskResult: ReturnType<typeof pickNextTask> extends Effect.Effect<infer T, any, any> ? T : never;
      if (config.task) {
        taskResult = config.task;
      } else {
        taskResult = yield* durableStep(
          stepResultsManager,
          "select_task",
          () =>
            pickNextTask(tasksPath).pipe(
              Effect.catchAll((error) => Effect.fail(new Error(`No ready tasks: ${error}`)))
            ),
          { inputHash: tasksPath }
        );
      }

      if (!taskResult) {
        state.phase = "done";
        progress.nextSession.suggestedNextSteps = ["No tasks available - add new tasks"];
        writeProgress(openagentsDir, progress);
        yield* recordUsage();
        emit({ type: "session_complete", success: true, summary: "No tasks to process" });
        yield* stepResultsManager.clear();
        return state;
      }

      state.task = taskResult;
      progress.taskId = taskResult.id;
      progress.taskTitle = taskResult.title;
      emit({ type: "task_selected", task: taskResult });

      // Write checkpoint after task selection
      const gitState = yield* captureGitState(config.cwd).pipe(
        Effect.catchAll(() => Effect.succeed({
          branch: "unknown",
          headCommit: "unknown",
          isDirty: false,
          stagedFiles: [] as string[],
        }))
      );

      currentCheckpoint = createCheckpoint({
        sessionId,
        phase: "selecting_task",
        taskId: taskResult.id,
        taskTitle: taskResult.title,
        completedSubtaskIds: [],
        currentSubtaskId: null,
        git: gitState,
      });
      yield* saveCheckpoint(openagentsDir, currentCheckpoint, emit);

      // Phase 3: Decompose
      state.phase = "decomposing";

      // Read existing subtasks unless forceNewSubtasks is set (for parallel runs)
      let subtaskList = config.forceNewSubtasks
        ? null
        : readSubtasks(openagentsDir, taskResult.id);

      if (!subtaskList) {
        // Create new subtask list with intelligent decomposition
        const subtaskOptions =
          config.maxSubtasksPerTask !== undefined ? { maxSubtasks: config.maxSubtasksPerTask } : undefined;
        subtaskList = yield* durableStep(
          stepResultsManager,
          "decompose_task",
          () => Effect.succeed(createSubtaskList(taskResult, subtaskOptions)),
          { inputHash: taskResult.id }
        );
      }

      // If typecheck is failing at start, inject a "fix typecheck" subtask at the beginning
      if (!progress.orientation.testsPassingAtStart) {
        const fixTypecheckSubtaskId = `${taskResult.id}-fix-typecheck`;
        const existingFixSubtask = subtaskList.subtasks.find(s => s.id === fixTypecheckSubtaskId);

        if (!existingFixSubtask) {
          const fixTypecheckSubtask: Subtask = {
            id: fixTypecheckSubtaskId,
            description: `## CRITICAL: Fix Typecheck Errors First

The codebase has typecheck errors that MUST be fixed before any other work can proceed.

Run \`bun run typecheck\` to see the errors, then fix them.

Common causes:
- Unused imports or variables (TS6133)
- Type mismatches (TS2322)
- Missing or extra arguments (TS2554)

After fixing, verify with \`bun run typecheck\` that it passes before proceeding.`,
            status: "pending",
          };
          // Insert at the beginning
          subtaskList.subtasks.unshift(fixTypecheckSubtask);
          subtaskList.updatedAt = new Date().toISOString();

          // Clear session resumption for all other subtasks - they need fresh context
          // after typecheck is fixed, not stale sessions from before the fix
          for (const s of subtaskList.subtasks) {
            if (s.id !== fixTypecheckSubtaskId && s.claudeCode?.sessionId) {
              s.claudeCode.resumeStrategy = "fork";
            }
          }
        }
      }

      state.subtasks = subtaskList;
      writeSubtasks(openagentsDir, subtaskList);
      emit({ type: "task_decomposed", subtasks: subtaskList.subtasks });

      // Update checkpoint after decomposition
      if (currentCheckpoint) {
        currentCheckpoint = updateCheckpointPhase(currentCheckpoint, "decomposing");
        yield* saveCheckpoint(openagentsDir, currentCheckpoint, emit);
      }

      // Phase 4: Execute Subtasks
      state.phase = "executing_subtask";

      for (const subtask of subtaskList.subtasks) {
        if (subtask.status === "done" || subtask.status === "verified") {
          continue; // Skip completed subtasks
        }

        subtask.status = "in_progress";
        subtask.startedAt = new Date().toISOString();
        progress.work.subtasksInProgress.push(subtask.id);
        emit({ type: "subtask_start", subtask });

        // Fetch reflections if Reflexion is enabled
        let reflectionsText: string | undefined;
        let recentReflections: ReflectionType[] = [];
        if (config.reflectionService && config.reflexionConfig?.enabled !== false) {
          recentReflections = yield* loadRecentReflections(subtask.id);
          if (recentReflections.length > 0 && (subtask.failureCount ?? 0) > 0) {
            reflectionsText = yield* config.reflectionService.formatForPrompt(recentReflections).pipe(
              Effect.catchAll(() => Effect.succeed(undefined))
            );
          }
        }

        const result: SubagentResult = yield* subagentRunner({
          subtask,
          cwd: config.cwd,
          openagentsDir,
          tools: SUBAGENT_TOOLS,
          ...(config.subagentModel ? { model: config.subagentModel } : {}),
          ...(config.claudeCode ? { claudeCode: config.claudeCode } : {}),
          verificationCommands,
          verifyFn: (commands, cwd) =>
            Effect.runPromise(
              runVerification(commands, cwd, emit, sandboxRunnerConfig).pipe(
                Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] }))
              )
            ),
          ...(config.signal ? { signal: config.signal } : {}),
          ...(config.onOutput ? { onOutput: config.onOutput } : {}),
          ...(config.additionalContext ? { additionalContext: config.additionalContext } : {}),
          ...(reflectionsText ? { reflections: reflectionsText } : {}),
        }).pipe(
          Effect.catchAll((error) =>
            Effect.succeed({
              success: false,
              subtaskId: subtask.id,
              filesModified: [],
              error: error.message,
              turns: 0,
            })
          )
        );

        progress.work.filesModified.push(...result.filesModified);

        // Accumulate usage metrics for the session
        if (result.sessionMetadata?.usage || result.tokenUsage) {
          usageAccumulator.inputTokens += result.sessionMetadata?.usage?.inputTokens ?? result.tokenUsage?.input ?? 0;
          usageAccumulator.outputTokens += result.sessionMetadata?.usage?.outputTokens ?? result.tokenUsage?.output ?? 0;
          usageAccumulator.cacheReadTokens += result.sessionMetadata?.usage?.cacheReadInputTokens ?? 0;
          usageAccumulator.cacheCreationTokens += result.sessionMetadata?.usage?.cacheCreationInputTokens ?? 0;
        }
        if (result.sessionMetadata?.totalCostUsd !== undefined) {
          usageAccumulator.totalCostUsd += result.sessionMetadata.totalCostUsd;
        }
        usageAccumulator.subtasks += 1;
        if (result.agent) {
          usageAccumulator.agents.add(result.agent);
        }

        // Persist Claude Code session metadata for resumption across runs
        const previousClaudeState = subtask.claudeCode ?? {};
        if (result.claudeCodeSessionId || previousClaudeState.sessionId) {
          const updatedClaudeState: NonNullable<Subtask["claudeCode"]> = { ...previousClaudeState };

          if (result.claudeCodeSessionId) {
            updatedClaudeState.sessionId = result.claudeCodeSessionId;
          } else if (previousClaudeState.sessionId) {
            updatedClaudeState.sessionId = previousClaudeState.sessionId;
          }

          if (result.claudeCodeForkedFromSessionId) {
            updatedClaudeState.forkedFromSessionId = result.claudeCodeForkedFromSessionId;
          } else if (previousClaudeState.forkedFromSessionId) {
            updatedClaudeState.forkedFromSessionId = previousClaudeState.forkedFromSessionId;
          }

          if (previousClaudeState.resumeStrategy === "fork" && result.claudeCodeSessionId) {
            updatedClaudeState.resumeStrategy = "continue";
          } else if (previousClaudeState.resumeStrategy) {
            updatedClaudeState.resumeStrategy = previousClaudeState.resumeStrategy;
          }

          subtask.claudeCode = updatedClaudeState;
        }

        // Capture Claude Code session metadata for context bridging
        if (result.sessionMetadata) {
          const sessionId =
            result.sessionMetadata.sessionId ?? result.claudeCodeSessionId ?? subtask.claudeCode?.sessionId;
          const forkedFromSessionId =
            result.sessionMetadata.forkedFromSessionId ??
            result.claudeCodeForkedFromSessionId ??
            subtask.claudeCode?.forkedFromSessionId;

          progress.work.claudeCodeSession = {
            ...(sessionId ? { sessionId } : {}),
            ...(forkedFromSessionId ? { forkedFromSessionId } : {}),
            ...(result.sessionMetadata.toolsUsed ? { toolsUsed: result.sessionMetadata.toolsUsed } : {}),
            ...(result.sessionMetadata.summary ? { summary: result.sessionMetadata.summary } : {}),
            ...(result.sessionMetadata.usage ? { usage: result.sessionMetadata.usage } : {}),
            ...(result.sessionMetadata.totalCostUsd !== undefined
              ? { totalCostUsd: result.sessionMetadata.totalCostUsd }
              : {}),
          };

          // Merge session blockers into next session blockers
          if (result.sessionMetadata.blockers && result.sessionMetadata.blockers.length > 0) {
            progress.nextSession.blockers = [
              ...(progress.nextSession.blockers || []),
              ...result.sessionMetadata.blockers,
            ];
          }

          // Merge suggested next steps
          if (result.sessionMetadata.suggestedNextSteps && result.sessionMetadata.suggestedNextSteps.length > 0) {
            progress.nextSession.suggestedNextSteps.push(...result.sessionMetadata.suggestedNextSteps);
          }
        }

        if (result.success) {
          subtask.status = "done";
          subtask.completedAt = new Date().toISOString();
          // Clear any previous error from failed attempts
          delete subtask.error;
          progress.work.subtasksCompleted.push(subtask.id);
          progress.work.subtasksInProgress = progress.work.subtasksInProgress.filter(
            (id) => id !== subtask.id
          );
          emit({ type: "subtask_complete", subtask, result });

          // Update checkpoint with completed subtask
          if (currentCheckpoint) {
            currentCheckpoint = updateCheckpointPhase(currentCheckpoint, "executing_subtask", {
              completedSubtaskIds: [...currentCheckpoint.completedSubtaskIds, subtask.id],
              currentSubtaskId: null,
            });
            yield* saveCheckpoint(openagentsDir, currentCheckpoint, emit);
          }
        } else {
          // Track consecutive failures
          const MAX_CONSECUTIVE_FAILURES = 3;
          subtask.failureCount = (subtask.failureCount ?? 0) + 1;
          subtask.lastFailureReason = result.error || "Unknown error";
          subtask.status = "failed";
          if (result.error) {
            subtask.error = result.error;
          }
          emit({ type: "subtask_failed", subtask, error: result.error || "Unknown error" });

          // Reflexion: Generate a reflection on the failure
          if (config.reflectionService && config.reflexionConfig?.enabled !== false) {
            const failureContext: FailureContextType = {
              id: `fc-${Date.now().toString(36)}`,
              sessionId: state.sessionId,
              taskId: taskResult.id,
              subtaskId: subtask.id,
              subtaskDescription: subtask.description.slice(0, 1000),
              attemptNumber: subtask.failureCount ?? 1,
              failureType: result.verificationOutputs?.some(o => o.includes("typecheck"))
                ? "typecheck_failure"
                : result.verificationOutputs?.some(o => o.includes("test"))
                ? "test_failure"
                : "runtime_error",
              errorOutput: (result.error || "").slice(0, 2000),
              filesModified: result.filesModified || [],
              previousReflections: mapReflectionsToNotes(recentReflections, maxReflectionsPerRetry),
              createdAt: new Date().toISOString(),
            };

            const reflection = yield* config.reflectionService.generate(failureContext).pipe(
              Effect.catchAll((e) => {
                console.log(`[Reflexion] Failed to generate reflection: ${e instanceof Error ? e.message : e}`);
                return Effect.succeed(null);
              })
            );

            if (reflection) {
              yield* config.reflectionService.save(reflection).pipe(
                Effect.catchAll(() => Effect.void)
              );
              console.log(`[Reflexion] Generated reflection for attempt ${reflection.attemptNumber}: ${reflection.analysis.slice(0, 100)}...`);
            }
          }

          // Healer: Attempt to fix subtask failures
          if (config.healerService && config.projectConfig) {
            const healerOutcome = yield* config.healerService.maybeRun(
              { type: "subtask_failed", subtask, error: result.error || "Unknown error" },
              state,
              config.projectConfig,
              config.healerCounters ?? createHealerCounters()
            );

            if (healerOutcome?.status === "resolved") {
              // Healer fixed the issue! Reset failure count and retry
              subtask.failureCount = 0;
              subtask.status = "pending";
              delete subtask.error;
              delete subtask.lastFailureReason;
              // Continue to next iteration to retry this subtask
              continue;
            }
          }

          // Check if we've exceeded max consecutive failures
          if (subtask.failureCount >= MAX_CONSECUTIVE_FAILURES) {
            // Mark the parent task as blocked to prevent infinite loops
            state.phase = "failed";
            state.error = `Task blocked after ${MAX_CONSECUTIVE_FAILURES} consecutive failures: ${result.error}`;
            progress.nextSession.blockers = [
              `Task failed ${MAX_CONSECUTIVE_FAILURES} times consecutively`,
              `Last error: ${result.error || "Unknown"}`,
              "Task marked as blocked - requires manual intervention",
            ];
            progress.nextSession.suggestedNextSteps = [
              "Review the task requirements and errors",
              "Fix underlying issues before retrying",
              "Consider breaking the task into smaller pieces",
            ];

            // Mark the task as blocked in tasks.jsonl
            yield* updateTask({
              tasksPath,
              id: taskResult.id,
              update: {
                status: "blocked",
                closeReason: `Blocked after ${MAX_CONSECUTIVE_FAILURES} consecutive failures: ${result.error}`,
              },
            }).pipe(Effect.catchAll(() => Effect.void));

            writeProgress(openagentsDir, progress);
            writeSubtasks(openagentsDir, subtaskList);
            yield* recordUsage();
            yield* clearCheckpoint(openagentsDir);
            emit({ type: "session_complete", success: false, summary: `Task blocked after ${MAX_CONSECUTIVE_FAILURES} failures` });
            yield* stepResultsManager.clear();
            return state;
          }

          // Stop on failure, but allow retry next cycle (under MAX_CONSECUTIVE_FAILURES)
          state.phase = "failed";
          if (result.error) {
            state.error = result.error;
          }
          const blockers: string[] = [
            `Failure ${subtask.failureCount}/${MAX_CONSECUTIVE_FAILURES}: ${result.error || "Subtask failed"}`,
          ];
          const verificationHint = summarizeOutput(result.verificationOutputs?.[0]);
          if (verificationHint) {
            blockers.push(verificationHint);
          }
          progress.nextSession.blockers = blockers;

          // Clear session for fresh start on next attempt (fork instead of continue)
          if (subtask.claudeCode) {
            subtask.claudeCode.resumeStrategy = "fork";
          }

          writeProgress(openagentsDir, progress);
          writeSubtasks(openagentsDir, subtaskList);
          yield* recordUsage();
          yield* clearCheckpoint(openagentsDir);
          yield* stepResultsManager.clear();
          return state;
        }

        // Update subtasks file after each subtask
        writeSubtasks(openagentsDir, subtaskList);
      }

      // Phase 5: Verify
      state.phase = "verifying";
      progress.work.testsRun = true;

      const verifyResult = yield* runVerification(
        verificationCommands,
        config.cwd,
        emit,
        sandboxRunnerConfig
      ).pipe(Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] })));

      progress.work.testsPassingAfterWork = verifyResult.passed;

      if (!verifyResult.passed) {
        // Healer: Attempt to fix verification failures
        if (config.healerService && config.projectConfig) {
          const healerOutcome = yield* config.healerService.maybeRun(
            {
              type: "verification_complete",
              command: verificationCommands.join(" && "),
              passed: false,
              output: verifyResult.outputs[0] ?? "",
            },
            state,
            config.projectConfig,
            config.healerCounters ?? createHealerCounters()
          );

          if (healerOutcome?.status === "resolved") {
            // Healer fixed the issue! Re-run verification
            const retryResult = yield* runVerification(
              verificationCommands,
              config.cwd,
              emit,
              sandboxRunnerConfig
            ).pipe(Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] })));

            if (retryResult.passed) {
              // Verification now passes! Update progress and continue
              progress.work.testsPassingAfterWork = true;
              // Continue to commit phase
            } else {
              // Still failing after heal - proceed with failure
              yield* recordVerificationReflection(retryResult.outputs[0], "verification_failed");
              state.phase = "failed";
              state.error = "Verification failed (after healing attempt)";
              progress.nextSession.blockers = [
                "Tests or typecheck failed after changes (healing attempted)",
                ...(retryResult.outputs[0] ? [summarizeOutput(retryResult.outputs[0]) ?? ""] : []),
              ].filter(Boolean) as string[];
              progress.nextSession.suggestedNextSteps = ["Fix failing tests/typecheck", "Review changes"];
              writeProgress(openagentsDir, progress);
              yield* recordUsage();
              yield* clearCheckpoint(openagentsDir);
              emit({ type: "session_complete", success: false, summary: "Verification failed after healing" });
              yield* stepResultsManager.clear();
              return state;
            }
          } else {
            // Healer didn't resolve or wasn't triggered - proceed with failure
            yield* recordVerificationReflection(verifyResult.outputs[0], "verification_failed");
            state.phase = "failed";
            state.error = "Verification failed";
            progress.nextSession.blockers = [
              "Tests or typecheck failed after changes",
              ...(verifyResult.outputs[0] ? [summarizeOutput(verifyResult.outputs[0]) ?? ""] : []),
            ].filter(Boolean) as string[];
            progress.nextSession.suggestedNextSteps = ["Fix failing tests/typecheck", "Review changes"];
            writeProgress(openagentsDir, progress);
            yield* recordUsage();
            yield* clearCheckpoint(openagentsDir);
            emit({ type: "session_complete", success: false, summary: "Verification failed" });
            yield* stepResultsManager.clear();
            return state;
          }
        } else {
          // No Healer configured - proceed with failure
          yield* recordVerificationReflection(verifyResult.outputs[0], "verification_failed");
          state.phase = "failed";
          state.error = "Verification failed";
          progress.nextSession.blockers = [
            "Tests or typecheck failed after changes",
            ...(verifyResult.outputs[0] ? [summarizeOutput(verifyResult.outputs[0]) ?? ""] : []),
          ].filter(Boolean) as string[];
          progress.nextSession.suggestedNextSteps = ["Fix failing tests/typecheck", "Review changes"];
          writeProgress(openagentsDir, progress);
          yield* recordUsage();
          yield* clearCheckpoint(openagentsDir);
          emit({ type: "session_complete", success: false, summary: "Verification failed" });
          yield* stepResultsManager.clear();
          return state;
        }
      }

      // Run e2e commands when configured and task requires them
      const effectiveE2eCommands = buildE2eCommands(config.e2eCommands);
      const shouldRunE2eTests =
        effectiveE2eCommands.length > 0 && state.task && shouldRunE2e(state.task.labels ?? []);

      if (shouldRunE2eTests) {
        progress.work.e2eRun = true;
        const e2eResult = yield* runE2eOnHost(effectiveE2eCommands, config.cwd, emit).pipe(
          Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] }))
        );
        progress.work.e2ePassingAfterWork = e2eResult.passed;

        if (!e2eResult.passed) {
          yield* recordVerificationReflection(e2eResult.outputs[0], "test_failure");
          state.phase = "failed";
          state.error = "E2E failed";
          progress.nextSession.blockers = [
            "E2E tests failed after changes",
            ...(e2eResult.outputs[0] ? [summarizeOutput(e2eResult.outputs[0]) ?? ""] : []),
          ].filter(Boolean) as string[];
          progress.nextSession.suggestedNextSteps = ["Fix failing e2e tests", "Review changes"];
          writeProgress(openagentsDir, progress);
          yield* recordUsage();
          yield* clearCheckpoint(openagentsDir);
          emit({ type: "session_complete", success: false, summary: "E2E failed" });
          yield* stepResultsManager.clear();
          return state;
        }
      } else {
        emit({
          type: "e2e_skipped",
          reason:
            effectiveE2eCommands.length === 0
              ? "No e2eCommands configured"
              : "Task labels do not require e2e",
        });
      }

      // Update checkpoint after verification passes
      if (currentCheckpoint) {
        currentCheckpoint = updateCheckpointPhase(currentCheckpoint, "verifying", {
          verification: {
            typecheckPassed: true,
            testsPassed: true,
            verifiedAt: new Date().toISOString(),
          },
        });
        yield* saveCheckpoint(openagentsDir, currentCheckpoint, emit);
      }

      // Phase 6: Two-Phase Commit
      // Phase 6a: Mark task as commit_pending BEFORE creating git commit
      // This ensures we can recover if a crash occurs between git commit and task update
      state.phase = "committing";
      const commitMessage = taskResult.title;
      const commitBranch = yield* Effect.tryPromise({
        try: async () => {
          const { execSync } = await import("node:child_process");
          return execSync("git rev-parse --abbrev-ref HEAD", {
            cwd: config.cwd,
            encoding: "utf-8",
          }).trim();
        },
        catch: () => new Error("Failed to get branch"),
      });

      yield* updateTask({
        tasksPath,
        id: taskResult.id,
        update: {
          status: "commit_pending",
          pendingCommit: {
            message: commitMessage,
            timestamp: new Date().toISOString(),
            branch: commitBranch,
          },
        },
      }).pipe(Effect.catchAll(() => Effect.void));

      // Phase 6b: Create git commit
      const stagePaths = Array.from(
        new Set([
          ...progress.work.filesModified.map((file) => file.trim()).filter((file) => file.length > 0),
          openagentsDir,
        ])
      );
      const sha = yield* createCommit(taskResult.id, commitMessage, config.cwd, stagePaths);
      emit({ type: "commit_created", sha, message: commitMessage });

      // Update pending commit with SHA (for crash recovery verification)
      yield* updateTask({
        tasksPath,
        id: taskResult.id,
        update: {
          pendingCommit: {
            message: commitMessage,
            timestamp: new Date().toISOString(),
            branch: commitBranch,
            sha,
          },
        },
      }).pipe(Effect.catchAll(() => Effect.void));

      // Phase 7: Push (if allowed) - idempotent, safe to retry
      if (config.allowPush) {
        yield* pushToRemote(commitBranch, config.cwd);
        emit({ type: "push_complete", branch: commitBranch });
      }

      // Phase 8: Update Task - Complete the two-phase commit
      state.phase = "updating_task";
      yield* updateTask({
        tasksPath,
        id: taskResult.id,
        update: {
          status: "closed",
          closeReason: "Completed by MechaCoder orchestrator",
          pendingCommit: null, // Clear the pending commit
        },
        appendCommits: [sha],
      }).pipe(Effect.catchAll(() => Effect.void));

      emit({ type: "task_updated", task: taskResult, status: "closed" });

      // Phase 9: Log
      state.phase = "logging";
      progress.completedAt = new Date().toISOString();
      progress.nextSession.suggestedNextSteps = ["Pick next task"];
      writeProgress(openagentsDir, progress);
      emit({ type: "progress_written", path: getProgressPath(openagentsDir) });

      state.phase = "done";
      yield* recordUsage();

      // Clear checkpoint on successful completion
      yield* clearCheckpoint(openagentsDir);
      emit({ type: "checkpoint_cleared" });

      // Clear memoized step results on success
      yield* stepResultsManager.clear();

      emit({
        type: "session_complete",
        success: true,
        summary: `Completed task ${taskResult.id}: ${taskResult.title}`,
      });

      return state;
    } catch (error: any) {
      state.phase = "failed";
      state.error = error.message;
      emit({ type: "error", phase: state.phase, error: error.message });

      progress.nextSession.blockers = [error.message];
      writeProgress(openagentsDir, progress);
      yield* recordUsage();

      // Clear checkpoint on failed completion (checkpoint is for crash recovery, not failed runs)
      yield* clearCheckpoint(openagentsDir);
      // Also clear memoized step results to avoid stale replays on next run
      yield* stepResultsManager.clear();

      return state;
    }
  });
