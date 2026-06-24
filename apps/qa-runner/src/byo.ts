#!/usr/bin/env bun
// `qa` — the OSS, local-first, BYO-model QA CLI (issue #6191 / Rhys req #5).
//
// Runs an autonomous e2e scenario LOCALLY against any target, driven by ANY
// OpenAI-compatible model you bring (model + base URL + key via flags/env). It
// records a playable video + Playwright trace + per-step screenshots, and
// DISTILLS the session into a COMMITTED executor-style e2e test file.
//
//   *** No OpenAgents account, login, or key is required. ***
//
// Khala is just one option (point --base-url at openagents.com if you want it).
// OpenAgents-specific add-ons (Cloud VMs, /pro, receipts, settlement) are NOT
// used by this path and are NOT dependencies of it.
//
// Usage:
//   qa run \
//     --url https://your-dev-server.example \
//     --goal "open /login, confirm the sign-in form renders, finish" \
//     --model gpt-4o-mini \
//     --base-url https://api.openai.com/v1 \
//     --api-key sk-... \
//     --out ./runs/qa \
//     [--emit generated/<name>.e2e.test.ts] [--headed] [--max-turns 16]
//     [--allow-keyless]
//
//   # Deterministic, no-network, no-key, no-OpenAgents proof of the local loop
//   # (drives a canned /login scenario; emits video + a committed test):
//   qa run --fake-model --url https://example.test --out ./runs/qa-fake
//
// Env equivalents (de-facto OpenAI standard, so existing CI works as-is):
//   QA_MODEL / OPENAI_MODEL, QA_BASE_URL / OPENAI_BASE_URL, QA_API_KEY / OPENAI_API_KEY
//
// Exit code is honest: 0 only on a clean pass + an admissible distilled test.

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Effect } from "effect";
import { localBackend } from "./backend";
import { makeByoChatClient, resolveByoModelConfig, ByoModelConfigError } from "./byo-model";
import { assessCandidate, distill } from "./distiller";
import { makeFakeChromium } from "./fake-chromium";
import type { ChatClient } from "./khala-driver";
import { runKhalaSession } from "./khala-session";
import { makeTarget } from "./target";

