import { spawn } from "node:child_process"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { basename, join } from "node:path"

import { Effect } from "effect"

import {
  khalaCodeQaMetricBudgets,
  type KhalaCodeQaMetricBudget,
  type KhalaCodeQaMetricName,
  type KhalaCodeQaMetricSample,
  type KhalaCodeQaMetricUnit,
  type KhalaCodeQaMetricsSnapshot,
} from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"
import type { KhalaCodeQaDriver } from "./driver.js"

export const KHALA_CODE_LAG_PROFILING_SWEEP_SCHEMA =
  "openagents.khala_code.lag_profiling_sweep.v1" as const

export type KhalaCodeLagProfilingMode =
  | "fixture"
  | "mode_p_preview_bridge"
  | "mode_d_built_webview"
  | "packaged_electrobun"

export type KhalaCodeLagProfilingWorkload =
  | "startup"
  | "thread_switch"
  | "long_transcript"
  | "fifty_card_cockpit"
  | "streaming_turn"
  | "panel_navigation"
  | "app_server_spawn"
  | "fleet_supervisor"

export type KhalaCodeLagProfilingSnapshotInput = {
  readonly label: string
  readonly mode: KhalaCodeLagProfilingMode
  readonly snapshot: KhalaCodeQaMetricsSnapshot
  readonly workload: readonly KhalaCodeLagProfilingWorkload[]
}

export type KhalaCodeLagProfilingSampleEvidence = {
  readonly context?: Readonly<Record<string, string | number | boolean>>
  readonly label: string
  readonly mode: KhalaCodeLagProfilingMode
  readonly observedAt: string
  readonly unit: KhalaCodeQaMetricUnit
  readonly value: number
  readonly workload: readonly KhalaCodeLagProfilingWorkload[]
}

export type KhalaCodeLagProfilingBudgetRow = {
  readonly budgetId: string
  readonly description: string
  readonly metric: KhalaCodeQaMetricName
  readonly offender: boolean
  readonly p95: number | null
  readonly rank: number | null
  readonly ratio: number | null
  readonly sampleCount: number
  readonly sampleEvidence: readonly KhalaCodeLagProfilingSampleEvidence[]
  readonly status: "pass" | "offender" | "no_samples"
  readonly threshold: number
  readonly unit: KhalaCodeQaMetricBudget["unit"]
  readonly worst: number | null
}

export type KhalaCodeLagProfilingSweepReport = {
  readonly budgets: readonly KhalaCodeLagProfilingBudgetRow[]
  readonly generatedAt: string
  readonly offenderCount: number
  readonly offenders: readonly KhalaCodeLagProfilingBudgetRow[]
  readonly runId: string
  readonly sampleCount: number
  readonly sampledBudgetCount: number
  readonly schema: typeof KHALA_CODE_LAG_PROFILING_SWEEP_SCHEMA
  readonly snapshotCount: number
}

export type KhalaCodeLagProfilingIssueRequest = {
  readonly body: string
  readonly budgetId: string
  readonly labels: readonly string[]
  readonly title: string
}

export type KhalaCodeLagProfilingIssueResult = {
  readonly budgetId: string
  readonly number?: number
  readonly url?: string
}

export type KhalaCodeLagProfilingWriteResult = {
  readonly issueBodyPaths: Readonly<Record<string, string>>
  readonly jsonPath: string
  readonly markdownPath: string
}

const matchesRequiredContext = (
  sample: KhalaCodeQaMetricSample,
  requiredContext: KhalaCodeQaMetricBudget["requiredContext"],
): boolean => {
  if (requiredContext === undefined) return true
  const context = sample.context ?? {}
  return Object.entries(requiredContext).every(([key, value]) => context[key] === value)
}

const percentileValue = (
  values: readonly number[],
  percentile: number,
): number | null => {
  if (values.length === 0) return null
  const sorted = [...values].sort((left, right) => left - right)
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil((percentile / 100) * sorted.length) - 1),
  )
  return sorted[index] ?? null
}

