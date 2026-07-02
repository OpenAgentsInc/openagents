import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  assertKhalaQaVisibleRect,
  decodeKhalaCodeQaScenario,
  khalaQaRectsOverlap,
  loadKhalaCodeQaScenario,
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaScenario,
  waitForKhalaQaHttp,
  type KhalaCodeRpcFetch,
} from "./index.js"

const fixtureScenario = {
  backend: "fixture",
  commitments: [
    {
      claim: "appInfo decodes with the schema oracle",
      evidence: "phase-oracle",
      id: "schema.app_info",
      match: "boot-rpc:schema",
    },
    {
      claim: "the scenario completes without failed observations",
      evidence: "run-pass",
      id: "run.pass",
    },
  ],
  id: "scenario.khala_code.fixture_rpc_app_info.v1",
  modes: ["rpc"],
  phases: [
    {
      act: [{ kind: "rpc_call", method: "appInfo" }],
      expect: [
        {
          oracle: "schema",
          query: "appInfo",
        },
        {
          oracle: "crash",
        },
      ],
      name: "boot-rpc",
    },
  ],
}

const jsonResponse = (payload: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(payload), {
    headers: { "content-type": "application/json" },
    status: init?.status ?? 200,
  })

describe("Khala Code QA scenario DSL", () => {
  test("loads typed fixture-tier scenarios", () => {
    const scenario = loadKhalaCodeQaScenario(fixtureScenario)

    expect(scenario.id).toBe("scenario.khala_code.fixture_rpc_app_info.v1")
    expect(scenario.backend).toBe("fixture")
    expect(scenario.phases[0]?.expect[0]?.oracle).toBe("schema")
  })

  test("rejects a phase without an oracle", () => {
    const decoded = decodeKhalaCodeQaScenario({
      ...fixtureScenario,
      phases: [{ act: [{ kind: "rpc_call", method: "appInfo" }], expect: [], name: "bad" }],
    })

    expect("_tag" in decoded).toBe(true)
    if ("_tag" in decoded) {
      expect(decoded.message).toContain("has no oracle expectations")
      expect(decoded.phaseName).toBe("bad")
    }
  })
})

describe("Khala Code QA RPC driver and runner", () => {
  test("runs an RPC-mode scenario and confirms commitments from observed oracle outcomes", async () => {
    const calls: Array<{ headers: Record<string, string>; url: string }> = []
    const driver = makeKhalaCodeRpcQaDriver({
      accessToken: "preview-token",
      baseUrl: "http://fixture.local",
      fetch: ((input, init) => {
        calls.push({
          headers: Object.fromEntries(new Headers(init?.headers)),
          url: String(input),
        })
        return Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
        }))
      }) as KhalaCodeRpcFetch,
      now: () => "2026-07-01T00:00:00.000Z",
    })

    const report = await Effect.runPromise(
      runKhalaCodeQaScenario({
        driver,
        scenario: loadKhalaCodeQaScenario(fixtureScenario),
      }),
    )

    expect(calls).toEqual([
      {
        headers: {
          "content-type": "application/json",
          "x-khala-code-preview-token": "preview-token",
        },
        url: "http://fixture.local/rpc/appInfo",
      },
    ])
    expect(report.status).toBe("pass")
    expect(report.phaseOutcomes[0]?.oracles.map((oracle) => oracle.ok)).toEqual([true, true])
    expect(report.commitments.verdict).toBe("CONFIRMED")
    expect(report.commitments.observed).toBe(true)
  })

  test("refutes a run-pass commitment when the schema oracle fails", async () => {
    const driver = makeKhalaCodeRpcQaDriver({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
        }))) as KhalaCodeRpcFetch,
    })

    const report = await Effect.runPromise(
      runKhalaCodeQaScenario({
        driver,
        scenario: loadKhalaCodeQaScenario(fixtureScenario),
      }),
    )

    expect(report.status).toBe("fail")
    expect(report.commitments.verdict).toBe("REFUTED")
    expect(report.phaseOutcomes[0]?.oracles[0]?.ok).toBe(false)
  })

  test("keeps unobserved commitments inconclusive", async () => {
    const scenario = loadKhalaCodeQaScenario({
      ...fixtureScenario,
      commitments: [
        {
          claim: "a later oracle was observed",
          evidence: "phase-oracle",
          id: "missing",
          match: "missing-phase:schema",
        },
      ],
    })
    const driver = makeKhalaCodeRpcQaDriver({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
        }))) as KhalaCodeRpcFetch,
    })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario }))

    expect(report.status).toBe("pass")
    expect(report.commitments.verdict).toBe("INCONCLUSIVE")
  })
})

describe("desktop smoke helper extraction", () => {
  test("waitForKhalaQaHttp uses injectable fetch and sleep", async () => {
    let calls = 0
    await waitForKhalaQaHttp("http://fixture.local", {
      fetch: (() => {
        calls += 1
        return Promise.resolve(new Response("ok", { status: calls === 1 ? 503 : 200 }))
      }) as unknown as typeof fetch,
      intervalMs: 1,
      sleep: () => Promise.resolve(),
      timeoutMs: 100,
    })

    expect(calls).toBe(2)
  })

  test("probe helpers preserve the existing visible-rect and overlap behavior", () => {
    const viewport = { height: 200, width: 200, x: 0, y: 0 }
    assertKhalaQaVisibleRect("panel", { height: 50, width: 50, x: 10, y: 10 }, viewport)
    expect(khalaQaRectsOverlap(
      { height: 20, width: 20, x: 10, y: 10 },
      { height: 20, width: 20, x: 15, y: 15 },
    )).toBe(true)
    expect(() =>
      assertKhalaQaVisibleRect("panel", { height: 50, width: 50, x: 190, y: 10 }, viewport),
    ).toThrow("overflows")
  })
})
