#!/usr/bin/env bun
import { spawn } from "node:child_process"
import { readdir, readFile, mkdir, writeFile } from "node:fs/promises"
import { basename, dirname, join, relative, resolve } from "node:path"

import {
  buildBehaviorContractReceipts,
  checkBehaviorContractCoverageFromFiles,
  validateBehaviorContractRegistry,
  type BehaviorContractReceipt,
} from "../packages/behavior-contracts/src/index.js"
import {
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  KhalaCodeRpcMethodNames,
  createEmptyKhalaCodeQaCoverageLedger,
  khalaCodeQaCoverageFrontierReport,
  mergeKhalaCodeQaCoverageLedgers,
  type KhalaCodeQaCoverageFrontierReport,
  type KhalaCodeQaCoverageLedger,
} from "../packages/khala-qa-harness/src/index.js"
import {
  evaluateKhalaCodeQaMetricBudget,
  evaluateKhalaCodeQaMetricBudgets,
  khalaCodeQaMetricBudgets,
  type KhalaCodeQaMetricBudget,
  type KhalaCodeQaMetricBudgetEvaluation,
  type KhalaCodeQaMetricBudgetUnit,
  type KhalaCodeQaMetricName,
  type KhalaCodeQaMetricSample,
  type KhalaCodeQaMetricsSnapshot,
} from "../clients/khala-code-desktop/src/shared/qa-metrics.js"
import { khalaCodeUxContractRegistry } from "../clients/khala-code-desktop/src/contracts/ux-contracts.js"

export const QA_NIGHTLY_MATRIX_SCHEMA =
  "openagents.khala_code.qa_nightly_matrix.v1"
export const QA_STATUS_SURFACE_SCHEMA =
  "openagents.khala_code.qa_status_surface.v1"

export const QA_NIGHTLY_DEFAULT_RUNS = 16
export const QA_NIGHTLY_DEFAULT_STEPS = 64
export const QA_NIGHTLY_DEFAULT_STEP_TIMEOUT_MS = 30 * 60 * 1000

export type QaNightlyStepId =
  | "harness-suite"
  | "behavior-contracts"
  | "real-bridge-smoke"
  | "desktop-verify"
  | "visual-part2-ui"
  | "visual-cockpit"
  | "visual-composer"
  | "monkey-night"
  | "model-based"
  | "property-tier"

export type QaNightlyStep = Readonly<{
  id: QaNightlyStepId
  label: string
  command: readonly string[]
  cwd: string
  expectedArtifactRefs?: readonly string[]
}>

export type QaNightlyStepResult = Readonly<{
  id: QaNightlyStepId
  label: string
  command: readonly string[]
  cwd: string
  attempts: readonly QaNightlyStepAttemptResult[]
  durationMs: number
  exitCode: number
  logRef: string
  status: "passed" | "failed" | "timed_out" | "flaky"
}>

export type QaNightlyStepAttemptResult = Readonly<{
  attempt: 1 | 2
  durationMs: number
  exitCode: number
  logRef: string
  status: "passed" | "failed" | "timed_out"
}>

export type QaNightlyReport = Readonly<{
  schema: typeof QA_NIGHTLY_MATRIX_SCHEMA
  runId: string
  generatedAt: string
  status: "passed" | "failed"
  steps: readonly QaNightlyStepResult[]
  artifactDir: string
  reportJsonPath: string
  reportMarkdownPath: string
  coverageLedgerPath: string
  coverageLedgerSourcePaths: readonly string[]
  coverageFrontierReportPath: string
  coverageSteeringInputPath: string
  behaviorContractReceiptPath: string
  behaviorContractRun: QaNightlyBehaviorContractRun
  quarantineLedgerPath: string
  statusSurfaceJsonPath: string
  statusSurfaceMarkdownPath: string
  latencyBudgetRun: QaNightlyLatencyBudgetRun
  issueStatus?: QaNightlyIssueStatus | undefined
  behaviorContractDeviationIssueStatus?: QaNightlyIssueStatus | undefined
  quarantineIssueStatus?: QaNightlyIssueStatus | undefined
  latencyBudgetRegressionIssueStatus?: QaNightlyIssueStatus | undefined
  zeroCoverageIssueStatus?: QaNightlyIssueStatus | undefined
}>

export type QaNightlyBehaviorContractRun = Readonly<{
  schema: "openagents.khala_code.behavior_contract_nightly_run.v1"
  checkedAt: string
  failedContractIds: readonly string[]
  passCount: number
  receiptCount: number
  receiptRefs: readonly string[]
  registryVersion: string
  skippedCount: number
  status: "pass" | "fail"
}>

export type QaNightlyIssueStatus = Readonly<
  | { status: "disabled"; reason: string }
  | { status: "filed"; issueUrl?: string | undefined }
  | { status: "failed"; reason: string }
>

export type QaNightlyCommandResult = Readonly<{
  durationMs: number
  exitCode: number
  stderr: string
  stdout: string
  timedOut?: boolean | undefined
}>

export type QaNightlyCommandRunner = (
  step: QaNightlyStep,
  timeoutMs: number,
) => Promise<QaNightlyCommandResult>

export type QaNightlyIssueFiler = (
  input: Readonly<{
    bodyPath: string
    report: QaNightlyReport
    title: string
  }>,
) => Promise<QaNightlyIssueStatus>

export type QaNightlyCoverageDimensionCount = Readonly<{
  covered: number
  coveredPercent: number
  missing: number
  total: number
}>

export type QaNightlyCoverageDimension =
  | "approvalDecisionKinds"
  | "hotbarPanels"
  | "rpcMethods"
  | "selectors"
  | "settingsKeys"
  | "slashCommandAvailabilityStates"
  | "slashCommands"
  | "threadItemVariants"

export type QaNightlyPerfTrend = Readonly<{
  deltaMs?: number | undefined
  latestDurationMs: number
  previousDurationMs?: number | undefined
  sampleCount: number
  stepId: QaNightlyStepId
  trend: "first_sample" | "flat" | "improved" | "regressed"
}>

export type QaNightlyLatencyBudgetCatalogEntry = Readonly<{
  budgetId: string
  evaluationStatus: KhalaCodeQaMetricBudgetEvaluation["status"]
  metric: KhalaCodeQaMetricName
  percentile?: number | undefined
  sampleCount: number
  threshold: number
  unit: KhalaCodeQaMetricBudgetUnit
}>

export type QaNightlyLatencyBudgetSampleEvidence = Readonly<{
  observedAt: string
  sourceRef: string
  unit: KhalaCodeQaMetricSample["unit"]
  value: number
}>

export type QaNightlyLatencyBudgetRunEntry = Readonly<{
  actual: number | null
  budgetId: string
  evaluationStatus: KhalaCodeQaMetricBudgetEvaluation["status"]
  metric: KhalaCodeQaMetricName
  percentile?: number | undefined
  sampleCount: number
  sampleEvidence: readonly QaNightlyLatencyBudgetSampleEvidence[]
  threshold: number
  unit: KhalaCodeQaMetricBudgetUnit
}>

export type QaNightlyLatencyBudgetRun = Readonly<{
  schema: "openagents.khala_code.qa_latency_budget_run.v1"
  budgetCount: number
  budgets: readonly QaNightlyLatencyBudgetRunEntry[]
  generatedAt: string
  sourceSnapshotRefs: readonly string[]
}>

export type QaNightlyLatencyBudgetTrend = Readonly<{
  budgetId: string
  delta?: number | undefined
  latestActual: number | null
  metric: KhalaCodeQaMetricName
  previousActual?: number | undefined
  sampleCount: number
  sampleEvidence: readonly QaNightlyLatencyBudgetSampleEvidence[]
  threshold: number
  trend: "no_samples" | "first_sample" | "flat" | "improved" | "regressed"
  unit: KhalaCodeQaMetricBudgetUnit
}>

export type QaNightlyStatusSurface = Readonly<{
  schema: typeof QA_STATUS_SURFACE_SCHEMA
  generatedAt: string
  runId: string
  status: QaNightlyReport["status"]
  statusSummary: "blocked" | "healthy"
  reportJsonPath: string
  reportMarkdownPath: string
  coverage: Readonly<{
    artifactRefs: Readonly<{
      frontierReportPath: string
      steeringInputPath: string
      unionLedgerPath: string
    }>
    counts: Readonly<Record<QaNightlyCoverageDimension, QaNightlyCoverageDimensionCount>>
    frontierRefCount: number
    sourceLedgerCount: number
    unionRunCount: number
    zeroForAWeekCount: number
  }>
  issueStatuses: Readonly<{
    behaviorContractDeviation: QaNightlyIssueStatus | undefined
    latencyBudgetRegression: QaNightlyIssueStatus | undefined
    nightly: QaNightlyIssueStatus | undefined
    quarantine: QaNightlyIssueStatus | undefined
    zeroCoverage: QaNightlyIssueStatus | undefined
  }>
  liveTier: Readonly<{
    evidenceRefs: readonly string[]
    reason: string
    status: "not_in_matrix"
  }>
  behaviorContracts: Readonly<{
    basis: "behavior_contract_receipts"
    checkedAt: string
    evidenceBoard: "qa-swarm"
    failedContractIds: readonly string[]
    latestReceiptPath: string
    passCount: number
    receiptCount: number
    registryVersion: string
    skippedCount: number
    status: QaNightlyBehaviorContractRun["status"]
  }>
  latencyBudgets: Readonly<{
    basis: "qaMetrics_budget_catalog"
    budgetCount: number
    budgets: readonly QaNightlyLatencyBudgetCatalogEntry[]
    evaluatedBy: "packages/khala-qa-harness perf oracle"
    regressionCount: number
    status: "trend_series_active"
    trends: readonly QaNightlyLatencyBudgetTrend[]
  }>
  perfTrends: Readonly<{
    basis: "nightly_step_duration_ms"
    status: "step_duration_trends_budget_catalog_active"
    steps: readonly QaNightlyPerfTrend[]
  }>
  surfaceMarkdownPath: string
  surfaceJsonPath: string
}>