const roundMetric = (value: number): number =>
  Number.isInteger(value) ? value : Number(value.toFixed(2))

const formatMetric = (value: number | null, unit: string): string =>
  value === null ? "n/a" : `${roundMetric(value)}${unit}`

const evidenceForBudget = (
  budget: KhalaCodeQaMetricBudget,
  inputs: readonly KhalaCodeLagProfilingSnapshotInput[],
): readonly KhalaCodeLagProfilingSampleEvidence[] =>
  inputs.flatMap((input) =>
    input.snapshot.samples
      .filter((sample) =>
        sample.metric === budget.metric &&
        sample.unit === budget.unit &&
        matchesRequiredContext(sample, budget.requiredContext)
      )
      .map((sample) => ({
        ...(sample.context === undefined ? {} : { context: sample.context }),
        label: input.label,
        mode: input.mode,
        observedAt: sample.observedAt,
        unit: sample.unit,
        value: sample.value,
        workload: input.workload,
      }))
  )

const rowForBudget = (
  budget: KhalaCodeQaMetricBudget,
  inputs: readonly KhalaCodeLagProfilingSnapshotInput[],
): KhalaCodeLagProfilingBudgetRow => {
  const evidence = evidenceForBudget(budget, inputs)
  const values = evidence.map((sample) => sample.value)
  const p95 = percentileValue(values, 95)
  const worst = values.length === 0 ? null : Math.max(...values)
  const ratio = p95 === null ? null : p95 / budget.threshold
  const offender = p95 !== null && p95 > budget.threshold
  const sampleEvidence = [...evidence]
    .sort((left, right) => right.value - left.value)
    .slice(0, 5)
  return {
    budgetId: budget.budgetId,
    description: budget.description,
    metric: budget.metric,
    offender,
    p95,
    rank: null,
    ratio,
    sampleCount: evidence.length,
    sampleEvidence,
    status: p95 === null ? "no_samples" : offender ? "offender" : "pass",
    threshold: budget.threshold,
    unit: budget.unit,
    worst,
  }
}

export const buildKhalaCodeLagProfilingSweepReport = (input: {
  readonly budgets?: readonly KhalaCodeQaMetricBudget[]
  readonly generatedAt?: string
  readonly runId?: string
  readonly snapshots: readonly KhalaCodeLagProfilingSnapshotInput[]
}): KhalaCodeLagProfilingSweepReport => {
  const budgets = input.budgets ?? khalaCodeQaMetricBudgets
  const baseRows = budgets.map((budget) => rowForBudget(budget, input.snapshots))
  const rankedOffenders = baseRows
    .filter((row) => row.offender)
    .sort((left, right) =>
      (right.ratio ?? 0) - (left.ratio ?? 0) ||
      (right.p95 ?? 0) - (left.p95 ?? 0) ||
      left.budgetId.localeCompare(right.budgetId)
    )
    .map((row, index) => ({ ...row, rank: index + 1 }))
  const offenderRanks = new Map(rankedOffenders.map((row) => [row.budgetId, row.rank]))
  const rows = baseRows.map((row) => ({
    ...row,
    rank: offenderRanks.get(row.budgetId) ?? null,
  }))
  return {
    budgets: rows,
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    offenderCount: rankedOffenders.length,
    offenders: rankedOffenders,
    runId: input.runId ?? `khala-code-lag-sweep-${Date.now()}`,
    sampleCount: rows.reduce((total, row) => total + row.sampleCount, 0),
    sampledBudgetCount: rows.filter((row) => row.sampleCount > 0).length,
    schema: KHALA_CODE_LAG_PROFILING_SWEEP_SCHEMA,
    snapshotCount: input.snapshots.length,
  }
}

export const collectKhalaCodeLagProfilingSnapshot = (input: {
  readonly driver: Pick<KhalaCodeQaDriver, "metrics">
  readonly label: string
  readonly mode: KhalaCodeLagProfilingMode
  readonly workload: readonly KhalaCodeLagProfilingWorkload[]
}): Effect.Effect<KhalaCodeLagProfilingSnapshotInput, unknown> =>
  input.driver.metrics().pipe(
    Effect.map((snapshot) => ({
      label: input.label,
      mode: input.mode,
      snapshot,
      workload: input.workload,
    })),
  )

