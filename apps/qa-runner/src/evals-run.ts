#!/usr/bin/env bun
// Chill-eval CLI (#6183): one command kicks an eval; the result is a link.
//
// Holds the /login scenario fixed and compares two agent-config variants over
// it, then prints the comparison + the shareable /pro/evals/<id> link. With
// REAL chromium by default (the proof path); deterministic-fixtures mode for a
// no-network/no-spend demo via `--fixtures`.
//
// Usage:
//   bun run src/evals-run.ts --url https://openagents.com --out ./runs/eval
//     [--id login-compare] [--reps 1] [--fixtures] [--md]
//
// Variants here are illustrative agent configs expressed through the existing
// brain seam (NOT a khala-config edit): a "good" scripted variant vs a
// "regressed" scripted variant. Real model-A-vs-B variants plug in the same way
// (each variant supplies its own brain factory) without touching the
// khala-config/openrouter lane.

import { Effect } from "effect";
import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { type EvalVariant, runEval } from "./evals";
import { renderEvalConsole, renderEvalMarkdown } from "./evals-report";
import { makeFakeChromium } from "./fake-chromium";
import {
  executorPublicHomeSteps,
  executorPublicHomeStepsWrong,
  loginRegressionSteps,
  loginRegressionStepsWrong,
} from "./scenarios";
import { makeTarget, resolveTarget } from "./target";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--fixtures" || a === "--md") args[a.slice(2)] = true;
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

type EvalScenarioChoice = "login-regression" | "executor-public-home";

interface ScenarioChoice {
  readonly id: EvalScenarioChoice;
  readonly title: string;
  readonly label: string;
  readonly baselineNote: string;
  readonly candidateNote: string;
  readonly baselineAxis: string;
  readonly candidateAxis: string;
  readonly baselineSteps: typeof loginRegressionSteps;
  readonly candidateSteps: typeof loginRegressionStepsWrong;
  readonly fixturePages: Record<string, { text: string; html?: string }>;
}

const resolveScenarioChoice = (value: unknown): ScenarioChoice => {
  const id = typeof value === "string" ? value : "login-regression";
  if (id === "login-regression") {
    return {
      id,
      title: "Login scenario: baseline vs candidate",
      label: "/login renders sign-in",
      baselineNote: "current /login scenario",
      candidateNote: "asserts a redirect that should not happen (regressed)",
      baselineAxis: "filesystem:on",
      candidateAxis: "filesystem:off",
      baselineSteps: loginRegressionSteps,
      candidateSteps: loginRegressionStepsWrong,
      fixturePages: {
        "/login": {
          text: "Log in to OpenAgents",
          html: "<form>Log in to OpenAgents</form>",
        },
      },
    };
  }
  if (id === "executor-public-home") {
    return {
      id,
      title: "Executor public-home scenario: baseline vs candidate",
      label: "executor.sh public landing page renders",
      baselineNote: "read-only executor public-home verification",
      candidateNote: "asserts impossible executor copy (regressed)",
      baselineAxis: "executor-public:baseline",
      candidateAxis: "executor-public:impossible-copy",
      baselineSteps: executorPublicHomeSteps,
      candidateSteps: executorPublicHomeStepsWrong,
      fixturePages: {
        "/": {
          text: "Connect any agent to everything. Executor is an MCP gateway. Codex.",
          html: "<main>Connect any agent to everything. Executor is an MCP gateway. Codex.</main>",
        },
      },
    };
  }
  throw new Error(`unknown eval scenario "${id}"`);
};

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const fixtures = args.fixtures === true;
  let scenarioChoice: ScenarioChoice;
  try {
    scenarioChoice = resolveScenarioChoice(args.scenario);
  } catch (error) {
    console.error(error instanceof Error ? `error: ${error.message}` : String(error));
    process.exit(2);
  }
  const target =
    typeof args.url === "string"
      ? makeTarget({
          name: typeof args.name === "string" ? args.name : args.url,
          baseUrl: args.url,
        })
      : fixtures
        ? makeTarget({ name: "fixtures", baseUrl: "https://example.test" })
        : resolveTarget();
  const artifactDir =
    typeof args.out === "string" ? args.out : "./runs/eval-once";
  const id = typeof args.id === "string" ? args.id : "login-compare";
  const reps = typeof args.reps === "string" ? Number(args.reps) : 1;
  const proBaseUrl =
    typeof args["pro-base-url"] === "string"
      ? args["pro-base-url"]
      : "https://openagents.com";

  // In fixtures mode the backend serves a deterministic page (no network); the
  // real path uses real chromium against the Target.
  const chromium = fixtures
    ? makeFakeChromium({
        pages: scenarioChoice.fixturePages,
      })
    : undefined;

  const backend = () =>
    localBackend(chromium !== undefined ? { chromium } : {});

  const variants: ReadonlyArray<EvalVariant> = [
    {
      id: "baseline",
      label: "baseline",
      note: scenarioChoice.baselineNote,
      axis: { kind: "mcp_set", value: scenarioChoice.baselineAxis, baseline: true },
      brain: () => scriptedBrain(scenarioChoice.baselineSteps()),
      backend,
    },
    {
      id: "candidate",
      label: "candidate",
      note: scenarioChoice.candidateNote,
      axis: { kind: "mcp_set", value: scenarioChoice.candidateAxis },
      brain: () => scriptedBrain(scenarioChoice.candidateSteps()),
      backend,
    },
  ];

  const outcome = await Effect.runPromise(
    runEval({
      id,
      title: scenarioChoice.title,
      target,
      scenario: { id: scenarioChoice.id, label: scenarioChoice.label },
      variants,
      repetitions: Number.isFinite(reps) ? reps : 1,
      artifactDir,
    }),
  );

  console.log("=== CHILL-EVAL ===");
  console.log(renderEvalConsole(outcome.result));
  console.log("");
  console.log("eval.json:", outcome.resultPath);
  if (args.md === true) {
    console.log("");
    console.log("--- PR comment markdown ---");
    console.log(renderEvalMarkdown(outcome.result, { proBaseUrl }));
  }

  // Honest exit code: non-zero if ANY variant did not fully pass.
  const allPass = outcome.result.variants.every((v) => v.passRate === 1);
  process.exit(allPass ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
