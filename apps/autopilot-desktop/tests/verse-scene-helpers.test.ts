import { describe, expect, test } from "bun:test"

import {
  appendVerseVisualization,
  compactVerseLines,
  finitePositiveVerseNumber,
  roundedVerseVector,
  uniqueVerseStrings,
} from "../src/shared/verse-scene-helpers"

describe("Verse scene helpers (#5917)", () => {
  test("dedupes and trims public refs and board lines", () => {
    expect(uniqueVerseStrings([" a ", "", null, "a", "b", undefined, " b "])).toEqual([
      "a",
      "b",
    ])
    expect(compactVerseLines([" Loading ", "Loading", "Ready"])).toEqual([
      "Loading",
      "Ready",
    ])
  })

  test("guards finite positive numbers and rounded vectors", () => {
    expect(finitePositiveVerseNumber(5)).toBe(5)
    expect(finitePositiveVerseNumber(-5)).toBe(0)
    expect(finitePositiveVerseNumber(Number.POSITIVE_INFINITY)).toBe(0)
    expect(roundedVerseVector([1.23456, -2.34567, Number.NaN])).toEqual([
      1.235,
      -2.346,
      0,
    ])
  })

  test("appends visualization descriptor arrays without replacing existing layers", () => {
    const out = appendVerseVisualization(
      {
        nodes: [
          {
            detail: "base",
            id: "base",
            label: "base",
            role: "run",
            status: "active",
          },
        ],
      },
      {
        nodes: [
          {
            detail: "extra",
            id: "extra",
            label: "extra",
            role: "lifecycle",
            status: "queued",
          },
        ],
        worldItems: [
          {
            detail: "board",
            id: "board",
            kind: "bulletin_board",
            label: "Board",
            position: [0, 0, 0],
          },
        ],
      },
    )

    expect(out.nodes?.map(node => node.id)).toEqual(["base", "extra"])
    expect(out.worldItems?.map(item => item.id)).toEqual(["board"])
  })
})
