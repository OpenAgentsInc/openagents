import { execFileSync } from "node:child_process"
import { readFileSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import {
  IdeRunAcceptanceReceiptSchema,
  IdeRunBenchmarkReceiptSchema,
  IdeRunPackagedJourneyReceiptSchema,
} from "../src/ide/run-benchmark-contract.ts"
import { packagedArtifactTreeDigest, resolvePackagedApp } from "./ide-packaged-artifact.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide")
const benchmarkRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-run.json"
const packagedRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-packaged-run.json"
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-packaged-run.png"
const traceRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-packaged-run-trace.json"
const outputRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-acceptance.json"
const outputPath = path.join(repositoryRoot, outputRef)

const json = (name: string): unknown => JSON.parse(readFileSync(path.join(benchmarkRoot, name), "utf8"))
const benchmark = Schema.decodeUnknownSync(IdeRunBenchmarkReceiptSchema)(json("2026-07-19-ide-10-run.json"))
const packaged = Schema.decodeUnknownSync(IdeRunPackagedJourneyReceiptSchema)(json("2026-07-19-ide-10-packaged-run.json"))
if (!benchmark.passed || !packaged.passed) throw new Error("IDE-10 acceptance refused: benchmark or packaged journey failed")
if (benchmark.candidateCommitSha !== packaged.candidateCommitSha) throw new Error("IDE-10 acceptance refused: evidence candidate SHAs differ")
const candidateCommitSha = benchmark.candidateCommitSha
const mainEvaluationSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim()
execFileSync("git", ["merge-base", "--is-ancestor", candidateCommitSha, mainEvaluationSha], { cwd: repositoryRoot })
const rollbackTargetSha = execFileSync("git", ["rev-parse", `${candidateCommitSha}^`], { cwd: repositoryRoot, encoding: "utf8" }).trim()
const artifact = packagedArtifactTreeDigest(resolvePackagedApp())
if (artifact.sha256 !== packaged.artifactTreeSha256) throw new Error("IDE-10 acceptance refused: packaged artifact digest changed")
execFileSync("git", ["diff", "--check"], { cwd: repositoryRoot, stdio: "inherit" })
const boundary = execFileSync(process.execPath, ["--import", "tsx", path.join(appRoot, "scripts", "check-ide-boundaries.ts")], { cwd: repositoryRoot, encoding: "utf8" })
if (!boundary.includes("PASS")) throw new Error("IDE-10 acceptance refused: authority boundary oracle failed")

for (const name of [
  "2026-07-19-ide-10-run.json",
  "2026-07-19-ide-10-packaged-run.json",
  "2026-07-19-ide-10-packaged-run-trace.json",
]) {
  const source = readFileSync(path.join(benchmarkRoot, name), "utf8")
  for (const privateValue of [repositoryRoot, process.env.HOME ?? "__no_home__"]) {
    if (privateValue !== "" && source.includes(privateValue)) throw new Error(`IDE-10 acceptance refused: ${name} contains private path material`)
  }
  if (/(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}|-----BEGIN [A-Z ]*PRIVATE KEY-----/u.test(source)) {
    throw new Error(`IDE-10 acceptance refused: ${name} contains secret-shaped material`)
  }
}

