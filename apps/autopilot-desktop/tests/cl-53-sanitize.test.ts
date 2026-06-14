import { describe, expect, test } from "bun:test"
import { initialModel } from "../src/ui/model"
import { sanitizeTree, view } from "../src/ui/view"

// Regression for the blank-screen crash: Foldkit's element constructors strip
// `null` children but NOT `undefined`/`false`, so such a child reaches
// `dedupeSharedVNodes` which does `child.children` and throws
// "undefined is not an object". `sanitizeTree` drops those before the runtime
// patches, so the view can never blank-screen on a stray falsy child.

const vnode = (children: unknown[]): { sel: string; children: unknown[] } => ({
  sel: "div",
  children,
})

const treeContainsSelector = (node: unknown, selector: string): boolean => {
  if (node === null || typeof node !== "object") return false
  const vnode = node as { sel?: string; children?: unknown[] }
  if (vnode.sel === selector) return true
  return Array.isArray(vnode.children)
    ? vnode.children.some((child) => treeContainsSelector(child, selector))
    : false
}

const treeContainsClass = (node: unknown, className: string): boolean => {
  if (node === null || typeof node !== "object") return false
  const vnode = node as {
    children?: unknown[]
    data?: { class?: Record<string, boolean> }
  }
  if (vnode.data?.class?.[className]) return true
  return Array.isArray(vnode.children)
    ? vnode.children.some((child) => treeContainsClass(child, className))
    : false
}

const metric = (value: number) => ({
  provenanceLabel: "",
  sourceRefs: [],
  value,
})

const liveTrainingProjection = {
  ok: true,
  fetchedAt: "2026-06-14T00:00:00.000Z",
  sourceUrl: "https://openagents.test/api/training/runs",
  runs: [
    {
      createdAtDisplay: "today",
      maxAllowedStale: 5,
      promiseRef: "pylon.first_real_model_training_run.v1",
      receiptRefs: ["receipt.1"],
      sealInFlight: false,
      sealPublicationCadenceWindows: 1,
      sourceRefs: [],
      state: "planned",
      trainingRunRef: "run.demo",
      updatedAtDisplay: "today",
    },
  ],
  summaries: [
    {
      copyBoundaryRefs: [],
      emptyState: { idle: false, reason: "" },
      metrics: {
        activeWindowCount: metric(0),
        assignedContributorCount: metric(2),
        pendingPayoutCount: metric(0),
        plannedWindowCount: metric(1),
        providerConfirmedSettledPayoutSats: metric(0),
        receiptRefCount: metric(1),
        reconciledWindowCount: metric(0),
        rejectedWorkCount: metric(0),
        sealedWindowCount: metric(0),
        verifiedWorkCount: metric(3),
      },
      realGradient: {
        closeoutRequirement: {
          evalRef: null,
          freivaldsCommitmentRefs: ["freivalds.1"],
          gradientCloseoutRefs: ["closeout.1"],
          mergeRef: null,
          provenanceLabel: "",
          satisfied: false,
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
          budgetRef: null,
          finalValidationLoss: null,
          maxValidationLoss: null,
          provenanceLabel: "",
          satisfied: false,
          sourceRefs: [],
        },
        scopeBoundaryRefs: [],
      },
      receiptRefs: ["receipt.1"],
      run: {
        createdAtDisplay: "today",
        maxAllowedStale: 5,
        promiseRef: "pylon.first_real_model_training_run.v1",
        receiptRefs: ["receipt.1"],
        sealInFlight: false,
        sealPublicationCadenceWindows: 1,
        sourceRefs: [],
        state: "planned",
        trainingRunRef: "run.demo",
        updatedAtDisplay: "today",
      },
      sourceRefs: [],
      windows: [],
    },
  ],
}

describe("CL-53 sanitizeTree", () => {
  test("drops undefined / null / false children", () => {
    const out = sanitizeTree(
      vnode(["text", undefined, null, false, vnode(["ok"])]),
    ) as { children: unknown[] }
    expect(out.children).toHaveLength(2)
    expect(out.children[0]).toBe("text")
    expect((out.children[1] as { children: unknown[] }).children).toEqual(["ok"])
  })

  test("recurses into nested children", () => {
    const out = sanitizeTree(
      vnode([vnode(["a", undefined, vnode([false, "b"])])]),
    ) as { children: Array<{ children: unknown[] }> }
    const inner = out.children[0]
    expect(inner.children).toHaveLength(2) // "a" + nested vnode
    const deepest = inner.children[1] as { children: unknown[] }
    expect(deepest.children).toEqual(["b"]) // false dropped
  })

  test("leaves strings and childless nodes untouched", () => {
    expect(sanitizeTree("hello")).toBe("hello")
    expect(sanitizeTree({ sel: "br" })).toEqual({ sel: "br" })
  })

  test("nodes home includes the three-effect scenes", () => {
    const document = view(initialModel)
    expect(treeContainsSelector(document.body, "oa-spinning-cube")).toBe(true)
    expect(treeContainsSelector(document.body, "oa-bezier-nodes")).toBe(true)
  })

  test("training pane includes the training scene", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsSelector(document.body, "oa-training-run")).toBe(true)
  })

  test("training pane renders live run projection rows", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
      trainingRunsStatus: { text: "1 run", tone: "success" },
    })
    expect(treeContainsClass(document.body, "training-run-row")).toBe(true)
  })
})
