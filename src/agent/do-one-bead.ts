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

const SYSTEM_PROMPT = `You are MechaCoder, an autonomous coding agent. You complete ONE bead per run.

${GIT_CONVENTIONS}

## CRITICAL: Use full path for bd command
Always use: \`$HOME/.local/bin/bd\` (never just \`bd\`)

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

### Phase 1: Find Work
1. Run: \`$HOME/.local/bin/bd ready --json\`
2. If empty or only epics: respond "NO_BEADS_AVAILABLE" and STOP
3. Pick highest priority TASK (skip epics)
4. Claim it: \`$HOME/.local/bin/bd update <id> --status in_progress --json\`

### Phase 2: Understand
5. Read the relevant source files with the read tool
6. Read existing tests if any
7. Understand what changes are needed

### Phase 3: Implement (REQUIRED - DO NOT SKIP)
8. Use the edit tool or write tool to ACTUALLY modify files
9. You MUST call edit or write tool at least once
10. Do NOT claim completion without writing code

### Phase 4: Verify
11. Run tests: \`bun test <specific-test-file>\` or \`bun test\`
12. If tests fail, fix and re-run
13. Run \`git diff\` to verify your changes exist

### Phase 5: Commit & Push (REQUIRED)
14. Stage and commit:
\`\`\`bash
git add -A && git commit -m "$(cat <<'EOF'
<type>(<scope>): <description> (<bead-id>)

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>
EOF
)"
\`\`\`
15. Push: \`git push origin main\`
16. Verify push succeeded (check output)

### Phase 6: Close
17. Close bead: \`$HOME/.local/bin/bd close <id> --reason "Completed: <what you did>" --json\`
18. ONLY THEN respond: "BEAD_COMPLETED: <bead-id>"

## VALIDATION CHECKLIST (before saying BEAD_COMPLETED)
- [ ] Did I use edit/write tool to modify at least one file?
- [ ] Did I run tests and they passed?
- [ ] Did I run git commit and see success message?
- [ ] Did I run git push and see "main -> main"?
- [ ] Did I run bd close and see the bead status change?

If ANY of these are NO, you have NOT completed the bead. Keep working.

## Rules
- Do ONE bead only
- NEVER claim completion without actual code changes
- NEVER skip the commit/push steps
- If stuck after 15+ turns, close bead with blocking reason instead
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
