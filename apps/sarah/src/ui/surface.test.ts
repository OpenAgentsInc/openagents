/**
 * SQ-7 (#8624): the Sarah surface consumes the Effect Native catalog pieces
 * that replaced its local workarounds — the transcript is the EN `Transcript`
 * primitive (effect-native#35) and the avatar pane is the EN `MediaVideo`
 * host (effect-native#67, catalog v26) mounted for the session lifetime.
 */
import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { Effect as NativeEffect } from "@effect-native/core/effect"
import {
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetAttemptEntity,
  decodeFleetCommandOutcomeEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  decodeFleetWorkUnitEntity,
} from "@openagentsinc/khala-sync"

import { projectSarahCodingCloseoutReceipts } from "../contracts/coding-closeout-receipt.ts"
import { projectSarahFleetOwnerRun } from "../contracts/fleet-owner-projection.ts"
import type { SarahOwnerFleetHostIntentHandlers } from "./main.ts"

// main.ts boots against the real page on import; give it an inert document so
// the module loads headlessly and boot() no-ops (no #sarah-root here).
;(globalThis as { document?: unknown }).document ??= {
  readyState: "complete",
  getElementById: () => null,
  addEventListener: () => {},
}

const {
  sarahSurfaceView,
  sarahAvatarPaneView,
  sarahAvatarContinuityProjection,
  sarahOwnerFleetInteractionMode,
  sarahOwnerFleetHostIntents,
  ownerFleetViewStateFromBrowser,
  reconcileSarahOwnerFleetViewState,
} = await import("./main.ts")
const { sarahEffectNativeTheme } = await import("./theme.ts")
const sarahCss = readFileSync(new URL("./sarah.css", import.meta.url), "utf8")

type SurfaceState = Parameters<typeof sarahSurfaceView>[0]

const baseState: SurfaceState = {
  status: "idle",
  avatarMedia: { status: "not_requested" },
  avatarStart: { status: "idle" },
  avatarStop: { status: "idle" },
  avatarArmed: true,
  avatarActive: false,
  avatarSessionOpen: false,
  sandbox: false,
  input: "",
  transcript: [{ key: "welcome", role: "assistant", text: "Hello from Sarah" }],
  cards: [],
  accountPhase: "anonymous",
  accountEmail: null,
  activePanel: "blueprint",
  pendingAction: null,
  blueprintProspectRef: null,
  blueprintDraft: null,
  blueprintFacts: [],
  blueprintContactEmail: null,
  receiptsProspectRef: null,
  receipts: [],
}

const surfaceRun = decodeFleetRunEntity({
  runId: "fleet.run.surface",
  status: "running",
  desiredSlots: 1,
  workerKind: "auto",
  startedAt: "2026-07-09T19:30:00.000Z",
  counters: {
    workUnitsTotal: 1,
    activeAssignments: 0,
    completedAssignments: 1,
    failedAssignments: 0,
    blockedAssignments: 0,
  },
  updatedAt: "2026-07-09T20:00:00.000Z",
})

const surfaceAssignment = decodeFleetAssignmentEntity({
  assignmentRef: "assignment.surface.grok",
  issueRef: "#8639",
  status: "accepted_work",
  closeoutClass: "accepted_work",
  updatedAt: "2026-07-09T20:00:00.000Z",
})

const surfaceWorker = decodeFleetWorkerEntity({
  workerId: "worker.surface.grok",
  phase: "completed",
  harnessKind: "grok",
  assignmentRef: "assignment.surface.grok",
  accountRefHash: `account.pylon.grok.${"3".repeat(24)}`,
  lastProgressAt: "2026-07-09T20:00:00.000Z",
  updatedAt: "2026-07-09T20:00:00.000Z",
})

const surfaceAttempt = decodeFleetAttemptEntity({
  attemptRef: "work_claim.surface.grok",
  workUnitRef: "unit.surface.grok",
  intakeClaimRef: `claim.sarah_fleet_run.${"a".repeat(24)}`,
  pylonRef: "pylon-owner-1",
  workerKind: "grok",
  state: "succeeded",
  progressClass: "terminal",
  assignmentRef: "assignment.surface.grok",
  accountRefHash: `account.pylon.grok.${"3".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "api_metered",
  verification: {
    truth: "passed",
    verifierRef: "verification.surface.grok",
    evidenceRefs: ["test.surface.grok"],
  },
  artifactRefs: ["artifact.public.surface.grok"],
  proofRefs: ["proof.surface.grok"],
  authorityReceiptRefs: ["authority.owner.surface.grok"],
  closeoutRef: "closeout.surface.grok",
  usageEvidence: {
    schema: "openagents.pylon.fleet_run_usage_evidence.v1",
    truth: "not_measured",
    harnessKind: "grok",
    evidenceRef: "evidence.surface.grok",
    assignmentRef: "assignment.surface.grok",
    receiptRef: "receipt.surface.grok",
    tokenUsageRefs: [],
    caveatRefs: ["caveat.surface.grok.not_measured"],
  },
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"1".repeat(24)}`,
  startedAt: "2026-07-09T19:55:00.000Z",
  lastObservedAt: "2026-07-09T20:00:00.000Z",
  remoteObservedAt: "2026-07-09T19:59:59.000Z",
  terminalAt: "2026-07-09T20:00:00.000Z",
  updatedAt: "2026-07-09T20:00:00.000Z",
})

