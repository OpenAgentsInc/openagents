import { describe, expect, test } from "vite-plus/test"

import {
  LiveAgentGraphError,
  LiveAgentGraphSchemaLiteral,
  applyLiveAgentGraphDelta,
  decodeLiveAgentGraphDelta,
  decodeLiveAgentGraphSnapshot,
  validateLiveAgentGraphSnapshot,
  type LiveAgentGraphDelta,
  type LiveAgentGraphEdge,
  type LiveAgentGraphNode,
  type LiveAgentGraphParent,
  type LiveAgentGraphSnapshot,
  type LiveAgentGraphStatus,
} from "./live-agent-graph.js"

const at = (minute: number): string =>
  `2026-07-11T19:${String(minute).padStart(2, "0")}:00.000Z`

const node = (input: Readonly<{
  agentRef: string
  parent?: LiveAgentGraphParent
  status?: LiveAgentGraphStatus
  version?: number
  cursor?: number
  updatedAt?: string
}>): LiveAgentGraphNode => {
  const status = input.status ?? "running"
  const terminal = status === "completed" || status === "failed" ||
    status === "canceled" || status === "interrupted"
  return {
    agentRef: input.agentRef,
    sessionRef: "session.graph.1",
    threadRef: `thread.${input.agentRef}`,
    transcriptRef: `transcript.${input.agentRef}`,
    runRef: `run.${input.agentRef}`,
    parent: input.parent ?? { kind: "root" },
    provider: { state: "known", kind: "codex", providerRef: "provider.codex.1" },
    runtime: { state: "known", kind: "codex_app_server", runtimeRef: "runtime.codex.1" },
    worktree: { state: "unknown", reason: "provider_omitted" },
    status,
    attention: { state: "none" },
    terminal: terminal
      ? { state: "terminal", reason: status, at: input.updatedAt ?? at(1) }
      : { state: "active" },
    currentTool: { state: "unknown", reason: "not_observed" },
    attachmentGeneration: 1,
    activityCursor: input.cursor ?? 0,
    createdAt: at(0),
    updatedAt: input.updatedAt ?? at(1),
    startedAt: status === "queued" ? null : at(0),
    endedAt: terminal ? input.updatedAt ?? at(1) : null,
    version: input.version ?? 1,
  }
}

const parentEdge = (
  parentRef: string,
  childRef: string,
  version = 1,
): LiveAgentGraphEdge => ({
  edgeRef: `edge.parent.${parentRef}.${childRef}`,
  kind: "parent",
  fromAgentRef: parentRef,
  toAgentRef: childRef,
  version,
})

const snapshot = (
  nodes: ReadonlyArray<LiveAgentGraphNode> = [node({ agentRef: "agent.root" })],
  edges: ReadonlyArray<LiveAgentGraphEdge> = [],
): LiveAgentGraphSnapshot => ({
  schema: LiveAgentGraphSchemaLiteral,
  graphRef: "graph.session.1",
  sessionRef: "session.graph.1",
  threadRef: "thread.canonical.1",
  attachmentGeneration: 1,
  cursor: 0,
  lastDeltaRef: null,
  nodes,
  edges,
  updatedAt: at(1),
})

const delta = (input: Partial<LiveAgentGraphDelta> = {}): LiveAgentGraphDelta => ({
  schema: LiveAgentGraphSchemaLiteral,
  deltaRef: "delta.graph.1",
  graphRef: "graph.session.1",
  sessionRef: "session.graph.1",
  threadRef: "thread.canonical.1",
  attachmentGeneration: 1,
  previousCursor: 0,
  cursor: 1,
  upsertNodes: [],
  removeAgentRefs: [],
  upsertEdges: [],
  removeEdgeRefs: [],
  committedAt: at(2),
  ...input,
})

const errorReason = (run: () => unknown): string => {
  try {
    run()
    return "none"
  } catch (error) {
    return error instanceof LiveAgentGraphError ? error.reason : "unexpected"
  }
}

const shuffled = <Value>(values: ReadonlyArray<Value>, seed: number): Array<Value> => {
  const result = [...values]
  let state = seed || 1
  for (let index = result.length - 1; index > 0; index -= 1) {
    state = (state * 48271) % 0x7fffffff
    const target = state % (index + 1)
    ;[result[index], result[target]] = [result[target]!, result[index]!]
  }
  return result
}

