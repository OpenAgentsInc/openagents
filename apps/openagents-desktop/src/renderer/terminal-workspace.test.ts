/**
 * Terminal workspace unit tests (CUT-20, #8700): pure state -> component tree
 * across empty/running/recovered phases, the streamed-event transitions
 * (ready/output/exit/preview/closed/error), and the typed intent loop run
 * headlessly through the real registry with a fake bridge (no Electron).
 */
import { describe, expect, test } from "vite-plus/test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  emptyTerminalWorkspaceState,
  makeTerminalWorkspaceHandlers,
  terminalWorkspaceIntents,
  terminalWorkspaceView,
  withTerminalEvent,
  withTerminalEvents,
  withTerminalSnapshot,
  type TerminalRendererBridge,
  type TerminalWorkspaceState,
} from "./terminal-workspace.ts"
import type { TerminalEvent } from "../terminal-contract.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>

const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) walk(item)
      return
    }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find((node) => node.key === key)

const ready = (sessionRef: string): TerminalEvent => ({
  kind: "ready",
  sessionRef,
  cwdLabel: "repo",
  shellLabel: "sh",
  cols: 80,
  rows: 24,
})

const runningState = (sessionRef = "terminal.aaa111bbb"): TerminalWorkspaceState =>
  withTerminalEvent(emptyTerminalWorkspaceState(), ready(sessionRef))

// ---------------------------------------------------------------------------
// Pure transitions.
// ---------------------------------------------------------------------------

describe("terminal transitions", () => {
  test("ready adds a running session and makes it active", () => {
    const state = runningState()
    expect(state.phase).toBe("ready")
    expect(state.activeRef).toBe("terminal.aaa111bbb")
    expect(state.sessions).toHaveLength(1)
    expect(state.sessions[0]!.status).toBe("running")
  })

  test("output appends to the active session's bounded tail", () => {
    const state = withTerminalEvent(runningState(), {
      kind: "output",
      sessionRef: "terminal.aaa111bbb",
      chunk: "hello world\n",
    })
    expect(state.sessions[0]!.output).toBe("hello world\n")
  })

  test("10,000 output events fold exactly in one bounded transition", () => {
    const events: TerminalEvent[] = Array.from({ length: 10_000 }, () => ({
      kind: "output", sessionRef: "terminal.aaa111bbb", chunk: "0123456789",
    }))
    const state = withTerminalEvents(runningState(), events)
    expect(state.sessions[0]!.output).toHaveLength(100_000)
    expect(state.sessions[0]!.output).toBe("0123456789".repeat(10_000))
  })

  test("exit marks the session exited with its code; input controls disable", () => {
    const state = withTerminalEvent(runningState(), {
      kind: "exit",
      sessionRef: "terminal.aaa111bbb",
      exitCode: 2,
      signal: null,
    })
    expect(state.sessions[0]!.status).toBe("exited")
    expect(state.sessions[0]!.exitCode).toBe(2)
    const view = terminalWorkspaceView(state)
    expect(nodeByKey(view, "terminal-send")?.disabled).toBe(true)
    expect(nodeByKey(view, "terminal-interrupt")?.disabled).toBe(true)
  })

  test("preview records the announced port once and renders an open control", () => {
    let state = withTerminalEvent(runningState(), {
      kind: "preview",
      sessionRef: "terminal.aaa111bbb",
      port: 5173,
      url: "http://localhost:5173/",
      ready: true,
    })
    // A duplicate announcement does not double-add.
    state = withTerminalEvent(state, {
      kind: "preview",
      sessionRef: "terminal.aaa111bbb",
      port: 5173,
      url: "http://localhost:5173/",
      ready: true,
    })
    expect(state.sessions[0]!.previews).toHaveLength(1)
    const view = terminalWorkspaceView(state)
    expect(nodeByKey(view, "terminal-preview-open-5173")?._tag).toBe("Button")
  })

  test("closed removes the session and reselects the remaining active ref", () => {
    let state = runningState("terminal.aaa111bbb")
    state = withTerminalEvent(state, ready("terminal.ccc333ddd"))
    state = withTerminalEvent(state, { kind: "closed", sessionRef: "terminal.ccc333ddd", reason: "user" })
    expect(state.sessions.map((session) => session.sessionRef)).toEqual(["terminal.aaa111bbb"])
    expect(state.activeRef).toBe("terminal.aaa111bbb")
  })

  test("error surfaces a bounded notice", () => {
    const state = withTerminalEvent(runningState(), {
      kind: "error",
      sessionRef: "terminal.aaa111bbb",
      message: "preview port 5173 is already owned by another terminal session",
    })
    expect(state.notice).toContain("already owned")
    expect(nodeByKey(terminalWorkspaceView(state), "terminal-notice")?._tag).toBe("Text")
  })

  test("snapshot hydration marks a recovered, gap session and shows the recovery note", () => {
    const state = withTerminalSnapshot(emptyTerminalWorkspaceState(), {
      sessions: [{
        sessionRef: "terminal.recover01",
        cwdLabel: "repo",
        shellLabel: "sh",
        status: "recovered",
        exitCode: null,
        recovered: true,
        gap: true,
        tail: "previous output line\n",
        previews: [],
      }],
    })
    expect(state.sessions[0]!.recovered).toBe(true)
    expect(state.sessions[0]!.output).toBe("previous output line\n")
    expect(nodeByKey(terminalWorkspaceView(state), "terminal-recovered-note")?._tag).toBe("Text")
  })
})

