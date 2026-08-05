// The English source catalog. English is the source language: its key set is
// the typed key set every call site checks against, and every other locale
// back-fills from it at load, so a partial locale can never render blank.
//
// Parameterised messages are functions with typed parameters. The
// primary-action refusal law (SWAP-0) requires a refusal to state the limit
// in the user's current units, so the amount refusals carry the limit and the
// denomination as data — never pre-baked into the string.

import { swapErrorMessages } from "./error-table.js";

export interface MinimumAmountParams {
  /** The limit, already formatted in the user's current display unit. */
  readonly minimum: string;
  /** The user's current display unit, e.g. "sats" or "BTC". */
  readonly denomination: string;
}

export interface MaximumAmountParams {
  readonly maximum: string;
  readonly denomination: string;
}

export interface AmountRangeParams {
  readonly minimum: string;
  readonly maximum: string;
  readonly denomination: string;
}

export const en = {
  ...swapErrorMessages,
  "swap.refusal.below_minimum": (params: MinimumAmountParams) =>
    `Below the minimum: enter at least ${params.minimum} ${params.denomination}.`,
  "swap.refusal.above_maximum": (params: MaximumAmountParams) =>
    `Above the maximum: enter at most ${params.maximum} ${params.denomination}.`,
  "swap.refusal.amount_range": (params: AmountRangeParams) =>
    `Enter an amount between ${params.minimum} and ${params.maximum} ${params.denomination}.`,
} as const;
