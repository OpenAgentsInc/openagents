/**
 * The headless pair/direction/amount state machine (openagents#9316 §1,
 * §3, §4). The SWAP-0 widget shell mounts this as a pure reducer: events
 * in, state out, no DOM and no IO.
 *
 * Laws enforced here (behaviour contracts, market-swap-pair registry in
 * `@openagentsinc/behavior-contracts`):
 *
 * - `openagents_web.swap_pair.unreachable_direction_disclosed.v1`
 *   A direction is unreachable *before* selection with a typed reason
 *   folded from the live Offering corpus, and can never be selected into
 *   a fundable state.
 * - `openagents_web.swap_pair.no_auto_unit_switch.v1`
 *   The display denomination changes only on the explicit toggle event;
 *   no input path ever reinterprets the digits the user typed.
 *
 * Selecting the counterparty's asset on the other side swaps the sides
 * rather than refusing (teardown §3.1). Asset identity is the exact
 * `asset_id` string throughout; labels never participate.
 */
import {
  formatAmountText,
  parseAmountText,
  type AmountParseFailureMode,
  type DecimalSeparator,
  type Denomination,
} from "./amount.js";
import { directionKey, swapTypeOf, type SwapDirection } from "./asset.js";
import {
  directionAvailability,
  foldOfferingCorpus,
  type CorpusFoldConfig,
  type DirectionAvailability,
  type DirectionUnreachableReason,
  type DiscoveredOffering,
  type OfferingCorpusFold,
} from "./corpus.js";
import {
  checkPinnedPriceFeed,
  isPinnedFeedUrlValid,
  type PriceFeedCheck,
  type PriceFeedFetchRecord,
} from "./price-feed.js";
import { verifyQuoteTerms, type QuoteTermsVerification, type SwapQuoteTerms } from "./quote.js";

export type AmountSide = "input" | "output";

export type PairNotice =
  | { readonly notice: "sides_swapped" }
  | { readonly notice: "quote_cleared_by_amount_edit" }
  | { readonly notice: "quote_cleared_by_direction_change" };

/**
 * A quote that verifies arithmetically can still fail to bind to this
 * session: its committed pair must be the selected direction
 * (`swp_invalid_pair`) and its authoritative-side amount must be exactly
 * the amount the user entered (`swp_terms_mismatch`, §4.4's "one
 * `input_amount` within a range explicitly offered by that Quote"). The
 * binding holds by construction here, not by SWAP-3's call ordering.
 */
export interface QuoteBindingRefusal {
  readonly ok: false;
  readonly error: "swp_invalid_pair" | "swp_terms_mismatch";
  readonly detail: string;
}

/**
 * Pinned-feed verification state for a held verified quote (MKT-SWP
 * §3.4). "Terms reproduced" and "pinned feed checked" are distinct
 * states: a quote that pins a feed stays `unchecked` — and the primary
 * action stays gated — until the requester's own fetch is delivered via
 * `price_feed_checked` and passes `checkPinnedPriceFeed`.
 */
export type HeldQuotePriceFeed =
  | { readonly state: "none_pinned" }
  | { readonly state: "unchecked" }
  | { readonly state: "verified"; readonly fetched: PriceFeedFetchRecord }
  | {
      readonly state: "refused";
      readonly check: Extract<PriceFeedCheck, { ok: false }>;
      readonly fetched: PriceFeedFetchRecord | null;
    };

export type HeldQuote =
  | {
      readonly status: "verified";
      readonly terms: SwapQuoteTerms;
      readonly priceFeed: HeldQuotePriceFeed;
    }
  | {
      readonly status: "refused";
      readonly terms: SwapQuoteTerms;
      readonly refusal:
        | Extract<QuoteTermsVerification, { ok: false }>
        | QuoteBindingRefusal;
    };

export interface PairSelectionState {
  readonly fold: OfferingCorpusFold;
  readonly inputAssetId: string | null;
  readonly outputAssetId: string | null;
  readonly amountText: { readonly input: string; readonly output: string };
  /** Which field the user edited last; the other side derives via quote. */
  readonly authoritativeSide: AmountSide;
  /** Persisted display preferences (issue §3). */
  readonly denomination: Denomination;
  readonly decimalSeparator: DecimalSeparator;
  /** Whichever Quote the session holds (SWAP-3 selects; we render terms). */
  readonly quote: HeldQuote | null;
  /** Transient notices for this transition. */
  readonly notices: readonly PairNotice[];
}

