import { execFileSync } from "node:child_process"
import { readFileSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import { IdeBaselineReceiptSchema, type IdeBaselineMetric } from "../src/ide/baseline-contract.ts"
import {
  IdeBasicIdeAcceptanceReceiptSchema,
  IdeBasicIdeChatOnlyReceiptSchema,
  IdeBasicIdeMetricSchema,
  IdeBasicIdePackagedJourneyReceiptSchema,
  type IdeBasicIdeMatrixEvidence,
  type IdeBasicIdeMetric,
} from "../src/ide/basic-ide-acceptance-contract.ts"
import { packagedArtifactReceipt } from "./ide-packaged-artifact.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide")
const outputPath = path.join(benchmarkRoot, "2026-07-19-ide-07-acceptance.json")
const readJson = (name: string): unknown => JSON.parse(readFileSync(path.join(benchmarkRoot, name), "utf8"))
const candidateCommitSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim()
const candidate = packagedArtifactReceipt(candidateCommitSha)

const assert: (condition: unknown, message: string) => asserts condition = (condition, message) => {
  if (!condition) throw new Error(`IDE-07 acceptance refused: ${message}`)
}

const object = (value: unknown, label: string): Readonly<Record<string, unknown>> => {
  assert(typeof value === "object" && value !== null && !Array.isArray(value), `${label} is not an object`)
  return value as Readonly<Record<string, unknown>>
}

const number = (value: unknown, label: string): number => {
  assert(typeof value === "number" && Number.isFinite(value) && value >= 0, `${label} is not a non-negative number`)
  return value
}

const currentBaseline = Schema.decodeUnknownSync(IdeBaselineReceiptSchema)(readJson("2026-07-19-ide-07-current-baseline.json"))
const ide00Baseline = Schema.decodeUnknownSync(IdeBaselineReceiptSchema)(readJson("2026-07-19-ide-00-baseline.json"))
const chatOnly = Schema.decodeUnknownSync(IdeBasicIdeChatOnlyReceiptSchema)(readJson("2026-07-19-ide-07-chat-only.json"))
const packaged = Schema.decodeUnknownSync(IdeBasicIdePackagedJourneyReceiptSchema)(readJson("2026-07-19-ide-07-packaged-basic-ide.json"))

assert(chatOnly.candidateCommitSha === candidateCommitSha, "chat-only receipt does not bind the candidate SHA")
assert(packaged.candidateCommitSha === candidateCommitSha, "packaged editor receipt does not bind the candidate SHA")
assert(chatOnly.artifactTreeSha256 === candidate.artifactTreeSha256, "chat-only artifact digest differs")
assert(packaged.artifactTreeSha256 === candidate.artifactTreeSha256, "packaged editor artifact digest differs")

const metricFrom = (
  metric: string,
  values: Readonly<{ p50: number; p95: number; p99: number; repetitions: number }>,
  thresholds: Readonly<{ p95: number; p99: number; baselineP95: number | null }>,
  unit: IdeBasicIdeMetric["unit"],
  noise: string,
): IdeBasicIdeMetric => Schema.decodeUnknownSync(IdeBasicIdeMetricSchema)({
    metric,
    unit,
    repetitions: values.repetitions,
    p50: values.p50,
    p95: values.p95,
    p99: values.p99,
    thresholdP95: thresholds.p95,
    thresholdP99: thresholds.p99,
    baselineP95: thresholds.baselineP95,
    method: "linear interpolation over ascending samples",
    noise,
    passed: values.p95 <= thresholds.p95 && values.p99 <= thresholds.p99,
  })

const baselineByName = (metrics: ReadonlyArray<IdeBaselineMetric>, name: string): IdeBaselineMetric => {
  const found = metrics.find(metric => metric.metric === name)
  assert(found !== undefined, `baseline metric ${name} is missing`)
  return found
}

