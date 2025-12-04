#!/usr/bin/env bun
/**
 * Do One Task - Picks up ONE task from .openagents/tasks.jsonl, completes it, commits, pushes, exits.
 * Designed to be run by cron/launchd every few minutes.
 * 
 * Usage: bun src/agent/do-one-task.ts --dir ~/code/some-repo
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect, Layer } from "effect";
import * as fs from "node:fs";
import * as nodePath from "node:path";
import { agentLoop, type AgentTurn } from "./loop.js";
import { GIT_CONVENTIONS } from "./prompts.js";
import {
  pickNextTask,
  updateTask,
  type Task,
  type TaskFilter,
  TaskServiceError,
} from "../tasks/index.js";
import { loadProjectConfig } from "../tasks/project.js";
import { readTool } from "../tools/read.js";
import { editTool } from "../tools/edit.js";
import { bashTool } from "../tools/bash.js";
import { writeTool } from "../tools/write.js";
import { openRouterLive } from "../llm/openrouter.js";
import {
  createRunMetadata,
  writeRunLog,
  appendRunEventSync,
  generateRunId,
  nowTs,
  type TaskRunEvent,
} from "./runLog.js";
import {
  createSession,
  writeSessionStart,
  writeUserMessage,
  writeTurn,
  writeSessionEnd,
  getSessionPath,
} from "./session.js";
import { runOrchestrator, type OrchestratorEvent, runClaudeCodeSubagent } from "./orchestrator/index.js";
import { createHudCallbacks } from "../hud/index.js";
import type { Subtask } from "./orchestrator/types.js";
import { createHealerService } from "../healer/service.js";
import { createHealerCounters } from "../healer/types.js";
import type { ProjectConfig } from "../tasks/schema.js";

const tools = [readTool, editTool, bashTool, writeTool];

// Helper to detect typecheck/test failures from tool results
interface ToolResultInfo {
  name: string;
  text: string;
  isError: boolean;
}

const extractToolResults = (turns: Array<{ toolResults?: Array<{ name: string; result: { content: Array<{ type: string; text?: string }> }; isError: boolean }> }>): ToolResultInfo[] => {
  const results: ToolResultInfo[] = [];
  for (const turn of turns) {
    if (turn.toolResults) {
      for (const res of turn.toolResults) {
        const text = res.result.content
          .filter((c): c is { type: "text"; text: string } => c.type === "text")
          .map(c => c.text)
          .join("\n");
        results.push({ name: res.name, text, isError: res.isError });
      }
    }
  }
  return results;
};

const detectTypecheckFailure = (toolResults: ToolResultInfo[]): { failed: boolean; errors: string } => {
  for (const res of toolResults) {
    if (res.name === "bash" && res.text.includes("bun run typecheck")) {
      // Check if the output contains TS errors
      if (res.text.includes("error TS") || res.text.includes("exited with code 1") || res.text.includes("exited with code 2")) {
        return { failed: true, errors: res.text };
      }
    }
  }
  return { failed: false, errors: "" };
};

const detectTestFailure = (toolResults: ToolResultInfo[]): { failed: boolean; errors: string } => {
  for (const res of toolResults) {
    if (res.name === "bash" && (res.text.includes("bun test") || res.text.includes("bun run test"))) {
      if (res.text.includes("fail") && !res.text.includes("0 fail")) {
        return { failed: true, errors: res.text };
      }
    }
  }
  return { failed: false, errors: "" };
};

// Detect garbage final messages (git status output, etc.)
const isGarbageFinalMessage = (msg: string): boolean => {
  const trimmed = msg.trim();
  // Git status patterns
  if (trimmed.startsWith("On branch ")) return true;
  if (trimmed.includes("Changes not staged for commit:")) return true;
  if (trimmed.includes("Your branch is up to date")) return true;
  if (trimmed.includes("nothing to commit, working tree clean")) return true;
  if (trimmed.includes("Untracked files:")) return true;
  // Raw command output patterns
  if (trimmed.startsWith("$") && trimmed.includes("\n")) return true;
  // Very short messages that aren't TASK_COMPLETED
  if (trimmed.length < 20 && !trimmed.includes("TASK_COMPLETED")) return true;
  return false;
};

// Logging
const OPENAGENTS_ROOT = "/Users/christopherdavid/code/openagents";
const getLogDir = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return nodePath.join(OPENAGENTS_ROOT, "docs", "logs", `${year}${month}${day}`);
};

const getLogPath = () => {
  const now = new Date();
  const hours = String(now.getHours()).padStart(2, "0");
  const mins = String(now.getMinutes()).padStart(2, "0");
  const secs = String(now.getSeconds()).padStart(2, "0");
  return nodePath.join(getLogDir(), `${hours}${mins}${secs}-task-run.md`);
};

let logFile: string;

const initLog = () => {
  const logDir = getLogDir();
  if (!fs.existsSync(logDir)) {
    fs.mkdirSync(logDir, { recursive: true });
  }
  logFile = getLogPath();
  fs.writeFileSync(logFile, `# Task Run Log\n\nStarted: ${new Date().toISOString()}\n\n`);
};

const log = (msg: string) => {
  const ts = new Date().toISOString();
  const line = `[${ts}] ${msg}`;
  console.log(line);
  if (logFile) {
    fs.appendFileSync(logFile, line + "\n");
  }
};

const logMd = (md: string) => {
  console.log(md);
  if (logFile) {
    fs.appendFileSync(logFile, md + "\n");
  }
};

const SYSTEM_PROMPT = `You are MechaCoder, an autonomous coding agent. You complete ONE task per run.

${GIT_CONVENTIONS}

## Effect TypeScript Patterns (MUST FOLLOW)

When writing Effect code, use these EXACT patterns:

### Accessing a Service
\`\`\`typescript
// CORRECT - use yield* inside Effect.gen
const program = Effect.gen(function* () {
  const service = yield* MyService  // yields the service
  const result = yield* service.doSomething()  // yields the effect
  return result
}).pipe(Effect.provide(MyServiceLive))

await Effect.runPromise(program)
\`\`\`

### WRONG patterns (DO NOT USE):
- \`Effect.service(MyService)\` - THIS DOES NOT EXIST
- \`Effect.flatMap(s => ...)\` without Effect.gen - harder to read
- \`yield* _(service)\` - old adapter pattern, deprecated

### Running Effects in Tests
\`\`\`typescript
test("example", async () => {
  const result = await Effect.gen(function* () {
    const service = yield* MyService
    return yield* service.method()
  }).pipe(
    Effect.provide(MyServiceLive),
    Effect.runPromise
  )
  expect(result).toBe(expected)
})
\`\`\`

## Step-by-Step Workflow (FOLLOW EXACTLY)

### Phase 1: Understand
1. Read the relevant source files with the read tool
2. Read existing tests if any
3. Understand what changes are needed

### Phase 2: Implement (REQUIRED - DO NOT SKIP)
4. Use the edit tool or write tool to ACTUALLY modify files
5. You MUST call edit or write tool at least once
6. Do NOT claim completion without writing code

### Phase 3: Verify (BOTH TESTS AND TYPES)
7. Run typecheck FIRST: \`bun run typecheck\`
8. If typecheck fails, fix type errors before proceeding
9. Run tests: \`bun test\`
10. If tests fail, fix and re-run
11. Run \`git diff\` to verify your changes exist

### Phase 4: Commit & Push (REQUIRED - DO NOT SKIP)
12. Stage and commit:
\`\`\`bash
git add -A && git commit -m "<task-id>: <description>

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>"
\`\`\`
13. Push (NOTE: pre-push hooks may take 1-2 minutes, wait for it):
\`\`\`bash
git push origin main
\`\`\`
14. Verify push succeeded - look for "main -> main" in output
15. If push fails with type errors, fix them and retry

### Phase 5: Update Task and Report Completion
16. Update the task in .openagents/tasks.jsonl using the tasks CLI:
\`\`\`bash
COMMIT_SHA=$(git rev-parse HEAD)
echo '{"id":"<task-id>","status":"closed","commits":["'$COMMIT_SHA'"]}' | bun run tasks:update --json-input
\`\`\`
17. Your FINAL message MUST be EXACTLY this format:
    TASK_COMPLETED: <task-id> - <brief summary of what was done>

## CRITICAL: FINAL MESSAGE FORMAT

You are NOT allowed to send a final message until ALL of the following are true:
- You have run the configured tests (\`bun test\`) and they passed
- You have run typecheck (\`bun run typecheck\`) and it passed
- You have staged changes and created a git commit
- You have pushed the commit to the configured branch (saw "main -> main")
- You have closed the task using the tasks CLI

ONLY THEN may you send your final message. Your final message MUST start with:
TASK_COMPLETED: <task-id>

For example:
TASK_COMPLETED: oa-73016a - Added version bump to package.json and electrobun.config.ts

If you cannot complete all steps, explain what's blocking you instead of saying TASK_COMPLETED.

## VALIDATION CHECKLIST (before saying TASK_COMPLETED)
- [ ] Did I use edit/write tool to modify at least one file?
- [ ] Did I run \`bun run typecheck\` and it passed with no errors?
- [ ] Did I run tests and they passed?
- [ ] Did I run git commit and see success message?
- [ ] Did I run git push and see "main -> main"?
- [ ] Did I close the task using tasks:update CLI?

If ANY of these are NO, you have NOT completed the task. Keep working.

## Rules
- Do ONE task only
- NEVER claim completion without actual code changes
- NEVER skip the commit/push/close steps
- ALWAYS end with TASK_COMPLETED: <task-id> on success
- If stuck after 15+ turns, report the blocking reason
`;

interface Config {
  workDir: string;
  sessionsDir: string;
  runLogDir: string;
  /** Use legacy Grok-based agentLoop instead of orchestrator+Claude Code */
  legacy: boolean;
  /** Force Claude Code only - no Grok fallback */
  ccOnly: boolean;
}

