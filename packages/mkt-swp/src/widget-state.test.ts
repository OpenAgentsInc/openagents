// Oracles for the behaviour contracts
// `openagents_web.swap_widget.state_exhaustive_explanation.v1` and
// `openagents_web.swap_widget.funding_gate.v1`
// (packages/behavior-contracts/src/market-swap-widget.ts).
import { describe, expect, test } from "vite-plus/test";
import { catalogFor } from "@openagentsinc/swap-i18n";
import { classifySwpState } from "@openagentsinc/mkt-swp-status";
import { derivePrimaryAction } from "./primary-action.js";
import {
  enabledFundingGate,
  everySampleWidgetState,
  failedFundingGate,
  pendingFundingGate,
  sampleFundingAuthorization,
  sampleWidgetStates,
} from "./testkit.js";
import {
  SWAP_WIDGET_STATE_TAGS,
  SwapWidgetEvent,
  SwapWidgetStateSchema,
  initialSwapWidgetState,
  isFormPhase,
  transitionSwapWidgetState,
} from "./widget-state.js";

const cases = SwapWidgetStateSchema.cases;
const catalog = catalogFor("en");

const sampleEvents: ReadonlyArray<SwapWidgetEvent> = [
  SwapWidgetEvent.FormRederived({ state: cases.Ready.make({}) }),
  SwapWidgetEvent.FormRederived({ state: cases.NoDestination.make({}) }),
  SwapWidgetEvent.SubmitPressed(),
  SwapWidgetEvent.FundingGateChanged({ gate: enabledFundingGate() }),
  SwapWidgetEvent.FundingGateChanged({ gate: pendingFundingGate() }),
  SwapWidgetEvent.FundingGateChanged({
    gate: failedFundingGate({
      id: "script_tree_parsed",
      status: "fail",
      error: "swp_script_invalid",
    }),
  }),
  SwapWidgetEvent.EngineRefused({ identifier: "swp_quote_expired" }),
  SwapWidgetEvent.FundingAuthorized({ authorization: sampleFundingAuthorization }),
  SwapWidgetEvent.SessionAdvanced({ state: "funding_observed" }),
  SwapWidgetEvent.SessionAdvanced({ state: "executing" }),
  SwapWidgetEvent.SessionAdvanced({ state: "completed" }),
  SwapWidgetEvent.SessionAdvanced({ state: "refund_pending" }),
  SwapWidgetEvent.SessionAdvanced({ state: "accepted" }),
  SwapWidgetEvent.SessionAdvanced({ state: "funding_required" }),
];