const slugForBudgetId = (budgetId: string): string =>
  budgetId.replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase()

const contextSummary = (context: KhalaCodeLagProfilingSampleEvidence["context"]): string => {
  if (context === undefined) return ""
  return Object.entries(context)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${String(value)}`)
    .join(", ")
}

const reportMarkdown = (report: KhalaCodeLagProfilingSweepReport): string => {
  const lines = [
    "# Khala Code Lag Profiling Sweep",
    "",
    `Run: \`${report.runId}\``,
    `Generated: ${report.generatedAt}`,
    `Snapshots: ${report.snapshotCount}`,
    `Samples: ${report.sampleCount}`,
    `Sampled budgets: ${report.sampledBudgetCount}/${report.budgets.length}`,
    `Offenders: ${report.offenderCount}`,
    "",
    "## Ranked Offenders",
    "",
  ]
  if (report.offenders.length === 0) {
    lines.push("No p95 offenders were observed in this sweep.", "")
  } else {
    lines.push("| Rank | Budget | Metric | p95 | Threshold | Ratio | Samples |", "| ---: | --- | --- | ---: | ---: | ---: | ---: |")
    for (const offender of report.offenders) {
      lines.push(
        `| ${offender.rank ?? ""} | \`${offender.budgetId}\` | \`${offender.metric}\` | ${formatMetric(offender.p95, offender.unit)} | ${formatMetric(offender.threshold, offender.unit)} | ${roundMetric(offender.ratio ?? 0)}x | ${offender.sampleCount} |`,
      )
    }
    lines.push("")
  }
  lines.push("## Budget Coverage", "")
  lines.push("| Status | Budget | Metric | p95 | Worst | Samples |")
  lines.push("| --- | --- | --- | ---: | ---: | ---: |")
  for (const row of report.budgets) {
    lines.push(
      `| ${row.status} | \`${row.budgetId}\` | \`${row.metric}\` | ${formatMetric(row.p95, row.unit)} | ${formatMetric(row.worst, row.unit)} | ${row.sampleCount} |`,
    )
  }
  return `${lines.join("\n")}\n`
}

