import { readFileSync } from "node:fs"
import { describe, expect, test } from "bun:test"
import {
  BLUEPRINT_CHAT_CONTEXT_TOOL_REF,
  BLUEPRINT_CHAT_REPLAY_EVIDENCE_REF,
  BLUEPRINT_CHAT_REPLAY_MODULE_REF,
  BLUEPRINT_CHAT_REPLAY_RECEIPT_REF,
  BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF,
  BLUEPRINT_CHAT_REPLAY_TOOL_REF,
  BLUEPRINT_CHAT_TASSADAR_EVIDENCE_REF,
  BLUEPRINT_CHAT_SIGNATURE_REF,
  BLUEPRINT_CHAT_TASSADAR_DIGEST_REF,
  BLUEPRINT_CHAT_TASSADAR_MODULE_REF,
  BLUEPRINT_CHAT_TASSADAR_RECEIPT_REF,
  BLUEPRINT_CHAT_TASSADAR_TOOL_REF,
  blueprintChatScopedSteps,
  initialModel,
  type PaneId,
} from "../src/ui/model"
import { sanitizeTree, view } from "../src/ui/view"
// #5466: a verified turn's steps are now derived from real session events, not
// the seed. Build one to exercise the rich step view + redaction guards.
import { liveChatScopedSteps } from "../src/ui/blueprint-chat-runtime"
import { selectSignatureForMessage } from "../src/ui/blueprint-chat-routing"

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

