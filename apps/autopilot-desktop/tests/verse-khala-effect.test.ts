// EPIC #6017: talk to Khala from an in-world Verse textbox.
//
// Covers the two new, load-bearing pieces with NO node and NO live gateway:
//   1. the receipt → LOCAL crackling-arc effect mapper (evidence-bound):
//      a real receipt ref produces a crackling_arc beam Khala-nexus → avatar,
//      a missing receipt ref produces NOTHING (the §5 motion contract).
//   2. the in-world textbox → khalaTurn reducer flow: input change, submit
//      (emits the streamed turn command + flips in-flight), live token append,
//      the terminal receipt landing (drives the effect), and the honest 402
//      add-credit state.
//   3. a render assertion: a model carrying a real receipt makes the Khala
//      effect nodes + the crackling arc appear in verseSceneVisualization.

import { describe, expect, test } from "bun:test"

import {
  verseKhalaEffectLayer,
  withVerseKhalaEffectLayer,
  VERSE_KHALA_NEXUS_NODE_ID,
  VERSE_KHALA_EFFECT_NODE_PREFIX,
} from "../src/shared/verse-khala-effect"
import { metaverseStreetLayout } from "@openagentsinc/three-effect/core"
import type { KhalaReceiptProjection } from "../src/shared/khala-cockpit"
import { initialModel, Model } from "../src/ui/model"
import { update } from "../src/ui/update"
import { verseSceneVisualization } from "../src/ui/view"
import {
  ChangedVerseKhalaInput,
  SubmittedVerseKhala,
  GotVerseKhalaToken,
  RespondedVerseKhala,
  FailedVerseKhala,
} from "../src/ui/message"

const liveReceipt = (
  overrides: Partial<KhalaReceiptProjection> = {},
): KhalaReceiptProjection => ({
  requestedModel: "openagents/khala-mini",
  servedModel: "khala-mini-served",
  worker: "pylon.worker.abc",
  lane: "cheap",
  verification: "test_passed",
  verified: true,
  receipt: "receipt.khala.req_abc123",
  receiptUrl: "/receipts/req_abc123",
  rubric: null,
  ...overrides,
})

const avatar = { x: 4, y: 0, z: -2 }

describe("verseKhalaEffectLayer (evidence-bound local crackling arc)", () => {
  test("a real receipt ref produces a crackling_arc beam Khala → avatar", () => {
    const layer = verseKhalaEffectLayer({
      receipt: liveReceipt(),
      avatar,
      generatedAt: "2026-06-22T00:00:00.000Z",
    })
    expect(layer.beams).toHaveLength(1)
    const beam = layer.beams[0]!
    expect(beam.style).toBe("crackling_arc")
    expect(beam.fromId).toBe(VERSE_KHALA_NEXUS_NODE_ID)
    expect(beam.toId).toBe(
      `${VERSE_KHALA_EFFECT_NODE_PREFIX}receipt.khala.req_abc123`,
    )
    // The receipt ref is the motion evidence (the §5 contract).
    expect(beam.sourceRefs).toEqual(["receipt.khala.req_abc123"])
    expect(beam.simulated).toBe(false)
    // Two endpoint entities (nexus + avatar) carry the receipt for the inspector.
    expect(layer.entities).toHaveLength(2)
    expect(layer.entities.every((e) => e.detail?.includes("receipt.khala.req_abc123"))).toBe(true)
    // A verified turn bursts at the avatar.
    expect(layer.bursts).toHaveLength(1)
  })

  test("the arc endpoints land in front of the avatar IN THE CAMERA'S VIEW (root-frame fix)", () => {
    // The avatar anchor is SCENE-WORLD; the renderer interprets entity positions
    // in ROOT-LOCAL (rotated/scaled/offset to the Tassadar lot). The endpoints are
    // therefore converted scene-world → root-local. Reconstruct them back to
    // scene-world (the camera/avatar frame) and assert the arc sits at the avatar
    // at chest/overhead height — NOT ~5 units up at the lot (the no-effect bug).
    const a = { x: 4, y: 0, z: -2 }
    const layer = verseKhalaEffectLayer({ receipt: liveReceipt(), avatar: a })
    const nexus = layer.entities.find((e) => e.id === VERSE_KHALA_NEXUS_NODE_ID)!
    const you = layer.entities.find((e) =>
      e.id.startsWith(VERSE_KHALA_EFFECT_NODE_PREFIX),
    )!
    const s = metaverseStreetLayout.tassadarSceneScale
    const toWorld = (l: readonly [number, number, number]) =>
      [
        s * l[0] + metaverseStreetLayout.tassadarLotX,
        s * l[2],
        -s * l[1] + metaverseStreetLayout.tassadarLotZ,
      ] as const
    const youWorld = toWorld(you.position!)
    const nexusWorld = toWorld(nexus.position!)
    // The "You" endpoint sits at the avatar's scene-world position, torso height.
    expect(Math.abs(youWorld[0] - a.x)).toBeLessThan(0.05)
    expect(Math.abs(youWorld[2] - a.z)).toBeLessThan(0.05)
    expect(youWorld[1]).toBeGreaterThan(0.5)
    expect(youWorld[1]).toBeLessThan(2.5)
    // The nexus floats above + in front of the avatar (overhead, toward -Z), not
    // pinned at the lot far from the camera.
    expect(nexusWorld[1]).toBeGreaterThan(youWorld[1])
    expect(nexusWorld[2]).toBeLessThan(a.z)
    expect(Math.abs(nexusWorld[0] - a.x)).toBeLessThan(0.5)
  })

  test("NO receipt ref ⇒ no motion (the evidence gate)", () => {
    const noRef = verseKhalaEffectLayer({
      receipt: liveReceipt({ receipt: null }),
      avatar,
    })
    expect(noRef.beams).toHaveLength(0)
    expect(noRef.entities).toHaveLength(0)
    expect(noRef.bursts).toHaveLength(0)

    const noReceipt = verseKhalaEffectLayer({ receipt: null, avatar })
    expect(noReceipt.beams).toHaveLength(0)
    expect(noReceipt.entities).toHaveLength(0)
  })

  test("a failed verification arcs but does not burst", () => {
    const layer = verseKhalaEffectLayer({
      receipt: liveReceipt({ verification: "failed", verified: false }),
      avatar,
    })
    expect(layer.beams).toHaveLength(1)
    expect(layer.bursts).toHaveLength(0)
    expect(layer.entities.every((e) => e.status === "blocked")).toBe(true)
  })

  test("withVerseKhalaEffectLayer forces evidence:required and is a no-op with no receipt", () => {
    const base = { nodes: [], entities: [], beams: [], bursts: [] } as never
    const withReceipt = withVerseKhalaEffectLayer(base, {
      receipt: liveReceipt(),
      avatar,
    })
    expect(withReceipt.motionPolicy?.evidence).toBe("required")
    expect((withReceipt.beams ?? []).length).toBe(1)

    const withoutReceipt = withVerseKhalaEffectLayer(base, {
      receipt: null,
      avatar,
    })
    expect(withoutReceipt).toBe(base)
  })
})

