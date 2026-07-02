import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
  KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES,
} from "../../../clients/khala-code-desktop/src/bun/codex-parity-contract.js"
import {
  KHALA_CODE_DESKTOP_SLASH_COMMANDS,
} from "../../../clients/khala-code-desktop/src/shared/codex-slash-commands.js"

import {
  decodeKhalaCodeQaScenario,
  KHALA_CODE_QA_ERROR_STATE_CASES,
  KHALA_CODE_QA_ERROR_STATE_CASE_IDS,
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  KHALA_CODE_QA_SEED_CROSS_MODE_SURFACES,
  KHALA_CODE_QA_SEED_SCENARIOS,
  KHALA_CODE_QA_THREAD_ITEM_FIXTURES,
  KHALA_CODE_QA_THREAD_ITEM_FIXTURE_SOURCE,
  KHALA_CODE_QA_THREAD_ITEM_VARIANTS,
  KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS,
  loadKhalaCodeQaScenario,
  makeKhalaCodeDomFixtureQaDriver,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaCrossModeScenario,
  runKhalaCodeQaScenario,
} from "./index.js"

const scenarioIds = KHALA_CODE_QA_SEED_SCENARIOS.map((scenario) => scenario.id)

const idsForGroup = (group: string): readonly string[] =>
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST.scenarioIdsByGroup.find((entry) => entry.group === group)
    ?.scenarioIds ?? []

const rpcMethodsForGroup = (group: string): readonly string[] => {
  const ids = new Set(idsForGroup(group))
  return [...new Set(KHALA_CODE_QA_SEED_SCENARIOS
    .filter((scenario) => ids.has(scenario.id))
    .flatMap((scenario) => scenario.phases)
    .flatMap((phase) => phase.act)
    .flatMap((action) => action.kind === "rpc_call" ? [action.method] : []))]
    .sort()
}