const surfaceWorkUnit = decodeFleetWorkUnitEntity({
  workUnitRef: "unit.surface.grok",
  issueRef: "#8639",
  dependsOnRefs: [],
  state: "succeeded",
  latestAttemptRef: surfaceAttempt.attemptRef,
  acceptedAttemptRef: surfaceAttempt.attemptRef,
  updatedAt: "2026-07-09T20:00:00.000Z",
})

const surfaceProjection = (
  approvals: ReadonlyArray<ReturnType<typeof decodeFleetApprovalEntity>> = [],
) =>
  projectSarahFleetOwnerRun(
    {
      run: surfaceRun,
      workUnits: [surfaceWorkUnit],
      attempts: [surfaceAttempt],
      assignments: [surfaceAssignment],
      workers: [surfaceWorker],
      approvals,
      inboxFlags: [],
    },
    Date.parse("2026-07-09T20:00:00.000Z"),
  )

const fleetProjection = surfaceProjection()

const pauseOutcome = decodeFleetCommandOutcomeEntity({
  intentId: "intent.surface.pause",
  seq: 21,
  kind: "fleet_run_control",
  targetRef: surfaceRun.runId,
  deliveryOutcome: "applied",
  completionOutcome: "applied",
  effectiveOutcome: "paused",
  completionRef: `outcome.pylon.fleet_steering.${"e".repeat(24)}`,
  completedAt: "2026-07-09T20:00:02.000Z",
  outcomeRef: `outcome.pylon.fleet_steering.${"e".repeat(24)}`,
  observedAt: "2026-07-09T20:00:01.000Z",
  recordedAt: "2026-07-09T20:00:02.000Z",
  updatedAt: "2026-07-09T20:00:02.000Z",
})

const [fleetCloseoutReceipt] = projectSarahCodingCloseoutReceipts({
  projection: fleetProjection,
})

if (fleetCloseoutReceipt === undefined) {
  throw new Error("expected fleet closeout fixture")
}

const fleetProjectionWithApproval = surfaceProjection([
  decodeFleetApprovalEntity({
    approvalRef: "approval.surface.grok",
    status: "pending",
    workerId: "worker.surface.grok",
    toolClass: "write_file",
    openedAt: "2026-07-09T19:59:00.000Z",
    updatedAt: "2026-07-09T20:00:00.000Z",
  }),
])

const ownerFleetState = (
  closeouts: NonNullable<SurfaceState["ownerFleet"]>["closeouts"],
  projection: typeof fleetProjection | null = fleetProjection,
  connection: NonNullable<SurfaceState["ownerFleet"]>["connection"] = {
    phase: "idle",
  },
): NonNullable<SurfaceState["ownerFleet"]> => ({
  runRef: projection?.run.runRef ?? fleetProjection.run.runRef,
  scope: `scope.fleet_run.${projection?.run.runRef ?? fleetProjection.run.runRef}` as never,
  connection,
  projection,
  closeouts,
  expandedAuditWorkUnitRefs: [],
  expandedReceiptCardRefs: [],
  selectedNode: null,
  hostCommandSubmissions: [],
  steerDraft: null,
})

const ownerFleetHandlers: SarahOwnerFleetHostIntentHandlers = {
  SarahFleetRunControlRequested: () => NativeEffect.void,
  SarahFleetWorkUnitOpened: () => NativeEffect.void,
  SarahFleetApprovalDecisionRequested: () => NativeEffect.void,
  SarahFleetEvidenceOpened: () => NativeEffect.void,
  SarahCodingReceiptAction: () => NativeEffect.void,
  submitSteer: () => NativeEffect.succeed({ intentId: "intent.test.steer" }),
}

type AnyNode = { readonly _tag?: string; readonly [key: string]: unknown }

const findByTag = (node: unknown, tag: string): AnyNode | null => {
  if (node === null || typeof node !== "object") return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByTag(child, tag)
      if (found) return found
    }
    return null
  }
  const record = node as AnyNode
  if (record._tag === tag) return record
  for (const value of Object.values(record)) {
    const found = findByTag(value, tag)
    if (found) return found
  }
  return null
}

const findAllByTag = (node: unknown, tag: string): ReadonlyArray<AnyNode> => {
  if (node === null || typeof node !== "object") return []
  if (Array.isArray(node)) return node.flatMap((child) => findAllByTag(child, tag))
  const record = node as AnyNode
  return [
    ...(record._tag === tag ? [record] : []),
    ...Object.values(record).flatMap((value) => findAllByTag(value, tag)),
  ]
}

const findByKey = (node: unknown, key: string): AnyNode | null => {
  if (node === null || typeof node !== "object") return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByKey(child, key)
      if (found) return found
    }
    return null
  }
  const record = node as AnyNode
  if (record.key === key) return record
  for (const value of Object.values(record)) {
    const found = findByKey(value, key)
    if (found) return found
  }
  return null
}

