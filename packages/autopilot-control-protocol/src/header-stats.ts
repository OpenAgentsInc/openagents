import { sessionBadge } from "./session-badge.js"

export type HeaderStatsInput = {
  sessions: { state: string }[]
  pendingDecisions: number
  unreadNotifs: number
}

export type HeaderStatsBadge = {
  label: string
  tone: string
}

export type HeaderStats = {
  running: number
  total: number
  badges: HeaderStatsBadge[]
}

export function headerStats(input: HeaderStatsInput): HeaderStats {
  const sessions = Array.isArray(input.sessions) ? input.sessions : []
  const running = sessions.filter(isRunningSession).length
  const total = sessions.length
  const pendingDecisions = nonNegativeInteger(input.pendingDecisions)
  const unreadNotifs = nonNegativeInteger(input.unreadNotifs)
  const badges: HeaderStatsBadge[] = [
    {
      label: `sessions: ${running}/${total} running`,
      tone: running > 0 ? "running" : "idle",
    },
  ]

  if (pendingDecisions > 0) {
    badges.push({
      label: `decisions: ${pendingDecisions} pending`,
      tone: "warn",
    })
  }

  if (unreadNotifs > 0) {
    badges.push({
      label: `notifications: ${unreadNotifs} unread`,
      tone: "ok",
    })
  }

  return {
    running,
    total,
    badges,
  }
}

function isRunningSession(session: { state: string }): boolean {
  return sessionBadge(session.state.trim().toLowerCase()).tone === "running"
}

function nonNegativeInteger(value: number): number {
  if (!Number.isFinite(value)) return 0

  return Math.max(0, Math.trunc(value))
}
