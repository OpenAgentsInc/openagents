import { describe, expect, test } from "bun:test"

import { projectCharacterCreationOnboarding } from "./character-creation-onboarding"
import type { OnboardingStatusResponse } from "./onboarding-status"

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
    hasRetryableFailure: false,
    steps: ids.map(id => ({
      id,
      label: id,
      status: overrides[id] ?? "pending",
      message: `${id} message`,
      retryable: false,
    })),
  }
}

describe("character creation onboarding projection (#5738)", () => {
  test("maps real onboarding steps into character-creation beats", () => {
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
    })

    expect(projection.enabled).toBe(true)
    expect(projection.pylonOnlineCount).toBe(3)
    expect(projection.currentBeatId).toBe("customize")
    expect(projection.beats.map(beat => beat.id)).toEqual([
      "pylon-online",
      "agent-warp-in",
      "customize",
      "forum-intro",
      "work-search",
    ])
    expect(projection.beats[3]?.status).toBe("active")
  })

  test("completes once earned sats are observed", () => {
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
    })

    expect(projection.complete).toBe(true)
    expect(projection.mana).toBe(1)
    expect(projection.currentBeatId).toBe("work-search")
  })
})
