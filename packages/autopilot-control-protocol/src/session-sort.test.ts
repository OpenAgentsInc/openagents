import { describe, expect, test } from "bun:test"

import { sortSessions, type SessionSortRow } from "./session-sort.js"

const session = (
  sessionRef: string,
  state: string,
  updatedAt: string,
  parentRef?: string | null,
): SessionSortRow => ({
  sessionRef,
  state,
  updatedAt,
  ...(parentRef === undefined ? {} : { parentRef }),
})

describe("session sort", () => {
  test("orders running sessions before non-running sessions", () => {
    expect(sortSessions([
      session("completed-newer", "completed", "2026-06-13T12:00:00.000Z"),
      session("running-older", "running", "2026-06-13T09:00:00.000Z"),
      session("idle-newest", "idle", "2026-06-13T14:00:00.000Z"),
    ]).map((row) => row.sessionRef)).toEqual([
      "running-older",
      "idle-newest",
      "completed-newer",
    ])
  })

  test("orders sessions in the same state class by updatedAt descending", () => {
    expect(sortSessions([
      session("running-old", "running", "2026-06-13T08:00:00.000Z"),
      session("running-new", "running", "2026-06-13T10:00:00.000Z"),
      session("running-mid", "running", "2026-06-13T09:00:00.000Z"),
      session("idle-old", "idle", "2026-06-13T06:00:00.000Z"),
      session("idle-new", "idle", "2026-06-13T07:00:00.000Z"),
    ]).map((row) => row.sessionRef)).toEqual([
      "running-new",
      "running-mid",
      "running-old",
      "idle-new",
      "idle-old",
    ])
  })

  test("keeps timestamp ties stable", () => {
    expect(sortSessions([
      session("first", "running", "2026-06-13T08:00:00.000Z"),
      session("second", "running", "2026-06-13T08:00:00.000Z"),
      session("third", "running", "2026-06-13T08:00:00.000Z"),
    ]).map((row) => row.sessionRef)).toEqual([
      "first",
      "second",
      "third",
    ])
  })

  test("groups children directly under their parent even when children are newer", () => {
    expect(sortSessions([
      session("child", "running", "2026-06-13T13:00:00.000Z", "parent"),
      session("newer-root", "running", "2026-06-13T12:00:00.000Z"),
      session("parent", "running", "2026-06-13T10:00:00.000Z"),
    ]).map((row) => row.sessionRef)).toEqual([
      "newer-root",
      "parent",
      "child",
    ])
  })

  test("keeps sibling children stable under their parent", () => {
    expect(sortSessions([
      session("second-child", "running", "2026-06-13T12:00:00.000Z", "parent"),
      session("parent", "running", "2026-06-13T10:00:00.000Z"),
      session("first-child", "completed", "2026-06-13T13:00:00.000Z", "parent"),
    ]).map((row) => row.sessionRef)).toEqual([
      "parent",
      "second-child",
      "first-child",
    ])
  })

  test("keeps nested descendants grouped recursively", () => {
    expect(sortSessions([
      session("grandchild", "running", "2026-06-13T15:00:00.000Z", "child"),
      session("newer-root", "running", "2026-06-13T14:00:00.000Z"),
      session("child", "running", "2026-06-13T13:00:00.000Z", "parent"),
      session("parent", "running", "2026-06-13T12:00:00.000Z"),
    ]).map((row) => row.sessionRef)).toEqual([
      "newer-root",
      "parent",
      "child",
      "grandchild",
    ])
  })

  test("treats missing parents and self-parent refs as sortable roots", () => {
    expect(sortSessions([
      session("missing-parent", "idle", "2026-06-13T11:00:00.000Z", "absent"),
      session("self-parent", "running", "2026-06-13T09:00:00.000Z", "self-parent"),
      session("plain-root", "idle", "2026-06-13T10:00:00.000Z"),
    ]).map((row) => row.sessionRef)).toEqual([
      "self-parent",
      "missing-parent",
      "plain-root",
    ])
  })

  test("does not mutate the input and returns the original row objects", () => {
    const older = session("older", "idle", "2026-06-13T09:00:00.000Z")
    const newer = session("newer", "idle", "2026-06-13T10:00:00.000Z")
    const input = [older, newer]

    const sorted = sortSessions(input)

    expect(sorted).toEqual([newer, older])
    expect(sorted[0]).toBe(newer)
    expect(sorted[1]).toBe(older)
    expect(input).toEqual([older, newer])
  })
})
