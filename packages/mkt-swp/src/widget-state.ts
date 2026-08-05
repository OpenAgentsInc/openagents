/**
 * The explicit typed widget state (SWAP-0, openagents#9315; teardown §2.1-§2.2).
 *
 * Boltz's create form is the cross-product of ~14 reactive signals collapsed
 * only inside the primary button's label effect. This union is the
 * deliberate opposite: one discriminated state a behaviour contract can be
 * written against, exhaustive over the 27 enumerated pre-creation, in-flight,
 * and reported outcome states.
 *
 * The refusal *reasons* are not restated here. Asset and amount refusals come
 * from SWAP-1's `primaryActionGate`, destination refusals from SWAP-2's entry
 * state, the funding refusal from SWAP-3's `fundingGate`, and session
 * progression from SWAP-6's `classifySwpState`. This module owns the state
 * identity, the derivation precedence, and the totality of the fold.
 */
import { Data, Schema } from "effect";
import type { FundingGate } from "@openagentsinc/mkt-swp-compare";
import { SwpErrorIdentifierSchema, FundingAuthorizationSchema } from "./swap-engine.js";
import type { FundingAuthorization } from "./swap-engine.js";

/**
 * The NIP-MKT base `state` vocabulary, restated as a local literal union so
 * the presentation graph (this module, `compose.ts`, `primary-action.ts`)
 * never imports the relay stack. `widget-host.ts` — which does import it —
 * carries a compile-time assertion that this union and `nip-mkt`'s
 * `StatusState` are exactly the same set, so the restatement cannot drift.
 */
export const SESSION_BASE_STATES = [
  "accepted",
  "rejected",
  "awaiting_input",
  "funding_required",
  "funding_observed",
  "executing",
  "settlement_pending",
  "completed",
  "refund_pending",
  "refunded",
  "disputed",
  "failed",
] as const;
export type SessionBaseState = (typeof SESSION_BASE_STATES)[number];
export type SessionProgressState = SessionBaseState | "unresolved";

export const SwapWidgetStateSchema = Schema.TaggedUnion({
  /** 1. The relay/network is unreachable. Retryable, never permanent. */
  Offline: {},
  /** 2. The wasm engine binding is still loading. */
  EngineLoading: {},
  /** 3. The engine did not load; nothing downstream may proceed. */
  EngineFailed: { reason: Schema.Literals(["unavailable", "incompatible"]) },
  /** 4. Offering discovery in flight. */
  PairsLoading: {},
  /** 5. No Offering corpus at all (no provider is advertising). */
  NoOfferings: {},
  /** 6. The selected direction is unreachable (SWAP-1 folds availability). */
  UnsupportedDirection: { identifier: SwpErrorIdentifierSchema },
  /** 7. No amount entered yet. */
  Empty: {},
  /** 8. Entered text is not a canonical amount. */
  AmountUnparseable: {},
  /** 9. Below the Offering minimum for this side. */
  BelowMinimum: { minimumSats: Schema.String },
  /** 10. Above the Offering maximum for this side. */
  AboveMaximum: { maximumSats: Schema.String },
  /** 11. No provider serves this amount, though it is within global bounds. */
  CoverageGap: { nearestBelowSats: Schema.String, nearestAboveSats: Schema.String },
  /** 12. Positive input, zero output: fees exceed the input. */
  ZeroOutput: {},
  /** 13. Quotes are being requested or refreshed. */
  QuoteRefreshing: {},
  /** 14. No quote answered, or the quote's terms were refused. */
  QuoteFailed: { identifier: SwpErrorIdentifierSchema },
  /** 15. The held quote passed its expiration; refused locally. */
  QuoteExpired: {},
  /** 16. No destination entered. */
  NoDestination: {},
  /** 17. The destination failed SWAP-2's parse or local verification. */
  InvalidDestination: { identifier: SwpErrorIdentifierSchema },
  /** 18. Verify-before-fund rows outstanding (SWAP-3 checklist incomplete). */
  VerificationPending: {},
  /** 19. A verify-before-fund row failed; individually identifiable. */
  VerificationFailed: { identifier: SwpErrorIdentifierSchema },
  /** 20. Everything passed: the one enabled pre-order state. */
  Ready: {},
  /** 21. Order submission in flight. Blocked without a fresh explanation. */
  Ordering: {},
  /**
   * 22. Funding is authorised. Reachable only with an engine-issued
   * `FundingAuthorization`, so the fund action cannot be enabled while any
   * verify-before-fund check is unresolved or failed
   * (`swp_funding_not_authorized`).
   */
  AwaitingFunding: { authorization: FundingAuthorizationSchema },
  /** 23. Funding observed on the rail. */
  FundingObserved: {},
  /** 24. Both legs executing. */
  Executing: {},
  /** 25. Settlement pending on one leg. */
  SettlementPending: {},
  /** 26a. A Status reports completed; SWAP-6 Close projection owns terminality. */
  Completed: {},
  /** 26b. Refund prepared or pending. */
  RefundPending: {},
  /** 26c. A Status reports refunded; SWAP-6 Close projection owns terminality. */
  Refunded: {},
  /** 26d. Disputed. */
  Disputed: {},
  /** 27a. A Status reports failed; loss accounting decides watch terminality. */
  Failed: { identifier: Schema.optionalKey(SwpErrorIdentifierSchema) },
  /** 27b. Principal disposition unknown (§9.1); watching must continue. */
  Unresolved: {},
});
export type SwapWidgetState = typeof SwapWidgetStateSchema.Type;
export type SwapWidgetStateTag = SwapWidgetState["_tag"];

