import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  khalaCodeDesktopSlashCommandsWithAvailability,
} from "../../../clients/khala-code-desktop/src/shared/codex-slash-commands.js"

import {
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  KHALA_CODE_QA_SEED_SCENARIOS,
  KHALA_CODE_QA_THREAD_ITEM_VARIANTS,
  loadKhalaCodeQaScenario,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaScenario,
} from "./index.js"

const scenarioIds = KHALA_CODE_QA_SEED_SCENARIOS.map((scenario) => scenario.id)

const idsForGroup = (group: string): readonly string[] =>
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST.scenarioIdsByGroup.find((entry) => entry.group === group)
    ?.scenarioIds ?? []

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

  test("covers the mechanical seed groups requested by T6.7", () => {
    for (const group of [
      "rpc.threads",
      "rpc.turns",
      "rpc.fleet",
      "rpc.approvals",
      "rpc.settings",
      "rpc.ecosystem",
      "rpc.slash_commands",
      "hotbar",
      "thread_items",
    ]) {
      expect(idsForGroup(group).length).toBeGreaterThan(0)
    }

    expect(idsForGroup("hotbar")).toEqual([
      "scenario.khala_code.seed.hotbar_chat_panel.v1",
      "scenario.khala_code.seed.hotbar_fleet_panel.v1",
      "scenario.khala_code.seed.hotbar_settings_panel.v1",
    ])

    for (const variant of KHALA_CODE_QA_THREAD_ITEM_VARIANTS) {
      const normalized = variant.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)
      expect(idsForGroup("thread_items")).toContain(
        `scenario.khala_code.seed.thread_item_${normalized}.v1`,
      )
    }

    const slashCommandIds = idsForGroup("rpc.slash_commands")
    for (const command of khalaCodeDesktopSlashCommandsWithAvailability({ debug: true, platform: "darwin" })) {
      expect(slashCommandIds).toContain(
        `scenario.khala_code.seed.slash_command_${command.command.replace(/[^a-z0-9]+/g, "_")}.v1`,
      )
    }
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
  })

  test("runs the complete corpus against the fixture RPC backend", async () => {
    const reports = []
    for (const scenario of KHALA_CODE_QA_SEED_SCENARIOS) {
      const driver = makeKhalaCodeRpcQaDriver({
        baseUrl: "http://fixture.local",
        fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
        now: () => "2026-07-01T00:00:00.000Z",
      })
      const report = await Effect.runPromise(
        runKhalaCodeQaScenario({
          driver,
          scenario: loadKhalaCodeQaScenario(scenario),
        }),
      )
      reports.push(report)
    }

    expect(reports).toHaveLength(KHALA_CODE_QA_SEED_CORPUS_MANIFEST.scenarioCount)
    expect(reports.map((report) => [report.scenarioId, report.status])).toEqual(
      scenarioIds.map((id) => [id, "pass"]),
    )
    expect(reports.every((report) => report.commitments.verdict === "CONFIRMED")).toBe(true)
  })
})
