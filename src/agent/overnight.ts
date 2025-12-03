#!/usr/bin/env bun
/**
 * Overnight Agent - Long-running autonomous coding agent
 *
 * Usage: bun src/agent/overnight.ts [--cwd ~/code/some-repo] [--max-tasks 5] [--dry-run] [--cc-only] [--safe-mode] [--load-context]
 *
 * Options:
 *   --cwd, --dir    Target repo directory (default: current directory)
 *   --max-tasks     Maximum tasks to complete (default: 10)
 *   --dry-run       Print what would happen without executing
 *   --cc-only       Use Claude Code only (no Grok fallback)
 *   --legacy        Use legacy Grok-based agentLoop
 *   --safe-mode     Enable self-healing for init script failures (typecheck errors, etc.)
 *   --load-context  Load AGENTS.md/CLAUDE.md context files from working directory
 *
 * The agent will:
 * 1. Check for ready tasks in <cwd>/.openagents/tasks.jsonl
 * 2. Claim the highest priority task
 * 3. Read relevant files and implement the fix
 * 4. Run tests
 * 5. Commit and push to main
 * 6. Close the task
 * 7. Repeat until no more tasks or max reached
 *
 * Safe Mode:
 * When enabled, if the init script fails with typecheck errors, the agent will
 * spawn a Claude Code emergency subtask to fix the errors before continuing.
 *
 * Logs are saved to <cwd>/docs/logs/YYYYMMDD/
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect, Layer } from "effect";
import * as fs from "node:fs";
import * as path from "node:path";
import { agentLoop } from "./loop.js";
import { GIT_CONVENTIONS } from "./prompts.js";
import { readTool } from "../tools/read.js";
import { editTool } from "../tools/edit.js";
import { bashTool } from "../tools/bash.js";
import { writeTool } from "../tools/write.js";
import { openRouterLive } from "../llm/openrouter.js";
import {
  createSession,
  writeSessionStart,
  writeUserMessage,
  writeTurn,
  writeSessionEnd,
  getSessionPath,
} from "./session.js";
import { runOrchestrator } from "./orchestrator/orchestrator.js";
import type { OrchestratorEvent, OrchestratorState } from "./orchestrator/types.js";
import { loadProjectConfig } from "../tasks/project.js";
import { createHudCallbacks, sendAPMSnapshot, createAPMEmitter } from "../hud/index.js";
import { acquireLock, releaseLock } from "./orchestrator/agent-lock.js";
import { loadContextFiles } from "../cli/context-loader.js";
import { APMCollector } from "./apm.js";
import { parseProjectConversations } from "./apm-parser.js";

/**
 * Generate a descriptive commit message based on orchestrator state
 * Accepts either full OrchestratorState or minimal error state from catchAll
 */
type PartialOrchestratorState = Partial<OrchestratorState> & { phase: string; error?: string };

const generateCommitMessage = (state: PartialOrchestratorState, cwd: string): string => {
  const { execSync } = require("node:child_process") as typeof import("node:child_process");

  // Get task info
  const taskId = state.task?.id ?? "unknown";
  const taskTitle = state.task?.title ?? "task cycle";
  const shortId = taskId.replace("oa-", "");

  // Determine prefix based on success/failure
  const prefix = state.phase === "done" ? "feat" : "wip";

  // Get changed files summary
  let filesSummary = "";
  try {
    const diffStat = execSync("git diff --cached --stat --stat-width=50", { cwd, encoding: "utf-8" });
    const lines = diffStat.trim().split("\n");
    if (lines.length > 1) {
      // Last line is the summary like "3 files changed, 10 insertions(+), 2 deletions(-)"
      filesSummary = lines[lines.length - 1].trim();
    }
  } catch {
    // Ignore errors
  }

  // Get subtask summary
  let subtaskSummary = "";
  if (state.subtasks?.subtasks) {
    const completed = state.subtasks.subtasks.filter(s => s.status === "done" || s.status === "verified").length;
    const total = state.subtasks.subtasks.length;
    if (total > 0) {
      subtaskSummary = `Subtasks: ${completed}/${total} completed`;
    }
  }

  // Build the message
  const lines = [
    `${prefix}(${shortId}): ${taskTitle}`,
    "",
  ];

  if (state.phase === "failed" && state.error) {
    lines.push(`Status: Failed - ${state.error}`);
  } else if (state.phase === "done") {
    lines.push("Status: Completed successfully");
  }

  if (subtaskSummary) {
    lines.push(subtaskSummary);
  }

  if (filesSummary) {
    lines.push(`Changes: ${filesSummary}`);
  }

  lines.push("");
  lines.push("🤖 Generated with [OpenAgents](https://openagents.com)");
  lines.push("");
  lines.push("Co-Authored-By: MechaCoder <noreply@openagents.com>");

  return lines.join("\n");
};

