import { describe, expect, test } from "bun:test"

import {
  evaluateKhalaCodeQaMetricBudgets,
  khalaCodeQaMetricBudgets,
  khalaCodeQaMetricDefinitions,
  khalaCodeQaMetricUnitFor,
  type KhalaCodeQaMetricBudget,
  type KhalaCodeQaMetricSample,
} from "../src/shared/qa-metrics"

const q2BudgetExpectations = [
  ["budget.khala_code.startup_interactive.v1", "startup.interactive_ms", 3_000, "ms", undefined],
  ["budget.khala_code.thread_switch.optimistic.v1", "thread_switch.optimistic_render_ms", 100, "ms", undefined],
  ["budget.khala_code.thread_switch.full.v1", "thread_switch.full_render_ms", 400, "ms", undefined],
  ["budget.khala_code.turn_start.first_event.v1", "turn_start.first_event_ms", 400, "ms", undefined],
  ["budget.khala_code.composer.keystroke_echo.p95.v1", "composer.keystroke_echo_ms", 16, "ms", 95],
  ["budget.khala_code.panel.open.v1", "panel.open_ms", 150, "ms", undefined],
  ["budget.khala_code.sse.event_to_ui.p95.v1", "sse.event_to_ui_ms", 250, "ms", 95],
  ["budget.khala_code.transcript.scroll_dropped_frames.v1", "transcript.scroll_dropped_frames_pct", 5, "percent", undefined],
  ["budget.khala_code.app_server.spawn_ready.v1", "app_server.spawn_ready_ms", 2_000, "ms", undefined],
] as const

const sampleForBudget = (budget: KhalaCodeQaMetricBudget): KhalaCodeQaMetricSample => ({
  ...(budget.requiredContext === undefined ? {} : { context: budget.requiredContext }),
  metric: budget.metric,
  observedAt: "2026-07-02T12:00:00.000Z",
  unit: khalaCodeQaMetricUnitFor(budget.metric),
  value: budget.threshold,
})

describe("Khala Code QA metric budget catalog", () => {
  test("contains the full Q2.2 latency budget family as data", () => {
    const budgetsById = new Map(khalaCodeQaMetricBudgets.map(budget => [budget.budgetId, budget]))
    const definitionsByName = new Map(khalaCodeQaMetricDefinitions.map(definition => [definition.name, definition]))

    expect(new Set(khalaCodeQaMetricBudgets.map(budget => budget.budgetId)).size)
      .toBe(khalaCodeQaMetricBudgets.length)
    expect(khalaCodeQaMetricBudgets.map(budget => budget.budgetId)).toEqual([
      ...q2BudgetExpectations.map(([budgetId]) => budgetId),
      "budget.khala_code.cockpit_render.50_cards.v1",
      "budget.khala_code.lifecycle_event_to_card.p95.v1",
      "budget.khala_code.supervisor_tick.25_target.v1",
    ])

    for (const [budgetId, metric, threshold, unit, percentile] of q2BudgetExpectations) {
      const budget = budgetsById.get(budgetId)
      expect(budget).toMatchObject({
        budgetId,
        metric,
        operator: "lte",
        threshold,
        unit,
        ...(percentile === undefined ? {} : { percentile }),
      })
      expect(definitionsByName.get(metric)?.unit).toBe(unit)
    }

    for (const budget of khalaCodeQaMetricBudgets) {
      expect(definitionsByName.has(budget.metric)).toBe(true)
      expect(definitionsByName.get(budget.metric)?.unit).toBe(budget.unit)
    }
  })

  test("evaluates every budget with the matching metric unit", () => {
    const samples = khalaCodeQaMetricBudgets.map(sampleForBudget)
    const evaluations = evaluateKhalaCodeQaMetricBudgets(samples)

    expect(evaluations).toHaveLength(khalaCodeQaMetricBudgets.length)
    for (const budget of khalaCodeQaMetricBudgets) {
      expect(evaluations.find(evaluation => evaluation.budgetId === budget.budgetId))
        .toMatchObject({
          actual: budget.threshold,
          metric: budget.metric,
          ok: true,
          sampleCount: 1,
          status: "pass",
          threshold: budget.threshold,
          unit: budget.unit,
        })
    }
  })

  test("refutes percent budgets with percent units instead of milliseconds", () => {
    const budget = khalaCodeQaMetricBudgets.find(candidate =>
      candidate.budgetId === "budget.khala_code.transcript.scroll_dropped_frames.v1"
    )
    if (budget === undefined) throw new Error("missing transcript dropped-frame budget")

    const [evaluation] = evaluateKhalaCodeQaMetricBudgets([
      {
        metric: "transcript.scroll_dropped_frames_pct",
        observedAt: "2026-07-02T12:00:00.000Z",
        unit: "percent",
        value: 5.1,
      },
    ], [budget])

    expect(evaluation).toMatchObject({
      actual: 5.1,
      ok: false,
      status: "fail",
      threshold: 5,
      unit: "percent",
    })
  })
})
