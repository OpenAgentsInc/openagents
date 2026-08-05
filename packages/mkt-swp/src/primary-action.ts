/**
 * The primary-action law (SWAP-0, openagents#9315; teardown §2.3):
 *
 *   the primary action is always rendered, always states the single most
 *   proximate reason it cannot proceed, and states it in the user's current
 *   units.
 *
 * Four independent mechanisms, kept separate exactly as the teardown found
 * them separate in the one part of Boltz worth taking outright:
 *
 * 1. LABEL — one typed message key plus its parameters, resolved through
 *    SWAP-8's catalog. Amount refusals use the parameterised
 *    `swap.refusal.*` keys, so the limit is stated in the denomination on
 *    screen; typed §17 identifiers resolve through the SWAP-8 error table.
 *    A counterparty's prose never reaches this string.
 * 2. TONE — a separate decision over a "the user can act on this" set.
 *    Boltz styles the maximum as an error and the minimum not, and neither
 *    destination refusal; that asymmetry is deliberately not reproduced.
 * 3. DISABLED — a separate predicate, not derived from the label, so a
 *    state can be explained without blocking and blocked without a fresh
 *    explanation.
 * 4. BUSY — a spinner replaces the label while work is in flight, and never
 *    for a refusal no wait can resolve.
 */
import { messageForSwpError, render } from "@openagentsinc/swap-i18n";
import type { Catalog, MessageKey, SwpErrorIdentifier } from "@openagentsinc/swap-i18n";
import { denominationLabel, formatAmountText } from "@openagentsinc/mkt-swp-pair";
import type { DecimalSeparator, Denomination } from "@openagentsinc/mkt-swp-pair";
import type { SwapWidgetState, SwapWidgetStateTag } from "./widget-state.js";

export type PrimaryActionTone = "accent" | "danger" | "neutral";

export interface PrimaryActionModel {
  /** Always non-empty: the rendered action or the rendered refusal. */
  readonly label: string;
  /** The typed message key behind the label, for tests and oracles. */
  readonly messageKey: string;
  /** The §17 identifier when the refusal carries one, else null. */
  readonly swpError: SwpErrorIdentifier | null;
  readonly tone: PrimaryActionTone;
  readonly disabled: boolean;
  readonly busy: boolean;
}

/**
 * The only pressable states. `AwaitingFunding` carries an engine-issued
 * `FundingAuthorization` by construction, so the fund action is enabled only
 * after the engine's verdict and every verify-before-fund row passed.
 */
const enabledTags: ReadonlySet<SwapWidgetStateTag> = new Set(["Ready", "AwaitingFunding"]);

const busyTags: ReadonlySet<SwapWidgetStateTag> = new Set([
  "EngineLoading",
  "PairsLoading",
  "QuoteRefreshing",
  "VerificationPending",
  "Ordering",
]);

/**
 * Refusals no retry, wait, or spinner can resolve from the current form.
 * The content mechanism must never spin on these — Boltz's own rule, and
 * the reason its unroutable-pair label is the one that does not spin.
 */
const permanentRefusalTags: ReadonlySet<SwapWidgetStateTag> = new Set([
  "EngineFailed",
  "NoOfferings",
  "UnsupportedDirection",
]);

const dangerTags: ReadonlySet<SwapWidgetStateTag> = new Set([
  "Offline",
  "EngineFailed",
  "NoOfferings",
  "UnsupportedDirection",
  "AmountUnparseable",
  "BelowMinimum",
  "AboveMaximum",
  "CoverageGap",
  "QuoteFailed",
  "QuoteExpired",
  "InvalidDestination",
  "VerificationFailed",
  "Disputed",
  "Failed",
  "Unresolved",
]);

const accentTags: ReadonlySet<SwapWidgetStateTag> = new Set([
  "Ready",
  "Ordering",
  "AwaitingFunding",
]);

/**
 * Message keys whose catalog entry takes no parameters. Typing them as this
 * subset is what lets the shell read one straight off the catalog: a
 * parameterised key cannot be listed here without failing typecheck, so a
 * refusal that needs the user's units can never be rendered without them.
 */
type StaticMessageKey = {
  [K in MessageKey]: Catalog[K] extends string ? K : never;
}[MessageKey];