const issueBodyForOffender = (
  offender: KhalaCodeLagProfilingBudgetRow,
  parentIssueNumber: number,
): string => {
  const lines = [
    `Parent: #${parentIssueNumber}`,
    "",
    "This child issue was generated by the Khala Code lag profiling sweep.",
    "",
    "## Offender",
    "",
    `- Budget: \`${offender.budgetId}\``,
    `- Metric: \`${offender.metric}\``,
    `- p95: ${formatMetric(offender.p95, offender.unit)}`,
    `- Threshold: ${formatMetric(offender.threshold, offender.unit)}`,
    `- Ratio: ${roundMetric(offender.ratio ?? 0)}x`,
    `- Samples: ${offender.sampleCount}`,
    "",
    "## Sample Evidence",
    "",
    "| Value | Mode | Workload | Observed At | Context |",
    "| ---: | --- | --- | --- | --- |",
  ]
  for (const sample of offender.sampleEvidence) {
    lines.push(
      `| ${formatMetric(sample.value, sample.unit)} | \`${sample.mode}\` | ${sample.workload.map((workload) => `\`${workload}\``).join(", ")} | ${sample.observedAt} | ${contextSummary(sample.context)} |`,
    )
  }
  lines.push(
    "",
    "## Done",
    "",
    "- [ ] Optimize the ranked offender without relaxing the budget.",
    "- [ ] Add or update deterministic regression coverage for the observed class.",
    "- [ ] Re-run the lag profiling sweep and attach the new report evidence.",
  )
  return `${lines.join("\n")}\n`
}

export const createKhalaCodeLagProfilingIssueRequests = (input: {
  readonly parentIssueNumber?: number
  readonly report: KhalaCodeLagProfilingSweepReport
}): readonly KhalaCodeLagProfilingIssueRequest[] => {
  const parentIssueNumber = input.parentIssueNumber ?? 8019
  return input.report.offenders.map((offender) => ({
    body: issueBodyForOffender(offender, parentIssueNumber),
    budgetId: offender.budgetId,
    labels: ["qa", "roadmap"],
    title: `QA lag offender: ${offender.metric} p95 ${formatMetric(offender.p95, offender.unit)} over ${formatMetric(offender.threshold, offender.unit)}`,
  }))
}

export const fileKhalaCodeLagProfilingOffenderIssues = async (input: {
  readonly issueFiler: (request: KhalaCodeLagProfilingIssueRequest) => Promise<KhalaCodeLagProfilingIssueResult>
  readonly parentIssueNumber?: number
  readonly report: KhalaCodeLagProfilingSweepReport
}): Promise<readonly KhalaCodeLagProfilingIssueResult[]> => {
  const requests = createKhalaCodeLagProfilingIssueRequests(input)
  const results: KhalaCodeLagProfilingIssueResult[] = []
  for (const request of requests) {
    results.push(await input.issueFiler(request))
  }
  return results
}

export const writeKhalaCodeLagProfilingSweepReport = async (input: {
  readonly outDir: string
  readonly report: KhalaCodeLagProfilingSweepReport
}): Promise<KhalaCodeLagProfilingWriteResult> => {
  await mkdir(input.outDir, { recursive: true })
  const jsonPath = join(input.outDir, "lag-profiling-sweep-report.json")
  const markdownPath = join(input.outDir, "lag-profiling-sweep-report.md")
  await writeFile(jsonPath, `${JSON.stringify(input.report, null, 2)}\n`)
  await writeFile(markdownPath, reportMarkdown(input.report))

  const issueDir = join(input.outDir, "offender-issues")
  const issueRequests = createKhalaCodeLagProfilingIssueRequests({ report: input.report })
  const issueBodyPaths: Record<string, string> = {}
  if (issueRequests.length > 0) {
    await mkdir(issueDir, { recursive: true })
    for (const request of issueRequests) {
      const path = join(issueDir, `${slugForBudgetId(request.budgetId)}.md`)
      await writeFile(path, request.body)
      issueBodyPaths[request.budgetId] = path
    }
  }
  return { issueBodyPaths, jsonPath, markdownPath }
}

const observedAt = (index: number): string =>
  `2026-07-02T00:${String(index).padStart(2, "0")}:00.000Z`

const sampleForBudget = (
  budget: KhalaCodeQaMetricBudget,
  value: number,
  index: number,
): KhalaCodeQaMetricSample => ({
  context: {
    ...(budget.requiredContext ?? {}),
    fixture: true,
  },
  metric: budget.metric,
  observedAt: observedAt(index),
  unit: budget.unit,
  value,
})

export const buildKhalaCodeLagProfilingFixtureSnapshots = (): readonly KhalaCodeLagProfilingSnapshotInput[] => {
  const offenderMultiplier = new Map<string, readonly number[]>([
    ["budget.khala_code.composer.keystroke_echo.p95.v1", [0.6, 0.8, 1.15, 1.4, 1.9]],
    ["budget.khala_code.thread_switch.full.v1", [0.7, 0.9, 1.05, 1.2, 1.3]],
    ["budget.khala_code.transcript.scroll_dropped_frames.v1", [0.3, 0.6, 1.1, 1.4, 1.8]],
  ])
  const samples = khalaCodeQaMetricBudgets.flatMap((budget, budgetIndex) => {
    const multipliers = offenderMultiplier.get(budget.budgetId) ?? [0.3, 0.4, 0.5, 0.6, 0.7]
    return multipliers.map((multiplier, valueIndex) =>
      sampleForBudget(budget, roundMetric(budget.threshold * multiplier), budgetIndex + valueIndex)
    )
  })
  const snapshot: KhalaCodeQaMetricsSnapshot = {
    budgets: khalaCodeQaMetricBudgets,
    definitions: [],
    evaluations: [],
    ok: true,
    observedAt: "2026-07-02T00:00:00.000Z",
    samples,
    schema: "openagents.khala_code.qa_metrics.v1",
  }
  return [
    {
      label: "fixture-realistic-load",
      mode: "fixture",
      snapshot,
      workload: [
        "startup",
        "thread_switch",
        "long_transcript",
        "fifty_card_cockpit",
        "streaming_turn",
        "panel_navigation",
        "app_server_spawn",
        "fleet_supervisor",
      ],
    },
  ]
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const isQaMetricsSnapshot = (value: unknown): value is KhalaCodeQaMetricsSnapshot =>
  isRecord(value) &&
  value.ok === true &&
  value.schema === "openagents.khala_code.qa_metrics.v1" &&
  Array.isArray(value.budgets) &&
  Array.isArray(value.samples)

const isLagProfilingSnapshotInput = (value: unknown): value is KhalaCodeLagProfilingSnapshotInput =>
  isRecord(value) &&
  typeof value.label === "string" &&
  typeof value.mode === "string" &&
  Array.isArray(value.workload) &&
  isQaMetricsSnapshot(value.snapshot)

export const loadKhalaCodeLagProfilingSnapshotInputs = async (
  path: string,
): Promise<readonly KhalaCodeLagProfilingSnapshotInput[]> => {
  const parsed = JSON.parse(await readFile(path, "utf8")) as unknown
  if (Array.isArray(parsed)) {
    const nested = await Promise.all(parsed.map(async (item, index) => {
      if (isLagProfilingSnapshotInput(item)) return [item]
      if (isQaMetricsSnapshot(item)) {
        return [{
          label: `${basename(path)}#${index + 1}`,
          mode: "mode_p_preview_bridge" as const,
          snapshot: item,
          workload: ["thread_switch"] as const,
        }]
      }
      throw new Error(`Unsupported lag profiling snapshot entry in ${path}`)
    }))
    return nested.flat()
  }
  if (isLagProfilingSnapshotInput(parsed)) return [parsed]
  if (isQaMetricsSnapshot(parsed)) {
    return [{
      label: basename(path),
      mode: "mode_p_preview_bridge",
      snapshot: parsed,
      workload: ["thread_switch"],
    }]
  }
  throw new Error(`Unsupported lag profiling snapshot file: ${path}`)
}

