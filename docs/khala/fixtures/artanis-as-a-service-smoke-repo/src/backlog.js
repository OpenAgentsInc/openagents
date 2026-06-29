export function buildFleetPlan(accounts) {
  const rows = accounts.map((account, index) => ({
    ref: account.ref,
    ordinal: index + 1,
    readiness: account.readiness,
    canRunCodex: account.readiness === "ready",
    riskLevel: account.readiness === "ready" ? "low" : "needs-attention",
  }))

  return {
    rows,
    summary: {
      total: rows.length,
      ready: rows.filter((row) => row.canRunCodex).length,
      needsAttention: rows.filter((row) => !row.canRunCodex).length,
    },
  }
}