const visibleTextOutsideAccordion = (node: unknown): ReadonlyArray<string> => {
  if (node === null || typeof node !== "object") return []
  if (Array.isArray(node)) {
    return node.flatMap((child) => visibleTextOutsideAccordion(child))
  }
  const record = node as AnyNode
  if (record._tag === "Accordion") return []
  return [
    ...(record._tag === "Text" && typeof record.content === "string"
      ? [record.content]
      : []),
    ...Object.values(record).flatMap((value) =>
      visibleTextOutsideAccordion(value),
    ),
  ]
}

const relativeLuminance = (hex: string): number => {
  const channels = [1, 3, 5].map((start) => Number.parseInt(hex.slice(start, start + 2), 16) / 255)
  const linear = channels.map((channel) =>
    channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4,
  )
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!
}

const contrastRatio = (foreground: string, background: string): number => {
  const foregroundLuminance = relativeLuminance(foreground)
  const backgroundLuminance = relativeLuminance(background)
  return (
    (Math.max(foregroundLuminance, backgroundLuminance) + 0.05) /
    (Math.min(foregroundLuminance, backgroundLuminance) + 0.05)
  )
}

describe("Sarah accessible responsive renderer skin (#8610)", () => {
  test("secondary text is cool, AA-readable, and the UI canvas is near-black rather than pure black", () => {
    expect(sarahEffectNativeTheme.color.background).toBe("#03060b")
    expect(sarahEffectNativeTheme.color.textMuted).toBe("#aeb9c6")
    expect(
      contrastRatio(
        sarahEffectNativeTheme.color.textMuted,
        sarahEffectNativeTheme.color.surfaceRaised,
      ),
    ).toBeGreaterThanOrEqual(4.5)
    expect(sarahCss).not.toContain("color: #5b6b8c")
  })

  test("status and composer controls carry explicit accessible state and names", () => {
    const idleView = sarahSurfaceView(baseState)
    expect(findByKey(idleView, "status")?.a11y).toEqual({
      label: "Sarah conversation status: Idle",
    })
    expect(findByKey(idleView, "composer-input")?.a11y).toEqual({
      label: "Message Sarah",
    })

    const thinkingView = sarahSurfaceView({ ...baseState, status: "thinking" })
    expect(findByKey(thinkingView, "composer-send")).toMatchObject({
      label: "Sending…",
      disabled: true,
    })
  })

  test("real controls retain focus, touch-target, reduced-motion, high-contrast, and narrow-height rules", () => {
    expect(sarahCss).toMatch(/button:focus-visible/)
    expect(sarahCss).toMatch(/min-height:\s*44px/)
    expect(sarahCss).toContain("100dvh")
    expect(sarahCss).toContain("@media (max-width: 520px)")
    expect(sarahCss).toContain("@media (prefers-reduced-motion: reduce)")
    expect(sarahCss).toContain("@media (forced-colors: active)")
  })
})

