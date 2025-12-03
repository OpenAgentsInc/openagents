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
import { Effect } from "effect";
import { pickNextTask, updateTask } from "../../tasks/index.js";
import type { OpenRouterClient } from "../../llm/openrouter.js";
import { readTool } from "../../tools/read.js";
import { editTool } from "../../tools/edit.js";
import { bashTool } from "../../tools/bash.js";
import { writeTool } from "../../tools/write.js";
import { runBestAvailableSubagent } from "./subagent-router.js";
import { runInitScript } from "./init-script.js";
import { runClaudeCodeSubagent } from "./claude-code-subagent.js";
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
  testCommands: string[]
): string[] => [...(typecheckCommands ?? []), ...testCommands];

/**
 * Run verification commands (typecheck, tests)
 */
const runVerification = (
  commands: string[],
  cwd: string,
  emit: (event: OrchestratorEvent) => void
): Effect.Effect<{ passed: boolean; outputs: string[] }, Error, never> =>
  Effect.try({
    try: () => {
      const outputs: string[] = [];
      let allPassed = true;

      for (const cmd of commands) {
        emit({ type: "verification_start", command: cmd });
        try {
          const { execSync } = require("node:child_process") as typeof import("node:child_process");
          const output = execSync(cmd, {
            cwd,
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
            timeout: 120000,
          });
          outputs.push(String(output));
          emit({ type: "verification_complete", command: cmd, passed: true, output: String(output) });
        } catch (error: any) {
          const output = String(error?.stdout || error?.stderr || error?.message || error);
          outputs.push(output);
          emit({ type: "verification_complete", command: cmd, passed: false, output });
          allPassed = false;
        }
      }

      return { passed: allPassed, outputs };
    },
    catch: (error: any) => error as Error,
  });

/**
 * Create a git commit with the task ID
 */