// Logging setup - uses workDir for logs, not hardcoded path
let _logWorkDir = process.cwd();

const getLogDir = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return path.join(_logWorkDir, "docs", "logs", `${year}${month}${day}`);
};

const getLogPath = (sessionId: string) => {
  const now = new Date();
  const hours = String(now.getHours()).padStart(2, "0");
  const mins = String(now.getMinutes()).padStart(2, "0");
  return path.join(getLogDir(), `${hours}${mins}-overnight-${sessionId}.md`);
};

let logFilePath: string | null = null;
let logBuffer: string[] = [];

const tools = [readTool, editTool, bashTool, writeTool];

const OVERNIGHT_SYSTEM_PROMPT = `You are an autonomous coding agent working overnight to complete tasks.

${GIT_CONVENTIONS}

## Your Workflow

1. **Check tasks**: Tasks are loaded from .openagents/tasks.jsonl by the agent loop
2. **Understand the task**: Read the task description, read relevant source files
3. **Implement**: Make necessary code changes using edit tool
4. **Test**: Run relevant tests with bash (bun test, etc.)
5. **Commit**: Stage changes and commit with proper format:
   \`\`\`
   git add -A && git commit -m "$(cat <<'EOF'
   Your message here
   
   🤖 Generated with [OpenAgents](https://openagents.com)
   
   Co-Authored-By: MechaCoder <noreply@openagents.com>
   EOF
   )"
   \`\`\`
6. **Push**: Run \`git push origin main\`
7. **Complete**: Report "TASK_COMPLETED: <task-id>" when done

## Important Rules

- ALWAYS run tests before committing
- NEVER force push
- NEVER commit secrets or credentials
- If tests fail, fix them before committing
- If stuck, report the blocking reason
- Work on ONE task at a time
- Keep commits focused and atomic

## Current Task

Work in the current directory on the task provided.
`;

interface OvernightConfig {
  workDir: string;
  maxTasks: number;
  dryRun: boolean;
  sessionsDir: string;
  /** Use legacy Grok-based agentLoop instead of orchestrator */
  legacy: boolean;
  /** Force Claude Code only - no Grok fallback */
  ccOnly: boolean;
  /** Enable self-healing for init script failures */
  safeMode: boolean;
  /** Load AGENTS.md/CLAUDE.md context from working directory */
  loadContext: boolean;
}

