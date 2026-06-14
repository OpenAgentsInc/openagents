import { describe, expect, test } from "bun:test"
import {
  fetchTrainingRuns,
  planTrainingRunWindow,
} from "../src/bun/training-runs"

const sampleRun = {
  createdAtDisplay: "2 days ago",
  maxAllowedStale: 5,
  promiseRef: "pylon.first_real_model_training_run.v1",
  receiptRefs: ["receipt.1"],
  sealInFlight: false,
  sealPublicationCadenceWindows: 1,
  sourceRefs: ["route:/api/training/runs"],
  state: "planned",
  trainingRunRef: "run.cs336.a1.real_gradient.demo",
  updatedAtDisplay: "2 days ago",
}

const sampleSummary = {
  copyBoundaryRefs: ["no_unbounded_training_claim"],
  emptyState: { idle: false, reason: "" },
  metrics: {
    activeWindowCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    assignedContributorCount: { provenanceLabel: "", sourceRefs: [], value: 2 },
    pendingPayoutCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    plannedWindowCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    providerConfirmedSettledPayoutSats: {
      provenanceLabel: "",
      sourceRefs: [],
      value: 0,
    },
    receiptRefCount: { provenanceLabel: "", sourceRefs: ["receipt.1"], value: 1 },
    reconciledWindowCount: { provenanceLabel: "", sourceRefs: [], value: 2 },
    rejectedWorkCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    sealedWindowCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    verifiedWorkCount: { provenanceLabel: "", sourceRefs: [], value: 3 },
  },
  realGradient: {
    closeoutRequirement: {
      evalRef: "eval.1",
      freivaldsCommitmentRefs: ["freivalds.1"],
      gradientCloseoutRefs: ["closeout.1"],
      mergeRef: "merge.1",
      provenanceLabel: "",
      satisfied: true,
    },
    deviceRequirement: {
      observedDistinctContributorDevices: 2,
      provenanceLabel: "",
      requiredDistinctContributorDevices: 2,
      satisfied: true,
      sourceRefs: [],
    },
    externalAsk: {
      blockerRefs: [],
      psionicLaneRef: "psionic.lane.demo",
      requirementRefs: [],
      status: "observed",
    },
    lossUnderBudget: {
      budgetLabel: "tiny",
      budgetRef: "budget.1",
      finalValidationLoss: 3.1,
      maxValidationLoss: 4,
      provenanceLabel: "",
      satisfied: true,
      sourceRefs: [],
    },
    scopeBoundaryRefs: ["no_public_gradients"],
  },
  receiptRefs: ["receipt.1"],
  run: sampleRun,
  sourceRefs: ["route:/api/training/runs"],
  windows: [
    {
      datasetRefs: ["dataset.1"],
      homeworkKind: "cs336_a1_real_gradient",
      plannedAtDisplay: "2 days ago",
      priority: 10,
      receiptRefs: ["window.receipt.1"],
      sealMetadata: null,
      sourceRefs: [],
      state: "reconciled",
      trainingRunRef: sampleRun.trainingRunRef,
      updatedAtDisplay: "2 days ago",
      windowRef: "window.1",
    },
  ],
}

describe("fetchTrainingRuns", () => {
  test("decodes public run summaries", async () => {
    const result = await fetchTrainingRuns({
      baseUrl: "https://openagents.test",
      nowIso: () => "2026-06-14T00:00:00.000Z",
      fetchFn: async () =>
        new Response(JSON.stringify({ runs: [sampleRun], summaries: [sampleSummary] })),
    })

    expect(result.ok).toBe(true)
    expect(result.sourceUrl).toBe("https://openagents.test/api/training/runs")
    expect(result.runs).toHaveLength(1)
    expect(result.summaries).toHaveLength(1)
    expect(result.summaries[0]?.metrics.verifiedWorkCount.value).toBe(3)
    expect(
      result.summaries[0]?.realGradient.deviceRequirement.satisfied,
    ).toBe(true)
  })

  test("falls back to run-only summaries", async () => {
    const result = await fetchTrainingRuns({
      baseUrl: "https://openagents.test/",
      fetchFn: async () => new Response(JSON.stringify({ runs: [sampleRun] })),
    })

    expect(result.ok).toBe(true)
    expect(result.summaries).toHaveLength(1)
    expect(result.summaries[0]?.run.trainingRunRef).toBe(sampleRun.trainingRunRef)
    expect(result.summaries[0]?.metrics.receiptRefCount.value).toBe(1)
  })

  test("returns a typed error projection on HTTP failure", async () => {
    const result = await fetchTrainingRuns({
      baseUrl: "https://openagents.test",
      fetchFn: async () => new Response("nope", { status: 503 }),
    })

    expect(result.ok).toBe(false)
    expect(result.runs).toEqual([])
    expect(result.error).toBe("training runs 503")
  })
})

describe("planTrainingRunWindow", () => {
  test("does not call admin routes when disabled", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await planTrainingRunWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: false,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("disabled")
    expect(calls).toHaveLength(0)
  })

  test("does not call admin routes without a token", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await planTrainingRunWindow({
      adminToken: null,
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("admin_token_missing")
    expect(calls).toHaveLength(0)
  })

  test("plans a run and window through admin routes", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const expectedRunRef =
      "training.run.desktop.r1.2026.06.14t00.00.00.000z"
    const expectedWindowRef =
      "training.window.desktop.r1.2026.06.14t00.00.00.000z"
    const result = await planTrainingRunWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test/",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        if (String(url).endsWith("/api/training/runs")) {
          return new Response(
            JSON.stringify({
              run: { ...sampleRun, trainingRunRef: expectedRunRef },
            }),
          )
        }
        return new Response(
          JSON.stringify({
            window: {
              ...sampleSummary.windows[0],
              trainingRunRef: expectedRunRef,
              windowRef: expectedWindowRef,
            },
          }),
        )
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(true)
    expect(result.trainingRunRef).toBe(expectedRunRef)
    expect(result.windowRef).toBe(expectedWindowRef)
    expect(result.runPlanned).toBe(true)
    expect(result.windowPlanned).toBe(true)
    expect(calls.map(call => call.url)).toEqual([
      "https://openagents.test/api/training/runs",
      "https://openagents.test/api/training/windows/plan",
    ])
    expect((calls[0]?.init?.headers as Record<string, string>).authorization).toBe(
      "Bearer admin-token",
    )
    expect(
      JSON.parse(String(calls[1]?.init?.body)).trainingRunRef,
    ).toBe(expectedRunRef)
  })

  test("reports a partial plan if window planning fails", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await planTrainingRunWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        if (String(url).endsWith("/api/training/runs")) {
          return new Response(
            JSON.stringify({ run: { ...sampleRun, trainingRunRef: "run.ok" } }),
          )
        }
        return new Response(JSON.stringify({ reason: "window duplicate" }), {
          status: 409,
        })
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("window_plan_failed")
    expect(result.trainingRunRef).toBe("run.ok")
    expect(result.runPlanned).toBe(true)
    expect(result.windowPlanned).toBe(false)
    expect(result.error).toBe("window duplicate")
    expect(calls).toHaveLength(2)
  })
})
