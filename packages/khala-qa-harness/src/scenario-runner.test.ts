import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  assertKhalaQaVisibleRect,
  decodeKhalaCodeQaScenario,
  khalaQaRectsOverlap,
  loadKhalaCodeQaScenario,
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaScenario,
  unsupportedKhalaCodeQaDriver,
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

  test("rejects a scenario without phases", () => {
    const decoded = decodeKhalaCodeQaScenario({
      ...fixtureScenario,
      phases: [],
    })

    expect("_tag" in decoded).toBe(true)
    if ("_tag" in decoded) {
      expect(decoded.message).toContain("no phases")
    }
  })

  test("rejects a scenario without modes", () => {
    const decoded = decodeKhalaCodeQaScenario({
      ...fixtureScenario,
      modes: [],
    })

    expect("_tag" in decoded).toBe(true)
    if ("_tag" in decoded) {
      expect(decoded.message).toContain("no driver modes")
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

  test("surfaces unevaluated perf budget oracles as inconclusive", async () => {
    const scenario = loadKhalaCodeQaScenario({
      ...fixtureScenario,
      commitments: [
        {
          claim: "appInfo stays within the perf budget",
          evidence: "phase-oracle",
          id: "perf.app_info",
          match: "boot-rpc:perf",
        },
      ],
      phases: [
        {
          act: [{ kind: "rpc_call", method: "appInfo" }],
          expect: [{ budget: 10, metric: "rpc.duration_ms", oracle: "perf" }],
          name: "boot-rpc",
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

    expect(report.status).toBe("fail")
    expect(report.phaseOutcomes[0]?.oracles[0]).toMatchObject({
      ok: false,
      oracle: "perf",
      verdict: "INCONCLUSIVE",
    })
    expect(report.commitments.verdict).toBe("INCONCLUSIVE")
    expect(report.commitments.observed).toBe(false)
  })

  test("refutes a commitment when any matching schema oracle fails", async () => {
    let calls = 0
    const scenario = loadKhalaCodeQaScenario({
      ...fixtureScenario,
      commitments: [
        {
          claim: "all schema observations pass",
          evidence: "phase-oracle",
          id: "schema.all",
          match: "schema",
        },
      ],
      phases: [
        {
          act: [{ kind: "rpc_call", method: "appInfo" }],
          expect: [{ oracle: "schema", query: "appInfo" }],
          name: "first-schema",
        },
        {
          act: [{ kind: "rpc_call", method: "appInfo" }],
          expect: [{ oracle: "schema", query: "appInfo" }],
          name: "second-schema",
        },
      ],
    })
    const driver = makeKhalaCodeRpcQaDriver({
      fetch: (() => {
        calls += 1
        return Promise.resolve(
          calls === 1
            ? jsonResponse({
              app: "Khala Code Desktop",
              ok: true,
              observedAt: "2026-07-01T00:00:00.000Z",
            })
            : jsonResponse({
              app: "Khala Code Desktop",
              ok: true,
            }),
        )
      }) as KhalaCodeRpcFetch,
    })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario }))

    expect(report.status).toBe("fail")
    expect(report.phaseOutcomes.map((phase) => phase.oracles[0]?.ok)).toEqual([true, false])
    expect(report.commitments.findings[0]?.verdict).toBe("REFUTED")
    expect(report.commitments.findings[0]?.evidenceSummary).toContain("second-schema:schema=refuted")
    expect(report.commitments.verdict).toBe("REFUTED")
  })

  test("records boot failures as failed runs without synthesizing a handle", async () => {
    const scenario = loadKhalaCodeQaScenario({
      ...fixtureScenario,
      modes: ["dom"],
    })

    const report = await Effect.runPromise(
      runKhalaCodeQaScenario({
        driver: unsupportedKhalaCodeQaDriver("dom"),
        scenario,
      }),
    )

    expect(report.status).toBe("fail")
    expect(report.phaseOutcomes[0]).toMatchObject({
      name: "boot",
      status: "fail",
    })
    expect(report.phaseOutcomes[0]?.observations[0]?.error).toContain("driver is not implemented")
    expect(report.commitments.verdict).toBe("REFUTED")
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
