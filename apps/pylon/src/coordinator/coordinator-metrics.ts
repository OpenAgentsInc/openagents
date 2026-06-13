export type CoordinatorMetricsInput = {
  intents: any[]
  sessions: any[]
}

export type CoordinatorMetrics = {
  intentsByStatus: Record<string, number>
  sessionsByState: Record<string, number>
  activeAgents: number
  totalIntents: number
}

const ACTIVE_SESSION_STATES = new Set(["active", "running"])

function countBy(items: unknown, key: string): Record<string, number> {
  const counts: Record<string, number> = {}
  if (!Array.isArray(items)) return counts

  for (const item of items) {
    if (item === null || typeof item !== "object") continue
    const value = (item as Record<string, unknown>)[key]
    if (typeof value !== "string" || value.length === 0) continue
    counts[value] = (counts[value] ?? 0) + 1
  }

  return counts
}

function isActiveSession(session: unknown): session is Record<string, unknown> {
  if (session === null || typeof session !== "object") return false
  const state = (session as Record<string, unknown>).state
  return typeof state === "string" && ACTIVE_SESSION_STATES.has(state)
}

function agentRefFor(session: Record<string, unknown>): string | null {
  const agentRef = session.agentRef
  if (typeof agentRef === "string" && agentRef.length > 0) return agentRef

  const agentId = session.agentId
  if (typeof agentId === "string" && agentId.length > 0) return agentId

  return null
}

export function computeCoordinatorMetrics(input: CoordinatorMetricsInput): CoordinatorMetrics {
  const intents = Array.isArray(input?.intents) ? input.intents : []
  const sessions = Array.isArray(input?.sessions) ? input.sessions : []
  const activeAgentRefs = new Set<string>()
  let activeSessionsWithoutAgentRef = 0

  for (const session of sessions) {
    if (!isActiveSession(session)) continue
    const agentRef = agentRefFor(session)
    if (agentRef === null) {
      activeSessionsWithoutAgentRef += 1
    } else {
      activeAgentRefs.add(agentRef)
    }
  }

  return {
    intentsByStatus: countBy(intents, "status"),
    sessionsByState: countBy(sessions, "state"),
    activeAgents: activeAgentRefs.size + activeSessionsWithoutAgentRef,
    totalIntents: intents.length,
  }
}