const parseArgs = (): Config => {
  const args = process.argv.slice(2);
  let workDir = process.cwd();
  let legacy = false;
  let ccOnly = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--dir" && args[i + 1]) {
      workDir = args[i + 1].startsWith("~")
        ? args[i + 1].replace("~", process.env.HOME || "")
        : args[i + 1];
      i++;
    } else if (args[i] === "--legacy") {
      legacy = true;
    } else if (args[i] === "--cc-only") {
      ccOnly = true;
    }
  }

  return {
    workDir,
    sessionsDir: `${workDir}/.openagents/sessions`,
    runLogDir: `${workDir}/.openagents/run-logs`,
    legacy,
    ccOnly,
  };
};

const doOneTask = (config: Config) =>
  Effect.gen(function* () {
    const startedAt = new Date().toISOString();
    initLog();
    
    // Create session
    const session = createSession(
      { model: "x-ai/grok-4.1-fast:free", systemPrompt: SYSTEM_PROMPT, maxTurns: 30 },
      "Do one task",
    );
    const sessionPath = yield* getSessionPath(config.sessionsDir, session.id);
    
    // Ensure sessions directory exists
    if (!fs.existsSync(config.sessionsDir)) {
      fs.mkdirSync(config.sessionsDir, { recursive: true });
    }
    
    yield* writeSessionStart(sessionPath, session).pipe(Effect.catchAll(() => Effect.void));
    
    log("=".repeat(60));
    log("DO ONE TASK - Starting");
    log(`Work directory: ${config.workDir}`);
    log(`Log file: ${logFile}`);
    log(`Session: ${sessionPath}`);
    log("=".repeat(60));

    process.chdir(config.workDir);
    log(`Changed to: ${process.cwd()}`);

    const tasksPath = nodePath.join(config.workDir, ".openagents", "tasks.jsonl");
    const taskFilter: TaskFilter = { status: "open", sortPolicy: "priority" };

    const selected = yield* pickNextTask(tasksPath, taskFilter).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to pick next task: ${(e as Error).message}`,
          ),
      ),
    );

    if (!selected) {
      log("NO_TASKS_AVAILABLE: No ready tasks found in .openagents/tasks.jsonl");
      logMd(`\n## Final Message\n\nNo ready tasks found.\n`);
      
      // Write run log for no-tasks case
      const metadata = createRunMetadata({
        taskId: null,
        taskTitle: null,
        startedAt,
        workDir: config.workDir,
        logFilePath: logFile,
        sessionFilePath: sessionPath,
        totalTurns: 0,
        finalMessage: "NO_TASKS_AVAILABLE",
        error: null,
      });
      yield* writeRunLog(config.runLogDir, metadata).pipe(Effect.catchAll(() => Effect.void));
      yield* writeSessionEnd(sessionPath, 0, "NO_TASKS_AVAILABLE").pipe(Effect.catchAll(() => Effect.void));
      
      return { success: true, logFile };
    }

    const inProgressTask: Task = yield* updateTask({
      tasksPath,
      id: selected.id,
      update: { status: "in_progress" },
    }).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "write_error",
            `Failed to mark task in_progress: ${(e as Error).message}`,
          ),
      ),
    );

    log(`Selected task: ${inProgressTask.id} (${inProgressTask.title})`);

    const userMessage = [
      `Work on task ${inProgressTask.id}: ${inProgressTask.title}`,
      `Description: ${inProgressTask.description ?? ""}`,
      `Priority: P${inProgressTask.priority} | Type: ${inProgressTask.type}`,
      `Deps: ${(inProgressTask.deps ?? []).map((d) => `${d.type}:${d.id}`).join(", ") || "none"}`,
      `Labels: ${(inProgressTask.labels ?? []).join(", ") || "none"}`,
      `Assignee: ${inProgressTask.assignee ?? "unassigned"}`,
      "",
      `Deliverable: Implement the task, run typecheck + tests, commit, push, close the task via CLI, then reply "TASK_COMPLETED: ${inProgressTask.id} - <summary>".`,
    ].join("\n");

    yield* writeUserMessage(sessionPath, userMessage).pipe(Effect.catchAll(() => Effect.void));

    // Generate run ID for streaming events - use SYNCHRONOUS logging for immediate flush
    const runId = generateRunId();
    const emit = (event: TaskRunEvent) => {
      try {
        appendRunEventSync(config.runLogDir, runId, event);
      } catch (e) {
        // Ignore logging errors, don't break the run
      }
    };
    
    // Emit run_start event IMMEDIATELY
    emit({
      type: "run_start",
      ts: nowTs(),
      runId,
      taskId: inProgressTask.id,
      workDir: config.workDir,
      model: "x-ai/grok-4.1-fast:free",
    });
    emit({ type: "task_selected", ts: nowTs(), taskId: inProgressTask.id, title: inProgressTask.title });

    // Agent loop with retry on typecheck/test failures
    const MAX_TOTAL_TURNS = 50;
    const MAX_RETRIES = 5;  // Increased to handle verification enforcement
    const MAX_WALL_CLOCK_MS = 15 * 60 * 1000;  // 15 minute hard timeout
    const runStartTime = Date.now();
    
    let allTurns: AgentTurn[] = [];
    let totalTurnsUsed = 0;
    let retryCount = 0;
    let currentMessage = userMessage;
    let finalMessage = "";
    let loopError: string | null = null;

    while (retryCount <= MAX_RETRIES && totalTurnsUsed < MAX_TOTAL_TURNS) {
      // Check wall-clock timeout
      const elapsed = Date.now() - runStartTime;
      if (elapsed > MAX_WALL_CLOCK_MS) {
        log(`\nWall-clock timeout exceeded (${Math.round(elapsed / 1000)}s > ${MAX_WALL_CLOCK_MS / 1000}s)`);
        emit({ type: "timeout", ts: nowTs(), reason: `Wall-clock timeout after ${Math.round(elapsed / 1000)}s` });
        loopError = "WALL_CLOCK_TIMEOUT";
        break;
      }

      const turnsRemaining = MAX_TOTAL_TURNS - totalTurnsUsed;
      
      // Pass onEvent callback so agentLoop emits events DURING execution (not after)
      const loopResult = yield* agentLoop(
        currentMessage,
        tools as any,
        {
          systemPrompt: SYSTEM_PROMPT,
          maxTurns: Math.min(30, turnsRemaining),
          model: "x-ai/grok-4.1-fast:free",
          onEvent: (loopEvent) => {
            // Convert loop events to run events with timestamp
            if (loopEvent.type === "turn_start") {
              emit({ type: "turn_start", ts: nowTs(), turn: totalTurnsUsed + loopEvent.turn });
            } else if (loopEvent.type === "llm_response") {
              emit({
                type: "llm_response",
                ts: nowTs(),
                turn: totalTurnsUsed + loopEvent.turn,
                hasToolCalls: loopEvent.hasToolCalls,
                message: loopEvent.message,
                toolCalls: loopEvent.toolCalls ?? [],
              });
            } else if (loopEvent.type === "tool_call") {
              emit({
                type: "tool_call",
                ts: nowTs(),
                tool: loopEvent.tool,
                toolCallId: loopEvent.toolCallId,
                args: loopEvent.args,
              });
            } else if (loopEvent.type === "tool_result") {
              emit({
                type: "tool_result",
                ts: nowTs(),
                tool: loopEvent.tool,
                toolCallId: loopEvent.toolCallId,
                ok: loopEvent.ok,
                result: loopEvent.result,
              });
            } else if (loopEvent.type === "edit_detected") {
              emit({ type: "edit_detected", ts: nowTs(), tool: loopEvent.tool });
            }
          },
        }
      ).pipe(
        Effect.catchAll((error) => 
          Effect.succeed({ 
            turns: [] as AgentTurn[], 
            finalMessage: `Error: ${error.message}`, 
            totalTurns: 0,
            verifyState: { dirtySinceVerify: false, typecheckOk: false, testsOk: false }
          })
        )
      );

      allTurns = [...allTurns, ...loopResult.turns];
      totalTurnsUsed += loopResult.totalTurns;
      finalMessage = loopResult.finalMessage || "";
      
      // Events are now emitted DURING agentLoop via onEvent callback - no post-loop emission needed

      // Get verification state from this loop iteration
      const verifyState = loopResult.verifyState;

      // Check if we got TASK_COMPLETED
      if (finalMessage.includes("TASK_COMPLETED")) {
        // Verify that checks actually passed
        if (verifyState.dirtySinceVerify) {
          // Agent said TASK_COMPLETED but didn't verify after edits
          retryCount++;
          log(`\n[RETRY ${retryCount}/${MAX_RETRIES}] Agent said TASK_COMPLETED but has unverified edits`);
          emit({ type: "retry_prompt", ts: nowTs(), reason: "TASK_COMPLETED with unverified edits" });
          
          currentMessage = `You said TASK_COMPLETED, but you have edited files since your last successful \`bun run typecheck\` and \`bun test\`. You MUST re-run both verification commands AFTER your edits before you can complete the task.

Run these commands now:
1. \`bun run typecheck\` - must pass with no errors
2. \`bun test\` - must pass with no failures

If they pass, proceed with git commit, push, and closing the task. Then say TASK_COMPLETED again.`;
          
          yield* writeUserMessage(sessionPath, currentMessage).pipe(Effect.catchAll(() => Effect.void));
          continue;
        }
        
        log(`\nTask completed successfully in ${totalTurnsUsed} total turns`);
        emit({ type: "verify_ok", ts: nowTs() });
        break;
      }

      // Check for garbage final message (git status output, etc.)
      if (isGarbageFinalMessage(finalMessage)) {
        retryCount++;
        log(`\n[RETRY ${retryCount}/${MAX_RETRIES}] Garbage final message detected (git status output)`);
        emit({ type: "retry_prompt", ts: nowTs(), reason: "garbage final message" });
        
        currentMessage = `You just responded with raw command output (like \`git status\`) instead of a structured response. This is not acceptable as a final message.

You must either:
1. Continue working on the task (read files, edit code, run tests), OR
2. If the work is done, follow the completion checklist:
   - Run \`bun run typecheck\` and verify it passes
   - Run \`bun test\` and verify it passes
   - Run \`git add -A && git commit -m "..."\`
   - Run \`git push origin main\`
   - Close the task with the tasks CLI
   - Then reply with: TASK_COMPLETED: ${inProgressTask.id} - <brief summary>

What would you like to do?`;
        
        yield* writeUserMessage(sessionPath, currentMessage).pipe(Effect.catchAll(() => Effect.void));
        continue;
      }

      // Check if agent stopped without verifying after edits
      if (verifyState.dirtySinceVerify) {
        retryCount++;
        log(`\n[RETRY ${retryCount}/${MAX_RETRIES}] Agent stopped with unverified edits`);
        emit({ type: "retry_prompt", ts: nowTs(), reason: "unverified edits" });
        
        currentMessage = `You have edited files but have not run verification since your last edit. You MUST run both:
1. \`bun run typecheck\`
2. \`bun test\`

Run these commands now, fix any errors, then proceed with commit/push/close and say TASK_COMPLETED.`;
        
        yield* writeUserMessage(sessionPath, currentMessage).pipe(Effect.catchAll(() => Effect.void));
        continue;
      }

      // Check for typecheck/test failures in this batch of turns
      const toolResults = extractToolResults(loopResult.turns);
      const typecheckResult = detectTypecheckFailure(toolResults);
      const testResult = detectTestFailure(toolResults);

      if (typecheckResult.failed || testResult.failed) {
        retryCount++;
        if (retryCount > MAX_RETRIES) {
          log(`\nMax retries (${MAX_RETRIES}) exceeded for typecheck/test failures`);
          loopError = "MAX_RETRIES_EXCEEDED";
          emit({ type: "verify_fail", ts: nowTs(), stderr: "Max retries exceeded" });
          break;
        }

        // Build retry message
        const errorType = typecheckResult.failed ? "TypeScript typecheck" : "Tests";
        const errorOutput = typecheckResult.failed ? typecheckResult.errors : testResult.errors;
        
        emit({ type: "verify_fail", ts: nowTs(), stderr: errorOutput.slice(0, 500) });
        
        currentMessage = `${errorType} failed. You MUST fix these errors before continuing.

Here are the errors:
\`\`\`
${errorOutput.slice(0, 2000)}
\`\`\`

Fix the errors by editing the code, then re-run the failing command. Do NOT send a final message until all checks pass, you have committed, pushed, and closed the task.`;

        log(`\n[RETRY ${retryCount}/${MAX_RETRIES}] ${errorType} failed, prompting agent to fix...`);
        
        // Write retry message to session
        yield* writeUserMessage(sessionPath, currentMessage).pipe(Effect.catchAll(() => Effect.void));
        
        continue;
      }

      // No typecheck/test failure but also no TASK_COMPLETED - agent gave up
      log(`\nAgent stopped without TASK_COMPLETED (no obvious failures to retry)`);
      loopError = "INCOMPLETE_NO_TASK_COMPLETED";
      break;
    }

    if (totalTurnsUsed >= MAX_TOTAL_TURNS) {
      log(`\nMax total turns (${MAX_TOTAL_TURNS}) exceeded`);
      loopError = "MAX_TURNS_EXCEEDED";
    }

    const result = { turns: allTurns, finalMessage, totalTurns: totalTurnsUsed };
    log(`\nCompleted in ${result.totalTurns} turns (${retryCount} retries)`);
    
    // Write turns to session
    for (const turn of result.turns) {
      yield* writeTurn(sessionPath, turn).pipe(Effect.catchAll(() => Effect.void));
    }
    
    // Log all turns to markdown
    logMd("\n## Agent Turns\n");
    for (const turn of result.turns) {
      if (turn.content) {
        logMd(`\n### Assistant\n${turn.content}\n`);
      }
      if (turn.toolCalls) {
        for (const call of turn.toolCalls) {
          logMd(`\n### Tool Call: ${call.name}\n\`\`\`json\n${call.arguments}\n\`\`\`\n`);
        }
      }
      if (turn.toolResults) {
        for (const res of turn.toolResults) {
          const status = res.isError ? "❌ ERROR" : "✅ SUCCESS";
          const text = res.result.content
            .filter((c): c is { type: "text"; text: string } => c.type === "text")
            .map(c => c.text)
            .join("\n");
          logMd(`\n### Tool Result: ${res.name} ${status}\n\`\`\`\n${text.slice(0, 1000)}${text.length > 1000 ? "\n..." : ""}\n\`\`\`\n`);
        }
      }
    }

    const finalMsg = result.finalMessage || "";
    logMd(`\n## Final Message\n\n${finalMsg}\n`);
    
    // Log loop error if any
    if (loopError) {
      logMd(`\n## Loop Error\n\n${loopError}\n`);
    }
    
    log("=".repeat(60));
    
    const isCompleted = finalMsg.includes("TASK_COMPLETED");
    let runStatus: "success" | "incomplete" | "failed" = "incomplete";
    
    if (isCompleted) {
      // Agent already closed the task via CLI, just log success
      log("SUCCESS - Task completed!");
      runStatus = "success";
    } else if (finalMsg.includes("NO_TASKS")) {
      log("No tasks available");
      runStatus = "incomplete";
    } else if (loopError === "MAX_TURNS_EXCEEDED" || loopError === "MAX_RETRIES_EXCEEDED") {
      log(`FAILED - ${loopError}`);
      log("Task remains in_progress for manual review");
      runStatus = "failed";
      logMd(`\n## INCOMPLETE_RUN\n\nRun aborted: ${loopError}. Final message did not contain TASK_COMPLETED.\n`);
    } else {
      // Run was incomplete - agent didn't say TASK_COMPLETED
      log("INCOMPLETE_RUN: final message did not contain TASK_COMPLETED; task left in_progress");
      log("Task remains in_progress for next run");
      runStatus = "incomplete";
      logMd(`\n## INCOMPLETE_RUN\n\nFinal message did not contain TASK_COMPLETED. Task left in_progress for next run.\n`);
    }
    log(`Log saved: ${logFile}`);
    log(`Session saved: ${sessionPath}`);
    log("=".repeat(60));

    // Write run log metadata
    const metadata = createRunMetadata({
      taskId: inProgressTask.id,
      taskTitle: inProgressTask.title,
      startedAt,
      workDir: config.workDir,
      logFilePath: logFile,
      sessionFilePath: sessionPath,
      totalTurns: totalTurnsUsed,
      finalMessage: finalMsg,
      error: loopError,
    });
    const runLogPath = yield* writeRunLog(config.runLogDir, metadata).pipe(
      Effect.catchAll(() => Effect.succeed("")),
    );
    if (runLogPath) {
      log(`Run log saved: ${runLogPath}`);
    }

    // Emit run_end event
    emit({ 
      type: "run_end", 
      ts: nowTs(), 
      status: runStatus, 
      finalMessage: finalMsg.slice(0, 500),
      error: loopError
    });

    yield* writeSessionEnd(sessionPath, totalTurnsUsed, finalMsg).pipe(Effect.catchAll(() => Effect.void));

    return { success: isCompleted, logFile, incomplete: !isCompleted, status: runStatus };
  });

