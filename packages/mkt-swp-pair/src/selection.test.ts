/**
 * Behaviour-contract oracles for pair selection (registry:
 * `@openagentsinc/behavior-contracts`, market-swap-pair):
 * - openagents_web.swap_pair.unreachable_direction_disclosed.v1
 * - openagents_web.swap_pair.no_auto_unit_switch.v1
 */
import { describe, expect, test } from "vite-plus/test";

import { makeCatalog } from "@openagentsinc/swap-i18n";

import {
  AMOUNT_PARSE_FAILURE_MESSAGES,
  DIRECTION_UNREACHABLE_MESSAGES,
  EMPTY_CORPUS_MESSAGE,
  PAIR_NOTICE_MESSAGES,
  primaryActionRefusalMessage,
} from "./messages.js";
import {
  initialPairSelectionState,
  primaryActionGate,
  reducePairEvent,
  selectedDirection,
  type PairEvent,
  type PairSelectionState,
} from "./selection.js";
import {
  TEST_CHAIN_ASSET,
  TEST_FOLD_NOW,
  TEST_FRESHNESS_HORIZON,
  TEST_LIGHTNING_ASSET,
  testOffering,
  testSide,
} from "./testkit.js";
import { limitsView, pairSelectorView, primaryActionView } from "./view.js";

const catalog = makeCatalog();

const config = {
  nowSeconds: TEST_FOLD_NOW,
  freshnessHorizonSeconds: TEST_FRESHNESS_HORIZON,
};

const reduce = (
  state: PairSelectionState,
  ...events: readonly PairEvent[]
): PairSelectionState => events.reduce(reducePairEvent, state);

const populated = (): PairSelectionState =>
  reduce(initialPairSelectionState(), {
    type: "corpus_updated",
    offerings: [testOffering()],
    config,
  });

describe("corpus-driven selection", () => {
  test("seeds the first reachable direction when nothing is selected", () => {
    const state = populated();
    expect(selectedDirection(state)).toEqual({
      inputAssetId: TEST_CHAIN_ASSET,
      outputAssetId: TEST_LIGHTNING_ASSET,
    });
  });

  test("an existing selection survives a corpus update that removes its direction", () => {
    const state = reduce(populated(), {
      type: "corpus_updated",
      offerings: [testOffering({ sides: [testSide({ max: "0" })] })],
      config,
    });
    // Selection kept; it renders unreachable with its reason instead of
    // being silently reset (unreachable_direction_disclosed.v1).
    expect(selectedDirection(state)).not.toBe(null);
    const gate = primaryActionGate(state);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) {
      expect(gate.refusal).toMatchObject({
        kind: "direction_unreachable",
        reason: "side_disabled",
        swpError: "swp_side_disabled",
      });
    }
  });

  test("empty corpus gates the primary action as a typed state", () => {
    const gate = primaryActionGate(initialPairSelectionState());
    expect(gate).toEqual({ enabled: false, refusal: { kind: "empty_corpus" } });
    const view = primaryActionView(initialPairSelectionState(), catalog);
    expect(view).toMatchObject({
      enabled: false,
      messageKey: EMPTY_CORPUS_MESSAGE.key,
    });
  });
});

describe("selecting the counterparty's asset swaps the sides (teardown §3.1)", () => {
  test("input side", () => {
    const state = reduce(populated(), {
      type: "input_asset_selected",
      assetId: TEST_LIGHTNING_ASSET,
    });
    expect(selectedDirection(state)).toEqual({
      inputAssetId: TEST_LIGHTNING_ASSET,
      outputAssetId: TEST_CHAIN_ASSET,
    });
    expect(state.notices).toContainEqual({ notice: "sides_swapped" });
  });

  test("output side", () => {
    const state = reduce(populated(), {
      type: "output_asset_selected",
      assetId: TEST_CHAIN_ASSET,
    });
    expect(selectedDirection(state)).toEqual({
      inputAssetId: TEST_LIGHTNING_ASSET,
      outputAssetId: TEST_CHAIN_ASSET,
    });
    expect(state.notices).toContainEqual({ notice: "sides_swapped" });
  });

  test("direction toggle swaps and the selector view marks the swap option", () => {
    const state = reduce(populated(), { type: "direction_toggled" });
    expect(selectedDirection(state)).toEqual({
      inputAssetId: TEST_LIGHTNING_ASSET,
      outputAssetId: TEST_CHAIN_ASSET,
    });
    const view = pairSelectorView(populated());
    const swapOption = view.inputOptions.find(
      (option) => option.assetId === TEST_LIGHTNING_ASSET,
    );
    expect(swapOption?.wouldSwapSides).toBe(true);
  });

  test("the swapped-to reverse direction discloses its unreachability up front", () => {
    // Only submarine is advertised; reverse must render unreachable with
    // no_offering before the user commits to it.
    const view = pairSelectorView(populated());
    const reverseOption = view.inputOptions.find(
      (option) => option.assetId === TEST_LIGHTNING_ASSET,
    );
    expect(reverseOption?.reachability).toMatchObject({
      state: "unreachable",
      reason: "no_offering",
    });
    const state = reduce(populated(), { type: "direction_toggled" });
    const gate = primaryActionGate(state);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) {
      expect(gate.refusal).toMatchObject({
        kind: "direction_unreachable",
        reason: "no_offering",
      });
    }
  });
});

