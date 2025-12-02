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
import { agentLoop } from "./loop.js";
import { GIT_CONVENTIONS } from "./prompts.js";
import {
  pickNextTask,
  updateTask,
  type Task,
  type TaskFilter,
  TaskServiceError,
} from "../tasks/index.js";
import { readTool } from "../tools/read.js";
import { editTool } from "../tools/edit.js";
import { bashTool } from "../tools/bash.js";
import { writeTool } from "../tools/write.js";
import { openRouterLive } from "../llm/openrouter.js";
import { createRunMetadata, writeRunLog } from "./runLog.js";
import {
  createSession,
  writeSessionStart,
  writeUserMessage,
  writeTurn,
  writeSessionEnd,
  getSessionPath,
} from "./session.js";

const tools = [readTool, editTool, bashTool, writeTool];

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
}

const parseArgs = (): Config => {
  const args = process.argv.slice(2);
  let workDir = process.cwd();

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--dir" && args[i + 1]) {
      workDir = args[i + 1].startsWith("~") 
        ? args[i + 1].replace("~", process.env.HOME || "")
        : args[i + 1];
      i++;
    }
  }

  return {
    workDir,
    sessionsDir: `${workDir}/.openagents/sessions`,
    runLogDir: `${workDir}/.openagents/run-logs`,
  };
};

const doOneTask = (config: Config) =>
  Effect.gen(function* () {
    const startedAt = new Date().toISOString();
    initLog();
    
    // Create session
    const session = createSession(
      { model: "x-ai/grok-4.1-fast", systemPrompt: SYSTEM_PROMPT, maxTurns: 30 },
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

    const result = yield* agentLoop(
      userMessage,
      tools as any,
      {
        systemPrompt: SYSTEM_PROMPT,
        maxTurns: 30,
        model: "x-ai/grok-4.1-fast",
      }
    ).pipe(
      Effect.catchAll((error) => 
        Effect.succeed({ 
          turns: [], 
          finalMessage: `Error: ${error.message}`, 
          totalTurns: 0 
        })
      )
    );

    log(`\nCompleted in ${result.totalTurns} turns`);
    
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
    
    log("=".repeat(60));
    
    const isCompleted = finalMsg.includes("TASK_COMPLETED");
    
    if (isCompleted) {
      // Agent already closed the task via CLI, just log success
      log("SUCCESS - Task completed!");
    } else if (finalMsg.includes("NO_TASKS")) {
      log("No tasks available");
    } else {
      // Run was incomplete - agent didn't say TASK_COMPLETED
      log("INCOMPLETE - Agent did not complete the full loop");
      log("Missing: TASK_COMPLETED in final message");
      log("Task remains in_progress for next run");
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
      totalTurns: result.totalTurns,
      finalMessage: finalMsg,
      error: null,
    });
    const runLogPath = yield* writeRunLog(config.runLogDir, metadata).pipe(
      Effect.catchAll(() => Effect.succeed("")),
    );
    if (runLogPath) {
      log(`Run log saved: ${runLogPath}`);
    }

    yield* writeSessionEnd(sessionPath, result.totalTurns, finalMsg).pipe(Effect.catchAll(() => Effect.void));

    return { success: isCompleted, logFile, incomplete: !isCompleted };
  });

// Main
const config = parseArgs();

const liveLayer = Layer.mergeAll(openRouterLive, BunContext.layer);

Effect.runPromise(doOneTask(config).pipe(Effect.provide(liveLayer)))
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("Fatal:", err);
    process.exit(1);
  });
