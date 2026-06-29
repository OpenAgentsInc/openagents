type DecisionFeedEvent = {
  phase: string
  messageText: string
}

type DecisionFeedSession = {
  sessionRef: string
  state: string
  latestActivity?: string
  events?: DecisionFeedEvent[]
}

type PendingDecision = {
  sessionRef: string
  prompt: string
}

type DecisionFeedView = {
  pending: PendingDecision[]
  count: number
}

type RawRecord = Record<string, unknown>

const PENDING_DECISION_PHASES = new Set(["needs_decision", "needs_approval", "awaiting_input"])

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined
}

function isPendingDecisionPhase(value: unknown): boolean {
  const phase = readString(value)
  return phase !== undefined && PENDING_DECISION_PHASES.has(phase.trim().toLowerCase())
}

function findPendingDecisionEvent(events: unknown): RawRecord | undefined {
  if (!Array.isArray(events)) return undefined

  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index]
    if (!isRecord(event)) continue
    if (isPendingDecisionPhase(event.phase)) return event
  }

  return undefined
}

function projectPendingDecision(session: RawRecord): PendingDecision | undefined {
  const sessionRef = readString(session.sessionRef)
  if (sessionRef === undefined) return undefined

  const pendingEvent = findPendingDecisionEvent(session.events)
  if (pendingEvent === undefined) return undefined

  return {
    sessionRef,
    prompt: readString(pendingEvent.messageText) ?? "",
  }
}

export function projectDecisionFeed(sessions: DecisionFeedSession[]): DecisionFeedView {
  const pending: PendingDecision[] = []

  if (!Array.isArray(sessions)) {
    return { pending, count: 0 }
  }

  for (const session of sessions) {
    if (!isRecord(session)) continue

    const projected = projectPendingDecision(session)
    if (projected === undefined) continue

    pending.push(projected)
  }

  return {
    pending,
    count: pending.length,
  }
}
