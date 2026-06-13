import {
  acceptEvent,
  initialCursor,
  parseEventBatch,
  parseListResponse,
  type SessionEvent,
  type SessionSummary,
  type StreamCursor,
} from "@openagentsinc/autopilot-control-protocol"

import {
  sessionRowsViewModel,
  sessionTimelineRowsViewModel,
  type SessionRowViewModel,
  type SessionTimelineRowViewModel,
} from "./session-view-model"

type SessionSubscriptionEntry = {
  summary: SessionSummary | null
  events: SessionEvent[]
  cursor: StreamCursor
}

export type SessionSubscriptionRows = {
  sessions: SessionRowViewModel[]
  timelinesBySessionRef: Record<string, SessionTimelineRowViewModel[]>
}

export type SessionSubscription = {
  applyList(rawListResponse: unknown): SessionSummary[]
  applyEventBatch(rawBatch: unknown): SessionEvent[]
  selectRows(): SessionSubscriptionRows
}

export function createSessionSubscription(): SessionSubscription {
  const sessions = new Map<string, SessionSubscriptionEntry>()

  function ensureEntry(sessionRef: string): SessionSubscriptionEntry {
    const existing = sessions.get(sessionRef)
    if (existing !== undefined) return existing

    const entry = {
      summary: null,
      events: [],
      cursor: initialCursor(),
    }
    sessions.set(sessionRef, entry)
    return entry
  }

  return {
    applyList(rawListResponse) {
      const summaries = parseListResponse(rawListResponse)

      for (const summary of summaries) {
        const entry = ensureEntry(summary.sessionRef)
        entry.summary = summary
      }

      return summaries
    },

    applyEventBatch(rawBatch) {
      const acceptedEvents: SessionEvent[] = []
      const orderedEvents = [...parseEventBatch(rawBatch)].sort((left, right) => left.sequence - right.sequence)

      for (const event of orderedEvents) {
        const entry = ensureEntry(event.sessionRef)
        const result = acceptEvent(entry.cursor, event)
        entry.cursor = result.cursor

        if (result.accepted) {
          entry.events.push(event)
          acceptedEvents.push(event)
        }
      }

      return acceptedEvents
    },

    selectRows() {
      const summaries = [...sessions.values()].flatMap((entry) => entry.summary === null ? [] : [entry.summary])
      const timelinesBySessionRef = Object.fromEntries(
        [...sessions.entries()].map(([sessionRef, entry]) => [
          sessionRef,
          sessionTimelineRowsViewModel(entry.events),
        ]),
      )

      return {
        sessions: sessionRowsViewModel(summaries),
        timelinesBySessionRef,
      }
    },
  }
}
