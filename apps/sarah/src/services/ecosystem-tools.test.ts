/**
 * KHS-9 (#8608) live ecosystem tool tests. No network, no database: fetch is
 * injected with fake public-endpoint payloads and the embedder is the shared
 * semantic-answer-cache test override (the same lane production uses), so the
 * semantic-match law is exercised for real — cosine over embeddings, never a
 * keyword route.
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test"

import {
  __setSarahEmbedderForTest,
  __resetSarahAnswerCacheForTest,
} from "./semantic-answer-cache.ts"
import {
  __resetSarahEcosystemForTest,
  __setSarahEcosystemFetchForTest,
  ECOSYSTEM_GROUNDING_INTENTS,
  liveStats,
  maybeEcosystemGrounding,
  planCatalog,
  promiseLookup,
  promiseStateCaveat,
  sarahEcosystemGroundingEnabled,
} from "./ecosystem-tools.ts"

// Deterministic fake embedding space: pinned texts get pinned vectors; every
// other text gets a stable pseudo-random vector (near-orthogonal in 32 dims).
const DIM = 32

function hashVec(text: string): number[] {
  let h = 2166136261
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  let x = h || 1
  const v: number[] = []
  for (let i = 0; i < DIM; i++) {
    x ^= x << 13
    x ^= x >>> 17
    x ^= x << 5
    v.push(((x >>> 0) % 2000) / 1000 - 1)
  }
  return v
}

function axis(index: number): number[] {
  const v = new Array<number>(DIM).fill(0)
  v[index] = 1
  return v
}

const pinned = new Map<string, number[]>()
let embedCalls = 0

function installFakeEmbedder(): void {
  __setSarahEmbedderForTest(async (text) => {
    embedCalls += 1
    return pinned.get(text) ?? hashVec(text)
  })
}

const FAKE_REGISTRY = {
  promises: [
    {
      promiseId: "khala.free_api.v1",
      state: "green",
      claim: "Free OpenAI-compatible Khala API",
      safeCopy: "The free Khala API is live with a public tokens-served counter.",
      unsafeCopy: "SECRET-UNSAFE-COPY-NEVER-SERVE",
    },
    {
      promiseId: "business.workspace_packs.v1",
      state: "yellow",
      claim: "Prefilled business workspace packs",
      safeCopy: "Business workspace packs are delivered operator-assisted.",
      blockerRefs: ["blocker.workspaces.self_serve_not_live"],
    },
    {
      promiseId: "khala_code.mobile.v1",
      state: "planned",
      claim: "Khala Code mobile app-store availability",
      safeCopy: "Khala Code mobile is roadmap only.",
    },
  ],
}

// registryRecordText embeds `${promiseId}: ${claim}` (whitespace-normalized).
const GREEN_RECORD_TEXT = "khala.free_api.v1: Free OpenAI-compatible Khala API"
const YELLOW_RECORD_TEXT =
  "business.workspace_packs.v1: Prefilled business workspace packs"

const FAKE_TOKENS = {
  schemaVersion: "openagents.public_khala_tokens_served.v1",
  tokensServed: 123_456_789,
  generatedAt: "2026-07-09T12:00:00.000Z",
}

const FAKE_PYLONS = {
  pylonsOnlineNow: 3,
  pylonsSeen24h: 7,
  pylonsRegisteredTotal: 42,
}

const FAKE_PLANS = {
  catalog: {
    summary: "Two-plan structure: Free (pay with data) and Paid (private data).",
    promiseId: "khala_code.free_paid_plans.v1",
    plans: [
      {
        planId: "khala_code.plan.free.v1",
        kind: "free",
        label: "Khala Code Free",
        priceLabel: "$0",
        isDefault: true,
        captureExcluded: false,
      },
      {
        planId: "khala_code.plan.paid.v1",
        kind: "paid",
        label: "Khala Code Paid privacy",
        priceLabel: "TBD",
        isDefault: false,
        captureExcluded: true,
        purchase: { armed: false },
      },
    ],
  },
}

let fetchCalls: string[] = []

function installFakeFetch(): void {
  __setSarahEcosystemFetchForTest(async (url) => {
    fetchCalls.push(url)
    if (url.endsWith("/api/public/product-promises")) {
      return Response.json(FAKE_REGISTRY)
    }
    if (url.endsWith("/api/public/khala-tokens-served")) {
      return Response.json(FAKE_TOKENS)
    }
    if (url.endsWith("/api/public/pylon-stats")) {
      return Response.json(FAKE_PYLONS)
    }
    if (url.endsWith("/api/public/khala-code/plans")) {
      return Response.json(FAKE_PLANS)
    }
    return new Response("not found", { status: 404 })
  })
}

beforeEach(() => {
  __resetSarahEcosystemForTest()
  __resetSarahAnswerCacheForTest()
  pinned.clear()
  embedCalls = 0
  fetchCalls = []
  installFakeEmbedder()
  installFakeFetch()
  delete process.env.SARAH_ECOSYSTEM_GROUNDING
})

afterEach(() => {
  __resetSarahEcosystemForTest()
  __resetSarahAnswerCacheForTest()
  delete process.env.SARAH_ECOSYSTEM_GROUNDING
})

describe("promise_lookup (semantic match over the live registry)", () => {
  test("matches by embedding and returns safe copy with a state cap", async () => {
    const query = "is your free api actually live right now?"
    pinned.set(query, axis(0))
    pinned.set(GREEN_RECORD_TEXT, axis(0))

    const result = await promiseLookup(query)
    expect(result.ok).toBe(true)
    expect(result.matches.length).toBeGreaterThanOrEqual(1)
    const top = result.matches[0]!
    expect(top.promiseId).toBe("khala.free_api.v1")
    expect(top.state).toBe("green")
    expect(top.safeCopy).toContain("free Khala API is live")
    expect(top.caveat).toContain("safe copy")
  })

  test("yellow matches always carry operator-assisted caveat wording", async () => {
    const query = "can you set up a prefilled workspace for my business?"
    pinned.set(query, axis(1))
    pinned.set(YELLOW_RECORD_TEXT, axis(1))

    const result = await promiseLookup(query)
    expect(result.ok).toBe(true)
    const yellow = result.matches.find(
      (match) => match.promiseId === "business.workspace_packs.v1",
    )
    expect(yellow).toBeDefined()
    expect(yellow!.state).toBe("yellow")
    expect(yellow!.caveat).toContain("Operator-assisted")
    expect(yellow!.caveat).toContain("say this caveat plainly")
  })

  test("unsafeCopy never leaves the tool output", async () => {
    const query = "is your free api actually live right now?"
    pinned.set(query, axis(0))
    pinned.set(GREEN_RECORD_TEXT, axis(0))

    const result = await promiseLookup(query)
    expect(JSON.stringify(result)).not.toContain("SECRET-UNSAFE")
  })

  test("no embedder means an honest miss, never a keyword fallback", async () => {
    __setSarahEmbedderForTest(async () => null)
    const result = await promiseLookup("is the free api live?")
    expect(result.ok).toBe(false)
    expect(result.error).toBe("embedding_unavailable")
    expect(result.matches).toEqual([])
  })

  test("fetch failure degrades softly", async () => {
    __setSarahEcosystemFetchForTest(async () => {
      throw new Error("network down")
    })
    const result = await promiseLookup("anything")
    expect(result.ok).toBe(false)
    expect(result.matches).toEqual([])
  })

  test("state caveats cap every non-green state", () => {
    expect(promiseStateCaveat("planned")).toContain("Do not pitch")
    expect(promiseStateCaveat("red")).toContain("never available")
    expect(promiseStateCaveat("withdrawn")).toContain("Retired")
    expect(promiseStateCaveat("mystery")).toContain("cannot promise")
  })
})

describe("live_stats + plan_catalog (public projections, fail-soft)", () => {
  test("live stats returns tokens served and pylon counts with sources", async () => {
    const stats = await liveStats()
    expect(stats.ok).toBe(true)
    expect(stats.tokensServed).toBe(123_456_789)
    expect(stats.pylons.onlineNow).toBe(3)
    expect(stats.pylons.registeredTotal).toBe(42)
    expect(stats.sources.some((s) => s.includes("khala-tokens-served"))).toBe(true)
  })

  test("plan catalog passes through honest purchasability", async () => {
    const catalog = await planCatalog()
    expect(catalog.ok).toBe(true)
    expect(catalog.promiseId).toBe("khala_code.free_paid_plans.v1")
    const paid = catalog.plans.find((plan) => plan.kind === "paid")
    expect(paid?.purchasable).toBe(false)
    const free = catalog.plans.find((plan) => plan.kind === "free")
    expect(free?.isDefault).toBe(true)
  })

  test("both endpoints down is an honest ok:false", async () => {
    __setSarahEcosystemFetchForTest(async () => new Response("boom", { status: 500 }))
    const stats = await liveStats()
    expect(stats.ok).toBe(false)
    expect(stats.tokensServed).toBeNull()
  })
})

describe("ecosystem grounding hook (flag-gated, embedding-routed)", () => {
  const LIVE_STATS_CANONICAL = ECOSYSTEM_GROUNDING_INTENTS.find(
    (intent) => intent.intent === "live_stats",
  )!.canonical

  test("flag off: null, and neither the embedder nor the network is touched", async () => {
    delete process.env.SARAH_ECOSYSTEM_GROUNDING
    expect(sarahEcosystemGroundingEnabled()).toBe(false)
    const result = await maybeEcosystemGrounding(
      "how many tokens have you served?",
    )
    expect(result).toBeNull()
    expect(embedCalls).toBe(0)
    expect(fetchCalls).toEqual([])
  })

  test("flag on: a live-stats-shaped question gets a [live product truth] block", async () => {
    process.env.SARAH_ECOSYSTEM_GROUNDING = "1"
    const query = "how many tokens have you served so far?"
    pinned.set(query, axis(2))
    pinned.set(LIVE_STATS_CANONICAL, axis(2))

    const block = await maybeEcosystemGrounding(query)
    expect(block).not.toBeNull()
    expect(block!).toContain("[live product truth]")
    expect(block!).toContain("123456789")
    expect(block!).toContain("openagents.com/AGENTS.md")
  })

  test("flag on but no intent match: null (no forced grounding)", async () => {
    process.env.SARAH_ECOSYSTEM_GROUNDING = "1"
    // Unpinned texts embed to near-orthogonal random vectors — below the
    // grounding threshold, so no intent is selected.
    const block = await maybeEcosystemGrounding(
      "tell me a story about your weekend",
    )
    expect(block).toBeNull()
  })

  test("flag on with a promise-status question includes state-capped records", async () => {
    process.env.SARAH_ECOSYSTEM_GROUNDING = "1"
    const canonical = ECOSYSTEM_GROUNDING_INTENTS.find(
      (intent) => intent.intent === "promise_status",
    )!.canonical
    const query = "is the workspace pack capability live yet?"
    pinned.set(query, axis(3))
    pinned.set(canonical, axis(3))
    pinned.set(YELLOW_RECORD_TEXT, axis(3))

    const block = await maybeEcosystemGrounding(query)
    expect(block).not.toBeNull()
    expect(block!).toContain("business.workspace_packs.v1")
    expect(block!).toContain("[yellow]")
    expect(block!).toContain("Operator-assisted")
    expect(block!).not.toContain("SECRET-UNSAFE")
  })
})