export type QaNightlyCoverageSteeringInput = Readonly<{
  schema: "openagents.khala_code.coverage_frontier_steering_input.v1"
  generatedAt: string
  frontierReportPath: string
  frontierRefs: readonly string[]
  missingCounts: Readonly<Record<keyof KhalaCodeQaCoverageFrontierReport["missing"], number>>
  zeroForAWeekRefs: readonly string[]
}>

export type QaNightlyCoverageArtifacts = Readonly<{
  frontierReport: KhalaCodeQaCoverageFrontierReport
  frontierReportPath: string
  sourceLedgerPaths: readonly string[]
  steeringInput: QaNightlyCoverageSteeringInput
  steeringInputPath: string
  unionLedger: KhalaCodeQaCoverageLedger
  unionLedgerPath: string
}>

export type QaNightlyFlakeQuarantineEntry = Readonly<{
  schema: "openagents.khala_code.qa_flake_quarantine_entry.v1"
  command: readonly string[]
  cwd: string
  evidenceRefs: readonly string[]
  firstAttempt: QaNightlyStepAttemptResult
  generatedAt: string
  reason: "failed_then_passed_retry"
  retryAttempt: QaNightlyStepAttemptResult
  runId: string
  stepId: QaNightlyStepId
  stepLabel: string
}>

export type QaNightlyFlakeQuarantineLedger = Readonly<{
  schema: "openagents.khala_code.qa_flake_quarantine_ledger.v1"
  entries: readonly QaNightlyFlakeQuarantineEntry[]
  generatedAt: string
  policy: "retry_once_then_quarantine_no_silent_green"
  runId: string
}>

const sanitizeRunPart = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "unknown"

const nowIso = (): string => new Date().toISOString()

