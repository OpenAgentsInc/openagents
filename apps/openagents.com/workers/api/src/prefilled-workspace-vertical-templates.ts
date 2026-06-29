import type {
  CreatePrefilledWorkspaceInput,
  SeededMemoryEntry,
  StarterWorkflow,
} from './prefilled-workspace'

export const ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF =
  'forge.template.ecommerce.inventory_campaign.v1'

export const ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS = [
  'docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md#e-commerce-template',
  'docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md',
  'github.issue.OpenAgentsInc.openagents.5099',
] as const

export const ECOMMERCE_DESIGN_PARTNER_STAGE_KEYS = [
  'signal',
  'triage',
  'codegen',
  'validate',
  'release',
  'document',
  'monitor',
  'deploy',
] as const

export const LEGAL_DESIGN_PARTNER_TEMPLATE_REF =
  'forge.template.legal.forms_intake_copilot.v1'

export const LEGAL_DESIGN_PARTNER_SOURCE_REFS = [
  'docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md#legal-template',
  'docs/blitz/forge/2026-06-16-legal-prefilled-workspace.md',
  'github.issue.OpenAgentsInc.openagents.5100',
] as const

export const LEGAL_DESIGN_PARTNER_STAGE_KEYS = [
  'signal',
  'triage',
  'codegen',
  'validate',
  'release',
  'document',
  'monitor',
  'deploy',
] as const

export const MARKETING_AGENCY_DESIGN_PARTNER_TEMPLATE_REF =
  'forge.template.marketing_agency.white_label_launch.v1'

export const MARKETING_AGENCY_DESIGN_PARTNER_SOURCE_REFS = [
  'docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md#marketing-agency-template',
  'docs/blitz/forge/2026-06-16-marketing-agency-prefilled-workspace.md',
  'github.issue.OpenAgentsInc.openagents.5102',
] as const

export const MARKETING_AGENCY_DESIGN_PARTNER_STAGE_KEYS = [
  'signal',
  'triage',
  'codegen',
  'validate',
  'release',
  'document',
  'monitor',
  'deploy',
] as const

const ecommerceTemplateSourceRef = ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS[0]
const ecommerceWorkspaceSourceRef = ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS[1]
const legalTemplateSourceRef = LEGAL_DESIGN_PARTNER_SOURCE_REFS[0]
const legalWorkspaceSourceRef = LEGAL_DESIGN_PARTNER_SOURCE_REFS[1]
const marketingAgencyTemplateSourceRef =
  MARKETING_AGENCY_DESIGN_PARTNER_SOURCE_REFS[0]
const marketingAgencyWorkspaceSourceRef =
  MARKETING_AGENCY_DESIGN_PARTNER_SOURCE_REFS[1]

const ecommerceSeededMemory = (): ReadonlyArray<SeededMemoryEntry> => [
  {
    label: 'Selected Forge template',
    value: `${ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF}: use the e-commerce vertical mapping for Demand Signal, Offer Triage, Campaign Build, Commerce QA, Merchandising Release, Merchant Handoff, Conversion Watch, and Channel Publish.`,
    publicSourceRef: ecommerceTemplateSourceRef,
  },
  {
    label: 'Canonical stage keys',
    value: ECOMMERCE_DESIGN_PARTNER_STAGE_KEYS.join(', '),
    publicSourceRef: ecommerceTemplateSourceRef,
  },
  {
    label: 'Starter accepted outcome',
    value:
      'Inventory-aware ad-campaign workflow that uses real in-stock products only, accurate product imagery and source refs, an explicit spend cap, and a stats plus receipt handoff.',
    publicSourceRef: ecommerceWorkspaceSourceRef,
  },
  {
    label: 'Demand Signal input',
    value:
      'Start from a catalog or inventory signal, stock pressure, a seasonal event, or a storefront performance signal. Keep the brief public-safe until the merchant connects accounts.',
    publicSourceRef: ecommerceTemplateSourceRef,
  },
  {
    label: 'Offer Triage output',
    value:
      'Produce a prioritized offer plan with eligible SKU set, excluded out-of-stock SKUs, target channel, audience, spend cap, brand caveats, and missing-access blockers.',
    publicSourceRef: ecommerceTemplateSourceRef,
  },
  {
    label: 'Commerce QA gate',
    value:
      'Before release, verify real stock, accurate imagery/source references, offer math, links, price and margin caveats, shipping or tax caveats, and channel-policy fit.',
    publicSourceRef: ecommerceTemplateSourceRef,
  },
  {
    label: 'Authority blocker',
    value:
      'Do not publish or spend. Channel access, ad account access, merchant approval, spend-cap acceptance, and deployment permission are explicit blockers until receipted.',
    publicSourceRef: ecommerceWorkspaceSourceRef,
  },
  {
    label: 'Measurement contract',
    value:
      'Receipt must separate published artifact refs, spend cap, campaign stats window, attribution caveat, stockout or defect follow-up, and freshness timestamp.',
    publicSourceRef: ecommerceTemplateSourceRef,
  },
]

