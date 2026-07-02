import { createServer } from "node:net"

import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  assertKhalaQaVisibleRect,
  buildKhalaCodeQaShutdownOracle,
  decodeKhalaCodeQaScenario,
  findKhalaQaAvailablePort,
  khalaQaRectsOverlap,
  loadKhalaCodeQaScenario,
  makeKhalaCodeRpcQaDriver,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  runKhalaCodeQaScenario,
  unsupportedKhalaCodeQaDriver,
  waitForKhalaQaHttp,
  type KhalaCodeRpcFetch,
} from "./index.js"
import {
  khalaCodeQaMetricBudgets,
  khalaCodeQaMetricDefinitions,
  khalaCodeQaMetricUnitFor,
} from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"

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

const methodName = (input: RequestInfo | URL): string =>
  new URL(String(input)).pathname.split("/").pop() ?? ""

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
    expect(report.shutdownOracle.status).toBe("pass")
  })

  test("refutes run-pass commitments when driver shutdown leaves an orphan process", async () => {
    const baseDriver = makeKhalaCodeRpcQaDriver({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
        }))) as KhalaCodeRpcFetch,
    })
    const driver = {
      act: baseDriver.act.bind(baseDriver),
      boot: baseDriver.boot.bind(baseDriver),
      events: baseDriver.events.bind(baseDriver),
      metrics: baseDriver.metrics.bind(baseDriver),
      mode: baseDriver.mode,
      read: baseDriver.read.bind(baseDriver),
      shutdown: () =>
        Effect.succeed({
          refs: [],
          shutdownOracle: buildKhalaCodeQaShutdownOracle({
            observedAt: "2026-07-01T00:00:01.000Z",
            orphanProcesses: [{ command: "fixture-child", pid: 44, reason: "still running" }],
          }),
        }),
    }

    const report = await Effect.runPromise(
      runKhalaCodeQaScenario({
        driver,
        scenario: loadKhalaCodeQaScenario(fixtureScenario),
      }),
    )

    expect(report.status).toBe("fail")
    expect(report.shutdownOracle).toMatchObject({
      actualOrphans: 1,
      status: "fail",
    })
    expect(report.commitments.verdict).toBe("REFUTED")
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

  test("evaluates perf budgets from fixture qaMetrics samples", async () => {
    const scenario = loadKhalaCodeQaScenario({
      ...fixtureScenario,
      commitments: [
        {
          claim: "cockpit fixture render stays within budget",
          evidence: "phase-oracle",
          id: "perf.cockpit",
          match: "metrics:perf",
        },
      ],
      phases: [
        {
          act: [{ kind: "rpc_call", method: "qaMetrics" }],
          expect: [
            {
              match: "budget.khala_code.cockpit_render.50_cards.v1",
              oracle: "perf",
              query: "qaMetrics",
            },
          ],
          name: "metrics",
        },
      ],
    })
    const driver = makeKhalaCodeRpcQaDriver({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          budgets: [
            {
              budgetId: "budget.khala_code.cockpit_render.50_cards.v1",
              description: "Cockpit renders within 100ms with 50 worker cards.",
              metric: "cockpit.render_ms",
              operator: "lte",
              requiredContext: { cards: 50 },
              threshold: 100,
              unit: "ms",
            },
          ],
          definitions: [
            {
              description: "Fleet cockpit render duration.",
              kind: "timer",
              name: "cockpit.render_ms",
              unit: "ms",
            },
          ],
          evaluations: [],
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
          samples: [
            {
              context: { cards: 50 },
              metric: "cockpit.render_ms",
              observedAt: "2026-07-01T00:00:00.000Z",
              unit: "ms",
              value: 82,
            },
          ],
          schema: "openagents.khala_code.qa_metrics.v1",
        }))) as KhalaCodeRpcFetch,
    })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario }))

    expect(report.status).toBe("pass")
    expect(report.phaseOutcomes[0]?.oracles[0]).toMatchObject({
      ok: true,
      oracle: "perf",
      verdict: "CONFIRMED",
    })
    expect(report.commitments.verdict).toBe("CONFIRMED")
  })

  test("refutes perf budget failures from fixture qaMetrics samples", async () => {
    const scenario = loadKhalaCodeQaScenario({
      ...fixtureScenario,
      phases: [
        {
          act: [{ kind: "rpc_call", method: "qaMetrics" }],
          expect: [
            {
              metric: "lifecycle_event_to_card.ms",
              oracle: "perf",
              query: "qaMetrics",
            },
          ],
          name: "metrics",
        },
      ],
    })
    const driver = makeKhalaCodeRpcQaDriver({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          budgets: [
            {
              budgetId: "budget.khala_code.lifecycle_event_to_card.p95.v1",
              description: "Lifecycle event to worker card p95 stays below 500ms.",
              metric: "lifecycle_event_to_card.ms",
              operator: "lte",
              percentile: 95,
              threshold: 500,
              unit: "ms",
            },
          ],
          definitions: [
            {
              description: "Fleet lifecycle event to visible worker-card duration.",
              kind: "timer",
              name: "lifecycle_event_to_card.ms",
              unit: "ms",
            },
          ],
          evaluations: [],
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
          samples: [490, 520, 610].map((value) => ({
            metric: "lifecycle_event_to_card.ms",
            observedAt: "2026-07-01T00:00:00.000Z",
            unit: "ms",
            value,
          })),
          schema: "openagents.khala_code.qa_metrics.v1",
        }))) as KhalaCodeRpcFetch,
    })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario }))

    expect(report.status).toBe("fail")
    expect(report.phaseOutcomes[0]?.oracles[0]).toMatchObject({
      ok: false,
      oracle: "perf",
      verdict: "REFUTED",
    })
  })

  test("consumes the full Q2 latency budget catalog through qaMetrics perf oracles", async () => {
    const q2BudgetIds = [
      "budget.khala_code.startup_interactive.v1",
      "budget.khala_code.thread_switch.optimistic.v1",
      "budget.khala_code.thread_switch.full.v1",
      "budget.khala_code.turn_start.first_event.v1",
      "budget.khala_code.composer.keystroke_echo.p95.v1",
      "budget.khala_code.panel.open.v1",
      "budget.khala_code.sse.event_to_ui.p95.v1",
      "budget.khala_code.transcript.scroll_dropped_frames.v1",
      "budget.khala_code.app_server.spawn_ready.v1",
    ]
    const scenario = loadKhalaCodeQaScenario({
      ...fixtureScenario,
      commitments: [
        {
          claim: "Q2 latency budgets evaluate from qaMetrics",
          evidence: "phase-oracle",
          id: "perf.q2_catalog",
          match: "metrics:perf",
        },
      ],
      phases: [
        {
          act: [],
          expect: q2BudgetIds.map(budgetId => ({
            match: budgetId,
            oracle: "perf",
          })),
          name: "metrics",
        },
      ],
    })
    const samples = khalaCodeQaMetricBudgets.map(budget => ({
      ...(budget.requiredContext === undefined ? {} : { context: budget.requiredContext }),
      metric: budget.metric,
      observedAt: "2026-07-02T12:00:00.000Z",
      unit: khalaCodeQaMetricUnitFor(budget.metric),
      value: budget.threshold,
    }))
    const driver = makeKhalaCodeRpcQaDriver({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          budgets: khalaCodeQaMetricBudgets,
          definitions: khalaCodeQaMetricDefinitions,
          evaluations: [],
          ok: true,
          observedAt: "2026-07-02T12:00:00.000Z",
          samples,
          schema: "openagents.khala_code.qa_metrics.v1",
        }))) as KhalaCodeRpcFetch,
    })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario }))

    expect(report.status).toBe("pass")
    expect(report.phaseOutcomes[0]?.oracles).toHaveLength(q2BudgetIds.length)
    expect(report.phaseOutcomes[0]?.oracles.every(oracle => oracle.verdict === "CONFIRMED"))
      .toBe(true)
    expect(report.commitments.verdict).toBe("CONFIRMED")
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

  test("refutes consistency when successive indexed RPC reads differ", async () => {
    let calls = 0
    const scenario = loadKhalaCodeQaScenario({
      backend: "fixture",
      commitments: [
        {
          claim: "two indexed appInfo reads stay consistent",
          evidence: "phase-oracle",
          id: "consistency.app_info",
          match: "read-twice:consistency",
        },
      ],
      id: "scenario.khala_code.fixture_rpc_app_info_consistency.v1",
      modes: ["rpc"],
      phases: [
        {
          act: [
            { kind: "rpc_call", method: "appInfo" },
            { kind: "rpc_call", method: "appInfo" },
          ],
          expect: [
            {
              left: "rpc:appInfo#1",
              oracle: "consistency",
              right: "rpc:appInfo#2",
            },
          ],
          name: "read-twice",
        },
      ],
    })
    const driver = makeKhalaCodeRpcQaDriver({
      fetch: (() => {
        calls += 1
        return Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
          observedAt: calls === 1
            ? "2026-07-01T00:00:00.000Z"
            : "2026-07-01T00:00:01.000Z",
        }))
      }) as KhalaCodeRpcFetch,
    })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario }))

    expect(report.phaseOutcomes[0]?.observations.map((observation) => observation.label)).toEqual([
      "rpc:appInfo#1",
      "rpc:appInfo#2",
    ])
    expect(report.phaseOutcomes[0]?.oracles[0]).toMatchObject({
      ok: false,
      oracle: "consistency",
      verdict: "REFUTED",
    })
    expect(report.phaseOutcomes[0]?.oracles[0]?.summary).toContain("observedAt")
    expect(report.status).toBe("fail")
    expect(report.commitments.verdict).toBe("REFUTED")
  })

  test("does not refute claim invariant when the same claimant is re-observed", async () => {
    const scenario = loadKhalaCodeQaScenario({
      backend: "fixture",
      commitments: [{ claim: "claim invariant holds", evidence: "run-pass", id: "claim.pass" }],
      id: "scenario.khala_code.fixture_claim_reobservation.v1",
      modes: ["rpc"],
      phases: [{
        act: [
          { kind: "rpc_call", method: "codexFleetDelegateRun", args: [{ objective: "fixture", mode: "fixture", count: 1, noRun: true }] },
          { kind: "rpc_call", method: "codexFleetDelegateRun", args: [{ objective: "fixture", mode: "fixture", count: 1, noRun: true }] },
        ],
        expect: [{ id: "claim-invariant", oracle: "invariant" }, { oracle: "crash" }],
        name: "claims",
      }],
    })
    const driver = makeKhalaCodeRpcQaDriver({ fetch: makeKhalaCodeQaSeedCorpusFixtureFetch() })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario }))

    expect(report.status).toBe("pass")
    expect(report.phaseOutcomes[0]?.oracles[0]).toMatchObject({
      ok: true,
      oracle: "invariant",
      verdict: "CONFIRMED",
    })
  })

  test("refutes claim invariant for duplicate refs within one observation or conflicting claimants", async () => {
    const baseFetch = makeKhalaCodeQaSeedCorpusFixtureFetch()
    let call = 0
    const fetch = (async (input, init) => {
      if (methodName(input) !== "codexFleetDelegateRun") return baseFetch(input, init)
      call += 1
      const response = await baseFetch(input, init)
      const payload = await response.json() as Record<string, unknown>
      const results = Array.isArray(payload.results) ? payload.results : []
      if (call === 1) return jsonResponse({ ...payload, results: [results[0], results[0]] })
      return jsonResponse({
        ...payload,
        results: [{
          ...(results[0] as Record<string, unknown>),
          accountRef: "codex-2",
        }],
      })
    }) as KhalaCodeRpcFetch
    const scenario = loadKhalaCodeQaScenario({
      backend: "fixture",
      commitments: [{ claim: "claim invariant fails", evidence: "run-pass", id: "claim.fail" }],
      id: "scenario.khala_code.fixture_claim_duplicate.v1",
      modes: ["rpc"],
      phases: [{
        act: [
          { kind: "rpc_call", method: "codexFleetDelegateRun", args: [{ objective: "fixture", mode: "fixture", count: 1, noRun: true }] },
          { kind: "rpc_call", method: "codexFleetDelegateRun", args: [{ objective: "fixture", mode: "fixture", count: 1, noRun: true }] },
        ],
        expect: [{ id: "claim-invariant", oracle: "invariant" }],
        name: "claims",
      }],
    })

    const report = await Effect.runPromise(runKhalaCodeQaScenario({
      driver: makeKhalaCodeRpcQaDriver({ fetch }),
      scenario,
    }))

    expect(report.status).toBe("fail")
    expect(report.phaseOutcomes[0]?.oracles[0]).toMatchObject({
      data: {
        conflictingClaimants: [{ assignmentRef: "assignment-fixture", claimants: ["codex", "codex-2"] }],
        duplicateRefs: ["assignment-fixture"],
      },
      ok: false,
      oracle: "invariant",
      verdict: "REFUTED",
    })
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
    let now = 0
    await waitForKhalaQaHttp("http://fixture.local", {
      fetch: (() => {
        calls += 1
        return Promise.resolve(new Response("ok", { status: calls === 1 ? 503 : 200 }))
      }) as unknown as typeof fetch,
      intervalMs: 1,
      now: () => now,
      sleep: async ms => {
        now += ms
      },
      timeoutMs: 100,
    })

    expect(calls).toBe(2)
  })

  test("findKhalaQaAvailablePort falls back when the preferred port is occupied", async () => {
    const server = createServer()
    await new Promise<void>((resolveListen, rejectListen) => {
      server.once("error", rejectListen)
      server.listen(0, "127.0.0.1", () => resolveListen())
    })
    try {
      const address = server.address()
      if (typeof address !== "object" || address === null) throw new Error("missing test port")
      const fallbackPort = await findKhalaQaAvailablePort(address.port)
      expect(fallbackPort).not.toBe(address.port)
      expect(fallbackPort).toBeGreaterThan(0)
    } finally {
      await new Promise<void>(resolveClose => server.close(() => resolveClose()))
    }
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
