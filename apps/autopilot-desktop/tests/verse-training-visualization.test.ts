import { describe, expect, test } from "bun:test"

import { projectPylonNetworkScene } from "../src/shared/pylon-network-scene"
import type {
  TrainingPromiseGatesResponse,
  TrainingPublicMetric,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
} from "../src/shared/rpc"
import {
  VERSE_TASSADAR_CORE_NODE_ID,
  VERSE_TRAINING_NODE_PREFIX,
  withVerseTrainingLayer,
} from "../src/shared/verse-training-visualization"
import { pylonNetworkVisualizationOptions } from "../src/ui/pylon-network-visualization"

const metric = (
  value: number,
  sourceRefs: readonly string[] = [],
): TrainingPublicMetric => ({
  provenanceLabel: "public projection",
  sourceRefs,
  value,
})

const summary = (
  overrides: Partial<TrainingRunSummaryRow> = {},
): TrainingRunSummaryRow => ({
  copyBoundaryRefs: ["copy.boundary.public"],
  emptyState: { idle: false, reason: "" },
  metrics: {
    activeWindowCount: metric(1, ["window.active.public"]),
    assignedContributorCount: metric(3, ["assignment.public.1"]),
    pendingPayoutCount: metric(0),
    plannedWindowCount: metric(1),
    providerConfirmedSettledPayoutSats: metric(21, ["settlement.public.1"]),
    receiptRefCount: metric(5, ["trace.public.1"]),
    reconciledWindowCount: metric(1),
    rejectedWorkCount: metric(1, ["verdict.rejected.public.1"]),
    sealedWindowCount: metric(1),
    verifiedWorkCount: metric(4, ["verdict.accepted.public.1"]),
  },
  realGradient: {
    closeoutRequirement: {
      evalRef: "eval.public.1",
      freivaldsCommitmentRefs: ["freivalds.public.1"],
      gradientCloseoutRefs: ["gradient.public.1"],
      mergeRef: "merge.public.1",
      provenanceLabel: "public closeout",
      satisfied: true,
    },
    deviceRequirement: {
      observedDistinctContributorDevices: 2,
      provenanceLabel: "public devices",
      requiredDistinctContributorDevices: 2,
      satisfied: true,
      sourceRefs: ["device.public.1"],
    },
    externalAsk: {
      blockerRefs: ["blocker.public.1"],
      psionicLaneRef: "psionic.public.1",
      requirementRefs: ["requirement.public.1"],
      status: "blocked",
    },
    lossUnderBudget: {
      budgetLabel: "A1",
      budgetRef: "loss-budget.public.1",
      finalValidationLoss: 1.7,
      maxValidationLoss: 2,
      provenanceLabel: "public loss",
      satisfied: true,
      sourceRefs: ["loss.public.1"],
    },
    scopeBoundaryRefs: ["scope.public.1"],
  },
  receiptRefs: ["receipt.public.1"],
  run: {
    createdAtDisplay: "2026-06-20",
    maxAllowedStale: 4,
    promiseRef: "training.first_real_model_training_run.v1",
    receiptRefs: ["run.receipt.public.1"],
    sealInFlight: false,
    sealPublicationCadenceWindows: 2,
    sourceRefs: ["run.source.public.1"],
    state: "active",
    trainingRunRef: "training.run.public.1",
    updatedAtDisplay: "2026-06-20",
  },
  sourceRefs: ["summary.public.1"],
  windows: [
    {
      datasetRefs: ["dataset.public.1"],
      homeworkKind: "cs336.a1",
      plannedAtDisplay: "2026-06-20",
      priority: 1,
      receiptRefs: ["window.receipt.public.1"],
      sealMetadata: null,
      sourceRefs: ["window.source.public.1"],
      state: "sealed",
      trainingRunRef: "training.run.public.1",
      updatedAtDisplay: "2026-06-20",
      windowRef: "window.public.1",
    },
  ],
  ...overrides,
})

const response = (row: TrainingRunSummaryRow): TrainingRunsResponse => ({
  fetchedAt: "2026-06-20T00:00:00.000Z",
  ok: true,
  runs: [row.run],
  sourceUrl: "https://openagents.test/api/public/training/runs",
  summaries: [row],
})

const gates = (
  blockerRefs: readonly string[] = [],
): TrainingPromiseGatesResponse => ({
  blockerRefs,
  fetchedAt: "2026-06-20T00:00:00.000Z",
  ok: true,
  promises: [],
  registryVersion: "test",
  sourceUrl: "https://openagents.test/api/public/product-promises",
  stateCounts: {
    degraded: 0,
    green: 0,
    planned: 0,
    red: 0,
    unknown: 0,
    withdrawn: 0,
    yellow: 0,
  },
})

