// USD <-> msat conversion — the SINGLE source of truth (#5497).
//
// Both the inference metering pricing (`metering-hook.ts`) and the USD->msat
// credit bridge (`usd-credit-bridge.ts`) need the same fixed BTC/USD reference
// rate and the same rounding discipline. Before #5497 the rate + conversion
// lived only inside `metering-hook.ts`; centralizing it here means there is ONE
// place the rate is defined and ONE rounding rule, so a card-funded inference
// charge and a card-funded credit grant can never drift apart.
//
// PURE: no D1, no wallet, no clock, no payment material. The `agent_balances`
// ledger is denominated in msat, so any USD-denominated value (a metering
// charge OR a USD credit purchase) must convert through here to touch that
// balance.
//
// !! BILLING TODO: there is no live BTC/USD oracle wired into this Worker yet,
// so this uses a fixed reference rate. Replace `DEFAULT_BTC_USD` with a live
// oracle read (or inject the conversion) before publishing real prices; the
// rate is a single tunable knob and everything that converts re-solves.
export const DEFAULT_BTC_USD = 100_000 as const

const MSAT_PER_BTC = 100_000_000_000 as const

// Round away binary floating-point dust before ceiling/floor so an exact-integer
// charge (e.g. $1 @ $100k/BTC = 1_000_000 msat) is not pushed off by 1 from a
// ...0001 representation error, while a genuinely fractional charge still rounds
// in the intended direction.
const FLOAT_DUST = 1e-6

// Convert a USD charge to integer msat at a given BTC/USD rate, rounding UP so a
// nonzero charge never rounds away to a free request (the ledger CHECK requires
// cost_msat > 0). A zero/negative/non-finite USD charge maps to 0 msat (no row).
// Used by the metering hook (a charge must never be free).
export const usdToMsatCeil = (
  chargeUsd: number,
  btcUsd: number = DEFAULT_BTC_USD,
): number => {
  if (!Number.isFinite(chargeUsd) || chargeUsd <= 0) return 0
  if (!Number.isFinite(btcUsd) || btcUsd <= 0) return 0
  const msat = (chargeUsd / btcUsd) * MSAT_PER_BTC
  return Math.max(1, Math.ceil(msat - FLOAT_DUST))
}

// Convert a USD charge to integer SATOSHIS at a given BTC/USD rate, rounding UP
// (1 sat = 1000 msat). Used by the Lightning MPP rail to size a BOLT11 invoice
// in sats from the same per-call USD quote the other rails price against, at the
// SAME single-source BTC/USD rate. A zero/negative/non-finite charge maps to 0.
export const usdToSatsCeil = (
  chargeUsd: number,
  btcUsd: number = DEFAULT_BTC_USD,
): number => {
  const msat = usdToMsatCeil(chargeUsd, btcUsd)
  return msat <= 0 ? 0 : Math.ceil(msat / 1000)
}

// Convert USD CENTS to integer msat, rounding DOWN so a credit grant never
// over-credits the buyer beyond the dollars they actually paid (the grant must
// be bounded by — and never exceed — the USD debited). A zero/negative/
// non-finite cents value maps to 0 msat (no grant). The bridge uses cents (the
// USD ledger's native unit) directly so the round-trip stays exact at the same
// rate. Used by the USD->msat credit bridge (a grant must never exceed paid USD).
export const usdCentsToMsatFloor = (
  amountCents: number,
  btcUsd: number = DEFAULT_BTC_USD,
): number => {
  if (!Number.isFinite(amountCents) || amountCents <= 0) return 0
  if (!Number.isFinite(btcUsd) || btcUsd <= 0) return 0
  const chargeUsd = amountCents / 100
  const msat = (chargeUsd / btcUsd) * MSAT_PER_BTC
  return Math.max(0, Math.floor(msat + FLOAT_DUST))
}
