import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  buildKhalaCodeQaSeededMonkeyPlan,
  chooseKhalaCodeQaFrontierAction,
  collectKhalaCodeQaCoverageLedger,
  decodeKhalaCodeQaExplorerBrainDecision,
  distillKhalaCodeQaExplorerTraceToScenario,
  khalaCodeQaCoverageFrontierReport,
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  makeKhalaCodeQaDeterministicFixtureBrain,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  regressKhalaCodeQaDistilledScenario,
} from "./index.js"

const makeFixtureDriver = () =>
  makeKhalaCodeRpcQaDriver({
    baseUrl: "http://fixture.local",
    fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
    now: () => "2026-07-02T00:00:00.000Z",
  })

describe("Khala Code QA explorer brain", () => {
  test("decodes typed brain decisions and rejects invalid action shapes", () => {
    const decoded = decodeKhalaCodeQaExplorerBrainDecision({
      action: { kind: "hotbar", target: "fleet" },
      frontierRef: "hotbarPanels:fleet",
      rationale: "frontier:hotbarPanels:fleet",
      tier: "deterministic_fixture",
    })
    const rejected = decodeKhalaCodeQaExplorerBrainDecision({
      action: { kind: "not-a-qa-action" },
      rationale: "bad",
      tier: "deterministic_fixture",
    })

    expect("_tag" in decoded).toBe(false)
    expect("_tag" in rejected && rejected._tag).toBe("KhalaCodeQaExplorerBrainFailure")
  })

  test("frontier steering picks the least-covered available surface", async () => {
    const ledger = collectKhalaCodeQaCoverageLedger({
      generatedAt: "2026-07-02T00:00:00.000Z",
      observations: [{
        action: { kind: "hotbar", target: "chat" },
        label: "hotbar:chat",
        ok: true,
      }],
      runId: "partial-frontier",
    })
    const frontier = khalaCodeQaCoverageFrontierReport({
      generatedAt: "2026-07-02T00:00:00.000Z",
      ledger,
      manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
    })
    const actionSpace = [
      { kind: "hotbar", target: "chat" },
      { kind: "hotbar", target: "fleet" },
      { kind: "slash_command", value: "/status" },
    ] as const
    const fixtureBrain = makeKhalaCodeQaDeterministicFixtureBrain()
    const decision = await Effect.runPromise(
      fixtureBrain.nextAction({
        actionLog: [],
        actionSpace,
        frontier,
        stepIndex: 0,
      }),
    )

    expect(decision).toEqual(chooseKhalaCodeQaFrontierAction({
      actionSpace,
      fallbackIndex: 0,
      frontier,
    }))
    expect(decision.action).toEqual({ kind: "hotbar", target: "fleet" })
    expect(decision.frontierRef).toBe("hotbarPanels:fleet")
  })

  test("distills an explorer trace into a deterministic scenario regression", async () => {
    const plan = buildKhalaCodeQaSeededMonkeyPlan({
      mode: "fixture_smoke",
      seed: "t6.9-distill",
      steps: 8,
    })
    const distilled = distillKhalaCodeQaExplorerTraceToScenario({
      ...plan.explorerTrace,
      status: "fail",
    })
    const first = await Effect.runPromise(
      regressKhalaCodeQaDistilledScenario({
        distilled,
        driver: makeFixtureDriver(),
      }),
    )
    const second = await Effect.runPromise(
      regressKhalaCodeQaDistilledScenario({
        distilled,
        driver: makeFixtureDriver(),
      }),
    )

    expect(distilled.schema).toBe("khala_code_qa_distilled_scenario.v1")
    expect(distilled.scenario.phases[0]?.act).toEqual(plan.actionLog.map((entry) => entry.action))
    expect(first.status).toBe("pass")
    expect(first.phaseOutcomes).toEqual(second.phaseOutcomes)
    expect(first.coverageLedger.rpcMethods).toEqual(second.coverageLedger.rpcMethods)
  })
})
