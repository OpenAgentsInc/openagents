/**
 * `coder-effectiveness report` — score one graded Harbor job.
 *
 * The execution half of the suite already exists: `bench/run-suite.sh` packs
 * the working-tree CLI, runs a suite through Harbor, and leaves a job
 * directory. This command reads that directory and answers the question the
 * run was for.
 *
 *     bench/run-suite.sh bench/suites/tb2-cross-section.txt \
 *       --model openai/gpt-5.6-luna --jobs-dir /tmp/gym-jobs-run
 *
 *     pnpm run effectiveness:report -- /tmp/gym-jobs-run/<job> \
 *       --suite tb2-cross-section --lane proxy \
 *       --thresholds packages/coder-effectiveness/thresholds/tb2-cross-section.json
 *
 * EXIT CODES. 0 the gate passed, 1 a floor was breached, 2 the gate could not
 * be verified. The third code exists so a scheduled run on an unpriced lane
 * cannot be mistaken for a clean one by a CI step that only checks for zero.
 */

import { readFileSync } from "node:fs";

import { summarizeRun } from "./effectiveness.ts";
import { readHarborJob } from "./harbor-job.ts";
import {
  CODER_RATE_CATALOG,
  CODER_RATE_CATALOG_VERSION,
  type ModelRateRow,
  pricingFromModelsPayload,
} from "./pricing.ts";
import { renderReport } from "./render.ts";
import { evaluateThresholds, parseThresholds, type ThresholdGate } from "./thresholds.ts";

const USAGE = `Usage: coder-effectiveness report <job-dir> [options]

Arguments:
  <job-dir>              A completed Harbor job directory: the one holding
                         result.json, config.json, and one directory per trial.

Options:
  --suite <name>         Suite name recorded in the report. Default: terminal-bench@2.0
  --lane <proxy|local>   Lane the run used. Default: proxy
  --thresholds <file>    JSON floors to score the run against. Without it the
                         report is printed and no gate runs.
  --models <file>        A captured GET /api/v1/models body to price from,
                         instead of the pinned rate catalog. A model the served
                         catalog leaves unpriced stays unpriced here.
  --json                 Emit the report as JSON instead of text.
  -h, --help             Show this help.

Exit codes: 0 gate passed, 1 a floor was breached, 2 the gate was unverifiable.
An unverifiable gate is not a pass: a criterion could not be measured, most
often because the lane carries no published rate.`;

interface Arguments {
  readonly jobDir: string;
  readonly suite: string;
  readonly lane: string;
  readonly thresholdsPath: string | null;
  readonly modelsPath: string | null;
  readonly json: boolean;
}

/** `--` is what pnpm forwards, never an argument. */
const parseArguments = (argv: ReadonlyArray<string>): Arguments | "help" => {
  let jobDir: string | null = null;
  let suite = "terminal-bench@2.0";
  let lane = "proxy";
  let thresholdsPath: string | null = null;
  let modelsPath: string | null = null;
  let json = false;

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]!;
    if (argument === "--") continue;
    if (argument === "-h" || argument === "--help") return "help";
    if (argument === "--json") {
      json = true;
      continue;
    }
    if (argument === "--suite") {
      suite = expectValue(argv, (index += 1), "--suite");
      continue;
    }
    if (argument === "--lane") {
      lane = expectValue(argv, (index += 1), "--lane");
      continue;
    }
    if (argument === "--thresholds") {
      thresholdsPath = expectValue(argv, (index += 1), "--thresholds");
      continue;
    }
    if (argument === "--models") {
      modelsPath = expectValue(argv, (index += 1), "--models");
      continue;
    }
    if (argument.startsWith("-")) {
      throw new Error(`unknown option: ${argument}`);
    }
    if (jobDir !== null) {
      throw new Error(`unexpected extra argument: ${argument}`);
    }
    jobDir = argument;
  }

  if (jobDir === null) throw new Error("missing required <job-dir> argument");
  if (lane !== "proxy" && lane !== "local") {
    throw new Error(`--lane must be proxy or local, got: ${lane}`);
  }
  return { jobDir, suite, lane, thresholdsPath, modelsPath, json };
};

const expectValue = (argv: ReadonlyArray<string>, index: number, option: string): string => {
  const value = argv[index];
  if (value === undefined || value.startsWith("-")) {
    throw new Error(`${option} needs a value`);
  }
  return value;
};

const exitCodeFor = (gate: ThresholdGate | null): number => {
  if (gate === null) return 0;
  if (gate.status === "failed") return 1;
  if (gate.status === "unverifiable") return 2;
  return 0;
};

const main = (argv: ReadonlyArray<string>): number => {
  let parsed: Arguments | "help";
  try {
    parsed = parseArguments(argv);
  } catch (error) {
    process.stderr.write(`${(error as Error).message}\n\n${USAGE}\n`);
    return 1;
  }
  if (parsed === "help") {
    process.stdout.write(`${USAGE}\n`);
    return 0;
  }

  try {
    let catalog: Readonly<Record<string, ModelRateRow>> = CODER_RATE_CATALOG;
    let catalogVersion: string = CODER_RATE_CATALOG_VERSION;
    if (parsed.modelsPath !== null) {
      catalog = pricingFromModelsPayload(JSON.parse(readFileSync(parsed.modelsPath, "utf8")));
      catalogVersion = `served:${parsed.modelsPath}`;
    }

    const run = readHarborJob(parsed.jobDir, {
      suite: parsed.suite,
      lane: parsed.lane,
      rateCatalogVersion: catalogVersion,
    });
    const report = summarizeRun(run, catalog, catalogVersion);

    const gate =
      parsed.thresholdsPath === null
        ? null
        : evaluateThresholds(
            report,
            parseThresholds(JSON.parse(readFileSync(parsed.thresholdsPath, "utf8"))),
          );

    process.stdout.write(
      parsed.json ? `${JSON.stringify({ report, gate }, null, 2)}\n` : renderReport(report, gate),
    );
    return exitCodeFor(gate);
  } catch (error) {
    process.stderr.write(`coder-effectiveness: ${(error as Error).message}\n`);
    return 1;
  }
};

process.exitCode = main(process.argv.slice(2));