describe("Khala Code QA seed scenario corpus", () => {
  test("emits a manifest grouped for coverage counting", () => {
    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST).toMatchObject({
      backend: "fixture",
      schema: "khala_code_qa_seed_corpus_manifest.v1",
      scenarioCount: KHALA_CODE_QA_SEED_SCENARIOS.length,
    })
    expect(new Set(scenarioIds).size).toBe(scenarioIds.length)
    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.scenarioIdsByGroup.flatMap((entry) => entry.scenarioIds).sort())
      .toEqual([...scenarioIds].sort())
  })

  test("covers the mechanical seed groups requested by Q4.1", () => {
    const expectedRpcGroups = Object.keys(KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS)
    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.rpcGroups).toEqual(expectedRpcGroups)
    for (const group of [...expectedRpcGroups, "hotbar", "thread_items", "cross_mode", "error_states"]) {
      expect(idsForGroup(group).length).toBeGreaterThan(0)
    }
    for (const [group, methods] of Object.entries(KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS)) {
      expect(rpcMethodsForGroup(group)).toEqual(expect.arrayContaining(methods))
    }

    expect(idsForGroup("hotbar")).toEqual([
      "scenario.khala_code.seed.hotbar_chat_panel.v1",
      "scenario.khala_code.seed.hotbar_fleet_panel.v1",
      "scenario.khala_code.seed.hotbar_settings_panel.v1",
    ])

    const expectedThreadItemScenarioIds = KHALA_CODE_QA_THREAD_ITEM_VARIANTS.map((variant) => {
      const normalized = variant.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)
      return `scenario.khala_code.seed.thread_item_${normalized}.v1`
    })
    expect(idsForGroup("thread_items")).toEqual(expectedThreadItemScenarioIds)
    expect([...KHALA_CODE_QA_THREAD_ITEM_VARIANTS]).toEqual([...KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES])
    expect(KHALA_CODE_QA_THREAD_ITEM_FIXTURE_SOURCE.referenceCommit).toBe(KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT)
    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.threadItemFixtureSource).toEqual(KHALA_CODE_QA_THREAD_ITEM_FIXTURE_SOURCE)
    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.threadItemFixtures).toEqual(KHALA_CODE_QA_THREAD_ITEM_FIXTURES)
    expect(KHALA_CODE_QA_THREAD_ITEM_FIXTURES.map((fixture) => fixture.variant)).toEqual(
      [...KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES],
    )
    expect(new Set(KHALA_CODE_QA_THREAD_ITEM_FIXTURES.map((fixture) => fixture.fixtureId)).size)
      .toBe(KHALA_CODE_QA_THREAD_ITEM_FIXTURES.length)

    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.crossModeSurfaces).toEqual(
      KHALA_CODE_QA_SEED_CROSS_MODE_SURFACES,
    )
    expect(idsForGroup("cross_mode")).toEqual([
      "scenario.khala_code.seed.cross_mode_consistency.v1",
    ])
    const crossModeScenario = KHALA_CODE_QA_SEED_SCENARIOS.find((candidate) =>
      candidate.id === "scenario.khala_code.seed.cross_mode_consistency.v1"
    )
    expect(crossModeScenario?.modes).toEqual(["rpc", "dom"])
    expect(crossModeScenario?.phases.map((phase) => phase.name)).toEqual([
      "thread-list-cross-mode",
      "fleet-counts-cross-mode",
      "gym-state-cross-mode",
      "runtime-badges-cross-mode",
    ])
    for (const surface of KHALA_CODE_QA_SEED_CROSS_MODE_SURFACES) {
      expect(crossModeScenario?.phases.flatMap((phase) => phase.expect)).toContainEqual({
        left: `rpc:projection:${surface}`,
        oracle: "consistency",
        right: `dom:projection:${surface}`,
      })
    }

    const slashCommandIds = idsForGroup("rpc.slash_commands")
    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.slashCommands).toEqual(
      KHALA_CODE_DESKTOP_SLASH_COMMANDS.map((command) => command.command),
    )
    for (const command of KHALA_CODE_DESKTOP_SLASH_COMMANDS) {
      expect(slashCommandIds).toContain(
        `scenario.khala_code.seed.slash_command_${command.command.replace(/[^a-z0-9]+/g, "_")}.v1`,
      )
      const scenario = KHALA_CODE_QA_SEED_SCENARIOS.find((candidate) =>
        candidate.id === `scenario.khala_code.seed.slash_command_${command.command.replace(/[^a-z0-9]+/g, "_")}.v1`
      )
      expect(scenario?.phases.map((phase) => phase.name)).toContain("list-slash-command-availability")
      expect(scenario?.phases.map((phase) => phase.name)).toContain("dispatch-slash-command")
      expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.slashCommandAvailabilityStates[command.command]?.length)
        .toBeGreaterThan(0)
      if (!command.availableDuringTask) {
        expect(scenario?.phases.map((phase) => phase.name)).toContain("dispatch-while-task-active")
      }
      if (!command.availableInSideConversation) {
        expect(scenario?.phases.map((phase) => phase.name)).toContain("dispatch-from-side-conversation")
      }
      if (command.dispatch.kind === "app_server" && command.dispatch.requiresArgs === true) {
        expect(scenario?.phases.map((phase) => phase.name)).toContain("dispatch-without-required-args")
      }
      if (command.dispatch.kind === "app_server" && command.dispatch.requiresThread === true) {
        expect(scenario?.phases.map((phase) => phase.name)).toContain("dispatch-without-required-thread")
      }
    }

    expect(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.errorStateCases).toEqual(KHALA_CODE_QA_ERROR_STATE_CASE_IDS)
    expect(idsForGroup("error_states")).toEqual(
      KHALA_CODE_QA_ERROR_STATE_CASE_IDS.map((caseId) => `scenario.khala_code.seed.error_state_${caseId}.v1`),
    )
    for (const errorCase of KHALA_CODE_QA_ERROR_STATE_CASES) {
      const scenario = KHALA_CODE_QA_SEED_SCENARIOS.find((candidate) =>
        candidate.id === `scenario.khala_code.seed.error_state_${errorCase.caseId}.v1`
      )
      expect(scenario).toBeDefined()
      expect(scenario?.phases.flatMap((phase) => phase.act).some((action) =>
        action.kind === "rpc_call" && action.method === errorCase.targetMethod
      )).toBe(true)
      expect(scenario?.phases.flatMap((phase) => phase.expect)).toEqual(
        expect.arrayContaining([
          { id: "typed-degraded-state", match: errorCase.caseId, oracle: "invariant" },
          { id: "no-console-errors", oracle: "invariant" },
          { id: "no-data-loss", match: errorCase.caseId, oracle: "invariant" },
        ]),
      )
    }
    const partialDegradationScenario = KHALA_CODE_QA_SEED_SCENARIOS.find((candidate) =>
      candidate.id === "scenario.khala_code.seed.error_state_single_rpc_failure_partial_degradation.v1"
    )
    expect(partialDegradationScenario?.phases[0]?.name).toBe("fleet-panel-partial-degradation")
    expect(partialDegradationScenario?.phases[0]?.act).toEqual(
      expect.arrayContaining([
        { kind: "hotbar", target: "fleet" },
        { kind: "rpc_call", method: "codexFleetStatus" },
        { kind: "rpc_call", method: "fleetRunList", args: [{}] },
      ]),
    )
    const restartScenario = KHALA_CODE_QA_SEED_SCENARIOS.find((candidate) =>
      candidate.id === "scenario.khala_code.seed.error_state_app_server_crash_restart.v1"
    )
    expect(restartScenario?.phases.map((phase) => phase.name)).toEqual([
      "observe-app-server-crash",
      "restart-and-resume-thread",
    ])
    expect(restartScenario?.phases[1]?.act).toEqual(
      expect.arrayContaining([
        { kind: "rpc_call", method: "codexAppServerRestart" },
        { kind: "rpc_call", method: "codexThreadResume", args: [{ cwd: "/workspace", sessionId: "desktop-session-fixture", threadId: "thread-fixture" }] },
      ]),
    )
  })

  test("loads every seed scenario and rejects phases without expectations by construction", () => {
    for (const scenario of KHALA_CODE_QA_SEED_SCENARIOS) {
      const loaded = loadKhalaCodeQaScenario(scenario)
      expect(loaded.backend).toBe("fixture")
      expect(loaded.modes).toContain("rpc")
      expect(loaded.phases.length).toBeGreaterThan(0)
      expect(loaded.phases.every((phase) => phase.expect.length > 0)).toBe(true)
      expect(loaded.commitments.length).toBeGreaterThan(0)
    }

    const invalid = decodeKhalaCodeQaScenario({
      ...KHALA_CODE_QA_SEED_SCENARIOS[0],
      phases: [{
        act: [{ kind: "rpc_call", method: "appInfo" }],
        expect: [],
        name: "oracle-less-phase",
      }],
    })

    expect("_tag" in invalid).toBe(true)
    if ("_tag" in invalid) {
      expect(invalid.phaseName).toBe("oracle-less-phase")
      expect(invalid.message).toContain("has no oracle expectations")
    }
  })

  test("runs the complete corpus against the fixture backend", async () => {
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
        reports.push(report)
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

    expect(reports).toHaveLength(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.scenarioCount)
    expect(reports.map((report) => [report.scenarioId, report.status])).toEqual(
      scenarioIds.map((id) => [id, "pass"]),
    )
    expect(reports.every((report) =>
      "commitments" in report ? report.commitments.verdict === "CONFIRMED" : report.status === "pass"
    )).toBe(true)
  })
})
