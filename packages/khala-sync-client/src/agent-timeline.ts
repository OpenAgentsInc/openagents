import {
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  agentRunScope,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import type { OverlayError } from "./overlay.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type {
  ConfirmedEntity,
  KhalaSyncClientStoreError,
  KhalaSyncLocalStore,
} from "./store.js"

export const MAX_CONFIRMED_AGENT_TIMELINE_EVENTS = 500

export type ConfirmedAgentRun = Readonly<{
  runRef: string
  routeRef: string
  status: "queued" | "running" | "waiting_for_input" | "completed" | "failed" | "canceled"
  createdAt: string
  updatedAt: string
  startedAt: string | null
  completedAt: string | null
  failedAt: string | null
  canceledAt: string | null
  version: number
}>

export type ConfirmedAgentTimelineEvent = Readonly<{
  eventRef: string
  runRef: string
  sequence: number
  eventType: string
  summary: string
  status: string | null
  artifactRefs: ReadonlyArray<string>
  createdAt: string
  version: number
}>

export type KhalaSyncAgentTimelineStatus = Readonly<{
  phase: ScopeSyncState["phase"]
  cursor: number | null
  pendingMutationCount: number
}>

export type ConfirmedAgentTimelineSnapshot = Readonly<{
  status: KhalaSyncAgentTimelineStatus
  run: ConfirmedAgentRun | null
  events: ReadonlyArray<ConfirmedAgentTimelineEvent>
}>

export type KhalaSyncAgentTimeline = Readonly<{
  status: (runRef: string) => KhalaSyncAgentTimelineStatus
  open: (runRef: string) => Effect.Effect<void, OverlayError>
  snapshot: (runRef: string) => Effect.Effect<
    ConfirmedAgentTimelineSnapshot,
    KhalaSyncClientStoreError
  >
}>

const cursorFromState = (state: ScopeSyncState): number | null =>
  state.phase === "live" || state.phase === "catching_up"
    ? Number(state.cursor)
    : null

const confirmedRun = (
  runRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): ConfirmedAgentRun | null => {
  let result: ConfirmedAgentRun | null = null
  for (const row of rows) {
    try {
      const run = decodeAgentRunEntity(JSON.parse(row.postImageJson) as unknown)
      if (run.runId !== runRef) continue
      if (result !== null && result.version >= Number(row.version)) continue
      result = {
        runRef: run.runId,
        routeRef: run.routeId,
        status: run.status,
        createdAt: run.createdAt,
        updatedAt: run.updatedAt,
        startedAt: run.startedAt,
        completedAt: run.completedAt,
        failedAt: run.failedAt,
        canceledAt: run.canceledAt,
        version: Number(row.version),
      }
    } catch {
      // Ignore malformed/pre-contract rows; confirmed replacement self-heals.
    }
  }
  return result
}

const confirmedEvents = (
  runRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): ReadonlyArray<ConfirmedAgentTimelineEvent> => {
  const byRef = new Map<string, ConfirmedAgentTimelineEvent>()
  for (const row of rows) {
    try {
      const event = decodeAgentRunEventEntity(JSON.parse(row.postImageJson) as unknown)
      if (event.runId !== runRef) continue
      const projected: ConfirmedAgentTimelineEvent = {
        eventRef: event.id,
        runRef: event.runId,
        sequence: event.sequence,
        eventType: event.type,
        summary: event.summary,
        status: event.status,
        artifactRefs: event.artifactRefs,
        createdAt: event.createdAt,
        version: Number(row.version),
      }
      const previous = byRef.get(projected.eventRef)
      if (previous === undefined || previous.version < projected.version) {
        byRef.set(projected.eventRef, projected)
      }
    } catch {
      // Ignore malformed/pre-contract rows; confirmed replacement self-heals.
    }
  }
  return [...byRef.values()]
    .sort((left, right) =>
      left.sequence - right.sequence || left.eventRef.localeCompare(right.eventRef))
    .slice(-MAX_CONFIRMED_AGENT_TIMELINE_EVENTS)
}

export const createKhalaSyncAgentTimeline = (input: Readonly<{
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
}>): KhalaSyncAgentTimeline => {
  const status = (runRef: string): KhalaSyncAgentTimelineStatus => {
    const state = input.session.state(agentRunScope(runRef))
    return {
      phase: state.phase,
      cursor: cursorFromState(state),
      pendingMutationCount: input.session.pending().length,
    }
  }

  return {
    status,
    open: runRef => input.session.subscribe(agentRunScope(runRef)),
    snapshot: runRef => {
      const timelineStatus = status(runRef)
      if (timelineStatus.phase !== "live") {
        return Effect.succeed({ status: timelineStatus, run: null, events: [] })
      }
      const scope = agentRunScope(runRef)
      return Effect.map(
        Effect.all([
          input.store.readEntities(scope, AGENT_RUN_ENTITY_TYPE),
          input.store.readEntities(scope, AGENT_RUN_EVENT_ENTITY_TYPE),
        ]),
        ([runRows, eventRows]) => ({
          status: timelineStatus,
          run: confirmedRun(runRef, runRows),
          events: confirmedEvents(runRef, eventRows),
        }),
      )
    },
  }
}
