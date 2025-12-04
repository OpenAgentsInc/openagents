#!/usr/bin/env bun
import { parseArgs } from "node:util";
import { Effect } from "effect";
import * as BunContext from "@effect/platform-bun/BunContext";
import { summarizeUsage } from "./store.js";

interface CliOptions {
  rootDir: string;
  period: "day" | "week" | "month";
  json: boolean;
}

const parseCliArgs = (): CliOptions => {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      dir: { type: "string" },
      root: { type: "string" },
      period: { type: "string" },
      json: { type: "boolean" },
      help: { type: "boolean", short: "h" },
    },
  });

  if (values.help) {
    console.log(`Usage: bun src/usage/cli.ts summary [--period day|week|month] [--dir <path>] [--json]`);
    process.exit(0);
  }

  const rootDir =
    (values.dir as string | undefined) ??
    (values.root as string | undefined) ??
    process.cwd();

  const periodRaw = (values.period as string | undefined) ?? "day";
  const period = ["day", "week", "month"].includes(periodRaw) ? (periodRaw as CliOptions["period"]) : "day";

  return {
    rootDir,
    period,
    json: Boolean(values.json),
  };
};

const main = async () => {
  const options = parseCliArgs();
  const summary = await Effect.runPromise(
    summarizeUsage({ rootDir: options.rootDir, period: options.period }).pipe(Effect.provide(BunContext.layer)),
  );

  if (options.json) {
    console.log(JSON.stringify(summary, null, 2));
    return;
  }

  console.log(`Usage summary (period=${options.period})`);
  console.log("-----------------------------------");
  for (const [key, totals] of Object.entries(summary.byPeriod)) {
    console.log(`${key}`);
    console.log(`  sessions: ${totals.sessions}`);
    console.log(`  subtasks: ${totals.subtasks}`);
    console.log(`  tokens (in/out/cache read/cache create): ${totals.inputTokens}/${totals.outputTokens}/${totals.cacheReadTokens}/${totals.cacheCreationTokens}`);
    console.log(`  cost (usd): $${totals.totalCostUsd.toFixed(4)}`);
  }
  console.log("-----------------------------------");
  console.log("Overall:");
  console.log(
    `  sessions=${summary.overall.sessions} subtasks=${summary.overall.subtasks} tokens=${summary.overall.inputTokens}/${summary.overall.outputTokens}/${summary.overall.cacheReadTokens}/${summary.overall.cacheCreationTokens} cost=$${summary.overall.totalCostUsd.toFixed(4)}`,
  );
};

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
