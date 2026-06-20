import { projectEarnings } from "./earnings-view.js"

export type EarningsBalanceView = {
  balanceSats: number
  pendingSats: number
  lifetimeSats: number
  currency: "sats"
  readOnly: true
}

export function projectBalance(raw: unknown): EarningsBalanceView {
  const earnings = projectEarnings(raw)

  return {
    balanceSats: coerceSats(earnings.balanceSats),
    pendingSats: coerceSats(earnings.pendingSats),
    lifetimeSats: coerceSats(earnings.lifetimeSats),
    currency: "sats",
    readOnly: true,
  }
}

function coerceSats(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 0

  return Math.max(0, Math.trunc(value))
}