export const initialPairSelectionState = (
  preferences?: Partial<
    Pick<PairSelectionState, "denomination" | "decimalSeparator">
  >,
): PairSelectionState => ({
  fold: { state: "empty", foldedAtSeconds: 0 },
  inputAssetId: null,
  outputAssetId: null,
  amountText: { input: "", output: "" },
  authoritativeSide: "input",
  denomination: preferences?.denomination ?? "btc",
  decimalSeparator: preferences?.decimalSeparator ?? ".",
  quote: null,
  notices: [],
});

export type PairEvent =
  | {
      readonly type: "corpus_updated";
      readonly offerings: readonly DiscoveredOffering[];
      readonly config: CorpusFoldConfig;
    }
  | { readonly type: "input_asset_selected"; readonly assetId: string }
  | { readonly type: "output_asset_selected"; readonly assetId: string }
  | { readonly type: "direction_toggled" }
  | {
      readonly type: "amount_edited";
      readonly side: AmountSide;
      readonly text: string;
    }
  | { readonly type: "max_requested" }
  | { readonly type: "denomination_toggled" }
  | {
      readonly type: "decimal_separator_selected";
      readonly separator: DecimalSeparator;
    }
  | { readonly type: "quote_applied"; readonly terms: SwapQuoteTerms }
  | {
      /** The requester's own fetch of the pinned feed (MKT-SWP §3.4). */
      readonly type: "price_feed_checked";
      readonly fetched: PriceFeedFetchRecord;
      readonly nowSeconds: number;
    }
  | { readonly type: "quote_cleared" };

/** The selected ordered direction, when both sides are chosen. */
export const selectedDirection = (
  state: PairSelectionState,
): SwapDirection | null =>
  state.inputAssetId !== null && state.outputAssetId !== null
    ? { inputAssetId: state.inputAssetId, outputAssetId: state.outputAssetId }
    : null;

/** Availability of the selected direction under the current fold. */
export const selectedAvailability = (
  state: PairSelectionState,
): DirectionAvailability | null => {
  const direction = selectedDirection(state);
  return direction === null
    ? null
    : directionAvailability(state.fold, direction);
};

/**
 * First reachable direction in deterministic (direction-key) order; used
 * to seed a selection when the corpus first populates. Never overrides an
 * existing selection.
 */
export const defaultDirection = (
  fold: OfferingCorpusFold,
): SwapDirection | null => {
  if (fold.state !== "populated") return null;
  const reachable = [...fold.directions.values()]
    .filter((availability) => availability.reachable)
    .sort((a, b) =>
      directionKey(a.direction) < directionKey(b.direction) ? -1 : 1,
    );
  return reachable[0]?.direction ?? null;
};

const clearQuote = (
  state: PairSelectionState,
  notice: PairNotice,
): Pick<PairSelectionState, "quote" | "notices"> =>
  state.quote === null
    ? { quote: null, notices: [] }
    : { quote: null, notices: [notice] };

/** Reformat a field's text for a new denomination/separator, exactly. */
const reformatText = (
  text: string,
  from: { denomination: Denomination; separator: DecimalSeparator },
  to: { denomination: Denomination; separator: DecimalSeparator },
): string => {
  if (text.trim() === "") return text;
  const parsed = parseAmountText(text, from.denomination, from.separator);
  // Unparseable text stays verbatim: it was invalid before the change and
  // stays visibly invalid after it; nothing reinterprets the digits.
  if (!parsed.ok) return text;
  return formatAmountText(parsed.sats, to.denomination, to.separator);
};