/** The exhaustive tag set, in derivation precedence / lifecycle order. */
export const SWAP_WIDGET_STATE_TAGS = [
  "Offline",
  "EngineLoading",
  "EngineFailed",
  "PairsLoading",
  "NoOfferings",
  "UnsupportedDirection",
  "Empty",
  "AmountUnparseable",
  "BelowMinimum",
  "AboveMaximum",
  "CoverageGap",
  "ZeroOutput",
  "QuoteRefreshing",
  "QuoteFailed",
  "QuoteExpired",
  "NoDestination",
  "InvalidDestination",
  "VerificationPending",
  "VerificationFailed",
  "Ready",
  "Ordering",
  "AwaitingFunding",
  "FundingObserved",
  "Executing",
  "SettlementPending",
  "Completed",
  "RefundPending",
  "Refunded",
  "Disputed",
  "Failed",
  "Unresolved",
] as const satisfies ReadonlyArray<SwapWidgetStateTag>;

const formPhaseTags: ReadonlySet<SwapWidgetStateTag> = new Set([
  "Offline",
  "EngineLoading",
  "EngineFailed",
  "PairsLoading",
  "NoOfferings",
  "UnsupportedDirection",
  "Empty",
  "AmountUnparseable",
  "BelowMinimum",
  "AboveMaximum",
  "CoverageGap",
  "ZeroOutput",
  "QuoteRefreshing",
  "QuoteFailed",
  "QuoteExpired",
  "NoDestination",
  "InvalidDestination",
  "VerificationPending",
  "VerificationFailed",
  "Ready",
]);

/** Pre-order states, re-derivable from the composed sibling gates. */
export const isFormPhase = (state: SwapWidgetState): boolean => formPhaseTags.has(state._tag);

export const initialSwapWidgetState: SwapWidgetState =
  SwapWidgetStateSchema.cases.EngineLoading.make({});

/**
 * Widget events. `FormRederived` carries the state SWAP-1/2/3's gates
 * compose to (see `compose.ts`); everything after ordering advances only on
 * engine results and the folded per-signer session lifecycle. Unknown
 * `swp_state` values never become events: SWAP-6's `classifySwpState`
 * refuses them upstream, so they advance nothing.
 */