const parseArgs = (): OvernightConfig => {
  const args = process.argv.slice(2);
  let workDir = process.cwd();
  let maxTasks = 10;
  let dryRun = false;
  let legacy = false;
  let ccOnly = false;
  let safeMode = false;
  let loadContext = false;

  for (let i = 0; i < args.length; i++) {
    if ((args[i] === "--dir" || args[i] === "--cwd") && args[i + 1]) {
      workDir = args[i + 1].startsWith("~")
        ? args[i + 1].replace("~", process.env.HOME || "")
        : path.resolve(args[i + 1]);
      i++;
    } else if ((args[i] === "--max-tasks" || args[i] === "--max-beads") && args[i + 1]) {
      maxTasks = parseInt(args[i + 1], 10);
      i++;
    } else if (args[i] === "--dry-run") {
      dryRun = true;
    } else if (args[i] === "--legacy") {
      legacy = true;
    } else if (args[i] === "--cc-only") {
      ccOnly = true;
    } else if (args[i] === "--safe-mode") {
      safeMode = true;
    } else if (args[i] === "--load-context") {
      loadContext = true;
    }
  }

  // Use .openagents/sessions instead of .agent-sessions
  return {
    workDir,
    maxTasks,
    dryRun,
    sessionsDir: `${workDir}/.openagents/sessions`,
    legacy,
    ccOnly,
    safeMode,
    loadContext,
  };
};

const initLog = (sessionId: string) => {
  const logDir = getLogDir();
  if (!fs.existsSync(logDir)) {
    fs.mkdirSync(logDir, { recursive: true });
  }
  logFilePath = getLogPath(sessionId);
  logBuffer = [`# Overnight Agent Log\n`, `Session: ${sessionId}\n`, `Started: ${new Date().toISOString()}\n\n`];
  fs.writeFileSync(logFilePath, logBuffer.join(""));
};

const log = (msg: string) => {
  const ts = new Date().toISOString();
  const line = `[${ts}] ${msg}`;
  console.log(line);
  
  if (logFilePath) {
    fs.appendFileSync(logFilePath, line + "\n");
  }
};

const runTaskCycle = (
  config: OvernightConfig,
  taskNumber: number,
): Effect.Effect<{ completed: boolean; message: string }, Error, any> =>
  Effect.gen(function* () {
    log(`\n${"=".repeat(60)}`);
    log(`TASK CYCLE ${taskNumber}/${config.maxTasks}`);
    log(`Working directory: ${config.workDir}`);
    log(`${"=".repeat(60)}\n`);

    const prompt = taskNumber === 1
      ? `You are starting a new work session in ${config.workDir}.

Tasks are loaded from .openagents/tasks.jsonl by the agent loop.
Read the relevant files to understand what needs to be done.
Implement the changes, run tests, commit, push, and report completion.

Start now.`
      : `Continue working on the next task.
If there are no more ready tasks, respond with "NO_MORE_TASKS".
Otherwise, complete the next highest priority task.`;

    if (config.dryRun) {
      log("[DRY RUN] Would send prompt:");
      log(prompt);
      return { completed: false, message: "Dry run - no action taken" };
    }

    const result = yield* agentLoop(prompt, tools as any, {
      systemPrompt: OVERNIGHT_SYSTEM_PROMPT,
      maxTurns: 20,
      model: "x-ai/grok-4.1-fast:free",
    });

    log(`\nCompleted in ${result.totalTurns} turn(s)`);

    for (const turn of result.turns) {
      if (turn.content) {
        log(`\nAssistant: ${turn.content.slice(0, 200)}${turn.content.length > 200 ? "..." : ""}`);
      }
      if (turn.toolCalls) {
        for (const call of turn.toolCalls) {
          log(`\nTool: ${call.name}`);
          log(`Args: ${call.arguments.slice(0, 100)}${call.arguments.length > 100 ? "..." : ""}`);
        }
      }
      if (turn.toolResults) {
        for (const res of turn.toolResults) {
          const status = res.isError ? "ERROR" : "SUCCESS";
          log(`Result (${res.name}): ${status}`);
        }
      }
    }

    const finalMessage = result.finalMessage || "";
    const noMoreTasks = finalMessage.includes("NO_MORE_TASKS") || 
                        finalMessage.toLowerCase().includes("no more tasks") ||
                        finalMessage.toLowerCase().includes("no ready tasks");

    if (noMoreTasks) {
      return { completed: false, message: "No more tasks available" };
    }

    return { completed: true, message: finalMessage };
  });

