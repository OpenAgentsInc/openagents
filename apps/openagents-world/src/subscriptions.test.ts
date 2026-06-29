import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  WORLD_READ_MODEL_SCHEMA_VERSION,
  decodeWorldDelta,
  decodeWorldReadModel,
  decodeWorldRow,
} from "@openagentsinc/world-contract"

import {
  cursorForSequence,
  makeReconnectPlan,
} from "./protocol"
import {
  DEFAULT_FAR_UPDATE_MS,
  DEFAULT_NEAR_UPDATE_MS,
  REGION_FEED_SPLIT_AVATAR_THRESHOLD,
  applySubscriptionDeltaToReadModel,
  approveSubscriptionPlan,
  deltaFromSubscriptionInterest,
  emptySubscriptionInterestState,
  entitiesFromRows,
  planRegionFeedPolicy,
  planSubscriptionInterestDelta,
} from "./subscriptions"

const now = "2026-06-22T00:00:00.000Z"
const regionRef = "region.run.tassadar.street"
const safety = {
  publicProjectionAllowed: true,
  sourceRefs: ["github:OpenAgentsInc/openagents#5965"],
  blockerRefs: [],
  caveatRefs: [],
}

const positionRow = (
  avatarRef: string,
  x: number,
  z: number,
  animation: "idle" | "walk" | "run" | "emote" | "unknown" = "walk",
) => decodeWorldRow({
  kind: "avatar_position",
  avatarRef,
  regionRef,
  position: { x, y: 0, z },
  rotationY: 0,
  animation,
  observedAt: now,
  seq: 1,
  safety,
})

