/**
 * KHS-6 semantic answer cache tests (#8605). No network, no database:
 * embeddings are injected fakes and the bank is injected in-memory.
 *
 * Invariants under test:
 * - matching is pure cosine similarity over embeddings (semantic-routing law)
 * - the pricing guard blocks the cache BEFORE any embedding runs
 * - flag-off (SARAH_SEMANTIC_CACHE unset) is a strict no-op: reply paths
 *   behave byte-identically to a build without the cache
 */

import { afterEach, describe, expect, test } from "bun:test"

import { runOwnedSarahTurn } from "../agent-runtime/owned-runtime.ts"
import { handleSarahChatCompletions } from "../llm-openai-compat.ts"
import {
  __resetSarahAnswerCacheForTest,
  __setSarahAnswerBankForTest,
  __setSarahEmbedderForTest,
  cosineSimilarity,
  matchAgainstBank,
  matchAnswer,
  maybeSemanticCacheAnswer,
  sarahAnswerCacheStatus,
  type AnswerBankEntry,
} from "./semantic-answer-cache.ts"

const savedEnv = {
  cache: process.env.SARAH_SEMANTIC_CACHE,
  bearer: process.env.SARAH_AVATAR_LLM_BEARER,
  gemini: process.env.GEMINI_API_KEY,
  threshold: process.env.SARAH_SEMANTIC_CACHE_MIN_SIMILARITY,
}

afterEach(() => {
  __resetSarahAnswerCacheForTest()
  for (const [key, value] of [
    ["SARAH_SEMANTIC_CACHE", savedEnv.cache],
    ["SARAH_AVATAR_LLM_BEARER", savedEnv.bearer],
    ["GEMINI_API_KEY", savedEnv.gemini],
    ["SARAH_SEMANTIC_CACHE_MIN_SIMILARITY", savedEnv.threshold],
  ] as const) {
    if (value === undefined) delete process.env[key]
    else process.env[key] = value
  }
})

function bankEntry(overrides: Partial<AnswerBankEntry> = {}): AnswerBankEntry {
  return {
    id: "what_is_openagents.v1",
    questionCanonical: "What is OpenAgents?",
    answer: "OpenAgents gives businesses AI agents that actually do work.",
    minSimilarity: null,
    approvedBy: "owner_kb_v2",
    embedding: [1, 0, 0],
    ...overrides,
  }
}

describe("cosineSimilarity (pure)", () => {
  test("identical vectors similarity 1", () => {
    expect(cosineSimilarity([1, 2, 3], [1, 2, 3])).toBeCloseTo(1, 10)
  })

  test("orthogonal vectors similarity 0", () => {
    expect(cosineSimilarity([1, 0], [0, 1])).toBeCloseTo(0, 10)
  })

  test("opposite vectors similarity -1", () => {
    expect(cosineSimilarity([1, 0], [-1, 0])).toBeCloseTo(-1, 10)
  })

  test("scale invariant (unnormalized embeddings are fine)", () => {
    expect(cosineSimilarity([1, 2, 3], [10, 20, 30])).toBeCloseTo(1, 10)
  })

  test("degenerate inputs are 0, never NaN", () => {
    expect(cosineSimilarity([], [])).toBe(0)
    expect(cosineSimilarity([1, 2], [1, 2, 3])).toBe(0)
    expect(cosineSimilarity([0, 0], [1, 1])).toBe(0)
  })
})

