import { describe, expect, test } from "bun:test"

import { mergeSessionViews, type MergeSessionViewsInput, type UnifiedSessionRow } from "./multi-origin-view.js"

const session = (
  sessionRef: string,
  updatedAt: string,
  overrides: Record<string, unknown> = {},
) => ({
  sessionRef,
  state: "running",
  latestActivity: `activity:${sessionRef}`,
  updatedAt,
  ...overrides,
})

describe("multi-origin session view", () => {
  test("merges sessions from every origin", () => {
    expect(mergeSessionViews({
      local: [session("local.1", "2026-06-13T10:00:00.000Z")],
      bridge: [session("bridge.1", "2026-06-13T09:00:00.000Z")],
      cloud: [session("cloud.1", "2026-06-13T08:00:00.000Z")],
      external: [session("external.1", "2026-06-13T07:00:00.000Z")],
    }).map((row) => row.sessionRef)).toEqual([
      "local.1",
      "bridge.1",
      "cloud.1",
      "external.1",
    ])
  })

  test("tags each row with its origin", () => {
    expect(mergeSessionViews({
      local: [session("local.1", "2026-06-13T10:00:00.000Z")],
      bridge: [session("bridge.1", "2026-06-13T09:00:00.000Z")],
      cloud: [session("cloud.1", "2026-06-13T08:00:00.000Z")],
      external: [session("external.1", "2026-06-13T07:00:00.000Z")],
    }).map(({ sessionRef, origin }) => ({ sessionRef, origin }))).toEqual([
      { sessionRef: "local.1", origin: "local" },
      { sessionRef: "bridge.1", origin: "bridge" },
      { sessionRef: "cloud.1", origin: "cloud" },
      { sessionRef: "external.1", origin: "external" },
    ])
  })

  test("deduplicates by session ref with first origin winning", () => {
    const merged = mergeSessionViews({
      local: [session("same", "2026-06-13T08:00:00.000Z", { state: "completed" })],
      bridge: [session("same", "2026-06-13T10:00:00.000Z", { state: "failed" })],
      cloud: [session("same", "2026-06-13T12:00:00.000Z", { state: "running" })],
      external: [session("same", "2026-06-13T14:00:00.000Z", { state: "queued" })],
    })

    expect(merged).toEqual([
      {
        sessionRef: "same",
        origin: "local",
        state: "completed",
        latestActivity: "activity:same",
        parentRef: null,
        updatedAt: "2026-06-13T08:00:00.000Z",
      },
    ] satisfies UnifiedSessionRow[])
  })

  test("sorts by updatedAt descending and preserves origin order for timestamp ties", () => {
    expect(mergeSessionViews({
      local: [
        session("older", "2026-06-13T08:00:00.000Z"),
        session("tie.local", "2026-06-13T09:00:00.000Z"),
      ],
      bridge: [session("newer", "2026-06-13T10:00:00.000Z")],
      cloud: [session("tie.cloud", "2026-06-13T09:00:00.000Z")],
    }).map((row) => row.sessionRef)).toEqual([
      "newer",
      "tie.local",
      "tie.cloud",
      "older",
    ])
  })

  test("orders parents before their children", () => {
    expect(mergeSessionViews({
      local: [
        session("child", "2026-06-13T12:00:00.000Z", { parentRef: "parent" }),
        session("parent", "2026-06-13T10:00:00.000Z"),
      ],
    }).map((row) => row.sessionRef)).toEqual([
      "parent",
      "child",
    ])
  })

  test("returns an empty list for empty or missing origin arrays", () => {
    expect(mergeSessionViews({})).toEqual([])
    expect(mergeSessionViews({
      local: [],
      bridge: undefined,
      cloud: [],
      external: undefined,
    } as unknown as MergeSessionViewsInput)).toEqual([])
  })

  test("defensively skips non-object rows and rows without session refs", () => {
    expect(mergeSessionViews({
      local: [
        null,
        "bad",
        { state: "running", updatedAt: "2026-06-13T10:00:00.000Z" },
        session("valid", "2026-06-13T09:00:00.000Z", {
          latestActivity: 123,
          parentRef: 456,
        }),
      ],
    })).toEqual([
      {
        sessionRef: "valid",
        origin: "local",
        state: "running",
        latestActivity: "",
        parentRef: null,
        updatedAt: "2026-06-13T09:00:00.000Z",
      },
    ] satisfies UnifiedSessionRow[])
  })
})