const positiveInt = (value: string | undefined, fallback: number): number => {
  const parsed = Number.parseInt(value ?? "", 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const quote = (args: readonly string[]): string =>
  args.map(arg => (/^[A-Za-z0-9_./:=@+-]+$/.test(arg) ? arg : JSON.stringify(arg))).join(" ")

const toPosix = (path: string): string => path.split("\\").join("/")

const repoRelative = (root: string, path: string): string =>
  toPosix(relative(root, path)) || "."

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const sorted = (values: Iterable<string>): readonly string[] => [...new Set(values)].sort()

const isoDate = (value: string): string => value.slice(0, 10)

const addDaysIso = (date: string, days: number): string => {
  const timestamp = Date.parse(`${date}T00:00:00.000Z`)
  if (!Number.isFinite(timestamp)) return date
  return new Date(timestamp + days * 24 * 60 * 60 * 1000).toISOString().slice(0, 10)
}

const readJsonFile = async (path: string): Promise<unknown | undefined> => {
  try {
    return JSON.parse(await readFile(path, "utf8"))
  } catch {
    return undefined
  }
}

const monkeyNightOracleFailureReason = async (
  step: QaNightlyStep,
): Promise<string | undefined> => {
  if (step.id !== "monkey-night") return undefined
  const artifactDirIndex = step.command.indexOf("--artifact-dir")
  const artifactDir = artifactDirIndex === -1 ? undefined : step.command[artifactDirIndex + 1]
  if (artifactDir === undefined) return undefined
  const report = await readJsonFile(join(artifactDir, "monkey-night-report.json"))
  if (!isRecord(report)) return undefined
  const memoryOracle = isRecord(report.memoryOracle) ? report.memoryOracle : undefined
  const shutdownOracle = isRecord(report.shutdownOracle) ? report.shutdownOracle : undefined
  if (report.status === "fail") return "monkey night report status is fail"
  if (memoryOracle?.status === "fail") return "monkey night memory oracle failed"
  if (shutdownOracle?.status === "fail") return "monkey night shutdown oracle failed"
  return undefined
}

const walkFiles = async (root: string): Promise<readonly string[]> => {
  let entries: Awaited<ReturnType<typeof readdir>>
  try {
    entries = await readdir(root, { withFileTypes: true })
  } catch {
    return []
  }

  const files: string[] = []
  for (const entry of entries) {
    const path = join(root, entry.name)
    if (entry.isDirectory()) {
      files.push(...await walkFiles(path))
    } else if (entry.isFile()) {
      files.push(path)
    }
  }
  return files.sort((left, right) => left.localeCompare(right))
}

const isCoverageLedger = (value: unknown): value is KhalaCodeQaCoverageLedger =>
  isRecord(value) &&
  value.schema === "khala_code_qa_coverage_ledger.v1" &&
  typeof value.generatedAt === "string" &&
  Array.isArray(value.runIds) &&
  isRecord(value.rpcMethods) &&
  isRecord(value.slashCommands)

const isCoverageFrontierReport = (value: unknown): value is KhalaCodeQaCoverageFrontierReport =>
  isRecord(value) &&
  value.schema === "khala_code_qa_coverage_frontier.v1" &&
  typeof value.generatedAt === "string" &&
  isRecord(value.missing)

const isQaMetricsSnapshot = (value: unknown): value is KhalaCodeQaMetricsSnapshot =>
  isRecord(value) &&
  value.ok === true &&
  value.schema === "openagents.khala_code.qa_metrics.v1" &&
  Array.isArray(value.budgets) &&
  Array.isArray(value.samples)

const isLatencyBudgetRun = (value: unknown): value is QaNightlyLatencyBudgetRun =>
  isRecord(value) &&
  value.schema === "openagents.khala_code.qa_latency_budget_run.v1" &&
  typeof value.generatedAt === "string" &&
  Array.isArray(value.budgets) &&
  Array.isArray(value.sourceSnapshotRefs)

export type QaNightlyReportHistoryEntry = Readonly<{
  generatedAt: string
  latencyBudgetRun?: QaNightlyLatencyBudgetRun | undefined
  runId: string
  status: QaNightlyReport["status"]
  steps: ReadonlyArray<Readonly<{
    durationMs: number
    id: QaNightlyStepId
    status: QaNightlyStepResult["status"]
  }>>
}>

const QA_NIGHTLY_STEP_IDS = new Set<QaNightlyStepId>([
  "harness-suite",
  "behavior-contracts",
  "real-bridge-smoke",
  "desktop-verify",
  "visual-part2-ui",
  "visual-cockpit",
  "visual-composer",
  "monkey-night",
  "model-based",
  "property-tier",
])

const isQaNightlyStepId = (value: unknown): value is QaNightlyStepId =>
  typeof value === "string" && QA_NIGHTLY_STEP_IDS.has(value as QaNightlyStepId)

const isQaNightlyReportHistoryEntry = (value: unknown): value is QaNightlyReportHistoryEntry =>
  isRecord(value) &&
  value.schema === QA_NIGHTLY_MATRIX_SCHEMA &&
  typeof value.generatedAt === "string" &&
  typeof value.runId === "string" &&
  (value.status === "passed" || value.status === "failed") &&
  Array.isArray(value.steps) &&
  value.steps.every(step =>
    isRecord(step) &&
    isQaNightlyStepId(step.id) &&
    typeof step.durationMs === "number" &&
    (
      step.status === "passed" ||
      step.status === "failed" ||
      step.status === "timed_out" ||
      step.status === "flaky"
    ),
  ) &&
  (value.latencyBudgetRun === undefined || isLatencyBudgetRun(value.latencyBudgetRun))

export const khalaCoverageFrontierRefs = (
  frontier: KhalaCodeQaCoverageFrontierReport,
): readonly string[] => sorted([
  ...frontier.missing.rpcMethods.map(item => `rpcMethods:${item}`),
  ...frontier.missing.hotbarPanels.map(item => `hotbarPanels:${item}`),
  ...frontier.missing.slashCommands.map(item => `slashCommands:${item}`),
  ...frontier.missing.selectors.map(item => `selectors:${item}`),
  ...frontier.missing.settingsKeys.map(item => `settingsKeys:${item}`),
  ...frontier.missing.approvalDecisionKinds.map(item => `approvalDecisionKinds:${item}`),
  ...frontier.missing.threadItemVariants.map(item => `threadItemVariants:${item}`),
])

export const computeZeroForConsecutiveCoverageDays = (
  entries: ReadonlyArray<Readonly<{
    generatedAt: string
    refs: readonly string[]
  }>>,
  options: Readonly<{
    days?: number | undefined
    latestDate?: string | undefined
  }> = {},
): readonly string[] => {
  const days = options.days ?? 7
  const latestDate = options.latestDate ?? entries.map(entry => isoDate(entry.generatedAt)).sort().at(-1)
  if (latestDate === undefined) return []

  const refsByDate = new Map<string, Set<string>>()
  for (const entry of [...entries].sort((left, right) => left.generatedAt.localeCompare(right.generatedAt))) {
    refsByDate.set(isoDate(entry.generatedAt), new Set(entry.refs))
  }

  const requiredDates = Array.from({ length: days }, (_, index) => addDaysIso(latestDate, -index))
  const latestRefs = refsByDate.get(requiredDates[0] ?? "")
  if (latestRefs === undefined) return []

  let intersection = new Set(latestRefs)
  for (const date of requiredDates) {
    const refs = refsByDate.get(date)
    if (refs === undefined) return []
    intersection = new Set([...intersection].filter(ref => refs.has(ref)))
  }

  return sorted(intersection)
}

export const collectQaNightlyCoverageLedgerPaths = async (
  input: Readonly<{
    artifactRoot: string
    currentCoverageLedgerPath: string
  }>,
): Promise<readonly string[]> => {
  const paths = new Set<string>()
  for (const path of await walkFiles(input.artifactRoot)) {
    const name = basename(path)
    if (name.endsWith("coverage-ledger.json") && name !== "coverage-union-ledger.json") {
      paths.add(path)
    }
  }
  paths.add(input.currentCoverageLedgerPath)
  return [...paths].sort((left, right) => left.localeCompare(right))
}

const collectQaNightlyCoverageFrontierReportPaths = async (
  artifactRoot: string,
): Promise<readonly string[]> =>
  (await walkFiles(artifactRoot))
    .filter(path => basename(path) === "coverage-frontier-report.json")
    .sort((left, right) => left.localeCompare(right))

const readCoverageLedgers = async (
  paths: readonly string[],
): Promise<ReadonlyArray<{ ledger: KhalaCodeQaCoverageLedger; path: string }>> => {
  const ledgers: Array<{ ledger: KhalaCodeQaCoverageLedger; path: string }> = []
  for (const path of paths) {
    const value = await readJsonFile(path)
    if (isCoverageLedger(value)) ledgers.push({ ledger: value, path })
  }
  return ledgers
}

const readCoverageFrontierHistory = async (
  artifactRoot: string,
): Promise<ReadonlyArray<{ frontier: KhalaCodeQaCoverageFrontierReport; path: string }>> => {
  const frontiers: Array<{ frontier: KhalaCodeQaCoverageFrontierReport; path: string }> = []
  for (const path of await collectQaNightlyCoverageFrontierReportPaths(artifactRoot)) {
    const value = await readJsonFile(path)
    if (isCoverageFrontierReport(value)) frontiers.push({ frontier: value, path })
  }
  return frontiers
}

const collectQaNightlyReportPaths = async (
  artifactRoot: string,
): Promise<readonly string[]> =>
  (await walkFiles(artifactRoot))
    .filter(path => basename(path) === "qa-nightly-report.json")
    .sort((left, right) => left.localeCompare(right))

const readQaNightlyReportHistory = async (
  artifactRoot: string,
): Promise<readonly QaNightlyReportHistoryEntry[]> => {
  const reports: QaNightlyReportHistoryEntry[] = []
  for (const path of await collectQaNightlyReportPaths(artifactRoot)) {
    const value = await readJsonFile(path)
    if (isQaNightlyReportHistoryEntry(value)) reports.push(value)
  }
  return reports.sort((left, right) => left.generatedAt.localeCompare(right.generatedAt))
}

const countCoverageDimension = (
  total: number,
  missing: number,
): QaNightlyCoverageDimensionCount => {
  const boundedMissing = Math.max(0, Math.min(missing, total))
  const covered = Math.max(0, total - boundedMissing)
  return {
    covered,
    coveredPercent: total === 0 ? 100 : Math.round((covered / total) * 1000) / 10,
    missing: boundedMissing,
    total,
  }
}

const coverageDimensionCounts = (
  frontier: KhalaCodeQaCoverageFrontierReport,
): Readonly<Record<QaNightlyCoverageDimension, QaNightlyCoverageDimensionCount>> => {
  const manifest = KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage
  const slashCommandAvailabilityStateTotal = Object.values(manifest.slashCommandAvailabilityStates)
    .reduce((sum, states) => sum + states.length, 0)
  return {
    approvalDecisionKinds: countCoverageDimension(
      manifest.approvalDecisionKinds.length,
      frontier.missing.approvalDecisionKinds.length,
    ),
    hotbarPanels: countCoverageDimension(
      manifest.hotbarPanels.length,
      frontier.missing.hotbarPanels.length,
    ),
    rpcMethods: countCoverageDimension(
      KhalaCodeRpcMethodNames.length,
      frontier.missing.rpcMethods.length,
    ),
    selectors: countCoverageDimension(
      manifest.selectors.length,
      frontier.missing.selectors.length,
    ),
    settingsKeys: countCoverageDimension(
      manifest.settingsKeys.length,
      frontier.missing.settingsKeys.length,
    ),
    slashCommandAvailabilityStates: countCoverageDimension(
      slashCommandAvailabilityStateTotal,
      frontier.missing.slashCommandAvailabilityStates.length,
    ),
    slashCommands: countCoverageDimension(
      manifest.slashCommands.length,
      frontier.missing.slashCommands.length,
    ),
    threadItemVariants: countCoverageDimension(
      manifest.threadItemVariants.length,
      frontier.missing.threadItemVariants.length,
    ),
  }
}

const computeQaNightlyPerfTrends = (
  report: QaNightlyReport,
  history: readonly QaNightlyReportHistoryEntry[],
): readonly QaNightlyPerfTrend[] =>
  report.steps.map(step => {
    const prior = history
      .filter(entry => entry.runId !== report.runId)
      .flatMap(entry => entry.steps.filter(candidate => candidate.id === step.id))
    const previous = prior.at(-1)
    const deltaMs = previous === undefined ? undefined : step.durationMs - previous.durationMs
    return {
      ...(deltaMs === undefined ? {} : { deltaMs }),
      latestDurationMs: step.durationMs,
      ...(previous === undefined ? {} : { previousDurationMs: previous.durationMs }),
      sampleCount: prior.length + 1,
      stepId: step.id,
      trend: previous === undefined
        ? "first_sample"
        : deltaMs === 0
          ? "flat"
          : deltaMs < 0
            ? "improved"
            : "regressed",
    }
  })

const matchesRequiredContext = (
  sample: KhalaCodeQaMetricSample,
  requiredContext: KhalaCodeQaMetricBudget["requiredContext"],
): boolean => {
  if (requiredContext === undefined) return true
  const context = sample.context ?? {}
  return Object.entries(requiredContext).every(([key, value]) => context[key] === value)
}

const collectQaMetricSnapshots = async (input: Readonly<{
  artifactDir: string
  root: string
}>): Promise<ReadonlyArray<Readonly<{ path: string; snapshot: KhalaCodeQaMetricsSnapshot }>>> => {
  const snapshots: Array<{ path: string; snapshot: KhalaCodeQaMetricsSnapshot }> = []
  for (const path of await walkFiles(input.artifactDir)) {
    if (!path.endsWith(".json")) continue
    const value = await readJsonFile(path)
    if (isQaMetricsSnapshot(value)) {
      snapshots.push({ path: repoRelative(input.root, path), snapshot: value })
    }
  }
  return snapshots.sort((left, right) => left.path.localeCompare(right.path))
}

const sampleEvidenceForBudget = (
  budget: KhalaCodeQaMetricBudget,
  snapshots: ReadonlyArray<Readonly<{ path: string; snapshot: KhalaCodeQaMetricsSnapshot }>>,
): readonly QaNightlyLatencyBudgetSampleEvidence[] =>
  snapshots
    .flatMap(({ path, snapshot }) =>
      snapshot.samples
        .filter((sample) =>
          sample.metric === budget.metric &&
          sample.unit === budget.unit &&
          matchesRequiredContext(sample, budget.requiredContext)
        )
        .map((sample) => ({
          observedAt: sample.observedAt,
          sourceRef: path,
          unit: sample.unit,
          value: sample.value,
        }))
    )
    .sort((left, right) => right.value - left.value)
    .slice(0, 5)

export const buildQaNightlyLatencyBudgetRun = async (input: Readonly<{
  artifactDir: string
  generatedAt: string
  root: string
}>): Promise<QaNightlyLatencyBudgetRun> => {
  const snapshots = await collectQaMetricSnapshots({
    artifactDir: input.artifactDir,
    root: input.root,
  })
  const samples = snapshots.flatMap(({ snapshot }) => snapshot.samples)
  return {
    budgetCount: khalaCodeQaMetricBudgets.length,
    budgets: khalaCodeQaMetricBudgets.map((budget) => {
      const evaluation = evaluateKhalaCodeQaMetricBudget(budget, samples)
      return {
        actual: evaluation.actual,
        budgetId: budget.budgetId,
        evaluationStatus: evaluation.status,
        metric: budget.metric,
        ...(budget.percentile === undefined ? {} : { percentile: budget.percentile }),
        sampleCount: evaluation.sampleCount,
        sampleEvidence: sampleEvidenceForBudget(budget, snapshots),
        threshold: budget.threshold,
        unit: budget.unit,
      }
    }),
    generatedAt: input.generatedAt,
    schema: "openagents.khala_code.qa_latency_budget_run.v1",
    sourceSnapshotRefs: snapshots.map(({ path }) => path),
  }
}

const computeQaNightlyLatencyBudgetTrends = (
  current: QaNightlyLatencyBudgetRun,
  history: readonly QaNightlyReportHistoryEntry[],
): readonly QaNightlyLatencyBudgetTrend[] =>
  current.budgets.map((entry) => {
    const prior = history
      .filter(item => item.latencyBudgetRun !== undefined)
      .flatMap(item =>
        item.latencyBudgetRun?.budgets.filter(candidate => candidate.budgetId === entry.budgetId) ?? []
      )
      .filter(candidate => candidate.actual !== null)
    const previous = prior.at(-1)
    const previousActual = previous?.actual ?? undefined
    const delta = entry.actual === null || previousActual === undefined
      ? undefined
      : entry.actual - previousActual
    const trend: QaNightlyLatencyBudgetTrend["trend"] = entry.actual === null
      ? "no_samples"
      : previousActual === undefined
        ? "first_sample"
        : delta === 0
          ? "flat"
          : delta < 0
            ? "improved"
            : "regressed"
    return {
      budgetId: entry.budgetId,
      ...(delta === undefined ? {} : { delta }),
      latestActual: entry.actual,
      metric: entry.metric,
      ...(previousActual === undefined ? {} : { previousActual }),
      sampleCount: entry.sampleCount,
      sampleEvidence: entry.sampleEvidence,
      threshold: entry.threshold,
      trend,
      unit: entry.unit,
    }
  })

const latencyBudgetRegressions = (
  trends: readonly QaNightlyLatencyBudgetTrend[],
): readonly QaNightlyLatencyBudgetTrend[] =>
  trends.filter(trend => trend.trend === "regressed")

const buildQaNightlyLatencyBudgetCatalog = (
  current?: QaNightlyLatencyBudgetRun | undefined,
): readonly QaNightlyLatencyBudgetCatalogEntry[] => {
  const evaluations = new Map(
    evaluateKhalaCodeQaMetricBudgets([]).map(evaluation => [evaluation.budgetId, evaluation]),
  )
  const currentEntries = new Map(current?.budgets.map(entry => [entry.budgetId, entry]) ?? [])
  return khalaCodeQaMetricBudgets.map(budget => {
    const currentEntry = currentEntries.get(budget.budgetId)
    const evaluation = evaluations.get(budget.budgetId)
    return {
      budgetId: budget.budgetId,
      evaluationStatus: currentEntry?.evaluationStatus ?? evaluation?.status ?? "inconclusive",
      metric: budget.metric,
      ...(budget.percentile === undefined ? {} : { percentile: budget.percentile }),
      sampleCount: currentEntry?.sampleCount ?? evaluation?.sampleCount ?? 0,
      threshold: budget.threshold,
      unit: budget.unit,
    }
  })
}

export const emitQaNightlyBehaviorContractArtifacts = async (input: Readonly<{
  artifactDir: string
  generatedAt: string
  root: string
  runId: string
  steps: readonly QaNightlyStepResult[]
}>): Promise<Readonly<{
  receiptPath: string
  receipts: readonly BehaviorContractReceipt[]
  run: QaNightlyBehaviorContractRun
}>> => {
  const receiptPath = join(input.artifactDir, "behavior-contracts", "behavior-contract-receipts.json")
  await mkdir(dirname(receiptPath), { recursive: true })

  const registryValidation = validateBehaviorContractRegistry(khalaCodeUxContractRegistry)
  const coverage = await checkBehaviorContractCoverageFromFiles(
    khalaCodeUxContractRegistry,
    path => readFile(path, "utf8"),
    ref => resolve(process.cwd(), ref),
  )
  const behaviorStep = input.steps.find(step => step.id === "behavior-contracts")
  const desktopVerifyStep = input.steps.find(step => step.id === "desktop-verify")
  const stepCheck = (id: string, step: QaNightlyStepResult | undefined) => ({
    evidenceRefs: step === undefined ? [] : [step.logRef],
    id,
    status: step?.status === "passed" ? "pass" as const : "fail" as const,
    summary: step === undefined
      ? "Nightly step did not run."
      : `Nightly step ${step.id} finished with status ${step.status}.`,
  })
  const receipts = buildBehaviorContractReceipts(khalaCodeUxContractRegistry, {
    checkedAt: input.generatedAt,
    coverage,
    registryValidation,
    runId: input.runId,
    sweepChecks: [
      stepCheck("nightly_step.behavior_contracts", behaviorStep),
      stepCheck("nightly_step.desktop_verify", desktopVerifyStep),
    ],
  })
  await writeFile(receiptPath, `${JSON.stringify({
    checkedAt: input.generatedAt,
    receipts,
    schema: "openagents.khala_code.behavior_contract_receipts.v1",
  }, null, 2)}\n`)
  const failedContractIds = receipts
    .filter(receipt => receipt.status === "fail")
    .map(receipt => receipt.contractId)
  const run: QaNightlyBehaviorContractRun = {
    checkedAt: input.generatedAt,
    failedContractIds,
    passCount: receipts.filter(receipt => receipt.status === "pass").length,
    receiptCount: receipts.length,
    receiptRefs: [repoRelative(input.root, receiptPath)],
    registryVersion: khalaCodeUxContractRegistry.version,
    schema: "openagents.khala_code.behavior_contract_nightly_run.v1",
    skippedCount: receipts.filter(receipt => receipt.status === "skipped").length,
    status: failedContractIds.length === 0 ? "pass" : "fail",
  }

  return {
    receiptPath,
    receipts,
    run,
  }
}

export const buildQaNightlyStatusSurface = (input: Readonly<{
  frontierReport: KhalaCodeQaCoverageFrontierReport
  history: readonly QaNightlyReportHistoryEntry[]
  report: QaNightlyReport
  statusSurfaceJsonPath: string
  statusSurfaceMarkdownPath: string
  unionLedger: KhalaCodeQaCoverageLedger
}>): QaNightlyStatusSurface => {
  const latencyBudgetTrends = computeQaNightlyLatencyBudgetTrends(
    input.report.latencyBudgetRun,
    input.history,
  )
  return {
    coverage: {
      artifactRefs: {
        frontierReportPath: input.report.coverageFrontierReportPath,
        steeringInputPath: input.report.coverageSteeringInputPath,
        unionLedgerPath: input.report.coverageLedgerPath,
      },
      counts: coverageDimensionCounts(input.frontierReport),
      frontierRefCount: khalaCoverageFrontierRefs(input.frontierReport).length,
      sourceLedgerCount: input.report.coverageLedgerSourcePaths.length,
      unionRunCount: input.unionLedger.runIds.length,
      zeroForAWeekCount: input.frontierReport.zeroForAWeekIssueCandidates.length,
    },
    generatedAt: input.report.generatedAt,
    issueStatuses: {
      behaviorContractDeviation: input.report.behaviorContractDeviationIssueStatus,
      latencyBudgetRegression: input.report.latencyBudgetRegressionIssueStatus,
      nightly: input.report.issueStatus,
      quarantine: input.report.quarantineIssueStatus,
      zeroCoverage: input.report.zeroCoverageIssueStatus,
    },
    liveTier: {
      evidenceRefs: [
        "docs/fable/ROADMAP_QA.md",
        "https://github.com/OpenAgentsInc/openagents/issues/8037",
      ],
      reason: "Live-tier Q5 smokes are not part of the Q1 nightly matrix yet; fixture tiers remain no-spend and account-isolated.",
      status: "not_in_matrix",
    },
    behaviorContracts: {
      basis: "behavior_contract_receipts",
      checkedAt: input.report.behaviorContractRun.checkedAt,
      evidenceBoard: "qa-swarm",
      failedContractIds: input.report.behaviorContractRun.failedContractIds,
      latestReceiptPath: input.report.behaviorContractReceiptPath,
      passCount: input.report.behaviorContractRun.passCount,
      receiptCount: input.report.behaviorContractRun.receiptCount,
      registryVersion: input.report.behaviorContractRun.registryVersion,
      skippedCount: input.report.behaviorContractRun.skippedCount,
      status: input.report.behaviorContractRun.status,
    },
    latencyBudgets: {
      basis: "qaMetrics_budget_catalog",
      budgetCount: khalaCodeQaMetricBudgets.length,
      budgets: buildQaNightlyLatencyBudgetCatalog(input.report.latencyBudgetRun),
      evaluatedBy: "packages/khala-qa-harness perf oracle",
      regressionCount: latencyBudgetRegressions(latencyBudgetTrends).length,
      status: "trend_series_active",
      trends: latencyBudgetTrends,
    },
    perfTrends: {
      basis: "nightly_step_duration_ms",
      status: "step_duration_trends_budget_catalog_active",
      steps: computeQaNightlyPerfTrends(input.report, input.history),
    },
    reportJsonPath: input.report.reportJsonPath,
    reportMarkdownPath: input.report.reportMarkdownPath,
    runId: input.report.runId,
    schema: QA_STATUS_SURFACE_SCHEMA,
    status: input.report.status,
    statusSummary: input.report.status === "passed" ? "healthy" : "blocked",
    surfaceJsonPath: input.statusSurfaceJsonPath,
    surfaceMarkdownPath: input.statusSurfaceMarkdownPath,
  }
}

export const emitQaNightlyCoverageArtifacts = async (input: Readonly<{
  artifactDir: string
  artifactRoot: string
  currentCoverageLedgerPath: string
  generatedAt: string
  root: string
  runId: string
}>): Promise<QaNightlyCoverageArtifacts> => {
  const coverageDir = join(input.artifactDir, "coverage")
  const unionLedgerPath = join(coverageDir, "coverage-union-ledger.json")
  const frontierReportPath = join(coverageDir, "coverage-frontier-report.json")
  const steeringInputPath = join(coverageDir, "coverage-frontier-steering.json")
  await mkdir(coverageDir, { recursive: true })

  const ledgerPaths = await collectQaNightlyCoverageLedgerPaths({
    artifactRoot: input.artifactRoot,
    currentCoverageLedgerPath: input.currentCoverageLedgerPath,
  })
  const loadedLedgers = await readCoverageLedgers(ledgerPaths)
  const unionLedger = loadedLedgers.length === 0
    ? createEmptyKhalaCodeQaCoverageLedger({ generatedAt: input.generatedAt, runId: input.runId })
    : mergeKhalaCodeQaCoverageLedgers(loadedLedgers.map(item => item.ledger))

  const initialFrontier = khalaCodeQaCoverageFrontierReport({
    generatedAt: input.generatedAt,
    ledger: unionLedger,
    manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  })
  const historicalFrontiers = await readCoverageFrontierHistory(input.artifactRoot)
  const zeroForAWeek = computeZeroForConsecutiveCoverageDays([
    ...historicalFrontiers.map(({ frontier }) => ({
      generatedAt: frontier.generatedAt,
      refs: khalaCoverageFrontierRefs(frontier),
    })),
    {
      generatedAt: initialFrontier.generatedAt,
      refs: khalaCoverageFrontierRefs(initialFrontier),
    },
  ], {
    latestDate: isoDate(input.generatedAt),
  })
  const frontierReport = khalaCodeQaCoverageFrontierReport({
    generatedAt: input.generatedAt,
    ledger: unionLedger,
    manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
    zeroForAWeek,
  })
  const steeringInput: QaNightlyCoverageSteeringInput = {
    frontierRefs: khalaCoverageFrontierRefs(frontierReport),
    frontierReportPath: repoRelative(input.root, frontierReportPath),
    generatedAt: input.generatedAt,
    missingCounts: {
      approvalDecisionKinds: frontierReport.missing.approvalDecisionKinds.length,
      hotbarPanels: frontierReport.missing.hotbarPanels.length,
      rpcMethods: frontierReport.missing.rpcMethods.length,
      selectors: frontierReport.missing.selectors.length,
      settingsKeys: frontierReport.missing.settingsKeys.length,
      slashCommands: frontierReport.missing.slashCommands.length,
      threadItemVariants: frontierReport.missing.threadItemVariants.length,
    },
    schema: "openagents.khala_code.coverage_frontier_steering_input.v1",
    zeroForAWeekRefs: frontierReport.zeroForAWeekIssueCandidates,
  }

  await writeFile(unionLedgerPath, `${JSON.stringify(unionLedger, null, 2)}\n`)
  await writeFile(frontierReportPath, `${JSON.stringify(frontierReport, null, 2)}\n`)
  await writeFile(steeringInputPath, `${JSON.stringify(steeringInput, null, 2)}\n`)

  return {
    frontierReport,
    frontierReportPath,
    sourceLedgerPaths: loadedLedgers.map(item => item.path),
    steeringInput,
    steeringInputPath,
    unionLedger,
    unionLedgerPath,
  }
}

export const buildQaNightlySteps = (input: Readonly<{
  artifactDir: string
  monkeyRuns?: number | undefined
  monkeySteps?: number | undefined
}>): readonly QaNightlyStep[] => {
  const monkeyRuns = input.monkeyRuns ?? QA_NIGHTLY_DEFAULT_RUNS
  const monkeySteps = input.monkeySteps ?? QA_NIGHTLY_DEFAULT_STEPS
  const monkeyArtifactDir = join(input.artifactDir, "monkey-night")
  return [
    {
      command: ["bun", "run", "--cwd", "packages/khala-qa-harness", "test"],
      cwd: ".",
      id: "harness-suite",
      label: "Khala QA harness suite",
    },
    {
      command: ["bun", "run", "--cwd", "packages/behavior-contracts", "test"],
      cwd: ".",
      id: "behavior-contracts",
      label: "Behavior-contract registry suite",
    },
    {
      command: ["bun", "run", "--cwd", "packages/khala-qa-harness", "smoke:real-bridge"],
      cwd: ".",
      id: "real-bridge-smoke",
      label: "Real HTTP/bearer/SSE seed-corpus smoke",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "verify"],
      cwd: ".",
      id: "desktop-verify",
      label: "Khala Code Desktop verify",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:part2-ui"],
      cwd: ".",
      id: "visual-part2-ui",
      label: "Part 2 UI visual smoke",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:cockpit-visual"],
      cwd: ".",
      id: "visual-cockpit",
      label: "Fleet cockpit visual smoke",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:composer-visual"],
      cwd: ".",
      id: "visual-composer",
      label: "Composer visual smoke",
    },
    {
      command: [
        "bun",
        "src/monkey-night.ts",
        "--runs",
        String(monkeyRuns),
        "--steps",
        String(monkeySteps),
        "--artifact-dir",
        monkeyArtifactDir,
      ],
      cwd: "packages/khala-qa-harness",
      expectedArtifactRefs: [
        join("monkey-night", "monkey-night-report.json"),
        join("monkey-night", "monkey-night-coverage-ledger.json"),
        join("monkey-night", "monkey-night-memory-oracle.json"),
      ],
      id: "monkey-night",
      label: `Seeded monkey night (${monkeyRuns * monkeySteps} actions)`,
    },
    {
      command: ["bun", "test", "src/model-based.test.ts"],
      cwd: "packages/khala-qa-harness",
      id: "model-based",
      label: "Model-based tier",
    },
    {
      command: [
        "bun",
        "test",
        "tests/composer-draft-model.property.test.ts",
        "tests/codex-thread-item-projector.property.test.ts",
        "tests/transcript-render.property.test.ts",
      ],
      cwd: "clients/khala-code-desktop",
      id: "property-tier",
      label: "Property tier",
    },
  ] as const
}

export const runSpawnedQaNightlyCommand: QaNightlyCommandRunner = async (
  step,
  timeoutMs,
) => {
  const startedAt = Date.now()
  const [cmd, ...args] = step.command
  if (cmd === undefined) {
    return {
      durationMs: 0,
      exitCode: 1,
      stderr: "empty command",
      stdout: "",
    }
  }

  return new Promise(resolveResult => {
    const child = spawn(cmd, args, {
      cwd: step.cwd,
      detached: true,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    })
    const stdoutChunks: Buffer[] = []
    const stderrChunks: Buffer[] = []
    let timedOut = false
    let killTimer: ReturnType<typeof setTimeout> | undefined

    const killTree = (signal: NodeJS.Signals) => {
      try {
        process.kill(-child.pid, signal)
      } catch {
        child.kill(signal)
      }
    }

    const timer = setTimeout(() => {
      timedOut = true
      killTree("SIGTERM")
      killTimer = setTimeout(() => killTree("SIGKILL"), 5_000)
    }, timeoutMs)

    child.stdout?.on("data", chunk => stdoutChunks.push(Buffer.from(chunk)))
    child.stderr?.on("data", chunk => stderrChunks.push(Buffer.from(chunk)))
    child.on("error", error => {
      clearTimeout(timer)
      if (killTimer !== undefined) clearTimeout(killTimer)
      resolveResult({
        durationMs: Date.now() - startedAt,
        exitCode: 1,
        stderr: error instanceof Error ? error.message : String(error),
        stdout: Buffer.concat(stdoutChunks).toString("utf8"),
      })
    })
    child.on("close", code => {
      clearTimeout(timer)
      if (killTimer !== undefined) clearTimeout(killTimer)
      resolveResult({
        durationMs: Date.now() - startedAt,
        exitCode: timedOut ? 124 : code ?? 1,
        stderr: Buffer.concat(stderrChunks).toString("utf8"),
        stdout: Buffer.concat(stdoutChunks).toString("utf8"),
        timedOut,
      })
    })
  })
}

const assertPublicSafeArtifact = (artifact: unknown, label: string): void => {
  const text = JSON.stringify(artifact)
  const unsafe = /\/Users\/|\/home\/|~\/|auth\.json|bearer\s+[a-z0-9._-]+|sk-[a-z0-9]/i
  if (unsafe.test(text)) {
    throw new Error(`${label} contains a private path or credential-shaped value`)
  }
}

export const assertQaNightlyReportPublicSafe = (report: QaNightlyReport): void => {
  assertPublicSafeArtifact(report, "QA nightly report")
}

export const assertQaStatusSurfacePublicSafe = (surface: QaNightlyStatusSurface): void => {
  assertPublicSafeArtifact(surface, "QA status surface")
}

export const renderQaNightlyMarkdown = (report: QaNightlyReport): string => {
  const rows = report.steps
    .map(step => `| ${step.id} | ${step.status} | ${step.exitCode} | ${step.durationMs} | ${step.logRef} |`)
    .join("\n")
  const failed = report.steps.filter(step => step.status !== "passed")
  const failedLines = failed.length === 0
    ? "None."
    : failed.map(step => `- ${step.id}: ${step.status} (${step.logRef})`).join("\n")
  const sourceLedgers = report.coverageLedgerSourcePaths.length === 0
    ? "None; emitted an empty baseline union ledger."
    : report.coverageLedgerSourcePaths.map(path => `- \`${path}\``).join("\n")
  const zeroCoverage = report.zeroCoverageIssueStatus === undefined
    ? "Not evaluated."
    : report.zeroCoverageIssueStatus.status === "filed"
      ? `Filed: ${report.zeroCoverageIssueStatus.issueUrl ?? "issue URL unavailable"}`
      : `${report.zeroCoverageIssueStatus.status}: ${report.zeroCoverageIssueStatus.reason}`
  const quarantine = report.quarantineIssueStatus === undefined
    ? "Not evaluated."
    : report.quarantineIssueStatus.status === "filed"
      ? `Filed: ${report.quarantineIssueStatus.issueUrl ?? "issue URL unavailable"}`
      : `${report.quarantineIssueStatus.status}: ${report.quarantineIssueStatus.reason}`
  const behaviorContracts = report.behaviorContractDeviationIssueStatus === undefined
    ? "Not evaluated."
    : report.behaviorContractDeviationIssueStatus.status === "filed"
      ? `Filed: ${report.behaviorContractDeviationIssueStatus.issueUrl ?? "issue URL unavailable"}`
      : `${report.behaviorContractDeviationIssueStatus.status}: ${report.behaviorContractDeviationIssueStatus.reason}`

  return `# Khala Code QA Nightly Matrix

- Run: \`${report.runId}\`
- Generated: \`${report.generatedAt}\`
- Status: \`${report.status}\`
- Status surface JSON: \`${report.statusSurfaceJsonPath}\`
- Status surface markdown: \`${report.statusSurfaceMarkdownPath}\`
- Behavior-contract receipts: \`${report.behaviorContractReceiptPath}\`
- Flake quarantine ledger: \`${report.quarantineLedgerPath}\`
- Coverage union ledger: \`${report.coverageLedgerPath}\`
- Coverage frontier report: \`${report.coverageFrontierReportPath}\`
- Explorer steering input: \`${report.coverageSteeringInputPath}\`

## Steps

| Step | Status | Exit | Duration ms | Log |
| --- | --- | ---: | ---: | --- |
${rows}

## Failures

${failedLines}

## Flake Quarantine

Policy: retry once, quarantine pass-after-fail, never silently retry green.

Quarantine issue status: ${quarantine}

## Behavior Contracts

- Receipt status: \`${report.behaviorContractRun.status}\`
- Registry version: \`${report.behaviorContractRun.registryVersion}\`
- Receipts: \`${report.behaviorContractRun.receiptCount}\`
- Passed: \`${report.behaviorContractRun.passCount}\`
- Skipped: \`${report.behaviorContractRun.skippedCount}\`
- Failed contracts: ${report.behaviorContractRun.failedContractIds.length === 0 ? "None." : report.behaviorContractRun.failedContractIds.map(id => `\`${id}\``).join(", ")}
- Deviation issue status: ${behaviorContracts}

