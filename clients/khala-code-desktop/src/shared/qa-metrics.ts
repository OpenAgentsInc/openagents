export type KhalaCodeQaMetricKind = "counter" | "timer"

export type KhalaCodeQaMetricName =
  | "cache.hit"
  | "cockpit.render_ms"
  | "first_render.ms"
  | "lifecycle_event_to_card.ms"
  | "panel.open_ms"
  | "supervisor.tick_ms"
  | "thread_switch.full_render_ms"
  | "thread_switch.hydrated_render_ms"
  | "thread_switch.optimistic_render_ms"
  | "thread_switch.rpc_ms"
  | "turn_start.latency_ms"

export type KhalaCodeQaMetricSample = {
  readonly context?: Readonly<Record<string, string | number | boolean>>
  readonly metric: KhalaCodeQaMetricName
  readonly observedAt: string
  readonly unit: "count" | "ms"
  readonly value: number
}

export type KhalaCodeQaMetricDefinition = {
  readonly description: string
  readonly kind: KhalaCodeQaMetricKind
  readonly name: KhalaCodeQaMetricName
  readonly unit: "count" | "ms"
}

export type KhalaCodeQaMetricBudget = {
  readonly budgetId: string
  readonly description: string
  readonly metric: KhalaCodeQaMetricName
  readonly operator: "lte"
  readonly percentile?: number
  readonly requiredContext?: Readonly<Record<string, string | number | boolean>>
  readonly threshold: number
  readonly unit: "ms"
}

export type KhalaCodeQaMetricBudgetEvaluation = {
  readonly actual: number | null
  readonly budgetId: string
  readonly metric: KhalaCodeQaMetricName
  readonly ok: boolean
  readonly sampleCount: number
  readonly status: "pass" | "fail" | "inconclusive"
  readonly threshold: number
}

export type KhalaCodeQaMetricsSnapshot = {
  readonly budgets: readonly KhalaCodeQaMetricBudget[]
  readonly definitions: readonly KhalaCodeQaMetricDefinition[]
  readonly evaluations: readonly KhalaCodeQaMetricBudgetEvaluation[]
  readonly ok: true
  readonly observedAt: string
  readonly samples: readonly KhalaCodeQaMetricSample[]
  readonly schema: "openagents.khala_code.qa_metrics.v1"
}

export const khalaCodeQaMetricDefinitions: readonly KhalaCodeQaMetricDefinition[] = [
  {
    description: "Thread switch click to optimistic render duration.",
    kind: "timer",
    name: "thread_switch.optimistic_render_ms",
    unit: "ms",
  },
  {
    description: "Thread switch RPC round trip duration.",
    kind: "timer",
    name: "thread_switch.rpc_ms",
    unit: "ms",
  },
  {
    description: "Thread switch full history render duration.",
    kind: "timer",
    name: "thread_switch.full_render_ms",
    unit: "ms",
  },
  {
    description: "Thread switch hydrated render duration.",
    kind: "timer",
    name: "thread_switch.hydrated_render_ms",
    unit: "ms",
  },
  {
    description: "Chat turn start request latency.",
    kind: "timer",
    name: "turn_start.latency_ms",
    unit: "ms",
  },
  {
    description: "First visible render mark for a scenario surface.",
    kind: "timer",
    name: "first_render.ms",
    unit: "ms",
  },
  {
    description: "Panel open request to visible mark duration.",
    kind: "timer",
    name: "panel.open_ms",
    unit: "ms",
  },
  {
    description: "Cache-hit count recorded by the app.",
    kind: "counter",
    name: "cache.hit",
    unit: "count",
  },
  {
    description: "Fleet cockpit render duration.",
    kind: "timer",
    name: "cockpit.render_ms",
    unit: "ms",
  },
  {
    description: "Fleet lifecycle event to visible worker-card duration.",
    kind: "timer",
    name: "lifecycle_event_to_card.ms",
    unit: "ms",
  },
  {
    description: "Fleet-run supervisor tick duration.",
    kind: "timer",
    name: "supervisor.tick_ms",
    unit: "ms",
  },
]

export const khalaCodeQaMetricBudgets: readonly KhalaCodeQaMetricBudget[] = [
  {
    budgetId: "budget.khala_code.cockpit_render.50_cards.v1",
    description: "Cockpit renders within 100ms with 50 worker cards.",
    metric: "cockpit.render_ms",
    operator: "lte",
    requiredContext: { cards: 50 },
    threshold: 100,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.lifecycle_event_to_card.p95.v1",
    description: "Lifecycle event to worker card p95 stays below 500ms.",
    metric: "lifecycle_event_to_card.ms",
    operator: "lte",
    percentile: 95,
    threshold: 500,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.supervisor_tick.25_target.v1",
    description: "Supervisor tick stays below 1s at target 25.",
    metric: "supervisor.tick_ms",
    operator: "lte",
    requiredContext: { target: 25 },
    threshold: 1_000,
    unit: "ms",
  },
]

const matchesContext = (
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

export const evaluateKhalaCodeQaMetricBudget = (
  budget: KhalaCodeQaMetricBudget,
  samples: readonly KhalaCodeQaMetricSample[],
): KhalaCodeQaMetricBudgetEvaluation => {
  const matchingSamples = samples.filter((sample) =>
    sample.metric === budget.metric && matchesContext(sample, budget.requiredContext)
  )
  const values = matchingSamples.map((sample) => sample.value)
  const actual = budget.percentile === undefined
    ? values.at(-1) ?? null
    : percentileValue(values, budget.percentile)
  const ok = actual !== null && actual <= budget.threshold
  return {
    actual,
    budgetId: budget.budgetId,
    metric: budget.metric,
    ok,
    sampleCount: matchingSamples.length,
    status: actual === null ? "inconclusive" : ok ? "pass" : "fail",
    threshold: budget.threshold,
  }
}

export const evaluateKhalaCodeQaMetricBudgets = (
  samples: readonly KhalaCodeQaMetricSample[],
  budgets: readonly KhalaCodeQaMetricBudget[] = khalaCodeQaMetricBudgets,
): readonly KhalaCodeQaMetricBudgetEvaluation[] =>
  budgets.map((budget) => evaluateKhalaCodeQaMetricBudget(budget, samples))

export const emptyKhalaCodeQaMetricsSnapshot = (
  observedAt = new Date().toISOString(),
): KhalaCodeQaMetricsSnapshot => ({
  budgets: khalaCodeQaMetricBudgets,
  definitions: khalaCodeQaMetricDefinitions,
  evaluations: evaluateKhalaCodeQaMetricBudgets([]),
  ok: true,
  observedAt,
  samples: [],
  schema: "openagents.khala_code.qa_metrics.v1",
})
