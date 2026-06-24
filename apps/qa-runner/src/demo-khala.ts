#!/usr/bin/env bun
// Headline demo (epic #6174): Khala AUTONOMOUSLY drives real developer tools and
// distills the session into a committed e2e test.
//
// This is the artifact to send the executor author:
//   1. give Khala (`openagents/khala`) a real goal against https://openagents.com
//      (default: verify the login page works);
//   2. let Khala autonomously drive the #6175 computer-use tools (it chooses each
//      action as JSON; the session runner executes it against real chromium);
//   3. record session.<mp4|webm> + trace + screenshots + a public-safe result.json
//      + a deterministic session-trace.json;
//   4. run the distiller to EMIT A COMMITTED executor-style e2e scenario file
//      (generated/<slug>.e2e.ts) from that session.
// The video + the generated test together are the review artifact.
//
// Credential (no hardcoded secrets, see khala-config.ts): prefers OPENAGENTS_API_KEY
// (real openagents/khala), then a discovered OpenAgents agent token, then the
// OpenAI-compatible PROBE_OPENAI_API_KEY fallback (clearly labeled, loop-proof only).
//
// Usage:
//   bun run src/demo-khala.ts [--goal "..."] [--url https://openagents.com]
//     [--out ./runs/khala] [--headed] [--max-turns 16] [--emit generated/<name>.e2e.ts]
//     [--no-fallback]

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Effect } from "effect";
import { localBackend } from "./backend";
import { distill, assessCandidate } from "./distiller";
import { makeFetchChatClient, resolveKhalaConfig } from "./khala-config";
import { runKhalaSession } from "./khala-session";
import { makeTarget } from "./target";

const DEFAULT_GOAL =
  "Verify the login page works on this site: open /login, confirm the sign-in form renders " +
  '(the page shows "Log in to OpenAgents"), and confirm it does NOT redirect to the homepage ' +
  "(the URL still includes /login). Assert each of these, screenshot the login page, then finish.";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--headed") args.headed = true;
    else if (a === "--no-fallback") args["no-fallback"] = true;
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
  const baseUrl = typeof args.url === "string" ? args.url : "https://openagents.com";
  const goal = typeof args.goal === "string" ? args.goal : DEFAULT_GOAL;
  const artifactDir = typeof args.out === "string" ? args.out : "./runs/demo-khala";
  const maxTurns = typeof args["max-turns"] === "string" ? Number(args["max-turns"]) : 16;
  const emitPath = typeof args.emit === "string" ? args.emit : undefined;

  // Resolve the model endpoint (env/secrets-driven; never prints the key).
  const config = resolveKhalaConfig({ allowFallback: !args["no-fallback"] });
  console.log("=== Khala autonomous-QA demo (epic #6174) ===");
  console.log(`target:   ${baseUrl}`);
  console.log(`model:    ${config.model}  (base ${config.baseUrl})`);
  console.log(`key src:  ${config.keySource}`);
  if (config.mode === "fallback") {
    console.log(
      "MODE:     loop proven with FALLBACK model; arm OPENAGENTS_API_KEY for the real Khala-driven run.",
    );
  } else {
    console.log("MODE:     real openagents/khala endpoint.");
  }
  console.log(`goal:     ${goal}\n`);

  // gpt-oss-20b behind openagents/khala spends tokens on a reasoning channel
  // before emitting content; a generous budget keeps the JSON action from being
  // truncated (finish_reason=length yields empty/partial content).
  const chat = makeFetchChatClient(config, { timeoutMs: 90_000, maxTokens: 1500 });

  const outcome = await Effect.runPromise(
    runKhalaSession({
      target: makeTarget({ name: "openagents.com", baseUrl }),
      backend: localBackend(),
      chat,
      goal,
      artifactDir,
      maxTurns,
      model: config.model,
      ...(args.headed ? { headed: true } : {}),
    }),
  );

  console.log("\n--- session ---");
  console.log("verdict:    ", outcome.verdict);
  console.log("result:     ", outcome.result.status, `(${outcome.resultPath})`);
  console.log("trace:      ", outcome.tracePath, `digest ${outcome.trace.digest.slice(0, 16)}…`);
  console.log("artifacts:  ", JSON.stringify(outcome.result.artifacts));
  for (const step of outcome.result.steps) {
    console.log(`  [${step.status === "ok" ? "PASS" : "FAIL"}] ${step.kind}: ${step.label}`);
  }
  if (outcome.result.failure) console.log("failure:    ", outcome.result.failure);

  // Distill the captured session into a committed executor-style e2e scenario.
  console.log("\n--- distill -> e2e emitter (spec §E.2) ---");
  const result = distill(outcome.trace);
  const assessment = assessCandidate(result, outcome.trace);
  console.log("verification class:", result.verificationClass);
  console.log("signature:         ", result.signatureCandidate.name);
  console.log("e2e assertions:    ", result.emitters.e2e.assertionCount);
  console.log("candidate admissible:", assessment.admissible, assessment.reasons.length ? assessment.reasons : "");
  console.log("skill emitter:     ", result.emitters.skill?.status, `(${result.emitters.skill?.reason})`);

  // ".e2e.test.ts": executor-style ".e2e" marker + ".test" so `bun test` and CI
  // discover it without an explicit path.
  const outFile = resolve(emitPath ?? join("generated", `${result.emitters.e2e.slug}.e2e.test.ts`));
  mkdirSync(dirname(outFile), { recursive: true });
  writeFileSync(outFile, result.emitters.e2e.source);
  console.log("emitted e2e scenario:", outFile);

  // Honest exit: a clean Khala run + an admissible candidate is success. A
  // fallback-mode run that proves the loop is also a success (clearly labeled).
  const ok = outcome.verdict === "pass" && outcome.result.status === "pass" && assessment.admissible;
  if (!ok) {
    console.log(
      "\nNOTE: not a clean pass. The session/verdict above is reported HONESTLY; " +
        "a weak tool-calling model can fail to reach a verdict — that is real, not faked.",
    );
  }
  process.exit(ok ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