## Coverage Frontier

Source ledgers:

${sourceLedgers}

Zero-for-seven-days issue status: ${zeroCoverage}
`
}

export const renderQaStatusSurfaceMarkdown = (surface: QaNightlyStatusSurface): string => {
  const coverageRows = Object.entries(surface.coverage.counts)
    .map(([dimension, count]) =>
      `| ${dimension} | ${count.covered} | ${count.missing} | ${count.total} | ${count.coveredPercent}% |`
    )
    .join("\n")
  const trendRows = surface.perfTrends.steps
    .map(step => {
      const previous = step.previousDurationMs === undefined ? "n/a" : String(step.previousDurationMs)
      const delta = step.deltaMs === undefined ? "n/a" : String(step.deltaMs)
      return `| ${step.stepId} | ${step.latestDurationMs} | ${previous} | ${delta} | ${step.trend} | ${step.sampleCount} |`
    })
    .join("\n")
  const latencyBudgetRows = surface.latencyBudgets.budgets
    .map(budget => {
      const percentile = budget.percentile === undefined ? "n/a" : `p${budget.percentile}`
      return `| ${budget.budgetId} | ${budget.metric} | ${budget.threshold} | ${budget.unit} | ${percentile} | ${budget.evaluationStatus} | ${budget.sampleCount} |`
    })
    .join("\n")
  const latencyTrendRows = surface.latencyBudgets.trends
    .map(trend => {
      const previous = trend.previousActual === undefined ? "n/a" : String(trend.previousActual)
      const delta = trend.delta === undefined ? "n/a" : String(trend.delta)
      const latest = trend.latestActual === null ? "n/a" : String(trend.latestActual)
      return `| ${trend.budgetId} | ${latest} | ${previous} | ${delta} | ${trend.trend} | ${trend.sampleCount} |`
    })
    .join("\n")
  const issueStatus = (status: QaNightlyIssueStatus | undefined): string => {
    if (status === undefined) return "not evaluated"
    if (status.status === "filed") return `filed ${status.issueUrl ?? ""}`.trim()
    return `${status.status}: ${status.reason}`
  }

  return `# Khala Code QA Status

