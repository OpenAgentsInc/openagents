import { describe, expect, test } from "bun:test"

import { filterSessions, sortByUpdatedAtDesc } from "./session-filter.js"

const rows = [
  {
    sessionRef: "local.alpha",
    latestActivity: "Building payment observer",
    state: "running",
    agentKind: "autopilot",
    origin: "local",
    updatedAt: "2026-06-13T10:00:00.000Z",
  },
  {
    sessionRef: "cloud.beta",
    latestActivity: "Reviewing forum report",
    state: "idle",
    agentKind: "external",
    origin: "cloud",
    updatedAt: "2026-06-13T12:00:00.000Z",
  },
  {
    sessionRef: "bridge.gamma",
    latestActivity: "Deploy smoke check",
    state: "running",
    agentKind: "autopilot",
    origin: "bridge",
    updatedAt: "2026-06-13T11:00:00.000Z",
  },
]

describe("session filtering", () => {
  test("matches text case-insensitively against sessionRef", () => {
    expect(filterSessions(rows, { text: "ALPHA" }).map((row) => row.sessionRef)).toEqual([
      "local.alpha",
    ])
  })

  test("matches text case-insensitively against latestActivity", () => {
    expect(filterSessions(rows, { text: "FORUM" }).map((row) => row.sessionRef)).toEqual([
      "cloud.beta",
    ])
  })

  test("matches exact state agentKind and origin filters together", () => {
    expect(filterSessions(rows, {
      state: "running",
      agentKind: "autopilot",
      origin: "bridge",
    }).map((row) => row.sessionRef)).toEqual([
      "bridge.gamma",
    ])
  })

  test("keeps missing and blank filters unconstrained", () => {
    expect(filterSessions(rows, { text: " ", state: "", agentKind: undefined } as unknown as Parameters<typeof filterSessions>[1]).map((row) => row.sessionRef)).toEqual([
      "local.alpha",
      "cloud.beta",
      "bridge.gamma",
    ])
  })

  test("uses exact case-sensitive equality for non-text filters", () => {
    expect(filterSessions(rows, { state: "Running" })).toEqual([])
  })

  test("defensively skips non-object rows and tolerates malformed fields", () => {
    expect(filterSessions([
      null,
      "bad",
      { sessionRef: "valid", latestActivity: 42, state: "running" },
      { sessionRef: 123, latestActivity: "valid activity", state: "running" },
    ], { text: "valid", state: "running" })).toEqual([
      { sessionRef: "valid", latestActivity: 42, state: "running" },
      { sessionRef: 123, latestActivity: "valid activity", state: "running" },
    ])
  })
})

describe("session sorting", () => {
  test("sorts by updatedAt descending without mutating the input", () => {
    const input = [rows[0], rows[1], rows[2]]

    expect(sortByUpdatedAtDesc(input).map((row) => row.sessionRef)).toEqual([
      "cloud.beta",
      "bridge.gamma",
      "local.alpha",
    ])
    expect(input.map((row) => row.sessionRef)).toEqual([
      "local.alpha",
      "cloud.beta",
      "bridge.gamma",
    ])
  })

  test("keeps malformed and missing updatedAt rows stable at the end", () => {
    const malformed = { sessionRef: "missing" }
    const invalid = "bad"

    expect(sortByUpdatedAtDesc([
      malformed,
      rows[0],
      invalid,
    ])).toEqual([
      rows[0],
      malformed,
      invalid,
    ])
  })
})