const staticKeys: Partial<Record<SwapWidgetStateTag, StaticMessageKey>> = {
  Offline: "swap.widget.offline",
  EngineLoading: "swap.widget.engine_loading",
  EngineFailed: "swap.widget.engine_failed",
  PairsLoading: "swap.widget.pairs_loading",
  NoOfferings: "swap.widget.pairs_loading",
  Empty: "swap.widget.enter_amount",
  AmountUnparseable: "swap.widget.enter_amount",
  ZeroOutput: "swap.widget.zero_output",
  QuoteRefreshing: "swap.widget.quote_refreshing",
  QuoteExpired: "swap.widget.quote_expired",
  NoDestination: "swap.widget.no_destination",
  VerificationPending: "swap.widget.verification_pending",
  Ready: "swap.widget.action.create",
  // Ordering keeps the action label: the control is blocked (busy) without
  // manufacturing a fresh explanation.
  Ordering: "swap.widget.action.create",
  AwaitingFunding: "swap.widget.action.fund",
  FundingObserved: "swap.widget.funding_observed",
  Executing: "swap.widget.executing",
  SettlementPending: "swap.widget.settlement_pending",
  Completed: "swap.widget.completed",
  RefundPending: "swap.widget.refund_pending",
  Refunded: "swap.widget.refunded",
  Disputed: "swap.widget.disputed",
  Unresolved: "swap.widget.unresolved",
};

interface Rendered {
  readonly label: string;
  readonly messageKey: string;
  readonly swpError: SwpErrorIdentifier | null;
}

const staticLabel = (catalog: Catalog, key: StaticMessageKey): string => catalog[key];

const identifierLabel = (
  catalog: Catalog,
  identifier: SwpErrorIdentifier,
  fallbackKey: StaticMessageKey,
): Rendered => ({
  label: `${staticLabel(catalog, fallbackKey)}: ${messageForSwpError(catalog, identifier)}`,
  messageKey: fallbackKey,
  swpError: identifier,
});

const renderLabel = (
  state: SwapWidgetState,
  catalog: Catalog,
  denomination: Denomination,
  separator: DecimalSeparator,
): Rendered => {
  switch (state._tag) {
    case "BelowMinimum":
      return {
        label: render(catalog, "swap.refusal.below_minimum", {
          minimum: formatAmountText(BigInt(state.minimumSats), denomination, separator),
          denomination: denominationLabel(denomination),
        }),
        messageKey: "swap.refusal.below_minimum",
        swpError: "swp_invalid_amount",
      };
    case "AboveMaximum":
      return {
        label: render(catalog, "swap.refusal.above_maximum", {
          maximum: formatAmountText(BigInt(state.maximumSats), denomination, separator),
          denomination: denominationLabel(denomination),
        }),
        messageKey: "swap.refusal.above_maximum",
        swpError: "swp_invalid_amount",
      };
    case "CoverageGap":
      return {
        label: render(catalog, "swap.refusal.amount_range", {
          minimum: formatAmountText(BigInt(state.nearestBelowSats), denomination, separator),
          maximum: formatAmountText(BigInt(state.nearestAboveSats), denomination, separator),
          denomination: denominationLabel(denomination),
        }),
        messageKey: "swap.refusal.amount_range",
        swpError: "swp_invalid_amount",
      };
    case "UnsupportedDirection":
      return identifierLabel(catalog, state.identifier, "swap.widget.unsupported_direction");
    case "QuoteFailed":
      return identifierLabel(catalog, state.identifier, "swap.widget.quote_failed");
    case "InvalidDestination":
      return identifierLabel(catalog, state.identifier, "swap.widget.invalid_destination");
    case "VerificationFailed":
      return identifierLabel(catalog, state.identifier, "swap.widget.verification_failed");
    case "Failed":
      return state.identifier === undefined
        ? {
            label: staticLabel(catalog, "swap.widget.failed"),
            messageKey: "swap.widget.failed",
            swpError: null,
          }
        : identifierLabel(catalog, state.identifier, "swap.widget.failed");
    default: {
      const key: StaticMessageKey = staticKeys[state._tag] ?? "swap.widget.unresolved";
      return { label: staticLabel(catalog, key), messageKey: key, swpError: null };
    }
  }
};

/**
 * Derive the primary-action control from the widget state. Each mechanism is
 * computed from its own predicate rather than from the label, so the four
 * stay independently testable — and so a change to one cannot silently move
 * another.
 */
export const derivePrimaryAction = (
  state: SwapWidgetState,
  catalog: Catalog,
  denomination: Denomination,
  separator: DecimalSeparator = ".",
): PrimaryActionModel => {
  const rendered = renderLabel(state, catalog, denomination, separator);
  return {
    label: rendered.label,
    messageKey: rendered.messageKey,
    swpError: rendered.swpError,
    tone: dangerTags.has(state._tag) ? "danger" : accentTags.has(state._tag) ? "accent" : "neutral",
    disabled: !enabledTags.has(state._tag),
    busy: busyTags.has(state._tag) && !permanentRefusalTags.has(state._tag),
  };
};

/** True when no retry, wait, or spinner can resolve the refusal. */
export const isPermanentRefusal = (state: SwapWidgetState): boolean =>
  permanentRefusalTags.has(state._tag);
