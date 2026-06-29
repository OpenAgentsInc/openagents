import { describe, expect, test } from "bun:test"

import type {
  TrainingPromiseGatesResponse,
  TrainingPublicMetric,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
} from "../src/shared/rpc"
import { verseRunHudProjection } from "../src/shared/verse-run-hud"

const metric = (
  value: number,
  sourceRefs: readonly string[] = [],
): TrainingPublicMetric => ({
  provenanceLabel: "public projection",
  sourceRefs,
  value,
})

const summary = (): TrainingRunSummaryRow => ({
  copyBoundaryRefs: ["copy.public"],
  emptyState: { idle: false, reason: "" },
  metrics: {
    activeWindowCount: metric(1),
    assignedContributorCount: metric(5, ["assignment.public"]),
    pendingPayoutCount: metric(2, ["payout.public"]),
    plannedWindowCount: metric(1),
    providerConfirmedSettledPayoutSats: metric(21_000, ["settlement.public"]),
    receiptRefCount: metric(8, ["trace.public"]),
    reconciledWindowCount: metric(0),
    rejectedWorkCount: metric(1, ["reject.public"]),
    sealedWindowCount: metric(1),
    verifiedWorkCount: metric(7, ["accept.public"]),
  },
  realGradient: {
    closeoutRequirement: {
      evalRef: "eval.public",
      freivaldsCommitmentRefs: ["freivalds.public"],
      gradientCloseoutRefs: ["gradient.public"],
      mergeRef: "merge.public",
      provenanceLabel: "public closeout",
      satisfied: true,
    },
    deviceRequirement: {
      observedDistinctContributorDevices: 3,
      provenanceLabel: "public devices",
      requiredDistinctContributorDevices: 2,
      satisfied: true,
      sourceRefs: ["device.public"],
    },
    externalAsk: {
      blockerRefs: ["blocker.external.public"],
      psionicLaneRef: "psionic.public",
      requirementRefs: [],
      status: "blocked",
    },
    lossUnderBudget: {
      budgetLabel: "A1",
      budgetRef: "loss.budget.public",
      finalValidationLoss: 1.23,
      maxValidationLoss: 2,
      provenanceLabel: "public loss",
      satisfied: true,
      sourceRefs: ["loss.public"],
    },
    scopeBoundaryRefs: ["scope.public"],
  },
  receiptRefs: ["receipt.public"],
  run: {
    createdAtDisplay: "2026-06-20",
    maxAllowedStale: 4,
    promiseRef: "training.first_real_model_training_run.v1",
    receiptRefs: ["run.receipt.public"],
    sealInFlight: false,
    sealPublicationCadenceWindows: 2,
    sourceRefs: ["run.public"],
    state: "active",
    trainingRunRef: "training.run.public.very-long-reference",
    updatedAtDisplay: "2026-06-20",
  },
  sourceRefs: ["summary.public"],
  windows: [],
})

const runs = (): TrainingRunsResponse => {
  const row = summary()
  return {
    fetchedAt: "2026-06-20T00:00:00.000Z",
    ok: true,
    runs: [row.run],
    sourceUrl: "https://openagents.test/api/public/training/runs",
    summaries: [row],
  }
}

const gates = (): TrainingPromiseGatesResponse => ({
  blockerRefs: ["blocker.promise.public"],
  fetchedAt: "2026-06-20T00:00:00.000Z",
  ok: true,
  promises: [
    {
      blockerRefs: [],
      claim: "claim",
      evidenceRefCount: 1,
      productArea: "training",
      promiseId: "promise.green",
      safeCopy: "green",
      state: "green",
      verification: "public",
    },
    {
      blockerRefs: ["blocker.promise.public"],
      claim: "claim",
      evidenceRefCount: 0,
      productArea: "training",
      promiseId: "promise.yellow",
      safeCopy: "yellow",
      state: "yellow",
      verification: "public",
    },
  ],
  registryVersion: "test",
  sourceUrl: "https://openagents.test/api/public/product-promises",
  stateCounts: {
    degraded: 0,
    green: 1,
    planned: 0,
    red: 0,
    unknown: 0,
    withdrawn: 0,
    yellow: 1,
  },
})