const createCommit = (
  taskId: string,
  message: string,
  cwd: string
): Effect.Effect<string, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      const { execSync } = await import("node:child_process");
      
      // Stage all changes
      execSync("git add -A", { cwd, encoding: "utf-8" });
      
      // Create commit
      const fullMessage = `${taskId}: ${message}

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>`;

      execSync(`git commit -m "${fullMessage.replace(/"/g, '\\"')}"`, {
        cwd,
        encoding: "utf-8",
      });
      
      // Get commit SHA
      const sha = execSync("git rev-parse HEAD", { cwd, encoding: "utf-8" }).trim();
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
    const sessionId = generateSessionId();
    const now = new Date().toISOString();
    const subagentRunner = deps?.runSubagent ?? runBestAvailableSubagent;
    const openagentsDir = config.openagentsDir ?? `${config.cwd}/.openagents`;
    const tasksPath = `${openagentsDir}/tasks.jsonl`;
    
    emit({ type: "session_start", sessionId, timestamp: now });

    const state: OrchestratorState = {
      sessionId,
      task: null,
      subtasks: null,
      progress: null,
      phase: "orienting",
    };
    const verificationCommands = buildVerificationCommands(
      config.typecheckCommands,
      config.testCommands
    );

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
      },
      nextSession: {
        suggestedNextSteps: [],
      },
    };

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
        initScriptResult = yield* runInitScript(openagentsDir, config.cwd, emit);
      }
      progress.orientation.initScript = initScriptResult;

      // Safe mode: Attempt self-healing for recoverable failures
      if (initScriptResult.ran && !initScriptResult.success && config.safeMode && initScriptResult.canSelfHeal) {
        emit({ type: "error", phase: "orienting", error: `Init script failed with ${initScriptResult.failureType}, attempting self-heal...` });

        // Create emergency subtask for self-healing
        const healingSubtask: Subtask = {
          id: `emergency-${initScriptResult.failureType}-fix`,
          description: initScriptResult.failureType === "typecheck_failed"
            ? `## EMERGENCY: Fix All TypeScript Errors

The init script failed due to typecheck errors. You MUST fix ALL typecheck errors immediately.

Init script output:
\`\`\`
${initScriptResult.output?.slice(0, 2000) ?? "No output available"}
\`\`\`

Steps:
1. Run \`bun run typecheck\` to see all errors
2. Fix each error - do NOT just suppress them
3. Verify fix with \`bun run typecheck\`
4. All errors must be resolved before continuing`
            : initScriptResult.failureType === "test_failed"
            ? `## EMERGENCY: Fix Failing Tests

The init script failed due to test failures. You MUST fix ALL failing tests.

Init script output:
\`\`\`
${initScriptResult.output?.slice(0, 2000) ?? "No output available"}
\`\`\`

Steps:
1. Run \`bun test\` to see all failing tests
2. Fix the root cause of each failure
3. Verify fix with \`bun test\`
4. All tests must pass before continuing`
            : `## EMERGENCY: Fix Init Script Error

The init script failed. Fix the issue.

Init script output:
\`\`\`
${initScriptResult.output?.slice(0, 2000) ?? "No output available"}
\`\`\``,
          status: "in_progress",
          startedAt: new Date().toISOString(),
        };

        emit({ type: "subtask_start", subtask: healingSubtask });

        // Run Claude Code to fix the issue
        const healResult = yield* Effect.tryPromise({
          try: () => runClaudeCodeSubagent(healingSubtask, {
            cwd: config.cwd,
            openagentsDir,
            maxTurns: 50,
            permissionMode: config.claudeCode?.permissionMode ?? "bypassPermissions",
            ...(config.onOutput ? { onOutput: config.onOutput } : {}),
            ...(config.signal ? { signal: config.signal } : {}),
          }),
          catch: (e: any) => new Error(`Self-healing failed: ${e.message}`),
        }).pipe(
          Effect.catchAll((error) => Effect.succeed({
            success: false,
            subtaskId: healingSubtask.id,
            filesModified: [],
            turns: 0,
            error: error.message,
          }))
        );

        if (healResult.success) {
          emit({ type: "subtask_complete", subtask: healingSubtask, result: healResult });

          // Re-run init script to verify the fix
          const retryResult = yield* runInitScript(openagentsDir, config.cwd, emit);

          if (retryResult.success) {
            // Self-healing succeeded! Update the init script result and continue
            initScriptResult = retryResult;
            progress.orientation.initScript = retryResult;
            emit({ type: "error", phase: "orienting", error: `Self-healing succeeded for ${healingSubtask.id}` });
          } else {
            // Self-healing fixed something but init still fails
            emit({ type: "subtask_failed", subtask: healingSubtask, error: "Self-healing completed but init script still fails" });
          }
        } else {
          emit({ type: "subtask_failed", subtask: healingSubtask, error: healResult.error || "Self-healing failed" });
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
        emit({ type: "session_complete", success: false, summary: state.error });
        return state;
      }

      // Quick test check
      if (verificationCommands.length > 0) {
        const testResult = yield* runVerification(
          [verificationCommands[0]],
          config.cwd,
          emit
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

      // Phase 2: Select Task
      state.phase = "selecting_task";
      const taskResult = yield* pickNextTask(tasksPath).pipe(
        Effect.catchAll((error) => Effect.fail(new Error(`No ready tasks: ${error}`)))
      );

      if (!taskResult) {
        state.phase = "done";
        progress.nextSession.suggestedNextSteps = ["No tasks available - add new tasks"];
        writeProgress(openagentsDir, progress);
        emit({ type: "session_complete", success: true, summary: "No tasks to process" });
        return state;
      }

      state.task = taskResult;
      progress.taskId = taskResult.id;
      progress.taskTitle = taskResult.title;
      emit({ type: "task_selected", task: taskResult });

      // Phase 3: Decompose
      state.phase = "decomposing";
      let subtaskList = readSubtasks(openagentsDir, taskResult.id);
      
      if (!subtaskList) {
        // Create new subtask list with intelligent decomposition
        const subtaskOptions =
          config.maxSubtasksPerTask !== undefined ? { maxSubtasks: config.maxSubtasksPerTask } : undefined;
        subtaskList = createSubtaskList(taskResult, subtaskOptions);
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
              runVerification(commands, cwd, emit).pipe(
                Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] }))
              )
            ),
          ...(config.signal ? { signal: config.signal } : {}),
          ...(config.onOutput ? { onOutput: config.onOutput } : {}),
          ...(config.additionalContext ? { additionalContext: config.additionalContext } : {}),
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
            emit({ type: "session_complete", success: false, summary: `Task blocked after ${MAX_CONSECUTIVE_FAILURES} failures` });
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
        emit
      ).pipe(Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] })));

      progress.work.testsPassingAfterWork = verifyResult.passed;

      if (!verifyResult.passed) {
        state.phase = "failed";
        state.error = "Verification failed";
        progress.nextSession.blockers = [
          "Tests or typecheck failed after changes",
          ...(verifyResult.outputs[0] ? [summarizeOutput(verifyResult.outputs[0]) ?? ""] : []),
        ].filter(Boolean) as string[];
        progress.nextSession.suggestedNextSteps = ["Fix failing tests/typecheck", "Review changes"];
        writeProgress(openagentsDir, progress);
        emit({ type: "session_complete", success: false, summary: "Verification failed" });
        return state;
      }

      // Phase 6: Commit
      state.phase = "committing";
      const commitMessage = taskResult.title;
      const sha = yield* createCommit(taskResult.id, commitMessage, config.cwd);
      emit({ type: "commit_created", sha, message: commitMessage });

      // Phase 7: Push (if allowed)
      if (config.allowPush) {
        const branch = yield* Effect.tryPromise({
          try: async () => {
            const { execSync } = await import("node:child_process");
            return execSync("git rev-parse --abbrev-ref HEAD", {
              cwd: config.cwd,
              encoding: "utf-8",
            }).trim();
          },
          catch: () => new Error("Failed to get branch"),
        });

        yield* pushToRemote(branch, config.cwd);
        emit({ type: "push_complete", branch });
      }

      // Phase 8: Update Task
      state.phase = "updating_task";
      yield* updateTask({
        tasksPath,
        id: taskResult.id,
        update: {
          status: "closed",
          closeReason: "Completed by MechaCoder orchestrator",
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
      
      return state;
    }
  });