const overnightLoop = (config: OvernightConfig) =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    yield* Path.Path; // needed for getSessionPath

    // Ensure sessions directory exists
    const sessionsDirExists = yield* fs.exists(config.sessionsDir).pipe(
      Effect.catchAll(() => Effect.succeed(false)),
    );
    if (!sessionsDirExists) {
      yield* fs.makeDirectory(config.sessionsDir, { recursive: true }).pipe(
        Effect.catchAll(() => Effect.void),
      );
    }

    // Create session
    const session = createSession(
      { model: "x-ai/grok-4.1-fast:free", systemPrompt: OVERNIGHT_SYSTEM_PROMPT, maxTurns: 25 },
      "Overnight agent session",
    );

    // Initialize logging (set log directory to workDir)
    _logWorkDir = config.workDir;
    initLog(session.id);
    
    const sessionPath = yield* getSessionPath(config.sessionsDir, session.id);

    yield* writeSessionStart(sessionPath, session).pipe(Effect.catchAll(() => Effect.void));
    yield* writeUserMessage(sessionPath, "Starting overnight loop").pipe(Effect.catchAll(() => Effect.void));

    log(`${"#".repeat(60)}`);
    log("OVERNIGHT AGENT STARTING");
    log(`Session: ${session.id}`);
    log(`Log file: ${logFilePath}`);
    log(`Work directory: ${config.workDir}`);
    log(`Max tasks: ${config.maxTasks}`);
    log(`Dry run: ${config.dryRun}`);
    log(`${"#".repeat(60)}\n`);

    // Change to work directory
    process.chdir(config.workDir);
    log(`Changed to directory: ${process.cwd()}`);

    let tasksCompleted = 0;
    let continueLoop = true;

    while (continueLoop && tasksCompleted < config.maxTasks) {
      try {
        const result = yield* runTaskCycle(config, tasksCompleted + 1).pipe(
          Effect.catchAll((error) => 
            Effect.succeed({ completed: false, message: `Error: ${error.message}` })
          ),
        );

        yield* writeTurn(sessionPath, {
          role: "assistant",
          content: result.message,
        }).pipe(Effect.catchAll(() => Effect.void));

        if (result.completed) {
          tasksCompleted++;
          log(`\n✓ Task ${tasksCompleted} completed`);
        } else {
          log(`\n✗ Stopping: ${result.message}`);
          continueLoop = false;
        }

        // Small delay between tasks
        if (continueLoop) {
          yield* Effect.sleep(2000);
        }
      } catch (error) {
        log(`\n✗ Error: ${error}`);
        continueLoop = false;
      }
    }

    yield* writeSessionEnd(sessionPath, tasksCompleted, `Completed ${tasksCompleted} tasks`).pipe(
      Effect.catchAll(() => Effect.void),
    );

    log(`\n${"#".repeat(60)}`);
    log("OVERNIGHT AGENT FINISHED");
    log(`Tasks completed: ${tasksCompleted}`);
    log(`Session saved: ${sessionPath}`);
    log(`${"#".repeat(60)}\n`);

    return { tasksCompleted, sessionId: session.id };
  });

/**
 * Orchestrator-based overnight loop (Claude Code primary, Grok fallback).
 * This is the new default path for overnight runs.
 */
