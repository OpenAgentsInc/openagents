export type PerformanceSummary = {
  readonly count: number
  readonly p50: number
  readonly p95: number
  readonly p99: number
  readonly max: number
}

export type PerformanceBudget = {
  readonly p95Max?: number
}

export type PerformanceBudgetCheck = {
  readonly ok: boolean
  readonly breaches: readonly string[]
}

export function percentile(samples: readonly number[], p: number): number {
  if (samples.length === 0) {
    return 0
  }

  const sorted = [...samples].sort((left, right) => left - right)
  const boundedPercentile = Math.min(100, Math.max(0, p))
  const rank = (boundedPercentile / 100) * (sorted.length - 1)
  const lowerIndex = Math.floor(rank)
  const upperIndex = Math.ceil(rank)
  const lower = sorted[lowerIndex] ?? 0
  const upper = sorted[upperIndex] ?? lower

  if (lowerIndex === upperIndex) {
    return lower
  }

  return lower + (upper - lower) * (rank - lowerIndex)
}

export function summarize(samples: readonly number[]): PerformanceSummary {
  if (samples.length === 0) {
    return {
      count: 0,
      p50: 0,
      p95: 0,
      p99: 0,
      max: 0,
    }
  }

  return {
    count: samples.length,
    p50: percentile(samples, 50),
    p95: percentile(samples, 95),
    p99: percentile(samples, 99),
    max: Math.max(...samples),
  }
}

export function checkBudget(
  summary: PerformanceSummary,
  budget: PerformanceBudget,
): PerformanceBudgetCheck {
  const breaches: string[] = []

  if (budget.p95Max !== undefined && summary.p95 > budget.p95Max) {
    breaches.push(`p95 ${summary.p95} exceeds budget ${budget.p95Max}`)
  }

  return {
    ok: breaches.length === 0,
    breaches,
  }
}
