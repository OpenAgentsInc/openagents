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

const templateSourceRef = ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS[0]
const workspaceSourceRef = ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS[1]

const seededMemory = (): ReadonlyArray<SeededMemoryEntry> => [
  {
    label: 'Selected Forge template',
    value: `${ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF}: use the e-commerce vertical mapping for Demand Signal, Offer Triage, Campaign Build, Commerce QA, Merchandising Release, Merchant Handoff, Conversion Watch, and Channel Publish.`,
    publicSourceRef: templateSourceRef,
  },
  {
    label: 'Canonical stage keys',
    value: ECOMMERCE_DESIGN_PARTNER_STAGE_KEYS.join(', '),
    publicSourceRef: templateSourceRef,
  },
  {
    label: 'Starter accepted outcome',
    value:
      'Inventory-aware ad-campaign workflow that uses real in-stock products only, accurate product imagery and source refs, an explicit spend cap, and a stats plus receipt handoff.',
    publicSourceRef: workspaceSourceRef,
  },
  {
    label: 'Demand Signal input',
    value:
      'Start from a catalog or inventory signal, stock pressure, a seasonal event, or a storefront performance signal. Keep the brief public-safe until the merchant connects accounts.',
    publicSourceRef: templateSourceRef,
  },
  {
    label: 'Offer Triage output',
    value:
      'Produce a prioritized offer plan with eligible SKU set, excluded out-of-stock SKUs, target channel, audience, spend cap, brand caveats, and missing-access blockers.',
    publicSourceRef: templateSourceRef,
  },
  {
    label: 'Commerce QA gate',
    value:
      'Before release, verify real stock, accurate imagery/source references, offer math, links, price and margin caveats, shipping or tax caveats, and channel-policy fit.',
    publicSourceRef: templateSourceRef,
  },
  {
    label: 'Authority blocker',
    value:
      'Do not publish or spend. Channel access, ad account access, merchant approval, spend-cap acceptance, and deployment permission are explicit blockers until receipted.',
    publicSourceRef: workspaceSourceRef,
  },
  {
    label: 'Measurement contract',
    value:
      'Receipt must separate published artifact refs, spend cap, campaign stats window, attribution caveat, stockout or defect follow-up, and freshness timestamp.',
    publicSourceRef: templateSourceRef,
  },
]

const starterWorkflows = (): ReadonlyArray<StarterWorkflow> => [
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
    seededMemory: seededMemory(),
    starterWorkflows: starterWorkflows(),
    introReceipt: {
      summary:
        'Seeded the first e-commerce design-partner workspace with the Forge e-commerce stage template, an inventory-aware ad-campaign starter, Commerce QA gate, authority blockers, and a stats/receipt handoff contract. This is public-safe seed material only; live merchant catalog, channel, and spend authority stay blocked until the holder connects accounts and approves them.',
      publicSourceRefs: [...ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS],
    },
  })
