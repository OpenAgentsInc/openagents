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

  // History surface (SWAP-5, openagents#9320). Actions, terminal outcomes
  // (MKT-SWP §15 Close outcomes), and evidence-rung labels (§11 — the rung
  // renders what the evidence proves and is never inferred upward).
  "swap.history.empty": "No swaps on this device yet.",
  "swap.history.action.resume": "Resume",
  "swap.history.action.claim": "Claim funds",
  "swap.history.action.refund": "Refund",
  "swap.history.action.exit": "Finish exit",
  "swap.history.delete_confirm":
    "Delete this swap's local record? Its exit package is removed from this device with it.",
  "swap.history.outcome.completed": "Completed",
  "swap.history.outcome.cancelled": "Cancelled",
  "swap.history.outcome.expired": "Expired",
  "swap.history.outcome.failed": "Failed",
  "swap.history.outcome.refunded": "Refunded",
  "swap.history.outcome.disputed": "Disputed",
  "swap.history.outcome.unresolved": "Unresolved",
  "swap.history.rung.claimed_only": "reported, not verified",
  "swap.history.rung.pledged": "pledged",
  "swap.history.rung.reserved": "reserved",
  "swap.history.rung.measured": "observed",
  "swap.history.rung.verified": "verified",
  "swap.history.rung.paid": "paid",
  "swap.history.rung.settled": "settled",
  "swap.history.reload_guard":
    "A swap has a payment or broadcast in flight. Leaving now is safe for your funds but will interrupt it; it will resume when you return.",

  // Export/import (SWAP-5). The export holds no keys or secrets, but it is
  // the user's complete private financial history; say so at the download.
  "swap.history.export.sensitivity":
    "This file contains your full swap history and exit packages. It holds no keys and cannot spend funds, but it is private financial data — store it like a bank statement.",
  "swap.history.import.refused.not_an_export": "This file is not a swap history export.",
  "swap.history.import.refused.unsupported_version":
    "This export was made by a newer version of the app. Update the app, then import again.",
  "swap.history.import.refused.session_invalid":
    "A record in this file is malformed. Nothing was imported.",
  "swap.history.import.refused.digest_mismatch":
    "A record in this file does not match its integrity digest. Nothing was imported.",
  "swap.history.import.refused.secret_material":
    "This file appears to contain key material, which never belongs in a history export. Nothing was imported.",
  "swap.history.import.refused.conflicting_session":
    "A swap in this file already exists here with different content. Nothing was imported.",

  // Swap widget shell (SWAP-0, openagents#9315). The primary action is always
  // rendered and always states the single most proximate reason it cannot
  // proceed; these are that control's labels. Amount refusals reuse the
  // parameterised `swap.refusal.*` keys above so the limit is stated in the
  // user's current units.
  "swap.widget.offline": "Connection lost. Reconnect to continue.",
  "swap.widget.engine_loading": "Loading the swap engine",
  "swap.widget.engine_failed": "The swap engine did not load. Reload to try again.",
  "swap.widget.pairs_loading": "Loading available pairs",
  "swap.widget.no_offerings": "No providers are offering this pair.",
  "swap.widget.unsupported_direction": "This direction is not available",
  "swap.widget.enter_amount": "Enter an amount",
  "swap.widget.zero_output": "This amount is too small to cover the fees",
  "swap.widget.quote_refreshing": "Getting quotes",
  "swap.widget.quote_failed": "No quote available",
  "swap.widget.quote_expired": "That quote expired. Get a new one.",
  "swap.widget.no_destination": "Enter a destination",
  "swap.widget.invalid_destination": "Check the destination",
  "swap.widget.verification_pending": "Verifying the swap terms",
  "swap.widget.verification_failed": "Verification failed",
  "swap.widget.action.create": "Create swap",
  "swap.widget.action.fund": "Fund swap",
  "swap.widget.ordering": "Creating swap",
  "swap.widget.funding_observed": "Funding seen",
  "swap.widget.executing": "Swap in progress",
  "swap.widget.settlement_pending": "Waiting for settlement",
  "swap.widget.completed": "Swap complete",
  "swap.widget.refund_pending": "Refund in progress",
  "swap.widget.refunded": "Refunded",
  "swap.widget.disputed": "Disputed",
  "swap.widget.failed": "This swap failed",
  "swap.widget.unresolved": "Outcome unresolved",
} as const;