- Run: \`${surface.runId}\`
- Generated: \`${surface.generatedAt}\`
- Health: \`${surface.statusSummary}\`
- Matrix status: \`${surface.status}\`
- Report JSON: \`${surface.reportJsonPath}\`
- Report markdown: \`${surface.reportMarkdownPath}\`

## Live Tier

Status: \`${surface.liveTier.status}\`

${surface.liveTier.reason}

Evidence refs:
${surface.liveTier.evidenceRefs.map(ref => `- \`${ref}\``).join("\n")}

## Behavior Contracts

Basis: \`${surface.behaviorContracts.basis}\`

Status: \`${surface.behaviorContracts.status}\`

- Registry version: \`${surface.behaviorContracts.registryVersion}\`
- Checked at: \`${surface.behaviorContracts.checkedAt}\`
- Latest receipts: \`${surface.behaviorContracts.latestReceiptPath}\`
- Receipt count: \`${surface.behaviorContracts.receiptCount}\`
- Passed: \`${surface.behaviorContracts.passCount}\`
- Skipped: \`${surface.behaviorContracts.skippedCount}\`
- Failed contracts: ${surface.behaviorContracts.failedContractIds.length === 0 ? "None." : surface.behaviorContracts.failedContractIds.map(id => `\`${id}\``).join(", ")}

