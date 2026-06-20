import { describe, expect, test } from "bun:test"

import { buildSessionListView } from "./session-filter-view.js"
import type { SessionSortRow } from "./session-sort.js"

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

describe("session filter view", () => {
  test("returns all rows sorted with running roots first and recent roots first", () => {
    const view = buildSessionListView({
      filter: "all",
      rows: [
        session("completed-newer", "completed", "2026-06-13T12:00:00.000Z"),
        session("idle-newest", "idle", "2026-06-13T14:00:00.000Z"),
        session("running-older", "running", "2026-06-13T09:00:00.000Z"),
      ],
    })

    expect(view.rows.map((row) => row.sessionRef)).toEqual([
      "running-older",
      "idle-newest",
      "completed-newer",
    ])
  })

  test("filters running rows and keeps counts for the unfiltered input", () => {
    const view = buildSessionListView({
      filter: "running",
      rows: [
        session("completed", "completed", "2026-06-13T12:00:00.000Z"),
        session("running-old", "running", "2026-06-13T09:00:00.000Z"),
        session("running-new", "running", "2026-06-13T11:00:00.000Z"),
      ],
    })

    expect(view.rows.map((row) => row.sessionRef)).toEqual([
      "running-new",
      "running-old",
    ])
    expect(view.counts).toEqual({
      all: 3,
      running: 2,
      completed: 1,
    })
  })

  test("filters completed rows and sorts them by most recent first", () => {
    const view = buildSessionListView({
      filter: "completed",
      rows: [
        session("completed-old", "completed", "2026-06-13T09:00:00.000Z"),
        session("running", "running", "2026-06-13T12:00:00.000Z"),
        session("completed-new", "completed", "2026-06-13T11:00:00.000Z"),
      ],
    })

    expect(view.rows.map((row) => row.sessionRef)).toEqual([
      "completed-new",
      "completed-old",
    ])
  })

  test("groups children under their parent after sorting", () => {
    const view = buildSessionListView({
      filter: "all",
      rows: [
        session("child-newer-than-parent", "running", "2026-06-13T13:00:00.000Z", "parent"),
        session("newer-root", "running", "2026-06-13T12:00:00.000Z"),
        session("parent", "running", "2026-06-13T10:00:00.000Z"),
      ],
    })

    expect(view.rows.map((row) => row.sessionRef)).toEqual([
      "newer-root",
      "parent",
      "child-newer-than-parent",
    ])
  })

  test("keeps nested matching descendants grouped recursively", () => {
    const view = buildSessionListView({
      filter: "running",
      rows: [
        session("grandchild", "running", "2026-06-13T15:00:00.000Z", "child"),
        session("child", "running", "2026-06-13T14:00:00.000Z", "parent"),
        session("parent", "running", "2026-06-13T13:00:00.000Z"),
        session("completed", "completed", "2026-06-13T16:00:00.000Z"),
      ],
    })

    expect(view.rows.map((row) => row.sessionRef)).toEqual([
      "parent",
      "child",
      "grandchild",
    ])
  })

  test("treats children with filtered out parents as sortable roots", () => {
    const view = buildSessionListView({
      filter: "running",
      rows: [
        session("parent", "completed", "2026-06-13T12:00:00.000Z"),
        session("child", "running", "2026-06-13T11:00:00.000Z", "parent"),
        session("newer-running-root", "running", "2026-06-13T13:00:00.000Z"),
      ],
    })

    expect(view.rows.map((row) => row.sessionRef)).toEqual([
      "newer-running-root",
      "child",
    ])
  })

  test("does not mutate input and returns original row objects", () => {
    const older = session("older", "completed", "2026-06-13T09:00:00.000Z")
    const newer = session("newer", "completed", "2026-06-13T10:00:00.000Z")
    const rows = [older, newer]

    const view = buildSessionListView({ filter: "completed", rows })

    expect(view.rows).toEqual([newer, older])
    expect(view.rows[0]).toBe(newer)
    expect(view.rows[1]).toBe(older)
    expect(rows).toEqual([older, newer])
  })
})