const DEFAULT_GOAL =
  "Verify the login page works on this site: open /login, confirm the sign-in form renders " +
  '(the page shows "Log in"), and confirm it does NOT redirect to the homepage ' +
  "(the URL still includes /login). Assert each of these, screenshot the login page, then finish.";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--headed" || a === "--fake-model" || a === "--allow-keyless") {
      args[a.slice(2)] = true;
    } else if (a.startsWith("--")) {
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

const HELP = `qa — OSS, local-first, BYO-model autonomous QA runner

USAGE
  qa run [options]

OPTIONS
  --url <url>          Target dev/prod server to drive (required for a real run).
  --goal "<text>"      What the agent should verify (defaults to a /login check).
  --model <id>         BYO model id (or env QA_MODEL / OPENAI_MODEL).
  --base-url <url>     OpenAI-compatible base URL (or env QA_BASE_URL / OPENAI_BASE_URL).
  --api-key <key>      Bearer key (or env QA_API_KEY / OPENAI_API_KEY). Never printed.
  --allow-keyless      Permit a keyless local server (llama.cpp / vLLM / Ollama shim).
  --out <dir>          Artifact dir for video/trace/screenshots/result (default ./runs/qa).
  --emit <path>        Where to write the distilled e2e test (default generated/<slug>.e2e.test.ts).
  --max-turns <n>      Hard cap on model turns (default 16).
  --headed             Run a visible browser (default headless).
  --fake-model         Deterministic, no-network, no-key, no-OpenAgents proof of
                       the loop. Drives a canned /login scenario against a fake
                       page; still emits a real video + a committed e2e test.

NO OPENAGENTS LOGIN IS REQUIRED. Khala is one optional backend, not a dependency.
`;

/** A deterministic, no-network chat client: replays canned /login actions. */
function fakeModelChat(): ChatClient {
  const replies = [
    '{"action":"navigate","url":"/login"}',
    '{"action":"waitFor","condition":{"kind":"text-visible","value":"Log in"}}',
    '{"action":"screenshot","label":"login-page"}',
    '{"action":"assert","label":"stays at /login","check":{"kind":"url-includes","value":"/login"}}',
    '{"action":"assert","label":"shows the sign-in form","check":{"kind":"text-contains","value":"Log in"}}',
    '{"action":"done","verdict":"pass","summary":"login page renders and stays at /login"}',
  ];
  let i = 0;
  return { complete: async () => replies[i++] ?? '{"action":"fail","reason":"out of script"}' };
}

async function runCommand(argv: ReadonlyArray<string>): Promise<number> {
  const args = parseArgs(argv);
  const fake = args["fake-model"] === true;
  const baseUrlTarget = typeof args.url === "string" ? args.url : fake ? "https://example.test" : undefined;
  if (!baseUrlTarget) {
    console.error("error: --url <target> is required (the dev/prod server to drive).\n");
    console.error(HELP);
    return 2;
  }
  const goal = typeof args.goal === "string" ? args.goal : DEFAULT_GOAL;
  const artifactDir = typeof args.out === "string" ? args.out : "./runs/qa";
  const maxTurns = typeof args["max-turns"] === "string" ? Number(args["max-turns"]) : 16;
  const emitPath = typeof args.emit === "string" ? args.emit : undefined;
  const headed = args.headed === true;

  // Resolve the brain (model). The fake path is the no-key, no-network, no-login
  // deterministic proof; otherwise build a BYO OpenAI-compatible client.
  let chat: ChatClient;
  let modelLabel: string;
  let backend = localBackend();
  if (fake) {
    chat = fakeModelChat();
    modelLabel = "fake-model (deterministic, no network)";
    // Use a fake chromium so the deterministic path needs no real browser/network.
    backend = localBackend({
      chromium: makeFakeChromium({
        pages: { "/login": { text: "Log in", html: "<form>Log in</form>" } },
      }),
    });
    console.log("=== qa run (BYO-model, OSS, local-first) ===");
    console.log("MODE:    --fake-model — deterministic, no network, NO OpenAgents login, NO model key.");
  } else {
    let config;
    try {
      config = resolveByoModelConfig({
        flags: {
          ...(typeof args.model === "string" ? { model: args.model } : {}),
          ...(typeof args["base-url"] === "string" ? { baseUrl: args["base-url"] } : {}),
          ...(typeof args["api-key"] === "string" ? { apiKey: args["api-key"] } : {}),
        },
        allowKeyless: args["allow-keyless"] === true,
      });
    } catch (error) {
      if (error instanceof ByoModelConfigError) {
        console.error(`error: ${error.message}\n`);
        console.error(HELP);
        return 2;
      }
      throw error;
    }
    chat = makeByoChatClient(config, { timeoutMs: 90_000, maxTokens: 1500 }) as ChatClient;
    modelLabel = config.model;
    console.log("=== qa run (BYO-model, OSS, local-first) ===");
    console.log(`model:   ${config.model}  (base ${config.baseUrl})`);
    console.log(`key src: ${config.keySource}`); // label only, never the value
    console.log("login:   NONE — this is a bring-your-own-model run, no OpenAgents account.");
  }
  console.log(`target:  ${baseUrlTarget}`);
  console.log(`goal:    ${goal}\n`);

  const outcome = await Effect.runPromise(
    runKhalaSession({
      target: makeTarget({ name: baseUrlTarget, baseUrl: baseUrlTarget }),
      backend,
      chat,
      goal,
      artifactDir,
      maxTurns,
      model: modelLabel,
      ...(headed ? { headed: true } : {}),
    }),
  );

  console.log("--- session ---");
  console.log("verdict:   ", outcome.verdict);
  console.log("result:    ", outcome.result.status, `(${outcome.resultPath})`);
  console.log("artifacts: ", JSON.stringify(outcome.result.artifacts));
  for (const step of outcome.result.steps) {
    console.log(`  [${step.status === "ok" ? "PASS" : "FAIL"}] ${step.kind}: ${step.label}`);
  }
  if (outcome.result.failure) console.log("failure:   ", outcome.result.failure);

  // Distill the session into a COMMITTED, runnable e2e test (the review artifact).
  console.log("\n--- distill -> committed e2e test ---");
  const distilled = distill(outcome.trace);
  const assessment = assessCandidate(distilled, outcome.trace);
  console.log("verification class:  ", distilled.verificationClass);
  console.log("e2e assertions:      ", distilled.emitters.e2e.assertionCount);
  console.log("candidate admissible:", assessment.admissible, assessment.reasons.length ? assessment.reasons : "");

  const outFile = resolve(emitPath ?? join("generated", `${distilled.emitters.e2e.slug}.e2e.test.ts`));
  mkdirSync(dirname(outFile), { recursive: true });
  writeFileSync(outFile, distilled.emitters.e2e.source);
  console.log("emitted committed test:", outFile);

  const ok = outcome.verdict === "pass" && outcome.result.status === "pass" && assessment.admissible;
  if (!ok) {
    console.log(
      "\nNOTE: not a clean pass. The session/verdict above is reported HONESTLY; a weak " +
        "model can fail to reach a verdict — that is real, never faked.",
    );
  }
  return ok ? 0 : 1;
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  const [command, ...rest] = argv;
  if (!command || command === "help" || command === "--help" || command === "-h") {
    console.log(HELP);
    process.exit(command ? 0 : 2);
  }
  if (command === "run") {
    process.exit(await runCommand(rest));
  }
  console.error(`error: unknown command "${command}"\n`);
  console.error(HELP);
  process.exit(2);
}

if (import.meta.main) {
  await main();
}

export { runCommand };
