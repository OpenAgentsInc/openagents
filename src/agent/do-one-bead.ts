#!/usr/bin/env bun
/**
 * Do One Task - Picks up ONE task from .openagents/tasks.jsonl, completes it, commits, pushes, exits.
 * Designed to be run by cron/launchd every few minutes.
 * 
 * Usage: bun src/agent/do-one-bead.ts --dir ~/code/some-repo
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
  closeTask,
  type Task,
  type TaskFilter,
  TaskServiceError,
} from "../tasks/index.js";
import { readTool } from "../tools/read.js";
import { editTool } from "../tools/edit.js";
import { bashTool } from "../tools/bash.js";
import { writeTool } from "../tools/write.js";
import { openRouterLive } from "../llm/openrouter.js";

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
5. Read the relevant source files with the read tool
6. Read existing tests if any
7. Understand what changes are needed

### Phase 3: Implement (REQUIRED - DO NOT SKIP)
8. Use the edit tool or write tool to ACTUALLY modify files
9. You MUST call edit or write tool at least once
10. Do NOT claim completion without writing code

### Phase 4: Verify (BOTH TESTS AND TYPES)
11. Run typecheck FIRST: \`bun run typecheck\`
12. If typecheck fails, fix type errors before proceeding
13. Run tests: \`bun test <specific-test-file>\`
14. If tests fail, fix and re-run
15. Run \`git diff\` to verify your changes exist

### Phase 5: Commit & Push (REQUIRED - DO NOT SKIP)
14. Stage and commit:
\`\`\`bash
git add -A && git commit -m "<type>(<scope>): <description> (<bead-id>)

Generated with OpenAgents
Co-Authored-By: MechaCoder <noreply@openagents.com>"
\`\`\`
15. Push (NOTE: pre-push hooks may take 1-2 minutes, wait for it):
\`\`\`bash
git push origin main
\`\`\`
16. Verify push succeeded - look for "main -> main" in output
17. If push fails with type errors, fix them and retry

### Phase 6: Close
17. The task will be automatically closed by the agent loop after successful completion.
18. ONLY THEN respond: "TASK_COMPLETED: <task-id>"

## VALIDATION CHECKLIST (before saying TASK_COMPLETED)
- [ ] Did I use edit/write tool to modify at least one file?
- [ ] Did I run \`bun run typecheck\` and it passed with no errors?
- [ ] Did I run tests and they passed?
- [ ] Did I run git commit and see success message?
- [ ] Did I run git push and see "main -> main"?

If ANY of these are NO, you have NOT completed the task. Keep working.

## Rules
- Do ONE task only
- NEVER claim completion without actual code changes
- NEVER skip the commit/push steps
- If stuck after 15+ turns, report the blocking reason
`;

interface Config {
  workDir: string;
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

  return { workDir };
};

const doOneTask = (config: Config) =>
  Effect.gen(function* () {
    initLog();
    
    log("=".repeat(60));
    log("DO ONE TASK - Starting");
    log(`Work directory: ${config.workDir}`);
    log(`Log file: ${logFile}`);
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
      `Deliverable: Implement the task, run typecheck + relevant tests, commit, push, and reply "TASK_COMPLETED: ${inProgressTask.id} - <summary>" with tests run.`,
    ].join("\n");

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
    
    // Log all turns
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
    if (finalMsg.includes("TASK_COMPLETED")) {
      yield* closeTask({
        tasksPath,
        id: inProgressTask.id,
        reason: finalMsg,
      }).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError(
              "write_error",
              `Failed to close task: ${(e as Error).message}`,
            ),
        ),
      );
      log("SUCCESS - Task completed!");
    } else if (finalMsg.includes("NO_TASKS")) {
      log("No tasks available");
    } else {
      log("Run finished (check log for details)");
    }
    log(`Log saved: ${logFile}`);
    log("=".repeat(60));

    return { success: true, logFile };
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
