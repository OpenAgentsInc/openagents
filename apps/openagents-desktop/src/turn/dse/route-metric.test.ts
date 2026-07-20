import { describe, expect, test } from "vite-plus/test"

import { rewardBundle, type MetricComponent, type TurnRouteOutput } from "@openagentsinc/dse"

import { REQUIRED_ROUTE_METRIC_COMPONENTS, turnRouteMetric } from "./route-metric.ts"

/**
 * AFS-09 route-metric coverage and two-sided behavior. The metric must score
 * every required dimension and must penalize BOTH a false delegation and a
 * false refusal — the two hand-observed failure modes.
 */

const answerLocal: TurnRouteOutput = { decision: "answer_local", candidate: null, taskSummary: null, claimedActions: [] }
const delegateCodex: TurnRouteOutput = {
  decision: "delegate",
  candidate: "codex",
  taskSummary: "Hand off the requested task to codex.",
  claimedActions: [],
}

const componentByName = (components: ReadonlyArray<MetricComponent>, name: string): number => {
  const found = components.find((component) => component.name === name)
  if (found === undefined) throw new Error(`missing metric component ${name}`)
  return found.value
}

const score = (expected: TurnRouteOutput, actual: TurnRouteOutput): number =>
  rewardBundle(turnRouteMetric.score({ expected, actual, formatValid: true })).score

describe("AFS-09 route metric coverage", () => {
  test("every required route dimension is scored by name", () => {
    const components = turnRouteMetric.score({ expected: delegateCodex, actual: delegateCodex, formatValid: true })
    const names = new Set(components.map((component) => component.name))
    for (const required of REQUIRED_ROUTE_METRIC_COMPONENTS) {
      expect(names.has(required)).toBe(true)
    }
  })
})

describe("AFS-09 route metric penalizes both failure modes", () => {
  test("a correct delegation beats a false refusal (answering locally when work needed a provider)", () => {
    const falseRefusal = turnRouteMetric.score({ expected: delegateCodex, actual: answerLocal, formatValid: true })
    expect(componentByName(falseRefusal, "false_local_answer_for_provider_work")).toBe(0)
    expect(score(delegateCodex, delegateCodex)).toBeGreaterThan(score(delegateCodex, answerLocal))
  })

  test("a correct local answer beats a false delegation (needless provider recommendation)", () => {
    const falseDelegation = turnRouteMetric.score({ expected: answerLocal, actual: delegateCodex, formatValid: true })
    expect(componentByName(falseDelegation, "needless_provider_recommendation")).toBe(0)
    expect(score(answerLocal, answerLocal)).toBeGreaterThan(score(answerLocal, delegateCodex))
  })

  test("recommending a candidate other than the allowed one is a disallowed-provider penalty", () => {
    const delegateGrok: TurnRouteOutput = { ...delegateCodex, candidate: "grok_acp", taskSummary: "Hand off to grok." }
    const disallowed = turnRouteMetric.score({ expected: delegateCodex, actual: delegateGrok, formatValid: true })
    expect(componentByName(disallowed, "unavailable_or_disallowed_provider")).toBe(0)
    expect(componentByName(disallowed, "data_destination_cost_policy")).toBe(0)
  })

  test("an action claim tanks safety and cannot be bought back by resource savings", () => {
    const unsafe: TurnRouteOutput = { ...delegateCodex, claimedActions: ["ran_command"] }
    const components = turnRouteMetric.score({ expected: delegateCodex, actual: unsafe, formatValid: true })
    expect(componentByName(components, "unsafe_action_claim")).toBe(0)
    expect(score(delegateCodex, delegateCodex)).toBeGreaterThan(score(delegateCodex, unsafe))
  })

  test("a decode failure fails closed (every quality component scores zero)", () => {
    const components = turnRouteMetric.score({ expected: delegateCodex, actual: null, formatValid: false })
    for (const component of components) {
      if (component.kind === "quality") expect(component.value).toBe(0)
    }
  })
})
