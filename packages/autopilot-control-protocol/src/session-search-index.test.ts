import { describe, expect, test } from "bun:test"

import { searchSessionEvents } from "./session-search-index.js"

describe("session event search index", () => {
  test("returns no matches for an empty query", () => {
    expect(searchSessionEvents([
      { phase: "agent_message", messageText: "deploy complete" },
    ], "")).toEqual([])
  })

  test("matches messageText case-insensitively and preserves event indexes", () => {
    expect(searchSessionEvents([
      { phase: "agent_message", messageText: "checking forum intake" },
      { phase: "agent_message", messageText: "Deploy smoke passed" },
    ], "deploy")).toEqual([
      { index: 1, snippet: "Deploy smoke passed" },
    ])
  })

  test("matches messageFull when messageText does not contain the query", () => {
    expect(searchSessionEvents([
      {
        phase: "tool_result",
        messageText: "short summary",
        messageFull: "first line\nsecond line has Product Promise evidence\nthird line",
      },
    ], "promise")).toEqual([
      { index: 0, snippet: "second line has Product Promise evidence" },
    ])
  })

  test("searches text fields without matching phase names", () => {
    expect(searchSessionEvents([
      { phase: "deployment_verification", messageText: "ordinary status" },
      { phase: "agent_message", messageFull: "verification copied to the transcript" },
    ], "verification")).toEqual([
      { index: 1, snippet: "verification copied to the transcript" },
    ])
  })

  test("returns one snippet per matching event in source order", () => {
    expect(searchSessionEvents([
      { phase: "agent_message", messageText: "alpha payment trace" },
      { phase: "agent_message", messageText: "no match here" },
      { phase: "tool_result", messageFull: "PAYMENT receipt observed" },
    ], "payment")).toEqual([
      { index: 0, snippet: "alpha payment trace" },
      { index: 2, snippet: "PAYMENT receipt observed" },
    ])
  })

  test("crops long matched lines to a 120 character snippet around the match", () => {
    const prefix = "a".repeat(90)
    const suffix = "z".repeat(90)
    const results = searchSessionEvents([
      { phase: "agent_message", messageText: `${prefix}needle${suffix}` },
    ], "needle")

    expect(results).toHaveLength(1)
    expect(results[0].snippet).toContain("needle")
    expect(results[0].snippet.length).toBeLessThanOrEqual(120)
  })
})
