#!/usr/bin/env bun
// One-shot QA run with REAL chromium against a Target.
//
// This is the real-chromium proof path (NOT in default CI): it provisions the
// local backend, drives a scripted session, and writes real artifacts. Exit
// code is honest: 0 on pass, 1 on fail.
//
// Usage:
//   bun run src/run-once.ts --url https://openagents.com --out ./runs/manual
//     [--headed]
//
// By default it runs the same /login regression scenario the demo uses, so
// `run-once` against the live site is a quick honest smoke.

import { Effect, Fiber } from "effect";
import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { loginRegressionSteps } from "./scenarios";
import { runQaSession } from "./runner";
import { makeTarget, resolveTarget } from "./target";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--headed") args.headed = true;
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
  const target =
    typeof args.url === "string"
      ? makeTarget({ name: typeof args.name === "string" ? args.name : args.url, baseUrl: args.url })
      : resolveTarget();
  const artifactDir = typeof args.out === "string" ? args.out : "./runs/run-once";

  // Fork the run so a SIGINT (Ctrl-C) INTERRUPTS the fiber rather than killing
  // the process outright. Interruption runs the runner's `ensuring` finalizer,
  // which flushes video/trace/screenshots + result.json (#6193) — so a cancelled
  // real run still leaves dereferenceable artifacts behind.
  const fiber = Effect.runFork(
    runQaSession({
      target,
      brain: scriptedBrain(loginRegressionSteps()),
      backend: localBackend(),
      artifactDir,
      ...(args.headed ? { headed: true } : {}),
    }),
  );
  const onSignal = () => {
    Effect.runFork(Fiber.interrupt(fiber));
  };
  process.once("SIGINT", onSignal);
  process.once("SIGTERM", onSignal);

  const exit = await Effect.runPromise(Fiber.await(fiber));
  process.off("SIGINT", onSignal);
  process.off("SIGTERM", onSignal);
  if (exit._tag === "Failure") {
    console.error("=== QA RUN (run-once) — interrupted/failed; artifacts flushed ===");
    console.error("artifactDir:", artifactDir);
    process.exit(1);
  }
  const outcome = exit.value;

  console.log("=== QA RUN (run-once) ===");
  console.log("status:", outcome.result.status);
  console.log("target:", outcome.result.target.name, outcome.result.target.baseUrl);
  console.log("result:", outcome.resultPath);
  console.log("artifacts:", JSON.stringify(outcome.result.artifacts));
  if (outcome.result.failure) console.log("failure:", outcome.result.failure);
  process.exit(outcome.result.status === "pass" ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
