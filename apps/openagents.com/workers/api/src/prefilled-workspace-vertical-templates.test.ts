import { describe, expect, test } from 'vitest'

import { makePrefilledWorkspaceRecord } from './prefilled-workspace'
import {
  ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS,
  ECOMMERCE_DESIGN_PARTNER_STAGE_KEYS,
  ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF,
  LEGAL_DESIGN_PARTNER_SOURCE_REFS,
  LEGAL_DESIGN_PARTNER_STAGE_KEYS,
  LEGAL_DESIGN_PARTNER_TEMPLATE_REF,
  makeEcommerceDesignPartnerWorkspaceInput,
  makeLegalDesignPartnerWorkspaceInput,
} from './prefilled-workspace-vertical-templates'

const fixtureRuntime = {
  makeId: (prefix: string) => `${prefix}_fixture`,
  nowIso: () => '2026-06-16T18:00:00.000Z',
}

const flattenTemplateInput = (
  input: ReturnType<typeof makeEcommerceDesignPartnerWorkspaceInput>,
): string =>
  [
    ...(input.seededMemory ?? []).flatMap(entry => [
      entry.label,
      entry.value,
      entry.publicSourceRef,
    ]),
    ...(input.starterWorkflows ?? []).flatMap(workflow => [
      workflow.title,
      workflow.description,
      workflow.outcomeKind,
      workflow.status,
    ]),
    input.introReceipt.summary,
    ...input.introReceipt.publicSourceRefs,
  ]
    .join(' ')
    .toLowerCase()

describe('e-commerce prefilled workspace template', () => {
  test('creates a valid draft workspace record from public-safe seed material', () => {
    const input = makeEcommerceDesignPartnerWorkspaceInput()
    const record = makePrefilledWorkspaceRecord(input, fixtureRuntime)

    expect(record.id).toBe('workspace_fixture')
    expect(record.projectName).toBe('Inventory-Aware Campaign Workspace')
    expect(record.holderRef).toBe(
      'design_partner.ecommerce.inventory_campaign.v1',
    )
    expect(record.status).toBe('draft')
    expect(record.holderUserId).toBeNull()
    expect(record.seededMemory.length).toBeGreaterThanOrEqual(8)
    expect(record.starterWorkflows).toHaveLength(3)
    expect(record.introReceipt.publicSourceRefs).toEqual(
      expect.arrayContaining([...ECOMMERCE_DESIGN_PARTNER_SOURCE_REFS]),
    )
    expect(record.introReceipt.summary).toContain('public-safe seed material')
  })

  test('preserves the Forge canonical stage keys and e-commerce template ref', () => {
    const input = makeEcommerceDesignPartnerWorkspaceInput()
    const templateMemory = input.seededMemory?.find(
      entry => entry.label === 'Selected Forge template',
    )
    const canonicalStageMemory = input.seededMemory?.find(
      entry => entry.label === 'Canonical stage keys',
    )

    expect(templateMemory?.value).toContain(
      ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF,
    )
    expect(canonicalStageMemory?.value).toBe(
      ECOMMERCE_DESIGN_PARTNER_STAGE_KEYS.join(', '),
    )
    expect(input.introReceipt.publicSourceRefs).toContain(
      'github.issue.OpenAgentsInc.openagents.5099',
    )
  })

  test('seeds the requested stock, imagery, spend-cap, and receipt gates', () => {
    const input = makeEcommerceDesignPartnerWorkspaceInput()
    const flattenedSeed = flattenTemplateInput(input)

    expect(flattenedSeed).toContain('in-stock')
    expect(flattenedSeed).toContain('stock')
    expect(flattenedSeed).toContain('imagery')
    expect(flattenedSeed).toContain('spend cap')
    expect(flattenedSeed).toContain('receipt')
    expect(flattenedSeed).toContain('stats')
    expect(flattenedSeed).toContain('approval')
    expect(flattenedSeed).toContain('do not publish')
  })

  test('keeps every seeded-memory fact backed by a public source reference', () => {
    const input = makeEcommerceDesignPartnerWorkspaceInput()

    expect(input.seededMemory).toBeDefined()
    for (const entry of input.seededMemory ?? []) {
      expect(entry.publicSourceRef).not.toBe('')
      expect(entry.publicSourceRef).not.toMatch(/secret|token|password/i)
    }
  })
})

describe('legal prefilled workspace template', () => {
  test('creates a valid draft workspace record from public-safe seed material', () => {
    const input = makeLegalDesignPartnerWorkspaceInput()
    const record = makePrefilledWorkspaceRecord(input, fixtureRuntime)

    expect(record.id).toBe('workspace_fixture')
    expect(record.projectName).toBe('Forms Intake Copilot Workspace')
    expect(record.holderRef).toBe(
      'design_partner.legal.forms_intake_copilot.v1',
    )
    expect(record.status).toBe('draft')
    expect(record.holderUserId).toBeNull()
    expect(record.seededMemory.length).toBeGreaterThanOrEqual(10)
    expect(record.starterWorkflows).toHaveLength(3)
    expect(record.introReceipt.publicSourceRefs).toEqual(
      expect.arrayContaining([...LEGAL_DESIGN_PARTNER_SOURCE_REFS]),
    )
    expect(record.introReceipt.summary).toContain('public-safe seed material')
  })

  test('preserves the Forge canonical stage keys and legal template ref', () => {
    const input = makeLegalDesignPartnerWorkspaceInput()
    const templateMemory = input.seededMemory?.find(
      entry => entry.label === 'Selected Forge template',
    )
    const canonicalStageMemory = input.seededMemory?.find(
      entry => entry.label === 'Canonical stage keys',
    )

    expect(templateMemory?.value).toContain(LEGAL_DESIGN_PARTNER_TEMPLATE_REF)
    expect(canonicalStageMemory?.value).toBe(
      LEGAL_DESIGN_PARTNER_STAGE_KEYS.join(', '),
    )
    expect(input.introReceipt.publicSourceRefs).toContain(
      'github.issue.OpenAgentsInc.openagents.5100',
    )
  })

  test('seeds the requested intake, review, source-link, and time-entry gates', () => {
    const input = makeLegalDesignPartnerWorkspaceInput()
    const flattenedSeed = flattenTemplateInput(input)

    expect(flattenedSeed).toContain('nda')
    expect(flattenedSeed).toContain('intake')
    expect(flattenedSeed).toContain('review checklist')
    expect(flattenedSeed).toContain('source-linked')
    expect(flattenedSeed).toContain('suggested time entry')
    expect(flattenedSeed).toContain('human-review')
    expect(flattenedSeed).toContain('no-legal-advice')
    expect(flattenedSeed).toContain('do not deliver')
  })

  test('keeps every legal seeded-memory fact backed by a public source reference', () => {
    const input = makeLegalDesignPartnerWorkspaceInput()

    expect(input.seededMemory).toBeDefined()
    for (const entry of input.seededMemory ?? []) {
      expect(entry.publicSourceRef).not.toBe('')
      expect(entry.publicSourceRef).not.toMatch(/secret|token|password/i)
    }
  })
})