export const reducePairEvent = (
  state: PairSelectionState,
  event: PairEvent,
): PairSelectionState => {
  switch (event.type) {
    case "corpus_updated": {
      const fold = foldOfferingCorpus(event.offerings, event.config);
      const next: PairSelectionState = { ...state, fold, notices: [] };
      // Seed a selection only when nothing is selected yet. An existing
      // selection whose direction became unreachable is kept and renders
      // unreachable with its reason — never silently reset.
      if (next.inputAssetId === null && next.outputAssetId === null) {
        const seeded = defaultDirection(fold);
        if (seeded !== null) {
          return {
            ...next,
            inputAssetId: seeded.inputAssetId,
            outputAssetId: seeded.outputAssetId,
          };
        }
      }
      return next;
    }

    case "input_asset_selected": {
      if (event.assetId === state.inputAssetId) return { ...state, notices: [] };
      // Selecting the counterparty's asset swaps the sides (teardown §3.1).
      if (state.outputAssetId !== null && event.assetId === state.outputAssetId) {
        const cleared = clearQuote(state, {
          notice: "quote_cleared_by_direction_change",
        });
        return {
          ...state,
          inputAssetId: state.outputAssetId,
          outputAssetId: state.inputAssetId,
          ...cleared,
          notices: [{ notice: "sides_swapped" }, ...cleared.notices],
        };
      }
      const cleared = clearQuote(state, {
        notice: "quote_cleared_by_direction_change",
      });
      return { ...state, inputAssetId: event.assetId, ...cleared };
    }

    case "output_asset_selected": {
      if (event.assetId === state.outputAssetId) return { ...state, notices: [] };
      if (state.inputAssetId !== null && event.assetId === state.inputAssetId) {
        const cleared = clearQuote(state, {
          notice: "quote_cleared_by_direction_change",
        });
        return {
          ...state,
          inputAssetId: state.outputAssetId,
          outputAssetId: state.inputAssetId,
          ...cleared,
          notices: [{ notice: "sides_swapped" }, ...cleared.notices],
        };
      }
      const cleared = clearQuote(state, {
        notice: "quote_cleared_by_direction_change",
      });
      return { ...state, outputAssetId: event.assetId, ...cleared };
    }

    case "direction_toggled": {
      if (state.inputAssetId === null || state.outputAssetId === null) {
        return { ...state, notices: [] };
      }
      const cleared = clearQuote(state, {
        notice: "quote_cleared_by_direction_change",
      });
      return {
        ...state,
        inputAssetId: state.outputAssetId,
        outputAssetId: state.inputAssetId,
        ...cleared,
        notices: [{ notice: "sides_swapped" }, ...cleared.notices],
      };
    }

    case "amount_edited": {
      // A held quote binds an exact amount; editing invalidates it with an
      // explicit notice (the fresh terms come from SWAP-3's next quote).
      const cleared = clearQuote(state, {
        notice: "quote_cleared_by_amount_edit",
      });
      return {
        ...state,
        amountText: { ...state.amountText, [event.side]: event.text },
        authoritativeSide: event.side,
        ...cleared,
      };
    }

    case "max_requested": {
      const availability = selectedAvailability(state);
      if (availability === null || !availability.reachable) {
        return { ...state, notices: [] };
      }
      const cleared = clearQuote(state, {
        notice: "quote_cleared_by_amount_edit",
      });
      return {
        ...state,
        amountText: {
          ...state.amountText,
          input: formatAmountText(
            availability.maxSats,
            state.denomination,
            state.decimalSeparator,
          ),
        },
        authoritativeSide: "input",
        ...cleared,
      };
    }

    case "denomination_toggled": {
      // The ONLY path that changes the display denomination (the
      // no-auto-unit-switch law). Parsed values convert exactly; the
      // digits themselves are never reinterpreted in another unit.
      const from = {
        denomination: state.denomination,
        separator: state.decimalSeparator,
      };
      const denomination: Denomination =
        state.denomination === "btc" ? "sats" : "btc";
      const to = { denomination, separator: state.decimalSeparator };
      return {
        ...state,
        denomination,
        amountText: {
          input: reformatText(state.amountText.input, from, to),
          output: reformatText(state.amountText.output, from, to),
        },
        notices: [],
      };
    }

    case "decimal_separator_selected": {
      if (event.separator === state.decimalSeparator) {
        return { ...state, notices: [] };
      }
      const from = {
        denomination: state.denomination,
        separator: state.decimalSeparator,
      };
      const to = {
        denomination: state.denomination,
        separator: event.separator,
      };
      return {
        ...state,
        decimalSeparator: event.separator,
        amountText: {
          input: reformatText(state.amountText.input, from, to),
          output: reformatText(state.amountText.output, from, to),
        },
        notices: [],
      };
    }

    case "quote_applied": {
      const verification = verifyQuoteTerms(event.terms);
      if (!verification.ok) {
        return {
          ...state,
          quote: {
            status: "refused",
            terms: event.terms,
            refusal: verification,
          },
          notices: [],
        };
      }
      // A held quote binds the selected direction and the entered amount
      // by construction, not by SWAP-3's call ordering: a quote for a
      // different pair or a different amount refuses instead of rendering
      // its terms beside the user's number.
      const binding = bindQuoteToSelection(state, event.terms, verification.amounts);
      if (binding !== null) {
        return {
          ...state,
          quote: { status: "refused", terms: event.terms, refusal: binding },
          notices: [],
        };
      }
      // The quote's amounts are authoritative for the derived side: render
      // the promise, in the user's current denomination.
      const amounts = verification.amounts;
      const derived: { input?: string; output?: string } =
        state.authoritativeSide === "input"
          ? {
              output: formatAmountText(
                amounts.outputSats,
                state.denomination,
                state.decimalSeparator,
              ),
            }
          : {
              input: formatAmountText(
                amounts.inputSats,
                state.denomination,
                state.decimalSeparator,
              ),
            };
      // A pinned feed starts unchecked (the requester's own fetch arrives
      // via `price_feed_checked`); a pinned URL breaking the §3.4 form
      // rules refuses immediately, before any fetch.
      const priceFeed: HeldQuotePriceFeed =
        event.terms.priceFeed === null
          ? { state: "none_pinned" }
          : isPinnedFeedUrlValid(event.terms.priceFeed.url)
            ? { state: "unchecked" }
            : {
                state: "refused",
                check: {
                  ok: false,
                  error: "swp_price_feed_invalid",
                  mode: "pinned_url_invalid",
                },
                fetched: null,
              };
      return {
        ...state,
        quote: { status: "verified", terms: event.terms, priceFeed },
        amountText: { ...state.amountText, ...derived },
        notices: [],
      };
    }

    case "price_feed_checked": {
      if (state.quote === null || state.quote.status !== "verified") {
        return { ...state, notices: [] };
      }
      const pinned = state.quote.terms.priceFeed;
      if (pinned === null) return { ...state, notices: [] };
      const check = checkPinnedPriceFeed(pinned, event.fetched, event.nowSeconds);
      const priceFeed: HeldQuotePriceFeed = check.ok
        ? { state: "verified", fetched: event.fetched }
        : { state: "refused", check, fetched: event.fetched };
      return {
        ...state,
        quote: { ...state.quote, priceFeed },
        notices: [],
      };
    }

    case "quote_cleared":
      return { ...state, quote: null, notices: [] };
  }
};

