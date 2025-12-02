#!/usr/bin/env bun
/**
 * Overnight Agent - Long-running autonomous coding agent
 * 
 * Usage: bun src/agent/overnight.ts --dir ~/code/some-repo [--max-beads 5] [--dry-run]
 * 
 * The agent will:
 * 1. Check for ready beads (bd ready)
 * 2. Claim the highest priority task
 * 3. Read relevant files and implement the fix
 * 4. Run tests
 * 5. Commit and push to main
 * 6. Close the bead
 * 7. Repeat until no more beads or max reached
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

1. **Check beads**: Run \`bd ready --json\` to see available work
2. **Claim a bead**: Pick highest priority TASK (not epic), run \`bd update <id> --status in_progress --json\`
3. **Understand the task**: Read the bead description, read relevant source files
4. **Implement**: Make necessary code changes using edit tool
5. **Test**: Run relevant tests with bash (bun test, etc.)
6. **Commit**: Stage changes and commit with proper format:
   \`\`\`
   git add -A && git commit -m "$(cat <<'EOF'
   Your message here
   
   🤖 Generated with [OpenAgents](https://openagents.com)
   
   Co-Authored-By: MechaCoder <noreply@openagents.com>
   EOF
   )"
   \`\`\`
7. **Push**: Run \`git push origin main\`
8. **Close bead**: Run \`bd close <id> --reason "description" --json\`

## Important Rules

- ALWAYS run tests before committing
- NEVER force push
- NEVER commit secrets or credentials
- If tests fail, fix them before committing
- If stuck, close the bead with reason explaining the blocker
- Work on ONE bead at a time
- Keep commits focused and atomic

## Current Task

Work in the current directory. Start by checking what beads are ready.
`;

interface OvernightConfig {
  workDir: string;
  maxBeads: number;
  dryRun: boolean;
  sessionsDir: string;
}

const parseArgs = (): OvernightConfig => {
  const args = process.argv.slice(2);
  let workDir = process.cwd();
  let maxBeads = 10;
  let dryRun = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--dir" && args[i + 1]) {
      workDir = args[i + 1].startsWith("~") 
        ? args[i + 1].replace("~", process.env.HOME || "")
        : args[i + 1];
      i++;
    } else if (args[i] === "--max-beads" && args[i + 1]) {
      maxBeads = parseInt(args[i + 1], 10);
      i++;
    } else if (args[i] === "--dry-run") {
      dryRun = true;
    }
  }

  return {
    workDir,
    maxBeads,
    dryRun,
    sessionsDir: `${workDir}/.agent-sessions`,
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

const runBeadCycle = (
  config: OvernightConfig,
  beadNumber: number,
): Effect.Effect<{ completed: boolean; message: string }, Error, any> =>
  Effect.gen(function* () {
    log(`\n${"=".repeat(60)}`);
    log(`BEAD CYCLE ${beadNumber}/${config.maxBeads}`);
    log(`Working directory: ${config.workDir}`);
    log(`${"=".repeat(60)}\n`);

    const prompt = beadNumber === 1
      ? `You are starting a new work session in ${config.workDir}.

First, run \`bd ready --json\` to see what beads are available.
Then pick the highest priority TASK bead (not epic) and claim it.
Read the relevant files to understand what needs to be done.
Implement the changes, run tests, commit, push, and close the bead.

Start now.`
      : `Continue working. Check \`bd ready --json\` for the next bead.
If there are no more ready beads, respond with "NO_MORE_BEADS".
Otherwise, claim the next highest priority task and complete it.`;

    if (config.dryRun) {
      log("[DRY RUN] Would send prompt:");
      log(prompt);
      return { completed: false, message: "Dry run - no action taken" };
    }

    const result = yield* agentLoop(prompt, tools as any, {
      systemPrompt: OVERNIGHT_SYSTEM_PROMPT,
      maxTurns: 20,
      model: "x-ai/grok-4.1-fast",
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
    const noMoreBeads = finalMessage.includes("NO_MORE_BEADS") || 
                        finalMessage.toLowerCase().includes("no more beads") ||
                        finalMessage.toLowerCase().includes("no ready beads");

    if (noMoreBeads) {
      return { completed: false, message: "No more beads available" };
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
      { model: "x-ai/grok-4.1-fast", systemPrompt: OVERNIGHT_SYSTEM_PROMPT, maxTurns: 25 },
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
    log(`Max beads: ${config.maxBeads}`);
    log(`Dry run: ${config.dryRun}`);
    log(`${"#".repeat(60)}\n`);

    // Change to work directory
    process.chdir(config.workDir);
    log(`Changed to directory: ${process.cwd()}`);

    let beadsCompleted = 0;
    let continueLoop = true;

    while (continueLoop && beadsCompleted < config.maxBeads) {
      try {
        const result = yield* runBeadCycle(config, beadsCompleted + 1).pipe(
          Effect.catchAll((error) => 
            Effect.succeed({ completed: false, message: `Error: ${error.message}` })
          ),
        );

        yield* writeTurn(sessionPath, {
          role: "assistant",
          content: result.message,
        }).pipe(Effect.catchAll(() => Effect.void));

        if (result.completed) {
          beadsCompleted++;
          log(`\n✓ Bead ${beadsCompleted} completed`);
        } else {
          log(`\n✗ Stopping: ${result.message}`);
          continueLoop = false;
        }

        // Small delay between beads
        if (continueLoop) {
          yield* Effect.sleep(2000);
        }
      } catch (error) {
        log(`\n✗ Error: ${error}`);
        continueLoop = false;
      }
    }

    yield* writeSessionEnd(sessionPath, beadsCompleted, `Completed ${beadsCompleted} beads`).pipe(
      Effect.catchAll(() => Effect.void),
    );

    log(`\n${"#".repeat(60)}`);
    log("OVERNIGHT AGENT FINISHED");
    log(`Beads completed: ${beadsCompleted}`);
    log(`Session saved: ${sessionPath}`);
    log(`${"#".repeat(60)}\n`);

    return { beadsCompleted, sessionId: session.id };
  });

// Main
const config = parseArgs();

if (!config.workDir) {
  console.error("Usage: bun src/agent/overnight.ts --dir <work-directory> [--max-beads N] [--dry-run]");
  process.exit(1);
}

const liveLayer = Layer.mergeAll(openRouterLive, BunContext.layer);

Effect.runPromise((overnightLoop(config) as any).pipe(Effect.provide(liveLayer)))
  .then((result: unknown) => {
    const r = result as { beadsCompleted: number; sessionId: string };
    console.log(`\nDone! Completed ${r.beadsCompleted} beads.`);
    process.exit(0);
  })
  .catch((error: unknown) => {
    console.error("Fatal error:", error);
    process.exit(1);
  });
