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
import { verifyQuoteTerms, type QuoteTermsVerification, type SwapQuoteTerms } from "./quote.js";

export type AmountSide = "input" | "output";

export type PairNotice =
  | { readonly notice: "sides_swapped" }
  | { readonly notice: "quote_cleared_by_amount_edit" }
  | { readonly notice: "quote_cleared_by_direction_change" };

export type HeldQuote =
  | { readonly status: "verified"; readonly terms: SwapQuoteTerms }
  | {
      readonly status: "refused";
      readonly terms: SwapQuoteTerms;
      readonly refusal: Extract<QuoteTermsVerification, { ok: false }>;
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
      return {
        ...state,
        quote: { status: "verified", terms: event.terms },
        amountText: { ...state.amountText, ...derived },
        notices: [],
      };
    }

    case "quote_cleared":
      return { ...state, quote: null, notices: [] };
  }
};

// ---------------------------------------------------------------------------
// Primary-action gate
// ---------------------------------------------------------------------------

/**
 * The single most proximate refusal, walked in a fixed precedence
 * (SWAP-0 primary-action law): corpus, direction, reachability, amount
 * presence, amount validity, then limits — so the button always states
 * exactly one reason, the nearest one to being fixed by the user.
 */
export type PrimaryActionGate =
  | { readonly enabled: true; readonly amountSats: bigint }
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
      readonly refusal: Extract<QuoteTermsVerification, { ok: false }>;
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

  return { enabled: true, amountSats };
};
