/**
 * The composition seam (SWAP-0, openagents#9315).
 *
 * Every sibling package answers its own question and stops. SWAP-1 says
 * whether the pair and amount can proceed, SWAP-2 whether the destination is
 * bound and locally verified, SWAP-3 whether the verify-before-fund
 * checklist authorises funding. Nobody owns the order those answers are
 * asked in — and that ordering *is* the widget's behaviour, because the
 * primary-action law says exactly one refusal, the most proximate one, is
 * stated at a time.
 *
 * This module is that ordering, written once, as a stated contract rather
 * than an emergent property of signal evaluation. It re-derives no refusal
 * of its own: each branch reads a sibling's typed verdict and names the
 * widget state it maps to.
 */
import type { FundingGate } from "@openagentsinc/mkt-swp-compare";
import type { DestinationEntryState } from "@openagentsinc/mkt-swp-destination";
import type { PrimaryActionGate } from "@openagentsinc/mkt-swp-pair";
import { SwapWidgetStateSchema } from "./widget-state.js";
import type { SwapWidgetState } from "./widget-state.js";

const cases = SwapWidgetStateSchema.cases;

/** Engine lifecycle as the host observes it through the binding. */
export type EngineStatus =
  | { readonly status: "loading" }
  | { readonly status: "failed"; readonly reason: "unavailable" | "incompatible" }
  | { readonly status: "ready" };

/** Quote acquisition as SWAP-3's compare surface reports it. */
export type QuoteStatus =
  | { readonly status: "idle" }
  | { readonly status: "refreshing" }
  | { readonly status: "selected" }
  | { readonly status: "none_answered" }
  | { readonly status: "expired" };

/**
 * Everything the widget needs to name its pre-order state, all of it a
 * sibling's output. `pairGate` is `null` before the Offering corpus loads.
 */
export interface WidgetInputs {
  readonly online: boolean;
  readonly engine: EngineStatus;
  readonly pairsLoaded: boolean;
  readonly pairGate: PrimaryActionGate | null;
  readonly outputAmountSats: bigint | null;
  readonly quote: QuoteStatus;
  readonly destination: DestinationEntryState | null;
  readonly fundingGate: FundingGate | null;
}

/**
 * Map SWAP-1's refusal to its widget state. The identifiers are the ones
 * SWAP-1 already attached; none is invented here.
 */
const fromPairRefusal = (gate: Extract<PrimaryActionGate, { enabled: false }>): SwapWidgetState => {
  const refusal = gate.refusal;
  switch (refusal.kind) {
    case "empty_corpus":
      return cases.NoOfferings.make({});
    case "direction_unselected":
    case "direction_unsupported":
      return cases.UnsupportedDirection.make({ identifier: "swp_invalid_pair" });
    case "direction_unreachable":
      return cases.UnsupportedDirection.make({ identifier: refusal.swpError });
    case "amount_missing":
      return cases.Empty.make({});
    case "amount_unparseable":
      return cases.AmountUnparseable.make({});
    case "below_minimum":
      return cases.BelowMinimum.make({ minimumSats: refusal.minimumSats.toString() });
    case "above_maximum":
      return cases.AboveMaximum.make({ maximumSats: refusal.maximumSats.toString() });
    case "coverage_gap":
      return cases.CoverageGap.make({
        nearestBelowSats: refusal.nearestBelowSats.toString(),
        nearestAboveSats: refusal.nearestAboveSats.toString(),
      });
    case "quote_terms_refused":
      return cases.QuoteFailed.make({ identifier: refusal.refusal.error });
    case "quote_input_unserviceable":
      return cases.QuoteFailed.make({ identifier: refusal.swpError });
    case "price_feed_unchecked":
      // The pinned feed still needs the requester's own fetch (§3.4).
      return cases.VerificationPending.make({});
    case "price_feed_refused":
      return cases.QuoteFailed.make({ identifier: refusal.swpError });
  }
};

/** Map SWAP-2's entry state to its widget state, or `null` when it is fine. */
const fromDestination = (entry: DestinationEntryState): SwapWidgetState | null => {
  if (entry.failure !== null) {
    return cases.InvalidDestination.make({ identifier: "swp_invoice_invalid" });
  }
  if (entry.verification.status === "failed") {
    return cases.InvalidDestination.make({ identifier: entry.verification.verdict.error });
  }
  if (entry.bound === null) return cases.NoDestination.make({});
  return null;
};

/**
 * Derive the pre-order widget state. First match wins; this order is the
 * stated refusal precedence (teardown §2.2), so exactly one — the most
 * proximate — reason surfaces at a time.
 */
export const composeWidgetState = (inputs: WidgetInputs): SwapWidgetState => {
  if (!inputs.online) return cases.Offline.make({});
  if (inputs.engine.status === "loading") return cases.EngineLoading.make({});
  if (inputs.engine.status === "failed") {
    return cases.EngineFailed.make({ reason: inputs.engine.reason });
  }
  if (!inputs.pairsLoaded || inputs.pairGate === null) return cases.PairsLoading.make({});
  if (!inputs.pairGate.enabled) return fromPairRefusal(inputs.pairGate);
  if (inputs.outputAmountSats !== null && inputs.outputAmountSats === 0n) {
    return cases.ZeroOutput.make({});
  }
  if (inputs.quote.status === "refreshing") return cases.QuoteRefreshing.make({});
  if (inputs.quote.status === "none_answered") {
    return cases.QuoteFailed.make({ identifier: "swp_terms_mismatch" });
  }
  if (inputs.quote.status === "expired") return cases.QuoteExpired.make({});
  if (inputs.destination === null) return cases.NoDestination.make({});
  const destinationState = fromDestination(inputs.destination);
  if (destinationState !== null) return destinationState;
  if (inputs.quote.status === "idle") return cases.QuoteRefreshing.make({});
  if (inputs.fundingGate === null) return cases.VerificationPending.make({});
  if (!inputs.fundingGate.enabled) {
    const failed = inputs.fundingGate.failed.at(0);
    return failed === undefined
      ? cases.VerificationPending.make({})
      : cases.VerificationFailed.make({ identifier: failed.error });
  }
  return cases.Ready.make({});
};
