#!/usr/bin/env bun
// Run ONE scenario across N targets with REAL chromium (#6190, Rhys req #3).
//
// The multi-target proof path (NOT in default CI): it resolves a SELECTION of
// registry targets and runs the SAME /login regression scenario against each —
// from a single definition, no rewrite — writing per-target artifacts + a
// target-matrix.json. Exit code is honest: 0 iff every target passed.
//
// Usage:
//   bun run src/run-targets.ts --targets dev,prod --out ./runs/matrix
//     [--headed]
//   # base URLs come from the registry / env: QA_DEV_URL, QA_PROD_URL, ...
//   # selfhost requires QA_SELFHOST_URL.
//
// Restrictions are honored: a read-only target (prod by default) refuses a
// mutating step with a recorded reason, so a prod run never creates data.

import { Effect } from "effect";
import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { loginRegressionCommitments, loginRegressionSteps } from "./scenarios";
import { parseTargetSelection } from "./target-registry";
import { runScenarioAcrossTargets } from "./target-registry-run";

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
  const selection = typeof args.targets === "string" ? args.targets : undefined;
  const targets = parseTargetSelection(selection);
  const artifactDir = typeof args.out === "string" ? args.out : "./runs/target-matrix";

  const outcome = await Effect.runPromise(
    runScenarioAcrossTargets({
      scenario: {
        id: "login-regression",
        title: "/login renders the sign-in form",
        brain: () => scriptedBrain(loginRegressionSteps()),
        backend: () => localBackend(),
        commitments: loginRegressionCommitments(),
      },
      targets,
      artifactDir,
      ...(args.headed ? { headed: true } : {}),
    }),
  );

  const m = outcome.result;
  console.log("=== QA MULTI-TARGET RUN ===");
  console.log("scenario:", m.scenarioId, "-", m.title);
  console.log("matrix:", outcome.resultPath);
  console.log("pass:", `${m.passCount}/${m.targetCount}`, `(${Math.round(m.passRate * 100)}%)`);
  for (const t of m.targets) {
    const tags = [t.readOnly ? "read-only" : "writable", ...(t.verdict ? [t.verdict] : [])].join(", ");
    console.log(`  - ${t.targetName} [${tags}] ${t.targetBaseUrl}: ${t.status}`);
    if (t.failure) console.log(`      failure: ${t.failure}`);
  }
  // honest: green only when EVERY selected target passed.
  process.exit(m.passCount === m.targetCount ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