describe("verseRunHudProjection", () => {
  test("turns public training/task signals into bounded compact HUD samples", () => {
    const projection = verseRunHudProjection(runs(), gates())

    expect(projection.state).toBe("blocked")
    expect(projection.runRef).toBe("ery-long-reference")
    expect(projection.lossLabel).toBe("1.23 / 2.00")
    expect(projection.promiseGreenCount).toBe(1)
    expect(projection.promiseTotalCount).toBe(2)
    expect(projection.blockerCount).toBe(2)
    expect(projection.samples.map(sample => sample.id)).toEqual([
      "assign",
      "trace",
      "accept",
      "reject",
      "proof",
      "settle",
      "payout",
    ])
    expect(projection.samples.every(sample => sample.value >= 0 && sample.value <= 1)).toBe(true)
    expect(projection.samples.find(sample => sample.id === "trace")?.sourceRefs).toEqual([
      "trace.public",
    ])
    expect(projection.samples.find(sample => sample.id === "settle")?.valueText).toBe(
      "21,000 sats",
    )
  })

  test("prefers the live Tassadar summary over stale real-gradient gate status", () => {
    const staleRuns = runs()
    const projection = verseRunHudProjection(
      {
        ...staleRuns,
        tassadarSummary: {
          generatedAt: "2026-06-21T19:56:21.452Z",
          metrics: {
            acceptedTraceCount: { value: 893 },
            activeWindowCount: { value: 21 },
            assignedContributorCount: { value: 11 },
            providerConfirmedSettledPayoutSats: { value: 1020 },
            rejectedWorkCount: { value: 3 },
            verifiedWorkCount: { value: 12 },
          },
          runRef: "run.tassadar.executor.20260615",
          runState: "active",
          sourceRefs: ["training.run.run.tassadar.executor.20260615"],
        },
      },
      gates(),
    )

    expect(projection.state).toBe("active")
    expect(projection.blockerCount).toBe(0)
    expect(projection.runRef).toBe("executor.20260615")
    expect(projection.lossLabel).toBe("21 active")
    expect(projection.samples.find(sample => sample.id === "assign")?.valueText).toBe("11")
    expect(projection.samples.find(sample => sample.id === "trace")?.valueText).toBe("893")
    expect(projection.samples.find(sample => sample.id === "settle")?.valueText).toBe(
      "1,020 sats",
    )
  })

  test("uses the live Tassadar summary even when the legacy aggregate timed out", () => {
    const projection = verseRunHudProjection(
      {
        fetchedAt: "2026-06-21T19:56:21.452Z",
        ok: false,
        sourceUrl: "https://openagents.test/api/training/runs",
        runs: [],
        summaries: [],
        error: "training runs timeout",
        tassadarSummary: {
          metrics: {
            activeWindowCount: { value: 21 },
            assignedContributorCount: { value: 11 },
          },
          runRef: "run.tassadar.executor.20260615",
          runState: "active",
        },
      },
      null,
    )

    expect(projection.state).toBe("active")
    expect(projection.runRef).toBe("executor.20260615")
    expect(projection.samples.find(sample => sample.id === "assign")?.valueText).toBe("11")
  })

  test("does not treat external requirements as blockers in the fallback HUD", () => {
    const row = summary()
    const projection = verseRunHudProjection(
      {
        ...runs(),
        summaries: [
          {
            ...row,
            realGradient: {
              ...row.realGradient,
              externalAsk: {
                blockerRefs: [],
                psionicLaneRef: "psionic.public",
                requirementRefs: ["requirement.public"],
                status: "observed",
              },
            },
          },
        ],
      },
      null,
    )

    expect(projection.state).toBe("active")
    expect(projection.blockerCount).toBe(0)
  })

  test("stays honest on cold start", () => {
    const projection = verseRunHudProjection(null, null)

    expect(projection.state).toBe("waiting")
    expect(projection.runRef).toBe("waiting")
    expect(projection.lossLabel).toBe("loss n/a")
    expect(projection.samples.every(sample => sample.value === 0)).toBe(true)
  })
})
