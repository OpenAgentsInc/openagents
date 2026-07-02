export const KHALA_CODE_QA_MEMORY_ORACLE_SCHEMA =
  "khala_code_qa_memory_oracle.v1" as const
export const KHALA_CODE_QA_SHUTDOWN_ORACLE_SCHEMA =
  "khala_code_qa_shutdown_oracle.v1" as const
export const KHALA_CODE_QA_SHUTDOWN_ORACLE_SUMMARY_SCHEMA =
  "khala_code_qa_shutdown_oracle_summary.v1" as const
export const KHALA_CODE_QA_RSS_MONOTONIC_GROWTH_MIN_DELTA_BYTES = 64 * 1024 * 1024

export type KhalaCodeQaMemoryPhase =
  | "before_monkey_run"
  | "after_monkey_run"
  | "after_monkey_night"

export type KhalaCodeQaMemorySample = {
  readonly arrayBuffersBytes?: number | undefined
  readonly externalBytes?: number | undefined
  readonly heapTotalBytes: number
  readonly heapUsedBytes: number
  readonly observedAt: string
  readonly phase: KhalaCodeQaMemoryPhase
  readonly rssBytes: number
  readonly runIndex?: number | undefined
  readonly seed?: string | undefined
}

export type KhalaCodeQaMemoryBudget = {
  readonly budgetId: string
  readonly description: string
  readonly metric: "rss_after_bytes" | "js_heap_after_bytes"
  readonly thresholdBytes: number
}

export type KhalaCodeQaMemoryBudgetEvaluation = {
  readonly actualBytes: number | null
  readonly budgetId: string
  readonly description: string
  readonly metric: KhalaCodeQaMemoryBudget["metric"]
  readonly sampleCount: number
  readonly status: "pass" | "fail" | "no_samples"
  readonly thresholdBytes: number
}

export type KhalaCodeQaMemoryTrendEvaluation = {
  readonly budgetId: "memory.rss_monotonic_growth_after_monkey_night.v1"
  readonly description: string
  readonly growthBytes: number | null
  readonly minimumGrowthBytes: number
  readonly rssAfterRunBytes: readonly number[]
  readonly sampleCount: number
  readonly status: "pass" | "fail" | "no_samples"
}

export type KhalaCodeQaMemoryOracleReport = {
  readonly schema: typeof KHALA_CODE_QA_MEMORY_ORACLE_SCHEMA
  readonly budgetEvaluations: readonly KhalaCodeQaMemoryBudgetEvaluation[]
  readonly generatedAt: string
  readonly samples: readonly KhalaCodeQaMemorySample[]
  readonly status: "pass" | "fail"
  readonly trendEvaluation: KhalaCodeQaMemoryTrendEvaluation
}

export type KhalaCodeQaOrphanProcess = {
  readonly command?: string | undefined
  readonly parentPid?: number | undefined
  readonly pid: number
  readonly reason?: string | undefined
}

export type KhalaCodeQaShutdownOracle = {
  readonly schema: typeof KHALA_CODE_QA_SHUTDOWN_ORACLE_SCHEMA
  readonly actualOrphans: number
  readonly budgetId: "process.orphan_after_driver_shutdown.v1"
  readonly observedAt: string
  readonly orphanProcesses: readonly KhalaCodeQaOrphanProcess[]
  readonly status: "pass" | "fail"
  readonly thresholdOrphans: 0
}

export type KhalaCodeQaShutdownOracleSummary = {
  readonly schema: typeof KHALA_CODE_QA_SHUTDOWN_ORACLE_SUMMARY_SCHEMA
  readonly actualOrphans: number
  readonly checkedShutdowns: number
  readonly failedShutdowns: number
  readonly orphanProcesses: readonly KhalaCodeQaOrphanProcess[]
  readonly status: "pass" | "fail"
}

