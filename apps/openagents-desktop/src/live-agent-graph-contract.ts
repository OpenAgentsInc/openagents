/**
 * CUT-11 (#8691): canonical live agent graph IPC contract — the additive
 * main -> renderer delivery seam for the desktop-local
 * `openagents.live_agent_graph.v1` post-images assembled by
 * `live-agent-graph-host.ts` from the REAL claude-local / codex-local event
 * streams.
 *
 * Delivery follows the established desktop seams exactly:
 * - snapshot on invoke (`LiveAgentGraphSnapshotChannel`, renderer-argument
 *   free, mirrors the usage-ledger snapshot channel); and
 * - push on change (`LiveAgentGraphUpdateChannel`, mirrors the terminal
 *   event channel broadcast).
 *
 * The payload is the ENCODED canonical graph snapshot (the same bytes the
 * shared reducer's post-image carries), so the renderer re-validates through
 * the shared `decodeLiveAgentGraphEntity` law — no parallel graph shape, no
 * renderer-trusted pre-parsed structure. Graph PRESENTATION stays CUT-12;
 * this contract only makes the canonical post-images available.
 */
import { decodeLiveAgentGraphEntity, type LiveAgentGraphEntity } from "@openagentsinc/khala-sync"

export const LiveAgentGraphSnapshotChannel = "openagents:live-agent-graph:snapshot" as const
export const LiveAgentGraphUpdateChannel = "openagents:live-agent-graph:update" as const

/**
 * One pushed graph update as sent over the wire: the owning thread plus the
 * ENCODED canonical snapshot (validated by the renderer decode below).
 */
export type LiveAgentGraphUpdateWire = Readonly<{
  threadRef: string
  graph: unknown
}>

/** The invoke snapshot wire shape: every retained thread graph. */
export type LiveAgentGraphHostSnapshotWire = Readonly<{
  sessionRef: string
  graphs: ReadonlyArray<LiveAgentGraphUpdateWire>
}>

/** One renderer-validated graph update. */
export type LiveAgentGraphUpdate = Readonly<{
  threadRef: string
  graph: LiveAgentGraphEntity
}>

/** The renderer-validated invoke snapshot, oldest-updated first. */
export type LiveAgentGraphHostSnapshot = Readonly<{
  sessionRef: string
  graphs: ReadonlyArray<LiveAgentGraphUpdate>
}>

const decodeGraphValue = (value: unknown): LiveAgentGraphEntity | null => {
  try {
    return decodeLiveAgentGraphEntity(value)
  } catch {
    return null
  }
}

/** Renderer-side validation for one pushed update. Malformed -> null. */
export const decodeLiveAgentGraphUpdate = (value: unknown): LiveAgentGraphUpdate | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as { threadRef?: unknown; graph?: unknown }
  if (typeof record.threadRef !== "string" || record.threadRef.length === 0) return null
  const graph = decodeGraphValue(record.graph)
  return graph === null ? null : { threadRef: record.threadRef, graph }
}

/** Renderer-side validation for the invoke snapshot. Malformed -> null. */
export const decodeLiveAgentGraphHostSnapshot = (value: unknown): LiveAgentGraphHostSnapshot | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as { sessionRef?: unknown; graphs?: unknown }
  if (typeof record.sessionRef !== "string" || record.sessionRef.length === 0) return null
  if (!Array.isArray(record.graphs)) return null
  const graphs: Array<LiveAgentGraphUpdate> = []
  for (const entry of record.graphs) {
    const decoded = decodeLiveAgentGraphUpdate(entry)
    // One malformed row invalidates the snapshot rather than silently
    // shrinking it — a partial graph list would be a silent loss.
    if (decoded === null) return null
    graphs.push(decoded)
  }
  return { sessionRef: record.sessionRef, graphs }
}
