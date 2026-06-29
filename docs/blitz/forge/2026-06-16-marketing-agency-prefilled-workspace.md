# Marketing-Agency Prefilled Workspace

Date: 2026-06-16
Status: Implemented seed template for #5102
Related: #5092, #5093, #5102, #5105

## Purpose

#5102 asks for the first marketing-agency design-partner deliverable: a client
landing page plus welcome email in the agency's brand at a white-label subdomain,
and an "operator on Autopilot" admin lane so the agency can run its own business
on Forge. The agency is both a customer and a white-label channel.

The current implementation is a reusable seed template in
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts`.
It deliberately creates a **draft** workspace input, not a live client campaign,
sent email, DNS/subdomain configuration, or active admin lane. Operators can pass
the returned input into the existing `POST /api/workspaces` flow, set
`status: "invited"` when the workspace is ready, and send the returned
`/workspaces/{workspaceId}` invite URL.

## Template Ref

`forge.template.marketing_agency.white_label_launch.v1`

Holder ref:

`design_partner.marketing_agency.white_label_launch.v1`

Project name:

`Agency White-Label Launch Workspace`

## Seeded Memory

The template seeds only public-safe facts:

- Selected Forge template: marketing-agency stage names mapped onto the
  canonical `signal`, `triage`, `codegen`, `validate`, `release`, `document`,
  `monitor`, and `deploy` keys.
- Starter accepted outcomes: agency-branded landing page and welcome email for a
  white-label subdomain, paired with an operator-on-Autopilot admin lane.
- Client Signal input: client brief, brand guideline, analytics issue, launch
  date, stakeholder comment, or agency internal operator need.
- Creative Triage output: audience, channel, tone, success metric, approval lane,
  brand-fit checks, missing assets, and stakeholder dependencies.
- Creative Production output: landing-page structure, welcome-email copy,
  ad/social variants, campaign plan, asset brief, or operator-admin lane setup.
- Brand QA gate: brand consistency, factuality, accessibility, link/CTA behavior,
  channel fit, white-label domain state, and reviewer notes.
- Authority blocker: no page publish, email send, DNS/subdomain configuration,
  white-label delivery claim, or client/admin lane operation without client
  approval, domain authority, channel access, and delivery permission.
- Agency operator lane: internal accepted-outcome work on Forge is kept separate
  from client-facing approvals and receipts.
- Measurement contract: approved deliverable refs, white-label subdomain state,
  email sent/scheduled state, operator-lane acceptance, metric window,
  attribution caveat, and freshness timestamp.

No customer names, individual names, private brand assets, DNS credentials,
email tokens, CRM data, raw prompts, wallet data, or client account secrets are
seeded.

## Starter Workflows

1. **White-label landing page** — draft the agency-branded landing-page structure
   for a white-label subdomain with brand-source refs, audience, CTA,
   accessibility checks, and domain/publish authority blockers.
2. **Welcome email** — draft the agency-branded welcome email with source-linked
   claims, channel caveats, approval state, send/schedule blocker, and
   measurement handoff.
3. **Operator on Autopilot admin lane** — set up the agency internal admin lane
   for running its own accepted-outcome work on Forge, with approvals, owner
   refs, and receipts separated from client-facing work.

## Operator Boundary

This template is ready for manual or automated operator seeding through the
existing workspace primitive. The #5093 invite flow now generates the personal
invite URL, lets the first signed-in holder claim an unbound invited workspace,
and records view/revisit/first-run engagement for operator inspection.

Until a holder connects brand, domain, email, analytics, client, or agency admin
systems, the workspace must treat live agency/client data as absent. Any
publication, email send, DNS/subdomain change, client delivery, or agency admin
operation remains blocked until the relevant authority receipt exists.

## Verification

The implementation is covered by
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.test.ts`.
The tests verify that the template:

- produces a valid draft `CreatePrefilledWorkspaceInput`;
- preserves the canonical Forge stage keys;
- carries the requested landing-page, welcome-email, agency-brand,
  white-label-subdomain, operator/admin-lane, client-approval, and DNS/publish
  gates; and
- backs every seeded-memory fact with a public source reference.
