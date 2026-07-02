export type KhalaCodeQaMetricKind = "counter" | "gauge" | "timer"
export type KhalaCodeQaMetricUnit = "count" | "ms" | "percent"
export type KhalaCodeQaMetricBudgetUnit = "ms" | "percent"

export type KhalaCodeQaMetricName =
  | "app_server.spawn_ready_ms"
  | "cache.hit"
  | "cockpit.render_ms"
  | "composer.keystroke_echo_ms"
  | "first_render.ms"
  | "lifecycle_event_to_card.ms"
  | "panel.open_ms"
  | "sse.event_to_ui_ms"
  | "startup.interactive_ms"
  | "supervisor.tick_ms"
  | "thread_switch.full_render_ms"
  | "thread_switch.hydrated_render_ms"
  | "thread_switch.optimistic_render_ms"
  | "thread_switch.rpc_ms"
  | "transcript.scroll_dropped_frames_pct"
  | "turn_start.first_event_ms"
  | "turn_start.latency_ms"

export type KhalaCodeQaMetricSample = {
  readonly context?: Readonly<Record<string, string | number | boolean>>
  readonly metric: KhalaCodeQaMetricName
  readonly observedAt: string
  readonly unit: KhalaCodeQaMetricUnit
  readonly value: number
}

export type KhalaCodeQaMetricDefinition = {
  readonly description: string
  readonly kind: KhalaCodeQaMetricKind
  readonly name: KhalaCodeQaMetricName
  readonly unit: KhalaCodeQaMetricUnit
}

export type KhalaCodeQaMetricBudget = {
  readonly budgetId: string
  readonly description: string
  readonly metric: KhalaCodeQaMetricName
  readonly operator: "lte"
  readonly percentile?: number
  readonly requiredContext?: Readonly<Record<string, string | number | boolean>>
  readonly threshold: number
  readonly unit: KhalaCodeQaMetricBudgetUnit
}

export type KhalaCodeQaMetricBudgetEvaluation = {
  readonly actual: number | null
  readonly budgetId: string
  readonly metric: KhalaCodeQaMetricName
  readonly ok: boolean
  readonly sampleCount: number
  readonly status: "pass" | "fail" | "inconclusive"
  readonly threshold: number
  readonly unit: KhalaCodeQaMetricBudgetUnit
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
    description: "Cold boot to first interactive app surface duration.",
    kind: "timer",
    name: "startup.interactive_ms",
    unit: "ms",
  },
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
    description: "Chat turn request to first visible response event duration.",
    kind: "timer",
    name: "turn_start.first_event_ms",
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
    description: "Composer keystroke to echoed text paint duration.",
    kind: "timer",
    name: "composer.keystroke_echo_ms",
    unit: "ms",
  },
  {
    description: "Server-sent event receipt to visible UI update duration.",
    kind: "timer",
    name: "sse.event_to_ui_ms",
    unit: "ms",
  },
  {
    description: "Dropped-frame percentage while scrolling a transcript.",
    kind: "gauge",
    name: "transcript.scroll_dropped_frames_pct",
    unit: "percent",
  },
  {
    description: "App-server spawn to ready-state duration.",
    kind: "timer",
    name: "app_server.spawn_ready_ms",
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
    budgetId: "budget.khala_code.startup_interactive.v1",
    description: "Cold startup reaches an interactive app surface within 3s.",
    metric: "startup.interactive_ms",
    operator: "lte",
    threshold: 3_000,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.thread_switch.optimistic.v1",
    description: "Thread switch optimistic paint stays below 100ms.",
    metric: "thread_switch.optimistic_render_ms",
    operator: "lte",
    threshold: 100,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.thread_switch.full.v1",
    description: "Thread switch full-history render stays below 400ms.",
    metric: "thread_switch.full_render_ms",
    operator: "lte",
    threshold: 400,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.turn_start.first_event.v1",
    description: "Chat turn start reaches the first visible response event within 400ms.",
    metric: "turn_start.first_event_ms",
    operator: "lte",
    threshold: 400,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.composer.keystroke_echo.p95.v1",
    description: "Composer keystroke echo p95 stays below one 60Hz frame.",
    metric: "composer.keystroke_echo_ms",
    operator: "lte",
    percentile: 95,
    threshold: 16,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.panel.open.v1",
    description: "Hotbar panel open to visible mark stays below 150ms.",
    metric: "panel.open_ms",
    operator: "lte",
    threshold: 150,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.sse.event_to_ui.p95.v1",
    description: "SSE event receipt to UI update p95 stays below 250ms.",
    metric: "sse.event_to_ui_ms",
    operator: "lte",
    percentile: 95,
    threshold: 250,
    unit: "ms",
  },
  {
    budgetId: "budget.khala_code.transcript.scroll_dropped_frames.v1",
    description: "Transcript scroll drops fewer than 5% of frames.",
    metric: "transcript.scroll_dropped_frames_pct",
    operator: "lte",
    threshold: 5,
    unit: "percent",
  },
  {
    budgetId: "budget.khala_code.app_server.spawn_ready.v1",
    description: "App-server spawn reaches ready state within 2s.",
    metric: "app_server.spawn_ready_ms",
    operator: "lte",
    threshold: 2_000,
    unit: "ms",
  },
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

export const khalaCodeQaMetricUnitFor = (
  metric: KhalaCodeQaMetricName,
): KhalaCodeQaMetricUnit =>
  khalaCodeQaMetricDefinitions.find((definition) => definition.name === metric)?.unit ?? "ms"

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
    unit: budget.unit,
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
