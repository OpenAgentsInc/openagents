import { describe, expect, test } from "bun:test"

import {
  projectKhalaCodeDesktopCodexThread,
  projectKhalaCodeDesktopCodexThreadList,
} from "../src/shared/codex-threads"

describe("Codex thread sidebar projection", () => {
  test("projects thread status, grouping, lineage, and badges", () => {
    const response = {
      data: [
        {
          id: "thread-a",
          sessionId: "session-a",
          name: "Named thread",
          preview: "First prompt",
          cwd: "/repo/app",
          modelProvider: "openai",
          source: "appServer",
          forkedFromId: null,
          parentThreadId: null,
          createdAt: 1,
          updatedAt: 2,
          recencyAt: 3,
          status: { type: "active", activeFlags: [] },
          gitInfo: { branch: "main", sha: "abc", originUrl: null },
          turns: [],
        },
        {
          id: "thread-b",
          sessionId: "session-b",
          name: null,
          preview: "Fork prompt\nsecond line",
          cwd: "/repo/app",
          modelProvider: "openai",
          source: { custom: "desktop" },
          forkedFromId: "thread-a",
          parentThreadId: "thread-parent",
          createdAt: 4,
          updatedAt: 5,
          recencyAt: null,
          status: { type: "notLoaded" },
          gitInfo: null,
          turns: [{ status: "failed" }],
        },
      ],
    }

    const projection = projectKhalaCodeDesktopCodexThreadList({
      activeThreadId: "thread-a",
      archived: false,
      response,
      searchTerm: "prompt",
    })

    expect(projection).toMatchObject({
      activeThreadId: "thread-a",
      archived: false,
      searchTerm: "prompt",
      groups: [{
        key: "/repo/app",
        label: "app",
        threadIds: ["thread-a", "thread-b"],
      }],
    })
    expect(projection.threads.map(thread => thread.title)).toEqual([
      "Named thread",
      "Fork prompt",
    ])
    expect(projection.threads[0]?.badges).toEqual(["git", "running"])
    expect(projection.threads[1]?.badges).toEqual(["child", "failed", "fork"])
  })

  test("rejects malformed thread rows", () => {
    expect(projectKhalaCodeDesktopCodexThread({ preview: "missing id" })).toBeNull()
    expect(projectKhalaCodeDesktopCodexThreadList({
      response: { data: [{ preview: "missing id" }] },
    }).threads).toEqual([])
  })

  test("hides internal missing-rollout diagnostics from visible thread text", () => {
    const projection = projectKhalaCodeDesktopCodexThreadList({
      response: {
        data: [
          {
            id: "thread-missing-rollout",
            name: null,
            preview: "thread/resume failed: no rollout found for thread id thread-missing-rollout",
            status: { type: "systemError" },
          },
          {
            id: "thread-missing-state",
            name: "thread not found",
            preview: "thread not found",
            status: { type: "notLoaded" },
          },
        ],
      },
    })

    expect(projection.threads.map(thread => ({
      title: thread.title,
      preview: thread.preview,
      badges: thread.badges,
    }))).toEqual([
      {
        title: "thread-missing-rollout",
        preview: "",
        badges: ["failed"],
      },
      {
        title: "thread-missing-state",
        preview: "",
        badges: [],
      },
    ])
    expect(JSON.stringify(projection)).not.toContain("no rollout found")
    expect(JSON.stringify(projection)).not.toContain("thread not found")
  })
})