export type KhalaCodeLagProfilingSnapshotFileFailure = {
  readonly path: string
  readonly error: string
}

export type KhalaCodeLagProfilingSnapshotLoadResult = {
  readonly snapshots: readonly KhalaCodeLagProfilingSnapshotInput[]
  readonly failures: readonly KhalaCodeLagProfilingSnapshotFileFailure[]
}

/**
 * Loads every snapshot file, isolating each file's load (Promise.all
 * cron-landmine audit finding #6): one bad or missing file (ENOENT, invalid
 * JSON, an unsupported shape) must not kill visibility into every OTHER
 * file's already-loaded snapshots. Never throws — each failure is reported in
 * `failures` alongside the snapshots successfully loaded from the rest.
 */
export const loadKhalaCodeLagProfilingSnapshotFiles = async (
  snapshotPaths: readonly string[],
  loadOne: (path: string) => Promise<readonly KhalaCodeLagProfilingSnapshotInput[]> = loadKhalaCodeLagProfilingSnapshotInputs,
): Promise<KhalaCodeLagProfilingSnapshotLoadResult> => {
  const outcomes = await Promise.allSettled(snapshotPaths.map((path) => loadOne(path)))
  const snapshots: KhalaCodeLagProfilingSnapshotInput[] = []
  const failures: KhalaCodeLagProfilingSnapshotFileFailure[] = []
  outcomes.forEach((outcome, index) => {
    const path = snapshotPaths[index]!
    if (outcome.status === "fulfilled") {
      snapshots.push(...outcome.value)
    } else {
      failures.push({
        error: outcome.reason instanceof Error ? outcome.reason.message : String(outcome.reason),
        path,
      })
    }
  })
  return { failures, snapshots }
}

