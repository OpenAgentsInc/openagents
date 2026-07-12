/**
 * CUT-11 (#8691): live main-process wiring for the canonical desktop-local
 * agent graph. This is the residual the assembler landing (`0f475ce89b`)
 * named: one host that owns per-thread `createLocalAgentGraphAssembler`
 * instances and folds the REAL local runtime event stream — the same
 * `emit` callbacks `main.ts` already forwards to the renderer — into
 * canonical `openagents.live_agent_graph.v1` post-images.
 *
 * Wiring shape (deliberately one-callback per lane):
 * - `beginTurn` is called by the fable-local / codex-local start handlers
 *   right before `runTurn`, registering the root turn on the owning thread's
 *   assembler.
 * - `applyEvent` is called once inside each lane's existing `emit` callback
 *   with the SAME typed envelope the renderer receives, so the graph is fed
 *   by the real event flow, not a parallel stream.
 *
 * Every applied observation produces a new validated post-image through the
 * SHARED reducer (graph laws enforced exactly as on the server projection
 * path); each change is pushed to the renderer through `options.emit` and
 * the newest post-images stay available via `snapshot()`. Refusals inside
 * the assembler stay typed and bounded there — this host never guesses and
 * never throws into the turn's emit path.
 */
import { randomUUID } from "node:crypto"

import type { FableLocalEventEnvelope } from "./fable-local-contract.ts"
import type { LiveAgentGraphHostSnapshotWire, LiveAgentGraphUpdateWire } from "./live-agent-graph-contract.ts"
import {
  createLocalAgentGraphAssembler,
  type LocalAgentGraphAssembler,
  type LocalAgentGraphLane,
  type LocalAgentGraphResult,
} from "./live-agent-graph-local.ts"

/**
 * Bounded retained thread graphs. Matches the confirmed Sync client read
 * model's newest-payload bound (8 graphs) so the desktop-local surface never
 * grows past what the canonical delivery path itself would carry.
 */
const HOST_GRAPH_LIMIT = 8

export type LiveAgentGraphHostOptions = Readonly<{
  /** Push one changed graph to the renderer (broadcast seam in main.ts). */
  emit: (update: LiveAgentGraphUpdateWire) => void
  /** Clock seam for deterministic tests. */
  now?: () => string
  /** Session identity seam; defaults to one stable ref per app run. */
  sessionRef?: string
  /** Retention bound seam for tests. */
  graphLimit?: number
}>

export type LiveAgentGraphHost = Readonly<{
  /** Register a root turn before its stream events arrive. */
  beginTurn: (input: Readonly<{
    turnRef: string
    threadRef: string
    lane: LocalAgentGraphLane
  }>) => LocalAgentGraphResult
  /** Fold one typed local runtime event envelope from the REAL emit path. */
  applyEvent: (threadRef: string, envelope: FableLocalEventEnvelope) => LocalAgentGraphResult | null
  /** Current retained canonical graphs (encoded snapshots, oldest first). */
  snapshot: () => LiveAgentGraphHostSnapshotWire
}>

type ThreadGraphState = Readonly<{
  assembler: LocalAgentGraphAssembler
}> & { lastUpdatedAt: string; runningTurns: number }

export const makeLiveAgentGraphHost = (options: LiveAgentGraphHostOptions): LiveAgentGraphHost => {
  const now = options.now ?? (() => new Date().toISOString())
  const sessionRef = options.sessionRef ?? `session.desktop.${randomUUID()}`
  const graphLimit = options.graphLimit ?? HOST_GRAPH_LIMIT
  const threads = new Map<string, ThreadGraphState>()

  const encodedGraph = (state: ThreadGraphState): unknown =>
    JSON.parse(state.assembler.postImage().postImageJson) as unknown

  const push = (threadRef: string, state: ThreadGraphState): void => {
    state.lastUpdatedAt = now()
    try {
      options.emit({ threadRef, graph: encodedGraph(state) })
    } catch {
      // The renderer push is best-effort; a broken listener must never
      // break the live turn's emit path.
    }
  }

  /**
   * Evict the least-recently-updated IDLE thread graph past the bound. A
   * graph with a running turn is never evicted — losing a live graph
   * mid-turn would be silent loss, so with all graphs live the map may
   * briefly exceed the bound rather than lie.
   */
  const evictPastLimit = (): void => {
    while (threads.size > graphLimit) {
      let oldest: string | null = null
      let oldestAt = ""
      for (const [threadRef, state] of threads) {
        if (state.runningTurns > 0) continue
        if (oldest === null || state.lastUpdatedAt < oldestAt) {
          oldest = threadRef
          oldestAt = state.lastUpdatedAt
        }
      }
      if (oldest === null) return
      threads.delete(oldest)
    }
  }

  const stateFor = (threadRef: string): ThreadGraphState => {
    const existing = threads.get(threadRef)
    if (existing !== undefined) return existing
    const created: ThreadGraphState = {
      assembler: createLocalAgentGraphAssembler({
        sessionRef,
        threadRef,
        createdAt: now(),
      }),
      lastUpdatedAt: now(),
      runningTurns: 0,
    }
    threads.set(threadRef, created)
    evictPastLimit()
    return created
  }

  const isTerminalRootEvent = (envelope: FableLocalEventEnvelope): boolean =>
    envelope.event.kind === "turn_completed" || envelope.event.kind === "turn_failed"

  return {
    beginTurn: input => {
      const state = stateFor(input.threadRef)
      const result = state.assembler.startTurn(
        { turnRef: input.turnRef, threadRef: input.threadRef, lane: input.lane },
        now(),
      )
      if (result.applied) {
        state.runningTurns += 1
        push(input.threadRef, state)
      }
      return result
    },
    applyEvent: (threadRef, envelope) => {
      const state = threads.get(threadRef)
      // An event for a thread this host never began is not guessed into a
      // graph — the assembler would refuse it as unknown_turn anyway, but
      // without a begun turn there is no assembler to record the refusal.
      if (state === undefined) return null
      const result = state.assembler.applyEvent(envelope, now())
      if (result.applied) {
        if (isTerminalRootEvent(envelope) && state.runningTurns > 0) {
          state.runningTurns -= 1
        }
        push(threadRef, state)
      }
      return result
    },
    snapshot: () => ({
      sessionRef,
      graphs: [...threads.entries()]
        .sort((a, b) => (a[1].lastUpdatedAt < b[1].lastUpdatedAt ? -1 : 1))
        .map(([threadRef, state]) => ({ threadRef, graph: encodedGraph(state) })),
    }),
  }
}