/**
 * Bind an arithmetically verified quote to the current selection: the
 * quote's committed pair must equal the selected direction, and its
 * amount on the user's authoritative side must equal the entered amount
 * exactly (§4.4). Returns the typed refusal, or `null` when bound.
 */
const bindQuoteToSelection = (
  state: PairSelectionState,
  terms: SwapQuoteTerms,
  amounts: { readonly inputSats: bigint; readonly outputSats: bigint },
): QuoteBindingRefusal | null => {
  const direction = selectedDirection(state);
  if (direction === null) {
    return {
      ok: false,
      error: "swp_invalid_pair",
      detail: "no direction is selected to bind the quote's pair to",
    };
  }
  if (
    terms.inputAssetId !== direction.inputAssetId ||
    terms.outputAssetId !== direction.outputAssetId
  ) {
    return {
      ok: false,
      error: "swp_invalid_pair",
      detail: `the quote's ordered pair ${terms.inputAssetId} -> ${terms.outputAssetId} is not the selected direction`,
    };
  }
  const side = state.authoritativeSide;
  const parsed = parseAmountText(
    state.amountText[side],
    state.denomination,
    state.decimalSeparator,
  );
  if (!parsed.ok) {
    return {
      ok: false,
      error: "swp_terms_mismatch",
      detail: `no entered ${side} amount to bind the quote to`,
    };
  }
  const quotedSats = side === "input" ? amounts.inputSats : amounts.outputSats;
  if (parsed.sats !== quotedSats) {
    return {
      ok: false,
      error: "swp_terms_mismatch",
      detail: `the quote's ${side} amount ${quotedSats} is not the entered amount ${parsed.sats}`,
    };
  }
  return null;
};