const treeContainsStringStyle = (node: unknown): boolean => {
  if (node === null || typeof node !== "object") return false
  const vnode = node as {
    children?: unknown[]
    data?: { style?: unknown }
  }
  if (typeof vnode.data?.style === "string") return true
  return Array.isArray(vnode.children)
    ? vnode.children.some(child => treeContainsStringStyle(child))
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

const publicActivityProjection = {
  ok: true,
  fetchedAt: "2026-06-18T00:00:00.000Z",
  sourceUrl: "https://openagents.test/api/public/activity-timeline?limit=20",
  envelope: {
    generatedAt: "2026-06-18T00:00:00.000Z",
    nextCursor: null,
    sourceLag: [
      {
        sourceKind: "forum",
        status: "stale",
        latestSourceEventAt: null,
        observedAt: "2026-06-18T00:00:00.000Z",
        lagSeconds: null,
        maxStalenessSeconds: 30,
        sourceRefs: ["forum.activity.public.1"],
        blockerRefs: ["blocker.public.activity_timeline.source_lag.forum"],
        caveatRefs: ["caveat.public.activity_timeline.source_lag"],
      },
    ],
    events: [
      {
        eventRef: "activity.training.settlement.1",
        cursor:
          "2026-06-18T00:00:01.000Z:settlement_receipt:activity.training.settlement.1",
        ts: "2026-06-18T00:00:01.000Z",
        kind: "real_bitcoin_moved",
        sourceKind: "settlement_receipt",
        runRef: "run.cs336.a1.demo",
        refs: ["receipt.public.real.1"],
        sourceRefs: ["receipt.public.real.1"],
        blockerRefs: [],
        caveatRefs: [],
        amountSats: 2100,
        realBitcoinMoved: true,
        state: "settled",
        text: "Receipt-backed real Bitcoin movement confirmed.",
      },
      {
        eventRef: "activity.forum.topic.1",
        cursor: "2026-06-18T00:00:02.000Z:forum:activity.forum.topic.1",
        ts: "2026-06-18T00:00:02.000Z",
        kind: "forum_topic_created",
        sourceKind: "forum",
        refs: ["forum.topic.public.1"],
        sourceRefs: ["forum.topic.public.1"],
        blockerRefs: [],
        caveatRefs: ["caveat.public.activity_timeline.source_lag"],
        state: "posted",
        text: "Public Forum topic created.",
      },
    ],
  },
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

  test("network home waits on the typed desktop proof replay bundle, no stale pylon overlay", () => {
    const document = view(initialModel) // default pane is "network"
    expect(treeContainsSelector(document.body, "oa-tassadar-proof-replay")).toBe(false)
    expect(treeContainsClass(document.body, "network-replay-status")).toBe(true)
    expect(treeContainsClass(document.body, "network-public-activity-panel")).toBe(true)
    expect(treeContainsText(document.body, "Live Public Activity")).toBe(true)
    expect(treeContainsText(document.body, "Loading Tassadar replay")).toBe(true)
    expect(treeContainsSelector(document.body, "oa-training-run")).toBe(false)
    expect(treeContainsSelector(document.body, "oa-desktop-pylon-diamonds")).toBe(false)
    expect(treeContainsClass(document.body, "network-overlay")).toBe(false)
    expect(treeContainsClass(document.body, "app-shell-network")).toBe(true)
    // immersive: no sidebar chrome on the home
    expect(treeContainsClass(document.body, "sidebar")).toBe(false)
  })

  test("network home renders public activity without a local node", () => {
    const document = view({
      ...initialModel,
      node: { ok: false, schema: "desktop.test", sessions: [] },
      publicActivityTimeline: publicActivityProjection,
      publicActivityTimelineStatus: {
        text: "2 events · 1 source warnings",
        tone: "info",
      },
    })

    expect(treeContainsClass(document.body, "network-public-activity-panel")).toBe(true)
    expect(treeContainsText(document.body, "Receipt-backed real Bitcoin movement confirmed.")).toBe(
      true,
    )
    expect(treeContainsText(document.body, "receipt.public.real.1")).toBe(true)
    expect(treeContainsText(document.body, "blocker.public.activity_timeline.source_lag.forum")).toBe(
      true,
    )
    expect(treeContainsClass(document.body, "sidebar")).toBe(false)
  })

  test("network home renders the controlled Tassadar proof replay after Bun loads the bundle", () => {
    const document = view({
      ...initialModel,
      proofReplay: {
        ok: true,
        fetchedAt: "2026-06-18T02:38:00.000Z",
        sourceUrl:
          "https://openagents.com/api/public/tassadar-replays/first-real-settlement",
        entry: {
          bundleEndpoint:
            "https://openagents.com/api/public/tassadar-replays/first-real-settlement",
          primarySourceRefs: ["receipt.real"],
          slug: "first-real-settlement",
          summary: "receipt-backed replay",
          title: "Tassadar Run 1: First Real Bitcoin Settlement",
          websitePath:
            "https://openagents.com/tassadar/replay/first-real-settlement",
        },
        bundle: { title: "Tassadar Run 1: First Real Bitcoin Settlement" },
        summary: {
          actorCount: 4,
          confirmedZapSats: 1000,
          durationSecond: 60,
          eventCount: 12,
          gapCount: 1,
          sourceRefCount: 6,
        },
        blockerRefs: [],
        cacheState: "live_https",
        cacheLabel: "live HTTPS read from openagents.com; no offline snapshot",
      },
      proofReplayStatus: {
        text: "Tassadar Run 1: First Real Bitcoin Settlement · 12 events · 1,000 sats",
        tone: "success",
      },
    })
    const replay = findSelectorNode(document.body, "oa-tassadar-proof-replay") as {
      data?: {
        attrs?: Record<string, string>
        props?: { bundle?: unknown }
      }
    } | null
    expect(replay?.data?.attrs?.["data-replay-slug"]).toBe(
      "first-real-settlement",
    )
    expect(replay?.data?.attrs?.["data-replay-origin"]).toBe(
      "https://openagents.com",
    )
    expect(replay?.data?.props?.bundle).toEqual({
      title: "Tassadar Run 1: First Real Bitcoin Settlement",
    })
    expect(treeContainsSelector(document.body, "oa-training-run")).toBe(false)
    expect(treeContainsSelector(document.body, "oa-desktop-pylon-diamonds")).toBe(false)
    expect(treeContainsClass(document.body, "network-overlay")).toBe(false)
    expect(treeContainsClass(document.body, "app-shell-network")).toBe(true)
    // immersive: no sidebar chrome on the home
    expect(treeContainsClass(document.body, "sidebar")).toBe(false)
  })

  test("all rendered panes use style objects, not CSS strings", () => {
    const panes: ReadonlyArray<PaneId> = [
      "network",
      "onboarding",
      "builtin-agent",
      "nodes",
      "training",
      "training-fullscreen",
      "sessions",
      "decisions",
      "spawn",
      "chat",
      "settings",
    ]

    for (const pane of panes) {
      expect(treeContainsStringStyle(view({ ...initialModel, pane }).body)).toBe(
        false,
      )
    }
  })

  test("desktop UI sources do not pass raw CSS strings to Foldkit Style", () => {
    const sources = [
      new URL("../src/ui/main.ts", import.meta.url),
      new URL("../src/ui/view.ts", import.meta.url),
    ]

    for (const source of sources) {
      expect(readFileSync(source, "utf8")).not.toMatch(/\bStyle\(\s*["'`]/)
    }
  })

  // #5466: the FIRST-PAINT pane no longer fakes a verified Tassadar step — the
  // honest intro carries no steps. A real, completed turn (derived from live
  // session events with a real digest) renders the rich step view + replay
  // refs, and surfaces "Verified" only because the live evidence says so.
  test("chat pane renders live Blueprint program steps + exact-replay refs (no first-paint fake)", () => {
    // Honest first paint: the intro shows no fabricated verdict.
    const firstPaint = view({ ...initialModel, pane: "chat", verseEnabled: false })
    expect(treeContainsClass(firstPaint.body, "chat-pane")).toBe(true)
    expect(treeContainsText(firstPaint.body, "Verified")).toBe(false)

    // A real, completed turn: steps derived from a live terminal event.
    const digest = `sha256:${"b".repeat(64)}`
    const selection = selectSignatureForMessage("show me the proof replay bundle")
    const messageId = "chat.test.verified"
    const completedMessage = {
      id: messageId,
      role: "assistant" as const,
      body: "Blueprint program turn completed.",
      timestamp: "2026-06-19T00:00:00.000Z",
      linkedSessionRef: "session.blueprint.chat.verified",
      steps: liveChatScopedSteps({
        selection,
        linkedSessionRef: "session.blueprint.chat.verified",
        events: [
          { eventIndex: 0, phase: "started", state: "running", observedAt: "2026-06-19T00:00:00Z", detail: "turn" },
          { eventIndex: 1, phase: "completed", state: "completed", observedAt: "2026-06-19T00:00:05Z", detail: `exact replay ${digest}` },
        ],
        proofReplaySlug: initialModel.selectedProofReplaySlug,
      }),
    }

    // Collapsed-by-default (#5466 chat-UX fix): the conversation text shows, but
    // the Blueprint/Tassadar scoped-step scaffolding stays behind the per-message
    // "program details" disclosure. The toggle is present; the refs are not yet.
    const collapsed = view({
      ...initialModel,
      pane: "chat",
      verseEnabled: false,
      chatMessages: [completedMessage],
      expandedChatMessages: [],
    })
    expect(treeContainsClass(collapsed.body, "chat-message-list")).toBe(true)
    expect(treeContainsText(collapsed.body, completedMessage.body)).toBe(true)
    expect(treeContainsText(collapsed.body, "program details")).toBe(true)
    expect(treeContainsText(collapsed.body, selection.signatureRef)).toBe(false)
    expect(treeContainsText(collapsed.body, "Verified")).toBe(false)

    // Expanded: the same message id is in `expandedChatMessages`, so the scoped
    // steps + exact-replay refs render.
    const document = view({
      ...initialModel,
      pane: "chat",
      verseEnabled: false,
      chatMessages: [completedMessage],
      expandedChatMessages: [messageId],
    })
    expect(treeContainsClass(document.body, "chat-message-list")).toBe(true)
    expect(treeContainsText(document.body, selection.signatureRef)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_CONTEXT_TOOL_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_TASSADAR_TOOL_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_TASSADAR_MODULE_REF)).toBe(true)
    // The rendered digest is the REAL one from the live event, not a constant.
    expect(treeContainsText(document.body, digest)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_TASSADAR_EVIDENCE_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_TASSADAR_RECEIPT_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_REPLAY_TOOL_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_REPLAY_MODULE_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_REPLAY_EVIDENCE_REF)).toBe(true)
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_REPLAY_RECEIPT_REF)).toBe(true)
    expect(treeContainsText(document.body, "Verified")).toBe(true)
    // The Tassadar REPLAY timeline is NOT rendered inline in chat anymore — it
    // lives on the Network home scene and the Training Proof Replays panel. The
    // chat disclosure shows only the public-safe ref.
    expect(treeContainsSelector(document.body, "oa-tassadar-proof-replay")).toBe(false)
    expect(treeContainsText(document.body, "raw_trace")).toBe(false)
    expect(treeContainsText(document.body, "raw_prompt")).toBe(false)
    expect(treeContainsText(document.body, "private_key")).toBe(false)
  })

  test("chat pane shows rejected Tassadar evidence honestly without raw traces", () => {
    const document = view({
      ...initialModel,
      pane: "chat",
      verseEnabled: false,
      // Expanded so the rejected-evidence refs in the per-message "program
      // details" disclosure render (collapsed-by-default after the #5466 fix).
      expandedChatMessages: ["chat.test.rejected"],
      chatMessages: [
        {
          id: "chat.test.rejected",
          role: "assistant",
          body: "Blueprint turn produced a rejected exact replay.",
          timestamp: "2026-06-19T00:00:00.000Z",
          linkedSessionRef: "session.blueprint.chat.rejected",
          steps: blueprintChatScopedSteps({
            linkedSessionRef: "session.blueprint.chat.rejected",
            tassadarStatus: "blocked",
            tassadarVerdict: "rejected",
            tassadarReceiptRef: null,
          }),
        },
      ],
    })

    expect(treeContainsText(document.body, BLUEPRINT_CHAT_TASSADAR_EVIDENCE_REF)).toBe(
      true,
    )
    expect(treeContainsText(document.body, BLUEPRINT_CHAT_TASSADAR_DIGEST_REF)).toBe(
      true,
    )
    expect(treeContainsText(document.body, "Rejected")).toBe(true)
    expect(treeContainsText(document.body, "raw_trace")).toBe(false)
    expect(treeContainsText(document.body, "raw_prompt")).toBe(false)
    expect(treeContainsText(document.body, "private_key")).toBe(false)
  })

  test("settings pane includes first-run health blockers (#5064)", () => {
    const document = view({
      ...initialModel,
      pane: "settings",
      installReadiness: {
        ok: false,
        fetchedAt: "2026-06-15T00:00:00.000Z",
        sourceUrl: "desktop:install-readiness",
        platform: "darwin",
        arch: "arm64",
        runtime: "packaged",
        nodeLaunchStatus: "failed",
        pylonHomePresent: false,
        controlTokenPresent: false,
        localPylonReady: false,
        builtInAgentReady: false,
        appleFmReady: false,
        userApiKeyRequired: false,
        autoUpdateEnabled: true,
        highestRoiAction: "Restart Autopilot or install a newer build",
        blockerRefs: ["blocker.autopilot.install.local_pylon_failed"],
        items: [
          {
            id: "local-pylon",
            label: "Local node",
            status: "blocked",
            detail: "The local Pylon node did not become reachable.",
            blockerRef: "blocker.autopilot.install.local_pylon_failed",
          },
        ],
      },
    })
    expect(treeContainsText(document.body, "First-run Health")).toBe(true)
    expect(
      treeContainsText(
        document.body,
        "Restart Autopilot or install a newer build",
      ),
    ).toBe(true)
    expect(
      treeContainsText(
        document.body,
        "blocker.autopilot.install.local_pylon_failed",
      ),
    ).toBe(true)
  })

  test("agent pane distinguishes hosted compute from local Apple FM (#5071)", () => {
    const document = view({
      ...initialModel,
      pane: "builtin-agent",
      agentMode: "local-apple-fm",
      appleFmReadiness: {
        ok: false,
        fetchedAt: "2026-06-15T00:00:00.000Z",
        sourceUrl: "desktop:apple-fm-readiness",
        localPylonReady: true,
        available: false,
        status: "unavailable",
        backendKind: "apple_fm_bridge",
        profileId: "apple-fm-local",
        model: "apple-foundation-model",
        capability: "probe.backend.apple_fm_bridge",
        advertisedCapabilities: [],
        baseUrl: "http://127.0.0.1:11435",
        platform: "darwin-arm64",
        version: "fake-bridge",
        unavailableReason: "apple_intelligence_disabled",
        message: "Apple Intelligence is disabled.",
        blockerRefs: ["blocker.pylon.apple_fm.apple_intelligence_disabled"],
      },
      appleFmStatus: {
        text: "Apple Intelligence is disabled.",
        tone: "info",
      },
    })

    expect(treeContainsText(document.body, "Hosted OpenAgents Compute")).toBe(true)
    expect(treeContainsText(document.body, "Local Apple FM")).toBe(true)
    expect(treeContainsText(document.body, "on-device Apple Foundation Models")).toBe(true)
    expect(treeContainsText(document.body, "Local selected")).toBe(true)
    expect(
      treeContainsText(
        document.body,
        "blocker.pylon.apple_fm.apple_intelligence_disabled",
      ),
    ).toBe(true)
  })

  test("agent pane exposes ready local Apple FM session start (#5072)", () => {
    const document = view({
      ...initialModel,
      pane: "builtin-agent",
      agentMode: "local-apple-fm",
      appleFmReadiness: {
        ok: true,
        fetchedAt: "2026-06-15T00:00:00.000Z",
        sourceUrl: "desktop:apple-fm-readiness",
        localPylonReady: true,
        available: true,
        status: "ready",
        backendKind: "apple_fm_bridge",
        profileId: "apple-fm-local",
        model: "apple-foundation-model",
        capability: "probe.backend.apple_fm_bridge",
        advertisedCapabilities: ["probe.backend.apple_fm_bridge"],
        baseUrl: "http://127.0.0.1:11435",
        platform: "darwin-arm64",
        version: "fake-bridge",
        unavailableReason: null,
        message: null,
        blockerRefs: [],
      },
      appleFmStatus: {
        text: "ready · apple-foundation-model",
        tone: "success",
      },
    })

    expect(treeContainsText(document.body, "Local selected")).toBe(true)
    expect(treeContainsText(document.body, "Start local")).toBe(true)
    expect(treeContainsText(document.body, "Run entirely locally through Apple Foundation Models")).toBe(false)
    expect(treeContainsText(document.body, "/tmp/openagents-builtin-agent")).toBe(false)
  })

  test("agent pane includes Product Promises surfacing flow (#5065)", () => {
    const document = view({
      ...initialModel,
      pane: "builtin-agent",
      promiseSurfacingReadiness: {
        ok: false,
        fetchedAt: "2026-06-15T00:00:00.000Z",
        sourceUrl: "desktop:promise-surfacing-readiness",
        forumSlug: "product-promises",
        baseUrl: "https://openagents.test",
        productPromisesUrl: "https://openagents.test/api/public/product-promises",
        forumTopicsUrl:
          "https://openagents.test/api/forum/forums/product-promises/topics",
        agentTokenPresent: false,
        blockerRefs: ["env.OPENAGENTS_AGENT_TOKEN"],
      },
      promiseSurfacingResult: {
        ok: false,
        mode: "drafted",
        draft: {
          title: "[Promise Report] autopilot.builtin_compute_agent.v1",
          requestedSlug: "promise-report-autopilot-builtin-compute-agent-v1",
          bodyText: "Surface only. Do not ship code.",
          ledgerVerdict: "ledger_claims_fixed_report_new_mismatch",
          registryVersion: "2026-06-15.4",
          promiseState: "green",
          relatedTopicRefs: [],
        },
        blockerRefs: ["env.OPENAGENTS_AGENT_TOKEN"],
      },
    })
    expect(treeContainsText(document.body, "Surface Promise Gap")).toBe(true)
    expect(treeContainsText(document.body, "env.OPENAGENTS_AGENT_TOKEN")).toBe(true)
    expect(
      treeContainsText(
        document.body,
        "[Promise Report] autopilot.builtin_compute_agent.v1",
      ),
    ).toBe(true)
    expect(treeContainsText(document.body, "Draft report")).toBe(true)
  })

  test("training pane includes the training scene", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsSelector(document.body, "oa-training-run")).toBe(true)
  })

  test("training pane includes the public activity strip", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      publicActivityTimeline: publicActivityProjection,
    })
    expect(treeContainsClass(document.body, "training-public-activity-panel")).toBe(true)
    expect(treeContainsText(document.body, "Public Activity")).toBe(true)
    expect(treeContainsText(document.body, "Public Forum topic created.")).toBe(true)
    expect(treeContainsText(document.body, "receipt.public.real.1")).toBe(true)
  })

  test("training pane includes the proof replay shelf for web and desktop parity", () => {
    const document = view({ ...initialModel, pane: "training" })
    expect(treeContainsClass(document.body, "training-proof-replay-panel")).toBe(true)
    expect(treeContainsClass(document.body, "training-proof-replay-viewport")).toBe(true)
    expect(treeContainsSelector(document.body, "oa-tassadar-proof-replay")).toBe(false)
    expect(treeContainsClass(document.body, "training-proof-replay-placeholder")).toBe(true)
    expect(treeContainsText(document.body, "Proof Replays")).toBe(true)
    expect(treeContainsText(document.body, "First settlement")).toBe(true)
    expect(treeContainsText(document.body, "Recognition")).toBe(true)
    expect(treeContainsText(document.body, "Open social cut")).toBe(true)
    expect(treeContainsText(document.body, "/Users/")).toBe(false)
    expect(treeContainsText(document.body, ".secrets")).toBe(false)
  })

  test("training pane renders generated replay filters, source refs, and caveats", () => {
    const document = view({
      ...initialModel,
      pane: "training",
      selectedProofReplayMode: "generated",
      proofReplayStatus: {
        text: "Generated Public Activity Replay · 1 event · 1,000 sats",
        tone: "success",
      },
      proofReplay: {
        ok: true,
        fetchedAt: "2026-06-18T12:03:00.000Z",
        sourceUrl:
          "https://openagents.com/api/public/proof-replays?mode=activity-timeline&from=2026-06-18T12%3A00%3A00.000Z&to=2026-06-18T12%3A05%3A00.000Z",
        entry: null,
        request: {
          mode: "generated",
          filters: {
            from: "2026-06-18T12:00:00.000Z",
            to: "2026-06-18T12:05:00.000Z",
            kind: "real_bitcoin_moved",
            pairRef: "pylon.448ba824b5fc879f3a59+pylon.treasury",
            since: "2026-06-18T12:00:00.000Z:settlement_receipt:event.1",
            source: "settlement_receipt",
          },
        },
        filterLabel:
          "2026-06-18T12:00:00.000Z → 2026-06-18T12:05:00.000Z · real_bitcoin_moved",
        generatedFrom: {
          caveatRefs: [
            "caveat.public.proof_replay.generated_from_activity_timeline_observation_only",
          ],
          source: {
            route: "/api/public/activity-timeline",
            url: "https://openagents.com/api/public/activity-timeline?from=2026-06-18T12%3A00%3A00.000Z&to=2026-06-18T12%3A05%3A00.000Z",
          },
          sourceLag: [{ sourceKind: "forum", status: "stale" }],
        },
        caveatRefs: [
          "caveat.public.proof_replay.generated_from_activity_timeline_observation_only",
          "caveat.public_activity_timeline.source_lag.forum",
        ],
        bundle: {
          title: "Generated Public Activity Replay",
          events: [
            {
              amountSats: 1000,
              displayText: "Receipt-backed real Bitcoin movement confirmed.",
              kind: "payment_zap_confirmed",
              sourceRefs: ["receipt.public.real.1"],
            },
          ],
          gaps: [
            {
              gapRef: "gap.source_lag.1.forum",
              reason: "Public activity source forum is stale",
              sourceRefs: [
                "caveat.public_activity_timeline.source_lag.forum",
              ],
            },
          ],
          sourceRefs: [
            {
              kind: "receipt",
              ref: "receipt.public.real.1",
            },
            {
              kind: "api",
              ref: "public_activity_timeline.generated.range",
              url: "https://openagents.com/api/public/activity-timeline?from=2026-06-18T12%3A00%3A00.000Z&to=2026-06-18T12%3A05%3A00.000Z",
            },
          ],
        },
        summary: {
          actorCount: 2,
          confirmedZapSats: 1000,
          durationSecond: 12,
          eventCount: 1,
          gapCount: 1,
          sourceRefCount: 1,
        },
        blockerRefs: [],
        cacheState: "live_https",
        cacheLabel: "live HTTPS read from openagents.com; no offline snapshot",
      },
    })

    expect(treeContainsText(document.body, "Load generated")).toBe(true)
    expect(treeContainsText(document.body, "Pair")).toBe(true)
    expect(treeContainsText(document.body, "Source")).toBe(true)
    expect(treeContainsText(document.body, "Since")).toBe(true)
    expect(treeContainsText(document.body, "Generated Public Activity Replay")).toBe(true)
    expect(treeContainsText(document.body, "receipt.public.real.1")).toBe(true)
    expect(treeContainsText(document.body, "caveat.public_activity_timeline.source_lag.forum")).toBe(true)
    expect(treeContainsText(document.body, "Open activity API")).toBe(true)
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