const overnightLoopOrchestrator = (config: OvernightConfig) =>
  Effect.gen(function* () {
    const openagentsDir = path.join(config.workDir, ".openagents");

    // Load project config with defaults
    const defaultConfig = {
      projectId: "unknown",
      defaultBranch: "main",
      testCommands: ["bun test"],
      typecheckCommands: ["bun run typecheck"],
      allowPush: true,
      claudeCode: {
        enabled: true,
        preferForComplexTasks: true,
        fallbackToMinimal: true,
      },
    };
    // Note: loadProjectConfig expects the root dir, not the .openagents dir
    const loadedConfig = yield* loadProjectConfig(config.workDir).pipe(
      Effect.catchAll(() => Effect.succeed(null))
    );
    const projectConfig = loadedConfig ?? defaultConfig;

    // Initialize logging (set log directory to workDir)
    _logWorkDir = config.workDir;
    const sessionId = `orchestrator-${Date.now()}`;
    initLog(sessionId);

    log(`${"#".repeat(60)}`);
    log("OVERNIGHT AGENT STARTING - Orchestrator Mode");
    log(`Session: ${sessionId}`);
    log(`Work directory: ${config.workDir}`);
    log(`Max tasks: ${config.maxTasks}`);
    log(`Claude Code enabled: ${projectConfig.claudeCode?.enabled ?? true}`);
    log(`Safe mode: ${config.safeMode}`);
    log(`${"#".repeat(60)}\n`);

    // Change to work directory
    process.chdir(config.workDir);
    log(`Changed to directory: ${process.cwd()}`);

    // Acquire agent lock to prevent concurrent runs
    const lockResult = acquireLock(openagentsDir, sessionId);
    if (!lockResult.acquired && lockResult.reason === "already_running") {
      log(`ERROR: Another agent is already running (PID ${lockResult.existingLock.pid})`);
      log(`Lock acquired at: ${lockResult.existingLock.timestamp}`);
      if (lockResult.existingLock.sessionId) {
        log(`Session: ${lockResult.existingLock.sessionId}`);
      }
      log("\nTo force remove the lock (if the process is not actually running):");
      log(`  rm -f ${openagentsDir}/agent.lock`);
      return { tasksCompleted: 0, sessionId };
    }
    if (lockResult.acquired) {
      log(`Lock acquired (PID ${lockResult.lock.pid})`);
    } else if (lockResult.reason === "stale_removed") {
      log(`WARNING: Removed stale lock from PID ${lockResult.removedLock.pid}`);
      log(`New lock acquired (PID ${lockResult.newLock.pid})`);
    }

    let tasksCompleted = 0;
    let consecutiveFailures = 0;
    let lastFailureReason = "";
    const MAX_CONSECUTIVE_FAILURES = 3;

    // Load AGENTS.md/CLAUDE.md context if --load-context is enabled
    let additionalContext: string | undefined;
    if (config.loadContext) {
      const contextFiles = loadContextFiles(config.workDir);
      if (contextFiles.length > 0) {
        additionalContext = contextFiles.join("\n\n---\n\n");
        log(`Loaded ${contextFiles.length} context file(s) (${additionalContext.length} chars)`);
      } else {
        log("No context files found (AGENTS.md/CLAUDE.md)");
      }
    }

    // Create HUD callbacks for real-time updates to the desktop HUD
    // These silently fail if the HUD isn't running
    const { emit: hudEmit, onOutput: hudOnOutput, client: hudClient } = createHudCallbacks();

    // Create APM collector for tracking actions per minute
    const projectName = path.basename(config.workDir);
    const apmCollector = new APMCollector(sessionId, projectName);
    const apmEmit = createAPMEmitter(hudClient, apmCollector);

    // Send initial APM snapshot with historical data
    yield* parseProjectConversations(config.workDir).pipe(
      Effect.tap((stats) => {
        sendAPMSnapshot(hudClient, stats);
        log(`[APM] Lifetime: ${stats.combined.apmLifetime.toFixed(2)} | MechaCoder: ${stats.mechaCoder.apmLifetime.toFixed(2)} vs Claude Code: ${stats.claudeCode.apmLifetime.toFixed(2)}`);
      }),
      Effect.catchAll(() => Effect.void), // Ignore errors, APM is optional
    );

    // Event handler for logging (also forwards to HUD + APM tracking)
    const emit = (event: OrchestratorEvent) => {
      // Forward to HUD for real-time UI updates
      hudEmit(event);

      // Track APM-relevant events
      switch (event.type) {
        case "subtask_start":
          apmCollector.recordAction("message");
          break;
        case "subtask_complete":
          // Each subtask completion represents significant tool usage
          apmCollector.recordAction("tool_call", event.result.agent);
          apmEmit(); // Send periodic APM update
          break;
        case "verification_start":
        case "commit_created":
          apmCollector.recordAction("tool_call", event.type);
          break;
      }

      // Log to file/console
      const ts = new Date().toISOString();
      switch (event.type) {
        case "session_start":
          log(`[${ts}] Orchestrator session started: ${event.sessionId}`);
          break;
        case "lock_acquired":
          log(`[${ts}] Lock acquired (PID ${event.pid})`);
          break;
        case "lock_stale_removed":
          log(`[${ts}] Stale lock removed (was PID ${event.stalePid}, now PID ${event.newPid})`);
          break;
        case "lock_failed":
          log(`[${ts}] Lock failed: ${event.reason} (existing PID: ${event.existingPid})`);
          break;
        case "lock_released":
          log(`[${ts}] Lock released`);
          break;
        case "task_selected":
          log(`[${ts}] Task selected: ${event.task.id} - ${event.task.title}`);
          break;
        case "subtask_start":
          log(`[${ts}] Subtask started: ${event.subtask.id}`);
          break;
        case "subtask_complete":
          log(`[${ts}] Subtask complete: ${event.subtask.id} (agent: ${event.result.agent})`);
          break;
        case "subtask_failed":
          log(`[${ts}] Subtask FAILED: ${event.subtask.id} - ${event.error}`);
          break;
        case "verification_start":
          log(`[${ts}] Running: ${event.command}`);
          break;
        case "verification_complete":
          log(`[${ts}] ${event.passed ? "PASS" : "FAIL"}: ${event.command}`);
          break;
        case "commit_created":
          log(`[${ts}] Commit: ${event.sha.slice(0, 8)} - ${event.message}`);
          break;
        case "push_complete":
          log(`[${ts}] Pushed to ${event.branch}`);
          break;
        case "session_complete":
          log(`[${ts}] Session ${event.success ? "SUCCESS" : "FAILED"}: ${event.summary}`);
          break;
        case "error":
          log(`[${ts}] ERROR in ${event.phase}: ${event.error}`);
          break;
      }
    };

    // Run orchestrator loop for multiple tasks
    for (let taskNum = 0; taskNum < config.maxTasks; taskNum++) {
      log(`\n${"=".repeat(60)}`);
      log(`TASK CYCLE ${taskNum + 1}/${config.maxTasks}`);
      log(`${"=".repeat(60)}\n`);

      if (config.dryRun) {
        log("[DRY RUN] Would run orchestrator");
        break;
      }

      // Build claudeCode config, applying --cc-only override if specified
      const claudeCodeConfig = config.ccOnly
        ? {
            ...projectConfig.claudeCode,
            enabled: true,
            preferForComplexTasks: false, // Use CC for ALL tasks
            fallbackToMinimal: false, // No Grok fallback
          }
        : projectConfig.claudeCode;

      const orchestratorConfig = {
        cwd: config.workDir,
        openagentsDir,
        testCommands: [...(projectConfig.testCommands ?? ["bun test"])],
        allowPush: projectConfig.allowPush ?? true,
        claudeCode: claudeCodeConfig,
        safeMode: config.safeMode,
        ...(projectConfig.typecheckCommands && { typecheckCommands: [...projectConfig.typecheckCommands] }),
        // Stream Claude Code output to console AND HUD
        onOutput: (text: string) => {
          process.stdout.write(text);
          hudOnOutput(text);
        },
        // Pass additional context if --load-context was specified
        ...(additionalContext ? { additionalContext } : {}),
      };

      const state: PartialOrchestratorState = yield* runOrchestrator(orchestratorConfig, emit).pipe(
        Effect.catchAll((error) => {
          log(`Orchestrator error: ${error.message}`);
          return Effect.succeed({ phase: "failed" as const, error: error.message } as PartialOrchestratorState);
        })
      );

      // Determine if we made meaningful progress (picked a task)
      const taskWasPicked = state.task !== null && state.task !== undefined;
      const didMeaningfulWork = taskWasPicked && (
        (state.phase === "done") ||
        (state.subtasks?.subtasks?.some(s => s.status === "done" || s.status === "in_progress"))
      );

      if (state.phase === "done") {
        tasksCompleted++;
        consecutiveFailures = 0; // Reset on success
        lastFailureReason = "";
        log(`\n✓ Task ${tasksCompleted} completed`);
      } else if (state.phase === "failed") {
        const currentError = state.error || "Unknown error";
        log(`\n✗ Task failed: ${currentError}`);

        // GUARDRAIL: Revert uncommitted changes on failure to prevent broken code
        // from being committed in cleanup. This ensures failed work doesn't persist.
        // See docs/mechacoder/GOLDEN-LOOP-v2.md Section 4.3 "Failed Subtask Cleanup Guardrails"
        try {
          const { execSync } = require("node:child_process") as typeof import("node:child_process");
          const status = execSync("git status --porcelain", { cwd: config.workDir, encoding: "utf-8" });
          if (status.trim()) {
            log("[Guardrail] Reverting uncommitted changes from failed subtask...");
            // Only revert tracked files - leave untracked files alone
            execSync("git checkout -- .", { cwd: config.workDir, encoding: "utf-8" });
            // Also clean up any new untracked files that were created
            // Use -f (force) and -d (directories) but NOT -x (keep .gitignore'd files)
            execSync("git clean -fd", { cwd: config.workDir, encoding: "utf-8" });
            log("[Guardrail] Uncommitted changes reverted.");
          }
        } catch (e) {
          log(`[Guardrail] Warning: Failed to revert changes: ${e}`);
        }

        // Check if this is a pre-task failure (init script, no tasks, etc.)
        const isPreTaskFailure = !taskWasPicked;

        // Track consecutive failures - especially for pre-task failures
        if (isPreTaskFailure || currentError === lastFailureReason) {
          consecutiveFailures++;
          log(`[Guardrail] Consecutive failure ${consecutiveFailures}/${MAX_CONSECUTIVE_FAILURES}`);
        } else {
          // Different error on a picked task - reset counter but record the error
          consecutiveFailures = 1;
        }
        lastFailureReason = currentError;

        // Stop if we've had too many consecutive failures
        if (consecutiveFailures >= MAX_CONSECUTIVE_FAILURES) {
          log(`[Guardrail] STOPPING: ${MAX_CONSECUTIVE_FAILURES} consecutive failures without progress`);
          log(`[Guardrail] Last error: ${currentError}`);
          break;
        }

        // Also stop for specific terminal errors
        if (state.error?.includes("No ready tasks")) {
          log("No more ready tasks available");
          break;
        }
      } else {
        log(`\n✗ Task incomplete: phase=${state.phase}`);
        break;
      }

      // GUARDRAIL: Only commit when meaningful work was done
      // This prevents empty commits from pre-task failures (init script, etc.)
      if (!didMeaningfulWork) {
        log("[Guardrail] Skipping commit - no meaningful work done this cycle");
        continue;
      }

      // Commit and push pending changes only after meaningful work
      try {
        const { execSync } = require("node:child_process") as typeof import("node:child_process");
        const status = execSync("git status --porcelain", { cwd: config.workDir, encoding: "utf-8" });
        if (status.trim()) {
          log("[Cycle cleanup] Committing pending changes...");
          execSync("git add -A", { cwd: config.workDir, encoding: "utf-8" });
          const commitMsg = generateCommitMessage(state, config.workDir);
          // Use heredoc to handle multiline commit message properly
          execSync(`git commit -m "$(cat <<'COMMITMSG'\n${commitMsg}\nCOMMITMSG\n)"`, {
            cwd: config.workDir,
            encoding: "utf-8",
            shell: "/bin/bash"
          });
          execSync("git push", { cwd: config.workDir, encoding: "utf-8" });
          log("[Cycle cleanup] Changes committed and pushed.");
        }
      } catch (e) {
        // Ignore commit errors (might be nothing to commit)
      }

      // Small delay between tasks
      if (taskNum < config.maxTasks - 1) {
        yield* Effect.sleep(2000);
      }
    }

    log(`\n${"#".repeat(60)}`);
    log("OVERNIGHT AGENT FINISHED - Orchestrator Mode");
    log(`Tasks completed: ${tasksCompleted}`);
    log(`${"#".repeat(60)}\n`);

    // Close HUD client connection
    hudClient.close();

    // Final cleanup commit - commit ONLY progress/log files, NOT any code changes
    // This is a guardrail to prevent accidentally committing broken code from failed subtasks
    // See docs/mechacoder/GOLDEN-LOOP-v2.md Section 4.3 "Failed Subtask Cleanup Guardrails"
    // Note: Use console.log not log() to avoid writing to the file we're about to commit
    try {
      const { execSync } = require("node:child_process") as typeof import("node:child_process");
      // Only add specific paths - progress files and logs, NOT all files
      // This prevents broken code from failed subtasks from being committed
      execSync("git add .openagents/progress.md .openagents/subtasks/ docs/logs/ 2>/dev/null || true", {
        cwd: config.workDir,
        encoding: "utf-8",
        shell: "/bin/bash"
      });
      // Check if we staged anything
      const staged = execSync("git diff --cached --name-only", { cwd: config.workDir, encoding: "utf-8" });
      if (staged.trim()) {
        console.log("[Cleanup] Committing remaining progress files...");
        const commitMsg = `chore: update progress files and logs

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>`;
        execSync(`git commit -m "${commitMsg.replace(/"/g, '\\"')}"`, { cwd: config.workDir, encoding: "utf-8" });
        execSync("git push", { cwd: config.workDir, encoding: "utf-8" });
        console.log("[Cleanup] Progress files committed and pushed.");
      }
    } catch (e) {
      // Ignore commit errors (might be nothing to commit)
    }

    // Release agent lock
    const released = releaseLock(openagentsDir);
    if (released) {
      console.log("[Cleanup] Agent lock released.");
    }

    return { tasksCompleted, sessionId };
  });