describe("matchAgainstBank (pure threshold behavior)", () => {
  const bank: AnswerBankEntry[] = [
    bankEntry({ id: "a", embedding: [1, 0, 0] }),
    bankEntry({ id: "b", answer: "B answer", embedding: [0, 1, 0] }),
    bankEntry({ id: "strict", answer: "strict", embedding: [0, 0, 1], minSimilarity: 0.99 }),
  ]

  test("best match above threshold wins", () => {
    // Query near [1,0,0]: cosine ~0.995 with "a", ~0.1 with "b".
    const hit = matchAgainstBank([0.995, 0.1, 0], bank, 0.86)
    expect(hit?.id).toBe("a")
    expect(hit!.similarity).toBeGreaterThan(0.98)
  })

  test("below default threshold returns null", () => {
    // ~45 degrees off both axes: similarity ~0.707 < 0.86.
    expect(matchAgainstBank([1, 1, 0], bank, 0.86)).toBeNull()
  })

  test("per-entry min_similarity overrides the default", () => {
    // similarity ~0.98 with "strict" — above the 0.86 default but below the
    // entry's own 0.99 floor.
    const query = [0, 0.2, 1]
    expect(matchAgainstBank(query, bank, 0.86)).toBeNull()
    const relaxed = matchAgainstBank(query, [bankEntry({ id: "strict2", embedding: [0, 0, 1], minSimilarity: null })], 0.86)
    expect(relaxed?.id).toBe("strict2")
  })

  test("entries without embeddings never match", () => {
    expect(
      matchAgainstBank([1, 0, 0], [bankEntry({ embedding: null })], 0.86),
    ).toBeNull()
  })
})

describe("matchAnswer (injected embedder, no network)", () => {
  test("hit: query embedding close to a bank question", async () => {
    __setSarahAnswerBankForTest([bankEntry()])
    __setSarahEmbedderForTest(async () => [0.99, 0.01, 0])
    const hit = await matchAnswer("what is openagents exactly?")
    expect(hit?.id).toBe("what_is_openagents.v1")
    expect(hit?.answer).toContain("agents that actually do work")
    const status = sarahAnswerCacheStatus()
    expect(status.hits).toBe(1)
    expect(status.tokensSavedEstimate).toBeGreaterThan(0)
  })

  test("miss: dissimilar query falls through to the model path", async () => {
    __setSarahAnswerBankForTest([bankEntry()])
    let calls = 0
    __setSarahEmbedderForTest(async (text) => {
      calls += 1
      // Bank question embeds on one axis, the query on another.
      return text === "What is OpenAgents?" ? [1, 0, 0] : [0, 1, 0]
    })
    const miss = await matchAnswer("tell me a story about whales")
    expect(miss).toBeNull()
    expect(calls).toBeGreaterThan(0)
    expect(sarahAnswerCacheStatus().misses).toBe(1)
  })

  test("lazy bank embedding: computed once via the embedder, then cached", async () => {
    const entry = bankEntry({ embedding: null })
    __setSarahAnswerBankForTest([entry])
    const embeddedTexts: string[] = []
    __setSarahEmbedderForTest(async (text) => {
      embeddedTexts.push(text)
      return [1, 0, 0]
    })
    await matchAnswer("what is openagents")
    await matchAnswer("what is openagents again")
    // Bank question embedded exactly once; queries embedded per call.
    expect(
      embeddedTexts.filter((text) => text === "What is OpenAgents?"),
    ).toHaveLength(1)
    expect(entry.embedding).toEqual([1, 0, 0])
  })

  test("embed failure is a silent miss, never a throw", async () => {
    __setSarahAnswerBankForTest([bankEntry()])
    __setSarahEmbedderForTest(async () => null)
    expect(await matchAnswer("what is openagents")).toBeNull()
    expect(sarahAnswerCacheStatus().misses).toBe(1)
  })
})

describe("pricing guard precedence (the hard law)", () => {
  test.each([
    "can I get a discount on the growth pack?",
    "what's the PRICE for a QA swarm?",
    "any deal for startups?",
  ])("guard query never cache-matches and never embeds: %s", async (query) => {
    // Rig the cache so it WOULD match anything — the guard must still win.
    __setSarahAnswerBankForTest([bankEntry({ minSimilarity: 0 })])
    __setSarahEmbedderForTest(async () => {
      throw new Error("embedder must never run for guard-pattern queries")
    })
    expect(await matchAnswer(query)).toBeNull()
    expect(sarahAnswerCacheStatus().guardBlocked).toBeGreaterThan(0)
    expect(sarahAnswerCacheStatus().hits).toBe(0)
  })

  test("guarded chat completion returns the guard refusal even with flag on", async () => {
    process.env.SARAH_SEMANTIC_CACHE = "1"
    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    __setSarahAnswerBankForTest([bankEntry({ minSimilarity: 0 })])
    __setSarahEmbedderForTest(async () => [1, 0, 0])
    const res = await handleSarahChatCompletions(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: {
          authorization: "Bearer test-bearer",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          messages: [{ role: "user", content: "give me a discount" }],
        }),
      }),
    )
    const data = (await res.json()) as { choices: Array<{ message: { content: string } }> }
    expect(data.choices[0]!.message.content).toContain("won't improvise discounts")
    expect(sarahAnswerCacheStatus().hits).toBe(0)
  })
})