export const khalaCodeQaMemoryBudgets: readonly KhalaCodeQaMemoryBudget[] = [
  {
    budgetId: "memory.rss_after_monkey_night.v1",
    description: "RSS after the seeded monkey night stays below 1.5 GB.",
    metric: "rss_after_bytes",
    thresholdBytes: 1_500_000_000,
  },
  {
    budgetId: "memory.js_heap_after_monkey_night.v1",
    description: "JS heap used after the seeded monkey night stays below 512 MiB.",
    metric: "js_heap_after_bytes",
    thresholdBytes: 512 * 1024 * 1024,
  },
]

const afterSamples = (
  samples: readonly KhalaCodeQaMemorySample[],
): readonly KhalaCodeQaMemorySample[] =>
  samples.filter((sample) =>
    sample.phase === "after_monkey_run" ||
    sample.phase === "after_monkey_night"
  )

const valueForBudget = (
  sample: KhalaCodeQaMemorySample,
  budget: KhalaCodeQaMemoryBudget,
): number =>
  budget.metric === "rss_after_bytes" ? sample.rssBytes : sample.heapUsedBytes

const evaluateBudget = (
  budget: KhalaCodeQaMemoryBudget,
  samples: readonly KhalaCodeQaMemorySample[],
): KhalaCodeQaMemoryBudgetEvaluation => {
  const matchingSamples = afterSamples(samples)
  const actualBytes = matchingSamples.at(-1) === undefined
    ? null
    : valueForBudget(matchingSamples.at(-1) as KhalaCodeQaMemorySample, budget)
  const status = actualBytes === null
    ? "no_samples"
    : actualBytes <= budget.thresholdBytes
      ? "pass"
      : "fail"
  return {
    actualBytes,
    budgetId: budget.budgetId,
    description: budget.description,
    metric: budget.metric,
    sampleCount: matchingSamples.length,
    status,
    thresholdBytes: budget.thresholdBytes,
  }
}

const strictlyIncreases = (values: readonly number[]): boolean =>
  values.length >= 3 && values.every((value, index) =>
    index === 0 || value > (values[index - 1] ?? value)
  )

const evaluateTrend = (
  samples: readonly KhalaCodeQaMemorySample[],
): KhalaCodeQaMemoryTrendEvaluation => {
  const rssAfterRunBytes = samples
    .filter((sample) => sample.phase === "after_monkey_run")
    .sort((left, right) => (left.runIndex ?? 0) - (right.runIndex ?? 0))
    .map((sample) => sample.rssBytes)
  const growthBytes = rssAfterRunBytes.length < 2
    ? null
    : (rssAfterRunBytes.at(-1) ?? 0) - (rssAfterRunBytes[0] ?? 0)
  const monotonicLeak = strictlyIncreases(rssAfterRunBytes) &&
    growthBytes !== null &&
    growthBytes >= KHALA_CODE_QA_RSS_MONOTONIC_GROWTH_MIN_DELTA_BYTES
  const status = rssAfterRunBytes.length === 0
    ? "no_samples"
    : monotonicLeak
      ? "fail"
      : "pass"
  return {
    budgetId: "memory.rss_monotonic_growth_after_monkey_night.v1",
    description: "RSS after-run samples must not increase monotonically across the monkey night by 64 MiB or more.",
    growthBytes,
    minimumGrowthBytes: KHALA_CODE_QA_RSS_MONOTONIC_GROWTH_MIN_DELTA_BYTES,
    rssAfterRunBytes,
    sampleCount: rssAfterRunBytes.length,
    status,
  }
}