describe("swap widget typed state", () => {
  test("the union is exhaustive over the enumerated pre-creation, in-flight, and terminal states", () => {
    // Teardown §2.2 enumerates 27 pre-creation states; several are the same
    // widget state under different triggers (three loading states, two
    // MAX/limit resolutions) and several are Boltz-specific (WASM-unsupported
    // page, gas top-up, ERC-20 commitment, embed lock) with no MKT-SWP v1
    // analogue. The set below is the complete coverage of what this profile
    // can reach, and it is checked exhaustively rather than by count.
    expect(SWAP_WIDGET_STATE_TAGS).toHaveLength(31);
    expect([...Object.keys(cases)].sort()).toEqual([...SWAP_WIDGET_STATE_TAGS].sort());
    expect([...Object.keys(sampleWidgetStates)].sort()).toEqual([...SWAP_WIDGET_STATE_TAGS].sort());
  });

  test("no state is reachable without a rendered explanation", () => {
    for (const state of everySampleWidgetState()) {
      for (const denomination of ["sats", "btc"] as const) {
        const action = derivePrimaryAction(state, catalog, denomination);
        expect(action.label.length).toBeGreaterThan(0);
        expect(action.label.includes("undefined")).toBe(false);
      }
    }
  });

  test("every transition is total: any state folds any event to a valid state", () => {
    for (const state of everySampleWidgetState()) {
      for (const event of sampleEvents) {
        const next = transitionSwapWidgetState(state, event);
        expect(SWAP_WIDGET_STATE_TAGS).toContain(next._tag);
      }
    }
  });

  test("reported outcomes can still surface disputed or unresolved risk", () => {
    for (const tag of ["Completed", "Refunded", "Failed"] as const) {
      const state = sampleWidgetStates[tag];
      expect(
        transitionSwapWidgetState(state, SwapWidgetEvent.SessionAdvanced({ state: "unresolved" }))
          ._tag,
      ).toBe("Unresolved");
    }
    expect(
      transitionSwapWidgetState(
        sampleWidgetStates.Unresolved,
        SwapWidgetEvent.SessionAdvanced({ state: "disputed" }),
      )._tag,
    ).toBe("Disputed");
    expect(
      transitionSwapWidgetState(
        sampleWidgetStates.Completed,
        SwapWidgetEvent.FormRederived({ state: sampleWidgetStates.Ready }),
      ),
    ).toEqual(sampleWidgetStates.Completed);
  });

  test("SubmitPressed advances only from Ready", () => {
    for (const state of everySampleWidgetState()) {
      const next = transitionSwapWidgetState(state, SwapWidgetEvent.SubmitPressed());
      if (state._tag === "Ready") expect(next._tag).toBe("Ordering");
      else expect(next).toEqual(state);
    }
  });

  test("AwaitingFunding is reachable only from Ordering with an engine authorization", () => {
    const event = SwapWidgetEvent.FundingAuthorized({
      authorization: sampleFundingAuthorization,
    });
    for (const state of everySampleWidgetState()) {
      const next = transitionSwapWidgetState(state, event);
      if (state._tag === "Ordering") {
        expect(next).toEqual(
          cases.AwaitingFunding.make({ authorization: sampleFundingAuthorization }),
        );
      } else {
        expect(next).toEqual(state);
      }
    }
  });

  test("no session claim advances the widget past the funding gate", () => {
    // A provider Status claiming `funding_required` — or anything else —
    // cannot open the fund action. Only `FundingAuthorized` does.
    for (const claim of ["funding_required", "funding_observed", "completed"] as const) {
      const next = transitionSwapWidgetState(
        sampleWidgetStates.Ordering,
        SwapWidgetEvent.SessionAdvanced({ state: claim }),
      );
      expect(next).toEqual(sampleWidgetStates.Ordering);
    }
  });

  test("SWAP-3's gate cannot enable funding, only keep it disabled", () => {
    // The gate reaching Ready is the pre-check; the fund action still needs
    // the engine's authorization, which no gate value can produce.
    const ready = transitionSwapWidgetState(
      sampleWidgetStates.VerificationPending,
      SwapWidgetEvent.FundingGateChanged({ gate: enabledFundingGate() }),
    );
    expect(ready._tag).toBe("Ready");
    const failed = transitionSwapWidgetState(
      sampleWidgetStates.VerificationPending,
      SwapWidgetEvent.FundingGateChanged({
        gate: failedFundingGate({
          id: "exit_package",
          status: "fail",
          error: "swp_exit_package_missing",
        }),
      }),
    );
    expect(failed).toEqual(
      cases.VerificationFailed.make({ identifier: "swp_exit_package_missing" }),
    );
    const stillPending = transitionSwapWidgetState(
      sampleWidgetStates.Ready,
      SwapWidgetEvent.FundingGateChanged({ gate: pendingFundingGate() }),
    );
    expect(stillPending._tag).toBe("VerificationPending");
  });

  test("a fresh enabled gate recovers VerificationFailed to Ready", () => {
    const ready = transitionSwapWidgetState(
      sampleWidgetStates.VerificationFailed,
      SwapWidgetEvent.FundingGateChanged({ gate: enabledFundingGate() }),
    );
    expect(ready._tag).toBe("Ready");
  });

  test("session progression is monotone and never regresses", () => {
    expect(
      transitionSwapWidgetState(
        sampleWidgetStates.Executing,
        SwapWidgetEvent.SessionAdvanced({ state: "funding_observed" }),
      ),
    ).toEqual(sampleWidgetStates.Executing);
    expect(
      transitionSwapWidgetState(
        sampleWidgetStates.FundingObserved,
        SwapWidgetEvent.SessionAdvanced({ state: "executing" }),
      )._tag,
    ).toBe("Executing");
    expect(
      transitionSwapWidgetState(
        sampleWidgetStates.Executing,
        SwapWidgetEvent.SessionAdvanced({ state: "refund_pending" }),
      )._tag,
    ).toBe("RefundPending");
  });

  test("a funded settlement or refund can degrade to disputed", () => {
    for (const state of [sampleWidgetStates.SettlementPending, sampleWidgetStates.RefundPending]) {
      expect(
        transitionSwapWidgetState(state, SwapWidgetEvent.SessionAdvanced({ state: "disputed" }))
          ._tag,
      ).toBe("Disputed");
    }
  });

  test("raw unresolved remains distinct from its failed base projection", () => {
    expect(classifySwpState("unresolved")).toEqual({ ok: true, base: "failed" });
    expect(
      transitionSwapWidgetState(
        sampleWidgetStates.FundingObserved,
        SwapWidgetEvent.SessionAdvanced({ state: "unresolved" }),
      )._tag,
    ).toBe("Unresolved");
  });

  test("re-derived form states apply only in the form phase", () => {
    const event = SwapWidgetEvent.FormRederived({ state: cases.Ready.make({}) });
    for (const state of everySampleWidgetState()) {
      const next = transitionSwapWidgetState(state, event);
      if (isFormPhase(state)) expect(next._tag).toBe("Ready");
      else expect(next).toEqual(state);
    }
  });

  test("the widget starts in engine-loading, not in a state that implies readiness", () => {
    expect(initialSwapWidgetState._tag).toBe("EngineLoading");
    expect(derivePrimaryAction(initialSwapWidgetState, catalog, "sats").disabled).toBe(true);
  });
});

describe("the §9 mapping is SWAP-6's, not a second copy", () => {
  test("unknown values and local projections advance nothing", () => {
    // SWAP-6 (#9321) owns the MKT-SWP §9 table. This suite asserts the
    // widget consumes that classification rather than restating it.
    expect(classifySwpState("contract_pending")).toEqual({
      ok: false,
      error: "swp_status_transition_invalid",
    });
    expect(classifySwpState("brand_new_state")).toEqual({
      ok: false,
      error: "swp_status_transition_invalid",
    });
    expect(classifySwpState("completed")).toEqual({ ok: true, base: "completed" });
  });
});
