import { describe, expect, test } from "bun:test"
import { Effect, Stream } from "effect"

import {
  buildKhalaCodeQaSeededMonkeyPlan,
  collectKhalaCodeQaCoverageLedger,
  decodeKhalaCodeQaExplorerBrainDecision,
  distillKhalaCodeQaMonkeyReportToScenario,
  khalaCodeQaCoverageFrontierReport,
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  makeKhalaCodeQaDeterministicFixtureBrain,
  makeKhalaCodeQaLiveLlmExplorerBrain,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  regressKhalaCodeQaDistilledScenario,
  runKhalaCodeQaSeededMonkey,
  type KhalaCodeQaDriver,
} from "./index.js"

const makeFixtureDriver = () =>
  makeKhalaCodeRpcQaDriver({
    baseUrl: "http://fixture.local",
    fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
    now: () => "2026-07-02T00:00:00.000Z",
  })

const makeInvariantFailingDriver = (): KhalaCodeQaDriver => ({
  mode: "rpc",
  boot: (opts) =>
    Effect.succeed({
      backend: opts.backend,
      mode: "rpc",
      startedAt: "2026-07-02T00:00:00.000Z",
    }),
  act: (action) =>
    Effect.succeed({
      action,
      data: {
        run: {
          counters: { activeAssignments: 2 },
          runRef: "run-invariant-fail",
          targetConcurrency: 1,
        },
      },
      label: action.kind === "rpc_call" ? `rpc:${action.method}#1` : action.kind,
      ok: true,
    }),
  events: () => Stream.empty,
  metrics: () =>
    Effect.fail({
      _tag: "KhalaCodeQaDriverFailure" as const,
      message: "metrics unavailable in invariant failing fixture",
    }),
  read: (query) => Effect.succeed({ label: query, value: null }),
  shutdown: () => Effect.succeed({ refs: [] }),
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

  test("seeded monkey consumes ExplorerBrain.nextAction for each exploratory step", async () => {
    let calls = 0
    const report = await Effect.runPromise(
      runKhalaCodeQaSeededMonkey({
        brain: {
          nextAction: (context) => {
            calls += 1
            return Effect.succeed({
              action: context.actionSpace.find((action) => action.kind === "hotbar" && action.target === "fleet") ??
                context.actionSpace[0],
              frontierRef: "hotbarPanels:fleet",
              rationale: "test brain forced fleet",
              tier: "deterministic_fixture",
            })
          },
          tier: "deterministic_fixture",
        },
        driver: makeFixtureDriver(),
        options: {
          mode: "fixture_smoke",
          seed: "brain-loop",
          steps: 7,
        },
      }),
    )

    expect(calls).toBe(7)
    expect(report.actionLog.every((entry) => entry.rationale === "test brain forced fleet")).toBe(true)
    expect(report.coverageLedger.hotbarPanelsOpened).toContain("fleet")
  })

  test("frontier steering remains a seeded bias instead of replacing randomness", () => {
    const first = buildKhalaCodeQaSeededMonkeyPlan({
      mode: "fleet_cockpit_night",
      seed: "alpha",
      steps: 96,
    })
    const second = buildKhalaCodeQaSeededMonkeyPlan({
      mode: "fleet_cockpit_night",
      seed: "bravo",
      steps: 96,
    })

    expect(first.actionLog.map((entry) => entry.action)).not.toEqual(second.actionLog.map((entry) => entry.action))
    expect(first.actionLog.some((entry) => entry.frontierRef !== undefined)).toBe(true)
    expect(second.actionLog.some((entry) => entry.frontierRef !== undefined)).toBe(true)
  })

  test("fixture brain uses PRNG among frontier-matching candidates", async () => {
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
      { kind: "hotbar", target: "fleet" },
      { kind: "slash_command", value: "/status" },
      { kind: "click", target: ".khala-fleet-worker-card" },
    ] as const
    const brain = makeKhalaCodeQaDeterministicFixtureBrain({ epsilon: 0 })
    const decisions = await Promise.all([1, 2, 3, 4, 5].map((prngState) =>
      Effect.runPromise(
        brain.nextAction({
          actionLog: [],
          actionSpace,
          frontier,
          prngState,
          stepIndex: 0,
        }),
      )
    ))

    expect(new Set(decisions.map((decision) => JSON.stringify(decision.action))).size).toBeGreaterThan(1)
    expect(decisions.every((decision) => decision.frontierRef !== undefined)).toBe(true)
  })

  test("live LLM tier is disabled unless explicitly enabled and injected", async () => {
    const disabled = await Effect.runPromiseExit(
      makeKhalaCodeQaLiveLlmExplorerBrain({ env: {} }).nextAction({
        actionLog: [],
        actionSpace: [{ kind: "hotbar", target: "fleet" }],
        frontier: khalaCodeQaCoverageFrontierReport({
          ledger: collectKhalaCodeQaCoverageLedger({ observations: [], runId: "empty" }),
          manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
        }),
        prngState: 1,
        stepIndex: 0,
      }),
    )
    const injected = await Effect.runPromise(
      makeKhalaCodeQaLiveLlmExplorerBrain({
        decide: () =>
          Effect.succeed({
            action: { kind: "hotbar", target: "fleet" },
            rationale: "injected",
            tier: "live_llm",
          }),
        env: { KHALA_QA_LIVE_EXPLORER_BRAIN: "1" },
      }).nextAction({
        actionLog: [],
        actionSpace: [{ kind: "hotbar", target: "fleet" }],
        frontier: khalaCodeQaCoverageFrontierReport({
          ledger: collectKhalaCodeQaCoverageLedger({ observations: [], runId: "empty" }),
          manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
        }),
        prngState: 1,
        stepIndex: 0,
      }),
    )

    expect(disabled._tag).toBe("Failure")
    expect(injected.action).toEqual({ kind: "hotbar", target: "fleet" })
    expect(injected.tier).toBe("live_llm")
  })

  test("distills real failing run reports and preserves invariant oracles", async () => {
    const report = await Effect.runPromise(
      runKhalaCodeQaSeededMonkey({
        brain: {
          nextAction: () =>
            Effect.succeed({
              action: { args: [{ runRef: "run-invariant-fail" }], kind: "rpc_call", method: "fleetRunStatus" },
              rationale: "force invariant evidence",
              tier: "deterministic_fixture",
            }),
          tier: "deterministic_fixture",
        },
        driver: makeInvariantFailingDriver(),
        options: {
          mode: "fleet_cockpit_night",
          seed: "real-fail",
          steps: 5,
        },
      }),
    )
    const distilled = distillKhalaCodeQaMonkeyReportToScenario(report)
    const replay = await Effect.runPromise(
      regressKhalaCodeQaDistilledScenario({
        distilled,
        driver: makeInvariantFailingDriver(),
      }),
    )

    expect(report.status).toBe("fail")
    expect(distilled.sourceStatus).toBe("fail")
    expect(distilled.scenario.phases[0]?.expect).toContainEqual({ id: "claim-invariant", oracle: "invariant" })
    expect(replay.status).toBe("fail")
  })
})
