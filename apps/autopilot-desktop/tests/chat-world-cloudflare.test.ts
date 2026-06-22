import { describe, expect, test } from "bun:test"

import {
  chatWorldRegionRefForRun,
} from "../src/shared/chat-world-multiplayer"
import {
  CHAT_WORLD_STARTER_REGION_CONTRACT,
  chatWorldDesktopAvatarIdentity,
  defaultChatWorldRegionForRun,
  planChatWorldAvatarPositionWrite,
  projectChatWorldCloudflareRows,
  type ChatWorldRegionRow,
} from "../src/shared/chat-world-cloudflare"

const runRef = "run.tassadar.executor.20260615"
const regionRef = chatWorldRegionRefForRun(runRef)

const regionRow: ChatWorldRegionRow = {
  regionRef,
  runRef,
  label: "Tassadar main",
  minX: CHAT_WORLD_STARTER_REGION_CONTRACT.bounds.minX,
  minY: CHAT_WORLD_STARTER_REGION_CONTRACT.bounds.minY,
  minZ: CHAT_WORLD_STARTER_REGION_CONTRACT.bounds.minZ,
  maxX: CHAT_WORLD_STARTER_REGION_CONTRACT.bounds.maxX,
  maxY: CHAT_WORLD_STARTER_REGION_CONTRACT.bounds.maxY,
  maxZ: CHAT_WORLD_STARTER_REGION_CONTRACT.bounds.maxZ,
  roadDirectionX: CHAT_WORLD_STARTER_REGION_CONTRACT.roadDirection.x,
  roadDirectionY: CHAT_WORLD_STARTER_REGION_CONTRACT.roadDirection.y,
  roadDirectionZ: CHAT_WORLD_STARTER_REGION_CONTRACT.roadDirection.z,
  localOriginX: CHAT_WORLD_STARTER_REGION_CONTRACT.localOrigin.x,
  localOriginY: CHAT_WORLD_STARTER_REGION_CONTRACT.localOrigin.y,
  localOriginZ: CHAT_WORLD_STARTER_REGION_CONTRACT.localOrigin.z,
  starterPylonSiteOffsetX: CHAT_WORLD_STARTER_REGION_CONTRACT.starterPylonSiteOffset.x,
  starterPylonSiteOffsetY: CHAT_WORLD_STARTER_REGION_CONTRACT.starterPylonSiteOffset.y,
  starterPylonSiteOffsetZ: CHAT_WORLD_STARTER_REGION_CONTRACT.starterPylonSiteOffset.z,
  streetPrevRegionRef: CHAT_WORLD_STARTER_REGION_CONTRACT.streetPrevRegionRef,
  streetNextRegionRef: CHAT_WORLD_STARTER_REGION_CONTRACT.streetNextRegionRef,
  proximityRadiusMeters: CHAT_WORLD_STARTER_REGION_CONTRACT.proximityRadiusMeters,
  avatarPositionMinIntervalMs: CHAT_WORLD_STARTER_REGION_CONTRACT.avatarPositionMinIntervalMs,
  staleAvatarPositionMs: CHAT_WORLD_STARTER_REGION_CONTRACT.staleAvatarPositionMs,
}

describe("projectChatWorldCloudflareRows", () => {
  test("maps generated Cloudflare world rows into the public Verse projection", () => {
    const projection = projectChatWorldCloudflareRows({
      flagEnabled: true,
      runRef,
      nowMs: 10_000,
      worldUrl: "https://world.openagents.com",
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
    expect(projection.world.projectedAtMs).toBe(10_000)
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
      lastSeenEpochMs: 9_000,
      chatMessages: ["near the pylon"],
      attentionRefs: ["attention.public.1"],
    })
    expect(projection.world.proximityChatCount).toBe(1)
  })

  test("keeps the generated Street region metadata in the normalized projection", () => {
    const projection = projectChatWorldCloudflareRows({
      flagEnabled: true,
      runRef,
      nowMs: 10_000,
      rows: {
        regions: [regionRow],
        stations: [],
        avatars: [],
        positions: [],
        messages: [],
        attention: [],
      },
    })

    expect(projection.regions[0]).toMatchObject({
      maxX: 160,
      maxY: 40,
      maxZ: 160,
      minX: -160,
      minY: 0,
      minZ: -160,
      roadDirectionX: 0,
      roadDirectionY: 0,
      roadDirectionZ: 1,
      localOriginX: 0,
      localOriginY: 0,
      localOriginZ: 0,
      starterPylonSiteOffsetX: 24,
      starterPylonSiteOffsetY: 0,
      starterPylonSiteOffsetZ: 0,
      streetNextRegionRef: "region.run.tassadar.executor.20260615.street.next",
      streetPrevRegionRef: "region.run.tassadar.executor.20260615.street.prev",
      staleAvatarPositionMs: 20_000,
    })
  })

  test("disconnect fallback is explicit and inert", () => {
    const projection = projectChatWorldCloudflareRows({
      flagEnabled: true,
      runRef,
      rows: null,
      nowMs: 10_000,
    })

    expect(projection.regions).toEqual([])
    expect(projection.world.connected).toBe(false)
    expect(projection.world.projectedAtMs).toBe(10_000)
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
        // Defaults to the stable single-character id when OA_CHARACTER is unset.
        characterId: "main",
      },
    })
  })

  test("threads an explicit characterId into the position write", () => {
    const plan = planChatWorldAvatarPositionWrite({
      region: regionRow,
      previous: null,
      nowMs: 1_000,
      x: 0,
      y: 0,
      z: 0,
      characterId: "alt",
    })
    expect(plan.ok).toBe(true)
    if (plan.ok) {
      // One account moving its "alt" character: the world module derives a
      // distinct avatar_ref from sender + "alt", so this is its own avatar.
      expect(plan.write.characterId).toBe("alt")
    }
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
        x: 999,
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
          lastSeenEpochMs: 2_950,
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

  test("accepts Street-scale positions and rejects writes beyond the starter chunk", () => {
    expect(
      planChatWorldAvatarPositionWrite({
        region: regionRow,
        previous: null,
        nowMs: 3_000,
        x: 155,
        y: 2,
        z: -150,
      }),
    ).toMatchObject({ ok: true })

    expect(
      planChatWorldAvatarPositionWrite({
        region: regionRow,
        previous: null,
        nowMs: 3_000,
        x: 161,
        y: 2,
        z: -150,
      }),
    ).toMatchObject({ ok: false, reason: "position outside region bounds" })
  })

  test("finds the default region for the active Tassadar run", () => {
    expect(defaultChatWorldRegionForRun([regionRow], runRef)).toBe(regionRow)
    expect(defaultChatWorldRegionForRun([], runRef)).toBeNull()
  })
})
