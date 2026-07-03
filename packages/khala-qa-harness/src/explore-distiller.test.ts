import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  distillKhalaCodeQaExploreSessionToRegression,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaScenario,
  type KhalaCodeQaExploreSession,
} from "./index.js"

const passingDesktopExploreSession = (): KhalaCodeQaExploreSession => ({
  actionLog: [
    {
      action: { kind: "boot", backend: "fixture", headless: true },
      index: 0,
      rationale: "desktop driver boot event is recorded but not replayed as a phase action",
    },
    {
      action: { kind: "rpc_call", method: "codexFleetStatus" },
      index: 1,
      rationale: "desktop driver observed fleet status through the Mode P bridge",
    },
    {
      action: { kind: "read", query: "projection:fleet_counts" },
      index: 2,
      rationale: "desktop driver captured the projected fleet count state",
    },
  ],
  backend: "fixture",
  explorer: "llm",
  mode: "rpc",
  oracleExpectations: [
    { oracle: "schema", query: "codexFleetStatus" },
    { oracle: "crash" },
  ],
  runId: "q6_2_desktop_fleet_counts",
  schema: "khala_code_qa_explore_session.v1",
  status: "pass",
})

const makeFixtureDriver = () =>
  makeKhalaCodeRpcQaDriver({
    baseUrl: "http://fixture.local",
    fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
    now: () => "2026-07-02T00:00:00.000Z",
  })

describe("Khala Code QA explore -> distill -> regress loop", () => {
  test("distills a passing desktop-driver explore session into a deterministic scenario", async () => {
    const result = distillKhalaCodeQaExploreSessionToRegression(passingDesktopExploreSession())

    expect(result.verdict).toBe("CONFIRMED")
    expect(result.distilled?.scenario.id).toBe("scenario.khala_code.distilled.q6_2_desktop_fleet_counts.v1")
    expect(result.distilled?.scenario.phases[0]?.act).toEqual([
      { kind: "rpc_call", method: "codexFleetStatus" },
      { kind: "read", query: "projection:fleet_counts" },
    ])

    const replay = await Effect.runPromise(
      runKhalaCodeQaScenario({
        driver: makeFixtureDriver(),
        scenario: result.distilled!.scenario,
      }),
    )

    expect(replay.status).toBe("pass")
    expect(replay.commitments.verdict).toBe("CONFIRMED")
  })

  test("marks non-replayable discoveries INCONCLUSIVE instead of shipping a scenario", () => {
    const result = distillKhalaCodeQaExploreSessionToRegression({
      ...passingDesktopExploreSession(),
      actionLog: [
        {
          action: { kind: "boot", backend: "fixture", headless: true },
          index: 0,
        },
      ],
      runId: "q6_2_non_replayable_discovery",
    })

    expect(result.reason).toBe("missing_replayable_actions")
    expect(result.verdict).toBe("INCONCLUSIVE")
    expect(result.distilled).toBeUndefined()
  })

  test("marks oracle-less discoveries INCONCLUSIVE instead of inflating confidence", () => {
    const result = distillKhalaCodeQaExploreSessionToRegression({
      ...passingDesktopExploreSession(),
      oracleExpectations: [],
      runId: "q6_2_oracleless_discovery",
    })

    expect(result.reason).toBe("missing_oracle_expectations")
    expect(result.verdict).toBe("INCONCLUSIVE")
    expect(result.distilled).toBeUndefined()
  })
})
