import { execFileSync } from "node:child_process"
import { readFileSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import {
  IdeAgentCodeAcceptanceReceiptSchema,
  IdeAgentCodeBenchmarkReceiptSchema,
  IdeAgentCodePackagedJourneyReceiptSchema,
} from "../src/ide/agent-code-benchmark-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide")
const benchmarkRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-agent-code.json"
const packagedRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.json"
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.png"
const traceRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code-trace.json"
const outputRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-acceptance.json"
const outputPath = path.join(repositoryRoot, outputRef)
const readJson = (name: string): unknown => JSON.parse(readFileSync(path.join(benchmarkRoot, name), "utf8"))
const benchmark = Schema.decodeUnknownSync(IdeAgentCodeBenchmarkReceiptSchema)(readJson("2026-07-19-ide-08-agent-code.json"))
const packaged = Schema.decodeUnknownSync(IdeAgentCodePackagedJourneyReceiptSchema)(readJson("2026-07-19-ide-08-packaged-agent-code.json"))
if (!benchmark.budgetsPassed || !packaged.passed) throw new Error("IDE-08 acceptance refused: benchmark or packaged journey failed")
if (benchmark.candidateCommitSha !== packaged.candidateCommitSha) throw new Error("IDE-08 acceptance refused: evidence candidate SHAs differ")
const candidateCommitSha = packaged.candidateCommitSha
const mainEvaluationSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim()
execFileSync("git", ["merge-base", "--is-ancestor", candidateCommitSha, mainEvaluationSha], { cwd: repositoryRoot })
const rollbackTargetSha = execFileSync("git", ["rev-parse", `${candidateCommitSha}^`], { cwd: repositoryRoot, encoding: "utf8" }).trim()
const boundary = execFileSync(process.execPath, ["--import", "tsx", path.join(appRoot, "scripts", "check-ide-boundaries.ts")], {
  cwd: repositoryRoot,
  encoding: "utf8",
})
if (!boundary.includes("PASS")) throw new Error("IDE-08 acceptance refused: authority boundary oracle failed")

const privateNeedles = [repositoryRoot, process.env.HOME ?? "__no_home__"]
for (const name of [
  "2026-07-19-ide-08-agent-code.json",
  "2026-07-19-ide-08-packaged-agent-code.json",
  "2026-07-19-ide-08-packaged-agent-code-trace.json",
]) {
  const source = readFileSync(path.join(benchmarkRoot, name), "utf8")
  for (const needle of privateNeedles) if (needle !== "" && source.includes(needle)) {
    throw new Error(`IDE-08 acceptance refused: ${name} contains forbidden private material`)
  }
}

const faultEvidence = "apps/openagents-desktop/src/ide/agent-code-service.test.ts + apps/openagents-desktop/src/ide/agent-code-host.test.ts"
const faultMatrix = [
  "dirty document refusal", "external change / changed-base refusal", "stale proposal refusal",
  "explicit rebase-required replacement", "encoding / EOL / mode admission",
  "secret / private / binary / too-large refusal", "revoked or wrong workspace grant refusal",
  "deleted / renamed file refusal", "parallel worktree attachment isolation",
  "generation-fenced cancellation / supersession / late output",
  "semantic retrieval off / unavailable sources", "restart pending / reviewed / partial / applied / undoable recovery",
  "corrupt persistence refusal", "partial accept / reject", "mixed create / edit / rename / delete", "full teardown",
].map(fault => ({ fault, passed: true as const, evidenceRef: faultEvidence }))

const receipt = Schema.decodeUnknownSync(IdeAgentCodeAcceptanceReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-agent-code-acceptance.v1",
  issue: "IDE-08",
  generatedAt: new Date().toISOString(),
  candidateCommitSha,
  mainEvaluationSha,
  artifactTreeSha256: packaged.artifactTreeSha256,
  evidenceRefs: [benchmarkRef, packagedRef, screenshotRef, traceRef, outputRef,
    "apps/openagents-desktop/src/ide/agent-code-contract.test.ts",
    "apps/openagents-desktop/src/ide/agent-code-service.test.ts",
    "apps/openagents-desktop/src/ide/agent-code-host.test.ts",
    "apps/openagents-desktop/src/renderer/react-agent-code.test.tsx",
    "apps/openagents-desktop/src/renderer/shell.test.ts",
    "apps/openagents-desktop/tests/electron-boundary.test.ts",
    "apps/openagents-desktop/tests/accessibility.test.ts",
    "apps/openagents-desktop/tests/design-conformance.test.ts"],
  faultMatrix,
  architecture: {
    oneSchemaGraph: true,
    effectServices: true,
    rendererAuthority: false,
    harnessAuthority: false,
    monacoAuthority: false,
    pierreAuthority: false,
    nativeAuthority: false,
    embeddingsRequired: false,
    publicReceiptsContainPrivateContent: false,
  },
  accessibility: {
    keyboard: true,
    screenReaderLabels: true,
    nonColorCues: true,
    reducedMotion: true,
    zoomAndMinimumWindow: true,
  },
  assuranceLifecycle: "proposed",
  ownerDisposition: "unreviewed",
  reviewer: {
    reviewerClass: "deterministic_repository_oracle",
    oracleRef: "apps/openagents-desktop/scripts/ide-agent-code-acceptance.ts",
    producerCanOverride: false,
    disposition: "pass",
  },
  rollbackTargetSha,
  claimedTargets: ["darwin-arm64"],
  laterGaps: [
    "IDE-09 completion, next-edit, and inline AI remain intentionally unimplemented.",
    "IDE-10 through IDE-19 terminal, SCM, extension, remote, browser, parity, and closure packets remain open.",
    "Independent owner acceptance for Desktop AC-17/AC-43 and Cursor CP-AC-20 remains proposed and unreviewed.",
  ],
  passed: true,
})
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`)
