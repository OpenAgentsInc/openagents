/**
 * Messages for pair selection and the rate/fee panel.
 *
 * The shared SWAP-8 catalog (`@openagentsinc/swap-i18n`) already carries
 * the MKT-SWP §17 identifier table and the parameterised amount refusals
 * (`swap.refusal.below_minimum` / `above_maximum` / `amount_range`); those
 * are used directly. Pair/panel-specific messages that have no shared key
 * yet live in the local `swap.pair.*` table below, shaped for migration
 * into the SWAP-8 source catalog (same key discipline as
 * `mkt-swp-destination`'s `swap.destination.*` table).
 *
 * The primary-action refusal law (SWAP-0): a refusal states the single
 * most proximate reason, and amount limits render in the user's current
 * denomination — the limit is formatted with the exact bigint formatter,
 * never pre-baked into a string.
 */
import {
  messageForSwpError,
  render,
  type Catalog,
} from "@openagentsinc/swap-i18n";

import {
  formatAmountText,
  type AmountParseFailureMode,
  type DecimalSeparator,
  type Denomination,
} from "./amount.js";
import type { DirectionUnreachableReason } from "./corpus.js";
import type { PriceFeedMismatchMode } from "./price-feed.js";
import type { PairNotice, PrimaryActionRefusal } from "./selection.js";

export interface PairMessage {
  /** Stable key, shaped for the SWAP-8 shared table. */
  readonly key: `swap.pair.${string}`;
  /** Default English message. Localisation arrives via SWAP-8. */
  readonly message: string;
}

/**
 * Unreachable-direction disclosure (issue §1): each reason renders its own
 * message saying what would need to change, before selection, never a
 * generic "unavailable". The view attaches the advertising providers.
 */
export const DIRECTION_UNREACHABLE_MESSAGES: Readonly<
  Record<DirectionUnreachableReason, PairMessage>
> = {
  no_offering: {
    key: "swap.pair.unreachable.no_offering",
    message:
      "No provider currently advertises this direction. It becomes available when a provider publishes an Offering for it.",
  },
  side_disabled: {
    key: "swap.pair.unreachable.side_disabled",
    message:
      "Every provider advertising this direction has disabled it (max = 0). It becomes available when a provider re-enables the side.",
  },
  provider_paused: {
    key: "swap.pair.unreachable.provider_paused",
    message:
      "The providers offering this direction are currently paused or unavailable. It becomes available when one resumes.",
  },
  offerings_stale: {
    key: "swap.pair.unreachable.offerings_stale",
    message:
      "The Offerings for this direction have not been observed recently enough to trust. Waiting for a fresh head from the relay.",
  },
};

export const EMPTY_CORPUS_MESSAGE: PairMessage = {
  key: "swap.pair.empty_corpus",
  message:
    "No live Offerings discovered yet. Available pairs appear as providers' Offering heads arrive from the relay.",
};

export const AMOUNT_PARSE_FAILURE_MESSAGES: Readonly<
  Record<AmountParseFailureMode, PairMessage>
> = {
  empty: {
    key: "swap.pair.amount.empty",
    message: "Enter an amount",
  },
  not_a_number: {
    key: "swap.pair.amount.not_a_number",
    message: "The amount contains characters that are not digits",
  },
  multiple_separators: {
    key: "swap.pair.amount.multiple_separators",
    message: "The amount has more than one decimal separator",
  },
  sats_fractional: {
    key: "swap.pair.amount.sats_fractional",
    message:
      "Satoshi amounts are whole numbers — switch to BTC to enter a fraction",
  },
  too_many_decimal_places: {
    key: "swap.pair.amount.too_many_decimal_places",
    message: "BTC amounts have at most 8 decimal places (1 satoshi)",
  },
  exceeds_supply: {
    key: "swap.pair.amount.exceeds_supply",
    message: "The amount exceeds 21,000,000 BTC",
  },
};

export const PAIR_NOTICE_MESSAGES: Readonly<
  Record<PairNotice["notice"], PairMessage>
> = {
  sides_swapped: {
    key: "swap.pair.notice.sides_swapped",
    message: "Sides swapped — that asset was selected on the other side",
  },
  quote_cleared_by_amount_edit: {
    key: "swap.pair.notice.quote_cleared_by_amount_edit",
    message: "Quote cleared — the amount changed, so fresh terms are needed",
  },
  quote_cleared_by_direction_change: {
    key: "swap.pair.notice.quote_cleared_by_direction_change",
    message: "Quote cleared — the direction changed, so fresh terms are needed",
  },
};

export const DIRECTION_UNSELECTED_MESSAGE: PairMessage = {
  key: "swap.pair.direction_unselected",
  message: "Choose the asset you send and the asset you receive",
};

export const DIRECTION_UNSUPPORTED_MESSAGE: PairMessage = {
  key: "swap.pair.direction_unsupported",
  message: "This combination is not a v1 swap shape",
};

export const COVERAGE_GAP_MESSAGES = {
  key: "swap.pair.amount.coverage_gap",
  /** Parameterised locally; migrates to a SWAP-8 function message. */
  message: (nearestBelow: string, nearestAbove: string, denomination: string) =>
    `No single provider covers this amount — enter at most ${nearestBelow} ${denomination} or at least ${nearestAbove} ${denomination}.`,
} as const;

/**
 * The fee panel's promise framing (MKT-SWP §3.3): the output amount is a
 * fill promise, not an estimate — stated explicitly, which the Boltz panel
 * never does.
 */