describe("sarah surface consumes the EN catalog (SQ-7 #8624)", () => {
  test("the transcript is the EN Transcript primitive, pinned, with keyed role-tagged messages", () => {
    const view = sarahSurfaceView(baseState)
    const transcript = findByTag(view, "Transcript")
    expect(transcript).not.toBeNull()
    expect(transcript?.pinToEnd).toBe(true)
    const messages = transcript?.messages as ReadonlyArray<{
      key: string
      role: string
      body: ReadonlyArray<{ _tag: string }>
    }>
    expect(messages.length).toBe(1)
    expect(messages[0]?.key).toBe("welcome")
    expect(messages[0]?.role).toBe("assistant")
    // The message body keeps the Card visual of the previous List+Card shell.
    expect(messages[0]?.body[0]?._tag).toBe("Card")
  })

  test("an open avatar session mounts the media-video host attach target", () => {
    const pane = sarahAvatarPaneView({ ...baseState, avatarSessionOpen: true })
    const media = findByTag(pane, "Host")
    expect(media).not.toBeNull()
    expect(media?.kind).toBe("media-video")
    expect(findByKey(pane, "avatar-overlay")).not.toBeNull()
  })

  test("with no open session the avatar pane renders no media host but keeps EN overlay controls", () => {
    const pane = sarahAvatarPaneView(baseState)
    expect(findByTag(pane, "Host")).toBeNull()
    expect(findByKey(pane, "avatar-start-overlay")).not.toBeNull()
  })

  test("shows video LIVE only for a fresh browser frame and transport lease", () => {
    const nowMs = 10_000
    const pane = sarahAvatarPaneView(
      {
        ...baseState,
        status: "live",
        avatarActive: true,
        avatarSessionOpen: true,
        avatarMedia: {
          status: "live",
          lease: {
            transportLeaseRef: "browser.media.surface",
            transportExpiresAtMs: nowMs + 1_000,
            lastFrameAtMs: nowMs,
          },
        },
      },
      nowMs,
    )

    expect(findByKey(pane, "avatar-media-status")).toMatchObject({
      label: "VIDEO · LIVE",
      tone: "success",
      a11y: { label: "Sarah video status: Live, moving frames" },
    })
    expect(findByKey(pane, "avatar-media-reconnect")).toBeNull()
  })

  test("keeps a text-live conversation and Fleet controls available when video is stale", () => {
    const nowMs = 10_000
    const staleState: SurfaceState = {
      ...baseState,
      status: "live",
      avatarActive: true,
      avatarSessionOpen: true,
      avatarMedia: { status: "stale", lastFrameAtMs: nowMs - 1_000 },
      ownerFleet: ownerFleetState({ status: "not_reported" }),
    }
    const continuity = sarahAvatarContinuityProjection(staleState, nowMs)
    const pane = sarahAvatarPaneView(staleState, nowMs)
    const surface = sarahSurfaceView(
      staleState,
      sarahOwnerFleetInteractionMode(ownerFleetHandlers),
    )

    expect(continuity.conversation).toEqual({ status: "text_live" })
    expect(continuity.media.status).toBe("stale")
    expect(continuity.continuation).toMatchObject({
      status: "text_continuation_reconnect",
      textControl: "available",
      fleetControl: "available",
      action: "reconnect_media",
    })
    expect(findByKey(pane, "avatar-media-reconnecting")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Sarah video status: Reconnecting. Video paused. Keep working in text; Fleet controls remain available.",
      },
    })
    expect(findByKey(pane, "avatar-media-status")).toMatchObject({
      label: "VIDEO · RECONNECTING",
      tone: "warn",
    })
    expect(findByKey(pane, "avatar-media-reconnect")).toMatchObject({
      label: "Reconnect video",
      onPress: { name: "SarahReconnectAvatarMedia" },
    })
    expect(findByKey(surface, "composer-input")).not.toBeNull()
    expect(findByKey(surface, "fleet-panel")).not.toBeNull()
  })

  test("stale and unavailable copy never promises Fleet without an exact Fleet scope", () => {
    const nowMs = 10_000
    const stale = sarahAvatarPaneView(
      {
        ...baseState,
        status: "live",
        avatarActive: true,
        avatarSessionOpen: true,
        avatarMedia: { status: "stale", lastFrameAtMs: nowMs - 1_000 },
      },
      nowMs,
    )
    const unavailable = sarahAvatarPaneView(
      {
        ...baseState,
        status: "live",
        avatarSessionOpen: true,
        avatarMedia: { status: "unavailable" },
      },
      nowMs,
    )

    expect(findByKey(stale, "avatar-media-reconnecting")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Sarah video status: Reconnecting. Video paused. Keep working in text.",
      },
    })
    expect(findByKey(stale, "avatar-media-reconnecting-copy")).toMatchObject({
      content: "Video paused. Text stays available.",
    })
    expect(findByKey(unavailable, "avatar-media-unavailable")).toMatchObject({
      a11y: {
        role: "group",
        label: "Sarah video status: Unavailable. Keep working in text.",
      },
    })
    expect(findByKey(unavailable, "avatar-media-unavailable-copy")).toMatchObject({
      content: "Video unavailable. Text stays available.",
    })
    expect(JSON.stringify(stale)).not.toContain("Fleet controls")
    expect(JSON.stringify(unavailable)).not.toContain("Fleet controls")
  })

  test("an unconfirmed stop fails closed without offering a replacement action", () => {
    const pane = sarahAvatarPaneView({
      ...baseState,
      status: "live",
      avatarSessionOpen: true,
      avatarMedia: { status: "unavailable" },
      avatarStop: { status: "timed_out" },
    })

    expect(findByKey(pane, "avatar-media-stop-unconfirmed")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Sarah video status: Stop unconfirmed. A replacement video will not start. Keep working in text.",
      },
    })
    expect(findByKey(pane, "avatar-media-reconnect")).toBeNull()
    expect(findByKey(pane, "avatar-close-overlay")).toMatchObject({
      label: "Close video",
      disabled: false,
      onPress: { name: "SarahStopAvatar" },
    })
  })

  test("a start stays visibly single-flight even when callbacks report live before its handle resolves", () => {
    const pane = sarahAvatarPaneView({
      ...baseState,
      status: "live",
      avatarActive: true,
      avatarSessionOpen: true,
      avatarMedia: { status: "connecting" },
      avatarStart: { status: "starting" },
    })

    expect(findByKey(pane, "avatar-media-status")).toMatchObject({
      label: "VIDEO · STARTING",
      tone: "info",
    })
    expect(findByKey(pane, "avatar-start-overlay")).toMatchObject({
      label: "Starting video…",
      disabled: true,
    })
    expect(findByKey(pane, "avatar-stop-overlay")).toBeNull()
  })

  test("a timed-out start blocks replacement visibly while text and scoped Fleet stay available", () => {
    const timedOut: SurfaceState = {
      ...baseState,
      status: "error",
      avatarSessionOpen: true,
      avatarMedia: { status: "unavailable" },
      avatarStart: { status: "timed_out" },
      ownerFleet: ownerFleetState({ status: "not_reported" }),
    }
    const pane = sarahAvatarPaneView(timedOut)
    const surface = sarahSurfaceView(
      timedOut,
      sarahOwnerFleetInteractionMode(ownerFleetHandlers),
    )

    expect(findByKey(pane, "avatar-media-start-unconfirmed")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Sarah video status: Start unconfirmed. A replacement video will not start until the pending start is resolved. Keep working in text; Fleet controls remain available.",
      },
    })
    expect(findByKey(pane, "avatar-media-start-unconfirmed-copy")).toMatchObject({
      content:
        "Pending video start is unresolved. Text and Fleet controls stay available.",
    })
    expect(findByKey(pane, "avatar-media-reconnect")).toBeNull()
    expect(findByKey(pane, "avatar-close-overlay")).toMatchObject({
      label: "Close video",
      disabled: false,
      onPress: { name: "SarahStopAvatar" },
    })
    expect(findByKey(surface, "composer-input")).not.toBeNull()
    expect(findByKey(surface, "fleet-panel")).not.toBeNull()
  })

  test("a timed-out start never promises Fleet without an exact Fleet scope", () => {
    const pane = sarahAvatarPaneView({
      ...baseState,
      status: "error",
      avatarSessionOpen: true,
      avatarMedia: { status: "unavailable" },
      avatarStart: { status: "timed_out" },
    })

    expect(findByKey(pane, "avatar-media-start-unconfirmed-copy")).toMatchObject({
      content: "Pending video start is unresolved. Text stays available.",
    })
    expect(JSON.stringify(pane)).not.toContain("Fleet controls")
  })

  test("unconfirmed cleanup exposes no replacement action while text and scoped Fleet remain", () => {
    const unconfirmed: SurfaceState = {
      ...baseState,
      status: "error",
      avatarSessionOpen: true,
      avatarMedia: { status: "unavailable" },
      avatarStart: { status: "cleanup_unconfirmed" },
      avatarStop: { status: "failed" },
      ownerFleet: ownerFleetState({ status: "not_reported" }),
    }
    const pane = sarahAvatarPaneView(unconfirmed)
    const surface = sarahSurfaceView(
      unconfirmed,
      sarahOwnerFleetInteractionMode(ownerFleetHandlers),
    )

    expect(findByKey(pane, "avatar-media-start-cleanup-unconfirmed")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Sarah video status: Start and stop unconfirmed. A replacement video will not start. Keep working in text; Fleet controls remain available.",
      },
    })
    expect(findByKey(pane, "avatar-media-status")).toMatchObject({
      label: "VIDEO · START/STOP UNCONFIRMED",
      tone: "danger",
    })
    expect(findByKey(pane, "avatar-media-reconnect")).toBeNull()
    expect(findByKey(pane, "avatar-close-overlay")).toMatchObject({
      label: "Close video",
      onPress: { name: "SarahStopAvatar" },
    })
    expect(findByKey(surface, "composer-input")).not.toBeNull()
    expect(findByKey(surface, "fleet-panel")).not.toBeNull()
  })

  test("unconfirmed cleanup never promises Fleet without an exact Fleet scope", () => {
    const pane = sarahAvatarPaneView({
      ...baseState,
      status: "error",
      avatarSessionOpen: true,
      avatarMedia: { status: "unavailable" },
      avatarStart: { status: "cleanup_unconfirmed" },
      avatarStop: { status: "failed" },
    })

    expect(
      findByKey(pane, "avatar-media-start-cleanup-unconfirmed-copy"),
    ).toMatchObject({
      content: "Video cleanup is unconfirmed. Text stays available.",
    })
    expect(JSON.stringify(pane)).not.toContain("Fleet controls")
  })
})

