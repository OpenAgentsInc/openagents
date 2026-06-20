import { describe, expect, test } from "bun:test"

import {
  chatWorldRegionRefForRun,
} from "../src/shared/chat-world-multiplayer"
import {
  chatWorldDesktopAvatarIdentity,
  defaultChatWorldRegionForRun,
  planChatWorldAvatarPositionWrite,
  projectChatWorldSpacetimeRows,
  type ChatWorldRegionRow,
} from "../src/shared/chat-world-spacetimedb"

const runRef = "run.tassadar.executor.20260615"
const regionRef = chatWorldRegionRefForRun(runRef)

const regionRow: ChatWorldRegionRow = {
  regionRef,
  runRef,
  label: "Tassadar main",
  minX: -20,
  minY: -4,
  minZ: -20,
  maxX: 20,
  maxY: 8,
  maxZ: 20,
  proximityRadiusMeters: 12,
  avatarPositionMinIntervalMs: 1_500,
}

describe("projectChatWorldSpacetimeRows", () => {
  test("maps generated SpacetimeDB rows into the public Verse projection", () => {
    const projection = projectChatWorldSpacetimeRows({
      flagEnabled: true,
      runRef,
      nowMs: 10_000,
      worldUrl: "https://spacetime.openagents.com",
      database: "openagents-world",
      rows: {
        regions: [regionRow],
        stations: [{
          pylonRef: "pylon.public.1",
          runRef,
          regionRef,
          label: "Public Pylon",
          positionX: 2.25,
          positionY: 0,
          positionZ: -4.5,
        }],
        avatars: [{
          avatarRef: "avatar.public.1",
          actorRef: "agent.public.1",
          actorKind: "pylon_agent",
          displayName: "Agent One",
        }],
        positions: [{
          avatarRef: "avatar.public.1",
          regionRef,
          positionX: -1,
          positionY: 0.5,
          positionZ: 3,
          yaw: 0.75,
          movementMode: "walking",
          lastSeenEpochMs: 9_000,
        }],
        messages: [{
          messageRef: "message.public.1",
          speakerAvatarRef: "avatar.public.1",
          regionRef,
          body: "near the pylon",
          radiusMeters: 10,
          expiresAtEpochMs: 20_000,
        }],
        attention: [{
          attentionRef: "attention.public.1",
          avatarRef: "avatar.public.1",
          pylonRef: "pylon.public.1",
          attentionKind: "nearby",
          expiresAtEpochMs: 20_000,
        }],
      },
    })

    expect(projection.regions).toEqual([regionRow])
    expect(projection.world.connected).toBe(true)
    expect(projection.world.regionRef).toBe(regionRef)
    expect(projection.world.stations).toEqual([{
      pylonRef: "pylon.public.1",
      label: "Public Pylon",
      x: 2.25,
      y: 0,
      z: -4.5,
    }])
    expect(projection.world.agents).toHaveLength(1)
    expect(projection.world.agents[0]).toMatchObject({
      avatarRef: "avatar.public.1",
      actorRef: "agent.public.1",
      avatarKind: "pylon_agent",
      label: "Agent One",
      x: -1,
      y: 0.5,
      z: 3,
      movementMode: "walking",
      chatMessages: ["near the pylon"],
      attentionRefs: ["attention.public.1"],
    })
    expect(projection.world.proximityChatCount).toBe(1)
  })

  test("disconnect fallback is explicit and inert", () => {
    const projection = projectChatWorldSpacetimeRows({
      flagEnabled: true,
      runRef,
      rows: null,
      nowMs: 10_000,
    })

    expect(projection.regions).toEqual([])
    expect(projection.world.connected).toBe(false)
    expect(projection.world.agents).toEqual([])
    expect(projection.world.stations).toEqual([])
  })
})

describe("chatWorldDesktopAvatarIdentity", () => {
  test("uses public pylon identity without leaking private host data", () => {
    expect(
      chatWorldDesktopAvatarIdentity({
        pylonRef: "pylon.public.1",
        nodeLabel: "Local Pylon",
      }),
    ).toEqual({
      pylonRef: "pylon.public.1",
      actorRef: "pylon.public.1",
      displayName: "Local Pylon",
    })
  })
})

describe("planChatWorldAvatarPositionWrite", () => {
  test("emits a bounded, rounded set_avatar_position write", () => {
    const plan = planChatWorldAvatarPositionWrite({
      region: regionRow,
      previous: {
        avatarRef: "avatar.public.1",
        regionRef,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        movementMode: "idle",
        lastSeenEpochMs: 1_000,
      },
      nowMs: 3_000,
      x: 1.23456,
      y: 0,
      z: 2.34567,
      yaw: 0.12345,
      pitch: 0.54321,
      movementMode: "walking",
    })

    expect(plan).toEqual({
      ok: true,
      write: {
        regionRef,
        positionX: 1.235,
        positionY: 0,
        positionZ: 2.346,
        yaw: 0.123,
        pitch: 0.543,
        movementMode: "walking",
      },
    })
  })

  test("rejects unsafe local avatar movement writes", () => {
    expect(
      planChatWorldAvatarPositionWrite({
        region: null,
        previous: null,
        nowMs: 3_000,
        x: 0,
        y: 0,
        z: 0,
      }),
    ).toMatchObject({ ok: false, reason: "region unavailable" })

    expect(
      planChatWorldAvatarPositionWrite({
        region: regionRow,
        previous: null,
        nowMs: 3_000,
        x: 99,
        y: 0,
        z: 0,
      }),
    ).toMatchObject({ ok: false, reason: "position outside region bounds" })

    expect(
      planChatWorldAvatarPositionWrite({
        region: regionRow,
        previous: {
          avatarRef: "avatar.public.1",
          regionRef,
          x: 0,
          y: 0,
          z: 0,
          yaw: 0,
          movementMode: "idle",
          lastSeenEpochMs: 2_500,
        },
        nowMs: 3_000,
        x: 1,
        y: 0,
        z: 0,
      }),
    ).toMatchObject({ ok: false, reason: "position update rate limited" })

    expect(
      planChatWorldAvatarPositionWrite({
        region: regionRow,
        previous: {
          avatarRef: "avatar.public.1",
          regionRef: "region.other.main",
          x: 0,
          y: 0,
          z: 0,
          yaw: 0,
          movementMode: "idle",
          lastSeenEpochMs: 1_000,
        },
        nowMs: 3_000,
        x: 1,
        y: 0,
        z: 0,
      }),
    ).toMatchObject({
      ok: false,
      reason: "avatar must join region before moving",
    })
  })

  test("finds the default region for the active Tassadar run", () => {
    expect(defaultChatWorldRegionForRun([regionRow], runRef)).toBe(regionRow)
    expect(defaultChatWorldRegionForRun([], runRef)).toBeNull()
  })
})
