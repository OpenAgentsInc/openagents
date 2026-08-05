/**
 * Behaviour-contract oracle for the fee panel's promise framing
 * (registry: `@openagentsinc/behavior-contracts`, market-swap-pair):
 * - openagents_web.swap_pair.fee_output_promise.v1
 */
import { describe, expect, test } from "vite-plus/test";

import { FEE_PROMISE_MESSAGE } from "./messages.js";
import { verifyQuoteTerms, type SwapQuoteTerms } from "./quote.js";
import {
  initialPairSelectionState,
  reducePairEvent,
  type PairEvent,
  type PairSelectionState,
} from "./selection.js";
import { TEST_FOLD_NOW, TEST_FRESHNESS_HORIZON, testOffering } from "./testkit.js";
import { feePanelView } from "./view.js";

/**
 * Exact fixture: input 100000, fee_bps 25 → provider_fee floor(100000 *
 * 25 / 10000) = 250; miner 1200; routing 30; output 100000 - 250 - 1200 -
 * 30 = 98520; maximum_total_fee 1480.
 */
const goodTerms: SwapQuoteTerms = {
  inputAmount: "100000",
  outputAmount: "98520",
  feeBps: "25",
  providerFee: "250",
  minerFeeBudget: "1200",
  lightningRoutingFeeBudget: "30",
  maximumTotalFee: "1480",
  feePayer: "requester",
  rounding: "floor_output_sats",
  amountEquation: "input_minus_provider_and_quoted_fees",
  priceFeed: null,
};

describe("verifyQuoteTerms reproduces the output from the quoted terms", () => {
  test("a conforming quote verifies with exact bigint components", () => {
    const result = verifyQuoteTerms(goodTerms);
    expect(result).toEqual({
      ok: true,
      amounts: {
        inputSats: 100_000n,
        outputSats: 98_520n,
        feeBps: 25n,
        providerFeeSats: 250n,
        minerFeeBudgetSats: 1_200n,
        lightningRoutingFeeBudgetSats: 30n,
        maximumTotalFeeSats: 1_480n,
        amountEquation: "input_minus_provider_and_quoted_fees",
      },
    });
  });

  const mismatches: readonly {
    readonly name: string;
    readonly terms: SwapQuoteTerms;
    readonly error: string;
  }[] = [
    {
      name: "output not reproducing from the fee components",
      terms: { ...goodTerms, outputAmount: "98521" },
      error: "swp_amount_equation_mismatch",
    },
    {
      name: "provider fee not floor(input * fee_bps / 10000)",
      terms: { ...goodTerms, providerFee: "251" },
      error: "swp_amount_equation_mismatch",
    },
    {
      name: "rounding rule other than floor_output_sats",
      terms: { ...goodTerms, rounding: "round_half_even" },
      error: "swp_amount_equation_mismatch",
    },
    {
      name: "amount equation outside the v1 allowlist",
      terms: { ...goodTerms, amountEquation: "market_rate" },
      error: "swp_amount_equation_mismatch",
    },
    {
      name: "maximum_total_fee not the component sum",
      terms: { ...goodTerms, maximumTotalFee: "1481" },
      error: "swp_amount_equation_mismatch",
    },
    {
      name: "fees consuming the whole input (output would be zero)",
      terms: {
        ...goodTerms,
        inputAmount: "1233",
        outputAmount: "0",
        providerFee: "3",
        maximumTotalFee: "1233",
      },
      error: "swp_amount_equation_mismatch",
    },
    {
      name: "non-canonical amount (float string)",
      terms: { ...goodTerms, inputAmount: "100000.0" },
      error: "swp_invalid_amount",
    },
    {
      name: "non-canonical amount (leading zero)",
      terms: { ...goodTerms, minerFeeBudget: "01200" },
      error: "swp_invalid_amount",
    },
    {
      name: "fee_bps above 10000",
      terms: { ...goodTerms, feeBps: "10001" },
      error: "swp_invalid_fee",
    },
  ];
  for (const c of mismatches) {
    test(`refuses ${c.name}`, () => {
      const result = verifyQuoteTerms(c.terms);
      expect(result.ok).toBe(false);
      if (!result.ok) expect(result.error).toBe(c.error);
    });
  }

  test("both allowlisted equations verify with the same arithmetic", () => {
    const oneToOne = verifyQuoteTerms({
      ...goodTerms,
      amountEquation: "one_to_one_less_quoted_fees",
    });
    expect(oneToOne.ok).toBe(true);
  });
});

