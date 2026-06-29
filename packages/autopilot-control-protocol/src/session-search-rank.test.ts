import { describe, expect, test } from "bun:test"

import { rankSessionSearch, type SessionSearchRankRow } from "./session-search-rank.js"

const row = (
  sessionRef: string,
  latestActivity: string,
  state: string,
): SessionSearchRankRow => ({
  sessionRef,
  latestActivity,
  state,
})

describe("session search ranking", () => {
  test("returns no matches for an empty query", () => {
    expect(rankSessionSearch("",
      [row("local.alpha", "Building payment observer", "running")],
    )).toEqual([])
  })

  test("matches latestActivity case-insensitively", () => {
    expect(rankSessionSearch("PAYMENT", [
      row("local.alpha", "Building payment observer", "idle"),
      row("local.beta", "Reviewing forum report", "idle"),
    ])).toEqual([
      { sessionRef: "local.alpha", score: 1 },
    ])
  })

  test("matches sessionRef case-insensitively", () => {
    expect(rankSessionSearch("ALPHA", [
      row("local.alpha", "Building observer", "idle"),
      row("local.beta", "Building observer", "idle"),
    ])).toEqual([
      { sessionRef: "local.alpha", score: 1 },
    ])
  })

  test("scores by non-overlapping match count across latestActivity and sessionRef", () => {
    expect(rankSessionSearch("ship", [
      row("local.ship", "Ship plan and ship receipt", "idle"),
      row("local.once", "Ship plan", "idle"),
    ])).toEqual([
      { sessionRef: "local.ship", score: 3 },
      { sessionRef: "local.once", score: 1 },
    ])
  })

  test("adds a running-state boost to matching rows", () => {
    expect(rankSessionSearch("deploy", [
      row("idle.deploy", "Deploy smoke", "idle"),
      row("running.deploy", "Deploy smoke", "running"),
    ])).toEqual([
      { sessionRef: "running.deploy", score: 3 },
      { sessionRef: "idle.deploy", score: 2 },
    ])
  })

  test("does not return running rows that have no substring match", () => {
    expect(rankSessionSearch("promise", [
      row("local.alpha", "Building payment observer", "running"),
      row("local.beta", "Product promise report", "idle"),
    ])).toEqual([
      { sessionRef: "local.beta", score: 1 },
    ])
  })

  test("keeps equal-score rows in input order", () => {
    expect(rankSessionSearch("report", [
      row("first", "Forum report", "idle"),
      row("second", "Forum report", "idle"),
      row("third", "Forum report", "idle"),
    ])).toEqual([
      { sessionRef: "first", score: 1 },
      { sessionRef: "second", score: 1 },
      { sessionRef: "third", score: 1 },
    ])
  })

  test("does not mutate input rows", () => {
    const rows = [
      row("local.alpha", "Alpha report", "idle"),
      row("local.beta", "Beta report", "running"),
    ]
    const snapshot = rows.map((item) => ({ ...item }))

    rankSessionSearch("report", rows)

    expect(rows).toEqual(snapshot)
  })
})
