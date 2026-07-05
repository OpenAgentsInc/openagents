import { describe, expect, test } from "bun:test"

import {
  dispatchKhalaCodeDesktopRecoveryAction,
  khalaCodeDesktopRecoveryActionsFor,
  KHALA_CODE_DESKTOP_RECOVERY_ACTION_LABELS,
  khalaCodeDesktopLoadFailureState,
  khalaCodeDesktopUnresponsiveState,
  type KhalaCodeDesktopRecoveryActionDispatch,
} from "../src/shared/recovery-state"

describe("khalaCodeDesktopRecoveryActionsFor", () => {
  test("unresponsive offers relaunch, export, keep waiting, and quit in order", () => {
    expect(khalaCodeDesktopRecoveryActionsFor("unresponsive")).toEqual([
      "relaunch",
      "export_logs",
      "keep_waiting",
      "quit",
    ])
  })

  test("load_failure omits keep_waiting (nothing to wait on)", () => {
    expect(khalaCodeDesktopRecoveryActionsFor("load_failure")).toEqual([
      "relaunch",
      "export_logs",
      "quit",
    ])
  })

  test("every action has a human-readable label", () => {
    for (const kind of ["unresponsive", "load_failure"] as const) {
      for (const action of khalaCodeDesktopRecoveryActionsFor(kind)) {
        expect(KHALA_CODE_DESKTOP_RECOVERY_ACTION_LABELS[action].length).toBeGreaterThan(0)
      }
    }
  })
})

describe("state constructors", () => {
  test("khalaCodeDesktopLoadFailureState builds a load_failure state", () => {
    const state = khalaCodeDesktopLoadFailureState("failed to load bundled view", "2026-07-05T00:00:00.000Z")
    expect(state).toEqual({
      detail: "failed to load bundled view",
      kind: "load_failure",
      since: "2026-07-05T00:00:00.000Z",
    })
  })

  test("khalaCodeDesktopUnresponsiveState builds an unresponsive state", () => {
    const state = khalaCodeDesktopUnresponsiveState("no heartbeat for 12s", "2026-07-05T00:00:00.000Z")
    expect(state.kind).toBe("unresponsive")
  })
})

const recordingDispatch = (): {
  readonly calls: string[]
  readonly dispatch: KhalaCodeDesktopRecoveryActionDispatch
} => {
  const calls: string[] = []
  return {
    calls,
    dispatch: {
      exportDebugLogs: async () => {
        calls.push("exportDebugLogs")
        return { path: "/tmp/khala-code-debug-logs.zip" }
      },
      quit: async () => {
        calls.push("quit")
      },
      relaunch: async () => {
        calls.push("relaunch")
      },
    },
  }
}

describe("dispatchKhalaCodeDesktopRecoveryAction", () => {
  test("keep_waiting is a pure no-op and calls nothing", async () => {
    const { calls, dispatch } = recordingDispatch()
    const outcome = await dispatchKhalaCodeDesktopRecoveryAction("keep_waiting", dispatch)
    expect(outcome).toEqual({ kind: "noop" })
    expect(calls).toEqual([])
  })

  test("export_logs calls exportDebugLogs and returns the produced path", async () => {
    const { calls, dispatch } = recordingDispatch()
    const outcome = await dispatchKhalaCodeDesktopRecoveryAction("export_logs", dispatch)
    expect(outcome).toEqual({ kind: "export", path: "/tmp/khala-code-debug-logs.zip" })
    expect(calls).toEqual(["exportDebugLogs"])
  })

  test("relaunch calls relaunch and dismisses", async () => {
    const { calls, dispatch } = recordingDispatch()
    const outcome = await dispatchKhalaCodeDesktopRecoveryAction("relaunch", dispatch)
    expect(outcome).toEqual({ kind: "dismiss" })
    expect(calls).toEqual(["relaunch"])
  })

  test("quit calls quit and dismisses", async () => {
    const { calls, dispatch } = recordingDispatch()
    const outcome = await dispatchKhalaCodeDesktopRecoveryAction("quit", dispatch)
    expect(outcome).toEqual({ kind: "dismiss" })
    expect(calls).toEqual(["quit"])
  })
})
