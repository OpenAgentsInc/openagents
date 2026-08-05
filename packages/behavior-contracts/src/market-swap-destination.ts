import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";

/**
 * Behaviour contracts for swap destination entry (openagents#9317, SWAP-2).
 * The issue requires contracts for exactly three laws: amountless-invoice
 * refusal, amount-locked-invoice precedence, and the amount-edit-clears-
 * invoice rule. The oracles live in `@openagentsinc/mkt-swp-destination`.
 */
export const marketSwapDestinationContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract covers UI destination pre-checking only. It grants no funding authority: verify-before-fund truth is computed by the MKT-SWP engine behind the SWAP-0 boundary, and a parse-level acceptance is never presented as the safety layer.",
      blockerRefs: [],
      contractId: "openagents_web.swap_destination.amountless_invoice_refused.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-destination/src/entry.test.ts",
        "packages/mkt-swp-destination/src/parse.test.ts",
        "github:OpenAgentsInc/openagents#9317",
        "docs/nips/MKT-SWP.md",
      ],
      oracles: [
        {
          description:
            "An amountless BOLT11 invoice never binds to the destination field; the refusal carries the typed identifier swp_invoice_invalid (MKT-SWP §7.2: amountless invoices are invalid in v1), and funding stays blocked.",
          id: "openagents_web.swap_destination.amountless_invoice_refused.entry",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-destination/src/entry.test.ts",
        },
      ],
      productArea: "Swap widget destination entry",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "An amountless invoice is refused with the typed identifier swp_invoice_invalid, not prose, and never binds as a destination.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-destination tests drive the entry reducer with an amountless invoice and assert the invoice_amountless discriminant, the swp_invoice_invalid identifier on both the parse failure and the rendered view, a null binding, and a blocked funding gate.",
    },
    {
      authorityBoundary:
        "This contract covers amount reconciliation in the entry state only. The engine's amount-equation check (swp_amount_equation_mismatch) remains the funding authority.",
      blockerRefs: [],
      contractId: "openagents_web.swap_destination.invoice_amount_precedence.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-destination/src/entry.test.ts",
        "packages/mkt-swp-destination/src/parse.test.ts",
        "github:OpenAgentsInc/openagents#9317",
      ],
      oracles: [
        {
          description:
            "An amount-locked invoice overrides the typed amount with an explicit notice, and a unified QR carrying both a BIP-21 amount and an amount-bearing invoice yields exactly one effective amount (the invoice's), with the BIP-21 value suppressed.",
          id: "openagents_web.swap_destination.invoice_amount_precedence.entry",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-destination/src/entry.test.ts",
        },
      ],
      productArea: "Swap widget destination entry",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "An amount-locked invoice overrides the typed amount, and a unified QR carrying both a BIP-21 amount and an amount-bearing invoice must not produce two conflicting amounts.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-destination tests bind an amount-bearing invoice over a typed amount and a unified bitcoin: URI carrying both amounts, asserting a single effective amount sourced from the invoice and the suppression/override notices.",
    },
    {
      authorityBoundary:
        "This contract covers entry-state clearing behaviour only. It does not authorise order creation or resolution of deferred destinations.",
      blockerRefs: [],
      contractId: "openagents_web.swap_destination.amount_edit_clears_invoice.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-destination/src/entry.test.ts",
        "github:OpenAgentsInc/openagents#9317",
      ],
      oracles: [
        {
          description:
            "Editing the amount after a concrete invoice was entered clears that invoice with an explicit notice; deferred destinations (LNURL, lightning address, BOLT12 offer), whose amount is not yet bound, survive the edit.",
          id: "openagents_web.swap_destination.amount_edit_clears_invoice.entry",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-destination/src/entry.test.ts",
        },
      ],
      productArea: "Swap widget destination entry",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "Editing the amount after a concrete invoice was entered clears that invoice with an explicit notice; deferred destinations whose amount is not yet bound survive.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-destination tests edit the amount with a concrete invoice bound (asserting the invoice clears with the invoice_cleared_by_amount_edit notice) and with a deferred lightning address bound (asserting it survives).",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-08-04.1",
};