## Coverage

- Union runs: \`${surface.coverage.unionRunCount}\`
- Source ledgers: \`${surface.coverage.sourceLedgerCount}\`
- Frontier refs: \`${surface.coverage.frontierRefCount}\`
- Zero for seven days: \`${surface.coverage.zeroForAWeekCount}\`
- Union ledger: \`${surface.coverage.artifactRefs.unionLedgerPath}\`
- Frontier report: \`${surface.coverage.artifactRefs.frontierReportPath}\`
- Steering input: \`${surface.coverage.artifactRefs.steeringInputPath}\`

| Dimension | Covered | Missing | Total | Covered % |
| --- | ---: | ---: | ---: | ---: |
${coverageRows}

## Perf Trends

Basis: \`${surface.perfTrends.basis}\`

Status: \`${surface.perfTrends.status}\`

| Step | Latest ms | Previous ms | Delta ms | Trend | Samples |
| --- | ---: | ---: | ---: | --- | ---: |
${trendRows}

## Latency Budgets

Basis: \`${surface.latencyBudgets.basis}\`

Status: \`${surface.latencyBudgets.status}\`

Evaluated by: \`${surface.latencyBudgets.evaluatedBy}\`

Budget count: \`${surface.latencyBudgets.budgetCount}\`

Regression count: \`${surface.latencyBudgets.regressionCount}\`

| Budget | Metric | Threshold | Unit | Percentile | Evaluation | Samples |
| --- | --- | ---: | --- | --- | --- | ---: |
${latencyBudgetRows}

### Budget Trends

| Budget | Latest | Previous | Delta | Trend | Samples |
| --- | ---: | ---: | ---: | --- | ---: |
${latencyTrendRows}

## Issues

- Nightly: ${issueStatus(surface.issueStatuses.nightly)}
- Behavior contract deviation: ${issueStatus(surface.issueStatuses.behaviorContractDeviation)}
- Flake quarantine: ${issueStatus(surface.issueStatuses.quarantine)}
- Latency budget regression: ${issueStatus(surface.issueStatuses.latencyBudgetRegression)}
- Zero coverage: ${issueStatus(surface.issueStatuses.zeroCoverage)}
`
}

export const buildQaNightlyFailureIssueBody = (report: QaNightlyReport): string => {
  const failed = report.steps.filter(step => step.status !== "passed")
  return `### Affected surface

Khala Code Desktop fully automated QA cycle, nightly owned-runner matrix.

### Actual behavior

Nightly run \`${report.runId}\` finished with status \`${report.status}\`.

Failed steps:
${failed.map(step => `- \`${step.id}\` ${step.status}, exit ${step.exitCode}, log \`${step.logRef}\``).join("\n")}

### Expected behavior

The full fixture QA matrix should pass: harness suite, desktop verify, visual smokes, monkey night, model-based tier, and property tier.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.reportMarkdownPath}\` and the step logs referenced above.

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Report markdown: \`${report.reportMarkdownPath}\`
- Status surface JSON: \`${report.statusSurfaceJsonPath}\`
- Status surface markdown: \`${report.statusSurfaceMarkdownPath}\`
- Flake quarantine ledger: \`${report.quarantineLedgerPath}\`
- Coverage union ledger: \`${report.coverageLedgerPath}\`
- Coverage frontier report: \`${report.coverageFrontierReportPath}\`
- Explorer steering input: \`${report.coverageSteeringInputPath}\`

### Severity

S1 - supported user or agent path is broken

### Environment

Owned runner nightly matrix. Generated at \`${report.generatedAt}\`.

### Safety and redaction

The report includes public-safe refs, step IDs, exit codes, and artifact paths only. Raw command logs remain in the owned-runner artifact directory and must be redaction-reviewed before external publication.
`
}

export const buildQaNightlyZeroCoverageIssueBody = (
  report: QaNightlyReport,
  frontier: KhalaCodeQaCoverageFrontierReport,
): string => {
  const candidates = frontier.zeroForAWeekIssueCandidates
  return `### Affected surface

Khala Code Desktop fully automated QA coverage frontier.

### Actual behavior

Nightly run \`${report.runId}\` found coverage classes that remained at zero for seven consecutive coverage days:
${candidates.map(candidate => `- \`${candidate}\``).join("\n")}

### Expected behavior

The explorer should eventually exercise each manifest coverage class, or the missing class should be explicitly owned with a new scenario, selector, RPC action, or model-boundary exception.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.coverageFrontierReportPath}\` and \`${report.coverageSteeringInputPath}\`.

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Status surface JSON: \`${report.statusSurfaceJsonPath}\`
- Coverage union ledger: \`${report.coverageLedgerPath}\`
- Coverage frontier report: \`${report.coverageFrontierReportPath}\`
- Explorer steering input: \`${report.coverageSteeringInputPath}\`

### Severity

S3 - coverage gap with automated steering impact

### Environment

Owned runner nightly matrix. Generated at \`${report.generatedAt}\`.

### Safety and redaction

The frontier contains coverage class names and repo-relative artifact refs only; it does not include account data, secrets, or raw command logs.
`
}