// ---------------------------------------------------------------------------
// Primary-action gate
// ---------------------------------------------------------------------------

/**
 * The single most proximate refusal, walked in a fixed precedence
 * (SWAP-0 primary-action law): corpus, direction, reachability, amount
 * presence, amount validity, limits, then held-quote binding — so the
 * button always states exactly one reason, the nearest one to being
 * fixed by the user.
 *
 * An enabled gate names the side its amount was measured on: `amountSats`
 * is the entered amount on `side`, never an implicit input amount. The
 * amount a consumer may fund is `fundableInputSats` — the entered input
 * when `side` is `"input"`, the held verified quote's input (validated
 * against the Offering limits) when `side` is `"output"`, and `null`
 * when no quote yet names an input. An output-side figure can never be
 * misread as an input amount to fund.
 */
export type PrimaryActionGate =
  | {
      readonly enabled: true;
      /** Which field the enabled amount was measured on. */
      readonly side: AmountSide;
      /** The entered amount on `side`, exact satoshis. */
      readonly amountSats: bigint;
      /**
       * The input amount a consumer may fund, validated against the
       * Offering limits; `null` while only an output amount is known.
       */
      readonly fundableInputSats: bigint | null;
    }
  | { readonly enabled: false; readonly refusal: PrimaryActionRefusal };

export type PrimaryActionRefusal =
  | { readonly kind: "empty_corpus" }
  | { readonly kind: "direction_unselected" }
  | { readonly kind: "direction_unsupported" }
  | {
      readonly kind: "direction_unreachable";
      readonly reason: DirectionUnreachableReason;
      readonly advertisingProviders: readonly string[];
      /** MKT-SWP §17 identifier for the disabled-side case. */
      readonly swpError: "swp_side_disabled";
    }
  | { readonly kind: "amount_missing" }
  | {
      readonly kind: "amount_unparseable";
      readonly mode: AmountParseFailureMode;
    }
  | {
      readonly kind: "below_minimum";
      readonly minimumSats: bigint;
      readonly swpError: "swp_invalid_amount";
    }
  | {
      readonly kind: "above_maximum";
      readonly maximumSats: bigint;
      readonly swpError: "swp_invalid_amount";
    }
  | {
      readonly kind: "coverage_gap";
      /** Largest serviceable amount below the entered one. */
      readonly nearestBelowSats: bigint;
      /** Smallest serviceable amount above the entered one. */
      readonly nearestAboveSats: bigint;
      readonly swpError: "swp_invalid_amount";
    }
  | {
      readonly kind: "quote_terms_refused";
      readonly refusal:
        | Extract<QuoteTermsVerification, { ok: false }>
        | QuoteBindingRefusal;
    }
  | {
      /** The held quote's input violates the folded Offering limits. */
      readonly kind: "quote_input_unserviceable";
      readonly inputSats: bigint;
      readonly minimumSats: bigint;
      readonly maximumSats: bigint;
      readonly swpError: "swp_invalid_amount";
    }
  | {
      /** A pinned feed exists and the requester's fetch has not run yet. */
      readonly kind: "price_feed_unchecked";
    }
  | {
      readonly kind: "price_feed_refused";
      readonly check: Extract<PriceFeedCheck, { ok: false }>;
      readonly swpError: "swp_price_feed_invalid" | "swp_price_feed_stale";
    };

