import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  collectKhalaCodeQaCoverageLedger,
  khalaCodeQaCoverageFrontierReport,
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  KHALA_CODE_QA_SEED_SCENARIOS,
  loadKhalaCodeQaScenario,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  mergeKhalaCodeQaCoverageLedgers,
  runKhalaCodeQaScenario,
} from "./index.js"

describe("Khala Code QA coverage ledger", () => {
  test("emits mergeable coverage from a seed corpus run", async () => {
    const reports = []
    for (const scenario of KHALA_CODE_QA_SEED_SCENARIOS) {
      const report = await Effect.runPromise(
        runKhalaCodeQaScenario({
          driver: makeKhalaCodeRpcQaDriver({
            baseUrl: "http://fixture.local",
            fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
            now: () => "2026-07-01T00:00:00.000Z",
          }),
          scenario: loadKhalaCodeQaScenario(scenario),
        }),
      )
      reports.push(report)
    }

    const ledger = mergeKhalaCodeQaCoverageLedgers(reports.map((report) => report.coverageLedger))

    expect(ledger.schema).toBe("khala_code_qa_coverage_ledger.v1")
    expect(ledger.runIds).toHaveLength(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.scenarioCount)
    expect(ledger.rpcMethods.codexThreadList).toMatchObject({
      calls: 2,
      distinctArgumentShapeCount: 1,
    })
    expect(ledger.rpcMethods.codexConfigValueWrite?.argumentShapes).toEqual(["[{keyPath:string,value:string}]"])
    expect(ledger.settingsKeysWritten).toContain("model")
    expect(ledger.approvalDecisionKinds).toContain("accept")
    expect(ledger.hotbarPanelsOpened).toEqual(["chat", "fleet", "settings"])
    for (const command of KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.slashCommands) {
      expect(ledger.slashCommands[command]?.dispatches).toBeGreaterThan(0)
      expect(ledger.slashCommands[command]?.availabilityStateCount).toBe(
        ledger.slashCommands[command]?.availabilityStates.length,
      )
      expect(ledger.slashCommands[command]?.availabilityStates).toEqual(
        expect.arrayContaining(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.slashCommandAvailabilityStates[command] ?? []),
      )
    }
    for (const variant of KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.threadItemVariants) {
      expect(ledger.threadItemVariantsRendered).toContain(variant)
    }
  })

  test("unions ledgers while preserving call counts and distinct argument shapes", () => {
    const first = collectKhalaCodeQaCoverageLedger({
      generatedAt: "2026-07-01T00:00:00.000Z",
      observations: [
        {
          action: { args: [{ sessionId: "a" }], kind: "rpc_call", method: "codexThreadStart" },
          label: "rpc:codexThreadStart#1",
          ok: true,
        },
        {
          action: { kind: "click", target: "[data-testid=thread-list]" },
          label: "click",
          ok: true,
        },
      ],
      runId: "run-a",
    })
    const second = collectKhalaCodeQaCoverageLedger({
      generatedAt: "2026-07-02T00:00:00.000Z",
      observations: [
        {
          action: { args: [{ sessionId: "b", cwd: "/workspace" }], kind: "rpc_call", method: "codexThreadStart" },
          label: "rpc:codexThreadStart#1",
          ok: true,
        },
        {
          action: { kind: "click", target: "[data-testid=thread-list]" },
          label: "click",
          ok: true,
        },
      ],
      runId: "run-b",
    })

    const merged = mergeKhalaCodeQaCoverageLedgers([first, second])

    expect(merged.generatedAt).toBe("2026-07-02T00:00:00.000Z")
    expect(merged.runIds).toEqual(["run-a", "run-b"])
    expect(merged.rpcMethods.codexThreadStart).toEqual({
      argumentShapes: ["[{cwd:string,sessionId:string}]", "[{sessionId:string}]"],
      calls: 2,
      distinctArgumentShapeCount: 2,
    })
    expect(merged.selectorsClicked).toEqual(["[data-testid=thread-list]"])
  })

  test("lists never-exercised coverage classes from the seed corpus manifest", () => {
    const ledger = createPartialLedgerForFrontier()
    const frontier = khalaCodeQaCoverageFrontierReport({
      generatedAt: "2026-07-03T00:00:00.000Z",
      ledger,
      manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
      zeroForAWeek: ["hotbarPanels:fleet"],
    })

    expect(frontier.schema).toBe("khala_code_qa_coverage_frontier.v1")
    expect(frontier.missing.hotbarPanels).toContain("fleet")
    expect(frontier.missing.settingsKeys).toContain("model")
    expect(frontier.missing.approvalDecisionKinds).toContain("accept")
    expect(frontier.missing.slashCommands.length).toBeGreaterThan(0)
    expect(frontier.missing.slashCommandAvailabilityStates.length).toBeGreaterThan(0)
    expect(frontier.zeroForAWeekIssueCandidates).toEqual(["hotbarPanels:fleet"])
  })
})

const createPartialLedgerForFrontier = () =>
  collectKhalaCodeQaCoverageLedger({
    generatedAt: "2026-07-03T00:00:00.000Z",
    observations: [
      {
        action: { kind: "hotbar", target: "chat" },
        label: "hotbar:chat",
        ok: true,
      },
      {
        action: { args: [{ sessionId: "a" }], kind: "rpc_call", method: "codexThreadStart" },
        label: "rpc:codexThreadStart#1",
        ok: true,
      },
    ],
    runId: "partial",
  })
