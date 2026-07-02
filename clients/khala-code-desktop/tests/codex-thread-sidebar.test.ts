import { describe, expect, test } from "bun:test"

import type { KhalaCodeDesktopCodexThreadSummary } from "../src/shared/codex-threads"
import type { KhalaCodeDesktopCodexThreadListResult } from "../src/shared/rpc"
import {
  renameThreadInListData,
  upsertPendingThreadInListData,
} from "../src/ui/codex-thread-sidebar"

const thread = (
  id: string,
  title: string,
): KhalaCodeDesktopCodexThreadSummary => ({
  id,
  sessionId: id,
  title,
  preview: `${title} preview`,
  cwd: "/repo/app",
  projectLabel: "app",
  status: "idle",
  statusLabel: "idle",
  modelProvider: "openai",
  source: "appServer",
  forkedFromId: null,
  parentThreadId: null,
  createdAt: 1,
  updatedAt: 2,
  recencyAt: 3,
  badges: [],
})

describe("Khala Code thread sidebar", () => {
  test("renames the visible thread title in list data immediately", () => {
    const data: KhalaCodeDesktopCodexThreadListResult = {
      ok: true,
      data: [],
      groups: [{ key: "/repo/app", label: "app", threadIds: ["thread-a", "thread-b"] }],
      threads: [
        thread("thread-a", "Old name"),
        thread("thread-b", "Other thread"),
      ],
    }

    const renamed = renameThreadInListData(data, "thread-a", "New name")

    expect(renamed).not.toBe(data)
    expect(renamed.data).toBe(data.data)
    expect(renamed.groups).toBe(data.groups)
    expect(renamed.threads?.map(item => item.title)).toEqual([
      "New name",
      "Other thread",
    ])
    expect(data.threads?.[0]?.title).toBe("Old name")
  })

  test("keeps list data stable when the thread title is already current", () => {
    const data: KhalaCodeDesktopCodexThreadListResult = {
      ok: true,
      data: [],
      threads: [thread("thread-a", "Current name")],
    }

    expect(renameThreadInListData(data, "thread-a", "Current name")).toBe(data)
    expect(renameThreadInListData(data, "missing-thread", "New name")).toBe(data)
  })

  test("prepends a pending active thread until persisted thread metadata catches up", () => {
    const data: KhalaCodeDesktopCodexThreadListResult = {
      ok: true,
      data: [],
      groups: [{ key: "/repo/app", label: "app", threadIds: ["thread-a"] }],
      threads: [thread("thread-a", "Existing")],
    }
    const pending = {
      ...thread("thread-new", "hi"),
      cwd: null,
      projectLabel: "Current chat",
      recencyAt: 10,
    }

    const next = upsertPendingThreadInListData(data, pending)

    expect(next).not.toBe(data)
    expect(next.threads?.map(item => item.id)).toEqual(["thread-new", "thread-a"])
    expect(next.groups?.[0]).toEqual({
      key: "cwd:none",
      label: "Current chat",
      threadIds: ["thread-new"],
    })
    expect(upsertPendingThreadInListData(next, pending)).toBe(next)
  })
})