const ecommerceStarterWorkflows = (): ReadonlyArray<StarterWorkflow> => [
  {
    title: 'Inventory-aware ad campaign',
    description:
      'Draft a campaign from in-stock products only, cite product/image sources, state the spend cap, and stop before publish until merchant approval and channel access are receipted.',
    outcomeKind: 'inventory_aware_ad_campaign',
    status: 'queued',
  },
  {
    title: 'Commerce QA pass',
    description:
      'Check stock, imagery, links, offer math, policy caveats, margin assumptions, and excluded SKUs before any release candidate is accepted.',
    outcomeKind: 'commerce_qa',
    status: 'queued',
  },
  {
    title: 'Campaign receipt and stats brief',
    description:
      'Prepare the merchant-safe handoff: artifact refs, spend cap, approval state, measurement window, attribution caveat, and follow-up signals.',
    outcomeKind: 'campaign_receipt_stats',
    status: 'queued',
  },
]

export const makeEcommerceDesignPartnerWorkspaceInput =
  (): CreatePrefilledWorkspaceInput => ({
    holderRef: 'design_partner.ecommerce.inventory_campaign.v1',
    projectName: 'Inventory-Aware Campaign Workspace',
    status: 'draft',
    seededMemory: ecommerceSeededMemory(),
    starterWorkflows: ecommerceStarterWorkflows(),
    introReceipt: {
      summary:
        'Seeded the first e-commerce design-partner workspace with the Forge e-commerce stage template, an inventory-aware ad-campaign starter, Commerce QA gate, authority blockers, and a stats/receipt handoff contract. This is public-safe seed material only; live merchant catalog, channel, and spend authority stay blocked until the holder connects accounts and approves them.',
      publicSourceRefs: [...ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS],
    },
  })

