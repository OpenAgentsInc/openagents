export type AccountCapacityBarTone = "ok" | "warn" | "exhausted"

export function capacityBar(input: {
  usedPct: number | null
  exhausted: boolean
}): {
  pct: number
  tone: AccountCapacityBarTone
  label: string
} {
  const pct = input.usedPct === null ? 0 : clampPercent(input.usedPct)

  if (input.exhausted) {
    return {
      pct,
      tone: "exhausted",
      label: "exhausted",
    }
  }

  if (input.usedPct === null) {
    return {
      pct,
      tone: "ok",
      label: "unknown",
    }
  }

  return {
    pct,
    tone: pct >= 85 ? "warn" : "ok",
    label: `${pct}%`,
  }
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(100, Math.max(0, value))
}