describe("limits render before the user types (issue §4)", () => {
  test("limitsView carries the folded min/max in the current denomination", () => {
    const state = populated();
    expect(state.amountText.input).toBe("");
    const limits = limitsView(state);
    expect(limits).toMatchObject({
      minimum: "0.0001",
      maximum: "0.01",
      denomination: "BTC",
      minimumSats: 10_000n,
      maximumSats: 1_000_000n,
      providerCount: 1,
    });
    const inSats = reduce(state, { type: "denomination_toggled" });
    expect(limitsView(inSats)).toMatchObject({
      minimum: "10000",
      maximum: "1000000",
      denomination: "sats",
    });
  });
});

describe("amount validation states the most proximate refusal in the current denomination", () => {
  test("below minimum, BTC denomination", () => {
    const state = reduce(populated(), {
      type: "amount_edited",
      side: "input",
      text: "0.00005",
    });
    const view = primaryActionView(state, catalog);
    expect(view).toMatchObject({
      enabled: false,
      messageKey: "swap.refusal.below_minimum",
      swpError: "swp_invalid_amount",
    });
    if (!view.enabled) {
      expect(view.message).toBe(
        "Below the minimum: enter at least 0.0001 BTC.",
      );
    }
  });

  test("above maximum, sats denomination", () => {
    const state = reduce(
      populated(),
      { type: "denomination_toggled" },
      { type: "amount_edited", side: "input", text: "1000001" },
    );
    const view = primaryActionView(state, catalog);
    expect(view).toMatchObject({
      enabled: false,
      messageKey: "swap.refusal.above_maximum",
      swpError: "swp_invalid_amount",
    });
    if (!view.enabled) {
      expect(view.message).toBe(
        "Above the maximum: enter at most 1000000 sats.",
      );
    }
  });

  test("limit boundaries are exact: min and max are enabled, min-1 and max+1 refused", () => {
    const base = reduce(populated(), { type: "denomination_toggled" });
    const at = (text: string) =>
      primaryActionGate(
        reduce(base, { type: "amount_edited", side: "input", text }),
      );
    expect(at("10000")).toEqual({ enabled: true, amountSats: 10_000n });
    expect(at("1000000")).toEqual({ enabled: true, amountSats: 1_000_000n });
    expect(at("9999").enabled).toBe(false);
    expect(at("1000001").enabled).toBe(false);
  });

  test("a coverage gap between providers is disclosed, not misreported as min/max", () => {
    const state = reduce(
      initialPairSelectionState(),
      {
        type: "corpus_updated",
        offerings: [
          testOffering({ sides: [testSide({ min: "1000", max: "2000" })] }),
          testOffering({ sides: [testSide({ min: "50000", max: "90000" })] }),
        ],
        config,
      },
      { type: "denomination_toggled" },
      { type: "amount_edited", side: "input", text: "10000" },
    );
    const gate = primaryActionGate(state);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) {
      expect(gate.refusal).toEqual({
        kind: "coverage_gap",
        nearestBelowSats: 2_000n,
        nearestAboveSats: 50_000n,
        swpError: "swp_invalid_amount",
      });
      const rendered = primaryActionRefusalMessage(
        catalog,
        gate.refusal,
        "sats",
        ".",
      );
      expect(rendered.message).toBe(
        "No single provider covers this amount — enter at most 2000 sats or at least 50000 sats.",
      );
    }
  });

  test("an unparseable amount refuses with its own mode, not a limit message", () => {
    const state = reduce(populated(), {
      type: "amount_edited",
      side: "input",
      text: "1.2.3",
    });
    const gate = primaryActionGate(state);
    expect(gate).toEqual({
      enabled: false,
      refusal: { kind: "amount_unparseable", mode: "multiple_separators" },
    });
  });

  test("output-side entry is bounded by the offering maximum pre-quote", () => {
    const state = reduce(
      populated(),
      { type: "denomination_toggled" },
      { type: "amount_edited", side: "output", text: "1000001" },
    );
    const gate = primaryActionGate(state);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) expect(gate.refusal.kind).toBe("above_maximum");
  });
});

