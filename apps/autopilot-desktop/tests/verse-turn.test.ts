import { describe, expect, test } from "bun:test"

import {
  VERSE_SYSTEM_PROMPT,
  buildVerseTurn,
} from "../src/bun/verse-turn"

const now = "2026-06-20T18:00:00.000Z"

const tassadarSummary = {
  schemaVersion: "openagents.public_tassadar_run_summary.v1",
  runRef: "run.tassadar.executor.20260615",
  runState: "active",
  generatedAt: now,
  corpus: { acceptedTraceCount: 11 },
  metrics: {
    providerConfirmedSettledPayoutSats: { value: 1020 },
    qualifiedContributorCount: { value: 5 },
  },
}

const pylonStats = {
  available: true,
  status: "live",
  pylonsOnlineNow: 7,
  pylonsAssignmentReadyNow: 4,
  pylonsWalletReadyNow: 5,
  nip90MarketSettlementStats: {
    compute: { satsSettled24h: 2, satsSettledTotal: 10 },
    data: { satsSettled24h: 3, satsSettledTotal: 20 },
    labor: { satsSettled24h: 4, satsSettledTotal: 30 },
  },
}

const activityTimeline = {
  schemaVersion: "openagents.public_activity_timeline.v1",
  generatedAt: now,
  staleness: {
    composition: "live_at_read",
    contractVersion: "projection_staleness.v1",
    maxStalenessSeconds: 0,
    rebuildsOn: ["training_verification_challenge_verified_transition_recorded"],
  },
  nextCursor: null,
  sourceLag: [],
  events: [
    {
      eventRef: "activity.real_bitcoin_moved.1",
      cursor: "2026-06-20T18:00:00.000Z:settlement_receipt:activity.real_bitcoin_moved.1",
      ts: now,
      kind: "real_bitcoin_moved",
      sourceKind: "settlement_receipt",
      refs: ["receipt.real.1"],
      sourceRefs: [
        "receipt.real.1",
        "route:/api/public/tassadar-run-summary",
      ],
      blockerRefs: [],
      caveatRefs: [],
      amountSats: 5,
      realBitcoinMoved: true,
      state: "settled",
      text: "5 real sats settled for a verified Tassadar contribution.",
    },
  ],
}

const productPromises = {
  registryVersion: "2026-06-20.42",
  promises: [
    {
      promiseId: "training.decentralized_training_launch.v1",
      productArea: "training",
      claim: "Tassadar training launch",
      safeCopy: "Bounded live training run",
      verification: "receipt backed",
      state: "green",
      evidenceRefs: ["receipt.real.1"],
      blockerRefs: [],
    },
    {
      promiseId: "training.public_distributed_training_run.v1",
      productArea: "training",
      claim: "Public distributed run",
      safeCopy: "Scale claim not yet green",
      verification: "needs broad accepted work",
      state: "red",
      evidenceRefs: [],
      blockerRefs: ["network_scale_threshold_missing"],
    },
  ],
}

