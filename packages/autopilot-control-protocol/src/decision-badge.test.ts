import { describe, expect, test } from "bun:test"

import { decisionBadge } from "./decision-badge.js"

describe("decision badge", () => {
  test("hides when there are no pending decisions", () => {
    expect(decisionBadge(0)).toEqual({
      show: false,
      label: "",
      tone: "none",
    })
  })

  test("hides when the pending count is negative", () => {
    expect(decisionBadge(-1)).toEqual({
      show: false,
      label: "",
      tone: "none",
    })
  })

  test("shows an attention badge for one pending decision", () => {
    expect(decisionBadge(1)).toEqual({
      show: true,
      label: "1 need you",
      tone: "attention",
    })
  })

  test("labels multiple pending decisions with the count", () => {
    expect(decisionBadge(2)).toEqual({
      show: true,
      label: "2 need you",
      tone: "attention",
    })
  })

  test("preserves larger pending counts in the label", () => {
    expect(decisionBadge(42)).toEqual({
      show: true,
      label: "42 need you",
      tone: "attention",
    })
  })
})