const legalSeededMemory = (): ReadonlyArray<SeededMemoryEntry> => [
  {
    label: 'Selected Forge template',
    value: `${LEGAL_DESIGN_PARTNER_TEMPLATE_REF}: use the legal vertical mapping for Matter Signal, Intake Triage, Draft Assembly, Legal Review Gate, Client-Ready Packet, Matter Handoff, Matter Follow-Up, and Secure Delivery.`,
    publicSourceRef: legalTemplateSourceRef,
  },
  {
    label: 'Canonical stage keys',
    value: LEGAL_DESIGN_PARTNER_STAGE_KEYS.join(', '),
    publicSourceRef: legalTemplateSourceRef,
  },
  {
    label: 'Starter accepted outcome',
    value:
      'Forms/intake copilot starter for an NDA-style draft packet with review checklist, source-linked assumptions, suggested time entry, and explicit human-review gate.',
    publicSourceRef: legalWorkspaceSourceRef,
  },
  {
    label: 'Legal safety boundary',
    value:
      'Workflow assistance only. Do not issue legal advice, decide rights-impacting questions, or imply attorney review. Jurisdiction-sensitive decisions must route to a qualified human reviewer.',
    publicSourceRef: legalTemplateSourceRef,
  },
  {
    label: 'Matter Signal input',
    value:
      'Start from an intake form, document upload, client question, deadline, or workflow request. Keep requester identity, confidentiality class, and jurisdiction caveat explicit.',
    publicSourceRef: legalTemplateSourceRef,
  },
  {
    label: 'Intake Triage output',
    value:
      'Produce a scoped workflow with document type, jurisdiction caveat, reviewer lane, risk flags, missing-info list, and human-review requirement.',
    publicSourceRef: legalTemplateSourceRef,
  },
  {
    label: 'Draft Assembly output',
    value:
      'Prepare a draft form packet, NDA checklist, clause comparison, issue list, or intake summary with source citation map and assumptions list.',
    publicSourceRef: legalTemplateSourceRef,
  },
  {
    label: 'Legal Review Gate',
    value:
      'Before client-ready release, verify citation coverage, redaction, jurisdiction warning, policy/risk flags, reviewer decision, and whether the packet is safe to share or blocked.',
    publicSourceRef: legalTemplateSourceRef,
  },
  {
    label: 'Authority blocker',
    value:
      'Do not deliver externally, update a matter system, or record billable time without reviewer acceptance, visibility class, delivery permission, and time-entry approval.',
    publicSourceRef: legalWorkspaceSourceRef,
  },
  {
    label: 'Measurement contract',
    value:
      'Receipt must separate source refs, review decision, retained assumptions, suggested time entry, delivery caveats, follow-up tasks, and freshness timestamp.',
    publicSourceRef: legalTemplateSourceRef,
  },
]

const legalStarterWorkflows = (): ReadonlyArray<StarterWorkflow> => [
  {
    title: 'NDA intake packet',
    description:
      'Assemble an NDA-style intake summary and draft packet from source-linked public-safe inputs, carrying jurisdiction caveats, assumptions, and missing-info blockers.',
    outcomeKind: 'legal_intake_packet',
    status: 'queued',
  },
  {
    title: 'Review checklist',
    description:
      'Produce a reviewer-facing checklist for citations, redaction, jurisdiction warnings, policy/risk flags, and whether the packet is safe to share, blocked, or needs more info.',
    outcomeKind: 'legal_review_checklist',
    status: 'queued',
  },
  {
    title: 'Suggested time entry and handoff',
    description:
      'Draft a non-billable suggested time-entry note plus matter handoff summary, source map, retained assumptions, delivery caveats, and follow-up tasks.',
    outcomeKind: 'legal_time_entry_handoff',
    status: 'queued',
  },
]

export const makeLegalDesignPartnerWorkspaceInput =
  (): CreatePrefilledWorkspaceInput => ({
    holderRef: 'design_partner.legal.forms_intake_copilot.v1',
    projectName: 'Forms Intake Copilot Workspace',
    status: 'draft',
    seededMemory: legalSeededMemory(),
    starterWorkflows: legalStarterWorkflows(),
    introReceipt: {
      summary:
        'Seeded the first legal design-partner workspace with the Forge legal stage template, forms/intake copilot starter, NDA-style packet workflow, source-linked review checklist, suggested time-entry handoff, and strict human-review/no-legal-advice boundaries. This is public-safe seed material only; live matter data, confidential documents, delivery, and time-entry authority stay blocked until the holder connects accounts and a qualified reviewer approves them.',
      publicSourceRefs: [...LEGAL_DESIGN_PARTNER_SOURCE_REFS],
    },
  })