export const buildQaNightlyLatencyBudgetRegressionIssueBody = (
  report: QaNightlyReport,
  regressions: readonly QaNightlyLatencyBudgetTrend[],
): string => {
  const rows = regressions
    .map(regression =>
      `| \`${regression.budgetId}\` | \`${regression.metric}\` | ${regression.previousActual ?? "n/a"} | ${regression.latestActual ?? "n/a"} | ${regression.delta ?? "n/a"} | ${regression.threshold}${regression.unit} |`
    )
    .join("\n")
  const sampleRows = regressions
    .flatMap(regression =>
      regression.sampleEvidence.map(sample =>
        `| \`${regression.budgetId}\` | ${sample.value}${sample.unit} | ${sample.observedAt} | \`${sample.sourceRef}\` |`
      )
    )
    .join("\n")
  return `### Affected surface

Khala Code Desktop latency budget trend reporting.

### Actual behavior

Nightly run \`${report.runId}\` observed latency budget regressions against the previous persisted nightly samples.

| Budget | Metric | Previous | Latest | Delta | Threshold |
| --- | --- | ---: | ---: | ---: | ---: |
${rows}

### Expected behavior

Each budget should stay flat or improve, or a child optimization issue should be opened with the offending samples.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.statusSurfaceJsonPath}\` and the \`latencyBudgets.trends\` rows.

### Sample evidence

| Budget | Sample | Observed at | Source |
| --- | ---: | --- | --- |
${sampleRows || "| n/a | n/a | n/a | n/a |"}

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Status surface JSON: \`${report.statusSurfaceJsonPath}\`
- Status surface markdown: \`${report.statusSurfaceMarkdownPath}\`
- Source qaMetrics snapshots: ${report.latencyBudgetRun.sourceSnapshotRefs.map(ref => `\`${ref}\``).join(", ") || "none"}

### Severity

S2 - measured latency regression in the automated QA loop

### Safety and redaction

The issue body includes budget IDs, metric names, numeric samples, timestamps, and repo-relative artifact refs only. It does not include raw command logs, local absolute paths, account identifiers, or provider payloads.
`
}

export const buildQaNightlyBehaviorContractDeviationIssueBody = (
  report: QaNightlyReport,
  failedReceipts: readonly BehaviorContractReceipt[],
): string => {
  const rows = failedReceipts
    .map(receipt =>
      `| \`${receipt.contractId}\` | ${receipt.statement} | \`${receipt.status}\` | \`${receipt.receiptId}\` |`
    )
    .join("\n")
  const checkRows = failedReceipts
    .flatMap(receipt =>
      receipt.checks
        .filter(check => check.status === "fail")
        .map(check =>
          `| \`${receipt.contractId}\` | \`${check.id}\` | ${check.summary} | ${check.evidenceRefs.map(ref => `\`${ref}\``).join(", ") || "none"} |`
        )
    )
    .join("\n")

  return `### Affected surface

Khala Code Desktop behavior-contract nightly enforcement.

### Actual behavior

Nightly run \`${report.runId}\` emitted failed behavior-contract receipts.

| Contract | Statement | Receipt status | Receipt |
| --- | --- | --- | --- |
${rows}

### Expected behavior

Every enforced behavior contract should pass its registry, oracle-coverage, and nightly sweep checks. Receipts record evidence only and do not flip registry state.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.behaviorContractReceiptPath}\` and the failed checks listed below.

### Failing checks

| Contract | Check | Summary | Evidence refs |
| --- | --- | --- | --- |
${checkRows || "| n/a | n/a | n/a | n/a |"}

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Status surface JSON: \`${report.statusSurfaceJsonPath}\`
- Status surface markdown: \`${report.statusSurfaceMarkdownPath}\`
- Behavior-contract receipts: \`${report.behaviorContractReceiptPath}\`

### Severity

S2 - stated product behavior contract deviation in the automated QA loop

### Safety and redaction

The issue body includes contract IDs, owner/customer statements already committed in the public registry, public-safe receipt IDs, and repo-relative evidence refs only. Raw command logs remain in the owned-runner artifact directory and must be redaction-reviewed before external publication.
`
}

export const buildQaNightlyFlakeQuarantineIssueBody = (
  report: QaNightlyReport,
  ledger: QaNightlyFlakeQuarantineLedger,
): string => {
  const entries = ledger.entries
  return `### Affected surface

Khala Code Desktop fully automated QA nightly flake quarantine.

### Actual behavior

Nightly run \`${report.runId}\` observed one or more steps fail once and pass on the single retry. These are quarantined bugs, not green retries:
${entries.map(entry =>
  `- \`${entry.stepId}\`: first ${entry.firstAttempt.status} exit ${entry.firstAttempt.exitCode} (\`${entry.firstAttempt.logRef}\`), retry passed (\`${entry.retryAttempt.logRef}\`)`
).join("\n")}

### Expected behavior

The first attempt should pass deterministically. A pass-after-fail must remain tracked until the underlying product or harness bug is fixed and the quarantine entry is removed.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.quarantineLedgerPath}\` and the first-attempt/retry logs listed above.

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Report markdown: \`${report.reportMarkdownPath}\`
- Status surface JSON: \`${report.statusSurfaceJsonPath}\`
- Status surface markdown: \`${report.statusSurfaceMarkdownPath}\`
- Quarantine ledger: \`${report.quarantineLedgerPath}\`
${entries.flatMap(entry => entry.evidenceRefs.map(ref => `- Evidence ref: \`${ref}\``)).join("\n")}

### Severity

S2 - intermittent failure in the automated QA gate

### Environment

Owned runner nightly matrix. Generated at \`${report.generatedAt}\`.

### Safety and redaction

The quarantine ledger contains step IDs, commands, exit codes, and public-safe artifact refs only. Raw logs remain in the owned-runner artifact directory and must be redaction-reviewed before external publication.
`
}

export const fileQaNightlyFailureIssueWithGh: QaNightlyIssueFiler = async ({
  bodyPath,
  report,
  title,
}) => {
  const gh = spawn("gh", [
    "issue",
    "create",
    "--title",
    title,
    "--body-file",
    bodyPath,
    "--label",
    "bug",
    "--label",
    "qa",
  ], {
    stdio: ["ignore", "pipe", "pipe"],
  })
  const stdoutChunks: Buffer[] = []
  const stderrChunks: Buffer[] = []
  gh.stdout?.on("data", chunk => stdoutChunks.push(Buffer.from(chunk)))
  gh.stderr?.on("data", chunk => stderrChunks.push(Buffer.from(chunk)))
  const exitCode = await new Promise<number>(resolveExit => {
    gh.on("close", code => resolveExit(code ?? 1))
    gh.on("error", () => resolveExit(1))
  })
  if (exitCode !== 0) {
    return {
      reason: Buffer.concat(stderrChunks).toString("utf8").trim() || `gh exited ${exitCode}`,
      status: "failed",
    }
  }
  const issueUrl = Buffer.concat(stdoutChunks).toString("utf8").trim().split(/\s+/)[0]
  return {
    issueUrl: issueUrl === "" ? undefined : issueUrl,
    status: "filed",
  }
}

