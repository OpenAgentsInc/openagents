import { describe, expect, test } from "vite-plus/test"

import {
  LiveAgentGraphSchemaLiteral,
  type LiveAgentGraphDelta,
  type LiveAgentGraphNode,
} from "@openagentsinc/agent-runtime-schema"

import {
  LIVE_AGENT_GRAPH_ENTITY_TYPE,
  advanceLiveAgentGraphPostImage,
  decodeLiveAgentGraphPostImageJson,
  emptyLiveAgentGraphEntity,
  liveAgentGraphScope,
  projectLiveAgentGraphPostImage,
} from "./index.js"

const at = (second: number): string =>
  `2026-07-11T20:00:${String(second).padStart(2, "0")}.000Z`

const node = (index: number): LiveAgentGraphNode => ({
  agentRef: `agent.codex.${index}`,
  sessionRef: "session.graph.sync.1",
  threadRef: `thread.codex.${index}`,
  transcriptRef: `transcript.codex.${index}`,
  runRef: `run.codex.${index}`,
  parent: { kind: "root" },
  provider: { state: "known", kind: "codex", providerRef: "provider.codex.owner" },
  runtime: { state: "known", kind: "codex_app_server", runtimeRef: "runtime.codex.owner" },
  worktree: { state: "unknown", reason: "provider_omitted" },
  status: "running",
  attention: { state: "none" },
  terminal: { state: "active" },
  currentTool: { state: "none" },
  attachmentGeneration: 1,
  activityCursor: index,
  createdAt: at(0),
  updatedAt: at(1),
  startedAt: at(0),
  endedAt: null,
  version: 1,
})

const delta = (
  previousCursor: number,
  cursor: number,
  upsertNodes: ReadonlyArray<LiveAgentGraphNode>,
): LiveAgentGraphDelta => ({
  schema: LiveAgentGraphSchemaLiteral,
  deltaRef: `delta.graph.sync.${cursor}`,
  graphRef: "graph.sync.1",
  sessionRef: "session.graph.sync.1",
  threadRef: "thread.canonical.sync.1",
  attachmentGeneration: 1,
  previousCursor,
  cursor,
  upsertNodes,
  removeAgentRefs: [],
  upsertEdges: [],
  removeEdgeRefs: [],
  committedAt: at(cursor + 1),
})

describe("Khala Sync live-agent graph entity", () => {
  test("projects one validated full post-image under the stable graph identity", () => {
    const empty = emptyLiveAgentGraphEntity({
      graphRef: "graph.sync.1",
      sessionRef: "session.graph.sync.1",
      threadRef: "thread.canonical.sync.1",
      attachmentGeneration: 1,
      updatedAt: at(0),
    })
    const projected = projectLiveAgentGraphPostImage(empty)
    expect(projected.entityType).toBe(LIVE_AGENT_GRAPH_ENTITY_TYPE)
    expect(projected.entityId).toBe("graph.sync.1")
    expect(String(liveAgentGraphScope(projected.value.threadRef))).toBe("scope.thread.thread.canonical.sync.1")
    expect(decodeLiveAgentGraphPostImageJson(projected.postImageJson)).toEqual(empty)
  })

  test("advances by the shared exact-cursor reducer and replays as one durable post-image", () => {
    const initial = projectLiveAgentGraphPostImage(emptyLiveAgentGraphEntity({
      graphRef: "graph.sync.1",
      sessionRef: "session.graph.sync.1",
      threadRef: "thread.canonical.sync.1",
      attachmentGeneration: 1,
      updatedAt: at(0),
    }))
    const firstDelta = delta(0, 1, [node(1)])
    const current = advanceLiveAgentGraphPostImage(initial, firstDelta)
    expect(current.value.cursor).toBe(1)
    expect(current.value.nodes).toHaveLength(1)
    expect(advanceLiveAgentGraphPostImage(current, firstDelta)).toEqual(current)
    expect(() => advanceLiveAgentGraphPostImage(current, delta(2, 3, []))).toThrow("exact durable cursor")
  })

  test("round-trips a bounded 2,000-node graph without changing canonical bytes", () => {
    const initial = projectLiveAgentGraphPostImage(emptyLiveAgentGraphEntity({
      graphRef: "graph.sync.1",
      sessionRef: "session.graph.sync.1",
      threadRef: "thread.canonical.sync.1",
      attachmentGeneration: 1,
      updatedAt: at(0),
    }))
    const nodes = Array.from({ length: 2_000 }, (_, index) => node(index))
    const projected = advanceLiveAgentGraphPostImage(initial, delta(0, 1, nodes))
    const decoded = decodeLiveAgentGraphPostImageJson(projected.postImageJson)
    expect(decoded.nodes).toHaveLength(2_000)
    expect(JSON.stringify(decoded)).toBe(projected.postImageJson)
    expect(() => advanceLiveAgentGraphPostImage(initial, delta(0, 1, [...nodes, node(2_001)]))).toThrow()
  })
})
