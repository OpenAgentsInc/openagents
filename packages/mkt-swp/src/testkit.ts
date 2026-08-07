/**
 * Sample states and inputs shared by the SWAP-0 suites. Exported so a
 * downstream surface can drive its own rendering against the exhaustive
 * state set rather than inventing a partial one.
 */
import type { FundingGate, VerifyCheckRow } from "@openagentsinc/mkt-swp-compare";
import { ImmortalFundingRequestSchema } from "./immortal-browser-abi.js";
import { SWAP_WIDGET_STATE_TAGS, SwapWidgetStateSchema } from "./widget-state.js";
import type { SwapWidgetState, SwapWidgetStateTag } from "./widget-state.js";

const cases = SwapWidgetStateSchema.cases;

export const sampleFundingRequest = ImmortalFundingRequestSchema.make({
  session_id: "1f".repeat(32),
  order_id: "ab".repeat(32),
  quote_id: "cd".repeat(32),
  swap_type: "submarine",
  action: {
    action: "broadcast_bitcoin",
    effect_id: "ef".repeat(32),
    leg_id: "source",
    raw_transaction: "00",
  },
});

/** One sample per state tag: the exhaustive iteration base for the suites. */
export const sampleWidgetStates: Record<SwapWidgetStateTag, SwapWidgetState> = {
  Offline: cases.Offline.make({}),
  EngineLoading: cases.EngineLoading.make({}),
  EngineFailed: cases.EngineFailed.make({ reason: "unavailable" }),
  PairsLoading: cases.PairsLoading.make({}),
  NoOfferings: cases.NoOfferings.make({}),
  UnsupportedDirection: cases.UnsupportedDirection.make({ identifier: "swp_side_disabled" }),
  Empty: cases.Empty.make({}),
  AmountUnparseable: cases.AmountUnparseable.make({}),
  BelowMinimum: cases.BelowMinimum.make({ minimumSats: "10000" }),
  AboveMaximum: cases.AboveMaximum.make({ maximumSats: "100000000" }),
  CoverageGap: cases.CoverageGap.make({
    nearestBelowSats: "50000",
    nearestAboveSats: "200000",
  }),
  ZeroOutput: cases.ZeroOutput.make({}),
  QuoteRefreshing: cases.QuoteRefreshing.make({}),
  QuoteFailed: cases.QuoteFailed.make({ identifier: "swp_terms_mismatch" }),
  QuoteExpired: cases.QuoteExpired.make({}),
  NoDestination: cases.NoDestination.make({}),
  InvalidDestination: cases.InvalidDestination.make({ identifier: "swp_invoice_invalid" }),
  VerificationPending: cases.VerificationPending.make({}),
  VerificationFailed: cases.VerificationFailed.make({ identifier: "swp_script_invalid" }),
  Ready: cases.Ready.make({}),
  Ordering: cases.Ordering.make({}),
  AwaitingFunding: cases.AwaitingFunding.make({ fundingRequest: sampleFundingRequest }),
  FundingObserved: cases.FundingObserved.make({}),
  Executing: cases.Executing.make({}),
  SettlementPending: cases.SettlementPending.make({}),
  Completed: cases.Completed.make({}),
  RefundPending: cases.RefundPending.make({}),
  Refunded: cases.Refunded.make({}),
  Disputed: cases.Disputed.make({}),
  Failed: cases.Failed.make({ identifier: "swp_refund_failed" }),
  Unresolved: cases.Unresolved.make({}),
};

export const everySampleWidgetState = (): ReadonlyArray<SwapWidgetState> =>
  SWAP_WIDGET_STATE_TAGS.map((tag) => sampleWidgetStates[tag]);

/** An all-pass SWAP-3 gate for the current epoch. */
export const enabledFundingGate = (reportEpoch = 1): FundingGate => ({
  enabled: true,
  reportEpoch,
});

/** A SWAP-3 gate blocked by one named failed row. */
export const failedFundingGate = (
  row: Extract<VerifyCheckRow, { status: "fail" }>,
): FundingGate => ({
  enabled: false,
  error: "swp_funding_not_authorized",
  reason: "rows_incomplete",
  unresolved: [],
  failed: [row],
});

/** A SWAP-3 gate blocked only by unresolved rows. */
export const pendingFundingGate = (): FundingGate => ({
  enabled: false,
  error: "swp_funding_not_authorized",
  reason: "rows_incomplete",
  unresolved: ["timeout_ladder"],
  failed: [],
});
