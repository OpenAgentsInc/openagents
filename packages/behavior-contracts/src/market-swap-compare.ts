import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";

/**
 * Behaviour contracts for multi-provider quote comparison
 * (openagents#9318, SWAP-3). The issue requires contracts for exactly four
 * laws: firm-versus-indicative rendering, reservation proof-class
 * distinction, quote expiry enforcement, and the
 * funding-disabled-until-all-checks-pass rule. The oracles live in
 * `@openagentsinc/mkt-swp-compare`.
 */
export const marketSwapCompareContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract covers comparison presentation and ranking only. A firm Quote's declaration is displayed as a declaration — it proves neither the declaration nor the provider's capacity, and nothing in the comparison surface accepts an order or authorises funding.",
      blockerRefs: [],
      contractId: "openagents_web.swap_compare.firm_indicative_distinct.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-compare/src/view.test.ts",
        "packages/mkt-swp-compare/src/ranking.test.ts",
        "github:OpenAgentsInc/openagents#9318",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            "Two providers answering one RFQ with different quote classes render as different commitments: the firm/hard row and the indicative/none row carry distinct badge tokens (the render key that selects the style), distinct message keys, and distinct copy, so they cannot be styled identically.",
          id: "openagents_web.swap_compare.firm_indicative_distinct.view",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-compare/src/view.test.ts",
        },
        {
          description:
            "Comparison ordering is commitment-tier major: a cheaper indicative quote is never ranked above any firm quote, and the ranking is deterministic and stable for equal terms (total order with an event-id tiebreak).",
          id: "openagents_web.swap_compare.firm_indicative_distinct.ranking",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-compare/src/ranking.test.ts",
        },
      ],
      productArea: "Swap widget quote comparison",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "Indicative and firm quotes render as the distinct commitments they are, and a cheaper indicative quote is never presented as if it were as binding as a firm one.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-compare tests build the compare table from an indicative/none and a firm/hard quote answering one RFQ, asserting distinct badge tokens, keys, and copy, and drive the ranking comparator to show class-major ordering with a stable deterministic tiebreak.",
    },
    {
      authorityBoundary:
        "This contract covers reservation-evidence presentation and client-side fork retention only. Reservation proofs are verified by the MKT-SWP engine; the client renders the claimed proof class and refuses disallowed class pairings, it does not validate proofs.",
      blockerRefs: [],
      contractId: "openagents_web.swap_compare.reservation_proof_class_distinct.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-compare/src/view.test.ts",
        "packages/mkt-swp-compare/src/reservation.test.ts",
        "github:OpenAgentsInc/openagents#9318",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            "All seven §5 reservation proof classes render pairwise-distinct labels — a provider-signed claim and a covenant-enforced reserve are not the same evidence and never render the same — and a disallowed proof/class pairing (provider_signed backing hard) is refused with swp_reservation_proof_invalid.",
          id: "openagents_web.swap_compare.reservation_proof_class_distinct.view",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-compare/src/view.test.ts",
        },
        {
          description:
            "A reservation fork (duplicate allocation sequence or exceeded capacity) is retained as an attributable swp_reservation_fork implicating every member quote, identical under either ingestion order — never resolved by arrival time.",
          id: "openagents_web.swap_compare.reservation_proof_class_distinct.fork",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-compare/src/reservation.test.ts",
        },
      ],
      productArea: "Swap widget quote comparison",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "Reservation tiers and their proof classes render as distinct evidence, a signed claim never renders like a covenant-enforced reserve, and a provider's reservation fork is retained and displayed rather than resolved.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-compare tests assert pairwise-distinct proof-class labels across all seven classes, refusal of disallowed proof/class pairings, and arrival-order-independent fork retention displayed on every implicated table row.",
    },
    {
      authorityBoundary:
        "This contract covers client-side expiry enforcement only. The provider and engine re-check expiry independently; the client's refusal is the local guarantee that an expired quote cannot be ordered from this surface.",
      blockerRefs: [],
      contractId: "openagents_web.swap_compare.quote_expiry_enforced.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-compare/src/expiry.test.ts",
        "packages/mkt-swp-compare/src/selection.test.ts",
        "github:OpenAgentsInc/openagents#9318",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            "Quote expiry is computed from the injected clock as the earlier of quote expiration and reservation expiry; at the bound the state is expired with the typed identifier swp_quote_expired, not a styled-stale rendering.",
          id: "openagents_web.swap_compare.quote_expiry_enforced.countdown",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-compare/src/expiry.test.ts",
        },
        {
          description:
            "An expired quote cannot be ordered: selectOrder refuses locally with swp_quote_expired (for quote and for reservation expiry) rather than relying on a provider to reject it.",
          id: "openagents_web.swap_compare.quote_expiry_enforced.selection",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-compare/src/selection.test.ts",
        },
      ],
      productArea: "Swap widget quote comparison",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "An expired quote becomes unusable with a visible countdown and the typed identifier swp_quote_expired; the client refuses locally rather than relying on a provider.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-compare tests drive quoteExpiryState across the bound (quote-bound and reservation-bound) and assert selectOrder refuses expired quotes locally with swp_quote_expired.",
    },
    {
      authorityBoundary:
        "This contract covers the UI funding gate only, and the gate is a UX pre-check in one direction: it can only keep funding disabled. Verification truth is computed by the MKT-SWP engine behind the SWAP-0 boundary and arrives as a typed report; the UI never computes a profile-level verdict and has no path from UI-side state to an enabled fund action.",
      blockerRefs: [],
      contractId: "openagents_web.swap_compare.funding_disabled_until_checks_pass.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-compare/src/verify.test.ts",
        "github:OpenAgentsInc/openagents#9318",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            "The fund action cannot be enabled while any §7.1–§7.4 checklist row is unresolved, missing, or failed, while the engine report is absent or from a superseded epoch, or while the engine verdict is not verification_passed — each case yields swp_funding_not_authorized, and every failing row remains individually identifiable by check id and §17 identifier.",
          id: "openagents_web.swap_compare.funding_disabled_until_checks_pass.gate",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-compare/src/verify.test.ts",
        },
      ],
      productArea: "Swap widget quote comparison",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "Funding stays disabled until every verify-before-fund row passes; a missing, stale, incomplete, or failed engine report is swp_funding_not_authorized, and every failing row is individually identifiable.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-compare tests drive fundingGate and verifyChecklistView through null, stale, unresolved-row, missing-row, failed-row, and blocked-verdict reports (asserting disabled with swp_funding_not_authorized and row-level identity) and assert enablement only on a complete current-epoch engine pass.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-08-04.1",
};
