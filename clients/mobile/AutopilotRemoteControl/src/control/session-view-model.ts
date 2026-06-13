import type {
  SessionEvent,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"

export type SessionRowViewModel = {
  sessionRef: string
  adapter: SessionSummary["adapter"]
  state: SessionSummary["state"]
  stateClassName: `state-${SessionSummary["state"]}`
  lastProgressRef: string
}

export type SessionTimelineRowViewModel = {
  eventId: string
  sessionRef: string
  sequence: number
  phase: SessionEvent["phase"]
  phaseLabel: string
  projectionLevel: SessionEvent["projectionLevel"]
  observedAt: string
  detailRef: string
}

export type NodeStatusViewModel = {
  status: "connected" | "offline"
  sessionCount: number
  text: string
}

export function sessionRowsViewModel(sessions: SessionSummary[]): SessionRowViewModel[] {
  return sessions.map((session) => ({
    sessionRef: session.sessionRef,
    adapter: session.adapter,
    state: session.state,
    stateClassName: `state-${session.state}`,
    lastProgressRef: session.lastProgressRef ?? "none",
  }))
}

export function sessionTimelineRowsViewModel(events: SessionEvent[]): SessionTimelineRowViewModel[] {
  return [...events]
    .sort((left, right) => left.sequence - right.sequence)
    .map((event) => ({
      eventId: event.eventId,
      sessionRef: event.sessionRef,
      sequence: event.sequence,
      phase: event.phase,
      phaseLabel: event.phase.replaceAll("_", " "),
      projectionLevel: event.projectionLevel,
      observedAt: event.observedAt,
      detailRef: event.detailRef ?? "none",
    }))
}

export function nodeStatusLineViewModel(state: {
  ok: boolean
  sessions: SessionSummary[]
}): NodeStatusViewModel {
  const status = state.ok ? "connected" : "offline"
  const count = state.sessions.length
  const noun = count === 1 ? "session" : "sessions"

  return {
    status,
    sessionCount: count,
    text: `${status} · ${count} ${noun}`,
  }
}