export const runQaNightlyMatrix = async (input: Readonly<{
  artifactRoot?: string | undefined
  commandRunner?: QaNightlyCommandRunner | undefined
  env?: Readonly<Record<string, string | undefined>> | undefined
  issueFiler?: QaNightlyIssueFiler | undefined
  now?: (() => string) | undefined
  root?: string | undefined
  stepTimeoutMs?: number | undefined
}> = {}): Promise<QaNightlyReport> => {
  const env = input.env ?? process.env
  const root = resolve(input.root ?? process.cwd())
  const generatedAt = input.now?.() ?? nowIso()
  const runId = `khala-code-qa-nightly-${sanitizeRunPart(generatedAt.replace(/[:]/g, ""))}`
  const artifactRoot = input.artifactRoot ?? env.OA_QA_NIGHTLY_ARTIFACT_DIR ?? join(root, "var", "qa-nightly")
  const artifactDir = join(artifactRoot, runId)
  const commandRunner = input.commandRunner ?? runSpawnedQaNightlyCommand
  const timeoutMs =
    input.stepTimeoutMs ?? positiveInt(env.OA_QA_NIGHTLY_STEP_TIMEOUT_MS, QA_NIGHTLY_DEFAULT_STEP_TIMEOUT_MS)
  const monkeyRuns = positiveInt(env.OA_QA_NIGHTLY_MONKEY_RUNS, QA_NIGHTLY_DEFAULT_RUNS)
  const monkeySteps = positiveInt(env.OA_QA_NIGHTLY_MONKEY_STEPS, QA_NIGHTLY_DEFAULT_STEPS)
  const steps = buildQaNightlySteps({ artifactDir, monkeyRuns, monkeySteps })
  const results: QaNightlyStepResult[] = []
  const quarantineEntries: QaNightlyFlakeQuarantineEntry[] = []

  await mkdir(artifactDir, { recursive: true })
  const runStepAttempt = async (
    step: QaNightlyStep,
    absoluteStep: QaNightlyStep,
    attempt: 1 | 2,
  ): Promise<QaNightlyStepAttemptResult> => {
    const logPath = join(artifactDir, "logs", attempt === 1 ? `${step.id}.log` : `${step.id}.retry.log`)
    await mkdir(dirname(logPath), { recursive: true })
    const result = await commandRunner(absoluteStep, timeoutMs)
    const status = result.timedOut === true
      ? "timed_out"
      : result.exitCode === 0
        ? "passed"
        : "failed"
    await writeFile(
      logPath,
      [
        `$ ${quote(step.command)}`,
        `cwd: ${step.cwd}`,
        `attempt: ${attempt}`,
        `status: ${status}`,
        `exitCode: ${result.exitCode}`,
        `durationMs: ${result.durationMs}`,
        "",
        "## stdout",
        result.stdout,
        "",
        "## stderr",
        result.stderr,
        "",
      ].join("\n"),
    )
    return {
      attempt,
      durationMs: result.durationMs,
      exitCode: result.exitCode,
      logRef: repoRelative(root, logPath),
      status,
    }
  }

  for (const step of steps) {
    const absoluteStep = {
      ...step,
      cwd: resolve(root, step.cwd),
    }
    const firstAttempt = await runStepAttempt(step, absoluteStep, 1)
    const attempts: QaNightlyStepAttemptResult[] = [firstAttempt]
    if (firstAttempt.status !== "passed") {
      attempts.push(await runStepAttempt(step, absoluteStep, 2))
    }
    const lastAttempt = attempts.at(-1) ?? firstAttempt
    const artifactFailureReason = await monkeyNightOracleFailureReason(absoluteStep)
    const status: QaNightlyStepResult["status"] = firstAttempt.status === "passed"
      ? artifactFailureReason === undefined ? "passed" : "failed"
      : lastAttempt.status === "passed"
        ? artifactFailureReason === undefined ? "flaky" : "failed"
        : lastAttempt.status
    const evidenceRefs = [
      firstAttempt.logRef,
      ...(attempts.length > 1 ? [lastAttempt.logRef] : []),
      ...(step.expectedArtifactRefs ?? []).map(ref => repoRelative(root, join(artifactDir, ref))),
    ]
    if (status === "flaky") {
      quarantineEntries.push({
        command: step.command,
        cwd: step.cwd,
        evidenceRefs,
        firstAttempt,
        generatedAt,
        reason: "failed_then_passed_retry",
        retryAttempt: lastAttempt,
        runId,
        schema: "openagents.khala_code.qa_flake_quarantine_entry.v1",
        stepId: step.id,
        stepLabel: step.label,
      })
    }
    results.push({
      attempts,
      command: step.command,
      cwd: step.cwd,
      durationMs: attempts.reduce((sum, attempt) => sum + attempt.durationMs, 0),
      exitCode: artifactFailureReason === undefined
        ? status === "flaky" ? firstAttempt.exitCode : lastAttempt.exitCode
        : 1,
      id: step.id,
      label: step.label,
      logRef: status === "flaky" ? firstAttempt.logRef : lastAttempt.logRef,
      status,
    })
  }

  const reportJsonPath = join(artifactDir, "qa-nightly-report.json")
  const reportMarkdownPath = join(artifactDir, "qa-nightly-report.md")
  const issueBodyPath = join(artifactDir, "qa-nightly-failure-issue.md")
  const zeroCoverageIssueBodyPath = join(artifactDir, "qa-nightly-zero-coverage-issue.md")
  const quarantineIssueBodyPath = join(artifactDir, "qa-nightly-flake-quarantine-issue.md")
  const latencyBudgetRegressionIssueBodyPath = join(artifactDir, "qa-nightly-latency-budget-regression-issue.md")
  const statusSurfaceJsonPath = join(artifactDir, "qa-status-surface.json")
  const statusSurfaceMarkdownPath = join(artifactDir, "qa-status-surface.md")
  const quarantineLedgerPath = join(artifactDir, "quarantine", "flake-quarantine-ledger.json")
  const quarantineLedger: QaNightlyFlakeQuarantineLedger = {
    entries: quarantineEntries,
    generatedAt,
    policy: "retry_once_then_quarantine_no_silent_green",
    runId,
    schema: "openagents.khala_code.qa_flake_quarantine_ledger.v1",
  }
  await mkdir(dirname(quarantineLedgerPath), { recursive: true })
  await writeFile(quarantineLedgerPath, `${JSON.stringify(quarantineLedger, null, 2)}\n`)
  const monkeyCoverageLedgerPath = join(artifactDir, "monkey-night", "monkey-night-coverage-ledger.json")
  const coverageArtifacts = await emitQaNightlyCoverageArtifacts({
    artifactDir,
    artifactRoot,
    currentCoverageLedgerPath: monkeyCoverageLedgerPath,
    generatedAt,
    root,
    runId,
  })
  const latencyBudgetRun = await buildQaNightlyLatencyBudgetRun({
    artifactDir,
    generatedAt,
    root,
  })
  const behaviorContractArtifacts = await emitQaNightlyBehaviorContractArtifacts({
    artifactDir,
    generatedAt,
    root,
    runId,
    steps: results,
  })
  const history = (await readQaNightlyReportHistory(artifactRoot))
    .filter(entry => entry.runId !== runId)
  const reportBase: QaNightlyReport = {
    artifactDir: repoRelative(root, artifactDir),
    behaviorContractReceiptPath: repoRelative(root, behaviorContractArtifacts.receiptPath),
    behaviorContractRun: behaviorContractArtifacts.run,
    coverageFrontierReportPath: repoRelative(root, coverageArtifacts.frontierReportPath),
    coverageLedgerPath: repoRelative(root, coverageArtifacts.unionLedgerPath),
    coverageLedgerSourcePaths: coverageArtifacts.sourceLedgerPaths.map(path => repoRelative(root, path)),
    coverageSteeringInputPath: repoRelative(root, coverageArtifacts.steeringInputPath),
    generatedAt,
    latencyBudgetRun,
    quarantineLedgerPath: repoRelative(root, quarantineLedgerPath),
    reportJsonPath: repoRelative(root, reportJsonPath),
    reportMarkdownPath: repoRelative(root, reportMarkdownPath),
    runId,
    schema: QA_NIGHTLY_MATRIX_SCHEMA,
    status: results.every(step => step.status === "passed") ? "passed" : "failed",
    statusSurfaceJsonPath: repoRelative(root, statusSurfaceJsonPath),
    statusSurfaceMarkdownPath: repoRelative(root, statusSurfaceMarkdownPath),
    steps: results,
  }

  const issueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    if (reportBase.status === "passed") {
      return { reason: "nightly matrix passed", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyFailureIssueBody(reportBase)
    await writeFile(issueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: issueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code QA nightly failed ${reportBase.runId}`,
    })
  })()
  const quarantineIssueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    if (quarantineLedger.entries.length === 0) {
      return { reason: "no pass-after-fail retries were observed", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_QUARANTINE_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_QUARANTINE_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyFlakeQuarantineIssueBody(reportBase, quarantineLedger)
    await writeFile(quarantineIssueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: quarantineIssueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code QA flake quarantined ${quarantineLedger.entries[0]?.stepId ?? reportBase.runId}`,
    })
  })()
  const zeroCoverageIssueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    if (coverageArtifacts.frontierReport.zeroForAWeekIssueCandidates.length === 0) {
      return { reason: "no coverage class has been zero for seven consecutive days", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_COVERAGE_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_COVERAGE_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyZeroCoverageIssueBody(reportBase, coverageArtifacts.frontierReport)
    await writeFile(zeroCoverageIssueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: zeroCoverageIssueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code QA coverage stayed zero ${coverageArtifacts.frontierReport.zeroForAWeekIssueCandidates[0]}`,
    })
  })()
  const latencyRegressions = latencyBudgetRegressions(
    computeQaNightlyLatencyBudgetTrends(latencyBudgetRun, history),
  )
  const latencyBudgetRegressionIssueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    if (latencyRegressions.length === 0) {
      return { reason: "no latency budget regression was observed", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_PERF_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_PERF_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyLatencyBudgetRegressionIssueBody(reportBase, latencyRegressions)
    await writeFile(latencyBudgetRegressionIssueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: latencyBudgetRegressionIssueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code latency budget regressed ${latencyRegressions[0]?.budgetId ?? reportBase.runId}`,
    })
  })()
  const behaviorContractDeviationIssueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    const failedReceipts = behaviorContractArtifacts.receipts.filter(receipt => receipt.status === "fail")
    if (failedReceipts.length === 0) {
      return { reason: "no behavior contract deviation was observed", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_CONTRACT_DEVIATION_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_CONTRACT_DEVIATION_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyBehaviorContractDeviationIssueBody(reportBase, failedReceipts)
    const issueBodyPath = join(artifactDir, "qa-nightly-behavior-contract-deviation-issue.md")
    await writeFile(issueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: issueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code behavior contract deviated ${failedReceipts[0]?.contractId ?? reportBase.runId}`,
    })
  })()

  const report: QaNightlyReport = {
    ...reportBase,
    ...(issueStatus === undefined ? {} : { issueStatus }),
    ...(behaviorContractDeviationIssueStatus === undefined ? {} : { behaviorContractDeviationIssueStatus }),
    ...(quarantineIssueStatus === undefined ? {} : { quarantineIssueStatus }),
    ...(latencyBudgetRegressionIssueStatus === undefined ? {} : { latencyBudgetRegressionIssueStatus }),
    ...(zeroCoverageIssueStatus === undefined ? {} : { zeroCoverageIssueStatus }),
  }
  assertQaNightlyReportPublicSafe(report)
  const statusSurface = buildQaNightlyStatusSurface({
    frontierReport: coverageArtifacts.frontierReport,
    history,
    report,
    statusSurfaceJsonPath: report.statusSurfaceJsonPath,
    statusSurfaceMarkdownPath: report.statusSurfaceMarkdownPath,
    unionLedger: coverageArtifacts.unionLedger,
  })
  assertQaStatusSurfacePublicSafe(statusSurface)
  await writeFile(reportJsonPath, `${JSON.stringify(report, null, 2)}\n`)
  await writeFile(reportMarkdownPath, renderQaNightlyMarkdown(report))
  await writeFile(statusSurfaceJsonPath, `${JSON.stringify(statusSurface, null, 2)}\n`)
  await writeFile(statusSurfaceMarkdownPath, renderQaStatusSurfaceMarkdown(statusSurface))
  return report
}

if (import.meta.main) {
  const report = await runQaNightlyMatrix()
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)
  process.exit(report.status === "passed" ? 0 : 1)
}
