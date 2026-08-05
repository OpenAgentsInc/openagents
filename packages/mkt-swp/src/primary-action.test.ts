// Oracles for the behaviour contract
// `openagents_web.swap_widget.primary_action_law.v1`
// (packages/behavior-contracts/src/market-swap-widget.ts): one test per state
// class, plus the always-rendered law over the whole union.
import { describe, expect, test } from "vite-plus/test";
import { catalogFor, makeCatalog } from "@openagentsinc/swap-i18n";
import { derivePrimaryAction, isPermanentRefusal } from "./primary-action.js";
import { everySampleWidgetState, sampleWidgetStates } from "./testkit.js";
import { SWAP_WIDGET_STATE_TAGS } from "./widget-state.js";

const catalog = catalogFor("en");

describe("primary-action law", () => {
  test("the primary action is always rendered, for every state", () => {
    for (const state of everySampleWidgetState()) {
      for (const denomination of ["sats", "btc"] as const) {
        const action = derivePrimaryAction(state, catalog, denomination);
        expect(action.label.length).toBeGreaterThan(0);
      }
    }
  });

  test("a partial locale still renders every label — never a blank action", () => {
    // SWAP-8's back-fill law, exercised through the widget's own keys.
    const partial = makeCatalog({ "swap.widget.action.create": "Créer" });
    for (const state of everySampleWidgetState()) {
      expect(derivePrimaryAction(state, partial, "sats").label.length).toBeGreaterThan(0);
    }
    expect(derivePrimaryAction(sampleWidgetStates.Ready, partial, "sats").label).toBe("Créer");
  });

  test("state class: loading states are busy, blocked, and still labelled", () => {
    for (const tag of ["EngineLoading", "PairsLoading", "QuoteRefreshing"] as const) {
      const action = derivePrimaryAction(sampleWidgetStates[tag], catalog, "sats");
      expect(action.busy).toBe(true);
      expect(action.disabled).toBe(true);
      expect(action.label.length).toBeGreaterThan(0);
    }
  });

  test("state class: offline is a danger refusal, never a spinner", () => {
    const action = derivePrimaryAction(sampleWidgetStates.Offline, catalog, "sats");
    expect(action.tone).toBe("danger");
    expect(action.busy).toBe(false);
    expect(action.disabled).toBe(true);
  });

  test("state class: a permanent refusal never spins", () => {
    for (const tag of ["EngineFailed", "NoOfferings", "UnsupportedDirection"] as const) {
      const state = sampleWidgetStates[tag];
      expect(isPermanentRefusal(state)).toBe(true);
      const action = derivePrimaryAction(state, catalog, "sats");
      expect(action.busy).toBe(false);
      expect(action.disabled).toBe(true);
      expect(action.tone).toBe("danger");
    }
  });

  test("NoOfferings names the permanent absence instead of reusing the loading label", () => {
    const action = derivePrimaryAction(sampleWidgetStates.NoOfferings, catalog, "sats");
    expect(action.messageKey).toBe("swap.widget.no_offerings");
    expect(action.label).toBe("No providers are offering this pair.");
    expect(action.label).not.toBe(
      derivePrimaryAction(sampleWidgetStates.PairsLoading, catalog, "sats").label,
    );
  });

  test("state class: amount refusals state the bound in the user's current units", () => {
    const belowSats = derivePrimaryAction(sampleWidgetStates.BelowMinimum, catalog, "sats");
    expect(belowSats.label).toContain("10000 sats");
    expect(belowSats.swpError).toBe("swp_invalid_amount");
    const belowBtc = derivePrimaryAction(sampleWidgetStates.BelowMinimum, catalog, "btc");
    expect(belowBtc.label).toContain("0.0001");
    expect(belowBtc.label).toContain("BTC");
    const aboveBtc = derivePrimaryAction(sampleWidgetStates.AboveMaximum, catalog, "btc");
    expect(aboveBtc.label).toContain("1");
    expect(aboveBtc.label).toContain("BTC");
    const gap = derivePrimaryAction(sampleWidgetStates.CoverageGap, catalog, "sats");
    expect(gap.label).toContain("50000");
    expect(gap.label).toContain("200000");
    // Zero output is explained without blame: the fees, not the user.
    const zero = derivePrimaryAction(sampleWidgetStates.ZeroOutput, catalog, "sats");
    expect(zero.tone).toBe("neutral");
    expect(zero.disabled).toBe(true);
  });

  test("state class: a comma decimal separator is honoured in the refusal", () => {
    const commaBtc = derivePrimaryAction(sampleWidgetStates.BelowMinimum, catalog, "btc", ",");
    expect(commaBtc.label).toContain("0,0001");
  });

  test("state class: destination refusals are labelled, typed, and blocked", () => {
    expect(derivePrimaryAction(sampleWidgetStates.NoDestination, catalog, "sats").disabled).toBe(
      true,
    );
    const invalid = derivePrimaryAction(sampleWidgetStates.InvalidDestination, catalog, "sats");
    expect(invalid.tone).toBe("danger");
    expect(invalid.disabled).toBe(true);
    expect(invalid.swpError).toBe("swp_invoice_invalid");
  });

  test("state class: quote refusals are typed, blocked, and never spin", () => {
    for (const tag of ["QuoteFailed", "QuoteExpired"] as const) {
      const action = derivePrimaryAction(sampleWidgetStates[tag], catalog, "sats");
      expect(action.tone).toBe("danger");
      expect(action.disabled).toBe(true);
      expect(action.busy).toBe(false);
    }
  });

  test("state class: the fund action is unreachable while verification is unresolved or failed", () => {
    const pending = derivePrimaryAction(sampleWidgetStates.VerificationPending, catalog, "sats");
    expect(pending.disabled).toBe(true);
    expect(pending.busy).toBe(true);
    const failed = derivePrimaryAction(sampleWidgetStates.VerificationFailed, catalog, "sats");
    expect(failed.disabled).toBe(true);
    expect(failed.tone).toBe("danger");
    expect(failed.swpError).toBe("swp_script_invalid");
  });

  test("state class: only Ready and AwaitingFunding are pressable", () => {
    for (const tag of SWAP_WIDGET_STATE_TAGS) {
      const action = derivePrimaryAction(sampleWidgetStates[tag], catalog, "sats");
      if (tag === "Ready" || tag === "AwaitingFunding") {
        expect(action.disabled).toBe(false);
        expect(action.tone).toBe("accent");
      } else {
        expect(action.disabled).toBe(true);
      }
    }
  });

  test("state class: Ordering is blocked without a fresh explanation", () => {
    // Disabled is computed separately from the label: the in-flight submit
    // keeps the action label and spins instead of inventing a new reason.
    const ready = derivePrimaryAction(sampleWidgetStates.Ready, catalog, "sats");
    const ordering = derivePrimaryAction(sampleWidgetStates.Ordering, catalog, "sats");
    expect(ordering.messageKey).toBe(ready.messageKey);
    expect(ordering.disabled).toBe(true);
    expect(ordering.busy).toBe(true);
    expect(ready.disabled).toBe(false);
    expect(ready.busy).toBe(false);
  });

  test("state class: in-flight and terminal states stay rendered and blocked", () => {
    for (const tag of [
      "FundingObserved",
      "Executing",
      "SettlementPending",
      "Completed",
      "RefundPending",
      "Refunded",
      "Disputed",
      "Failed",
      "Unresolved",
    ] as const) {
      const action = derivePrimaryAction(sampleWidgetStates[tag], catalog, "sats");
      expect(action.label.length).toBeGreaterThan(0);
      expect(action.disabled).toBe(true);
      expect(action.busy).toBe(false);
    }
  });

  test("no counterparty prose reaches the label: every refusal resolves a typed key", () => {
    for (const state of everySampleWidgetState()) {
      const action = derivePrimaryAction(state, catalog, "sats");
      expect(action.messageKey.startsWith("swap.")).toBe(true);
    }
  });
});
