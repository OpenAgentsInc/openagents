#!/usr/bin/env bun
// One-shot TERMINAL/TUI run with a REAL local PTY (no network).
//
// The terminal-surface analogue of run-once.ts: it spawns a command/TUI inside
// a PTY, replays a deterministic terminal scenario, and writes real artifacts —
// a text-snapshot timeline + an asciicast + result.json. Exit code is honest:
// 0 on pass, 1 on fail. By default it runs the shipped `echoPromptScenario`,
// which uses only a POSIX shell (no network), so it is a quick honest proof.
//
// Usage:
//   bun run src/terminal-once.ts --out ./runs/terminal [--wrong]

import { makeTarget } from "./target";
import { runTerminalScenario } from "./terminal-backend";
import { echoPromptScenario, echoPromptScenarioWrong } from "./terminal-scenario";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--wrong") args.wrong = true;
    else if (a.startsWith("--")) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith("--")) {
        args[key] = next;
        i++;
      } else args[key] = true;
    }
  }
  return args;
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const artifactDir = typeof args.out === "string" ? args.out : "./runs/terminal-once";
  const scenario = args.wrong ? echoPromptScenarioWrong() : echoPromptScenario();

  const outcome = await runTerminalScenario({
    target: makeTarget({ name: "local-terminal", baseUrl: "terminal://localhost", capabilities: ["terminal"] }),
    scenario,
    artifactDir,
  });

  console.log("=== QA TERMINAL RUN (terminal-once) ===");
  console.log("status:", outcome.result.status);
  console.log("scenario:", scenario.name);
  console.log("result:", outcome.resultPath);
  console.log("snapshot-timeline:", outcome.snapshotTimelinePath);
  console.log("asciicast:", outcome.asciicastPath);
  if (outcome.result.failure) console.log("failure:", outcome.result.failure);
  process.exit(outcome.result.status === "pass" ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