describe("withVerseTrainingLayer (#5822)", () => {
  test("default Verse scene includes the Tassadar run core without fake motion", () => {
    const base = pylonNetworkVisualizationOptions(projectPylonNetworkScene(null))
    const out = withVerseTrainingLayer(base, {
      promiseGates: gates(["blocker.promise.public.1"]),
      trainingRuns: null,
    })

    expect(out.nodes?.find(node => node.id === VERSE_TASSADAR_CORE_NODE_ID)).toMatchObject({
      label: "Tassadar",
      status: "blocked",
    })
    expect(out.nodes?.some(node => node.id === "network")).toBe(false)
    expect(out.nodes?.find(node => node.id === `${VERSE_TRAINING_NODE_PREFIX}assignment`)).toBeDefined()
    expect(out.nodes?.find(node => node.id === `${VERSE_TRAINING_NODE_PREFIX}blocked`)?.detail).toContain(
      "blocker.promise.public.1",
    )
    expect(out.beams ?? []).toHaveLength(0)
    expect(out.bursts ?? []).toHaveLength(0)
    expect(out.motionPolicy?.evidence).toBe("required")
    expect(out.motionPolicy?.structuralEdges).toBe("static")
    expect(out.motionPolicy?.ambient).toBe("static")
    expect(out.worldLabelDensity).toBe("pylons")
    expect(out.keyboardTargeting).toEqual({
      enabled: true,
      maxTargets: 18,
    })
    expect(
      out.nodes?.filter(node => node.id.startsWith(VERSE_TRAINING_NODE_PREFIX)).every(node => (node.position?.[2] ?? 0) !== 0),
    ).toBe(true)
  })

  test("projects benchmark lifecycle nodes and evidence-bound motion from public refs", () => {
    const base = pylonNetworkVisualizationOptions(
      projectPylonNetworkScene({
        available: true,
        status: "live",
        pylonsOnlineNow: 2,
        recentPylons: [
          { nostrPubkeyShort: "pylon-alpha", onlineNow: true, assignmentReadyNow: true },
          { nostrPubkeyShort: "pylon-beta", onlineNow: true, walletReadyNow: true },
        ],
      }),
    )
    const out = withVerseTrainingLayer(base, { trainingRuns: response(summary()) })
    const node = (id: string) => out.nodes?.find(candidate => candidate.id === id)

    expect(node(VERSE_TASSADAR_CORE_NODE_ID)?.detail).toContain("training.run.public.1")
    expect(node("pylon-alpha")?.connectedTo).toContain(VERSE_TASSADAR_CORE_NODE_ID)
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}assignment`)?.status).toBe("active")
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}trace`)?.status).toBe("sync")
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}replay`)?.status).toBe("verified")
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}verdict`)?.status).toBe("blocked")
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}settlement`)?.status).toBe("sealed")
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}recipient-confirmed`)?.status).toBe("verified")
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}blocked`)?.detail).toContain("requirement.public.1")
    expect(node(`${VERSE_TRAINING_NODE_PREFIX}replay`)?.position?.[2]).toBeGreaterThan(
      node(`${VERSE_TRAINING_NODE_PREFIX}assignment`)?.position?.[2] ?? 0,
    )

    const kinds = new Set((out.beams ?? []).map(beam => beam.motionKind))
    expect(kinds).toEqual(
      new Set([
        "assignment",
        "trace_submitted",
        "replay_verified",
        "replay_rejected",
        "settlement_recorded",
        "real_bitcoin_moved",
      ]),
    )
    expect((out.beams ?? []).every(beam => (beam.sourceRefs ?? []).length > 0)).toBe(true)
    expect((out.bursts ?? []).every(burst => (burst.sourceRefs ?? []).length > 0)).toBe(true)
  })

  test("does not emit training beams or bursts when source refs are absent", () => {
    const row = summary({
      metrics: {
        activeWindowCount: metric(1),
        assignedContributorCount: metric(3),
        pendingPayoutCount: metric(0),
        plannedWindowCount: metric(1),
        providerConfirmedSettledPayoutSats: metric(21),
        receiptRefCount: metric(5),
        reconciledWindowCount: metric(1),
        rejectedWorkCount: metric(0),
        sealedWindowCount: metric(1),
        verifiedWorkCount: metric(4),
      },
      realGradient: {
        closeoutRequirement: {
          evalRef: null,
          freivaldsCommitmentRefs: [],
          gradientCloseoutRefs: [],
          mergeRef: null,
          provenanceLabel: "public closeout",
          satisfied: true,
        },
        deviceRequirement: {
          observedDistinctContributorDevices: 2,
          provenanceLabel: "public devices",
          requiredDistinctContributorDevices: 2,
          satisfied: true,
          sourceRefs: [],
        },
        externalAsk: {
          blockerRefs: [],
          psionicLaneRef: "psionic.public.1",
          requirementRefs: [],
          status: "ready",
        },
        lossUnderBudget: {
          budgetLabel: "A1",
          budgetRef: null,
          finalValidationLoss: 1.7,
          maxValidationLoss: 2,
          provenanceLabel: "public loss",
          satisfied: true,
          sourceRefs: [],
        },
        scopeBoundaryRefs: [],
      },
      receiptRefs: [],
      run: {
        ...summary().run,
        receiptRefs: [],
        sourceRefs: [],
      },
      sourceRefs: [],
      windows: [
        {
          ...summary().windows[0]!,
          receiptRefs: [],
          sourceRefs: [],
        },
      ],
    })

    const out = withVerseTrainingLayer(
      pylonNetworkVisualizationOptions(projectPylonNetworkScene(null)),
      { trainingRuns: response(row) },
    )

    expect(out.beams ?? []).toHaveLength(0)
    expect(out.bursts ?? []).toHaveLength(0)
    expect(out.motionPolicy?.evidence).toBe("required")
  })
})