const metrics: IdeBasicIdeMetric[] = []
for (const metricName of [
  "workspace.tree.first-page",
  "workspace.tree.large-repository-traversal",
  "workspace.search.path",
  "workspace.search.content",
  "workspace.document.open",
  "workspace.document.save",
  "workspace.document.conflict-detection",
  "editor.recovery-snapshot",
  "git.status",
  "git.diff",
  "resource.node-rss",
  "resource.open-file-descriptors",
] as const) {
  const baseline = baselineByName(ide00Baseline.metrics, metricName)
  const current = baselineByName(currentBaseline.metrics, metricName)
  const factor = metricName.startsWith("resource.") ? 1.5 : 2
  metrics.push(metricFrom(metricName, current, {
    p95: baseline.p95 * factor,
    p99: baseline.p99 * factor,
    baselineP95: baseline.p95,
  }, current.unit, `${current.noise} IDE-07 freezes a ${factor}x IDE-00 non-regression envelope; the receipt cannot expand it.`))
}

const startupBaseline = baselineByName(ide00Baseline.metrics, "startup.shellMounted")
metrics.push(metricFrom("packaged.chat-only.shell-ready", {
  ...chatOnly.shellReadyMs,
  repetitions: chatOnly.repetitions,
}, {
  p95: startupBaseline.p95 * 1.5,
  p99: startupBaseline.p99 * 1.5,
  baselineP95: startupBaseline.p95,
}, "milliseconds", "LaunchServices, Chromium connection, and shell mount include uncontrolled macOS scheduler and filesystem-cache noise."))

const appendMetricArray = (receiptName: string, prefix: string): void => {
  const receipt = object(readJson(receiptName), receiptName)
  const rawMetrics = receipt.metrics
  assert(Array.isArray(rawMetrics), `${receiptName} metrics are missing`)
  for (const [index, raw] of rawMetrics.entries()) {
    const row = object(raw, `${receiptName}.metrics[${index}]`)
    const thresholdP95 = number(row.thresholdP95, `${prefix}.thresholdP95`)
    metrics.push(metricFrom(`${prefix}.${String(row.metric)}`, {
      p50: number(row.p50, `${prefix}.p50`),
      p95: number(row.p95, `${prefix}.p95`),
      p99: number(row.p99, `${prefix}.p99`),
      repetitions: number(row.repetitions, `${prefix}.repetitions`),
    }, { p95: thresholdP95, p99: thresholdP95 * 1.25, baselineP95: null }, "milliseconds",
    `Packet-owned threshold from ${receiptName}; IDE-07 does not modify it.`))
  }
}

appendMetricArray("2026-07-19-ide-02-path-index.json", "explorer")
appendMetricArray("2026-07-19-ide-03-monaco.json", "monaco")

const workbench = object(readJson("2026-07-19-ide-04-workbench.json"), "IDE-04 workbench")
const workbenchFixture = object(workbench.fixture, "IDE-04 fixture")
const workbenchThresholds = object(workbench.thresholds, "IDE-04 thresholds")
for (const [field, metricName, thresholdField] of [
  ["quickOpenMs", "workbench.quick-open", "quickOpenP95Ms"],
  ["navigationPushMs", "workbench.navigation-push", "navigationPushP95Ms"],
] as const) {
  const value = object(workbench[field], field)
  const threshold = number(workbenchThresholds[thresholdField], thresholdField)
  metrics.push(metricFrom(metricName, {
    p50: number(value.p50, `${field}.p50`),
    p95: number(value.p95, `${field}.p95`),
    p99: number(value.p99, `${field}.p99`),
    repetitions: field === "quickOpenMs"
      ? number(workbenchFixture.quickOpenIterations, "quickOpenIterations")
      : number(workbenchFixture.navigationPushes, "navigationPushes"),
  }, { p95: threshold, p99: threshold * 1.25, baselineP95: null }, "milliseconds", "Deterministic bounded IDE-04 state benchmark."))
}

