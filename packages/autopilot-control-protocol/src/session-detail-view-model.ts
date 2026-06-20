import { reduceSessionState } from "./session-state-reducer.js"

export type SessionDetailInput = {
  sessionRef: string
  events: { phase: string; messageText: string; observedAt: string }[]
  artifact: { kind: string; outcome: string | null } | null
}

export type SessionDetailViewModel = {
  sessionRef: string
  state: string
  eventCount: number
  lastActivity: string
  hasArtifact: boolean
  outcome: string | null
}

export function buildSessionDetail(input: SessionDetailInput): SessionDetailViewModel {
  const { state } = reduceSessionState(input.events)
  const lastEvent = input.events.at(-1)

  return {
    sessionRef: input.sessionRef,
    state,
    eventCount: input.events.length,
    lastActivity: lastEvent?.observedAt ?? "",
    hasArtifact: input.artifact !== null,
    outcome: input.artifact?.outcome ?? null,
  }
}
