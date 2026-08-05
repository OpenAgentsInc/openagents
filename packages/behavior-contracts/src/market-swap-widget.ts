import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";

/**
 * Behaviour contracts for the web swap widget (SWAP-0, openagents#9315;
 * evidence: docs/teardowns/2026-08-04-boltz-web-app-ux-parity-teardown.md
 * sections 2.2-2.3). The widget implementation lives in
 * `packages/mkt-swp`; these laws are cross-surface expectations — Omega's
 * `market_ui` GPUI components render the same typed state and are held to
 * the same statements.
 */
export const marketSwapWidgetContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract covers the rendering law of the swap widget's primary action only. It grants no funding, custody, settlement, payment, or mainnet authority, and an enabled action is never evidence that a counterparty performed anything.",
      blockerRefs: [],
      contractId: "openagents_web.swap_widget.primary_action_law.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp/src/primary-action.ts",
        "packages/mkt-swp/src/primary-action.test.ts",
        "apps/openagents.com/apps/start/src/features/swap/widget.tsx",
        "apps/openagents.com/apps/start/src/features/swap/swap-surface.test.tsx",
        "docs/teardowns/2026-08-04-boltz-web-app-ux-parity-teardown.md",
        "github:OpenAgentsInc/openagents#9315",
      ],
      oracles: [
        {
          description:
            "Loading states (engine, pairs, quote refresh) are busy, blocked, and still labelled.",
          id: "swap_widget.primary_action.loading_class",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/primary-action.test.ts",
        },
        {
          description:
            "Offline and structural refusals are danger-toned, blocked, and never spin; NoOfferings has its own permanent-absence label and never reuses the pair-loading label.",
          id: "swap_widget.primary_action.refusal_class",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/primary-action.test.ts",
        },
        {
          description:
            "Amount refusals state the violated bound formatted in the user's current denomination (sats and BTC).",
          id: "swap_widget.primary_action.amount_class",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/primary-action.test.ts",
        },
        {
          description:
            "Destination and quote refusals are typed, labelled, and blocked without spinning.",
          id: "swap_widget.primary_action.destination_quote_class",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/primary-action.test.ts",
        },
        {
          description:
            "Verification states keep the action blocked; only Ready and AwaitingFunding are pressable; Ordering is blocked without a fresh explanation because disabled is computed independently of the label.",
          id: "swap_widget.primary_action.verification_actionable_class",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/primary-action.test.ts",
        },
        {
          description:
            "In-flight and terminal states stay rendered and blocked, and the always-rendered law holds over the whole state union in both denominations and under a partial locale.",
          id: "swap_widget.primary_action.inflight_terminal_class",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/primary-action.test.ts",
        },
        {
          description:
            "The mounted /swap surface renders the primary action as one button from the same derivation: disabled, busy, and labelled from a typed key while the engine loads.",
          id: "swap_widget.primary_action.mounted_surface",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents.com/apps/start/src/features/swap/swap-surface.test.tsx",
        },
      ],
      productArea: "Liquidity market swap widget",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "The primary action is always rendered, always states the single most proximate reason it cannot proceed, and states it in the user's current units.",
      surface: "openagents.com swap widget",
      verification:
        "The mkt-swp package test sweep derives the primary action for every state in the typed union and asserts label, tone, disabled, and content per state class; the start-app surface sweep asserts the mounted /swap control renders from the same derivation.",
    },
    {
      authorityBoundary:
        "This contract covers the client-side funding gate only. Passing it authorizes no broadcast, spend, or custody action, and an engine authorization is not evidence of provider reservation or settlement.",
      blockerRefs: [],
      contractId: "openagents_web.swap_widget.funding_gate.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp/src/swap-engine.ts",
        "packages/mkt-swp/src/widget-state.ts",
        "packages/mkt-swp-compare/src/verify.ts",
        "packages/mkt-swp/src/engine-boundary.test.ts",
        "packages/mkt-swp/src/widget-state.test.ts",
        "github:OpenAgentsInc/openagents#9315",
      ],
      oracles: [
        {
          description:
            "The engine refuses authorizeFunding with swp_funding_not_authorized while any verify-before-fund row is failed or unresolved, when the verdict is blocked, when verification never ran, and refuses an exit package not digest-bound to the verified contract pair.",
          id: "swap_widget.funding_gate.engine",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/engine-boundary.test.ts",
        },
        {
          description:
            "AwaitingFunding is reachable only from Ordering with an engine-issued FundingAuthorization; no session Status claim advances the widget past the gate, and a fresh enabled verification gate recovers a prior VerificationFailed state.",
          id: "swap_widget.funding_gate.state_machine",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/widget-state.test.ts",
        },
      ],
      productArea: "Liquidity market swap widget",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "The fund action cannot be enabled while any verify-before-fund check is unresolved or failed (swp_funding_not_authorized).",
      surface: "openagents.com swap widget",
      verification:
        "The mkt-swp package test sweep drives a fixture session through order, verification, and funding authorisation, and asserts both the engine refusal and the state-machine guard fail closed.",
    },
    {
      authorityBoundary:
        "This contract covers state coverage and explanation rendering only. It does not claim the enumerated states are the complete protocol lifecycle, and it grants no authority over session outcomes.",
      blockerRefs: [],
      contractId: "openagents_web.swap_widget.state_exhaustive_explanation.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp/src/widget-state.ts",
        "packages/mkt-swp/src/widget-host.ts",
        "packages/mkt-swp/src/compose.ts",
        "packages/mkt-swp/src/widget-state.test.ts",
        "packages/mkt-swp/src/engine-boundary.test.ts",
        "github:OpenAgentsInc/openagents#9315",
      ],
      oracles: [
        {
          description:
            "The state union is exhaustive and checked against the sample set and schema cases; every state/event pair folds to a valid state; funded settlement and refund states may degrade to Disputed; raw unresolved remains Unresolved after its required failed-base normalization; and every state renders a non-empty explanation in both denominations.",
          id: "swap_widget.state.exhaustive_explanation",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp/src/widget-state.test.ts",
        },
      ],
      productArea: "Liquidity market swap widget",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "The typed state is exhaustive over the enumerated set and every transition is total; no state is reachable without a rendered explanation.",
      surface: "openagents.com swap widget",
      verification:
        "The mkt-swp package test sweep asserts the tag set matches the schema cases and the sample set exactly, folds every sample state through every sample event without leaving the union, and renders a non-empty explanation for every state in both denominations.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-08-04.2",
};