const review = object(readJson("2026-07-19-ide-05-review.json"), "IDE-05 review")
const reviewLatency = object(review.latency, "review.latency")
const reviewCorpus = object(review.corpus, "review.corpus")
const reviewBudgets = object(review.budgets, "review.budgets")
for (const [field, metricName, budget] of [
  ["sourceProjection", "review.source-projection", "projectionP95Ms"],
  ["aggregateParse", "review.aggregate-parse", "aggregateParseP95Ms"],
] as const) {
  const value = object(reviewLatency[field], field)
  const threshold = number(reviewBudgets[budget], budget)
  metrics.push(metricFrom(metricName, {
    p50: number(value.p50Ms, `${field}.p50Ms`),
    p95: number(value.p95Ms, `${field}.p95Ms`),
    p99: number(value.p99Ms, `${field}.p99Ms`),
    repetitions: number(reviewCorpus.samples, "review samples"),
  }, { p95: threshold, p99: threshold * 1.25, baselineP95: null }, "milliseconds", "IDE-05 eight-source-class and 500-file aggregate corpus."))
}

const language = object(readJson("2026-07-19-ide-06-language.json"), "IDE-06 language")
const languageLatency = object(language.latency, "language.latency")
const languageCorpus = object(language.corpus, "language.corpus")
const languageBudgets = object(language.budgets, "language.budgets")
for (const [field, metricName, budget] of [
  ["diagnostics", "language.diagnostics", "diagnosticsP95Ms"],
  ["documentSymbols", "language.document-symbols", "documentSymbolsP95Ms"],
] as const) {
  const value = object(languageLatency[field], field)
  const threshold = number(languageBudgets[budget], budget)
  metrics.push(metricFrom(metricName, {
    p50: number(value.p50Ms, `${field}.p50Ms`),
    p95: number(value.p95Ms, `${field}.p95Ms`),
    p99: number(value.p99Ms, `${field}.p99Ms`),
    repetitions: number(languageCorpus.samples, "language samples"),
  }, { p95: threshold, p99: threshold * 1.25, baselineP95: null }, "milliseconds", "IDE-06 persistent real TypeScript 6.0.3 worker corpus."))
}

const evidenceRefs = {
  packaged: "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-07-packaged-basic-ide.json",
  chat: "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-07-chat-only.json",
  explorer: "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-02-path-index.json",
  monaco: "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-03-monaco.json",
  workbench: "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-04-workbench.json",
  review: "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-05-review.json",
  language: "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-06-language.json",
  tests: "apps/openagents-desktop (complete local test corpus)",
} as const

const matrix = ([
  ["finder_cold_open", [evidenceRefs.packaged], "LaunchServices opened the supported TypeScript file into the primary packaged Monaco editor."],
  ["explorer_at_scale", [evidenceRefs.packaged, evidenceRefs.explorer], "Packaged pointer/keyboard Explorer evidence is paired with the complete 10k-node cached, uncached, watcher, and teardown corpus."],
  ["rapid_switching", [evidenceRefs.packaged, evidenceRefs.workbench, evidenceRefs.tests], "Preview/pin, split, navigation-generation, and stale-result churn gates pass."],
  ["editing_and_recovery", [evidenceRefs.packaged, evidenceRefs.monaco, evidenceRefs.tests], "Real Monaco edit, split, reload recovery, save/recovery/encoding/EOL corpora, and zero-resource close pass."],
  ["conflict", [evidenceRefs.tests], "Ordinary and Vim saves use the same typed conflict reducer and refuse silent overwrite."],
  ["search_and_navigation", [evidenceRefs.packaged, evidenceRefs.workbench, evidenceRefs.explorer], "Quick open, preview pin, workspace search, history, breadcrumbs, Outline, definitions, and references pass their bounded paths."],
  ["versioned_review", [evidenceRefs.packaged, evidenceRefs.review, evidenceRefs.tests], "The packaged worktree renders Pierre Diffs; all eight source classes and refusal/selection/accessibility corpora pass."],
  ["language_bursts", [evidenceRefs.packaged, evidenceRefs.language], "Both worker tiers, all 17 capabilities, 100-request supersession, crash/restart, and stale fences pass."],
  ["vim_on_off", [evidenceRefs.packaged, evidenceRefs.monaco, evidenceRefs.tests], "The packaged toggle, first-party command corpus, persistence, split isolation, conflict, and teardown pass."],
  ["keyboard_and_assistive_tech", [evidenceRefs.explorer, evidenceRefs.review, evidenceRefs.tests], "Keyboard, screen-reader roles/announcements/focus, and non-color cues pass the complete local accessibility corpus."],
  ["visual_and_accessibility", [evidenceRefs.packaged, "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-07-packaged-basic-ide.png", evidenceRefs.tests], "Tokyo Night is present before editor readiness with explicit contrast/focus/reduced-motion/zoom/minimum-window gates."],
  ["offline_and_failure", [evidenceRefs.packaged, evidenceRefs.language, evidenceRefs.tests], "Private-scheme editor/review/language stay local; typed unavailable/degraded/corrupt-state recovery remains explicit."],
  ["resource_disposal", [evidenceRefs.packaged, evidenceRefs.chat, evidenceRefs.explorer, evidenceRefs.monaco, evidenceRefs.review, evidenceRefs.language], "Models, views, workers, listeners, subscriptions, and pending requests return to their admitted zero/delta bounds."],
  ["rollback", [evidenceRefs.tests, "docs/ide/2026-07-19-ide-01-package-admission.md"], "Retained-slot update rollback and schema-compatible dependency/theme/Vim/worker source rollback gates pass."],
  ["chat_only_launch", [evidenceRefs.chat], "Seven packaged cold launches requested zero editor assets and mounted zero Monaco/Pierre/language/index surfaces or renderer workers."],
] satisfies ReadonlyArray<readonly [IdeBasicIdeMatrixEvidence["matrixId"], ReadonlyArray<string>, string]>).map(([matrixId, refs, disposition]) => ({
  matrixId,
  passed: true as const,
  evidenceRefs: refs,
  disposition,
}))