/**
 * Orchestrator-based task execution (Claude Code primary, Grok fallback).
 * This is the new default path for do-one-task.
 */
const doOneTaskOrchestrator = (config: Config) =>
  Effect.gen(function* () {
    const openagentsDir = nodePath.join(config.workDir, ".openagents");

    // Load project config with defaults using Schema decoders
    const defaultConfig: ProjectConfig = {
      version: 1,
      projectId: "unknown",
      defaultBranch: "main",
      defaultModel: "x-ai/grok-4.1-fast:free",
      rootDir: ".",
      typecheckCommands: ["bun run typecheck"],
      testCommands: ["bun test"],
      sandboxTestCommands: [],
      e2eCommands: [],
      allowPush: true,
      allowForcePush: false,
      maxTasksPerRun: 3,
      maxRuntimeMinutes: 240,
      idPrefix: "oa",
      sessionDir: ".openagents/sessions",
      runLogDir: ".openagents/run-logs",
      claudeCode: {
        enabled: true,
        preferForComplexTasks: true,
        maxTurnsPerSubtask: 50,
        permissionMode: "bypassPermissions",
        fallbackToMinimal: true,
      },
      sandbox: {
        enabled: false,
        backend: "auto",
        timeoutMs: 300_000,
      },
      parallelExecution: {
        enabled: false,
        maxAgents: 2,
        worktreeTimeout: 30 * 60 * 1000,
        installTimeoutMs: 15 * 60 * 1000,
        installArgs: ["--frozen-lockfile"],
        mergeStrategy: "auto",
        mergeThreshold: 4,
        prThreshold: 50,
      },
      trajectory: {
        enabled: false,
        retentionDays: 30,
        maxSizeGB: 5,
        includeToolArgs: true,
        includeToolResults: true,
        directory: "trajectories",
      },
      healer: {
        enabled: false,
        maxInvocationsPerSession: 2,
        maxInvocationsPerSubtask: 1,
        scenarios: {
          onInitFailure: true,
          onVerificationFailure: true,
          onSubtaskFailure: true,
          onRuntimeError: true,
          onStuckSubtask: false,
        },
        spells: {
          allowed: [],
          forbidden: [],
        },
        mode: "conservative",
        stuckThresholdHours: 2,
      },
      reflexion: {
        enabled: true,
        maxReflectionsPerRetry: 3,
        generationTimeoutMs: 30000,
        retentionDays: 30,
      },
    };
    const loadedConfig = yield* loadProjectConfig(config.workDir).pipe(
      Effect.catchAll(() => Effect.succeed(null))
    );
    const projectConfig = loadedConfig ?? defaultConfig;

    console.log("=".repeat(60));
    console.log("DO ONE TASK - Orchestrator Mode (Claude Code primary)");
    console.log(`Work directory: ${config.workDir}`);
    console.log(`Test commands: ${projectConfig.testCommands?.join(", ") || "none"}`);
    console.log(`Claude Code enabled: ${projectConfig.claudeCode?.enabled ?? true}`);
    console.log("=".repeat(60));

    process.chdir(config.workDir);

    // Create HUD callbacks for real-time updates to the desktop HUD
    // These silently fail if the HUD isn't running
    const { emit: hudEmit, onOutput: hudOnOutput, client: hudClient } = createHudCallbacks();

    // Initialize Healer service with LLM capabilities
    const healerCounters = createHealerCounters();

    // Adapter: Wrap runClaudeCodeSubagent for Healer
    const claudeCodeInvoker = async (subtask: Subtask, options: any) => {
      return await runClaudeCodeSubagent(subtask, {
        cwd: config.workDir,
        openagentsDir,
        maxTurns: options.maxTurns ?? 50,
        permissionMode: options.permissionMode ?? "bypassPermissions",
        onOutput: options.onOutput ?? ((text: string) => process.stdout.write(text)),
        signal: options.signal,
      });
    };

    // Adapter: Run typecheck commands for verification
    const verificationRunner = async (cwd: string) => {
      const typecheckCommands = projectConfig.typecheckCommands ?? ["bun run typecheck"];
      try {
        const result = await Bun.spawn(typecheckCommands[0]!.split(" "), {
          cwd,
          stdout: "pipe",
          stderr: "pipe",
        });
        const output = await new Response(result.stdout).text();
        const success = result.exitCode === 0;
        return { success, output };
      } catch (error: any) {
        return { success: false, output: error.message };
      }
    };

    // Create full Healer service
    const healerService = createHealerService({
      claudeCodeInvoker,
      verificationRunner,
      onOutput: (text: string) => process.stdout.write(text),
      openagentsDir,
    });

    // Event handler for logging (also forwards to HUD)
    const logOrchestratorEvent = (event: OrchestratorEvent) => {
      const ts = new Date().toISOString();
      switch (event.type) {
        case "session_start":
          console.log(`[${ts}] Session started: ${event.sessionId}`);
          break;
        case "task_selected":
          console.log(`[${ts}] Task selected: ${event.task.id} - ${event.task.title}`);
          break;
        case "subtask_start":
          console.log(`[${ts}] Subtask started: ${event.subtask.id}`);
          break;
        case "subtask_complete":
          console.log(`[${ts}] Subtask complete: ${event.subtask.id} (agent: ${event.result.agent})`);
          break;
        case "subtask_failed":
          console.log(`[${ts}] Subtask FAILED: ${event.subtask.id} - ${event.error}`);
          break;
        case "verification_start":
          console.log(`[${ts}] Running: ${event.command}`);
          break;
        case "verification_complete":
          console.log(`[${ts}] ${event.passed ? "PASS" : "FAIL"}: ${event.command}`);
          break;
        case "commit_created":
          console.log(`[${ts}] Commit: ${event.sha.slice(0, 8)} - ${event.message}`);
          break;
        case "push_complete":
          console.log(`[${ts}] Pushed to ${event.branch}`);
          break;
        case "session_complete":
          console.log(`[${ts}] Session ${event.success ? "SUCCESS" : "FAILED"}: ${event.summary}`);
          break;
        case "error":
          console.log(`[${ts}] ERROR in ${event.phase}: ${event.error}`);
          break;
      }
    };

    // Wrap emit to send orchestrator events to both log and HUD
    const emit = (event: OrchestratorEvent) => {
      logOrchestratorEvent(event);
      hudEmit(event);
    };

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
      ...(projectConfig.sandboxTestCommands?.length && {
        sandboxTestCommands: [...projectConfig.sandboxTestCommands],
      }),
      allowPush: projectConfig.allowPush ?? true,
      claudeCode: claudeCodeConfig,
      ...(projectConfig.typecheckCommands && { typecheckCommands: [...projectConfig.typecheckCommands] }),
      ...(projectConfig.sandbox && { sandbox: projectConfig.sandbox }),
      // Stream Claude Code output to console AND HUD
      onOutput: (text: string) => {
        process.stdout.write(text);
        hudOnOutput(text);
      },
      // Healer integration
      healerService,
      healerCounters,
      projectConfig,
    };
    const state = yield* runOrchestrator(orchestratorConfig, emit);

    // Generate a placeholder log file path for compatibility with legacy return type
    const logFile = nodePath.join(config.runLogDir, `orchestrator-${Date.now()}.log`);

    // Close HUD client connection
    hudClient.close();

    console.log("=".repeat(60));
    if (state.phase === "done") {
      console.log(`SUCCESS - Task ${state.task?.id} completed`);
      return { success: true, logFile, incomplete: false, status: "success" as const };
    } else if (state.phase === "failed") {
      console.log(`FAILED - ${state.error || "Unknown error"}`);
      return { success: false, logFile, incomplete: true, status: "failed" as const };
    } else {
      console.log(`INCOMPLETE - Phase: ${state.phase}`);
      return { success: false, logFile, incomplete: true, status: "incomplete" as const };
    }
  });

// Main
const config = parseArgs();

const liveLayer = Layer.mergeAll(openRouterLive, BunContext.layer);

// Route based on --legacy flag
const program = config.legacy
  ? doOneTask(config) // Legacy: Grok-based agentLoop
  : doOneTaskOrchestrator(config); // Default: Orchestrator with Claude Code

if (config.legacy) {
  console.log("[Legacy mode: Grok-based agentLoop]");
} else if (config.ccOnly) {
  console.log("[Orchestrator mode: Claude Code ONLY - no Grok fallback]");
} else {
  console.log("[Orchestrator mode: Claude Code primary, Grok fallback]");
}

Effect.runPromise(program.pipe(Effect.provide(liveLayer)))
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("Fatal:", err);
    process.exit(1);
  });
