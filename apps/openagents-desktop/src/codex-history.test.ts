import { describe, expect, test } from "vite-plus/test"

import { projectRow } from "./codex-history.ts"

/**
 * T9 #8866: rollout `approval` rows (any `event_msg` whose payload `type`
 * contains "approval") must project the recorded decision into a typed
 * `WorkbenchApprovalItem` sidecar so the read-only history card
 * (`packages/ui/src/workbench/dispatch.tsx` `case "approval"`) can render
 * the same shared `DesktopApprovalCard` component instead of a generic
 * work-entry shell. Before this change `projectRow` only computed a typed
 * `.item` sidecar for `tool_call`/`tool_result` kinds.
 */
describe("projectRow — approval kind typed sidecar (T9 #8866)", () => {
  test("an accepted tool approval row carries decision=approved and the reason as detail", () => {
    const row = {
      type: "event_msg",
      timestamp: "2026-07-16T00:00:00.000Z",
      payload: { type: "approval_decision", decision: "accept", reason: "run the test suite" },
    }
    const item = projectRow(row, "thread-1", 0)
    expect(item.kind).toBe("approval")
    expect(item.item).toMatchObject({ kind: "approval", status: "completed", decision: "approved", detail: "run the test suite" })
  })

  test("a declined command approval row carries decision=denied", () => {
    const row = {
      type: "event_msg",
      timestamp: "2026-07-16T00:00:01.000Z",
      payload: { type: "command_approval", decision: "decline", message: "risky command" },
    }
    const item = projectRow(row, "thread-1", 1)
    expect(item.kind).toBe("approval")
    expect(item.item).toMatchObject({ kind: "approval", status: "declined", decision: "denied", detail: "risky command" })
  })

  test("an unrecognized decision string keeps the row classified but omits a guessed decision", () => {
    const row = {
      type: "event_msg",
      timestamp: "2026-07-16T00:00:02.000Z",
      // Deliberately not "plan_approval": "plan" would classify earlier in
      // `projectRow`'s if/else chain (kind "plan") before the "approval"
      // check is ever reached.
      payload: { type: "network_approval", decision: "unclear" },
    }
    const item = projectRow(row, "thread-1", 2)
    expect(item.kind).toBe("approval")
    expect(item.item?.kind).toBe("approval")
    expect((item.item as { decision?: string } | undefined)?.decision).toBeUndefined()
  })

  test("non-approval tool_call rows keep their existing typed sidecar (no regression)", () => {
    const row = {
      type: "event_msg",
      timestamp: "2026-07-16T00:00:03.000Z",
      payload: { type: "commandExecution", command: "echo fixture", status: "completed" },
    }
    const item = projectRow(row, "thread-1", 3)
    expect(item.kind).toBe("tool_call")
    expect(item.item).toMatchObject({ kind: "command", command: "echo fixture" })
  })
})