describe("fee_output_promise.v1: the panel renders the promise, never an unreproduced number", () => {
  const config = {
    nowSeconds: TEST_FOLD_NOW,
    freshnessHorizonSeconds: TEST_FRESHNESS_HORIZON,
  };
  const reduce = (
    state: PairSelectionState,
    ...events: readonly PairEvent[]
  ): PairSelectionState => events.reduce(reducePairEvent, state);
  const withCorpus = (): PairSelectionState =>
    reduce(initialPairSelectionState(), {
      type: "corpus_updated",
      offerings: [testOffering()],
      config,
    });

  test("a verified quote renders separated components, payer, rounding, equation, and the promise framing", () => {
    const state = reduce(withCorpus(), {
      type: "quote_applied",
      terms: goodTerms,
    });
    const panel = feePanelView(state);
    expect(panel.state).toBe("verified");
    if (panel.state === "verified") {
      expect(panel.collapsed.totalFeeSats).toBe(1_480n);
      expect(panel.rows).toEqual([
        {
          component: "provider_fee",
          amountSats: 250n,
          amount: "0.0000025",
          paidBy: "requester",
        },
        {
          component: "miner_fee_budget",
          amountSats: 1_200n,
          amount: "0.000012",
          paidBy: "requester",
        },
        {
          component: "lightning_routing_fee_budget",
          amountSats: 30n,
          amount: "0.0000003",
          paidBy: "requester",
        },
      ]);
      expect(panel.roundingRule).toBe("floor_output_sats");
      expect(panel.amountEquation).toBe("input_minus_provider_and_quoted_fees");
      expect(panel.promise).toEqual(FEE_PROMISE_MESSAGE);
      expect(panel.priceFeed).toBe(null);
    }
  });

  test("a quote whose output does not reproduce surfaces swp_amount_equation_mismatch", () => {
    const state = reduce(withCorpus(), {
      type: "quote_applied",
      terms: { ...goodTerms, outputAmount: "98521" },
    });
    const panel = feePanelView(state);
    expect(panel).toMatchObject({
      state: "refused",
      swpError: "swp_amount_equation_mismatch",
    });
    expect(state.quote?.status).toBe("refused");
  });

  test("a verified quote fills the derived side with the promised output", () => {
    const state = reduce(
      withCorpus(),
      { type: "denomination_toggled" },
      { type: "amount_edited", side: "input", text: "100000" },
      { type: "quote_applied", terms: goodTerms },
    );
    expect(state.amountText.output).toBe("98520");
  });

  test("editing the amount clears the held quote with an explicit notice", () => {
    const state = reduce(
      withCorpus(),
      { type: "quote_applied", terms: goodTerms },
      { type: "amount_edited", side: "input", text: "0.002" },
    );
    expect(state.quote).toBe(null);
    expect(state.notices).toEqual([
      { notice: "quote_cleared_by_amount_edit" },
    ]);
    expect(feePanelView(state)).toEqual({ state: "no_quote" });
  });

  test("changing direction clears the held quote with an explicit notice", () => {
    const state = reduce(
      withCorpus(),
      { type: "quote_applied", terms: goodTerms },
      { type: "direction_toggled" },
    );
    expect(state.quote).toBe(null);
    expect(state.notices).toContainEqual({
      notice: "quote_cleared_by_direction_change",
    });
  });

  test("a pinned price feed renders its provenance verbatim", () => {
    const state = reduce(withCorpus(), {
      type: "quote_applied",
      terms: {
        ...goodTerms,
        priceFeed: {
          url: "https://feed.example/rate",
          valuePointer: "/data/value",
          observedValue: "100000000",
          observedAtSeconds: TEST_FOLD_NOW - 10,
          maxAgeSeconds: 30,
          responseSha256: "ab".repeat(32),
        },
      },
    });
    const panel = feePanelView(state);
    expect(panel.state).toBe("verified");
    if (panel.state === "verified") {
      expect(panel.priceFeed).toEqual({
        url: "https://feed.example/rate",
        jsonPointer: "/data/value",
        observedValue: "100000000",
        observedAtSeconds: TEST_FOLD_NOW - 10,
        maxAgeSeconds: 30,
        responseSha256: "ab".repeat(32),
      });
    }
  });
});
