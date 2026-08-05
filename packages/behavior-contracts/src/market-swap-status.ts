import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";

/**
 * Behaviour contracts for the swap status/progress view (openagents#9321,
 * SWAP-6). The issue requires contracts for exactly three laws: gap
 * rendering, fork rendering, and the never-infer-a-rung-upward rule. The
 * oracles live in `@openagentsinc/mkt-swp-status`.
 */
export const marketSwapStatusContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract covers status projection and rendering only. A Status is one signer's claim: settlement truth stays with the wallet, Bitcoin consensus, and the Lightning rail per MKT-SWP §11, and nothing in this projection authorises funding, claiming, or refunding.",
      blockerRefs: [],
      contractId: "openagents_web.swap_status.gap_renders_unknown.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-status/src/session.test.ts",
        "packages/mkt-swp-status/src/lane.test.ts",
        "github:OpenAgentsInc/openagents#9321",
        "docs/teardowns/2026-08-04-boltz-web-app-ux-parity-teardown.md",
      ],
      oracles: [
        {
          description:
            "A missing sequence number in either signer's Status stream renders as a visible gap (swp_status_gap) with an unknown-with-explanation display, never as optimistic progress; a later Status does not close the gap — only the exact missing record can (MKT-SWP §9.5).",
          id: "openagents_web.swap_status.gap_renders_unknown.session",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-status/src/session.test.ts",
        },
      ],
      productArea: "Swap status and progress",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "A sequence gap renders as unknown-with-explanation, never as optimistic progress, and a later Status cannot close it.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-status tests build a fixture session with a deliberate provider-lane sequence gap and assert the unknown_gap integrity and display key, the swp_status_gap lane marker, that post-gap claims are retained without advancing, that a later Status leaves the gap open, and that only the exact missing record closes it.",
    },
    {
      authorityBoundary:
        "This contract covers fork retention and display only. It does not adjudicate which fork branch is true — no component may, because arrival order is not truth and both records carry valid signatures from the same author.",
      blockerRefs: [],
      contractId: "openagents_web.swap_status.fork_retained_loud.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-status/src/session.test.ts",
        "packages/mkt-swp-status/src/lane.test.ts",
        "github:OpenAgentsInc/openagents#9321",
      ],
      oracles: [
        {
          description:
            "Two Status records from one author at the same (session, order, seq) are a fork (swp_status_fork): both records are retained with their author, the projection is identical under either arrival order (no arrival-time resolution exists), the session cannot advance past the fork, and a later Status cannot erase it.",
          id: "openagents_web.swap_status.fork_retained_loud.session",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-status/src/session.test.ts",
        },
      ],
      productArea: "Swap status and progress",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "A fork is surfaced prominently with both conflicting claims retained; it is never resolved by arrival time and never hidden behind one chosen event.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-status tests deliver two conflicting provider records at one sequence in both arrival orders, asserting deep-equal projections, both retained records with their author, the unknown_fork display, and a frozen session state that a later Status does not unfreeze.",
    },
    {
      authorityBoundary:
        "This contract binds display rungs to evidence authority (MKT-SWP §11). Verifier admission and rail observation remain engine and adapter responsibilities; the view only refuses to show what they have not proved.",
      blockerRefs: [],
      contractId: "openagents_web.swap_status.rung_never_inferred_upward.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-status/src/session.test.ts",
        "packages/mkt-swp-status/src/rungs.test.ts",
        "github:OpenAgentsInc/openagents#9321",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            "The rung label renders the narrowest rung the exact evidence proves and is never inferred upward: a provider-signed status caps at pledged regardless of what it claims, a completed Status without settled verifier evidence renders at the proved rung with swp_settlement_overclaim retained, and no status alone can render as settled or complete.",
          id: "openagents_web.swap_status.rung_never_inferred_upward.session",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-status/src/session.test.ts",
        },
      ],
      productArea: "Swap status and progress",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "The rung label renders the narrowest rung the exact evidence proves, never inferred upward; a completed Status is one signer's claim until an admitted verifier raises it.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-status tests feed a completed Status with only paid-rung evidence and assert the display stays at the proved state and rung with a retained swp_settlement_overclaim disposition, and that provider-status evidence claiming settled attributes at pledged with the overclaim flagged.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-08-04.1",
};
