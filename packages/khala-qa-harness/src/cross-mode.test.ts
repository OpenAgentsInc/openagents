import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  KHALA_CODE_QA_SEED_SCENARIOS,
  loadKhalaCodeQaScenario,
  makeKhalaCodeDomFixtureQaDriver,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaCrossModeScenario,
  type KhalaCodeQaCrossModeDisagreementBug,
  type KhalaCodeRpcFetch,
} from "./index.js"

const crossModeScenario = loadKhalaCodeQaScenario(
  KHALA_CODE_QA_SEED_SCENARIOS.find((scenario) =>
    scenario.id === "scenario.khala_code.seed.cross_mode_consistency.v1"
  )!,
)

const fixtureDriver = (mode: "dom" | "rpc", fetch: KhalaCodeRpcFetch = makeKhalaCodeQaSeedCorpusFixtureFetch()) =>
  mode === "rpc"
    ? makeKhalaCodeRpcQaDriver({
      baseUrl: "http://fixture.local",
      fetch,
      now: () => "2026-07-01T00:00:00.000Z",
    })
    : makeKhalaCodeDomFixtureQaDriver({
      baseUrl: "http://fixture.local",
      fetch,
      now: () => "2026-07-01T00:00:00.000Z",
    })

const jsonResponse = (payload: unknown): Response =>
  new Response(JSON.stringify(payload), {
    headers: { "content-type": "application/json" },
  })

const driftThreadListTitle = (fetch: KhalaCodeRpcFetch): KhalaCodeRpcFetch =>
  async (input, init) => {
    const response = await fetch(input, init)
    if (!String(input).endsWith("/rpc/codexThreadList")) return response
    const payload = await response.json() as {
      readonly data?: readonly unknown[]
      readonly threads?: readonly unknown[]
    }
    const mutateThreads = (threads: readonly unknown[] | undefined) =>
      threads?.map((thread) =>
        thread !== null && typeof thread === "object" && !Array.isArray(thread)
          ? { ...thread, title: "DOM drift fixture" }
          : thread
      ) ?? []
    return jsonResponse({
      ...payload,
      data: mutateThreads(payload.data),
      threads: mutateThreads(payload.threads),
    })
  }

describe("Khala Code QA cross-mode runner", () => {
  test("runs one scenario document through Mode P and Mode D consistency oracles", async () => {
    const report = await Effect.runPromise(
      runKhalaCodeQaCrossModeScenario({
        makeDriver: fixtureDriver,
        scenario: crossModeScenario,
      }),
    )

    expect(report.status).toBe("pass")
    expect(report.modeReports.rpc.mode).toBe("rpc")
    expect(report.modeReports.dom.mode).toBe("dom")
    expect(report.consistencyOutcomes.map((outcome) => outcome.verdict)).toEqual([
      "CONFIRMED",
      "CONFIRMED",
      "CONFIRMED",
      "CONFIRMED",
    ])
    expect(report.firstDisagreementBug).toBeUndefined()
  })

  test("files the first disagreement with both mode states attached", async () => {
    let filed: KhalaCodeQaCrossModeDisagreementBug | undefined
    const report = await Effect.runPromise(
      runKhalaCodeQaCrossModeScenario({
        fileDisagreement: (bug) =>
          Effect.sync(() => {
            filed = bug
            return { url: "https://example.test/issues/qa-cross-mode" }
          }),
        makeDriver: (mode) => fixtureDriver(
          mode,
          mode === "dom"
            ? driftThreadListTitle(makeKhalaCodeQaSeedCorpusFixtureFetch())
            : makeKhalaCodeQaSeedCorpusFixtureFetch(),
        ),
        scenario: crossModeScenario,
      }),
    )

    expect(report.status).toBe("fail")
    expect(report.bugIssue?.url).toBe("https://example.test/issues/qa-cross-mode")
    expect(filed?.schema).toBe("khala_code_qa_cross_mode_disagreement_bug.v1")
    expect(filed?.phaseName).toBe("thread-list-cross-mode")
    expect(filed?.body).toContain("Left state:")
    expect(filed?.body).toContain("Right state:")
    expect(filed?.leftState).toBeDefined()
    expect(filed?.rightState).toBeDefined()
    expect(filed?.mismatches.map((mismatch) => mismatch.path)).toContain("threads[0].title")
  })
})
