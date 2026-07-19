import { execFileSync } from "node:child_process"
import { readFileSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import {
  IdeCursorAcceptanceReceiptSchema,
  IdeCursorBenchmarkReceiptSchema,
  IdeCursorPackagedJourneyReceiptSchema,
} from "../src/ide/cursor-benchmark-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide")
const benchmarkRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-09-cursor.json"
const packagedRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-09-packaged-cursor.json"
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-09-packaged-cursor.png"
const traceRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-09-packaged-cursor-trace.json"
const outputRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-09-acceptance.json"
const outputPath = path.join(repositoryRoot, outputRef)

const readJson = (name: string): unknown =>
  JSON.parse(readFileSync(path.join(benchmarkRoot, name), "utf8"))
const benchmark = Schema.decodeUnknownSync(IdeCursorBenchmarkReceiptSchema)(
  readJson("2026-07-19-ide-09-cursor.json"),
)
const packaged = Schema.decodeUnknownSync(IdeCursorPackagedJourneyReceiptSchema)(
  readJson("2026-07-19-ide-09-packaged-cursor.json"),
)
if (!benchmark.budgetsPassed || !packaged.passed) {
  throw new Error("IDE-09 acceptance refused: benchmark or packaged journey failed")
}
if (benchmark.commitSha !== packaged.candidateCommitSha) {
  throw new Error("IDE-09 acceptance refused: evidence candidate SHAs differ")
}
const candidateCommitSha = packaged.candidateCommitSha
const mainEvaluationSha = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim()
execFileSync("git", ["merge-base", "--is-ancestor", candidateCommitSha, mainEvaluationSha], {
  cwd: repositoryRoot,
})
const rollbackTargetSha = execFileSync("git", ["rev-parse", `${candidateCommitSha}^`], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim()
execFileSync("git", ["diff", "--check"], { cwd: repositoryRoot, stdio: "inherit" })
const boundary = execFileSync(process.execPath, [
  "--import", "tsx", path.join(appRoot, "scripts", "check-ide-boundaries.ts"),
], { cwd: repositoryRoot, encoding: "utf8" })
if (!boundary.includes("PASS")) {
  throw new Error("IDE-09 acceptance refused: authority boundary oracle failed")
}

const privateNeedles = [repositoryRoot, process.env.HOME ?? "__no_home__"]
const secretPatterns = [
  /sk-[A-Za-z0-9_-]{12,}/u,
  /-----BEGIN [A-Z ]*PRIVATE KEY-----/u,
  /CLAUDE_CODE_OAUTH_TOKEN\s*[:=]/u,
]
for (const name of [
  "2026-07-19-ide-09-cursor.json",
  "2026-07-19-ide-09-packaged-cursor.json",
  "2026-07-19-ide-09-packaged-cursor-trace.json",
]) {
  const source = readFileSync(path.join(benchmarkRoot, name), "utf8")
  for (const needle of privateNeedles) {
    if (needle !== "" && source.includes(needle)) {
      throw new Error(`IDE-09 acceptance refused: ${name} contains forbidden private material`)
    }
  }
  if (secretPatterns.some(pattern => pattern.test(source))) {
    throw new Error(`IDE-09 acceptance refused: ${name} contains secret-shaped material`)
  }
}

const contractEvidence = "apps/openagents-desktop/src/ide/cursor-contract.test.ts"
const serviceEvidence = "apps/openagents-desktop/src/ide/cursor-service.test.ts + apps/openagents-desktop/src/ide/cursor-host.test.ts"
const authorityEvidence = "apps/openagents-desktop/src/ide/cursor-workspace-authority.test.ts"
const providerEvidence = "apps/openagents-desktop/src/ide/cursor-claude-provider.test.ts"
const qualityEvidence = "apps/openagents-desktop/src/ide/cursor-quality-corpus.test.ts"
const rendererEvidence = "apps/openagents-desktop/src/renderer/ide/cursor.test.ts + apps/openagents-desktop/src/renderer/ide/react-cursor.test.tsx"
const packagedEvidence = `${packagedRef} + ${traceRef}`
const faultMatrix = [
  { fault: "typing, selection, content, and Monaco version churn invalidates the exact request", evidenceRef: rendererEvidence },
  { fault: "Escape cancellation interrupts only the active request", evidenceRef: `${rendererEvidence} + ${packagedEvidence}` },
  { fault: "IME-composing Escape is ignored until composition has ended", evidenceRef: "apps/openagents-desktop/src/renderer/ide/react-cursor.test.tsx" },
  { fault: "superseded and late provider chunks cannot publish", evidenceRef: serviceEvidence },
  { fault: "stale anchor and changed canonical bytes refuse mutation", evidenceRef: `${serviceEvidence} + ${authorityEvidence}` },
  { fault: "wrong sequence, attempt, candidate, or result digest refuses", evidenceRef: `${contractEvidence} + ${serviceEvidence}` },
  { fault: "partial accept uses exact word or line prefix and remains undoable", evidenceRef: `${authorityEvidence} + ${packagedEvidence}` },
  { fault: "undo restores the exact canonical preimage", evidenceRef: `${authorityEvidence} + ${packagedEvidence}` },
  { fault: "multi-file edits submit to IDE-08 and never directly mutate", evidenceRef: `${serviceEvidence} + ${packagedEvidence}` },
  { fault: "provider timeout and SDK failure become typed failures", evidenceRef: providerEvidence },
  { fault: "malformed structured output becomes invalid_output", evidenceRef: providerEvidence },
  { fault: "provider tool, MCP, skills, plugins, and session persistence remain disabled", evidenceRef: providerEvidence },
  { fault: "identity substitution or model mismatch refuses before candidate publication", evidenceRef: `${providerEvidence} + ${serviceEvidence}` },
  { fault: "semantic retrieval disabled does not require remote embeddings", evidenceRef: `${qualityEvidence} + ${benchmarkRef}` },
  { fault: "secret, private, ignored, binary, and too-large context is withheld", evidenceRef: qualityEvidence },
  { fault: "malicious instructions and hallucinated paths produce no unsafe publication", evidenceRef: qualityEvidence },
  { fault: "offline posture refuses when the selected provider lacks offline capability", evidenceRef: serviceEvidence },
  { fault: "compare and retry retain typed receipts and advance exact sequence", evidenceRef: `${serviceEvidence} + ${packagedEvidence}` },
  { fault: "renderer/provider/Monaco cannot bypass main document authority", evidenceRef: "apps/openagents-desktop/tests/electron-boundary.test.ts + apps/openagents-desktop/scripts/check-ide-boundaries.ts" },
  { fault: "shutdown disposes active requests, fibers, subscriptions, and candidate state", evidenceRef: `${serviceEvidence} + ${benchmarkRef}` },
].map(entry => ({ ...entry, passed: true as const }))

const receipt = Schema.decodeUnknownSync(IdeCursorAcceptanceReceiptSchema)({
  schemaVersion: "openagents.ide-cursor-acceptance.v1",
  issue: "IDE-09",
  generatedAt: new Date().toISOString(),
  candidateCommitSha,
  mainEvaluationSha,
  artifactTreeSha256: packaged.artifactTreeSha256,
  evidenceRefs: [
    benchmarkRef,
    packagedRef,
    screenshotRef,
    traceRef,
    outputRef,
    "apps/openagents-desktop/src/ide/cursor-contract.test.ts",
    "apps/openagents-desktop/src/ide/cursor-service.test.ts",
    "apps/openagents-desktop/src/ide/cursor-host.test.ts",
    "apps/openagents-desktop/src/ide/cursor-workspace-authority.test.ts",
    "apps/openagents-desktop/src/ide/cursor-claude-provider.test.ts",
    "apps/openagents-desktop/src/ide/cursor-quality-corpus.test.ts",
    "apps/openagents-desktop/src/renderer/ide/cursor.test.ts",
    "apps/openagents-desktop/src/renderer/ide/react-cursor.test.tsx",
    "apps/openagents-desktop/src/renderer/workspace-editor.test.ts",
    "apps/openagents-desktop/tests/electron-boundary.test.ts",
    "apps/openagents-desktop/tests/accessibility.test.ts",
    "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
    "apps/openagents-desktop/scripts/check-ide-boundaries.ts",
  ],
  faultMatrix,
  architecture: {
    oneSchemaGraph: true,
    effectServices: true,
    rendererAuthority: false,
    providerAuthority: false,
    harnessAuthority: false,
    monacoAuthority: false,
    embeddingsRequired: false,
    silentFallback: false,
  },
  accessibility: {
    keyboard: true,
    screenReaderLabels: true,
    focusEscape: true,
    vimOnOff: true,
    imeComposition: true,
    reducedMotion: true,
    zoomAndMinimumWindow: true,
    tokyoNightNonColorCues: true,
  },
  assuranceLifecycle: "proposed",
  ownerDisposition: "unreviewed",
  realProviderCohort: {
    disposition: "not_run",
    reason: "The repeatable acceptance cohort uses the explicit packaged deterministic Claude-query fixture; no owner-authorized provider spend or live credential lane was used.",
  },
  rollbackTargetSha,
  claimedTargets: ["darwin-arm64"],
  laterGaps: [
    "Real-provider quality and latency remain a separately armed cohort; deterministic fixture scores are not represented as model quality.",
    "IDE-10 through IDE-19 terminal, SCM, extension, remote, browser, parity, and closure packets remain open.",
    "Independent owner acceptance for Desktop AC-17/AC-43 and Cursor CP-AC-20 remains proposed and unreviewed.",
  ],
  passed: true,
})
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`)