// Main
const config = parseArgs();

// workDir defaults to process.cwd() if not specified
if (!config.workDir) {
  config.workDir = process.cwd();
}

const liveLayer = Layer.mergeAll(openRouterLive, BunContext.layer);

// Route based on --legacy flag
const program = config.legacy
  ? overnightLoop(config) // Legacy: Grok-based agentLoop
  : overnightLoopOrchestrator(config); // Default: Orchestrator with Claude Code

if (config.legacy) {
  console.log("[Legacy mode: Grok-based agentLoop]");
} else if (config.ccOnly) {
  console.log("[Orchestrator mode: Claude Code ONLY - no Grok fallback]");
} else {
  console.log("[Orchestrator mode: Claude Code primary, Grok fallback]");
}
if (config.safeMode) {
  console.log("[Safe mode: Self-healing enabled for init script failures]");
}
if (config.loadContext) {
  console.log("[Context loading: Will load AGENTS.md/CLAUDE.md from working directory]");
}

Effect.runPromise((program as any).pipe(Effect.provide(liveLayer)))
  .then((result: unknown) => {
    const r = result as { tasksCompleted: number; sessionId: string };
    console.log(`\nDone! Completed ${r.tasksCompleted} tasks.`);
    process.exit(0);
  })
  .catch((error: unknown) => {
    console.error("Fatal error:", error);
    process.exit(1);
  });
