import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import { Schema } from "effect";

import {
  IdeSourceControlAcceptanceReceiptSchema,
  IdeSourceControlBenchmarkReceiptSchema,
  IdeSourceControlPackagedReceiptSchema,
} from "../src/ide/source-control-evidence-contract.ts";
import { packagedArtifactTreeDigest, resolvePackagedApp } from "./ide-packaged-artifact.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const evidenceRoot = path.join(appRoot, "benchmarks", "ide");
const benchmarkPath = path.join(evidenceRoot, "2026-07-20-ide-12-source-control.json");
const packagedPath = path.join(evidenceRoot, "2026-07-20-ide-12-source-control-packaged.json");
const tracePath = path.join(evidenceRoot, "2026-07-20-ide-12-source-control-packaged-trace.json");
const outputPath = path.join(evidenceRoot, "2026-07-20-ide-12-source-control-acceptance.json");

for (const evidencePath of [benchmarkPath, packagedPath, tracePath]) {
  if (!existsSync(evidencePath)) throw new Error(`IDE-12 acceptance evidence is absent: ${evidencePath}`);
}
const benchmarkRaw = readFileSync(benchmarkPath, "utf8");
const packagedRaw = readFileSync(packagedPath, "utf8");
const traceRaw = readFileSync(tracePath, "utf8");
const benchmark = Schema.decodeUnknownSync(IdeSourceControlBenchmarkReceiptSchema)(JSON.parse(benchmarkRaw));
const packaged = Schema.decodeUnknownSync(IdeSourceControlPackagedReceiptSchema)(JSON.parse(packagedRaw));
if (benchmark.candidateCommitSha !== packaged.candidateCommitSha) throw new Error("IDE-12 evidence candidate SHAs differ");
if (!benchmark.passed || !packaged.passed || Object.values(packaged.checks).some(value => !value)) throw new Error("IDE-12 evidence has a failed check");
const artifact = packagedArtifactTreeDigest(resolvePackagedApp());
if (artifact.sha256 !== packaged.artifactTreeSha256) throw new Error("IDE-12 packaged artifact digest changed after capture");

const evaluationSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim();
execFileSync("git", ["merge-base", "--is-ancestor", benchmark.candidateCommitSha, evaluationSha], { cwd: repositoryRoot });
execFileSync("git", ["diff", "--check"], { cwd: repositoryRoot, stdio: "inherit" });
const boundary = execFileSync(process.execPath, ["--import", "tsx", path.join(appRoot, "scripts", "check-ide-boundaries.ts")], { cwd: repositoryRoot, encoding: "utf8" });
if (!boundary.includes("PASS")) throw new Error("IDE-12 authority-boundary check failed");
for (const [label, source] of [["benchmark", benchmarkRaw], ["packaged", packagedRaw], ["trace", traceRaw]] as const) {
  for (const privateValue of [repositoryRoot, process.env.HOME ?? "__no_home__"]) {
    if (privateValue !== "" && source.includes(privateValue)) throw new Error(`IDE-12 ${label} evidence contains a private path`);
  }
  if (/(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}|-----BEGIN [A-Z ]*PRIVATE KEY-----/u.test(source)) {
    throw new Error(`IDE-12 ${label} evidence contains secret-shaped material`);
  }
}
const relative = (value: string): string => path.relative(repositoryRoot, value).split(path.sep).join("/");
const receipt = Schema.decodeUnknownSync(IdeSourceControlAcceptanceReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-source-control-acceptance.v1",
  issue: "IDE-12",
  candidateCommitSha: benchmark.candidateCommitSha,
  evaluationSha,
  artifactTreeSha256: artifact.sha256,
  generatedAt: new Date().toISOString(),
  benchmarkRef: relative(benchmarkPath),
  packagedRef: relative(packagedPath),
  screenshotRef: packaged.screenshotRef,
  traceRef: packaged.traceRef,
  evidenceRefs: [relative(benchmarkPath), relative(packagedPath), packaged.screenshotRef, packaged.traceRef],
  rollbackTargetSha: execFileSync("git", ["rev-parse", `${benchmark.candidateCommitSha}^`], { cwd: repositoryRoot, encoding: "utf8" }).trim(),
  reviewerDisposition: "unreviewed",
  ownerDisposition: "unreviewed",
  assuranceLifecycle: "proposed",
  remainingGaps: [
    "Independent owner and AssuranceSpec acceptance remain unreviewed.",
    "Windows and Linux packaged journeys remain unclaimed.",
    "IDE-13 owns remote project portability. IDE-17 owns broader parallel-agent comparison.",
  ],
  passed: true,
});
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 });
process.stdout.write(`[openagents-desktop] IDE-12 acceptance: ${outputPath}\n`);