const marketingAgencySeededMemory = (): ReadonlyArray<SeededMemoryEntry> => [
  {
    label: 'Selected Forge template',
    value: `${MARKETING_AGENCY_DESIGN_PARTNER_TEMPLATE_REF}: use the marketing-agency vertical mapping for Client Signal, Creative Triage, Creative Production, Brand QA, Client Approval, Account Handoff, Campaign Watch, and Channel Launch.`,
    publicSourceRef: marketingAgencyTemplateSourceRef,
  },
  {
    label: 'Canonical stage keys',
    value: MARKETING_AGENCY_DESIGN_PARTNER_STAGE_KEYS.join(', '),
    publicSourceRef: marketingAgencyTemplateSourceRef,
  },
  {
    label: 'Starter accepted outcomes',
    value:
      'Client landing page plus welcome email in the agency brand for a white-label subdomain, paired with an operator-on-Autopilot admin lane so the agency can run its own Forge work.',
    publicSourceRef: marketingAgencyWorkspaceSourceRef,
  },
  {
    label: 'Client Signal input',
    value:
      'Start from a client brief, brand guideline, analytics issue, launch date, stakeholder comment, or agency internal operator need. Keep brand-source refs and audience caveats explicit.',
    publicSourceRef: marketingAgencyTemplateSourceRef,
  },
  {
    label: 'Creative Triage output',
    value:
      'Produce a scoped deliverable with audience, channel, tone, success metric, approval lane, brand-fit checks, missing assets, and stakeholder dependencies.',
    publicSourceRef: marketingAgencyTemplateSourceRef,
  },
  {
    label: 'Creative Production output',
    value:
      'Prepare landing-page structure, welcome-email copy, ad/social variants, campaign plan, asset brief, or operator-admin lane setup with source and brand maps.',
    publicSourceRef: marketingAgencyTemplateSourceRef,
  },
  {
    label: 'Brand QA gate',
    value:
      'Before approval, verify brand consistency, factuality, accessibility, link and CTA behavior, channel fit, white-label domain state, and reviewer notes.',
    publicSourceRef: marketingAgencyTemplateSourceRef,
  },
  {
    label: 'Authority blocker',
    value:
      'Do not publish a landing page, send email, configure DNS/subdomain, claim white-label delivery, or operate client/admin lanes without client approval, domain authority, channel access, and delivery permission.',
    publicSourceRef: marketingAgencyWorkspaceSourceRef,
  },
  {
    label: 'Agency operator lane',
    value:
      'The agency is both customer and white-label channel. Seed an admin lane for running its own accepted-outcome work on Forge, separate from client-facing approvals and receipts.',
    publicSourceRef: marketingAgencyWorkspaceSourceRef,
  },
  {
    label: 'Measurement contract',
    value:
      'Receipt must separate approved deliverable refs, white-label subdomain state, email send/scheduled state, operator-lane acceptance, metric window, attribution caveat, and freshness timestamp.',
    publicSourceRef: marketingAgencyTemplateSourceRef,
  },
]

const marketingAgencyStarterWorkflows = (): ReadonlyArray<StarterWorkflow> => [
  {
    title: 'White-label landing page',
    description:
      'Draft the agency-branded landing-page structure for a white-label subdomain with brand-source refs, audience, CTA, accessibility checks, and domain/publish authority blockers.',
    outcomeKind: 'agency_white_label_landing_page',
    status: 'queued',
  },
  {
    title: 'Welcome email',
    description:
      'Draft the agency-branded welcome email with source-linked claims, channel caveats, approval state, send/schedule blocker, and measurement handoff.',
    outcomeKind: 'agency_welcome_email',
    status: 'queued',
  },
  {
    title: 'Operator on Autopilot admin lane',
    description:
      'Set up the agency internal admin lane for running its own accepted-outcome work on Forge, with separate approvals, owner refs, and receipts from client-facing work.',
    outcomeKind: 'agency_operator_autopilot_lane',
    status: 'queued',
  },
]

export const makeMarketingAgencyDesignPartnerWorkspaceInput =
  (): CreatePrefilledWorkspaceInput => ({
    holderRef: 'design_partner.marketing_agency.white_label_launch.v1',
    projectName: 'Agency White-Label Launch Workspace',
    status: 'draft',
    seededMemory: marketingAgencySeededMemory(),
    starterWorkflows: marketingAgencyStarterWorkflows(),
    introReceipt: {
      summary:
        'Seeded the first marketing-agency design-partner workspace with the Forge agency stage template, agency-branded landing page, welcome email, white-label subdomain delivery contract, and operator-on-Autopilot admin lane. This is public-safe seed material only; live brand assets, DNS/subdomain control, email channel access, client delivery, and admin-lane authority stay blocked until the holder connects accounts and approves them.',
      publicSourceRefs: [...MARKETING_AGENCY_DESIGN_PARTNER_SOURCE_REFS],
    },
  })