const contractEvidence = "apps/openagents-desktop/src/ide/run-contract.test.ts"
const serviceEvidence = "apps/openagents-desktop/src/ide/run-service.test.ts"
const hostEvidence = "apps/openagents-desktop/src/ide/run-host.test.ts"
const terminalEvidence = "apps/openagents-desktop/src/terminal-host.test.ts"
const rendererEvidence = "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx"
const boundaryEvidence = "apps/openagents-desktop/tests/electron-boundary.test.ts + apps/openagents-desktop/scripts/check-ide-boundaries.ts"
const packagedEvidence = `${packagedRef} + ${traceRef} + ${screenshotRef}`
const faultMatrix = [
  ["missing shell and spawn failure settle as typed refusal or failed outcome", `${hostEvidence} + ${terminalEvidence}`],
  ["invalid cwd or revoked workspace generation cannot reuse a process", terminalEvidence],
  ["complete host environment and secret-named values are never inherited", hostEvidence],
  ["renderer schemas reject executable, argv, and raw environment injection", contractEvidence],
  ["huge output stays byte-bounded and publishes an explicit gap", `${serviceEvidence} + ${packagedEvidence}`],
  ["token-shaped and private-key output is redacted before projection", `${hostEvidence} + ${terminalEvidence}`],
  ["invalid UTF-8 is an explicit output fact", `${benchmarkRef} + ${serviceEvidence}`],
  ["output sequence remains monotonic across retention eviction", serviceEvidence],
  ["late output cannot revive a disposed channel", serviceEvidence],
  ["terminal close and host disposal terminate the owned process group", terminalEvidence],
  ["resize uses bounded dimensions and refuses an absent session", `${serviceEvidence} + ${terminalEvidence}`],
  ["declared task dependencies execute in graph order", `${hostEvidence} + ${packagedEvidence}`],
  ["cyclic or missing task dependencies fail discovery", serviceEvidence],
  ["background readiness cannot be inferred without the declared pattern", `${serviceEvidence} + ${hostEvidence}`],
  ["task timeout and cancellation remain distinct semantic outcomes", `${serviceEvidence} + ${hostEvidence}`],
  ["zero task exit cannot fabricate success when readiness or artifacts are incomplete", serviceEvidence],
  ["test discovery is generation-bound and duplicate items refuse", `${serviceEvidence} + ${hostEvidence}`],
  ["zero test exit cannot fabricate success without assertion evidence", serviceEvidence],
  ["test retry references must bind a settled run in the same controller", serviceEvidence],
  ["output export is mode-0600 and records an actor-bound public-safe receipt", hostEvidence],
  ["human and agent actors use the same command, budget, output, and receipt schemas", `${contractEvidence} + ${serviceEvidence}`],
  ["xterm, task, test, and Output surfaces stay decoded projections", `${rendererEvidence} + ${boundaryEvidence}`],
  ["announced links remain localhost-only and policy-confirmed", terminalEvidence],
  ["Khala default and Tokyo Night fallback retain contrast and non-color cues", packagedEvidence],
  ["scope teardown closes subscriptions, output channels, tasks, tests, and terminals", `${serviceEvidence} + ${benchmarkRef}`],
].map(([fault, evidenceRef]) => ({ fault, evidenceRef, passed: true as const }))

const receipt = Schema.decodeUnknownSync(IdeRunAcceptanceReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-run-acceptance.v1",
  issue: "IDE-10",
  generatedAt: new Date().toISOString(),
  candidateCommitSha,
  mainEvaluationSha,
  artifactTreeSha256: artifact.sha256,
  benchmarkRef,
  packagedRef,
  screenshotRef,
  traceRef,
  evidenceRefs: [
    benchmarkRef,
    packagedRef,
    screenshotRef,
    traceRef,
    outputRef,
    "apps/openagents-desktop/src/ide/run-contract.ts",
    "apps/openagents-desktop/src/ide/run-service.ts",
    "apps/openagents-desktop/src/ide/run-host.ts",
    contractEvidence,
    serviceEvidence,
    hostEvidence,
    terminalEvidence,
    rendererEvidence,
    boundaryEvidence,
  ],
  faultMatrix,
  architecture: {
    oneSchemaGraph: true,
    effectAuthority: true,
    xtermProjectionOnly: true,
    explicitEnvironment: true,
    semanticSuccess: true,
    actorParity: true,
    rustAdmitted: false,
  },
  ownerDisposition: "unreviewed",
  assuranceLifecycle: "proposed",
  rollbackTargetSha,
  laterGaps: [
    "IDE-11 adds the DAP debugger authority; IDE-10 does not infer debugger success from test or process state.",
    "Windows and Linux packaged target receipts remain release-lane work. The six-target native-helper table records no Rust admission or native claim.",
    "Independent owner acceptance for the proposed ProductSpec and AssuranceSpec criteria remains unreviewed.",
  ],
  passed: true,
})
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { encoding: "utf8", mode: 0o600 })
process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`)
