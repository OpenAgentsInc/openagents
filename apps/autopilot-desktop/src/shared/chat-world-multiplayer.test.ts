import { describe, expect, test } from "bun:test"

import {
  estimateChatWorldPresenceFeedLoad,
  chatWorldMultiplayerSubscriptionQueries,
  chatWorldRegionRefForRun,
  projectChatWorldMultiplayer,
} from "./chat-world-multiplayer.js"

const runRef = "run.tassadar.executor.20260615"
const regionRef = chatWorldRegionRefForRun(runRef)

describe("chat world multiplayer projection (#5739)", () => {
  test("builds the desktop SpacetimeDB subscription query set", () => {
    const queries = chatWorldMultiplayerSubscriptionQueries(runRef)
    expect(queries).toContain(`SELECT * FROM world_region WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT * FROM pylon_station WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT agent_avatar.* FROM avatar_position JOIN agent_avatar ON avatar_position.avatar_ref = agent_avatar.avatar_ref WHERE avatar_position.region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT * FROM avatar_position WHERE avatar_position.region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT * FROM local_chat_message WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT pylon_attention.* FROM pylon_station JOIN pylon_attention ON pylon_station.pylon_ref = pylon_attention.pylon_ref WHERE pylon_station.region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT chat_bubble.* FROM local_chat_message JOIN chat_bubble ON local_chat_message.message_ref = chat_bubble.message_ref WHERE local_chat_message.region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT * FROM local_emote WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT agent_intent.* FROM avatar_position JOIN agent_intent ON avatar_position.avatar_ref = agent_intent.avatar_ref WHERE avatar_position.region_ref = '${regionRef}'`)
    expect(queries).not.toContain("SELECT * FROM agent_avatar")
    expect(queries).not.toContain("SELECT * FROM pylon_attention")
    expect(queries).not.toContain("SELECT * FROM chat_bubble")
    expect(queries).not.toContain("SELECT * FROM agent_intent")
  })

  test("can build split high/low resolution presence subscriptions", () => {
    const queries = chatWorldMultiplayerSubscriptionQueries(runRef, {
      centerX: 0,
      centerZ: 0,
      mode: "split-near-far",
      nearRadiusMeters: 64,
    })

    expect(queries).toContain(`SELECT agent_avatar.* FROM avatar_position_near JOIN agent_avatar ON avatar_position_near.avatar_ref = agent_avatar.avatar_ref WHERE avatar_position_near.region_ref = '${regionRef}' AND avatar_position_near.position_x >= -64 AND avatar_position_near.position_x <= 64 AND avatar_position_near.position_z >= -64 AND avatar_position_near.position_z <= 64`)
    expect(queries).toContain(`SELECT agent_avatar.* FROM avatar_position_far JOIN agent_avatar ON avatar_position_far.avatar_ref = agent_avatar.avatar_ref WHERE avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_x < -64 OR avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_x > 64 OR avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_z < -64 OR avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_z > 64`)
    expect(queries).toContain(`SELECT * FROM avatar_position_near WHERE avatar_position_near.region_ref = '${regionRef}' AND avatar_position_near.position_x >= -64 AND avatar_position_near.position_x <= 64 AND avatar_position_near.position_z >= -64 AND avatar_position_near.position_z <= 64`)
    expect(queries).toContain(`SELECT * FROM avatar_position_far WHERE avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_x < -64 OR avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_x > 64 OR avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_z < -64 OR avatar_position_far.region_ref = '${regionRef}' AND avatar_position_far.position_z > 64`)
    expect(queries).not.toContain(`SELECT * FROM avatar_position WHERE region_ref = '${regionRef}'`)
  })

  test("estimates when the single region feed should split", () => {
    expect(
      estimateChatWorldPresenceFeedLoad({
        avatarCount: 32,
        highUpdateIntervalMs: 100,
      }),
    ).toMatchObject({
      recommendedMode: "single-region",
      singleFeedRowsPerSecond: 320,
    })

    expect(
      estimateChatWorldPresenceFeedLoad({
        avatarCount: 160,
        highUpdateIntervalMs: 100,
        nearAvatarCount: 32,
      }),
    ).toMatchObject({
      recommendedMode: "split-near-far",
      splitNearRowsPerSecond: 320,
      splitFarRowsPerSecond: 128,
    })
  })

  test("projects only row-backed avatars with live positions and chat", () => {
    const otherRegionRef = "region.other.main"
    const projection = projectChatWorldMultiplayer({
      flagEnabled: true,
      runRef,
      nowMs: 1_000,
      rows: {
        stations: [{
          pylonRef: "pylon.public.1",
          runRef,
          regionRef,
          x: 1,
          y: 0,
          z: 2,
          label: "Pylon 1",
        }, {
          pylonRef: "pylon.other.1",
          runRef,
          regionRef: otherRegionRef,
          x: 9,
          y: 0,
          z: 9,
          label: "Other Pylon",
        }],
        avatars: [{
          avatarRef: "avatar.public.1",
          displayName: "Agent One",
          avatarKind: "pylon_agent",
          actorRef: "agent.public.1",
          colorHex: "#ffffff",
        }],
        positions: [
          {
            avatarRef: "avatar.public.1",
            regionRef,
            x: 3,
            y: 0,
            z: 4,
            yaw: 0.5,
            movementMode: "walk",
            lastSeenEpochMs: 900,
            presenceFeed: "low",
          },
          {
            avatarRef: "avatar.public.1",
            regionRef,
            x: 4,
            y: 0,
            z: 5,
            yaw: 0.75,
            movementMode: "running",
            lastSeenEpochMs: 850,
            presenceFeed: "high",
          },
          {
            avatarRef: "avatar.public.1",
            regionRef: otherRegionRef,
            x: 7,
            y: 0,
            z: 7,
            yaw: 0,
            movementMode: "walk",
            lastSeenEpochMs: 900,
          },
          {
            avatarRef: "avatar.missing",
            regionRef,
            x: 9,
            y: 0,
            z: 9,
            yaw: 0,
            movementMode: "walk",
            lastSeenEpochMs: 900,
          },
        ],
        messages: [{
          messageRef: "chat.public.1",
          avatarRef: "avatar.public.1",
          regionRef,
          text: "nearby",
          radiusMeters: 12,
          expiresAtEpochMs: 2_000,
        }],
        attention: [{
          attentionRef: "attention.public.1",
          avatarRef: "avatar.public.1",
          pylonRef: "pylon.public.1",
          attentionKind: "nearby",
          expiresAtEpochMs: 2_000,
        }, {
          attentionRef: "attention.other.1",
          avatarRef: "avatar.public.1",
          pylonRef: "pylon.other.1",
          attentionKind: "nearby",
          expiresAtEpochMs: 2_000,
        }],
      },
    })

    expect(projection.connected).toBe(true)
    expect(projection.projectedAtMs).toBe(1_000)
    expect(projection.stations).toHaveLength(1)
    expect(projection.stations.map(station => station.pylonRef)).toEqual(["pylon.public.1"])
    expect(projection.agents).toHaveLength(1)
    expect(projection.agents[0]).toMatchObject({
      lastSeenEpochMs: 850,
      movementMode: "running",
      presenceFeed: "high",
      x: 4,
      z: 5,
    })
    expect(projection.agents[0]?.chatMessages).toEqual(["nearby"])
    expect(projection.agents[0]?.attentionRefs).toEqual(["attention.public.1"])
    expect(projection.proximityChatCount).toBe(1)
  })

  test("stays inert when the multiplayer flag is off", () => {
    const projection = projectChatWorldMultiplayer({
      flagEnabled: false,
      runRef,
      nowMs: 1_000,
      rows: {
        stations: [],
        avatars: [],
        positions: [],
        messages: [],
        attention: [],
      },
    })

    expect(projection.connected).toBe(false)
    expect(projection.projectedAtMs).toBe(1_000)
    expect(projection.agents).toEqual([])
  })
})