describe("subscription interest policy", () => {
  test("approves typed server-controlled scope plans and rejects global avatar streams", () => {
    const plan = Effect.runSync(approveSubscriptionPlan({
      regionRef,
      scope: "selected_entity",
      selectedEntityRef: "avatar.selected",
      selectedRefs: ["pylon.a"],
      center: { x: 4, y: 0, z: 8 },
      resumeCursor: cursorForSequence(regionRef, 2),
    }))

    expect(plan.scope).toBe("selected_entity")
    expect(String(plan.selectedEntityRef)).toBe("avatar.selected")
    expect(plan.interest.selectedRefs.map(String)).toEqual(["pylon.a", "avatar.selected"])
    expect(plan.nearUpdateMs).toBe(DEFAULT_NEAR_UPDATE_MS)
    expect(plan.farUpdateMs).toBe(DEFAULT_FAR_UPDATE_MS)

    let rejection: unknown
    try {
      Effect.runSync(approveSubscriptionPlan({
        regionRef,
        scope: "global",
        requestedRows: ["avatar_position"],
        center: { x: 0, y: 0, z: 0 },
      }))
    } catch (error) {
      rejection = error
    }
    expect(rejection).toMatchObject({
      _tag: "WorldSubscriptionPolicyError",
      reason: expect.stringContaining("Unbounded global avatar/event subscriptions"),
    })
  })

  test("keeps a single region feed below threshold and splits near/far above it", () => {
    expect(planRegionFeedPolicy({ avatarCount: REGION_FEED_SPLIT_AVATAR_THRESHOLD }).kind)
      .toBe("single_region")
    expect(planRegionFeedPolicy({ avatarCount: REGION_FEED_SPLIT_AVATAR_THRESHOLD + 1 }).kind)
      .toBe("split_near_far")
    expect(planRegionFeedPolicy({ avatarCount: 12, estimatedRowsPerSecond: 1_200 }).kind)
      .toBe("split_near_far")
  })

  test("plans near/far enter, hysteresis exit, pruning, and re-entry full records", () => {
    const plan = Effect.runSync(approveSubscriptionPlan({
      regionRef,
      scope: "region",
      center: { x: 0, y: 0, z: 0 },
    }))
    const near = positionRow("avatar.near", 8, 0)
    const far = positionRow("avatar.far", 60, 0)
    const outsideEnter = positionRow("avatar.edge", 100, 0)
    const first = planSubscriptionInterestDelta({
      plan,
      previous: emptySubscriptionInterestState,
      entities: entitiesFromRows([near, far, outsideEnter]),
    })

    expect(first.fullRows.map(row => row.kind === "avatar_position" ? String(row.avatarRef) : "bad")).toEqual([
      "avatar.near",
      "avatar.far",
    ])
    expect(first.nearRefs).toEqual(["avatar.near"])
    expect(first.farRefs).toEqual(["avatar.far"])

    const heldByDropRadius = positionRow("avatar.far", 118, 0)
    const leftInterest = positionRow("avatar.near", 140, 0)
    const second = planSubscriptionInterestDelta({
      plan,
      previous: first.nextState,
      entities: entitiesFromRows([heldByDropRadius, leftInterest]),
    })

    expect(second.liteRows.map(row => row.kind === "avatar_position" ? String(row.avatarRef) : "bad")).toEqual(["avatar.far"])
    expect(second.prunedRefs).toEqual(["avatar.near"])

    const reentered = planSubscriptionInterestDelta({
      plan,
      previous: second.nextState,
      entities: entitiesFromRows([near]),
    })

    expect(reentered.fullRows.map(row => row.kind === "avatar_position" ? String(row.avatarRef) : "bad")).toEqual(["avatar.near"])
  })

  test("forces selected targets into high-resolution regardless of distance", () => {
    const plan = Effect.runSync(approveSubscriptionPlan({
      regionRef,
      scope: "region",
      selectedRefs: ["avatar.selected"],
      center: { x: 0, y: 0, z: 0 },
    }))
    const selectedFarAway = positionRow("avatar.selected", 220, 0)
    const delta = planSubscriptionInterestDelta({
      plan,
      previous: emptySubscriptionInterestState,
      entities: entitiesFromRows([selectedFarAway]),
    })

    expect(delta.nearRefs).toEqual(["avatar.selected"])
    expect(delta.updateMsByRef["avatar.selected"]).toBe(DEFAULT_NEAR_UPDATE_MS)
  })

  test("emits settle-on-stop patches and preserves absent-means-unchanged read models", () => {
    const plan = Effect.runSync(approveSubscriptionPlan({
      regionRef,
      center: { x: 0, y: 0, z: 0 },
    }))
    const idle = positionRow("avatar.resting", 2, 0, "idle")
    const interest = planSubscriptionInterestDelta({
      plan,
      previous: emptySubscriptionInterestState,
      entities: entitiesFromRows([idle]),
    })
    const delta = deltaFromSubscriptionInterest({
      regionRef,
      cursor: cursorForSequence(regionRef, 1),
      generatedAt: now,
      deltaRef: "delta.subscription.test.1",
      interest,
    })

    expect(delta.patches).toEqual([{ ref: "avatar.resting", movement: "settled" }])

    const model = decodeWorldReadModel({
      schemaVersion: WORLD_READ_MODEL_SCHEMA_VERSION,
      regionRef,
      cursor: cursorForSequence(regionRef, 0),
      generatedAt: now,
      regions: {},
      pylons: {},
      gateways: {},
      avatars: {},
      positions: {},
      chatMessages: {},
      chatBubbles: {},
      emotes: {},
      intents: {},
      runs: {},
      entities: {},
      edges: {},
      proofRefs: {},
      settlementRefs: {},
      events: {},
      diagnostics: [],
    })

    const applied = applySubscriptionDeltaToReadModel(model, delta)
    expect(applied.positions["avatar.resting"]?.animation).toBe("idle")

    const sparsePatchOnly = decodeWorldDelta({
      schemaVersion: "openagents.world_delta.v1",
      deltaRef: "delta.subscription.test.2",
      kind: "update",
      regionRef,
      cursor: cursorForSequence(regionRef, 2),
      generatedAt: now,
      patches: [{ ref: "avatar.resting", animation: "run" }],
    })
    const unchanged = applySubscriptionDeltaToReadModel(applied, sparsePatchOnly)

    expect(unchanged.positions["avatar.resting"]?.animation).toBe("idle")
  })

  test("uses stale cursor fallback when an approved plan resumes outside replay", () => {
    const plan = Effect.runSync(approveSubscriptionPlan({
      regionRef,
      center: { x: 0, y: 0, z: 0 },
      resumeCursor: cursorForSequence(regionRef, 1),
    }))
    const reconnect = makeReconnectPlan(
      regionRef,
      String(plan.resumeCursor),
      { currentSeq: 9, minReplaySeq: 5 },
      now,
    )

    expect(reconnect.kind).toBe("fresh_snapshot")
    expect(reconnect.frames[0]?.frameKind).toBe("diagnostic")
  })
})