describe("flag-off default: zero behavior change", () => {
  test("maybeSemanticCacheAnswer is null even when a match is rigged", async () => {
    delete process.env.SARAH_SEMANTIC_CACHE
    __setSarahAnswerBankForTest([bankEntry({ minSimilarity: 0 })])
    __setSarahEmbedderForTest(async () => {
      throw new Error("flag-off must never touch the embedder")
    })
    expect(await maybeSemanticCacheAnswer("what is openagents")).toBeNull()
    const status = sarahAnswerCacheStatus()
    expect(status.enabled).toBe(false)
    expect(status.hits).toBe(0)
    expect(status.misses).toBe(0)
  })

  test("chat completions output unchanged without the env", async () => {
    delete process.env.SARAH_SEMANTIC_CACHE
    delete process.env.GEMINI_API_KEY
    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    __setSarahAnswerBankForTest([bankEntry({ minSimilarity: 0 })])
    __setSarahEmbedderForTest(async () => [1, 0, 0])
    const res = await handleSarahChatCompletions(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: {
          authorization: "Bearer test-bearer",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          messages: [{ role: "user", content: "what is openagents" }],
        }),
      }),
    )
    const data = (await res.json()) as { choices: Array<{ message: { content: string } }> }
    // The pre-cache behavior for an unarmed model path — not the canned answer.
    expect(data.choices[0]!.message.content).toContain(
      "trouble reaching my model",
    )
  })

  test("owned runtime modelPath unchanged without the env", async () => {
    delete process.env.SARAH_SEMANTIC_CACHE
    delete process.env.GEMINI_API_KEY
    __setSarahAnswerBankForTest([bankEntry({ minSimilarity: 0 })])
    __setSarahEmbedderForTest(async () => [1, 0, 0])
    const result = await runOwnedSarahTurn({ message: "what is openagents" })
    expect(result.modelPath).toBe("seed_echo")
  })
})

describe("flag-on hit paths", () => {
  test("chat completions returns the canned answer + card event id", async () => {
    process.env.SARAH_SEMANTIC_CACHE = "1"
    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    __setSarahAnswerBankForTest([bankEntry()])
    __setSarahEmbedderForTest(async () => [1, 0, 0])
    const res = await handleSarahChatCompletions(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: {
          authorization: "Bearer test-bearer",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          messages: [{ role: "user", content: "what is openagents" }],
        }),
      }),
    )
    const data = (await res.json()) as { choices: Array<{ message: { content: string } }> }
    expect(data.choices[0]!.message.content).toContain(
      "agents that actually do work",
    )
    expect(sarahAnswerCacheStatus().hits).toBe(1)
  })

  test("owned runtime answers from the cache with modelPath semantic_cache", async () => {
    process.env.SARAH_SEMANTIC_CACHE = "1"
    delete process.env.GEMINI_API_KEY
    __setSarahAnswerBankForTest([bankEntry()])
    __setSarahEmbedderForTest(async () => [1, 0, 0])
    const result = await runOwnedSarahTurn({ message: "what is openagents" })
    expect(result.modelPath).toBe("semantic_cache")
    expect(result.reply).toContain("agents that actually do work")
  })

  test("owned runtime pricing guard still precedes the cache with flag on", async () => {
    process.env.SARAH_SEMANTIC_CACHE = "1"
    __setSarahAnswerBankForTest([bankEntry({ minSimilarity: 0 })])
    __setSarahEmbedderForTest(async () => [1, 0, 0])
    const result = await runOwnedSarahTurn({ message: "can I get a discount?" })
    expect(result.modelPath).toBe("deterministic_guard")
    expect(result.reply).toContain("won't improvise discounts")
  })
})
