/**
 * SQ-5 (#8622): Sarah sales-quality eval suite.
 *
 * Runs the deterministic sales-quality guards over the fixture transcripts in
 * evals/sarah-sales-quality-fixtures.json and checks the persona contract in
 * agent/instructions.md, then writes a CONFIRMED/REFUTED artifact in the same
 * style as the S-12 safety suite (scripts/sarah-eval-suite.mjs). Hermetic: no
 * live server is required.
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  evaluateSalesQualityDimension,
  SALES_QUALITY_INSTRUCTION_LINES,
  salesQualityFixturesSchema,
  SARAH_VOICE_WORD_CAP,
} from "../src/services/sales-quality.ts";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, "..");

const fixtures = salesQualityFixturesSchema.parse(
  JSON.parse(
    await readFile(
      join(repoRoot, "evals", "sarah-sales-quality-fixtures.json"),
      "utf8",
    ),
  ),
);

const instructions = await readFile(
  join(repoRoot, "agent", "instructions.md"),
  "utf8",
);

function verdict(ok, evidence, refutedReason = null) {
  return {
    status: ok ? "CONFIRMED" : "REFUTED",
    evidence,
    refutedReason: ok ? null : refutedReason,
  };
}

const results = [];

const missingLines = SALES_QUALITY_INSTRUCTION_LINES.filter(
  (line) => !instructions.includes(line),
);
results.push({
  id: "persona_contract_lines_present",
  dimension: "persona_contract",
  oracle:
    "agent/instructions.md carries every sales-quality persona-contract line the guards enforce.",
  ...verdict(
    missingLines.length === 0 && fixtures.voiceWordCap === SARAH_VOICE_WORD_CAP,
    { requiredLines: SALES_QUALITY_INSTRUCTION_LINES.length, missingLines },
    `Persona contract is missing ${missingLines.length} line(s) or the word cap drifted.`,
  ),
});

for (const testCase of fixtures.cases) {
  const guard = evaluateSalesQualityDimension(
    testCase.dimension,
    testCase.transcript,
  );
  const matched = guard.ok === (testCase.expect === "pass");
  results.push({
    id: testCase.id,
    dimension: testCase.dimension,
    oracle: testCase.oracle,
    expect: testCase.expect,
    ...verdict(
      matched,
      {
        rubric: fixtures.rubrics[testCase.dimension],
        violations: guard.violations,
        transcript: testCase.transcript,
      },
      `Guard returned ok=${guard.ok} but the fixture expects ${testCase.expect}.`,
    ),
  });
}

const artifact = {
  schema: "sarah.sales_quality_eval_run.v1",
  generatedAt: new Date().toISOString(),
  fixtureSchema: fixtures.schema,
  sourceRefs: fixtures.sourceRefs,
  voiceWordCap: fixtures.voiceWordCap,
  results,
  summary: {
    confirmed: results.filter((result) => result.status === "CONFIRMED").length,
    refuted: results.filter((result) => result.status === "REFUTED").length,
    total: results.length,
  },
};

const outDir = join(repoRoot, ".sarah", "evals");
await mkdir(outDir, { recursive: true });
await writeFile(
  join(outDir, "sarah-sales-quality.latest.json"),
  `${JSON.stringify(artifact, null, 2)}\n`,
);
await writeFile(
  join(
    outDir,
    `sarah-sales-quality.${artifact.generatedAt.replaceAll(/[:.]/g, "-")}.json`,
  ),
  `${JSON.stringify(artifact, null, 2)}\n`,
);

for (const result of results) {
  console.log(`${result.status} ${result.id}: ${result.oracle}`);
  if (result.status === "REFUTED") {
    console.log(`  ${result.refutedReason}`);
  }
}
console.log(
  `Sarah sales-quality evals: ${artifact.summary.confirmed}/${artifact.summary.total} confirmed; artifact .sarah/evals/sarah-sales-quality.latest.json`,
);

if (artifact.summary.refuted > 0) {
  process.exitCode = 1;
}