describe("MAX affordance", () => {
  test("MAX fills the input with the offering maximum in the current denomination", () => {
    const state = reduce(populated(), { type: "max_requested" });
    expect(state.amountText.input).toBe("0.01");
    expect(state.authoritativeSide).toBe("input");
    expect(primaryActionGate(state)).toEqual({
      enabled: true,
      amountSats: 1_000_000n,
    });
  });

  test("MAX is a no-op while the direction is unreachable", () => {
    const state = reduce(
      initialPairSelectionState(),
      {
        type: "corpus_updated",
        offerings: [testOffering({ sides: [testSide({ max: "0" })] })],
        config,
      },
      { type: "input_asset_selected", assetId: TEST_CHAIN_ASSET },
      { type: "output_asset_selected", assetId: TEST_LIGHTNING_ASSET },
      { type: "max_requested" },
    );
    expect(state.amountText.input).toBe("");
  });
});

describe("no_auto_unit_switch.v1: denomination changes only on the explicit toggle", () => {
  test("typing never changes the denomination", () => {
    let state = populated();
    expect(state.denomination).toBe("btc");
    for (const text of ["0.5", "50000", "21000000", "1.2.3", "abc", ""]) {
      state = reduce(state, { type: "amount_edited", side: "input", text });
      expect(state.denomination).toBe("btc");
      expect(state.amountText.input).toBe(text);
    }
  });

  test("sats text with a separator is refused, never reinterpreted as BTC", () => {
    const state = reduce(
      populated(),
      { type: "denomination_toggled" },
      { type: "amount_edited", side: "input", text: "0.5" },
    );
    expect(state.denomination).toBe("sats");
    const gate = primaryActionGate(state);
    expect(gate).toEqual({
      enabled: false,
      refusal: { kind: "amount_unparseable", mode: "sats_fractional" },
    });
  });

  test("the explicit toggle converts the parsed value exactly, both ways", () => {
    const btc = reduce(populated(), {
      type: "amount_edited",
      side: "input",
      text: "0.0015",
    });
    const sats = reduce(btc, { type: "denomination_toggled" });
    expect(sats.denomination).toBe("sats");
    expect(sats.amountText.input).toBe("150000");
    const backToBtc = reduce(sats, { type: "denomination_toggled" });
    expect(backToBtc.amountText.input).toBe("0.0015");
  });

  test("unparseable text survives the toggle verbatim instead of being reinterpreted", () => {
    const state = reduce(
      populated(),
      { type: "amount_edited", side: "input", text: "not-an-amount" },
      { type: "denomination_toggled" },
    );
    expect(state.amountText.input).toBe("not-an-amount");
    expect(state.denomination).toBe("sats");
  });

  test("decimal separator preference reformats exactly", () => {
    const state = reduce(
      populated(),
      { type: "amount_edited", side: "input", text: "0.0015" },
      { type: "decimal_separator_selected", separator: "," },
    );
    expect(state.decimalSeparator).toBe(",");
    expect(state.amountText.input).toBe("0,0015");
  });
});

describe("message distinctness", () => {
  test("no two pair messages collapse into one rendered string", () => {
    const messages = [
      ...Object.values(DIRECTION_UNREACHABLE_MESSAGES),
      ...Object.values(AMOUNT_PARSE_FAILURE_MESSAGES),
      ...Object.values(PAIR_NOTICE_MESSAGES),
      EMPTY_CORPUS_MESSAGE,
    ];
    const texts = messages.map((m) => m.message);
    expect(new Set(texts).size).toBe(texts.length);
    const keys = messages.map((m) => m.key);
    expect(new Set(keys).size).toBe(keys.length);
  });
});