describe("FC-3 owner fleet surface integration", () => {
  test("hydrates the live browser projection into receipts without a legacy evidence channel", () => {
    const ownerFleet = ownerFleetViewStateFromBrowser({
      config: {
        runRef: fleetProjection.run.runRef,
        scope: `scope.fleet_run.${fleetProjection.run.runRef}` as never,
      },
      connection: {
        phase: "live",
        scope: `scope.fleet_run.${fleetProjection.run.runRef}` as never,
        cursor: 12 as never,
        connectedAtMs: 100,
        lastActivityAtMs: 120,
      },
      projection: fleetProjection,
    })

    expect(ownerFleet.closeouts).toMatchObject({
      status: "ready",
      receipts: [
        {
          cardRef: surfaceAttempt.attemptRef,
          attemptRef: surfaceAttempt.attemptRef,
          workUnitRef: surfaceWorkUnit.workUnitRef,
        },
      ],
    })
    expect(() =>
      sarahSurfaceView({
        ...baseState,
        activePanel: "fleet",
        ownerFleet,
      }),
    ).not.toThrow()
  })

  test("keeps Fleet absent and Blueprint selected without an exact owner run scope", () => {
    const staleFleetSelection = sarahSurfaceView({
      ...baseState,
      activePanel: "fleet",
    })
    const tabs = findByTag(staleFleetSelection, "Tabs") as {
      selectedId?: string
      tabs?: ReadonlyArray<{ id: string; label: string }>
      panels?: ReadonlyArray<{ id: string }>
    } | null

    expect(tabs?.selectedId).toBe("blueprint")
    expect(tabs?.tabs?.map((tab) => tab.id)).toEqual([
      "blueprint",
      "chat",
      "actions",
      "receipts",
    ])
    expect(tabs?.panels?.map((panel) => panel.id)).toEqual([
      "blueprint",
      "chat",
      "actions",
      "receipts",
    ])
    expect(findByKey(staleFleetSelection, "fleet-panel")).toBeNull()
    expect(
      findByKey(staleFleetSelection, "fleet-closeouts-not_reported"),
    ).toBeNull()
  })

  test("adds one Fleet tab after Blueprint only for an exact configured run", () => {
    const view = sarahSurfaceView({
      ...baseState,
      ownerFleet: ownerFleetState({ status: "not_reported" }),
    })
    const tabs = findByTag(view, "Tabs") as {
      selectedId?: string
      tabs?: ReadonlyArray<{ id: string; label: string }>
      panels?: ReadonlyArray<{ id: string }>
    } | null

    expect(tabs?.selectedId).toBe("blueprint")
    expect(tabs?.tabs?.map((tab) => tab.label)).toEqual([
      "Blueprint map",
      "Fleet",
      "Chat",
      "Actions",
      "Receipts",
    ])
    expect(tabs?.panels?.map((panel) => panel.id)).toEqual([
      "blueprint",
      "fleet",
      "chat",
      "actions",
      "receipts",
    ])
    expect(findByKey(view, "fleet-panel")).not.toBeNull()
    expect(
      findByKey(view, "fleet-supervision-fleet.run.surface"),
    ).not.toBeNull()
    expect(findByKey(view, "fleet-closeouts-not_reported")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Closeouts not reported. Coding closeout receipts have not been reported.",
      },
    })
    expect(findByKey(view, "fleet-panel-list")?.style).toMatchObject({
      width: "full",
      flex: 1,
      minHeight: 0,
    })
  })

  test("uses the full Fleet tab for selected detail with an accessible Back action", () => {
    const ownerFleet = ownerFleetState({
      status: "ready",
      receipts: [fleetCloseoutReceipt],
    })
    const view = sarahSurfaceView({
      ...baseState,
      activePanel: "fleet",
      ownerFleet: {
        ...ownerFleet,
        selectedNode: {
          kind: "attempt",
          runRef: fleetProjection.run.runRef,
          workUnitRef: surfaceWorkUnit.workUnitRef,
          attemptRef: surfaceAttempt.attemptRef,
        },
      },
    })
    expect(findByKey(view, "fleet-drilldown-back")).toMatchObject({
      _tag: "Button",
      label: "Back to fleet",
      a11y: { label: "Back to the fleet plan and work canvas" },
      onPress: { name: "SarahFleetDrilldownClosed" },
    })
    expect(
      findByKey(view, `fleet-drilldown-attempt-${surfaceAttempt.attemptRef}`),
    ).not.toBeNull()
    expect(
      findByKey(view, `fleet-supervision-${fleetProjection.run.runRef}`),
    ).toBeNull()
    expect(findByKey(view, "fleet-closeouts")).toBeNull()
  })

  test("keeps selection through a blank reconnect but clears it when the entity or run disappears", () => {
    const current: NonNullable<SurfaceState["ownerFleet"]> = {
      ...ownerFleetState({ status: "not_reported" }),
      selectedNode: {
        kind: "work_unit",
        runRef: fleetProjection.run.runRef,
        workUnitRef: surfaceWorkUnit.workUnitRef,
      },
    }
    const reconnecting = reconcileSarahOwnerFleetViewState(current, {
      ...current,
      projection: null,
      selectedNode: null,
    })
    expect(reconnecting.selectedNode).toEqual(current.selectedNode)

    const missing = reconcileSarahOwnerFleetViewState(current, {
      ...current,
      projection: { ...fleetProjection, workUnits: [] },
      selectedNode: null,
    })
    expect(missing.selectedNode).toBeNull()

    const changedScope = reconcileSarahOwnerFleetViewState(current, {
      ...current,
      runRef: "fleet.run.other",
      scope: "scope.fleet_run.fleet.run.other" as never,
      selectedNode: current.selectedNode,
      hostCommandSubmissions: [
        {
          submissionRef: "host-command-old",
          intentId: null,
          kind: "fleet_run_control",
          targetRef: current.runRef,
          status: "failed",
          summary: "Old command failed",
        },
      ],
    })
    expect(changedScope.selectedNode).toBeNull()
    expect(changedScope.hostCommandSubmissions).toEqual([])
  })

  test("clears pending command UI only for the exact durable intent id", () => {
    const current: NonNullable<SurfaceState["ownerFleet"]> = {
      ...ownerFleetState({ status: "not_reported" }),
      hostCommandSubmissions: [
        {
          submissionRef: "host-pause",
          intentId: pauseOutcome.intentId,
          kind: "fleet_run_control",
          targetRef: fleetProjection.run.runRef,
          status: "requested",
          summary: "Pause fleet run requested",
        },
        {
          submissionRef: "host-stop",
          intentId: "intent.surface.stop",
          kind: "fleet_run_control",
          targetRef: fleetProjection.run.runRef,
          status: "requested",
          summary: "Stop fleet run requested",
        },
      ],
    }
    const reconciled = reconcileSarahOwnerFleetViewState(current, {
      ...current,
      projection: {
        ...fleetProjection,
        commandOutcomes: [pauseOutcome],
      },
      hostCommandSubmissions: [],
    })
    expect(
      reconciled.hostCommandSubmissions.map(
        (submission) => submission.submissionRef,
      ),
    ).toEqual(["host-stop"])
  })

  test("renders typed loading and reconnect states before exact projection hydration", () => {
    const loading = sarahSurfaceView({
      ...baseState,
      activePanel: "fleet",
      ownerFleet: ownerFleetState({ status: "not_reported" }, null),
    })
    expect(findByKey(loading, "fleet-connection-idle")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Fleet loading. Loading the owner-safe projection for this exact run.",
      },
    })
    expect(
      findByKey(loading, "fleet-supervision-fleet.run.surface"),
    ).toBeNull()

    const reconnecting = sarahSurfaceView({
      ...baseState,
      activePanel: "fleet",
      ownerFleet: ownerFleetState(
        { status: "not_reported" },
        null,
        {
          phase: "reconnecting",
          scope: "scope.fleet_run.fleet.run.surface" as never,
          cursor: 4 as never,
          attempt: 1,
          retryAtMs: 100,
          mustRefetchReason: null,
          error: {
            reason: "network_unavailable",
            messageSafe: "Fleet connection is temporarily unavailable.",
            retryable: true,
          },
        },
      ),
    })
    expect(findByKey(reconnecting, "fleet-connection-reconnecting")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Fleet reconnecting. Fleet connection is temporarily unavailable.",
      },
    })
    expect(JSON.stringify(reconnecting)).not.toContain("network_unavailable")
  })

  test("renders loading, error, and not-reported closeouts only from explicit variants", () => {
    for (const status of ["loading", "error", "not_reported"] as const) {
      const view = sarahSurfaceView({
        ...baseState,
        activePanel: "fleet",
        ownerFleet: ownerFleetState({ status }),
      })
      expect(findByKey(view, `fleet-closeouts-${status}`)).not.toBeNull()
      for (const other of ["loading", "error", "not_reported"] as const) {
        if (other === status) continue
        expect(findByKey(view, `fleet-closeouts-${other}`)).toBeNull()
      }
    }
  })

  test("composes ready coding closeouts without promoting raw refs into primary copy", () => {
    const view = sarahSurfaceView({
      ...baseState,
      activePanel: "fleet",
      ownerFleet: ownerFleetState({
        status: "ready",
        receipts: [fleetCloseoutReceipt],
      }),
    })
    const receiptKey = `coding-receipt-${fleetCloseoutReceipt.cardRef}`
    const primaryCopy = visibleTextOutsideAccordion(
      findByKey(view, "fleet-panel"),
    ).join(" ")

    expect(findByKey(view, receiptKey)).not.toBeNull()
    expect(primaryCopy).toContain("Run identity: fleet.run.surface")
    for (const ref of [
      "assignment.surface.grok",
      "worker.surface.grok",
      "verification.surface.grok",
      "artifact.public.surface.grok",
      "authority.owner.surface.grok",
      fleetCloseoutReceipt.cardRef,
    ]) {
      expect(primaryCopy).not.toContain(ref)
    }
    expect(findByKey(view, `${receiptKey}-evidence`)?.expandedIds).toEqual([])
  })

  test("mount state without handlers is read-only while evidence disclosures remain local", () => {
    const interactionMode = sarahOwnerFleetInteractionMode(undefined)
    const partialHandlers = {
      SarahFleetRunControlRequested:
        ownerFleetHandlers.SarahFleetRunControlRequested,
    } as unknown as SarahOwnerFleetHostIntentHandlers
    const view = sarahSurfaceView(
      {
        ...baseState,
        activePanel: "fleet",
        ownerFleet: ownerFleetState(
          { status: "ready", receipts: [fleetCloseoutReceipt] },
          fleetProjectionWithApproval,
        ),
      },
      interactionMode,
    )
    const receiptKey = `coding-receipt-${fleetCloseoutReceipt.cardRef}`

    expect(interactionMode).toBe("read_only")
    expect(sarahOwnerFleetInteractionMode(partialHandlers)).toBe("read_only")
    expect(findByKey(view, "fleet-controls-unavailable")).toMatchObject({
      a11y: {
        role: "group",
        label:
          "Fleet controls unavailable. This surface is read-only; fleet state and evidence references remain visible.",
      },
    })
    for (const key of [
      "fleet-supervision-fleet.run.surface-control-pause",
      "fleet-supervision-unit.surface.grok-open",
      "fleet-supervision-unit.surface.grok-verification",
      "fleet-supervision-approval.surface.grok-allow",
      `${receiptKey}-next-action`,
    ]) {
      expect(findByKey(view, key)).toBeNull()
    }
    expect(
      (findByKey(
        view,
        "fleet-supervision-unit.surface.grok-audit",
      )?.onToggle as { name?: string })?.name,
    ).toBe("SarahFleetAuditToggled")
    expect(
      (findByKey(view, `${receiptKey}-evidence`)?.onToggle as {
        name?: string
      })?.name,
    ).toBe("SarahCodingReceiptEvidenceToggle")
  })

  test("mount state with every host handler exposes only typed host-bound actions", () => {
    expect(sarahOwnerFleetHostIntents.map((intent) => intent.name)).toEqual([
      "SarahFleetRunControlRequested",
      "SarahFleetWorkUnitOpened",
      "SarahFleetApprovalDecisionRequested",
      "SarahFleetEvidenceOpened",
      "SarahCodingReceiptAction",
    ])
    const interactionMode = sarahOwnerFleetInteractionMode(ownerFleetHandlers)
    const view = sarahSurfaceView(
      {
        ...baseState,
        ownerFleet: ownerFleetState(
          { status: "ready", receipts: [fleetCloseoutReceipt] },
          fleetProjectionWithApproval,
        ),
      },
      interactionMode,
    )
    expect(interactionMode).toBe("interactive")
    expect(findByKey(view, "fleet-controls-unavailable")).toBeNull()
    expect(
      (findByKey(
        view,
        "fleet-supervision-fleet.run.surface-control-pause",
      )?.onPress as { name?: string })?.name,
    ).toBe("SarahFleetRunControlRequested")
    expect(
      findByKey(view, "fleet-supervision-approval.surface.grok-allow"),
    ).toBeNull()
    expect(
      findByKey(
        view,
        "fleet-supervision-approval.surface.grok-decisions-empty",
      )?.content,
    ).toBe("Decision options not reported.")
    expect(
      (findByKey(
        view,
        `coding-receipt-${fleetCloseoutReceipt.cardRef}-next-action`,
      )?.onPress as { name?: string })?.name,
    ).toBe("SarahCodingReceiptAction")
  })
})

