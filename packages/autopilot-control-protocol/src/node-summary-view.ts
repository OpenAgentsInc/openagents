export function nodeSummary(input: {
  nodeName: string | null
  sessions: { state: string }[]
  accountsReady: number
  accountsTotal: number
  balanceSats: number | null
}): { title: string; lines: string[] } {
  const totalSessions = input.sessions.length
  const runningSessions = input.sessions.filter(session =>
    session.state.trim().toLowerCase() === "running"
  ).length
  const accountsTotal = nonNegativeInteger(input.accountsTotal)
  const accountsReady = Math.min(nonNegativeInteger(input.accountsReady), accountsTotal)

  return {
    title: titleForNode(input.nodeName),
    lines: [
      `sessions: ${runningSessions}/${totalSessions} running`,
      `accounts: ${accountsReady}/${accountsTotal} ready`,
      `balance: ${balanceLabel(input.balanceSats)}`,
    ],
  }
}

function titleForNode(nodeName: string | null): string {
  if (nodeName === null) return "Autopilot node"

  const normalized = nodeName.replace(/\s+/g, " ").trim()
  return normalized === "" ? "Autopilot node" : normalized
}

function balanceLabel(balanceSats: number | null): string {
  if (balanceSats === null) return "unknown"

  return `${nonNegativeInteger(balanceSats)} sats`
}

function nonNegativeInteger(value: number): number {
  if (!Number.isFinite(value)) return 0

  return Math.max(0, Math.trunc(value))
}