describe("in-world Khala textbox → khalaTurn reducer flow (#6017)", () => {
  test("typing tracks the in-world input bar", () => {
    const [next] = update(
      initialModel,
      ChangedVerseKhalaInput({ value: "build crossy road" }),
    )
    expect(next.verseKhalaInput).toBe("build crossy road")
  })

  test("submit emits a streamed Khala turn command and flips in-flight", () => {
    const typed = Model.make({
      ...initialModel,
      verseKhalaInput: "hello khala",
    })
    const [next, commands] = update(typed, SubmittedVerseKhala())
    expect(next.verseKhalaInFlight).toBe(true)
    expect(next.verseKhalaInput).toBe("")
    expect(next.verseKhalaResponse).toBe("")
    expect(next.verseKhalaReceipt).toBeNull()
    expect(next.verseKhalaTurnId).not.toBeNull()
    // One command was scheduled (the RunVerseKhalaTurn turn). It is NOT executed
    // here — the reducer is pure — so no node/gateway is touched.
    expect(commands).toHaveLength(1)
  })

  test("an empty or in-flight submit is a no-op", () => {
    const [empty, emptyCmds] = update(initialModel, SubmittedVerseKhala())
    expect(empty.verseKhalaInFlight).toBe(false)
    expect(emptyCmds).toHaveLength(0)

    const inFlight = Model.make({
      ...initialModel,
      verseKhalaInput: "x",
      verseKhalaInFlight: true,
      verseKhalaTurnId: "verse.khala.1",
    })
    const [stillInFlight, cmds] = update(inFlight, SubmittedVerseKhala())
    expect(stillInFlight.verseKhalaInput).toBe("x")
    expect(cmds).toHaveLength(0)
  })

  test("live tokens append to the active turn's bubble; stale turns are ignored", () => {
    const active = Model.make({
      ...initialModel,
      verseKhalaInFlight: true,
      verseKhalaTurnId: "verse.khala.active",
    })
    const [one] = update(
      active,
      GotVerseKhalaToken({ turnId: "verse.khala.active", delta: "Hel" }),
    )
    const [two] = update(
      one,
      GotVerseKhalaToken({ turnId: "verse.khala.active", delta: "lo" }),
    )
    expect(two.verseKhalaResponse).toBe("Hello")
    // A delta for a different (stale) turn never cross-renders.
    const [stale] = update(
      two,
      GotVerseKhalaToken({ turnId: "verse.khala.other", delta: "XXX" }),
    )
    expect(stale.verseKhalaResponse).toBe("Hello")
  })

  test("the terminal receipt lands, keeps a real receipt, and clears in-flight", () => {
    const active = Model.make({
      ...initialModel,
      verseKhalaInFlight: true,
      verseKhalaTurnId: "verse.khala.active",
      verseKhalaResponse: "streamed answer",
    })
    const [next] = update(
      active,
      RespondedVerseKhala({
        turnId: "verse.khala.active",
        ok: true,
        text: "streamed answer",
        receipt: liveReceipt(),
        live: true,
      }),
    )
    expect(next.verseKhalaInFlight).toBe(false)
    expect(next.verseKhalaReceipt?.receipt).toBe("receipt.khala.req_abc123")
    expect(next.verseKhalaStatus.tone).toBe("success")
  })

  test("a no-receipt answer is kept but drives NO effect (unverified)", () => {
    const active = Model.make({
      ...initialModel,
      verseKhalaInFlight: true,
      verseKhalaTurnId: "verse.khala.active",
    })
    const [next] = update(
      active,
      RespondedVerseKhala({
        turnId: "verse.khala.active",
        ok: true,
        text: "an answer with no receipt",
        receipt: liveReceipt({ receipt: null, receiptUrl: null }),
        live: false,
      }),
    )
    expect(next.verseKhalaReceipt).toBeNull()
    expect(next.verseKhalaStatus.tone).toBe("info")
  })

  test("a 402 add-credit answer surfaces as the response text + error tone", () => {
    // The Bun host returns ok:false with an honest add-credit message as `text`.
    const active = Model.make({
      ...initialModel,
      verseKhalaInFlight: true,
      verseKhalaTurnId: "verse.khala.active",
    })
    const addCredit =
      "Your credit balance is empty. Add credit at https://openagents.com to keep running Khala."
    const [next] = update(
      active,
      RespondedVerseKhala({
        turnId: "verse.khala.active",
        ok: false,
        text: addCredit,
        receipt: null,
        live: false,
      }),
    )
    expect(next.verseKhalaInFlight).toBe(false)
    expect(next.verseKhalaResponse).toBe(addCredit)
    expect(next.verseKhalaReceipt).toBeNull()
    expect(next.verseKhalaStatus.tone).toBe("error")
    expect(next.verseKhalaStatus.text).toBe(addCredit)
  })

  test("a bridge failure clears in-flight with an honest error", () => {
    const active = Model.make({
      ...initialModel,
      verseKhalaInFlight: true,
      verseKhalaTurnId: "verse.khala.active",
    })
    const [next] = update(
      active,
      FailedVerseKhala({
        turnId: "verse.khala.active",
        error: "I couldn't reach the local app service to talk to Khala.",
      }),
    )
    expect(next.verseKhalaInFlight).toBe(false)
    expect(next.verseKhalaReceipt).toBeNull()
    expect(next.verseKhalaStatus.tone).toBe("error")
  })
})

