import { describe, expect, test } from "bun:test"

import {
  chatWorldMultiplayerSubscriptionQueries,
  chatWorldRegionRefForRun,
  projectChatWorldMultiplayer,
} from "./chat-world-multiplayer"

const runRef = "run.tassadar.executor.20260615"
const regionRef = chatWorldRegionRefForRun(runRef)

describe("chat world multiplayer projection (#5739)", () => {
  test("builds the desktop SpacetimeDB subscription query set", () => {
    const queries = chatWorldMultiplayerSubscriptionQueries(runRef)
    expect(queries).toContain(`SELECT * FROM world_region WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT * FROM pylon_station WHERE run_ref = '${runRef}'`)
    expect(queries).toContain("SELECT * FROM agent_avatar")
    expect(queries).toContain(`SELECT * FROM avatar_position WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain(`SELECT * FROM local_chat_message WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain("SELECT * FROM chat_bubble")
    expect(queries).toContain(`SELECT * FROM local_emote WHERE region_ref = '${regionRef}'`)
    expect(queries).toContain("SELECT * FROM agent_intent")
  })

  test("projects only row-backed avatars with live positions and chat", () => {
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
        }],
      },
    })

    expect(projection.connected).toBe(true)
    expect(projection.stations).toHaveLength(1)
    expect(projection.agents).toHaveLength(1)
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
    expect(projection.agents).toEqual([])
  })
})
