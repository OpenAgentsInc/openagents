import { describe, expect, test } from "bun:test"

import { cosineSimilarity, topK } from "../src/tas/semantic-retrieval"

describe("tas semantic retrieval core", () => {
  test("cosine of identical vectors is 1 and orthogonal vectors is 0", () => {
    expect(cosineSimilarity([1, 2, 3], [1, 2, 3])).toBeCloseTo(1)
    expect(cosineSimilarity([1, 0], [0, 1])).toBe(0)
  })

  test("guards length mismatch and zero vectors", () => {
    expect(() => cosineSimilarity([1, 2], [1])).toThrow(
      "Embedding length mismatch",
    )
    expect(cosineSimilarity([0, 0], [1, 1])).toBe(0)
    expect(cosineSimilarity([1, 1], [0, 0])).toBe(0)
  })

  test("topK ranks by descending cosine similarity", () => {
    const results = topK(
      [1, 0],
      [
        { ref: "orthogonal", embedding: [0, 1] },
        { ref: "same-direction", embedding: [2, 0] },
        { ref: "opposite", embedding: [-1, 0] },
      ],
      3,
    )

    expect(results.map((result) => result.ref)).toEqual([
      "same-direction",
      "orthogonal",
      "opposite",
    ])
  })

  test("topK applies k limit", () => {
    const results = topK(
      [1, 0],
      [
        { ref: "first", embedding: [1, 0] },
        { ref: "second", embedding: [0.5, 0] },
        { ref: "third", embedding: [0, 1] },
      ],
      2,
    )

    expect(results).toHaveLength(2)
    expect(results.map((result) => result.ref)).toEqual(["first", "second"])
  })

  test("topK breaks similarity ties by ref", () => {
    const results = topK(
      [1, 0],
      [
        { ref: "b", embedding: [1, 0] },
        { ref: "a", embedding: [2, 0] },
        { ref: "c", embedding: [3, 0] },
      ],
      3,
    )

    expect(results.map((result) => result.ref)).toEqual(["a", "b", "c"])
  })
})
