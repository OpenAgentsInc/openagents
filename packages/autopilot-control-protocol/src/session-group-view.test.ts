import { describe, expect, test } from "bun:test"

import { groupSessionsByParent, type SessionGroupRow } from "./session-group-view.js"

const session = (
  sessionRef: string,
  state: string,
  updatedAt: string,
  parentRef?: string | null,
): SessionGroupRow => ({
  sessionRef,
  state,
  updatedAt,
  ...(parentRef === undefined ? {} : { parentRef }),
})

const refs = (nodes: { parent: SessionGroupRow, children: any[] }[]): unknown[] =>
  nodes.map((node) => [
    node.parent.sessionRef,
    refs(node.children),
  ])

describe("session group view", () => {
  test("returns an empty tree and zero counts for empty rows", () => {
    expect(groupSessionsByParent([])).toEqual({
      tree: [],
      topCount: 0,
      totalCount: 0,
    })
  })

  test("sorts top-level groups with running sessions first then newest updatedAt", () => {
    const view = groupSessionsByParent([
      session("completed-newer", "completed", "2026-06-13T12:00:00.000Z"),
      session("running-older", "running", "2026-06-13T09:00:00.000Z"),
      session("idle-newest", "idle", "2026-06-13T14:00:00.000Z"),
    ])

    expect(view.tree.map((node) => node.parent.sessionRef)).toEqual([
      "running-older",
      "idle-newest",
      "completed-newer",
    ])
    expect(view.topCount).toBe(3)
    expect(view.totalCount).toBe(3)
  })

  test("groups direct children under a sorted parent", () => {
    const view = groupSessionsByParent([
      session("child", "running", "2026-06-13T13:00:00.000Z", "parent"),
      session("newer-root", "running", "2026-06-13T12:00:00.000Z"),
      session("parent", "running", "2026-06-13T10:00:00.000Z"),
    ])

    expect(refs(view.tree)).toEqual([
      ["newer-root", []],
      ["parent", [
        ["child", []],
      ]],
    ])
    expect(view.topCount).toBe(2)
    expect(view.totalCount).toBe(3)
  })

  test("keeps nested descendants recursively attached", () => {
    const view = groupSessionsByParent([
      session("grandchild", "running", "2026-06-13T15:00:00.000Z", "child"),
      session("newer-root", "running", "2026-06-13T14:00:00.000Z"),
      session("child", "running", "2026-06-13T13:00:00.000Z", "parent"),
      session("parent", "running", "2026-06-13T12:00:00.000Z"),
    ])

    expect(refs(view.tree)).toEqual([
      ["newer-root", []],
      ["parent", [
        ["child", [
          ["grandchild", []],
        ]],
      ]],
    ])
    expect(view.topCount).toBe(2)
    expect(view.totalCount).toBe(4)
  })

  test("keeps sibling child order aligned with session sorting", () => {
    const view = groupSessionsByParent([
      session("second-child", "running", "2026-06-13T12:00:00.000Z", "parent"),
      session("parent", "running", "2026-06-13T10:00:00.000Z"),
      session("first-child", "completed", "2026-06-13T13:00:00.000Z", "parent"),
    ])

    expect(refs(view.tree)).toEqual([
      ["parent", [
        ["second-child", []],
        ["first-child", []],
      ]],
    ])
    expect(view.topCount).toBe(1)
    expect(view.totalCount).toBe(3)
  })

  test("treats missing parent refs and self parent refs as top-level groups", () => {
    const view = groupSessionsByParent([
      session("missing-parent", "idle", "2026-06-13T11:00:00.000Z", "absent"),
      session("self-parent", "running", "2026-06-13T09:00:00.000Z", "self-parent"),
      session("plain-root", "idle", "2026-06-13T10:00:00.000Z"),
    ])

    expect(refs(view.tree)).toEqual([
      ["self-parent", []],
      ["missing-parent", []],
      ["plain-root", []],
    ])
    expect(view.topCount).toBe(3)
    expect(view.totalCount).toBe(3)
  })

  test("does not mutate input rows and returns the original row objects", () => {
    const child = session("child", "idle", "2026-06-13T10:00:00.000Z", "parent")
    const parent = session("parent", "running", "2026-06-13T09:00:00.000Z")
    const input = [child, parent]

    const view = groupSessionsByParent(input)

    expect(input).toEqual([child, parent])
    expect(view.tree[0]?.parent).toBe(parent)
    expect(view.tree[0]?.children[0]?.parent).toBe(child)
  })
})