describe("terminalWorkspaceView (state -> component tree)", () => {
  test("empty state shows the honest empty copy and a New terminal control", () => {
    const view = terminalWorkspaceView(emptyTerminalWorkspaceState())
    expect(nodeByKey(view, "terminal-title")?.content).toBe("Terminal")
    expect(nodeByKey(view, "terminal-empty")?._tag).toBe("EmptyMessage")
    expect(nodeByKey(view, "terminal-new")?._tag).toBe("Button")
    expect(nodeByKey(view, "terminal-output-code")).toBeUndefined()
  })

  test("running state renders the output well, an enabled input line, and Send", () => {
    const view = terminalWorkspaceView(runningState())
    expect(nodeByKey(view, "terminal-output-code")?._tag).toBe("CodeBlock")
    expect(nodeByKey(view, "terminal-input-field")?.disabled).toBe(false)
    expect(nodeByKey(view, "terminal-send")?.disabled).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Typed intent loop through the real registry with a fake bridge.
// ---------------------------------------------------------------------------

describe("typed terminal intent loop (registry -> bridge -> state)", () => {
  const makeHarness = (
    bridge: TerminalRendererBridge,
    initial: TerminalWorkspaceState = emptyTerminalWorkspaceState(),
  ) =>
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make({ terminal: initial })
      const registry = yield* makeIntentRegistry(
        terminalWorkspaceIntents,
        makeTerminalWorkspaceHandlers(state, bridge),
      )
      return { state, registry }
    })

  const baseBridge = (): TerminalRendererBridge => ({
    create: async () => ({ ok: true, sessionRef: "terminal.new111aaa", cwdLabel: "repo", shellLabel: "sh", cols: 80, rows: 24 }),
    input: async () => ({ ok: true }),
    interrupt: async () => ({ ok: true }),
    restart: async () => ({ ok: true }),
    close: async () => ({ ok: true }),
    snapshot: async () => ({ sessions: [] }),
    openPreview: async () => ({ ok: true, url: "http://localhost:5173/" }),
  })

  test("create routes through the bridge and adds the returned session", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { state, registry } = yield* makeHarness(baseBridge())
      const view = terminalWorkspaceView((yield* SubscriptionRef.get(state)).terminal)
      const create = nodeByKey(view, "terminal-new") as { onPress: Parameters<typeof resolveIntentRef>[0] }
      yield* registry.dispatch(resolveIntentRef(create.onPress, null))
      const next = (yield* SubscriptionRef.get(state)).terminal
      expect(next.activeRef).toBe("terminal.new111aaa")
      expect(next.sessions).toHaveLength(1)
    }))
  })

  test("submit writes the input line to stdin with a newline and clears the field", async () => {
    const writes: Array<unknown> = []
    await Effect.runPromise(Effect.gen(function* () {
      const bridge: TerminalRendererBridge = {
        ...baseBridge(),
        input: async (value) => { writes.push(value); return { ok: true } },
      }
      const initial: TerminalWorkspaceState = { ...runningState(), input: "npm run build" }
      const { state, registry } = yield* makeHarness(bridge, initial)
      const view = terminalWorkspaceView((yield* SubscriptionRef.get(state)).terminal)
      const send = nodeByKey(view, "terminal-send") as { onPress: Parameters<typeof resolveIntentRef>[0] }
      yield* registry.dispatch(resolveIntentRef(send.onPress, null))
      expect(writes).toEqual([{ sessionRef: "terminal.aaa111bbb", data: "npm run build\n" }])
      expect((yield* SubscriptionRef.get(state)).terminal.input).toBe("")
    }))
  })

  test("interrupt and restart target the active session", async () => {
    const interrupts: Array<unknown> = []
    const restarts: Array<unknown> = []
    await Effect.runPromise(Effect.gen(function* () {
      const bridge: TerminalRendererBridge = {
        ...baseBridge(),
        interrupt: async (value) => { interrupts.push(value); return { ok: true } },
        restart: async (value) => { restarts.push(value); return { ok: true } },
      }
      const { state, registry } = yield* makeHarness(bridge, runningState())
      const view = terminalWorkspaceView((yield* SubscriptionRef.get(state)).terminal)
      const interrupt = nodeByKey(view, "terminal-interrupt") as { onPress: Parameters<typeof resolveIntentRef>[0] }
      const restart = nodeByKey(view, "terminal-restart") as { onPress: Parameters<typeof resolveIntentRef>[0] }
      yield* registry.dispatch(resolveIntentRef(interrupt.onPress, null))
      yield* registry.dispatch(resolveIntentRef(restart.onPress, null))
      expect(interrupts).toEqual([{ sessionRef: "terminal.aaa111bbb" }])
      expect(restarts).toEqual([{ sessionRef: "terminal.aaa111bbb" }])
    }))
  })

  test("a failed preview-open surfaces a typed notice", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const bridge: TerminalRendererBridge = {
        ...baseBridge(),
        openPreview: async () => ({ ok: false, reason: "unknown_port" }),
      }
      const previewState = withTerminalEvent(runningState(), {
        kind: "preview",
        sessionRef: "terminal.aaa111bbb",
        port: 5173,
        url: "http://localhost:5173/",
        ready: true,
      })
      const { state, registry } = yield* makeHarness(bridge, previewState)
      const view = terminalWorkspaceView((yield* SubscriptionRef.get(state)).terminal)
      const open = nodeByKey(view, "terminal-preview-open-5173") as { onPress: Parameters<typeof resolveIntentRef>[0] }
      yield* registry.dispatch(resolveIntentRef(open.onPress, null))
      expect((yield* SubscriptionRef.get(state)).terminal.notice).toContain("unknown_port")
    }))
  })
})
