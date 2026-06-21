import { describe, expect, test } from "bun:test"

import {
  projectCharacterCreationOnboarding,
  type CharacterCreationBeat,
  type CharacterCreationForumReadiness,
} from "./character-creation-onboarding.js"
import type { OnboardingStatusResponse } from "./onboarding-status.js"

const onboarding = (
  overrides: Partial<Record<string, "pending" | "active" | "done" | "failed">>,
): OnboardingStatusResponse => {
  const ids = [
    "identity",
    "registered",
    "node-online",
    "wallet",
    "presence",
    "tassadar",
    "claimed",
    "earned",
  ] as const
  return {
    ok: true,
    fetchedAt: "2026-06-20T00:00:00.000Z",
    sourceUrl: "desktop:onboarding-status",
    complete: overrides.earned === "done",
    currentStepId: null,
    hasRetryableFailure: Object.values(overrides).includes("failed"),
    steps: ids.map(id => ({
      id,
      label: id,
      status: overrides[id] ?? "pending",
      message: `${id} message`,
      retryable: overrides[id] === "failed",
    })),
  }
}

const forumReady = (
  overrides: Partial<CharacterCreationForumReadiness> = {},
): CharacterCreationForumReadiness => ({
  ok: true,
  agentTokenPresent: true,
  forumTopicsUrl: "https://openagents.com/forum/f/product-promises",
  blockerRefs: [],
  ...overrides,
})

const beat = (
  beats: ReadonlyArray<CharacterCreationBeat>,
  id: CharacterCreationBeat["id"],
): CharacterCreationBeat => {
  const item = beats.find(candidate => candidate.id === id)
  if (item === undefined) throw new Error(`missing beat ${id}`)
  return item
}

describe("character creation onboarding projection (#5738/#5826)", () => {
  test("incomplete path maps real onboarding steps into hands-off beats", () => {
    const projection = projectCharacterCreationOnboarding({
      flagEnabled: true,
      onboardingStatus: onboarding({
        identity: "done",
        registered: "done",
        "node-online": "done",
        wallet: "active",
        presence: "pending",
      }),
      chatWorldScene: {
        empty: false,
        onlineNow: 3,
        nodes: [],
        growth: { tier: 0, scale: 1, facets: 6, brightness: 0, settledSats: 0 },
        asOfLabel: null,
      },
      forumReadiness: null,
    })

    expect(projection.enabled).toBe(true)
    expect(projection.complete).toBe(false)
    expect(projection.pylonOnlineCount).toBe(3)
    expect(projection.currentBeatId).toBe("customize")
    expect(projection.beats.map(item => item.id)).toEqual([
      "pylon-online",
      "agent-warp-in",
      "customize",
      "forum-intro",
      "work-search",
    ])
    expect(beat(projection.beats, "agent-warp-in").label).toBe("Agent spawned")
    expect(beat(projection.beats, "forum-intro").required).toBe(false)
    expect(beat(projection.beats, "forum-intro").status).toBe("active")
    expect(beat(projection.beats, "work-search").status).toBe("pending")
  })

  test("blocked Forum intro is explicit and optional; no surprise posting", () => {
    const projection = projectCharacterCreationOnboarding({
      flagEnabled: true,
      onboardingStatus: onboarding({
        identity: "done",
        registered: "done",
        "node-online": "done",
        wallet: "done",
        presence: "done",
        tassadar: "done",
        claimed: "done",
      }),
      chatWorldScene: null,
      forumReadiness: forumReady({
        ok: false,
        agentTokenPresent: false,
        blockerRefs: ["env.OPENAGENTS_AGENT_TOKEN", "/Users/private/token"],
      }),
    })

    const intro = beat(projection.beats, "forum-intro")
    expect(intro.status).toBe("blocked")
    expect(intro.required).toBe(false)
    expect(intro.message).toContain("nothing is posted")
    expect(intro.blockerRefs).toEqual(["env.OPENAGENTS_AGENT_TOKEN"])
    expect(intro.sourceRefs).toContain("https://openagents.com/forum/f/product-promises")
    expect(projection.complete).toBe(false)
    expect(projection.currentBeatId).toBe("work-search")
  })

  test("degraded onboarding paths surface a clean blocker in the active beat", () => {
    const projection = projectCharacterCreationOnboarding({
      flagEnabled: true,
      onboardingStatus: onboarding({
        identity: "done",
        registered: "done",
        "node-online": "failed",
        wallet: "pending",
      }),
      chatWorldScene: null,
      forumReadiness: forumReady(),
    })

    const pylon = beat(projection.beats, "pylon-online")
    expect(pylon.status).toBe("blocked")
    expect(pylon.blockerRefs).toEqual(["onboarding.node-online"])
    expect(projection.currentBeatId).toBe("pylon-online")
  })

  test("work search reports rejected when the real earned step fails", () => {
    const projection = projectCharacterCreationOnboarding({
      flagEnabled: true,
      onboardingStatus: onboarding({
        identity: "done",
        registered: "done",
        "node-online": "done",
        wallet: "done",
        presence: "done",
        tassadar: "done",
        claimed: "done",
        earned: "failed",
      }),
      chatWorldScene: null,
      forumReadiness: forumReady(),
    })

    const work = beat(projection.beats, "work-search")
    expect(work.status).toBe("rejected")
    expect(work.blockerRefs).toEqual(["onboarding.earned"])
    expect(projection.complete).toBe(false)
    expect(projection.currentBeatId).toBe("work-search")
  })

  test("complete path accepts work search once earned sats are observed", () => {
    const projection = projectCharacterCreationOnboarding({
      flagEnabled: true,
      onboardingStatus: onboarding({
        identity: "done",
        registered: "done",
        "node-online": "done",
        wallet: "done",
        presence: "done",
        tassadar: "done",
        claimed: "done",
        earned: "done",
      }),
      chatWorldScene: null,
      forumReadiness: forumReady({
        ok: false,
        agentTokenPresent: false,
        blockerRefs: ["env.OPENAGENTS_AGENT_TOKEN"],
      }),
    })

    const work = beat(projection.beats, "work-search")
    expect(work.status).toBe("accepted")
    expect(projection.complete).toBe(true)
    expect(projection.mana).toBe(1)
    expect(projection.currentBeatId).toBe("forum-intro")
  })
})
