import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";

/**
 * Behaviour contracts for swap pair selection and the rate/fee panel
 * (openagents#9316, SWAP-1). The issue requires contracts for exactly
 * three laws: the unreachable-direction disclosure, the fee-as-promise
 * framing, and the no-auto-unit-switch rule. The oracles live in
 * `@openagentsinc/mkt-swp-pair`.
 */
export const marketSwapPairContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract covers UI reachability disclosure folded from public 39601 Offering heads only. It grants no funding authority: per-amount and per-side validation remains with the MKT-SWP engine behind the SWAP-0 boundary (swp_side_disabled / swp_invalid_amount at Quote and Order time).",
      blockerRefs: [],
      contractId: "openagents_web.swap_pair.unreachable_direction_disclosed.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-pair/src/corpus.test.ts",
        "packages/mkt-swp-pair/src/selection.test.ts",
        "github:OpenAgentsInc/openagents#9316",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            'A direction with max="0" on every discovered Offering folds to the typed side_disabled reason, empty corpora and paused/stale carriers fold to their own typed reasons, and the selector discloses unreachability before selection.',
          id: "openagents_web.swap_pair.unreachable_direction_disclosed.corpus",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-pair/src/corpus.test.ts",
        },
        {
          description:
            "An unreachable direction can be selected for inspection but never into a fundable state: the primary-action gate refuses with the direction's typed reason and swp_side_disabled.",
          id: "openagents_web.swap_pair.unreachable_direction_disclosed.selection",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-pair/src/selection.test.ts",
        },
      ],
      productArea: "Swap widget pair selection",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "Reachable directions are a function of the live Offering corpus: a direction no live Offering enables renders unreachable with a typed reason (no_offering, side_disabled, provider_paused, offerings_stale) before selection, and can never be selected into a fundable state.",
      surface: "openagents.com/swap",
      verification:
        'packages/mkt-swp-pair tests fold fixture corpora (max="0" everywhere, paused providers, stale heads, empty corpus) and assert the typed reason per direction, the selector\'s pre-selection disclosure, and a blocked primary-action gate carrying swp_side_disabled for unreachable selections.',
    },
    {
      authorityBoundary:
        "This contract covers the rendered fee panel only. The engine's amount-equation check at contract time remains the funding authority; the panel re-derives for display and fails closed to the refused state.",
      blockerRefs: [],
      contractId: "openagents_web.swap_pair.fee_output_promise.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-pair/src/quote.test.ts",
        "github:OpenAgentsInc/openagents#9316",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            "The fee panel renders separated components (provider_fee, miner_fee_budget, lightning_routing_fee_budget), who pays, floor_output_sats, and the amount_equation, frames the output as the MKT-SWP §3.3 fill promise, and a quote whose output does not reproduce from its terms surfaces swp_amount_equation_mismatch instead of rendering.",
          id: "openagents_web.swap_pair.fee_output_promise.panel",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-pair/src/quote.test.ts",
        },
      ],
      productArea: "Swap widget rate and fee panel",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "The rendered output amount is the provider's fill promise, reproduced locally from the Quote's amount_equation and separated fee components; an unreproducible quote surfaces swp_amount_equation_mismatch and is never rendered as a promise.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-pair tests verify a conforming quote's exact bigint reproduction, a negative table over output/provider-fee/rounding/equation/total mismatches, and the fee panel's refused state carrying swp_amount_equation_mismatch.",
    },
    {
      authorityBoundary:
        "This contract covers display denomination handling only. Wire amounts remain canonical integer satoshi strings end to end regardless of the display unit.",
      blockerRefs: [],
      contractId: "openagents_web.swap_pair.no_auto_unit_switch.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-pair/src/selection.test.ts",
        "packages/mkt-swp-pair/src/amount.test.ts",
        "github:OpenAgentsInc/openagents#9316",
      ],
      oracles: [
        {
          description:
            "The display denomination changes only on the explicit toggle event; typing never changes it, sats text with a decimal separator is refused rather than reinterpreted as BTC, the toggle converts parsed values with exact bigint arithmetic, and unparseable text survives verbatim.",
          id: "openagents_web.swap_pair.no_auto_unit_switch.selection",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-pair/src/selection.test.ts",
        },
      ],
      productArea: "Swap widget amount entry",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "The display denomination (BTC/sats) changes only on the user's explicit toggle; no input path ever reinterprets the digits the user typed in another unit (no Boltz-style auto-denomination switching).",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-pair tests drive the reducer with typed amounts across both denominations, asserting typing never mutates the denomination, separator-bearing sats input refuses with sats_fractional, and the explicit toggle round-trips values exactly over atomic units.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-08-04.1",
};