describe("verseSceneVisualization renders the local Khala arc from a receipt (#6017)", () => {
  test("a model with a real receipt makes the Khala effect nodes + arc appear", () => {
    const withReceipt = Model.make({
      ...initialModel,
      pane: "chat",
      verseSceneRestorePose: {
        regionRef: "world.region.tassadar",
        x: 4,
        y: 0,
        z: -2,
        yaw: 0,
        animation: "idle",
        capturedAtMs: 1,
      },
      verseKhalaReceipt: {
        requestedModel: "openagents/khala-mini",
        servedModel: "khala-mini-served",
        worker: "pylon.worker.abc",
        lane: "cheap",
        verification: "test_passed",
        verified: true,
        receipt: "receipt.khala.req_render",
        receiptUrl: "/receipts/req_render",
      },
    })
    const visualization = verseSceneVisualization(withReceipt)
    const entityIds = (visualization.entities ?? []).map((e) => e.id)
    expect(entityIds).toContain(VERSE_KHALA_NEXUS_NODE_ID)
    expect(entityIds).toContain(
      `${VERSE_KHALA_EFFECT_NODE_PREFIX}receipt.khala.req_render`,
    )
    const khalaArc = (visualization.beams ?? []).find(
      (b) => b.fromId === VERSE_KHALA_NEXUS_NODE_ID && b.style === "crackling_arc",
    )
    expect(khalaArc).toBeDefined()
    expect(khalaArc?.sourceRefs).toEqual(["receipt.khala.req_render"])
    expect(visualization.motionPolicy?.evidence).toBe("required")

    // No receipt ⇒ no Khala effect nodes (evidence-bound).
    const withoutReceipt = Model.make({ ...withReceipt, verseKhalaReceipt: null })
    const bare = verseSceneVisualization(withoutReceipt)
    expect((bare.entities ?? []).map((e) => e.id)).not.toContain(
      VERSE_KHALA_NEXUS_NODE_ID,
    )
  })
})