const parseArgs = (argv: readonly string[]): {
  readonly fileIssues: boolean
  readonly fixture: boolean
  readonly outDir: string
  readonly repo: string
  readonly snapshotPaths: readonly string[]
} => {
  const snapshotPaths: string[] = []
  let outDir = "artifacts/qa/lag-profiling-sweep"
  let fixture = false
  let fileIssues = false
  let repo = "OpenAgentsInc/openagents"
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === "--fixture") {
      fixture = true
    } else if (arg === "--file-issues") {
      fileIssues = true
    } else if (arg === "--out-dir") {
      outDir = argv[index + 1] ?? outDir
      index += 1
    } else if (arg === "--repo") {
      repo = argv[index + 1] ?? repo
      index += 1
    } else if (arg === "--snapshot") {
      const path = argv[index + 1]
      if (path === undefined) throw new Error("--snapshot requires a path")
      snapshotPaths.push(path)
      index += 1
    } else if (arg === "--") {
      continue
    } else {
      throw new Error(`Unknown lag profiling sweep argument: ${arg}`)
    }
  }
  return {
    fileIssues,
    fixture,
    outDir,
    repo,
    snapshotPaths,
  }
}

const runGhIssueCreate = async (input: {
  readonly bodyFile: string
  readonly labels: readonly string[]
  readonly repo: string
  readonly title: string
}): Promise<string> =>
  await new Promise((resolve, reject) => {
    const args = [
      "issue",
      "create",
      "--repo",
      input.repo,
      "--title",
      input.title,
      "--body-file",
      input.bodyFile,
      ...input.labels.flatMap((label) => ["--label", label]),
    ]
    const child = spawn("gh", args, { stdio: ["ignore", "pipe", "pipe"] })
    let stdout = ""
    let stderr = ""
    child.stdout.setEncoding("utf8")
    child.stderr.setEncoding("utf8")
    child.stdout.on("data", (chunk) => {
      stdout += chunk
    })
    child.stderr.on("data", (chunk) => {
      stderr += chunk
    })
    child.on("error", reject)
    child.on("close", (code) => {
      if (code === 0) {
        resolve(stdout.trim())
      } else {
        reject(new Error(stderr.trim() || `gh issue create exited ${code}`))
      }
    })
  })

const main = async (): Promise<void> => {
  const args = parseArgs(process.argv.slice(2))
  const { failures, snapshots: loadedSnapshots } = await loadKhalaCodeLagProfilingSnapshotFiles(args.snapshotPaths)
  for (const failure of failures) {
    console.error(`Failed to load lag profiling snapshot file ${failure.path}: ${failure.error}`)
  }
  const snapshots = args.fixture || loadedSnapshots.length === 0
    ? [...loadedSnapshots, ...buildKhalaCodeLagProfilingFixtureSnapshots()]
    : loadedSnapshots
  const report = buildKhalaCodeLagProfilingSweepReport({ snapshots })
  const written = await writeKhalaCodeLagProfilingSweepReport({ outDir: args.outDir, report })
  if (args.fileIssues || process.env.OA_QA_LAG_SWEEP_FILE_ISSUES === "1") {
    const requests = createKhalaCodeLagProfilingIssueRequests({ report })
    for (const request of requests) {
      const bodyFile = written.issueBodyPaths[request.budgetId]
      if (bodyFile === undefined) continue
      const url = await runGhIssueCreate({
        bodyFile,
        labels: request.labels,
        repo: args.repo,
        title: request.title,
      })
      console.log(`${request.budgetId}: ${url}`)
    }
  }
  console.log(written.markdownPath)
  // The sweep itself completed against every snapshot file that loaded, but a
  // bad/missing file is still a real problem in an unattended CI run — signal
  // it via a non-zero exit code without throwing, so it never masks (or is
  // masked by) the report already written from the good files.
  if (failures.length > 0) process.exitCode = 1
}

if (import.meta.main) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  })
}
