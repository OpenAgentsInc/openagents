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

/**
 * Run verification commands (typecheck, tests)
 */
const runVerification = (
  testCommands: string[],
  cwd: string,
  emit: (event: OrchestratorEvent) => void
): Effect.Effect<{ passed: boolean; outputs: string[] }, Error, OpenRouterClient> =>
  Effect.gen(function* () {
    const outputs: string[] = [];
    let allPassed = true;

    for (const cmd of testCommands) {
      emit({ type: "verification_start", command: cmd });

      // Run the command via bash tool
      const result = yield* Effect.tryPromise({
        try: async () => {
          const { execSync } = await import("node:child_process");
          const output = execSync(cmd, {
            cwd,
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
            timeout: 120000, // 2 minute timeout
          });
          return { passed: true, output };
        },
        catch: (error: any) => {
          return { passed: false, output: error.stdout || error.stderr || error.message };
        },
      });

      const passed = typeof result === "object" && "passed" in result ? result.passed : false;
      const output = typeof result === "object" && "output" in result ? String(result.output) : String(result);

      outputs.push(output);
      emit({ type: "verification_complete", command: cmd, passed, output });

      if (!passed) {
        allPassed = false;
      }
    }

    return { passed: allPassed, outputs };
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
  emit: (event: OrchestratorEvent) => void = () => {}
): Effect.Effect<OrchestratorState, Error, FileSystem.FileSystem | Path.Path | OpenRouterClient> =>
  Effect.gen(function* () {
    const sessionId = generateSessionId();
    const now = new Date().toISOString();
    
    emit({ type: "session_start", sessionId, timestamp: now });

    const state: OrchestratorState = {
      sessionId,
      task: null,
      subtasks: null,
      progress: null,
      phase: "orienting",
    };

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
      progress.orientation.previousSessionSummary = getPreviousSessionSummary(config.openagentsDir) || undefined;

      const initScriptResult = yield* runInitScript(
        config.openagentsDir,
        config.cwd,
        emit
      );
      progress.orientation.initScript = initScriptResult;

      if (initScriptResult.ran && !initScriptResult.success) {
        state.phase = "failed";
        state.error = "Init script failed";
        progress.orientation.repoState = "init script failed";
        progress.orientation.testsPassingAtStart = false;

        emit({
          type: "orientation_complete",
          repoState: progress.orientation.repoState,
          testsPassingAtStart: progress.orientation.testsPassingAtStart,
          initScript: initScriptResult,
        });

        progress.nextSession.blockers = [
          "Init script failed",
          ...(summarizeOutput(initScriptResult.output) ? [summarizeOutput(initScriptResult.output)!] : []),
        ];
        progress.nextSession.suggestedNextSteps = [
          "Inspect .openagents/init.sh output",
          "Fix init script errors before rerunning",
        ];

        writeProgress(config.openagentsDir, progress);
        emit({ type: "session_complete", success: false, summary: "Init script failed" });
        return state;
      }

      // Quick test check
      if (config.testCommands.length > 0) {
        const testResult = yield* runVerification(
          [config.testCommands[0]],
          config.cwd,
          emit
        ).pipe(Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] })));
        progress.orientation.testsPassingAtStart = testResult.passed;
      } else {
        progress.orientation.testsPassingAtStart = true;
      }

      progress.orientation.repoState = "clean";
      emit({
        type: "orientation_complete",
        repoState: progress.orientation.repoState,
        testsPassingAtStart: progress.orientation.testsPassingAtStart,
        initScript: initScriptResult,
      });

      // Phase 2: Select Task
      state.phase = "selecting_task";
      const taskResult = yield* pickNextTask(config.cwd).pipe(
        Effect.catchAll((error) => Effect.fail(new Error(`No ready tasks: ${error}`)))
      );

      if (!taskResult) {
        state.phase = "done";
        progress.nextSession.suggestedNextSteps = ["No tasks available - add new tasks"];
        writeProgress(config.openagentsDir, progress);
        emit({ type: "session_complete", success: true, summary: "No tasks to process" });
        return state;
      }

      state.task = taskResult;
      progress.taskId = taskResult.id;
      progress.taskTitle = taskResult.title;
      emit({ type: "task_selected", task: taskResult });

      // Phase 3: Decompose
      state.phase = "decomposing";
      let subtaskList = readSubtasks(config.openagentsDir, taskResult.id);
      
      if (!subtaskList) {
        // Create new subtask list with intelligent decomposition
        const subtaskOptions =
          config.maxSubtasksPerTask !== undefined ? { maxSubtasks: config.maxSubtasksPerTask } : undefined;
        subtaskList = createSubtaskList(taskResult, subtaskOptions);
      }

      state.subtasks = subtaskList;
      writeSubtasks(config.openagentsDir, subtaskList);
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

        const result: SubagentResult = yield* runBestAvailableSubagent({
          subtask,
          cwd: config.cwd,
          openagentsDir: config.openagentsDir ?? config.cwd,
          tools: SUBAGENT_TOOLS,
          ...(config.subagentModel ? { model: config.subagentModel } : {}),
          claudeCode: config.claudeCode,
          signal: config.signal,
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

        if (result.success) {
          subtask.status = "done";
          subtask.completedAt = new Date().toISOString();
          progress.work.subtasksCompleted.push(subtask.id);
          progress.work.subtasksInProgress = progress.work.subtasksInProgress.filter(
            (id) => id !== subtask.id
          );
          emit({ type: "subtask_complete", subtask, result });
        } else {
          subtask.status = "failed";
          subtask.error = result.error;
          emit({ type: "subtask_failed", subtask, error: result.error || "Unknown error" });
          
          // Stop on first failure
          state.phase = "failed";
          state.error = result.error;
          progress.nextSession.blockers = [result.error || "Subtask failed"];
          writeProgress(config.openagentsDir, progress);
          writeSubtasks(config.openagentsDir, subtaskList);
          return state;
        }

        // Update subtasks file after each subtask
        writeSubtasks(config.openagentsDir, subtaskList);
      }

      // Phase 5: Verify
      state.phase = "verifying";
      progress.work.testsRun = true;
      
      const verifyResult = yield* runVerification(
        config.testCommands,
        config.cwd,
        emit
      ).pipe(Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] })));

      progress.work.testsPassingAfterWork = verifyResult.passed;

      if (!verifyResult.passed) {
        state.phase = "failed";
        state.error = "Verification failed";
        progress.nextSession.blockers = ["Tests failed after changes"];
        progress.nextSession.suggestedNextSteps = ["Fix failing tests", "Review changes"];
        writeProgress(config.openagentsDir, progress);
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
        tasksPath: `${config.cwd}/.openagents/tasks.jsonl`,
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
      writeProgress(config.openagentsDir, progress);
      emit({ type: "progress_written", path: getProgressPath(config.openagentsDir) });

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
      writeProgress(config.openagentsDir, progress);
      
      return state;
    }
  });
