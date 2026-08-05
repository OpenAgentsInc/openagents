/**
 * Render-ready view models for the pair selector, the amount fields with
 * their pre-typed limits, the primary-action gate, and the rate/fee panel
 * (openagents#9316 §1, §4, §5, §6). The SWAP-0 shell maps these straight
 * to markup; no presentation decision is left implicit in the reducer.
 */
import type { Catalog } from "@openagentsinc/swap-i18n";

import { formatAmountText } from "./amount.js";
import { labelFor, swapTypeOf, type AssetLabels, type SwapDirection } from "./asset.js";
import {
  directionAvailability,
  type DirectionUnreachableReason,
} from "./corpus.js";
import {
  DIRECTION_UNREACHABLE_MESSAGES,
  EMPTY_CORPUS_MESSAGE,
  FEE_PROMISE_MESSAGE,
  PAIR_NOTICE_MESSAGES,
  denominationLabel,
  primaryActionRefusalMessage,
  type PairMessage,
} from "./messages.js";
import { priceFeedProvenanceView, type PriceFeedProvenanceView } from "./price-feed.js";
import { verifyQuoteTerms } from "./quote.js";
import {
  primaryActionGate,
  selectedAvailability,
  selectedDirection,
  type AmountSide,
  type PairSelectionState,
} from "./selection.js";

/** One selectable asset on one side, with reachability disclosed up front. */
export interface AssetOptionView {
  readonly assetId: string;
  readonly labels: AssetLabels;
  /** Choosing this option swaps the sides (it is the other side's asset). */
  readonly wouldSwapSides: boolean;
  /**
   * Reachability of the direction this choice would produce (with the
   * currently selected counterpart). `reachable` is null while the other
   * side is unselected.
   */
  readonly reachability:
    | { readonly state: "reachable" }
    | {
        readonly state: "unreachable";
        readonly reason: DirectionUnreachableReason;
        readonly message: PairMessage;
        readonly advertisingProviders: readonly string[];
      }
    | { readonly state: "counterpart_unselected" };
}

export interface PairSelectorView {
  readonly corpus:
    | { readonly state: "empty"; readonly message: PairMessage }
    | {
        readonly state: "populated";
        readonly foldedAtSeconds: number;
        readonly droppedSideCount: number;
      };
  readonly inputOptions: readonly AssetOptionView[];
  readonly outputOptions: readonly AssetOptionView[];
  readonly selected: SwapDirection | null;
  readonly swapType: "submarine" | "reverse" | "chain" | null;
  readonly notices: readonly PairMessage[];
}

const optionsFor = (
  state: PairSelectionState,
  side: AmountSide,
): readonly AssetOptionView[] => {
  if (state.fold.state !== "populated") return [];
  const ownSelected =
    side === "input" ? state.inputAssetId : state.outputAssetId;
  const counterpart =
    side === "input" ? state.outputAssetId : state.inputAssetId;
  return state.fold.assets.map((asset) => {
    const wouldSwapSides =
      counterpart !== null &&
      asset.assetId === counterpart &&
      asset.assetId !== ownSelected;
    let reachability: AssetOptionView["reachability"];
    if (counterpart === null) {
      reachability = { state: "counterpart_unselected" };
    } else {
      const direction: SwapDirection = wouldSwapSides
        ? side === "input"
          ? { inputAssetId: asset.assetId, outputAssetId: ownSelected ?? "" }
          : { inputAssetId: ownSelected ?? "", outputAssetId: asset.assetId }
        : side === "input"
          ? { inputAssetId: asset.assetId, outputAssetId: counterpart }
          : { inputAssetId: counterpart, outputAssetId: asset.assetId };
      const availability = directionAvailability(state.fold, direction);
      reachability = availability.reachable
        ? { state: "reachable" }
        : {
            state: "unreachable",
            reason: availability.reason,
            message: DIRECTION_UNREACHABLE_MESSAGES[availability.reason],
            advertisingProviders: availability.advertisingProviders,
          };
    }
    return {
      assetId: asset.assetId,
      labels: labelFor(asset),
      wouldSwapSides,
      reachability,
    };
  });
};

export const pairSelectorView = (
  state: PairSelectionState,
): PairSelectorView => {
  const selected = selectedDirection(state);
  return {
    corpus:
      state.fold.state === "empty"
        ? { state: "empty", message: EMPTY_CORPUS_MESSAGE }
        : {
            state: "populated",
            foldedAtSeconds: state.fold.foldedAtSeconds,
            droppedSideCount: state.fold.droppedSideCount,
          },
    inputOptions: optionsFor(state, "input"),
    outputOptions: optionsFor(state, "output"),
    selected,
    swapType: selected === null ? null : swapTypeOf(selected),
    notices: state.notices.map((n) => PAIR_NOTICE_MESSAGES[n.notice]),
  };
};

/**
 * The applicable limits, shown *before* the user types (issue §4), in the
 * user's current denomination. Null while no reachable direction is
 * selected — the unreachable state carries its own message.
 */
