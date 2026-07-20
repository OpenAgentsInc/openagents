import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import { Schema } from "effect";

import {
  IdeDebugAcceptanceReceiptSchema,
  IdeDebugBenchmarkReceiptSchema,
  IdeDebugPackagedJourneyReceiptSchema,
  validateIdeDebugCapturedEvidence,
} from "../src/ide/debug-evidence-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide");
const defaultBenchmark = path.join(benchmarkRoot, "2026-07-20-ide-11-debug.json");
const defaultPackaged = path.join(benchmarkRoot, "2026-07-20-ide-11-debug-packaged.json");
const defaultOutput = path.join(benchmarkRoot, "2026-07-20-ide-11-debug-acceptance.json");

const argumentValue = (name: string, fallback: string): string => {
  const index = process.argv.indexOf(name);
  if (index < 0) return fallback;
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith("--"))
    throw new Error(`IDE-11 acceptance requires a value for ${name}`);
  return path.resolve(value);
};

const benchmarkPath = argumentValue("--benchmark", defaultBenchmark);
const packagedPath = argumentValue("--packaged", defaultPackaged);
const outputPath = argumentValue("--output", defaultOutput);
for (const evidencePath of [benchmarkPath, packagedPath]) {
  if (!existsSync(evidencePath))
    throw new Error(`IDE-11 acceptance evidence is absent: ${evidencePath}`);
}
const benchmarkRaw = readFileSync(benchmarkPath, "utf8");
const packagedRaw = readFileSync(packagedPath, "utf8");
const benchmark = Schema.decodeUnknownSync(IdeDebugBenchmarkReceiptSchema)(
  JSON.parse(benchmarkRaw),
);
const packaged = Schema.decodeUnknownSync(IdeDebugPackagedJourneyReceiptSchema)(
  JSON.parse(packagedRaw),
);
validateIdeDebugCapturedEvidence(benchmark);
if (benchmark.candidateCommitSha !== packaged.candidateCommitSha)
  throw new Error("IDE-11 acceptance evidence candidate SHAs differ");
if (benchmark.artifact.treeSha256 !== packaged.artifact.treeSha256)
  throw new Error("IDE-11 acceptance artifact digests differ");
if (benchmark.sourceEvidenceRef !== packaged.sourceEvidenceRef)
  throw new Error("IDE-11 acceptance source evidence refs differ");
for (const [label, benchmarkValue, packagedValue] of [
  ["environment", benchmark.environment, packaged.environment],
  ["artifact", benchmark.artifact, packaged.artifact],
  ["journeys", benchmark.journeys, packaged.journeys],
  ["controls", benchmark.controls, packaged.controls],
  ["sources", benchmark.sources, packaged.sources],
  ["lifecycle", benchmark.lifecycle, packaged.lifecycle],
  ["fault matrix", benchmark.faultMatrix, packaged.faultMatrix],
  ["accessibility matrix", benchmark.accessibilityMatrix, packaged.accessibilityMatrix],
] as const) {
  if (JSON.stringify(benchmarkValue) !== JSON.stringify(packagedValue)) {
    throw new Error(`IDE-11 acceptance ${label} evidence differs between receipts`);
  }
}

const mainEvaluationSha = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim();
execFileSync(
  "git",
  ["merge-base", "--is-ancestor", benchmark.candidateCommitSha, mainEvaluationSha],
  { cwd: repositoryRoot },
);
const rollbackTargetSha = execFileSync("git", ["rev-parse", `${benchmark.candidateCommitSha}^`], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim();
execFileSync("git", ["diff", "--check"], { cwd: repositoryRoot, stdio: "inherit" });
const boundary = execFileSync(
  process.execPath,
  ["--import", "tsx", path.join(appRoot, "scripts", "check-ide-boundaries.ts")],
  {
    cwd: repositoryRoot,
    encoding: "utf8",
  },
);
if (!boundary.includes("PASS"))
  throw new Error("IDE-11 acceptance authority-boundary check failed");

for (const [name, source] of [
  ["benchmark", benchmarkRaw],
  ["packaged", packagedRaw],
] as const) {
  for (const privateValue of [repositoryRoot, process.env.HOME ?? "__no_home__"]) {
    if (privateValue !== "" && source.includes(privateValue))
      throw new Error(`IDE-11 ${name} receipt contains a private path`);
  }
  if (
    /(?:^|[^A-Za-z0-9_-])(?:(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}|-----BEGIN [A-Z ]*PRIVATE KEY-----)/u.test(
      source,
    )
  ) {
    throw new Error(`IDE-11 ${name} receipt contains secret-shaped material`);
  }
}

const benchmarkRef = path.relative(repositoryRoot, benchmarkPath);
const packagedRef = path.relative(repositoryRoot, packagedPath);
const evidenceRefs = [
  benchmarkRef,
  packagedRef,
  benchmark.sourceEvidenceRef,
  benchmark.artifact.artifactRef,
  ...benchmark.journeys.flatMap((journey) => [
    journey.screenshotRef,
    journey.traceRef,
    journey.receiptRef,
  ]),
  ...benchmark.controls.map((control) => control.receiptRef),
  ...benchmark.sources.map((source) => source.evidenceRef),
  ...benchmark.lifecycle.map((row) => row.cleanupReceiptRef),
];
const receipt = Schema.decodeUnknownSync(IdeDebugAcceptanceReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-debug-acceptance.v1",
  issue: "IDE-11",
  generatedAt: new Date().toISOString(),
  candidateCommitSha: benchmark.candidateCommitSha,
  mainEvaluationSha,
  artifactTreeSha256: benchmark.artifact.treeSha256,
  benchmarkRef,
  packagedRef,
  evidenceRefs,
  exactReviewerRef: "docs/ide/2026-07-20-ide-11-effect-dap-graph.md#review-and-release-state",
  ownerDisposition: benchmark.ownerDisposition,
  assuranceLifecycle: benchmark.assuranceLifecycle,
  rollbackTargetSha,
  rustAdmitted: benchmark.nativeDecision.rustAdmitted,
  remainingGaps: [
    "IDE-12 adds Git mutation and delivery receipts. IDE-11 does not claim that authority.",
    "Windows and Linux packaged journeys stay unclaimed unless their target rows bind exact passing evidence.",
    "Independent owner and AssuranceSpec acceptance remain unreviewed.",
  ],
  passed: true,
});
mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, {
  encoding: "utf8",
  mode: 0o600,
});
process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