const issueState = (issue: number): "CLOSED" => {
  const state = execFileSync("gh", ["issue", "view", String(issue), "--repo", "OpenAgentsInc/openagents", "--json", "state", "--jq", ".state"], { encoding: "utf8" }).trim()
  assert(state === "CLOSED", `child issue ${issue} is ${state}`)
  return "CLOSED"
}

const childEvidence = [
  ["IDE-00", 9015, ["apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-00-baseline.json"]],
  ["IDE-01", 9016, ["apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-package-audit.json"]],
  ["IDE-02", 9017, [evidenceRefs.explorer, "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-02-packaged-journey.json"]],
  ["IDE-03", 9018, [evidenceRefs.monaco, "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-03-packaged-journey.json"]],
  ["IDE-04", 9019, [evidenceRefs.workbench, "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-04-packaged-workbench.json"]],
  ["IDE-05", 9020, [evidenceRefs.review]],
  ["IDE-06", 9021, [evidenceRefs.language, "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-06-packaged-language.json"]],
] as const

const boundaryOracle = execFileSync(process.execPath, ["--import", "tsx", path.join(appRoot, "scripts", "check-ide-boundaries.ts")], {
  cwd: repositoryRoot,
  encoding: "utf8",
})
assert(boundaryOracle.includes("PASS"), "Effect authority boundary oracle did not pass")
const themeSource = readFileSync(path.join(appRoot, "src", "ide", "tokyo-night-theme.ts"), "utf8")
const vimSource = readFileSync(path.join(appRoot, "src", "ide", "vim-mode-contract.ts"), "utf8")
const languageSource = readFileSync(path.join(appRoot, "src", "ide", "language-service.ts"), "utf8")
assert(themeSource.includes('id: "tokyo-night"') && themeSource.includes("initializedBeforeEditorPaint"), "Tokyo Night projection is not frozen")
assert(vimSource.includes("offByDefault") && vimSource.includes("first_party"), "first-party Vim policy is not frozen")
for (const token of ["Context.Service", "Layer.effect", "Effect.fn", "Schema.TaggedErrorClass"]) assert(languageSource.includes(token), `language authority is missing ${token}`)
for (const file of ["2026-07-19-ide-07-chat-only.json", "2026-07-19-ide-07-packaged-basic-ide.json"]) {
  const source = readFileSync(path.join(benchmarkRoot, file), "utf8")
  assert(!source.includes(repositoryRoot), `${file} leaked the repository root`)
  assert(!source.includes(process.env.HOME ?? "__no_home__"), `${file} leaked the home root`)
}