export const FEE_PROMISE_MESSAGE: PairMessage = {
  key: "swap.pair.fee.promise",
  message:
    "The receive amount is the provider's fill promise. It cannot be reduced after your order because the provider's route or miner fee changed.",
};

export const PRICE_FEED_MISMATCH_MESSAGES: Readonly<
  Record<PriceFeedMismatchMode, PairMessage>
> = {
  pinned_url_invalid: {
    key: "swap.pair.price_feed.pinned_url_invalid",
    message:
      "The pinned price feed URL breaks the exact-pinning rules (HTTPS only, no userinfo, no fragment).",
  },
  substituted_url: {
    key: "swap.pair.price_feed.substituted_url",
    message:
      "The fetched price feed URL is not the exact pinned URL — a substitute host, mirror, or fallback endpoint is a term mismatch.",
  },
  substituted_pointer: {
    key: "swap.pair.price_feed.substituted_pointer",
    message:
      "The fetched value came from a different JSON pointer than the pinned one.",
  },
  value_mismatch: {
    key: "swap.pair.price_feed.value_mismatch",
    message:
      "The fetched feed value does not equal the pinned observed value.",
  },
  digest_mismatch: {
    key: "swap.pair.price_feed.digest_mismatch",
    message:
      "The fetched response bytes do not match the pinned response digest.",
  },
};

export const PRICE_FEED_NONE_MESSAGE: PairMessage = {
  key: "swap.pair.price_feed.none",
  message: "No price feed is pinned — this quote's terms are fixed amounts.",
};

export const PRICE_FEED_STALE_MESSAGE: PairMessage = {
  key: "swap.pair.price_feed.stale",
  message:
    "The pinned price feed observation is older than its maximum age — fresh terms are needed.",
};

export const PRICE_FEED_UNCHECKED_MESSAGE: PairMessage = {
  key: "swap.pair.price_feed.unchecked",
  message:
    "This quote pins a price feed that has not been verified with your own fetch yet.",
};

export const QUOTE_INPUT_UNSERVICEABLE_MESSAGES = {
  key: "swap.pair.quote.input_unserviceable",
  /** Parameterised locally; migrates to a SWAP-8 function message. */
  message: (input: string, minimum: string, maximum: string, denomination: string) =>
    `The quote's input amount ${input} ${denomination} is outside the offered limits (${minimum} to ${maximum} ${denomination}).`,
} as const;

const DENOMINATION_LABELS: Readonly<Record<Denomination, string>> = {
  btc: "BTC",
  sats: "sats",
};

export const denominationLabel = (denomination: Denomination): string =>
  DENOMINATION_LABELS[denomination];

/**
 * The rendered primary-action refusal: exactly one message, the most
 * proximate reason, with limits in the user's current denomination
 * (SWAP-0 primary-action law). Shared-catalog keys are used where SWAP-8
 * defines them; local `swap.pair.*` keys otherwise.
 */
export const primaryActionRefusalMessage = (
  catalog: Catalog,
  refusal: PrimaryActionRefusal,
  denomination: Denomination,
  separator: DecimalSeparator,
): { readonly key: string; readonly message: string } => {
  const unit = denominationLabel(denomination);
  switch (refusal.kind) {
    case "empty_corpus":
      return EMPTY_CORPUS_MESSAGE;
    case "direction_unselected":
      return DIRECTION_UNSELECTED_MESSAGE;
    case "direction_unsupported":
      return DIRECTION_UNSUPPORTED_MESSAGE;
    case "direction_unreachable":
      return DIRECTION_UNREACHABLE_MESSAGES[refusal.reason];
    case "amount_missing":
      return AMOUNT_PARSE_FAILURE_MESSAGES.empty;
    case "amount_unparseable":
      return AMOUNT_PARSE_FAILURE_MESSAGES[refusal.mode];
    case "below_minimum":
      return {
        key: "swap.refusal.below_minimum",
        message: render(catalog, "swap.refusal.below_minimum", {
          minimum: formatAmountText(refusal.minimumSats, denomination, separator),
          denomination: unit,
        }),
      };
    case "above_maximum":
      return {
        key: "swap.refusal.above_maximum",
        message: render(catalog, "swap.refusal.above_maximum", {
          maximum: formatAmountText(refusal.maximumSats, denomination, separator),
          denomination: unit,
        }),
      };
    case "coverage_gap":
      return {
        key: COVERAGE_GAP_MESSAGES.key,
        message: COVERAGE_GAP_MESSAGES.message(
          formatAmountText(refusal.nearestBelowSats, denomination, separator),
          formatAmountText(refusal.nearestAboveSats, denomination, separator),
          unit,
        ),
      };
    case "quote_terms_refused":
      return {
        key: `swap.error.${refusal.refusal.error}`,
        message: messageForSwpError(catalog, refusal.refusal.error),
      };
    case "quote_input_unserviceable":
      return {
        key: QUOTE_INPUT_UNSERVICEABLE_MESSAGES.key,
        message: QUOTE_INPUT_UNSERVICEABLE_MESSAGES.message(
          formatAmountText(refusal.inputSats, denomination, separator),
          formatAmountText(refusal.minimumSats, denomination, separator),
          formatAmountText(refusal.maximumSats, denomination, separator),
          unit,
        ),
      };
    case "price_feed_unchecked":
      return PRICE_FEED_UNCHECKED_MESSAGE;
    case "price_feed_refused":
      return refusal.check.error === "swp_price_feed_stale"
        ? PRICE_FEED_STALE_MESSAGE
        : PRICE_FEED_MISMATCH_MESSAGES[refusal.check.mode];
  }
};
