import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  collectKhalaCodeQaCoverageLedger,
  makeKhalaCodeDomFixtureQaDriver,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaLiveExplorerSession,
  type KhalaCodeQaExplorerBrainContext,
} from "./index.js"

const makeRpcDriver = () =>
  makeKhalaCodeRpcQaDriver({
    baseUrl: "http://fixture.local",
    fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
    now: () => "2026-07-02T00:00:00.000Z",
  })

const makeDomDriver = () =>
  makeKhalaCodeDomFixtureQaDriver({
    baseUrl: "http://fixture.local",
    fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
    now: () => "2026-07-02T00:00:00.000Z",
  })

describe("Khala Code QA live explorer session", () => {
  test("fake_model path drives Mode P without live env or network transport", async () => {
    let decideCalls = 0
    const report = await Effect.runPromise(
      runKhalaCodeQaLiveExplorerSession({
        brainMode: "fake_model",
        decide: () => {
          decideCalls += 1
          return Effect.succeed({
            action: { kind: "hotbar", target: "fleet" },
            rationale: "should not be called by fake_model",
            tier: "live_llm",
          })
        },
        drivers: [makeRpcDriver()],
        env: {},
        seed: "fake-model-ci",
        steps: 10,
      }),
    )

    expect(decideCalls).toBe(0)
    expect(report.brainMode).toBe("fake_model")
    expect(report.driverModes).toEqual(["rpc"])
    expect(report.status).toBe("pass")
    expect(report.reports[0]?.actionLog).toHaveLength(10)
    expect(report.goals.length).toBeGreaterThan(0)
  })

  test("injected live brain receives frontier goals and replays the same plan through Mode P and Mode D", async () => {
    const contexts: KhalaCodeQaExplorerBrainContext[] = []
    const previousCoverageLedger = collectKhalaCodeQaCoverageLedger({
      observations: [{
        action: { kind: "hotbar", target: "chat" },
        label: "hotbar:chat",
        ok: true,
      }],
      runId: "partial-frontier",
    })
    const report = await Effect.runPromise(
      runKhalaCodeQaLiveExplorerSession({
        brainMode: "live_llm",
        decide: (context) => {
          contexts.push(context)
          return Effect.succeed({
            action: { kind: "hotbar", target: "fleet" },
            frontierRef: "hotbarPanels:fleet",
            rationale: "exercise frontier hotbar fleet",
            tier: "live_llm",
          })
        },
        drivers: [makeRpcDriver(), makeDomDriver()],
        env: { KHALA_QA_LIVE_EXPLORER_BRAIN: "1" },
        previousCoverageLedger,
        seed: "live-brain-pd",
        steps: 6,
      }),
    )

    expect(contexts).toHaveLength(6)
    expect(contexts[0]?.frontier.missing.hotbarPanels).toContain("fleet")
    expect(report.brainMode).toBe("live_llm")
    expect(report.driverModes).toEqual(["dom", "rpc"])
    expect(report.status).toBe("pass")
    expect(report.goals.some((goal) => goal.frontierRef === "hotbarPanels:fleet")).toBe(true)
    expect(report.plan.actionLog.every((entry) => entry.frontierRef === "hotbarPanels:fleet")).toBe(true)
    expect(report.reports).toHaveLength(2)
    expect(report.reports[0]?.actionLog).toEqual(report.reports[1]?.actionLog)
    expect(report.reports[1]?.scenarioReports[0]?.mode).toBe("dom")
  })
})
