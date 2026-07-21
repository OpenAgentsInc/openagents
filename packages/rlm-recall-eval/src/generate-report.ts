/**
 * Hermetic aggregate generator (`pnpm --filter @openagentsinc/rlm-recall-eval run eval:hermetic`).
 *
 * Runs the deterministic dense-recall matrix and writes the raw aggregate
 * artifact under `docs/rlm/`. The artifact carries NO timestamps or machine
 * facts, so a clean checkout reproduces it byte for byte. It is the raw evidence
 * the STE report cites. This generator makes NO product claim.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { Effect } from "effect";

import { runHermeticMatrix, type HarnessOutput } from "./harness.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, "..", "..", "..");
export const HERMETIC_ARTIFACT_PATH = resolve(
  REPO_ROOT,
  "docs",
  "rlm",
  "rlm-recall-eval-hermetic-aggregate.json",
);

/** Serialise the aggregate deterministically (stable key order, trailing newline). */
export const serializeAggregate = (output: HarnessOutput): string =>
  `${JSON.stringify(output, null, 2)}\n`;

const headline = (output: HarnessOutput): string => {
  const lines: Array<string> = [];
  lines.push(`total hermetic runs: ${String(output.meta.totalRuns)}`);
  lines.push(
    "tier | success | partial | incorrect | refused | cite.cov.p50 | calls.p95 | cost.p95 (usd)",
  );
  for (const t of output.tierAggregates) {
    lines.push(
      [
        t.tierId,
        t.outcomes.success,
        t.outcomes.partial,
        t.outcomes.incorrect,
        t.outcomes.refused,
        t.citationCoverage.p50.toFixed(2),
        t.overall.modelCalls.p95.toFixed(0),
        t.overall.cost.known.p95.toFixed(8),
      ].join(" | "),
    );
  }
  lines.push(
    `escalation gate wouldPass=${String(output.gates.escalation.wouldPass)} admitted=${String(output.gates.escalation.admitted)}`,
  );
  lines.push(
    `depth>1 gate wouldPass=${String(output.gates.depth.wouldPass)} admitted=${String(output.gates.depth.admitted)}`,
  );
  lines.push(
    `honesty probe: runs=${String(output.honesty.runs)} unknownUsage=${String(output.honesty.unknownUsageCount)} excludedCorrectly=${String(output.honesty.costExcludedCorrectly)}`,
  );
  return lines.join("\n");
};

const main = Effect.gen(function* () {
  const output = yield* runHermeticMatrix();
  mkdirSync(dirname(HERMETIC_ARTIFACT_PATH), { recursive: true });
  writeFileSync(HERMETIC_ARTIFACT_PATH, serializeAggregate(output), "utf8");
  yield* Effect.sync(() => {
    process.stdout.write(`wrote ${HERMETIC_ARTIFACT_PATH}\n`);
    process.stdout.write(`${headline(output)}\n`);
  });
});

const invokedDirectly =
  process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href;
if (invokedDirectly) {
  Effect.runPromise(main).catch((error: unknown) => {
    process.stderr.write(`rlm-recall-eval generate-report failed: ${String(error)}\n`);
    process.exit(1);
  });
}
