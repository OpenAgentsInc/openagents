import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  collectKhalaCodeQaCoverageLedger,
  khalaCodeQaCoverageFrontierReport,
  KHALA_CODE_QA_ERROR_STATE_CASE_IDS,
  KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS,
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  KHALA_CODE_QA_SEED_SCENARIOS,
  loadKhalaCodeQaScenario,
  makeKhalaCodeDomFixtureQaDriver,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  mergeKhalaCodeQaCoverageLedgers,
  runKhalaCodeQaCrossModeScenario,
  runKhalaCodeQaScenario,
} from "./index.js"

describe("Khala Code QA coverage ledger", () => {
  test("emits mergeable coverage from a seed corpus run", async () => {
    const reports = []
    for (const scenario of KHALA_CODE_QA_SEED_SCENARIOS) {
      const loaded = loadKhalaCodeQaScenario(scenario)
      if (loaded.modes.includes("dom")) {
        const report = await Effect.runPromise(
          runKhalaCodeQaCrossModeScenario({
            makeDriver: (mode) => mode === "rpc"
              ? makeKhalaCodeRpcQaDriver({
                baseUrl: "http://fixture.local",
                fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
                now: () => "2026-07-01T00:00:00.000Z",
              })
              : makeKhalaCodeDomFixtureQaDriver({
                baseUrl: "http://fixture.local",
                fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
                now: () => "2026-07-01T00:00:00.000Z",
              }),
            scenario: loaded,
          }),
        )
        reports.push(...Object.values(report.modeReports))
        continue
      }
      const report = await Effect.runPromise(
        runKhalaCodeQaScenario({
          driver: makeKhalaCodeRpcQaDriver({
            baseUrl: "http://fixture.local",
            fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
            now: () => "2026-07-01T00:00:00.000Z",
          }),
          scenario: loaded,
        }),
      )
      reports.push(report)
    }

    const ledger = mergeKhalaCodeQaCoverageLedgers(reports.map((report) => report.coverageLedger))

    expect(ledger.schema).toBe("khala_code_qa_coverage_ledger.v1")
    expect(ledger.runIds).toHaveLength(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.scenarioCount)
    expect(ledger.rpcMethods.codexThreadList).toMatchObject({
      calls: 4,
      distinctArgumentShapeCount: 1,
    })
    expect(ledger.rpcMethods.codexConfigValueWrite?.argumentShapes).toEqual(["[{keyPath:string,value:string}]"])
    for (const [group, methods] of Object.entries(KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS)) {
      expect(ledger.rpcGroups[group]?.calls).toBeGreaterThan(0)
      expect(ledger.rpcGroups[group]?.methods).toEqual(expect.arrayContaining(methods))
    }
    for (const key of KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.settingsKeys) {
      expect(ledger.settingsKeysWritten).toContain(key)
    }
    for (const decision of KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.approvalDecisionKinds) {
      expect(ledger.approvalDecisionKinds).toContain(decision)
    }
    expect(ledger.fleetRunControlVerbs).toEqual([...KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.fleetRunControlVerbs].sort())
    expect(ledger.inboxRoutingFlagKinds).toEqual([...KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.inboxRoutingFlagKinds].sort())
    expect(ledger.errorStateCasesExercised).toEqual([...KHALA_CODE_QA_ERROR_STATE_CASE_IDS].sort())
    expect(ledger.crossModeSurfacesExercised).toEqual(
      [...KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.crossModeSurfaces].sort(),
    )
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
      expect(ledger.threadItemVariantRenderCounts[variant]).toBeGreaterThan(0)
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
    expect(merged.rpcGroups["rpc.threads"]).toEqual({
      calls: 2,
      methods: ["codexThreadStart"],
    })
    expect(merged.selectorsClicked).toEqual(["[data-testid=thread-list]"])
  })

  test("counts repeated ThreadItem variant renders across ledgers", () => {
    const first = collectKhalaCodeQaCoverageLedger({
      generatedAt: "2026-07-01T00:00:00.000Z",
      observations: [{
        action: { args: [{ threadId: "thread-a", includeTurns: true }], kind: "rpc_call", method: "codexThreadRead" },
        data: {
          messages: [
            { codexItem: { itemType: "agentMessage" } },
            { harnessItem: { itemType: "commandExecution" } },
          ],
        },
        label: "rpc:codexThreadRead#1",
        ok: true,
      }],
      runId: "run-a",
    })
    const second = collectKhalaCodeQaCoverageLedger({
      generatedAt: "2026-07-02T00:00:00.000Z",
      observations: [{
        action: { args: [{ threadId: "thread-b", includeTurns: true }], kind: "rpc_call", method: "codexThreadRead" },
        data: {
          messages: [
            { codexItem: { itemType: "agentMessage" } },
          ],
        },
        label: "rpc:codexThreadRead#1",
        ok: true,
      }],
      runId: "run-b",
    })

    const merged = mergeKhalaCodeQaCoverageLedgers([first, second])

    expect(first.threadItemVariantRenderCounts).toEqual({
      agentMessage: 1,
      commandExecution: 1,
    })
    expect(merged.threadItemVariantRenderCounts).toEqual({
      agentMessage: 2,
      commandExecution: 1,
    })
    expect(merged.threadItemVariantsRendered).toEqual(["agentMessage", "commandExecution"])
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
    expect(frontier.missing.rpcGroups).toContain("rpc.fleet")
    expect(frontier.missing.fleetRunControlVerbs).toEqual(["drain", "pause", "resume", "stop"])
    expect(frontier.missing.inboxRoutingFlagKinds).toEqual(["flag", "interrupt", "retry"])
    expect(frontier.missing.errorStateCases).toEqual([...KHALA_CODE_QA_ERROR_STATE_CASE_IDS].sort())
    expect(frontier.missing.crossModeSurfaces).toEqual(
      [...KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.crossModeSurfaces].sort(),
    )
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
