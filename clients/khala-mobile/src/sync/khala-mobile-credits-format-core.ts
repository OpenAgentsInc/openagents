/**
 * MM-D3 (#8480): pure formatting helpers for the mobile Credits balance +
 * transaction history UI. Deliberately takes USD cents (never msat) — see
 * the header comment in `khala-mobile-credits-api.ts` for why the server
 * contract this issue proposes hands the client cents, not the ledger's
 * internal msat unit.
 */

export const formatUsdCents = (cents: number): string => {
  const dollars = cents / 100
  const sign = dollars < 0 ? "-" : ""
  return `${sign}$${Math.abs(dollars).toFixed(2)}`
}

/** A conservative low-balance threshold ($0.50) below which the UI should
 * surface the "buy more" affordance — chosen so a single small coding turn
 * doesn't silently strand the user with an unusable sliver of credit. */
export const LOW_BALANCE_THRESHOLD_CENTS = 50

export const isLowBalance = (balanceUsdCents: number): boolean =>
  balanceUsdCents < LOW_BALANCE_THRESHOLD_CENTS

export type KhalaMobileCreditsTransactionKind = "grant" | "purchase" | "charge" | "other"

export const transactionKindLabel = (kind: KhalaMobileCreditsTransactionKind): string => {
  switch (kind) {
    case "grant":
      return "Free credit"
    case "purchase":
      return "Purchase"
    case "charge":
      return "Usage"
    case "other":
      return "Other"
  }
}

/** Grants/purchases are positive (credit added); charges are negative (credit
 * spent) — this is a display convention only, independent of how the server
 * stores the underlying signed amount. */
export const signedAmountLabel = (kind: KhalaMobileCreditsTransactionKind, amountUsdCents: number): string => {
  const magnitude = Math.abs(amountUsdCents)
  const sign = kind === "charge" ? "-" : "+"
  return `${sign}${formatUsdCents(magnitude)}`
}
