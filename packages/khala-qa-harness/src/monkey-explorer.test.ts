import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  buildKhalaCodeQaSeededMonkeyPlan,
  khalaCodeQaMonkeyEnabledActionSpace,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  replayKhalaCodeQaSeededMonkeyPlan,
  runKhalaCodeQaSeededMonkey,
} from "./index.js"

const makeFixtureDriver = () =>
  makeKhalaCodeRpcQaDriver({
    baseUrl: "http://fixture.local",
    fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
    now: () => "2026-07-01T00:00:00.000Z",
  })

describe("Khala Code QA seeded monkey explorer", () => {
  test("builds a deterministic action log that replays exactly", () => {
    const first = buildKhalaCodeQaSeededMonkeyPlan({
      mode: "fixture_smoke",
      seed: "t6.8-smoke",
      steps: 24,
    })
    const second = buildKhalaCodeQaSeededMonkeyPlan({
      mode: "fixture_smoke",
      seed: "t6.8-smoke",
      steps: 24,
    })
    const different = buildKhalaCodeQaSeededMonkeyPlan({
      mode: "fixture_smoke",
      seed: "t6.8-other",
      steps: 24,
    })

    expect(first.actionLog).toEqual(second.actionLog)
    expect(first.actionLog).not.toEqual(different.actionLog)
    expect(first.scenario.phases[0]?.act).toEqual(first.actionLog.map((entry) => entry.action))
  })

  test("exposes the enabled action space categories required by T6.8", () => {
    const actions = khalaCodeQaMonkeyEnabledActionSpace("fleet_cockpit_night")

    expect(actions.some((action) => action.kind === "click" && action.target?.includes("data-"))).toBe(true)
    expect(actions.some((action) => action.kind === "slash_command")).toBe(true)
    expect(actions.some((action) => action.kind === "hotbar" && action.target === "fleet")).toBe(true)
    expect(actions.some((action) => action.kind === "type" && action.target === "composer")).toBe(true)
    expect(actions.some((action) => action.kind === "rpc_call" && action.method === "fleetRunStart")).toBe(true)
  })

  test("runs the bounded fixture smoke and emits a coverage ledger", async () => {
    const report = await Effect.runPromise(
      runKhalaCodeQaSeededMonkey({
        driver: makeFixtureDriver(),
        options: {
          mode: "fixture_smoke",
          seed: "t6.8-smoke",
          steps: 32,
        },
      }),
    )

    expect(report.schema).toBe("khala_code_qa_seeded_monkey_report.v1")
    expect(report.status).toBe("pass")
    expect(report.actionLog).toHaveLength(32)
    expect(report.coverageLedger.schema).toBe("khala_code_qa_coverage_ledger.v1")
    expect(report.coverageLedger.runIds).toEqual(["scenario.khala_code.monkey.fixture_smoke.t6.8-smoke.v1"])
  })

  test("replays a recorded action log exactly on the fixture backend", async () => {
    const plan = buildKhalaCodeQaSeededMonkeyPlan({
      mode: "fixture_smoke",
      seed: "t6.8-replay",
      steps: 20,
    })
    const first = await Effect.runPromise(
      replayKhalaCodeQaSeededMonkeyPlan({ driver: makeFixtureDriver(), plan }),
    )
    const second = await Effect.runPromise(
      replayKhalaCodeQaSeededMonkeyPlan({ driver: makeFixtureDriver(), plan }),
    )

    expect(first.status).toBe("pass")
    expect(first.actionLog).toEqual(second.actionLog)
    expect(first.coverageLedger.rpcMethods).toEqual(second.coverageLedger.rpcMethods)
    expect(first.coverageLedger.slashCommands).toEqual(second.coverageLedger.slashCommands)
  })

  test("fleet cockpit night entry attaches the claim-invariant oracle", async () => {
    const report = await Effect.runPromise(
      runKhalaCodeQaSeededMonkey({
        driver: makeFixtureDriver(),
        options: {
          mode: "fleet_cockpit_night",
          seed: "t6.8-fleet",
          steps: 48,
        },
      }),
    )
    const invariant = report.scenarioReports[0]?.phaseOutcomes
      .flatMap((phase) => phase.oracles)
      .find((oracle) => oracle.oracle === "invariant")

    expect(report.status).toBe("pass")
    expect(invariant?.ok).toBe(true)
    expect(report.coverageLedger.rpcMethods.fleetRunStatus?.calls ?? 0).toBeGreaterThan(0)
  })
})
