// Per-call MPP price derivation (EPIC #6049, Phase 2). PURE.
//
// The MPP/x402 flow charges BEFORE the completion runs (the agent pays, then we
// serve), but our metering is receipt-first (we price the real token usage AFTER
// the provider responds). So the 402 challenge quotes a FLAT per-call price — an
// estimate-then-settle — derived from the same per-token pricing model the
// metering hook uses. We model a representative call (a bounded token budget) at
// the model's published sell rate, then clamp to MPP's microtransaction floor
// (0.01 USDC) so even a tiny call is payable.
//
// On settlement the paid amount mints Khala credits; the actual completion is
// then metered receipt-first against that credit by the EXISTING metering hook.
// So the flat quote is an UP-FRONT credit purchase sized to comfortably cover a
// typical call, not a per-token charge — the per-token charge still happens, out
// of the minted credit, exactly as for any other Khala request.

import {
  type FundingKind,
  DEFAULT_MARGIN,
  priceRequest,
} from '../pricing'

// MPP / Stripe microtransaction floor: individual charges can be as low as
// 0.01 USDC (Stripe machine-payments docs). We never quote below this.
export const MPP_MIN_USDC = 0.01

// Stripe requires a minimum charge of 0.50 USD for card payments via SPT
// (Stripe MPP docs). Used for the card challenge floor only; crypto uses
// MPP_MIN_USDC.
export const SPT_MIN_USD = 0.5

// Representative per-call token budget used to size the flat quote. A typical
// single-turn Khala call is on the order of a few hundred prompt tokens and a
// few hundred completion tokens; we size the quote to comfortably cover that so
// the minted credit clears the metered charge with margin to spare. Tunable.
export const REPRESENTATIVE_PROMPT_TOKENS = 600
export const REPRESENTATIVE_COMPLETION_TOKENS = 400

export type MppRail = 'crypto' | 'card'

export type MppCallQuote = Readonly<{
  // The model the quote was priced against (canonical).
  model: string
  // The rail this quote is for (crypto floor 0.01 USDC, card floor 0.50 USD).
  rail: MppRail
  // The flat per-call price in USD (== USDC for the crypto rail). Always >= the
  // rail floor.
  priceUsd: number
  // The price in minor units (cents) — what the Stripe PaymentIntent `amount`
  // wants (USDC has 6 decimals but Stripe's crypto deposit amount is expressed
  // in USD cents in the deposit-mode flow). 0.01 USDC => 1 cent.
  amountCents: number
}>

// Round a USD amount UP to whole cents (never under-quote a charge).
const toCentsCeil = (usd: number): number =>
  Math.max(1, Math.ceil(usd * 100 - 1e-9))

// Derive the flat per-call quote for a model on a rail. PURE. Prices a
// representative call at the published sell rate (card funding — no Bitcoin
// discount on the inbound MPP rail), then clamps to the rail's microtransaction
// floor.
export const quoteMppCall = (
  input: Readonly<{
    model: string
    rail: MppRail
    // Margin override (defaults to the published DEFAULT_MARGIN). Pure knob.
    margin?: number
    // Representative token budget override (tests / tuning).
    promptTokens?: number
    completionTokens?: number
  }>,
): MppCallQuote => {
  // Inbound MPP funds are USDC/card (Stripe-custodied), NOT Bitcoin — so we
  // price at the card funding rate (no Bitcoin funding discount). The Bitcoin
  // discount only applies to the Bitcoin/Spark inbound rail.
  const fundingKind: FundingKind = 'card'

  const priced = priceRequest({
    fundingKind,
    margin: input.margin ?? DEFAULT_MARGIN,
    model: input.model,
    usage: {
      completionTokens:
        input.completionTokens ?? REPRESENTATIVE_COMPLETION_TOKENS,
      promptTokens: input.promptTokens ?? REPRESENTATIVE_PROMPT_TOKENS,
      totalTokens:
        (input.promptTokens ?? REPRESENTATIVE_PROMPT_TOKENS) +
        (input.completionTokens ?? REPRESENTATIVE_COMPLETION_TOKENS),
    },
  })

  const floor = input.rail === 'card' ? SPT_MIN_USD : MPP_MIN_USDC
  const priceUsd = Math.max(floor, priced.chargeUsd)

  return {
    amountCents: toCentsCeil(priceUsd),
    model: priced.model,
    priceUsd,
    rail: input.rail,
  }
}
