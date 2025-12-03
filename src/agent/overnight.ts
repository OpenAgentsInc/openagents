#!/usr/bin/env bun
/**
 * Overnight Agent - Long-running autonomous coding agent
 * 
 * Usage: bun src/agent/overnight.ts --dir ~/code/some-repo [--max-tasks 5] [--dry-run]
 * 
 * The agent will:
 * 1. Check for ready tasks in .openagents/tasks.jsonl
 * 2. Claim the highest priority task
 * 3. Read relevant files and implement the fix
 * 4. Run tests
 * 5. Commit and push to main
 * 6. Close the task
 * 7. Repeat until no more tasks or max reached
 * 
 * Logs are saved to ~/code/openagents/docs/logs/YYYYMMDD/
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
import type { OrchestratorEvent } from "./orchestrator/types.js";
import { loadProjectConfig } from "../tasks/project.js";

// Logging setup
const OPENAGENTS_ROOT = "/Users/christopherdavid/code/openagents";
const getLogDir = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return path.join(OPENAGENTS_ROOT, "docs", "logs", `${year}${month}${day}`);
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
}

const parseArgs = (): OvernightConfig => {
  const args = process.argv.slice(2);
  let workDir = process.cwd();
  let maxTasks = 10;
  let dryRun = false;
  let legacy = false;
  let ccOnly = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--dir" && args[i + 1]) {
      workDir = args[i + 1].startsWith("~")
        ? args[i + 1].replace("~", process.env.HOME || "")
        : args[i + 1];
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
    
    // Initialize logging
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
    const loadedConfig = yield* loadProjectConfig(openagentsDir).pipe(
      Effect.catchAll(() => Effect.succeed(null))
    );
    const projectConfig = loadedConfig ?? defaultConfig;

    // Initialize logging
    const sessionId = `orchestrator-${Date.now()}`;
    initLog(sessionId);

    log(`${"#".repeat(60)}`);
    log("OVERNIGHT AGENT STARTING - Orchestrator Mode");
    log(`Session: ${sessionId}`);
    log(`Work directory: ${config.workDir}`);
    log(`Max tasks: ${config.maxTasks}`);
    log(`Claude Code enabled: ${projectConfig.claudeCode?.enabled ?? true}`);
    log(`${"#".repeat(60)}\n`);

    // Change to work directory
    process.chdir(config.workDir);
    log(`Changed to directory: ${process.cwd()}`);

    let tasksCompleted = 0;

    // Event handler for logging
    const emit = (event: OrchestratorEvent) => {
      const ts = new Date().toISOString();
      switch (event.type) {
        case "session_start":
          log(`[${ts}] Orchestrator session started: ${event.sessionId}`);
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
        ...(projectConfig.typecheckCommands && { typecheckCommands: [...projectConfig.typecheckCommands] }),
        // Stream Claude Code output to console
        onOutput: (text: string) => process.stdout.write(text),
      };

      const state = yield* runOrchestrator(orchestratorConfig, emit).pipe(
        Effect.catchAll((error) => {
          log(`Orchestrator error: ${error.message}`);
          return Effect.succeed({ phase: "failed" as const, error: error.message });
        })
      );

      if (state.phase === "done") {
        tasksCompleted++;
        log(`\n✓ Task ${tasksCompleted} completed`);
      } else if (state.phase === "failed") {
        log(`\n✗ Task failed: ${state.error || "Unknown error"}`);
        // Continue to next task unless it's a critical failure
        if (state.error?.includes("No ready tasks")) {
          log("No more ready tasks available");
          break;
        }
      } else {
        log(`\n✗ Task incomplete: phase=${state.phase}`);
        break;
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

    // Final cleanup commit - commit any remaining progress/log files
    try {
      const { execSync } = require("node:child_process") as typeof import("node:child_process");
      const status = execSync("git status --porcelain", { cwd: config.workDir, encoding: "utf-8" });
      if (status.trim()) {
        log("Committing remaining progress files...");
        execSync("git add -A", { cwd: config.workDir, encoding: "utf-8" });
        const commitMsg = `chore: update progress files and logs

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>`;
        execSync(`git commit -m "${commitMsg.replace(/"/g, '\\"')}"`, { cwd: config.workDir, encoding: "utf-8" });
        execSync("git push", { cwd: config.workDir, encoding: "utf-8" });
        log("Progress files committed and pushed.");
      }
    } catch (e) {
      // Ignore commit errors (might be nothing to commit)
    }

    return { tasksCompleted, sessionId };
  });

// Main
const config = parseArgs();

if (!config.workDir) {
  console.error("Usage: bun src/agent/overnight.ts --dir <work-directory> [--max-tasks N] [--dry-run] [--legacy]");
  process.exit(1);
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
