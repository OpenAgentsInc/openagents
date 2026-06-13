export type EarningsSummary = Readonly<{
  balanceSats: number
  entries: ReadonlyArray<
    Readonly<{
      ref: string
      amountSats: number
      at: string
    }>
  >
}>

export type EarningsEntryViewModel = {
  ref: string
  amountSats: number
  amountLabel: string
  at: string
}

export type EarningsViewModel = {
  balanceSats: number
  balanceLabel: string
  entries: EarningsEntryViewModel[]
}

const satsLabel = (amountSats: number): string => `${amountSats} sats`

export function earningsViewModel(summary: EarningsSummary): EarningsViewModel {
  return {
    balanceSats: summary.balanceSats,
    balanceLabel: satsLabel(summary.balanceSats),
    entries: summary.entries.map((entry) => ({
      ref: entry.ref,
      amountSats: entry.amountSats,
      amountLabel: satsLabel(entry.amountSats),
      at: entry.at,
    })),
  }
}