describe("canonical live agent graph v1", () => {
  test("schema requires stable refs and explicit loss-accounted unknown facts", () => {
    const root = node({ agentRef: "agent.root" })
    expect(decodeLiveAgentGraphSnapshot(snapshot([root]))).toEqual(snapshot([root]))
    expect(() => decodeLiveAgentGraphSnapshot({
      ...snapshot([root]),
      nodes: [{ ...root, provider: undefined }],
    })).toThrow()
    expect(() => decodeLiveAgentGraphSnapshot({
      ...snapshot([root]),
      graphRef: "../private/path",
    })).toThrow()
    expect(root.worktree).toEqual({ state: "unknown", reason: "provider_omitted" })
    expect(root.currentTool).toEqual({ state: "unknown", reason: "not_observed" })
  })

  test("adds a child atomically with one stable parent edge", () => {
    const child = node({
      agentRef: "agent.child",
      parent: { kind: "agent", agentRef: "agent.root" },
    })
    const next = applyLiveAgentGraphDelta(snapshot(), delta({
      upsertNodes: [child],
      upsertEdges: [parentEdge("agent.root", "agent.child")],
    }))

    expect(next.cursor).toBe(1)
    expect(next.lastDeltaRef).toBe("delta.graph.1")
    expect(next.nodes.map(value => value.agentRef)).toEqual(["agent.child", "agent.root"])
    expect(validateLiveAgentGraphSnapshot(next)).toEqual(next)
  })

  test("exact replay is idempotent while gaps, conflicts, and stale generations fail", () => {
    const first = delta()
    const current = applyLiveAgentGraphDelta(snapshot(), first)
    expect(applyLiveAgentGraphDelta(current, first)).toEqual(current)
    expect(errorReason(() => applyLiveAgentGraphDelta(current, {
      ...first,
      deltaRef: "delta.conflict",
    }))).toBe("stale_delta")
    expect(errorReason(() => applyLiveAgentGraphDelta(current, delta({
      deltaRef: "delta.gap",
      previousCursor: 2,
      cursor: 3,
    })))).toBe("cursor_gap")
    expect(errorReason(() => applyLiveAgentGraphDelta(current, delta({
      deltaRef: "delta.generation",
      attachmentGeneration: 2,
      previousCursor: 1,
      cursor: 2,
    })))).toBe("generation_mismatch")
    expect(errorReason(() => applyLiveAgentGraphDelta(current, delta({
      deltaRef: "delta.thread",
      threadRef: "thread.canonical.other",
      previousCursor: 1,
      cursor: 2,
    })))).toBe("graph_mismatch")
  })

  test("terminal agents cannot reopen or regress their activity cursor", () => {
    const completed = node({
      agentRef: "agent.root",
      status: "completed",
      version: 2,
      cursor: 4,
      updatedAt: at(4),
    })
    const current = { ...snapshot([completed]), cursor: 4, updatedAt: at(4) }
    expect(errorReason(() => applyLiveAgentGraphDelta(current, delta({
      deltaRef: "delta.reopen",
      previousCursor: 4,
      cursor: 5,
      upsertNodes: [node({
        agentRef: "agent.root",
        status: "running",
        version: 3,
        cursor: 5,
        updatedAt: at(5),
      })],
      committedAt: at(5),
    })))).toBe("terminal_reopened")
    expect(errorReason(() => applyLiveAgentGraphDelta(snapshot(), delta({
      upsertNodes: [node({
        agentRef: "agent.root",
        version: 2,
        cursor: 0,
        updatedAt: at(2),
      })],
    })))).not.toBe("cursor_regression")
    const cursorFour = { ...snapshot([node({ agentRef: "agent.root", cursor: 4 })]) }
    expect(errorReason(() => applyLiveAgentGraphDelta(cursorFour, delta({
      upsertNodes: [node({ agentRef: "agent.root", version: 2, cursor: 3, updatedAt: at(2) })],
    })))).toBe("cursor_regression")
    expect(errorReason(() => validateLiveAgentGraphSnapshot(snapshot([{
      ...completed,
      terminal: { state: "terminal", reason: "failed", at: at(4) },
    }])))).toBe("terminal_mismatch")
    expect(errorReason(() => applyLiveAgentGraphDelta(snapshot(), delta({
      committedAt: at(0),
    })))).toBe("timestamp_regression")
  })

  test("rejects missing parents, mismatched edges, orphan tools, and cycles", () => {
    const child = node({
      agentRef: "agent.child",
      parent: { kind: "agent", agentRef: "agent.missing" },
    })
    expect(errorReason(() => validateLiveAgentGraphSnapshot(snapshot([child])))).toBe("missing_parent")
    expect(errorReason(() => validateLiveAgentGraphSnapshot(snapshot([
      node({ agentRef: "agent.root" }),
      node({ agentRef: "agent.child", parent: { kind: "agent", agentRef: "agent.root" } }),
    ])))).toBe("parent_edge_mismatch")
    expect(errorReason(() => validateLiveAgentGraphSnapshot(snapshot([], [{
      edgeRef: "edge.tool.orphan",
      kind: "tool",
      agentRef: "agent.missing",
      toolCallRef: "tool.missing",
      status: "unknown",
      version: 1,
    }])))).toBe("orphan_edge")
    const left = node({ agentRef: "agent.left", parent: { kind: "agent", agentRef: "agent.right" } })
    const right = node({ agentRef: "agent.right", parent: { kind: "agent", agentRef: "agent.left" } })
    expect(errorReason(() => validateLiveAgentGraphSnapshot(snapshot(
      [left, right],
      [parentEdge("agent.right", "agent.left"), parentEdge("agent.left", "agent.right")],
    )))).toBe("cycle")
  })

  test("permutation property: one atomic child batch converges byte-identically", () => {
    const children = Array.from({ length: 64 }, (_, index) => node({
      agentRef: `agent.child.${index}`,
      parent: { kind: "agent", agentRef: "agent.root" },
    }))
    const edges = children.map(child => parentEdge("agent.root", child.agentRef))
    const expected = applyLiveAgentGraphDelta(snapshot(), delta({
      upsertNodes: children,
      upsertEdges: edges,
    }))
    const encoded = JSON.stringify(expected)

    for (let seed = 1; seed <= 50; seed += 1) {
      const candidate = applyLiveAgentGraphDelta(snapshot(), decodeLiveAgentGraphDelta(delta({
        upsertNodes: shuffled(children, seed),
        upsertEdges: shuffled(edges, seed * 7),
      })))
      expect(JSON.stringify(candidate)).toBe(encoded)
    }
  })
})
