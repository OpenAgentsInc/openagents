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

const findSelectorNode = (node: unknown, selector: string): unknown | null => {
  if (node === null || typeof node !== "object") return null
  const vnode = node as { sel?: string; children?: unknown[] }
  if (vnode.sel === selector) return vnode
  if (!Array.isArray(vnode.children)) return null
  for (const child of vnode.children) {
    const match = findSelectorNode(child, selector)
    if (match !== null) return match
  }
  return null
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

const treeContainsText = (node: unknown, text: string): boolean => {
  if (typeof node === "string") return node.includes(text)
  if (node === null || typeof node !== "object") return false
  const vnode = node as { children?: unknown[]; text?: unknown }
  if (typeof vnode.text === "string" && vnode.text.includes(text)) return true
  return Array.isArray(vnode.children)
    ? vnode.children.some((child) => treeContainsText(child, text))
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

  test("nodes home excludes the three-effect demo scenes (launch cleanup #5020)", () => {
    const document = view(initialModel)
    expect(treeContainsSelector(document.body, "oa-spinning-cube")).toBe(false)
    expect(treeContainsSelector(document.body, "oa-bezier-nodes")).toBe(false)
  })

  test("nodes home excludes the three-effect source-map card (launch cleanup #5020)", () => {
    const document = view(initialModel)
    expect(treeContainsClass(document.body, "three-effect-source-list")).toBe(false)
    expect(treeContainsText(document.body, "examples/bezier-nodes")).toBe(false)
    expect(
      treeContainsText(
        document.body,
        "projects/repos/examples/demos/bezier-curves-and-nodes/src/Nodes.jsx",
      ),
    ).toBe(false)
    expect(
      treeContainsText(
        document.body,
        "projects/repos/drei/src/core/QuadraticBezierLine.tsx",
      ),
    ).toBe(false)
  })

  test("nodes home shows the node-launch status badge when set, hides it when null (#5025)", () => {
    // #5049: the default landing pane is now "network"; the node-launch badge is
    // a Nodes-pane concern, so target that pane explicitly.
    const hidden = view({ ...initialModel, pane: "nodes", nodeLaunchStatus: null })
    expect(treeContainsClass(hidden.body, "node-launch-badge")).toBe(false)

    const launching = view({ ...initialModel, pane: "nodes", nodeLaunchStatus: "launching" })
    expect(treeContainsClass(launching.body, "node-launch-badge")).toBe(true)
    expect(treeContainsClass(launching.body, "node-launch-launching")).toBe(true)
    expect(treeContainsText(launching.body, "Launching local node…")).toBe(true)

    const failed = view({ ...initialModel, pane: "nodes", nodeLaunchStatus: "failed" })
    expect(treeContainsClass(failed.body, "node-launch-failed")).toBe(true)
    expect(treeContainsText(failed.body, "Local node failed to start")).toBe(true)
  })

  test("network home is immersive: the scene canvas, no sidebar (#5049)", () => {
    const document = view(initialModel) // default pane is "network"
    expect(treeContainsSelector(document.body, "oa-training-run")).toBe(true)
    expect(treeContainsClass(document.body, "network-overlay")).toBe(true)
    expect(treeContainsClass(document.body, "app-shell-network")).toBe(true)
    // immersive: no sidebar chrome on the home
    expect(treeContainsClass(document.body, "sidebar")).toBe(false)
  })

  test("training pane includes the training scene", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsSelector(document.body, "oa-training-run")).toBe(true)
  })

  test("fullscreen training pane keeps sidebar and overlays stats on the scene", () => {
    const document = view({
      ...initialModel,
      pane: "training-fullscreen",
      trainingRuns: liveTrainingProjection,
    })
    expect(treeContainsClass(document.body, "sidebar")).toBe(true)
    expect(treeContainsClass(document.body, "training-fullscreen-pane")).toBe(true)
    expect(treeContainsClass(document.body, "training-fullscreen-page")).toBe(true)
    expect(treeContainsSelector(document.body, "oa-training-run")).toBe(true)
    expect(treeContainsClass(document.body, "training-fullscreen-overlay")).toBe(true)
    expect(treeContainsClass(document.body, "training-fullscreen-stats")).toBe(true)
    expect(treeContainsText(document.body, "active windows")).toBe(true)
    expect(treeContainsText(document.body, "verified work")).toBe(true)
  })

  test("fullscreen training pane shows selected node-specific overlay data", () => {
    const document = view({
      ...initialModel,
      pane: "training-fullscreen",
      trainingRuns: liveTrainingProjection,
      selectedTrainingSceneNodeId: "freivalds",
    })
    expect(treeContainsClass(document.body, "training-fullscreen-node-panel")).toBe(
      true,
    )
    expect(treeContainsText(document.body, "Freivalds")).toBe(true)
    expect(treeContainsText(document.body, "freivalds refs")).toBe(true)
    expect(treeContainsText(document.body, "gradient refs")).toBe(true)
  })

  test("training scene receives operator command signals", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
      trainingLeasePending: true,
      trainingLeaseStatus: { text: "claiming training lease...", tone: "info" },
      trainingPlanStatus: { text: "planned", tone: "success" },
    })
    const scene = findSelectorNode(document.body, "oa-training-run") as {
      data?: {
        props?: {
          visualization?: {
            operatorSignals?: Array<{
              detail: string
              id: string
              label: string
              state: string
            }>
          }
        }
      }
    } | null
    expect(scene?.data?.props?.visualization?.operatorSignals).toContainEqual({
      detail: "planned",
      id: "plan",
      label: "plan",
      state: "success",
    })
    expect(scene?.data?.props?.visualization?.operatorSignals).toContainEqual({
      detail: "not loaded",
      id: "readiness",
      label: "ready",
      state: "idle",
    })
    expect(scene?.data?.props?.visualization?.operatorSignals).toContainEqual({
      detail: "not loaded",
      id: "packet",
      label: "packet",
      state: "idle",
    })
    expect(scene?.data?.props?.visualization?.operatorSignals).toContainEqual({
      detail: "claiming training...",
      id: "lease",
      label: "lease",
      state: "info",
    })
    expect(scene?.data?.props?.visualization?.operatorSignals).toContainEqual({
      detail: "idle",
      id: "packet-build",
      label: "build",
      state: "idle",
    })
    expect(scene?.data?.props?.visualization?.operatorSignals).toContainEqual({
      detail: "idle",
      id: "admit",
      label: "admit",
      state: "idle",
    })
  })

  test("training pane includes the CS336 dashboard panel", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsClass(document.body, "training-dashboard-panel")).toBe(true)
  })

  test("training pane includes the selected run lifecycle panel", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
    })
    expect(treeContainsClass(document.body, "training-lifecycle-panel")).toBe(true)
    expect(treeContainsClass(document.body, "training-lifecycle-gates")).toBe(true)
    expect(treeContainsClass(document.body, "training-window-timeline")).toBe(true)
  })

  test("training pane includes the promise gates panel", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsClass(document.body, "training-promise-gates-panel")).toBe(true)
  })

  test("training pane includes the admin plan action", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsClass(document.body, "training-admin-plan-button")).toBe(true)
  })

  test("training pane includes the admin activate action", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingPlan: {
        ok: true,
        enabled: true,
        fetchedAt: "2026-06-14T00:00:00.000Z",
        sourceUrl: "https://openagents.test/api/training/windows/plan",
        trainingRunRef: "training.run.desktop.r1.test",
        windowRef: "training.window.desktop.r1.test",
        run: null,
        window: null,
        runPlanned: true,
        windowPlanned: true,
        reason: "planned",
        message: "planned",
      },
    })
    expect(treeContainsClass(document.body, "training-activate-button")).toBe(true)
  })

  test("training pane includes the training lease action", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingActivation: {
        ok: true,
        enabled: true,
        fetchedAt: "2026-06-14T00:00:00.000Z",
        sourceUrl:
          "https://openagents.test/api/training/windows/training.window.desktop.r1.test/activate",
        windowRef: "training.window.desktop.r1.test",
        window: null,
        reason: "activated",
        message: "activated",
      },
    })
    expect(treeContainsClass(document.body, "training-lease-button")).toBe(true)
  })

  test("training pane includes the bootstrap grant action", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
    })
    expect(treeContainsClass(document.body, "training-bootstrap-button")).toBe(true)
  })

  test("training pane includes the selected run evidence ledger", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
    })
    expect(treeContainsClass(document.body, "training-ledger-panel")).toBe(true)
    expect(treeContainsClass(document.body, "training-ledger-ref")).toBe(true)
  })

  test("training pane excludes the dev source-map panel (launch cleanup #5021)", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsClass(document.body, "training-source-map-panel")).toBe(false)
    expect(treeContainsClass(document.body, "training-source-map-refs")).toBe(false)
    expect(treeContainsText(document.body, "examples/training-run")).toBe(false)
  })

  test("training pane excludes the dev authority-boundary panel (launch cleanup #5021)", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
    })
    expect(
      treeContainsClass(document.body, "training-authority-boundary-panel"),
    ).toBe(false)
  })

  test("training pane boundary render excludes credential values and local paths", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
      trainingOperatorReadiness: {
        ok: true,
        fetchedAt: "2026-06-14T00:00:00.000Z",
        sourceUrl: "desktop:training-operator-readiness",
        trainingBaseUrl: "https://openagents.test",
        adminEnabled: true,
        adminTokenPresent: true,
        adminReady: true,
        leaseEnabled: true,
        leaseReady: true,
        pylonRefPresent: true,
        pylonRefSource: "identity",
        pylonRef: "pylon.training.1",
        pylonHomePresent: true,
        controlTokenPresent: true,
        localPylonReady: true,
        evidenceEnabled: true,
        evidencePacketPathPresent: true,
        evidenceReady: true,
        blockerRefs: [],
      },
      trainingEvidencePacketSummary: {
        ok: true,
        configured: true,
        fetchedAt: "2026-06-14T00:00:00.000Z",
        sourceUrl: "desktop:training-evidence-packet",
        packetSource: "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
        budgetLabel: "desktop tiny loss budget",
        budgetRefPresent: true,
        evalRefPresent: true,
        mergeRefPresent: true,
        finalValidationLoss: 2.8,
        maxValidationLoss: 3,
        lossPointCount: 2,
        freivaldsCommitmentRefCount: 1,
        gradientCloseoutRefCount: 1,
        evidenceRefCount: 10,
        receiptRefCount: 3,
        shardContributionCount: 2,
        distinctPylonCount: 2,
        blockerRefs: [],
      },
      trainingPlanStatus: {
        text: "planned",
        tone: "success",
      },
      trainingEvidencePacketBuildStatus: {
        text: "packet ready",
        tone: "success",
      },
    })
    expect(treeContainsText(document.body, "admin-token-value")).toBe(false)
    expect(treeContainsText(document.body, "sk-openagents-test")).toBe(false)
    expect(treeContainsText(document.body, "/Users/")).toBe(false)
    expect(treeContainsText(document.body, "/private/tmp/")).toBe(false)
  })

  test("training pane includes the operator feedback feed", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
      trainingRunsStatus: { text: "1 run", tone: "success" },
      trainingPlanStatus: { text: "planned", tone: "success" },
    })
    expect(treeContainsClass(document.body, "training-operator-feed-panel")).toBe(
      true,
    )
    expect(treeContainsClass(document.body, "training-operator-feed")).toBe(true)
  })

  test("training pane includes projection catch-up feedback", () => {
    const trainingRunRef = "training.run.desktop.r1.test"
    const windowRef = "training.window.desktop.r1.test"
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: {
        ...liveTrainingProjection,
        fetchedAt: "2026-06-14T00:05:00.000Z",
        runs: [
          {
            ...liveTrainingProjection.runs[0],
            receiptRefs: ["receipt.1", "receipt.2", "receipt.3"],
            state: "active",
            trainingRunRef,
          },
        ],
        summaries: [
          {
            ...liveTrainingProjection.summaries[0],
            metrics: {
              ...liveTrainingProjection.summaries[0].metrics,
              activeWindowCount: metric(1),
              plannedWindowCount: metric(0),
              receiptRefCount: metric(3),
            },
            receiptRefs: ["receipt.1", "receipt.2", "receipt.3"],
            run: {
              ...liveTrainingProjection.summaries[0].run,
              receiptRefs: ["receipt.1", "receipt.2", "receipt.3"],
              state: "active",
              trainingRunRef,
            },
            windows: [
              {
                datasetRefs: ["dataset.cs336.a1"],
                homeworkKind: "cs336_a1",
                plannedAtDisplay: "today",
                priority: 1,
                receiptRefs: ["receipt.1"],
                sealMetadata: null,
                sourceRefs: [],
                state: "active",
                trainingRunRef,
                updatedAtDisplay: "today",
                windowRef,
              },
            ],
          },
        ],
      },
      trainingPlan: {
        ok: true,
        enabled: true,
        fetchedAt: "2026-06-14T00:00:00.000Z",
        sourceUrl: "https://openagents.test/api/training/windows/plan",
        trainingRunRef,
        windowRef,
        run: null,
        window: null,
        runPlanned: true,
        windowPlanned: true,
        reason: "planned",
        message: "planned",
      },
      trainingPlanFirstObservedAt: "2026-06-14T00:05:00.000Z",
      trainingActivation: {
        ok: true,
        enabled: true,
        fetchedAt: "2026-06-14T00:01:00.000Z",
        sourceUrl: `https://openagents.test/api/training/windows/${windowRef}/activate`,
        windowRef,
        window: null,
        reason: "activated",
        message: "activated",
      },
      trainingLease: {
        ok: true,
        enabled: true,
        fetchedAt: "2026-06-14T00:02:00.000Z",
        sourceUrl: "https://openagents.test/api/training/leases/claim",
        pylonRef: "pylon.training.1",
        lease: {
          claimedAtDisplay: "now",
          leaseExpiresInSeconds: 900,
          leaseRef: "training.lease.1",
          pylonRef: "pylon.training.1",
          receiptRefs: ["receipt.lease.1"],
          state: "active",
          trainingRunRef,
          windowRef,
        },
        reason: "claimed",
        message: "claimed",
      },
      trainingEvidenceAdmission: {
        ok: true,
        enabled: true,
        fetchedAt: "2026-06-14T00:03:00.000Z",
        sourceUrl:
          "https://openagents.test/api/training/runs/training.run.desktop.r1.test/real-gradient-evidence",
        trainingRunRef,
        packetSource: "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
        run: null,
        realGradient: null,
        reason: "admitted",
        message: "admitted",
        evidenceRefCount: 6,
        receiptRefCount: 3,
        shardContributionCount: 2,
        distinctPylonCount: 2,
      },
    })

    expect(
      treeContainsClass(document.body, "training-projection-catchup-panel"),
    ).toBe(true)
    expect(treeContainsText(document.body, "Projection Catch-Up")).toBe(true)
    expect(
      treeContainsText(document.body, "2026-06-14T00:05:00.000Z"),
    ).toBe(true)
    expect(treeContainsText(document.body, "training.lease.1")).toBe(true)
    expect(treeContainsText(document.body, "3/3 receipts")).toBe(true)
  })

  test("training pane includes the operator readiness panel", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingOperatorReadiness: {
        ok: false,
        fetchedAt: "2026-06-14T00:00:00.000Z",
        sourceUrl: "desktop:training-operator-readiness",
        trainingBaseUrl: "https://openagents.test",
        adminEnabled: false,
        adminTokenPresent: false,
        adminReady: false,
        leaseEnabled: true,
        leaseReady: true,
        pylonRefPresent: true,
        pylonRefSource: "identity",
        pylonRef: "pylon.training.1",
        pylonHomePresent: true,
        controlTokenPresent: true,
        localPylonReady: true,
        evidenceEnabled: true,
        evidencePacketPathPresent: false,
        evidenceReady: false,
        blockerRefs: [
          "env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE",
          "env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN",
          "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
        ],
      },
      trainingOperatorReadinessStatus: {
        text: "3 operator blockers · https://openagents.test",
        tone: "info",
      },
    })
    expect(treeContainsClass(document.body, "training-operator-readiness-panel")).toBe(
      true,
    )
    expect(treeContainsClass(document.body, "training-readiness-blockers")).toBe(
      true,
    )
    expect(treeContainsText(document.body, "pylon.training.1")).toBe(true)
    expect(
      treeContainsText(
        document.body,
        "env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN",
      ),
    ).toBe(true)
    expect(
      treeContainsText(
        document.body,
        "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
      ),
    ).toBe(true)
  })

  test("training pane includes the evidence packet summary panel", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingEvidencePacketSummary: {
        ok: false,
        configured: true,
        fetchedAt: "2026-06-14T00:00:00.000Z",
        sourceUrl: "desktop:training-evidence-packet",
        packetSource: "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
        budgetLabel: "desktop tiny loss budget",
        budgetRefPresent: true,
        evalRefPresent: true,
        mergeRefPresent: true,
        finalValidationLoss: 3.4,
        maxValidationLoss: 3,
        lossPointCount: 2,
        freivaldsCommitmentRefCount: 1,
        gradientCloseoutRefCount: 1,
        evidenceRefCount: 10,
        receiptRefCount: 3,
        shardContributionCount: 1,
        distinctPylonCount: 1,
        blockerRefs: [
          "training.evidence_packet.loss_exceeds_budget",
          "training.evidence_packet.requires_two_distinct_pylons",
        ],
      },
      trainingEvidencePacketSummaryStatus: {
        text: "packet blocked · 2 blockers",
        tone: "info",
      },
    })
    expect(treeContainsClass(document.body, "training-evidence-packet-panel")).toBe(
      true,
    )
    expect(treeContainsClass(document.body, "training-evidence-packet-blockers")).toBe(
      true,
    )
    expect(
      treeContainsText(
        document.body,
        "training.evidence_packet.requires_two_distinct_pylons",
      ),
    ).toBe(true)
    expect(treeContainsText(document.body, "/Users/")).toBe(false)
  })

  test("training pane excludes the dev control-surface panel (launch cleanup #5021)", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
      trainingPlanStatus: { text: "planned", tone: "success" },
    })
    expect(treeContainsClass(document.body, "training-control-surface-panel")).toBe(
      false,
    )
    expect(
      treeContainsClass(document.body, "training-control-surface-list"),
    ).toBe(false)
    expect(
      treeContainsText(document.body, "/api/training/windows/{windowRef}/activate"),
    ).toBe(false)
    expect(
      treeContainsText(
        document.body,
        "apps/autopilot-desktop/src/bun/training-runs.ts",
      ),
    ).toBe(false)
  })

  test("training pane excludes the static roadmap ledger panel (launch cleanup #5022)", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsClass(document.body, "training-roadmap-panel")).toBe(false)
    expect(treeContainsClass(document.body, "training-roadmap-gates")).toBe(false)
    expect(treeContainsClass(document.body, "training-roadmap-refs")).toBe(false)
  })

  test("training pane includes the closeout packet action", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
    })
    expect(treeContainsClass(document.body, "training-closeout-button")).toBe(true)
  })

  test("training pane includes the evidence packet build action", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
      trainingEvidencePacketBuildStatus: {
        text: "wrote evidence packet candidate · 1 blockers",
        tone: "info",
      },
    })
    expect(treeContainsClass(document.body, "training-evidence-build-button")).toBe(
      true,
    )
    // The internal receipts-path env name was dev-doc scaffolding; it must not leak (launch cleanup #5021).
    expect(
      treeContainsText(
        document.body,
        "OPENAGENTS_TRAINING_WORKER_RECEIPTS_PATH",
      ),
    ).toBe(false)
    expect(treeContainsText(document.body, "/Users/")).toBe(false)
  })

  test("training pane includes the real-gradient evidence admission action", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: liveTrainingProjection,
    })
    expect(treeContainsClass(document.body, "training-evidence-button")).toBe(true)
    // The raw API route string was dev-doc scaffolding and is no longer rendered (launch cleanup #5021).
    expect(
      treeContainsText(
        document.body,
        "/api/training/runs/{runRef}/real-gradient-evidence",
      ),
    ).toBe(false)
  })

  test("training pane includes the reconcile action for sealed windows", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      trainingRuns: {
        ...liveTrainingProjection,
        summaries: [
          {
            ...liveTrainingProjection.summaries[0],
            windows: [
              {
                ...liveTrainingProjection.summaries[0].windows[0],
                state: "sealed",
                windowRef: "training.window.desktop.r1.sealed",
              },
            ],
          },
        ],
      },
    })
    expect(treeContainsClass(document.body, "training-reconcile-button")).toBe(true)
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
