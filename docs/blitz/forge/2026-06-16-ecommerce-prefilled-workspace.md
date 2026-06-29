# E-Commerce Prefilled Workspace

Date: 2026-06-16
Status: Implemented seed template for #5099
Related: #5092, #5093, #5099, #5105

## Purpose

#5099 asks for the first e-commerce design-partner deliverable: an
inventory-aware ad-campaign workspace that can start from real stock, use
accurate product imagery, respect a spend cap, and produce stats plus a receipt.

The current implementation is a reusable seed template in
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts`.
It deliberately creates a **draft** workspace input, not a fake live merchant
campaign. Operators can pass the returned input into the existing
`POST /api/workspaces` flow, set `status: "invited"` when the workspace is ready,
and send the returned `/workspaces/{workspaceId}` invite URL.

## Template Ref

`forge.template.ecommerce.inventory_campaign.v1`

Holder ref:

`design_partner.ecommerce.inventory_campaign.v1`

Project name:

`Inventory-Aware Campaign Workspace`

## Seeded Memory

The template seeds only public-safe facts:

- Selected Forge template: e-commerce stage names mapped onto the canonical
  `signal`, `triage`, `codegen`, `validate`, `release`, `document`, `monitor`,
  and `deploy` keys.
- Starter accepted outcome: inventory-aware ad campaign using real in-stock
  products only, accurate imagery/source refs, explicit spend cap, and stats plus
  receipt handoff.
- Demand Signal input: catalog delta, inventory pressure, seasonal event, or
  storefront performance signal.
- Offer Triage output: eligible SKU set, excluded out-of-stock SKUs, channel,
  audience, spend cap, brand caveats, and missing-access blockers.
- Commerce QA gate: stock, imagery, links, offer math, policy caveats, margin
  assumptions, shipping/tax caveats, and excluded SKUs.
- Authority blocker: no publish or spend without channel access, ad account
  access, merchant approval, spend-cap acceptance, and deployment permission.
- Measurement contract: artifact refs, spend cap, stats window, attribution
  caveat, stockout or defect follow-up, and freshness timestamp.

No customer names, individual names, private catalog rows, account credentials,
raw prompts, wallet data, or channel tokens are seeded.

## Starter Workflows

1. **Inventory-aware ad campaign** — draft the campaign from in-stock products
   only, cite product/image sources, state the spend cap, and stop before publish
   until merchant approval and channel access are receipted.
2. **Commerce QA pass** — check stock, imagery, links, offer math, policy
   caveats, margin assumptions, and excluded SKUs before a release candidate is
   accepted.
3. **Campaign receipt and stats brief** — prepare the merchant-safe handoff with
   artifact refs, spend cap, approval state, measurement window, attribution
   caveat, and follow-up signals.

## Operator Boundary

This template is ready for manual or automated operator seeding through the
existing workspace primitive. The #5093 invite flow now generates the personal
invite URL, lets the first signed-in holder claim an unbound invited workspace,
and records view/revisit/first-run engagement for operator inspection.

Until a holder connects storefront, catalog, ad, or analytics accounts, the
workspace must treat live merchant data as absent. Any campaign publish, spend,
or external channel write remains blocked until an authority receipt exists.

## Verification

The implementation is covered by
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.test.ts`.
The tests verify that the template:

- produces a valid draft `CreatePrefilledWorkspaceInput`;
- preserves the canonical Forge stage keys;
- carries the requested stock, imagery, spend-cap, stats, approval, and receipt
  gates; and
- backs every seeded-memory fact with a public source reference.