export const evaluateKhalaCodeQaMemoryOracle = (input: {
  readonly budgets?: readonly KhalaCodeQaMemoryBudget[]
  readonly generatedAt: string
  readonly samples: readonly KhalaCodeQaMemorySample[]
}): KhalaCodeQaMemoryOracleReport => {
  const budgetEvaluations = (input.budgets ?? khalaCodeQaMemoryBudgets)
    .map((budget) => evaluateBudget(budget, input.samples))
  const trendEvaluation = evaluateTrend(input.samples)
  const failed = budgetEvaluations.some((evaluation) => evaluation.status === "fail") ||
    trendEvaluation.status === "fail"
  return {
    budgetEvaluations,
    generatedAt: input.generatedAt,
    samples: input.samples,
    schema: KHALA_CODE_QA_MEMORY_ORACLE_SCHEMA,
    status: failed ? "fail" : "pass",
    trendEvaluation,
  }
}

export const sampleKhalaCodeQaMemory = (input: {
  readonly now?: () => string
  readonly phase: KhalaCodeQaMemoryPhase
  readonly runIndex?: number | undefined
  readonly seed?: string | undefined
}): KhalaCodeQaMemorySample => {
  const usage = process.memoryUsage()
  return {
    arrayBuffersBytes: usage.arrayBuffers,
    externalBytes: usage.external,
    heapTotalBytes: usage.heapTotal,
    heapUsedBytes: usage.heapUsed,
    observedAt: (input.now ?? (() => new Date().toISOString()))(),
    phase: input.phase,
    rssBytes: usage.rss,
    ...(input.runIndex === undefined ? {} : { runIndex: input.runIndex }),
    ...(input.seed === undefined ? {} : { seed: input.seed }),
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const isOrphanProcess = (value: unknown): value is KhalaCodeQaOrphanProcess =>
  isRecord(value) &&
  typeof value.pid === "number" &&
  (value.parentPid === undefined || typeof value.parentPid === "number") &&
  (value.command === undefined || typeof value.command === "string") &&
  (value.reason === undefined || typeof value.reason === "string")

const orphansFromUnknown = (value: unknown): readonly KhalaCodeQaOrphanProcess[] => {
  if (!isRecord(value)) return []
  const direct = value.orphanProcesses
  if (Array.isArray(direct)) return direct.filter(isOrphanProcess)
  return []
}

export const buildKhalaCodeQaShutdownOracle = (input: {
  readonly observedAt: string
  readonly orphanProcesses?: readonly KhalaCodeQaOrphanProcess[] | undefined
}): KhalaCodeQaShutdownOracle => {
  const orphanProcesses = input.orphanProcesses ?? []
  return {
    actualOrphans: orphanProcesses.length,
    budgetId: "process.orphan_after_driver_shutdown.v1",
    observedAt: input.observedAt,
    orphanProcesses,
    schema: KHALA_CODE_QA_SHUTDOWN_ORACLE_SCHEMA,
    status: orphanProcesses.length === 0 ? "pass" : "fail",
    thresholdOrphans: 0,
  }
}

export const evaluateKhalaCodeQaShutdownOracle = (input: {
  readonly artifacts?: { readonly shutdownOracle?: KhalaCodeQaShutdownOracle; readonly summary?: unknown } | undefined
  readonly observedAt: string
}): KhalaCodeQaShutdownOracle => {
  if (input.artifacts?.shutdownOracle !== undefined) return input.artifacts.shutdownOracle
  return buildKhalaCodeQaShutdownOracle({
    observedAt: input.observedAt,
    orphanProcesses: orphansFromUnknown(input.artifacts?.summary),
  })
}

export const summarizeKhalaCodeQaShutdownOracles = (
  oracles: readonly KhalaCodeQaShutdownOracle[],
): KhalaCodeQaShutdownOracleSummary => {
  const orphanProcesses = oracles.flatMap((oracle) => oracle.orphanProcesses)
  return {
    actualOrphans: orphanProcesses.length,
    checkedShutdowns: oracles.length,
    failedShutdowns: oracles.filter((oracle) => oracle.status === "fail").length,
    orphanProcesses,
    schema: KHALA_CODE_QA_SHUTDOWN_ORACLE_SUMMARY_SCHEMA,
    status: orphanProcesses.length === 0 ? "pass" : "fail",
  }
}
