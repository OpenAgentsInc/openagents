import {
  LIVE_AGENT_GRAPH_ENTITY_TYPE,
  LiveAgentGraphEntity,
  decodeLiveAgentGraphPostImageJson,
  threadScope,
} from "@openagentsinc/khala-sync"
import { Effect, Schema } from "effect"

import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type {
  ConfirmedEntity,
  KhalaSyncClientStoreError,
  KhalaSyncLocalStore,
} from "./store.js"

export const MAX_CONFIRMED_LIVE_AGENT_GRAPHS = 8
export const MAX_CONFIRMED_LIVE_AGENT_GRAPH_NODES = 2_000
export const MAX_CONFIRMED_LIVE_AGENT_GRAPH_EDGES = 4_000

export const ConfirmedLiveAgentGraphsSchema = Schema.Array(LiveAgentGraphEntity).pipe(
  Schema.check(
    Schema.isMaxLength(MAX_CONFIRMED_LIVE_AGENT_GRAPHS),
    Schema.makeFilter(
      graphs =>
        graphs.reduce((count, graph) => count + graph.nodes.length, 0) <=
          MAX_CONFIRMED_LIVE_AGENT_GRAPH_NODES &&
        graphs.reduce((count, graph) => count + graph.edges.length, 0) <=
          MAX_CONFIRMED_LIVE_AGENT_GRAPH_EDGES,
      { message: "confirmed live-agent graph snapshot exceeds aggregate bounds" },
    ),
  ),
)

export type KhalaSyncLiveAgentGraphStatus = Readonly<{
  phase: ScopeSyncState["phase"]
  cursor: number | null
  pendingMutationCount: number
}>

export type ConfirmedLiveAgentGraphSnapshot = Readonly<{
  status: KhalaSyncLiveAgentGraphStatus
  graphs: ReadonlyArray<LiveAgentGraphEntity>
}>

export type KhalaSyncLiveAgentGraph = Readonly<{
  status: (threadRef: string) => KhalaSyncLiveAgentGraphStatus
  snapshotForThread: (threadRef: string) => Effect.Effect<
    ConfirmedLiveAgentGraphSnapshot,
    KhalaSyncClientStoreError
  >
}>

const cursorFromState = (state: ScopeSyncState): number | null =>
  state.phase === "live" || state.phase === "catching_up"
    ? Number(state.cursor)
    : null

const confirmedGraphs = (
  threadRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): ReadonlyArray<LiveAgentGraphEntity> => {
  const graphs: Array<Readonly<{ graph: LiveAgentGraphEntity; version: number }>> = []
  for (const row of rows) {
    try {
      const graph = decodeLiveAgentGraphPostImageJson(row.postImageJson)
      if (graph.threadRef !== threadRef || graph.graphRef !== row.entityId) continue
      graphs.push({ graph, version: Number(row.version) })
    } catch {
      // Ignore malformed/pre-contract rows; a confirmed replacement self-heals.
    }
  }
  const newest = graphs.sort((left, right) =>
    right.version - left.version ||
    right.graph.graphRef.localeCompare(left.graph.graphRef))
  const bounded: Array<LiveAgentGraphEntity> = []
  let nodeCount = 0
  let edgeCount = 0
  for (const entry of newest) {
    if (bounded.length >= MAX_CONFIRMED_LIVE_AGENT_GRAPHS) break
    const { graph } = entry
    const nextNodeCount = nodeCount + graph.nodes.length
    const nextEdgeCount = edgeCount + graph.edges.length
    if (
      nextNodeCount > MAX_CONFIRMED_LIVE_AGENT_GRAPH_NODES ||
      nextEdgeCount > MAX_CONFIRMED_LIVE_AGENT_GRAPH_EDGES
    ) break
    bounded.push(graph)
    nodeCount = nextNodeCount
    edgeCount = nextEdgeCount
  }
  return bounded.reverse()
}

/** Read only server-confirmed graph post-images from the canonical thread scope. */
export const createKhalaSyncLiveAgentGraph = (input: Readonly<{
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
}>): KhalaSyncLiveAgentGraph => {
  const status = (threadRef: string): KhalaSyncLiveAgentGraphStatus => {
    const state = input.session.state(threadScope(threadRef))
    return {
      phase: state.phase,
      cursor: cursorFromState(state),
      pendingMutationCount: input.session.pending().length,
    }
  }

  return {
    status,
    snapshotForThread: threadRef => {
      const graphStatus = status(threadRef)
      if (graphStatus.phase !== "live") {
        return Effect.succeed({ status: graphStatus, graphs: [] })
      }
      return Effect.map(
        input.store.readEntities(threadScope(threadRef), LIVE_AGENT_GRAPH_ENTITY_TYPE),
        rows => ({ status: graphStatus, graphs: confirmedGraphs(threadRef, rows) }),
      )
    },
  }
}
