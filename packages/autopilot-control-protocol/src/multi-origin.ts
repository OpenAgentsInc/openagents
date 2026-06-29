import type { SessionSummary } from "./control.js"

export type SessionOrigin = "local" | "bridge" | "cloud"

export type OriginTaggedSession = SessionSummary & { origin: SessionOrigin }

export type MergeSessionsInput = {
  local: SessionSummary[]
  bridge: SessionSummary[]
  cloud: SessionSummary[]
}

type OrderedSession = OriginTaggedSession & { originalIndex: number }

export const mergeSessions = (input: MergeSessionsInput): OriginTaggedSession[] => {
  const ordered: OrderedSession[] = []

  const append = (origin: SessionOrigin, sessions: SessionSummary[]) => {
    for (const session of sessions) {
      ordered.push({ ...session, origin, originalIndex: ordered.length })
    }
  }

  append("local", input.local)
  append("bridge", input.bridge)
  append("cloud", input.cloud)

  const bySessionRef = new Map<string, OrderedSession>()
  for (const session of ordered) {
    if (!bySessionRef.has(session.sessionRef)) bySessionRef.set(session.sessionRef, session)
  }

  return [...bySessionRef.values()]
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt) || a.originalIndex - b.originalIndex)
    .map(({ originalIndex: _originalIndex, ...session }) => session)
}
