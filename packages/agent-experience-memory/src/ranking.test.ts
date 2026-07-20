import { describe, expect, test } from "vite-plus/test";

import {
  cosineSimilarity,
  estimateTokens,
  packWithinBudget,
  recallOrderBySalience,
  topK,
} from "./ranking.js";

describe("ranking primitives", () => {
  test("cosine similarity is 1 for identical vectors and 0 for a zero vector", () => {
    expect(cosineSimilarity([1, 0], [1, 0])).toBe(1);
    expect(cosineSimilarity([1, 2, 3], [0, 0, 0])).toBe(0);
  });

  test("cosine similarity rejects a length mismatch", () => {
    expect(() => cosineSimilarity([1, 2], [1])).toThrow();
  });

  test("topK is deterministic and tie-breaks by ref", () => {
    const items = [
      { ref: "b", embedding: [1, 0] },
      { ref: "a", embedding: [1, 0] },
      { ref: "c", embedding: [0, 1] },
    ] as const;
    const ranked = topK([1, 0], items, 2);
    expect(ranked.map((item) => item.ref)).toEqual(["a", "b"]);
  });

  test("topK returns nothing for k <= 0", () => {
    expect(topK([1], [{ ref: "a", embedding: [1] }], 0)).toEqual([]);
  });

  test("salience order combines salience and recency, stable on ties", () => {
    const order = recallOrderBySalience(
      [
        { ref: "old", salience: 0.5, lastUsedAt: 0 },
        { ref: "fresh", salience: 0.5, lastUsedAt: 100 },
      ],
      100,
    );
    expect(order[0]).toBe("fresh");
  });

  test("packing honors the budget and always keeps pinned items", () => {
    const result = packWithinBudget(
      [
        { ref: "pin", priority: 0, tokens: 100, pinned: true },
        { ref: "hi", priority: 10, tokens: 40 },
        { ref: "lo", priority: 1, tokens: 40 },
      ],
      160,
    );
    expect(result.included).toContain("pin");
    expect(result.included).toContain("hi");
    expect(result.dropped).toContain("lo");
    expect(result.usedTokens).toBe(140);
  });

  test("token estimate is a coarse ceiling", () => {
    expect(estimateTokens("abcd")).toBe(1);
    expect(estimateTokens("abcde")).toBe(2);
  });
});