describe("contract sarah.split_screen_blueprint_map.v1 (BM-3 #8629)", () => {
  test("the right pane is an Effect Native Tabs surface with Blueprint map default", () => {
    const view = sarahSurfaceView(baseState)
    const tabs = findByTag(view, "Tabs") as
      | {
          selectedId?: string
          keepMounted?: boolean
          tabs?: ReadonlyArray<{ id: string; label: string }>
          panels?: ReadonlyArray<{ id: string }>
        }
      | null
    expect(tabs).not.toBeNull()
    expect(tabs?.selectedId).toBe("blueprint")
    expect(tabs?.keepMounted).toBe(true)
    expect(tabs?.tabs?.map((tab) => tab.label)).toEqual([
      "Blueprint map",
      "Chat",
      "Actions",
      "Receipts",
    ])
    expect(tabs?.panels?.map((panel) => panel.id)).toEqual([
      "blueprint",
      "chat",
      "actions",
      "receipts",
    ])
    const graph = findByTag(view, "GraphFigure") as
      | { nodes?: ReadonlyArray<{ id: string }>; edges?: ReadonlyArray<{ id: string }> }
      | null
    expect(graph).not.toBeNull()
    expect(graph?.nodes?.some((node) => node.id === "prospect")).toBe(true)
    expect(graph?.edges?.some((edge) => edge.id === "edge:prospect:account")).toBe(true)
  })

  test("the cut list is absent from the EN surface tree", () => {
    const serialized = JSON.stringify(sarahSurfaceView(baseState))
    expect(serialized).not.toContain("OpenAgents sales · openagents.com/sarah")
    expect(serialized).not.toContain("avatar-controls")
    expect(serialized).not.toContain('"key":"title"')
    expect(findByKey(sarahSurfaceView(baseState), "sarah-toolbar")).not.toBeNull()
  })

  test("the transcript, composer, actions, and receipts stay inside tab panels", () => {
    const view = sarahSurfaceView({
      ...baseState,
      cards: [{ key: "receipt-1", title: "Receipt", body: "Tool call recorded" }],
    })
    expect(findByKey(view, "chat-panel")).not.toBeNull()
    expect(findByKey(view, "composer")).not.toBeNull()
    expect(findByKey(view, "actions-panel")).not.toBeNull()
    expect(findByKey(view, "actions-book-human")).not.toBeNull()
    expect(findByKey(view, "actions-checkout-empty")).not.toBeNull()
    expect(findByKey(view, "receipts-panel")).not.toBeNull()
    expect(findByKey(view, "receipts-cards")).not.toBeNull()
    expect(findByKey(view, "cards")).toBeNull()
    expect(findAllByTag(view, "List").length).toBe(1)
  })

  test("BM-4 actions expose recorded checkout and receipts show Blueprint code", () => {
    const view = sarahSurfaceView({
      ...baseState,
      accountPhase: "linked",
      accountEmail: "buyer@example.com",
      activePanel: "receipts",
      blueprintDraft: {
        schema: "sarah.customer_blueprint_draft.v1",
        prospectRef: "prospect-actions",
        revision: 7,
        createdAt: "2026-07-09T17:00:00.000Z",
        business: { facts: [] },
        contacts: { email: "buyer@example.com", contactId: "oa_user:buyer" },
        needs: [],
        suggestedModules: [],
        sources: {
          turnIds: [],
          factCount: 0,
          provenance:
            "sarah_prospect_profile + sarah_transcript_turns (per-fact source turn ids)",
        },
        handoff: {
          pipeline: "operator_assisted_business_workspace",
          automatedProvisioning: false,
          convergesWith:
            "CB-1.4 prefill pipeline (intake -> public-data research -> seeded workspace)",
          note: "Draft only.",
        },
      },
      receipts: [
        {
          id: "tool:sarah.checkout.test",
          key: "receipt-tool-sarah.checkout.test",
          title: "Checkout link recorded",
          body: "Prepared a test-mode checkout quote.",
          href: "https://openagents.com/business",
          toolName: "checkout_link_create",
          receiptRef: "sarah.checkout.test",
          mode: "dry_run",
          ok: true,
        },
      ],
    })
    expect(findByKey(view, "actions-account-linked")).not.toBeNull()
    expect(findByKey(view, "actions-open-checkout")).not.toBeNull()
    expect(findByKey(view, "receipts-blueprint-code")).not.toBeNull()
    expect(JSON.stringify(view)).toContain("sarah.customer_blueprint_draft.v1")
  })
})