describe("buildVerseTurn (#5821)", () => {
  test("missing token returns one clean Verse blocker and never calls the network", async () => {
    let called = false
    const fetchFn = (() => {
      called = true
      return Promise.resolve(new Response("{}"))
    }) as unknown as typeof fetch

    const result = await buildVerseTurn({
      prompt: "What is my Pylon doing?",
      env: {},
      agentToken: null,
      fetchFn,
      nowIso: () => now,
    })

    expect(called).toBe(false)
    expect(result.ok).toBe(false)
    expect(result.context.blockerRefs).toEqual(["verse.auth.token_missing"])
    expect(result.text).toContain("OpenAgents account token")
    expect(result.text).not.toContain("SpawnChatTurn")
    expect(result.text).not.toContain("session")
  })

  test("with a token, builds public Pylon/Tassadar context and posts one model turn", async () => {
    const requested: string[] = []
    let seenAuth: string | null = null
    let seenBody: unknown = null
    const fetchFn = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input)
      requested.push(url)
      if (url.endsWith("/api/public/tassadar-run-summary")) {
        return Promise.resolve(new Response(JSON.stringify(tassadarSummary)))
      }
      if (url.endsWith("/api/public/pylon-stats")) {
        return Promise.resolve(new Response(JSON.stringify(pylonStats)))
      }
      if (url.includes("/api/public/activity-timeline")) {
        return Promise.resolve(new Response(JSON.stringify(activityTimeline)))
      }
      if (url.endsWith("/api/public/product-promises")) {
        return Promise.resolve(new Response(JSON.stringify(productPromises)))
      }
      if (url.endsWith("/api/v1/chat/completions")) {
        seenAuth =
          (init?.headers as Record<string, string> | undefined)?.authorization ??
          null
        seenBody = init?.body ? JSON.parse(init.body as string) : null
        return Promise.resolve(
          new Response(
            JSON.stringify({
              choices: [
                {
                  message: {
                    role: "assistant",
                    content: "Tassadar sees 7 pylons online and 11 accepted traces.",
                  },
                },
              ],
            }),
          ),
        )
      }
      return Promise.resolve(new Response("{}", { status: 404 }))
    }) as unknown as typeof fetch

    const result = await buildVerseTurn({
      prompt: "What's happening in the Verse?",
      env: { OPENAGENTS_COM_BASE_URL: "https://openagents.test" },
      agentToken: "oa_agent_secret",
      fetchFn,
      nowIso: () => now,
    })

    expect(requested).toContain("https://openagents.test/api/public/tassadar-run-summary")
    expect(requested).toContain("https://openagents.test/api/public/pylon-stats")
    expect(requested).toContain("https://openagents.test/api/public/product-promises")
    expect(requested.some(url => url.includes("/api/public/activity-timeline?limit=8"))).toBe(true)
    expect(requested).toContain("https://openagents.test/api/v1/chat/completions")
    expect(seenAuth).toBe("Bearer oa_agent_secret")
    expect((seenBody as { messages: Array<{ role: string; content: string }> }).messages[0]).toEqual({
      role: "system",
      content: VERSE_SYSTEM_PROMPT,
    })
    const modelPrompt =
      (seenBody as { messages: Array<{ content: string }> }).messages[1]?.content ?? ""
    expect(modelPrompt).toContain("online=7")
    expect(modelPrompt).toContain("acceptedTraceCount=11")
    expect(modelPrompt).toContain("qualifiedContributors=5")
    expect(modelPrompt).toContain("settledPayoutSats=1,020")
    expect(modelPrompt).toContain("5 real sats settled")
    expect(JSON.stringify(result)).not.toContain("oa_agent_secret")
    expect(result.ok).toBe(true)
    expect(result.context.pylon.onlineNow).toBe(7)
    expect(result.context.pylon.satsSettledTotal).toBe(60)
    expect(result.context.training.acceptedTraceCount).toBe(11)
    expect(result.context.promises.green).toBe(1)
    expect(result.text).toContain("7 pylons")
  })

  test("gateway credit exhaustion is a clean Verse blocker, not session-spawn jargon", async () => {
    const fetchFn = ((input: RequestInfo | URL) => {
      const url = String(input)
      if (url.endsWith("/api/public/tassadar-run-summary")) {
        return Promise.resolve(new Response(JSON.stringify(tassadarSummary)))
      }
      if (url.endsWith("/api/public/pylon-stats")) {
        return Promise.resolve(new Response(JSON.stringify(pylonStats)))
      }
      if (url.includes("/api/public/activity-timeline")) {
        return Promise.resolve(new Response(JSON.stringify(activityTimeline)))
      }
      if (url.endsWith("/api/public/product-promises")) {
        return Promise.resolve(new Response(JSON.stringify(productPromises)))
      }
      return Promise.resolve(
        new Response(JSON.stringify({ error: "no credits" }), { status: 402 }),
      )
    }) as unknown as typeof fetch

    const result = await buildVerseTurn({
      prompt: "Talk to Tassadar",
      env: { OPENAGENTS_COM_BASE_URL: "https://openagents.test" },
      agentToken: "oa_agent_secret",
      fetchFn,
      nowIso: () => now,
    })

    expect(result.ok).toBe(false)
    expect(result.context.blockerRefs).toContain("verse.billing.allowance_exhausted")
    expect(result.text).toContain("model allowance")
    expect(result.text).not.toContain("Codex")
    expect(result.text).not.toContain("Claude")
    expect(result.text).not.toContain("session")
  })
})
