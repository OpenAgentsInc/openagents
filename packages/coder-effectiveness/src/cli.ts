/**
 * `coder-effectiveness report` — score one graded Harbor job.
 *
 * The execution half of the suite already exists: `bench/run-suite.sh` packs
 * the working-tree CLI, runs a suite through Harbor, and leaves a job
 * directory. This command reads that directory and answers the question the
 * run was for.
 *
 *     bench/run-suite.sh bench/suites/tb2-cross-section.suite.json \
 *       --model openai/gpt-5.6-luna --jobs-dir /tmp/gym-jobs-run
 *
 *     pnpm run effectiveness:report -- /tmp/gym-jobs-run/<job> \
 *       --suite tb2-cross-section --lane proxy \
 *       --suite-manifest bench/suites/tb2-cross-section.suite.json \
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
import { appendResultRow } from "./results-store.ts";
import { classifyRun, parseSuiteManifest, type RunClassification } from "./suite-manifest.ts";
import { evaluateThresholds, parseThresholds, type ThresholdGate } from "./thresholds.ts";

const USAGE = `Usage: coder-effectiveness report <job-dir> [options]

Arguments:
  <job-dir>              A completed Harbor job directory: the one holding
                         result.json, config.json, and one directory per trial.

Options:
  --suite <name>         Suite name recorded in the report. Default: terminal-bench@2.0
  --suite-manifest <file>
                         The suite manifest this run claims to have covered.
                         Required by --append: a run that names no manifest has
                         nothing to say which pinned task list it measured. A
                         run that did not cover every pinned task is a smoke
                         run, whatever it was called, and cannot be recorded.
  --lane <proxy|local>   Lane the run used. Default: proxy
  --thresholds <file>    JSON floors to score the run against. Without it the
                         report is printed and no gate runs.
  --models <file>        A captured GET /api/v1/models body to price from,
                         instead of the pinned rate catalog. A model the served
                         catalog leaves unpriced stays unpriced here.
  --append <store>       Append this run to an append-only bench-results store,
                         chained to the receipt of the row before it. Refuses a
                         Harbor job the store already holds, and refuses to
                         extend a store that does not verify.
  --json                 Emit the report as JSON instead of text.
  -h, --help             Show this help.

Exit codes: 0 gate passed, 1 a floor was breached, 2 the gate was unverifiable,
3 the run was scored but --append refused to record it. An unverifiable gate is
not a pass: a criterion could not be measured, most often because the lane
carries no published rate. A non-zero gate always outranks 3 — a breach matters
more than a bookkeeping refusal.`;

/**
 * Its own code, so a scheduled run whose result never reached the store is not
 * reported as a clean pass. It only ever replaces a 0: a gate that failed or
 * could not be verified is the more important finding.
 */
const APPEND_REFUSED_EXIT = 3;

interface Arguments {
  readonly jobDir: string;
  readonly suite: string;
  readonly manifestPath: string | null;
  readonly lane: string;
  readonly thresholdsPath: string | null;
  readonly modelsPath: string | null;
  readonly appendPath: string | null;
  readonly json: boolean;
}

/** `--` is what pnpm forwards, never an argument. */
const parseArguments = (argv: ReadonlyArray<string>): Arguments | "help" => {
  let jobDir: string | null = null;
  let suite = "terminal-bench@2.0";
  let manifestPath: string | null = null;
  let lane = "proxy";
  let thresholdsPath: string | null = null;
  let modelsPath: string | null = null;
  let appendPath: string | null = null;
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
    if (argument === "--suite-manifest") {
      manifestPath = expectValue(argv, (index += 1), "--suite-manifest");
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
    if (argument === "--append") {
      appendPath = expectValue(argv, (index += 1), "--append");
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
  return { jobDir, suite, manifestPath, lane, thresholdsPath, modelsPath, appendPath, json };
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

    const classification: RunClassification | null =
      parsed.manifestPath === null
        ? null
        : classifyRun(
            parseSuiteManifest(JSON.parse(readFileSync(parsed.manifestPath, "utf8"))),
            run.trials.map((trial) => trial.task),
          );

    const gate =
      parsed.thresholdsPath === null
        ? null
        : evaluateThresholds(
            report,
            parseThresholds(JSON.parse(readFileSync(parsed.thresholdsPath, "utf8"))),
            classification,
          );

    const appended =
      parsed.appendPath === null
        ? null
        : appendResultRow(parsed.appendPath, report, gate, classification, {
            recordedAt: new Date().toISOString(),
          });

    process.stdout.write(
      parsed.json
        ? `${JSON.stringify({ report, classification, gate, appended }, null, 2)}\n`
        : renderReport(report, gate, classification),
    );
    if (appended !== null && !parsed.json) {
      process.stdout.write(
        appended.appended
          ? `\nAppended to ${parsed.appendPath!} as ${appended.row.receipt}\n`
          : `\nNot appended (${appended.refusal}): ${appended.reason}\n`,
      );
    }

    const gateCode = exitCodeFor(gate);
    if (gateCode !== 0) return gateCode;
    return appended !== null && !appended.appended ? APPEND_REFUSED_EXIT : 0;
  } catch (error) {
    process.stderr.write(`coder-effectiveness: ${(error as Error).message}\n`);
    return 1;
  }
};

process.exitCode = main(process.argv.slice(2));