export interface LimitsView {
  readonly minimum: string;
  readonly maximum: string;
  readonly denomination: string;
  readonly minimumSats: bigint;
  readonly maximumSats: bigint;
  readonly feeBpsMin: bigint;
  readonly feeBpsMax: bigint;
  readonly freshestObservedAtSeconds: number;
  readonly providerCount: number;
}

export const limitsView = (state: PairSelectionState): LimitsView | null => {
  const availability = selectedAvailability(state);
  if (availability === null || !availability.reachable) return null;
  return {
    minimum: formatAmountText(
      availability.minSats,
      state.denomination,
      state.decimalSeparator,
    ),
    maximum: formatAmountText(
      availability.maxSats,
      state.denomination,
      state.decimalSeparator,
    ),
    denomination: denominationLabel(state.denomination),
    minimumSats: availability.minSats,
    maximumSats: availability.maxSats,
    feeBpsMin: availability.feeBpsMin,
    feeBpsMax: availability.feeBpsMax,
    freshestObservedAtSeconds: availability.freshestObservedAtSeconds,
    providerCount: new Set(
      availability.sides.map((side) => side.providerAddress),
    ).size,
  };
};

/**
 * The primary action's view: enabled, or disabled with exactly one message
 * — the most proximate refusal, limits in the current denomination
 * (SWAP-0 primary-action law).
 */
export type PrimaryActionView =
  | { readonly enabled: true; readonly amountSats: bigint }
  | {
      readonly enabled: false;
      readonly messageKey: string;
      readonly message: string;
      /** MKT-SWP §17 identifier when the refusal carries one. */
      readonly swpError: string | null;
    };

export const primaryActionView = (
  state: PairSelectionState,
  catalog: Catalog,
): PrimaryActionView => {
  const gate = primaryActionGate(state);
  if (gate.enabled) return gate;
  const rendered = primaryActionRefusalMessage(
    catalog,
    gate.refusal,
    state.denomination,
    state.decimalSeparator,
  );
  return {
    enabled: false,
    messageKey: rendered.key,
    message: rendered.message,
    swpError: "swpError" in gate.refusal ? gate.refusal.swpError : null,
  };
};

/** One row of the expanded fee breakdown. */
export interface FeeRowView {
  readonly component:
    | "provider_fee"
    | "miner_fee_budget"
    | "lightning_routing_fee_budget";
  readonly amountSats: bigint;
  readonly amount: string;
  readonly paidBy: "requester" | "provider";
}

export type FeePanelView =
  | { readonly state: "no_quote" }
  | {
      readonly state: "refused";
      /** The §17 identifier; the panel never renders unreproduced terms. */
      readonly swpError: string;
      readonly detail: string;
    }
  | {
      readonly state: "verified";
      readonly collapsed: {
        readonly totalFeeSats: bigint;
        readonly totalFee: string;
        readonly feeBps: bigint;
        readonly denomination: string;
      };
      readonly rows: readonly FeeRowView[];
      readonly roundingRule: "floor_output_sats";
      readonly amountEquation: string;
      /** The fill-promise framing (MKT-SWP §3.3), always rendered. */
      readonly promise: PairMessage;
      readonly inputSats: bigint;
      readonly outputSats: bigint;
      readonly priceFeed: PriceFeedProvenanceView | null;
    };

export const feePanelView = (state: PairSelectionState): FeePanelView => {
  if (state.quote === null) return { state: "no_quote" };
  if (state.quote.status === "refused") {
    return {
      state: "refused",
      swpError: state.quote.refusal.error,
      detail: state.quote.refusal.detail,
    };
  }
  const terms = state.quote.terms;
  const verification = verifyQuoteTerms(terms);
  if (!verification.ok) {
    // Unreachable for a held verified quote, but fail closed rather than
    // rendering an unreproduced promise.
    return {
      state: "refused",
      swpError: verification.error,
      detail: verification.detail,
    };
  }
  const amounts = verification.amounts;
  const format = (sats: bigint): string =>
    formatAmountText(sats, state.denomination, state.decimalSeparator);
  return {
    state: "verified",
    collapsed: {
      totalFeeSats: amounts.maximumTotalFeeSats,
      totalFee: format(amounts.maximumTotalFeeSats),
      feeBps: amounts.feeBps,
      denomination: denominationLabel(state.denomination),
    },
    rows: [
      {
        component: "provider_fee",
        amountSats: amounts.providerFeeSats,
        amount: format(amounts.providerFeeSats),
        paidBy: terms.feePayer,
      },
      {
        component: "miner_fee_budget",
        amountSats: amounts.minerFeeBudgetSats,
        amount: format(amounts.minerFeeBudgetSats),
        paidBy: terms.feePayer,
      },
      {
        component: "lightning_routing_fee_budget",
        amountSats: amounts.lightningRoutingFeeBudgetSats,
        amount: format(amounts.lightningRoutingFeeBudgetSats),
        paidBy: terms.feePayer,
      },
    ],
    roundingRule: "floor_output_sats",
    amountEquation: amounts.amountEquation,
    promise: FEE_PROMISE_MESSAGE,
    inputSats: amounts.inputSats,
    outputSats: amounts.outputSats,
    priceFeed:
      terms.priceFeed === null ? null : priceFeedProvenanceView(terms.priceFeed),
  };
};
