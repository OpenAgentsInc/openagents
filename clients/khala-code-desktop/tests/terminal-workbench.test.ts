import { describe, expect, test } from "bun:test"

import { projectKhalaCodeTerminalWorkbench } from "../src/shared/terminal-workbench"

describe("Khala Code terminal workbench projection", () => {
  test("projects Codex background terminals into bounded tabs", () => {
    const projection = projectKhalaCodeTerminalWorkbench({
      activeThreadId: "thread-1",
      response: {
        terminals: [
          {
            command: "bun test",
            cwd: "/work/openagents",
            outputPreview: "pass",
            processId: "proc-1",
            status: "running",
          },
          {
            cmd: "git status",
            id: "proc-2",
            output: "clean",
            status: "closed",
          },
        ],
      },
    })

    expect(projection.boundary).toBe("active_thread")
    expect(projection.transport).toBe("codex_background_terminal")
    expect(projection.activeProcessId).toBe("proc-1")
    expect(projection.tabs).toEqual([
      expect.objectContaining({
        command: "bun test",
        cwd: "/work/openagents",
        outputPreview: "pass",
        processId: "proc-1",
        status: "running",
      }),
      expect.objectContaining({
        command: "git status",
        outputPreview: "clean",
        processId: "proc-2",
        status: "exited",
      }),
    ])
  })

  test("keeps no-session state explicit and does not leak stale tabs", () => {
    const projection = projectKhalaCodeTerminalWorkbench({
      activeThreadId: null,
      response: { terminals: [{ processId: "stale", output: "should stay scoped to its own request" }] },
    })

    expect(projection.boundary).toBe("no_active_thread")
    expect(projection.activeThreadId).toBeNull()
    expect(projection.tabs).toHaveLength(0)
  })
})
