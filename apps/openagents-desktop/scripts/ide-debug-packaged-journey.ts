import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import { Schema } from "effect";

import {
  IdeDebugEvidenceInputSchema,
  IdeDebugPackagedJourneyReceiptSchema,
  validateIdeDebugCapturedEvidence,
} from "../src/ide/debug-evidence-contract.ts";
import { packagedArtifactTreeDigest, resolvePackagedApp } from "./ide-packaged-artifact.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const defaultInput = path.join(
  appRoot,
  "scripts",
  "fixtures",
  "ide-debug-evidence.unexecuted.json",
);
const defaultOutput = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-11-debug-packaged.json",
);

const argumentValue = (name: string, fallback: string): string => {
  const index = process.argv.indexOf(name);
  if (index < 0) return fallback;
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith("--"))
    throw new Error(`IDE-11 packaged journey requires a value for ${name}`);
  return path.resolve(value);
};

const inputPath = argumentValue("--input", defaultInput);
const outputPath = argumentValue("--output", defaultOutput);
if (!inputPath.startsWith(`${repositoryRoot}${path.sep}`))
  throw new Error("IDE-11 packaged input must be inside the repository");
const raw = readFileSync(inputPath, "utf8");
const input = Schema.decodeUnknownSync(IdeDebugEvidenceInputSchema)(JSON.parse(raw));
if (IdeDebugEvidenceInputSchema.guards.Unexecuted(input)) {
  throw new Error(
    `IDE-11 packaged journey refused unexecuted evidence: ${input.reason} Required runner: ${input.requiredRunner}`,
  );
}
validateIdeDebugCapturedEvidence(input);

const currentSha = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim();
if (input.candidateCommitSha !== currentSha)
  throw new Error("IDE-11 packaged journey evidence does not bind current HEAD");
const packagedApp = resolvePackagedApp();
const artifact = packagedArtifactTreeDigest(packagedApp);
if (
  artifact.sha256 !== input.artifact.treeSha256 ||
  artifact.files !== input.artifact.files ||
  artifact.bytes !== input.artifact.bytes
) {
  throw new Error("IDE-11 packaged journey artifact digest, file count, or byte count changed");
}

const evidencePaths = input.journeys.flatMap((journey) => [
  journey.screenshotRef,
  journey.traceRef,
  journey.receiptRef,
]);
for (const evidenceRef of evidencePaths) {
  const absolutePath = path.resolve(repositoryRoot, evidenceRef);
  if (!absolutePath.startsWith(`${repositoryRoot}${path.sep}`) || !existsSync(absolutePath)) {
    throw new Error(
      `IDE-11 packaged journey evidence is absent or outside the repository: ${evidenceRef}`,
    );
  }
  if (evidenceRef.endsWith(".json")) {
    const evidence = readFileSync(absolutePath, "utf8");
    for (const privateValue of [repositoryRoot, process.env.HOME ?? "__no_home__"]) {
      if (privateValue !== "" && evidence.includes(privateValue))
        throw new Error(`IDE-11 packaged evidence contains a private path: ${evidenceRef}`);
    }
    if (
      /(?:^|[^A-Za-z0-9_-])(?:(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}|-----BEGIN [A-Z ]*PRIVATE KEY-----)/u.test(
        evidence,
      )
    ) {
      throw new Error(`IDE-11 packaged evidence contains secret-shaped material: ${evidenceRef}`);
    }
  }
}

const receipt = Schema.decodeUnknownSync(IdeDebugPackagedJourneyReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-debug-packaged.v1",
  issue: "IDE-11",
  recordedAt: input.recordedAt,
  candidateCommitSha: input.candidateCommitSha,
  artifact: input.artifact,
  environment: input.environment,
  journeys: input.journeys,
  controls: input.controls,
  sources: input.sources,
  lifecycle: input.lifecycle,
  faultMatrix: input.faultMatrix,
  accessibilityMatrix: input.accessibilityMatrix,
  screenshotRefs: input.journeys.map((journey) => journey.screenshotRef),
  traceRefs: input.journeys.map((journey) => journey.traceRef),
  sourceEvidenceRef: path.relative(repositoryRoot, inputPath),
  passed: true,
});
mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, {
  encoding: "utf8",
  mode: 0o600,
});
process.stdout.write(`[openagents-desktop] IDE-11 debug packaged receipt: ${outputPath}\n`);
