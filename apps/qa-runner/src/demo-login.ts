#!/usr/bin/env bun
// Headline demo (#6177): Khala verifies a real openagents.com flow on video.
//
// Wires the computer-use tools (#6175) + the QA runner (#6176) + localBackend +
// scriptedBrain to drive https://openagents.com and verify the exact regression
// recently fixed:
//   - open /login; assert it STAYS at /login (no redirect to "/")
//   - assert the body contains "Log in to OpenAgents"
//   - (second step) assert /gym/oss redirects to "/" when logged out
//
// Emits session.<mp4|webm> + trace + screenshots + result.json. PASS on the
// working flow; honest FAIL (with the failure visible in the video) if it
// redirects/breaks. NO secrets in artifacts.
//
// NOTE: this demo's output is a FILM + RESULT, not yet a committed test. Two
// gated follow-ups complete the epic:
//   1. khalaBrain — Khala AUTONOMOUSLY drives the session via openagents/khala
//      inference (here we use the deterministic scriptedBrain).
//   2. the session -> committed e2e test distiller — lowers a recorded session
//      timeline into a black-box scenario committed to the repo.
//
// Usage:
//   bun run src/demo-login.ts [--out ./runs/login] [--headed] [--wrong]
//     [--url https://openagents.com]
//   --wrong points the same scenario at a deliberately-wrong assertion to prove
//   the runner FAILS honestly.

import { Effect } from "effect";
import { localBackend } from "./backend";
import { scriptedBrain, type BrainStep } from "./brain";
import { loginRegressionSteps, loginRegressionStepsWrong } from "./scenarios";
import { runQaSession } from "./runner";
import { makeTarget } from "./target";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--headed") args.headed = true;
    else if (a === "--wrong") args.wrong = true;
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

/** The full demo journey: /login regression + (logged-out) /gym/oss redirect. */
function demoSteps(): ReadonlyArray<BrainStep> {
  return [
    ...loginRegressionSteps(),
    // Second step: /gym/oss must redirect to "/" when logged out. The redirect
    // is client-side (the SPA boots at /gym/oss then navigates away once the
    // logged-out admin gate resolves), so WAIT on the URL leaving /gym/oss
    // before asserting — never assert on the transient pre-redirect URL, and
    // screenshot only after it settles so the frame shows the result, not a
    // blank mid-redirect page.
    { kind: "navigate", url: "/gym/oss", label: "open /gym/oss (logged out)" },
    {
      kind: "wait-for",
      condition: { kind: "url-not-includes", value: "/gym/oss" },
      label: "wait for /gym/oss admin-gate redirect to settle",
    },
    { kind: "screenshot", label: "gym-oss-redirect" },
    {
      kind: "assert",
      label: "/gym/oss redirects away when logged out",
      check: { kind: "url-not-includes", value: "/gym/oss" },
    },
  ];
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const baseUrl = typeof args.url === "string" ? args.url : "https://openagents.com";
  const target = makeTarget({ name: "openagents.com", baseUrl });
  const artifactDir = typeof args.out === "string" ? args.out : "./runs/demo-login";
  const steps = args.wrong ? loginRegressionStepsWrong() : demoSteps();

  console.log(`=== Khala QA demo: /login verification on ${baseUrl} ===`);
  console.log(args.wrong ? "(deliberately-wrong assertion — expecting honest FAIL)" : "(expecting PASS)");

  const outcome = await Effect.runPromise(
    runQaSession({
      target,
      brain: scriptedBrain(steps),
      backend: localBackend(),
      artifactDir,
      ...(args.headed ? { headed: true } : {}),
    }),
  );

  console.log("\nstatus:", outcome.result.status);
  console.log("result.json:", outcome.resultPath);
  console.log("artifacts:", JSON.stringify(outcome.result.artifacts, null, 2));
  for (const step of outcome.result.steps) {
    console.log(`  [${step.status === "ok" ? "PASS" : "FAIL"}] ${step.label}`);
  }
  if (outcome.result.failure) console.log("failure:", outcome.result.failure);

  // Honest exit code: PASS=0, FAIL=1. With --wrong, an honest FAIL (exit 1) is
  // the EXPECTED proof, so we invert the success check there.
  const expectedPass = !args.wrong;
  const actualPass = outcome.result.status === "pass";
  process.exit(actualPass === expectedPass ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