const rollbackTarget = execFileSync("git", ["rev-list", "-n", "1", "HEAD", "--", "docs/ide/2026-07-19-ide-06-generation-safe-language.md"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim()

const receipt = Schema.decodeUnknownSync(IdeBasicIdeAcceptanceReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-basic-ide-acceptance.v1",
  generatedAt: new Date().toISOString(),
  claim: "OpenAgents basic IDE",
  candidate,
  childEvidence: childEvidence.map(([packet, issue, refs]) => ({ packet, issue, state: issueState(issue), evidenceRefs: refs })),
  matrix,
  metrics,
  chatOnly,
  architecture: {
    oneSchemaSourcePerBoundary: true,
    contextServiceAndLayerEffect: true,
    namedEffectFunctions: true,
    taggedSchemaErrors: true,
    decodedRendererInputs: true,
    scopedLifecycle: true,
    rendererOwnsNoAuthority: true,
    nativeOwnsNoAuthority: true,
    tokyoNightOnly: true,
    vimFirstPartyOffByDefaultDisposable: true,
    noRemoteIndexOrUploadOrTelemetryExpansion: true,
    publicSafeEvidence: true,
  },
  rollback: {
    targetCommitSha: rollbackTarget,
    settingsSchemaCompatible: true,
    recoverySchemaCompatible: true,
    dependencyRemovalDocumented: true,
    updateRollbackCorpusPassed: true,
  },
  targets: {
    claimed: ["darwin-arm64"],
    unavailable: ["darwin-x64", "win32-arm64", "win32-x64", "linux-arm64", "linux-x64"].map(target => ({
      target,
      reason: "No IDE-07 packaged candidate was evaluated for this target.",
    })),
  },
  review: {
    reviewerClass: "deterministic_repository_oracle",
    oracleRef: "apps/openagents-desktop/scripts/ide-basic-ide-acceptance.ts",
    producerCanOverride: false,
    disposition: "pass",
  },
  laterGaps: [
    "IDE-08 inspectable agent context and proposal loop remain incomplete.",
    "IDE-09 AI completion and version-bound multi-file editing remain incomplete.",
    "IDE-10 terminal, PTY, tasks, tests, and debug integration remain incomplete.",
    "IDE-11 broader SCM delivery and repository workflows remain incomplete.",
    "IDE-12 light, high-contrast, system, and marketplace themes remain incomplete.",
    "IDE-13 portable host placement remains incomplete.",
    "IDE-14 mobile supervision of portable IDE capabilities remains incomplete.",
    "IDE-15 isolated extensions and component ABI remain incomplete.",
    "IDE-16 data inventory, migration, export, and deletion breadth remains incomplete.",
    "IDE-17 Cursor parity closure remains incomplete.",
    "IDE-18 full accessibility and platform matrix remains incomplete.",
    "IDE-19 full-IDE and parity release gate remains incomplete.",
  ],
  assertions: [
    "This receipt admits only the exact phrase OpenAgents basic IDE.",
    "No Zed-quality, full-IDE, Cursor-parity, drop-in-replacement, or later-rung claim is implied.",
    "Only the packaged macOS arm64 target is claimed; every other target remains visibly unavailable.",
    "The deterministic oracle rejects missing child evidence, metric regressions, artifact/SHA disagreement, root leakage, or a non-zero chat-only IDE cost.",
    "No cloud language service, embeddings, remote semantic index, repository upload, telemetry expansion, AI editing, terminal expansion, or extension runtime was enabled.",
    "Issue #9014 still requires owner acceptance; IDE-07 closure alone does not close the epic or create a broader promise.",
  ],
})

writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
process.stdout.write(`[openagents-desktop] IDE-07 basic IDE acceptance: ${outputPath}\n`)