export type SwapWidgetEvent = Data.TaggedEnum<{
  FormRederived: { readonly state: SwapWidgetState };
  SubmitPressed: Record<never, never>;
  FundingGateChanged: { readonly gate: FundingGate };
  EngineRefused: { readonly identifier: SwpErrorIdentifier };
  FundingAuthorized: { readonly authorization: FundingAuthorization };
  SessionAdvanced: { readonly state: SessionProgressState };
}>;
export const SwapWidgetEvent = Data.taggedEnum<SwapWidgetEvent>();

type SwpErrorIdentifier = typeof SwpErrorIdentifierSchema.Type;

const sessionRank: Partial<Record<SwapWidgetStateTag, number>> = {
  AwaitingFunding: 0,
  FundingObserved: 1,
  Executing: 2,
  SettlementPending: 3,
  RefundPending: 3,
  Disputed: 3,
  Completed: 4,
  Refunded: 4,
  Failed: 4,
  Unresolved: 4,
};

const cases = SwapWidgetStateSchema.cases;

const sessionTarget = (state: SessionProgressState): SwapWidgetState | undefined => {
  switch (state) {
    case "funding_observed":
      return cases.FundingObserved.make({});
    case "executing":
      return cases.Executing.make({});
    case "settlement_pending":
      return cases.SettlementPending.make({});
    case "completed":
      return cases.Completed.make({});
    case "refund_pending":
      return cases.RefundPending.make({});
    case "refunded":
      return cases.Refunded.make({});
    case "disputed":
      return cases.Disputed.make({});
    case "unresolved":
      return cases.Unresolved.make({});
    case "failed":
    case "rejected":
      return cases.Failed.make({});
    // `accepted`, `awaiting_input`, and `funding_required` do not move the
    // widget: funding readiness is established by `FundingAuthorized` only,
    // so a provider claiming `funding_required` cannot open the fund action.
    default:
      return undefined;
  }
};

/**
 * Fold one event into the widget state. Total: every state/event pair
 * returns a state and never throws. Guards are monotone along the ordinary
 * session path, while later disputed/unresolved evidence may replace a
 * reported outcome. A session state never re-enters the form phase, and
 * `AwaitingFunding` is reachable only with an engine-issued authorization.
 * Stop-watching is deliberately absent: SWAP-6's `SwapProgressView` is the
 * only authority for that decision.
 */
export const transitionSwapWidgetState = (
  state: SwapWidgetState,
  event: SwapWidgetEvent,
): SwapWidgetState => {
  return SwapWidgetEvent.$match(event, {
    FormRederived: ({ state: next }) => (isFormPhase(state) ? next : state),
    SubmitPressed: () => (state._tag === "Ready" ? cases.Ordering.make({}) : state),
    FundingGateChanged: ({ gate }) => {
      if (!isFormPhase(state)) return state;
      if (gate.enabled) {
        return state._tag === "VerificationPending" || state._tag === "VerificationFailed"
          ? cases.Ready.make({})
          : state;
      }
      const failed = gate.failed.at(0);
      if (failed !== undefined) {
        return cases.VerificationFailed.make({ identifier: failed.error });
      }
      // Unresolved rows, a missing report, or a stale epoch are pending, not
      // a refusal — and still short of funding either way.
      return state._tag === "Ready" ? cases.VerificationPending.make({}) : state;
    },
    EngineRefused: ({ identifier }) =>
      state._tag === "Ordering" || isFormPhase(state)
        ? cases.VerificationFailed.make({ identifier })
        : state,
    FundingAuthorized: ({ authorization }) =>
      state._tag === "Ordering" ? cases.AwaitingFunding.make({ authorization }) : state,
    SessionAdvanced: ({ state: sessionState }) => {
      const currentRank = sessionRank[state._tag];
      if (currentRank === undefined) return state;
      const target = sessionTarget(sessionState);
      if (target === undefined) return state;
      const targetRank = sessionRank[target._tag];
      if (
        (target._tag === "Disputed" || target._tag === "Unresolved") &&
        state._tag !== target._tag &&
        currentRank >= 1
      ) {
        return target;
      }
      return targetRank !== undefined && targetRank > currentRank ? target : state;
    },
  });
};
