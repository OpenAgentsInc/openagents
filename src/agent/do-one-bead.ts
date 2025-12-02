#!/usr/bin/env bun
/**
 * Do One Bead - Picks up ONE bead, completes it, commits, pushes, exits.
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
  return nodePath.join(getLogDir(), `${hours}${mins}${secs}-bead-run.md`);
};

let logFile: string;

const initLog = () => {
  const logDir = getLogDir();
  if (!fs.existsSync(logDir)) {
    fs.mkdirSync(logDir, { recursive: true });
  }
  logFile = getLogPath();
  fs.writeFileSync(logFile, `# Bead Run Log\n\nStarted: ${new Date().toISOString()}\n\n`);
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

const SYSTEM_PROMPT = `You are MechaCoder, an autonomous coding agent. You have ONE job this run: complete exactly ONE bead.

${GIT_CONVENTIONS}

## Your Mission (ONE BEAD ONLY)

1. Run \`bd ready --json\` to see available beads
2. If no beads ready, respond "NO_BEADS_AVAILABLE" and stop
3. Pick the HIGHEST PRIORITY **task** (not epic), claim it: \`bd update <id> --status in_progress --json\`
4. Read relevant files to understand the task
5. Implement the fix/feature using edit tool
6. Run tests: \`bun test\` or appropriate test command
7. If tests pass, commit:
   \`\`\`bash
   git add -A && git commit -m "$(cat <<'EOF'
   Your descriptive message (reference bead ID)
   
   🤖 Generated with [OpenAgents](https://openagents.com)
   
   Co-Authored-By: MechaCoder <noreply@openagents.com>
   EOF
   )"
   \`\`\`
8. Push: \`git push origin main\`
9. Close bead: \`bd close <id> --reason "Completed: brief description" --json\`
10. Respond "BEAD_COMPLETED: <bead-id>" 

## Critical Rules
- Do ONE bead only, then stop
- MUST run tests before committing
- MUST push after committing  
- If tests fail, try to fix. If stuck, close bead with blocking reason
- Never force push
- Never commit secrets
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

const doOneBead = (config: Config) =>
  Effect.gen(function* () {
    initLog();
    
    log("=".repeat(60));
    log("DO ONE BEAD - Starting");
    log(`Work directory: ${config.workDir}`);
    log(`Log file: ${logFile}`);
    log("=".repeat(60));

    process.chdir(config.workDir);
    log(`Changed to: ${process.cwd()}`);

    const result = yield* agentLoop(
      "Check bd ready --json. If there's a ready task bead, claim it, implement it, test, commit, push, and close it. Do ONE bead only.",
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
    if (finalMsg.includes("BEAD_COMPLETED")) {
      log("SUCCESS - Bead completed!");
    } else if (finalMsg.includes("NO_BEADS")) {
      log("No beads available");
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

Effect.runPromise(doOneBead(config).pipe(Effect.provide(liveLayer)))
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("Fatal:", err);
    process.exit(1);
  });