export const primaryActionGate = (
  state: PairSelectionState,
): PrimaryActionGate => {
  if (state.fold.state === "empty") {
    return { enabled: false, refusal: { kind: "empty_corpus" } };
  }
  const direction = selectedDirection(state);
  if (direction === null) {
    return { enabled: false, refusal: { kind: "direction_unselected" } };
  }
  if (swapTypeOf(direction) === null) {
    return { enabled: false, refusal: { kind: "direction_unsupported" } };
  }
  const availability = directionAvailability(state.fold, direction);
  if (!availability.reachable) {
    return {
      enabled: false,
      refusal: {
        kind: "direction_unreachable",
        reason: availability.reason,
        advertisingProviders: availability.advertisingProviders,
        swpError: "swp_side_disabled",
      },
    };
  }

  const side = state.authoritativeSide;
  const text = state.amountText[side];
  if (text.trim() === "") {
    return { enabled: false, refusal: { kind: "amount_missing" } };
  }
  const parsed = parseAmountText(
    text,
    state.denomination,
    state.decimalSeparator,
  );
  if (!parsed.ok) {
    return {
      enabled: false,
      refusal: { kind: "amount_unparseable", mode: parsed.mode },
    };
  }
  const amountSats = parsed.sats;

  if (side === "input") {
    // Offering limits denominate the input side (MKT-SWP §3.2).
    if (amountSats < availability.minSats) {
      return {
        enabled: false,
        refusal: {
          kind: "below_minimum",
          minimumSats: availability.minSats,
          swpError: "swp_invalid_amount",
        },
      };
    }
    if (amountSats > availability.maxSats) {
      return {
        enabled: false,
        refusal: {
          kind: "above_maximum",
          maximumSats: availability.maxSats,
          swpError: "swp_invalid_amount",
        },
      };
    }
    const covered = availability.sides.some(
      (source) => amountSats >= source.minSats && amountSats <= source.maxSats,
    );
    if (!covered) {
      let nearestBelowSats = availability.minSats;
      let nearestAboveSats = availability.maxSats;
      for (const source of availability.sides) {
        if (source.maxSats < amountSats && source.maxSats > nearestBelowSats) {
          nearestBelowSats = source.maxSats;
        }
        if (source.minSats > amountSats && source.minSats < nearestAboveSats) {
          nearestAboveSats = source.minSats;
        }
      }
      return {
        enabled: false,
        refusal: {
          kind: "coverage_gap",
          nearestBelowSats,
          nearestAboveSats,
          swpError: "swp_invalid_amount",
        },
      };
    }
  } else {
    // The output is a promise derived by the quote; pre-quote we can only
    // bound it: a promised output can never exceed the largest input.
    if (amountSats > availability.maxSats) {
      return {
        enabled: false,
        refusal: {
          kind: "above_maximum",
          maximumSats: availability.maxSats,
          swpError: "swp_invalid_amount",
        },
      };
    }
    if (amountSats === 0n) {
      return {
        enabled: false,
        refusal: {
          kind: "below_minimum",
          minimumSats: 1n,
          swpError: "swp_invalid_amount",
        },
      };
    }
  }

  if (state.quote?.status === "refused") {
    return {
      enabled: false,
      refusal: { kind: "quote_terms_refused", refusal: state.quote.refusal },
    };
  }

  // A held verified quote names the input that would actually fund. That
  // input must satisfy the folded Offering limits regardless of which
  // side the user typed — the output branch above cannot check the
  // minimum pre-quote, so the check lands here, on the amount that funds.
  let fundableInputSats: bigint | null = side === "input" ? amountSats : null;
  if (state.quote?.status === "verified") {
    const verification = verifyQuoteTerms(state.quote.terms);
    if (verification.ok) {
      const quoteInputSats = verification.amounts.inputSats;
      const serviceable =
        quoteInputSats >= availability.minSats &&
        quoteInputSats <= availability.maxSats &&
        availability.sides.some(
          (source) =>
            quoteInputSats >= source.minSats &&
            quoteInputSats <= source.maxSats,
        );
      if (!serviceable) {
        return {
          enabled: false,
          refusal: {
            kind: "quote_input_unserviceable",
            inputSats: quoteInputSats,
            minimumSats: availability.minSats,
            maximumSats: availability.maxSats,
            swpError: "swp_invalid_amount",
          },
        };
      }
      fundableInputSats = quoteInputSats;
    }
    // A pinned feed gates funding until the requester's own fetch has
    // verified it (MKT-SWP §3.4): unchecked fails closed, a failed check
    // refuses with its §17 identifier.
    const feed = state.quote.priceFeed;
    if (feed.state === "refused") {
      return {
        enabled: false,
        refusal: {
          kind: "price_feed_refused",
          check: feed.check,
          swpError: feed.check.error,
        },
      };
    }
    if (feed.state === "unchecked") {
      return { enabled: false, refusal: { kind: "price_feed_unchecked" } };
    }
  }

  return { enabled: true, side, amountSats, fundableInputSats };
};
