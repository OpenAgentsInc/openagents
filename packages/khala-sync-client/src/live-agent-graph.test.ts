import {
  ChangelogEntry,
  EntityId,
  EntityType,
  LIVE_AGENT_GRAPH_ENTITY_TYPE,
  SyncVersion,
  SyncVersionWatermark,
  decodeLiveAgentGraphEntity,
  emptyLiveAgentGraphEntity,
  projectLiveAgentGraphPostImage,
  threadScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  MAX_CONFIRMED_LIVE_AGENT_GRAPHS,
  createKhalaSyncLiveAgentGraph,
} from "./live-agent-graph.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const NOW = "2026-07-11T20:00:00.000Z"
const THREAD = "thread.live-agent-graph.fixture"
const scope = threadScope(THREAD)

const graphEntry = (
  version: number,
  graphRef: string,
  threadRef = THREAD,
  nodeCount = 0,
) => {
  const empty = emptyLiveAgentGraphEntity({
    graphRef,
    sessionRef: "session.live-agent-graph.fixture",
    threadRef,
    attachmentGeneration: 1,
    updatedAt: new Date(Date.parse(NOW) + version * 1_000).toISOString(),
  })
  const graph = decodeLiveAgentGraphEntity({
    ...empty,
    nodes: Array.from({ length: nodeCount }, (_, index) => ({
      agentRef: `agent.aggregate.${version}.${index}`,
      sessionRef: empty.sessionRef,
      threadRef: `thread.provider.${version}.${index}`,
      transcriptRef: `transcript.aggregate.${version}.${index}`,
      runRef: `run.aggregate.${version}.${index}`,
      parent: { kind: "root" },
      provider: { state: "unknown", reason: "provider_omitted" },
      runtime: { state: "unknown", reason: "not_observed" },
      worktree: { state: "unknown", reason: "provider_omitted" },
      status: "queued",
      attention: { state: "none" },
      terminal: { state: "active" },
      currentTool: { state: "unknown", reason: "not_observed" },
      attachmentGeneration: 1,
      activityCursor: 0,
      createdAt: empty.updatedAt,
      updatedAt: empty.updatedAt,
      startedAt: null,
      endedAt: null,
      version: 1,
    })),
  })
  const postImage = projectLiveAgentGraphPostImage(graph)
  return new ChangelogEntry({
    scope,
    version: SyncVersion.make(version),
    entityType: EntityType.make(LIVE_AGENT_GRAPH_ENTITY_TYPE),
    entityId: EntityId.make(postImage.entityId),
    op: "upsert",
    postImageJson: postImage.postImageJson,
    mutationRef: `mutation.live-agent-graph.${version}`,
    committedAt: NOW,
  })
}

const session = (
  phase: ScopeSyncState = {
    phase: "live",
    cursor: SyncVersionWatermark.make(1),
  },
): KhalaSyncSession => ({
  state: (_scope: SyncScope) => phase,
  pending: () => [],
}) as unknown as KhalaSyncSession

describe("confirmed live-agent graph read model", () => {
  test("returns only graph-valid post-images from the exact live thread scope", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [
        graphEntry(1, "graph.runtime.first"),
        graphEntry(2, "graph.runtime.foreign", "thread.other"),
        graphEntry(3, "graph.runtime.latest"),
      ], SyncVersion.make(3)))
      const model = createKhalaSyncLiveAgentGraph({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(3) }),
      })
      const snapshot = Effect.runSync(model.snapshotForThread(THREAD))

      expect(snapshot.status).toEqual({
        phase: "live",
        cursor: 3,
        pendingMutationCount: 0,
      })
      expect(snapshot.graphs.map(graph => graph.graphRef)).toEqual([
        "graph.runtime.first",
        "graph.runtime.latest",
      ])
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("hides cached graphs until the canonical thread scope is live", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(
        scope,
        [graphEntry(1, "graph.runtime.cached")],
        SyncVersion.make(1),
      ))
      const model = createKhalaSyncLiveAgentGraph({
        store,
        session: session({ phase: "must_refetch", reason: "retention_gap" }),
      })
      expect(Effect.runSync(model.snapshotForThread(THREAD))).toEqual({
        status: { phase: "must_refetch", cursor: null, pendingMutationCount: 0 },
        graphs: [],
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("bounds a busy thread to the newest confirmed graph post-images", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const count = MAX_CONFIRMED_LIVE_AGENT_GRAPHS + 2
      const entries = Array.from({ length: count }, (_, index) =>
        graphEntry(index + 1, `graph.runtime.${String(index + 1).padStart(3, "0")}`))
      Effect.runSync(store.applyConfirmed(scope, entries, SyncVersion.make(count)))
      const model = createKhalaSyncLiveAgentGraph({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(count) }),
      })
      const graphs = Effect.runSync(model.snapshotForThread(THREAD)).graphs

      expect(graphs).toHaveLength(MAX_CONFIRMED_LIVE_AGENT_GRAPHS)
      expect(graphs[0]?.graphRef).toBe(
        `graph.runtime.${String(count - MAX_CONFIRMED_LIVE_AGENT_GRAPHS + 1).padStart(3, "0")}`,
      )
      expect(graphs.at(-1)?.graphRef).toBe(
        `graph.runtime.${String(count).padStart(3, "0")}`,
      )
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("enforces the aggregate node budget across otherwise valid graphs", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [
        graphEntry(1, "graph.runtime.aggregate.older", THREAD, 1_001),
        graphEntry(2, "graph.runtime.aggregate.newest", THREAD, 1_001),
      ], SyncVersion.make(2)))
      const model = createKhalaSyncLiveAgentGraph({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(2) }),
      })
      const graphs = Effect.runSync(model.snapshotForThread(THREAD)).graphs

      expect(graphs).toHaveLength(1)
      expect(graphs[0]?.graphRef).toBe("graph.runtime.aggregate.newest")
    } finally {
      Effect.runSync(store.close())
    }
  })
})
