import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import { Schema } from "effect";

import {
  IdeDebugBenchmarkReceiptSchema,
  IdeDebugEvidenceInputSchema,
  validateIdeDebugCapturedEvidence,
} from "../src/ide/debug-evidence-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const defaultInput = path.join(
  appRoot,
  "scripts",
  "fixtures",
  "ide-debug-evidence.unexecuted.json",
);
const defaultOutput = path.join(appRoot, "benchmarks", "ide", "2026-07-20-ide-11-debug.json");

const argumentValue = (name: string, fallback: string): string => {
  const index = process.argv.indexOf(name);
  if (index < 0) return fallback;
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith("--"))
    throw new Error(`IDE-11 benchmark requires a value for ${name}`);
  return path.resolve(value);
};

const inputPath = argumentValue("--input", defaultInput);
const outputPath = argumentValue("--output", defaultOutput);
if (!inputPath.startsWith(`${repositoryRoot}${path.sep}`))
  throw new Error("IDE-11 benchmark input must be inside the repository");
const raw = readFileSync(inputPath, "utf8");
const input = Schema.decodeUnknownSync(IdeDebugEvidenceInputSchema)(JSON.parse(raw));
if (IdeDebugEvidenceInputSchema.guards.Unexecuted(input)) {
  throw new Error(
    `IDE-11 benchmark refused unexecuted evidence: ${input.reason} Required runner: ${input.requiredRunner}`,
  );
}
validateIdeDebugCapturedEvidence(input);

const currentSha = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim();
if (input.candidateCommitSha !== currentSha) {
  throw new Error(
    `IDE-11 benchmark refused stale evidence candidate ${input.candidateCommitSha}; current HEAD is ${currentSha}`,
  );
}
for (const privateValue of [repositoryRoot, process.env.HOME ?? "__no_home__"]) {
  if (privateValue !== "" && raw.includes(privateValue))
    throw new Error("IDE-11 benchmark evidence contains a private absolute path");
}
if (
  /(?:^|[^A-Za-z0-9_-])(?:(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}|-----BEGIN [A-Z ]*PRIVATE KEY-----)/u.test(
    raw,
  )
) {
  throw new Error("IDE-11 benchmark evidence contains secret-shaped material");
}

const receipt = Schema.decodeUnknownSync(IdeDebugBenchmarkReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-debug-benchmark.v1",
  issue: "IDE-11",
  recordedAt: input.recordedAt,
  candidateCommitSha: input.candidateCommitSha,
  environment: input.environment,
  artifact: input.artifact,
  journeys: input.journeys,
  controls: input.controls,
  sources: input.sources,
  lifecycle: input.lifecycle,
  faultMatrix: input.faultMatrix,
  accessibilityMatrix: input.accessibilityMatrix,
  metrics: input.metrics,
  policy: input.policy,
  security: input.security,
  resources: input.resources,
  targets: input.targets,
  nativeDecision: input.nativeDecision,
  ownerDisposition: input.ownerDisposition,
  assuranceLifecycle: input.assuranceLifecycle,
  sourceEvidenceRef: path.relative(repositoryRoot, inputPath),
  passed: true,
});
mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, {
  encoding: "utf8",
  mode: 0o600,
});
process.stdout.write(`[openagents-desktop] IDE-11 debug benchmark receipt: ${outputPath}\n`);
