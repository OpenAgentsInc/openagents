# Legal Prefilled Workspace

Date: 2026-06-16
Status: Implemented seed template for #5100
Related: #5092, #5093, #5100, #5105

## Purpose

#5100 asks for the first legal design-partner deliverable: a forms/intake
copilot starter, such as an NDA draft packet plus review checklist and suggested
time entry. It must be review-gated and source-linked.

The current implementation is a reusable seed template in
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts`.
It deliberately creates a **draft** workspace input, not a live matter, legal
advice surface, or attorney-reviewed packet. Operators can pass the returned
input into the existing `POST /api/workspaces` flow, set `status: "invited"`
when the workspace is ready, and send the returned `/workspaces/{workspaceId}`
invite URL.

## Template Ref

`forge.template.legal.forms_intake_copilot.v1`

Holder ref:

`design_partner.legal.forms_intake_copilot.v1`

Project name:

`Forms Intake Copilot Workspace`

## Seeded Memory

The template seeds only public-safe facts:

- Selected Forge template: legal stage names mapped onto the canonical `signal`,
  `triage`, `codegen`, `validate`, `release`, `document`, `monitor`, and
  `deploy` keys.
- Starter accepted outcome: forms/intake copilot for an NDA-style draft packet,
  review checklist, source-linked assumptions, suggested time entry, and explicit
  human-review gate.
- Legal safety boundary: workflow assistance only; no legal advice, no
  rights-impacting decisions, and no implied attorney review.
- Matter Signal input: intake form, document upload, client question, deadline,
  or workflow request, carrying requester state, confidentiality class, and
  jurisdiction caveat.
- Intake Triage output: scoped workflow with document type, jurisdiction caveat,
  reviewer lane, risk flags, missing-info list, and human-review requirement.
- Draft Assembly output: draft form packet, NDA checklist, clause comparison,
  issue list, or intake summary with source citation map and assumptions list.
- Legal Review Gate: citation coverage, redaction, jurisdiction warning,
  policy/risk flags, reviewer decision, and safe-to-share/blocked state.
- Authority blocker: no external delivery, matter-system update, or time entry
  without reviewer acceptance, visibility class, delivery permission, and
  time-entry approval.
- Measurement contract: source refs, review decision, retained assumptions,
  suggested time entry, delivery caveats, follow-up tasks, and freshness
  timestamp.

No customer names, individual names, confidential matter facts, uploaded
documents, account credentials, raw prompts, wallet data, or timekeeping tokens
are seeded.

## Starter Workflows

1. **NDA intake packet** — assemble an NDA-style intake summary and draft packet
   from source-linked public-safe inputs, carrying jurisdiction caveats,
   assumptions, and missing-info blockers.
2. **Review checklist** — produce a reviewer-facing checklist for citations,
   redaction, jurisdiction warnings, policy/risk flags, and whether the packet is
   safe to share, blocked, or needs more info.
3. **Suggested time entry and handoff** — draft a non-billable suggested
   time-entry note plus matter handoff summary, source map, retained
   assumptions, delivery caveats, and follow-up tasks.

## Operator Boundary

This template is ready for manual or automated operator seeding through the
existing workspace primitive. The #5093 invite flow now generates the personal
invite URL, lets the first signed-in holder claim an unbound invited workspace,
and records view/revisit/first-run engagement for operator inspection.

Until a holder connects matter, document, CRM, or timekeeping systems, the
workspace must treat live legal data as absent. Any client delivery, matter
system write, or time-entry record remains blocked until a qualified reviewer
accepts the packet and the holder grants authority.

## Verification

The implementation is covered by
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.test.ts`.
The tests verify that the template:

- produces a valid draft `CreatePrefilledWorkspaceInput`;
- preserves the canonical Forge stage keys;
- carries the requested NDA/intake, review checklist, source-link, suggested
  time-entry, human-review, and no-legal-advice gates; and
- backs every seeded-memory fact with a public source reference.
