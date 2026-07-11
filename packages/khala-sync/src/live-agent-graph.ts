import {
  LiveAgentGraphSchemaLiteral,
  LiveAgentGraphSnapshot,
  applyLiveAgentGraphDelta,
  validateLiveAgentGraphSnapshot,
  type LiveAgentGraphDelta,
  type LiveAgentGraphSnapshot as LiveAgentGraphSnapshotType,
} from "@openagentsinc/agent-runtime-schema"
import { Schema as S } from "effect"

/**
 * Canonical live-agent graph post-image for Khala Sync.
 *
 * One graph is stored under `scope.thread.<sessionRef>` with `graphRef` as the
 * entity id. Every committed cursor is a full, validated post-image. That
 * deliberately follows Khala Sync v1's post-image model: clients do not need
 * provider history or an out-of-band graph patch protocol to reconstruct the
 * current graph after bootstrap/reconnect.
 *
 * Provider observations must first terminate at the shared Codex/Claude
 * adapters. This boundary accepts only the provider-neutral graph schema and
 * re-runs all graph integrity laws before encoding a changelog post-image.
 */

export const LIVE_AGENT_GRAPH_ENTITY_TYPE = "live_agent_graph"

export const LiveAgentGraphEntity = LiveAgentGraphSnapshot
export type LiveAgentGraphEntity = typeof LiveAgentGraphEntity.Type

export type LiveAgentGraphPostImage = Readonly<{
  entityType: typeof LIVE_AGENT_GRAPH_ENTITY_TYPE
  entityId: string
  postImageJson: string
  value: LiveAgentGraphSnapshotType
}>

const encodeLiveAgentGraphEntity = S.encodeSync(LiveAgentGraphEntity)

export const decodeLiveAgentGraphEntity = (input: unknown): LiveAgentGraphEntity =>
  validateLiveAgentGraphSnapshot(input as LiveAgentGraphSnapshotType)

export const projectLiveAgentGraphPostImage = (
  input: LiveAgentGraphSnapshotType,
): LiveAgentGraphPostImage => {
  const value = validateLiveAgentGraphSnapshot(input)
  return {
    entityType: LIVE_AGENT_GRAPH_ENTITY_TYPE,
    entityId: value.graphRef,
    postImageJson: JSON.stringify(encodeLiveAgentGraphEntity(value)),
    value,
  }
}

export const advanceLiveAgentGraphPostImage = (
  current: LiveAgentGraphPostImage,
  delta: LiveAgentGraphDelta,
): LiveAgentGraphPostImage => {
  if (
    current.entityType !== LIVE_AGENT_GRAPH_ENTITY_TYPE ||
    current.entityId !== current.value.graphRef
  ) throw new Error("live-agent graph post-image identity mismatch")
  return projectLiveAgentGraphPostImage(applyLiveAgentGraphDelta(current.value, delta))
}

export const decodeLiveAgentGraphPostImageJson = (json: string): LiveAgentGraphEntity =>
  decodeLiveAgentGraphEntity(JSON.parse(json))

export const emptyLiveAgentGraphEntity = (input: Readonly<{
  graphRef: string
  sessionRef: string
  attachmentGeneration: number
  updatedAt: string
}>): LiveAgentGraphEntity => decodeLiveAgentGraphEntity({
  schema: LiveAgentGraphSchemaLiteral,
  graphRef: input.graphRef,
  sessionRef: input.sessionRef,
  attachmentGeneration: input.attachmentGeneration,
  cursor: 0,
  lastDeltaRef: null,
  nodes: [],
  edges: [],
  updatedAt: input.updatedAt,
})
