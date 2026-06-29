import { describe, expect, test } from 'vitest'

import {
  makePrefilledWorkspaceRecord,
  toPublicProjection,
} from './prefilled-workspace'

const runtime = {
  makeId: (prefix: string) => `${prefix}_fixed`,
  nowIso: () => '2026-06-16T12:00:00.000Z',
}

describe('prefilled workspace schema', () => {
  test('builds a record with defaults and a derived holder ref', () => {
    const record = makePrefilledWorkspaceRecord(
      {
        projectName: 'The Hardware Shop',
        introReceipt: {
          summary: 'Seeded from public storefront.',
          publicSourceRefs: ['https://example.com'],
        },
      },
      runtime,
    )

    expect(record.id).toBe('workspace_fixed')
    expect(record.holderUserId).toBeNull()
    expect(record.holderRef).toBe('workspace-the-hardware-shop-h_fixed')
    expect(record.status).toBe('draft')
    expect(record.seededMemory).toEqual([])
    expect(record.starterWorkflows).toEqual([])
    expect(record.createdAt).toBe('2026-06-16T12:00:00.000Z')
    expect(record.updatedAt).toBe('2026-06-16T12:00:00.000Z')
  })

  test('clamps and caps seeded memory and starter workflows', () => {
    const record = makePrefilledWorkspaceRecord(
      {
        projectName: 'Project',
        holderRef: 'prospect-ref-001',
        holderUserId: 'github:holder',
        status: 'invited',
        introReceipt: {
          summary: 'What we set up.',
          publicSourceRefs: ['ref-a', 'ref-b'],
        },
        starterWorkflows: [
          {
            title: 'A',
            description: 'a',
            outcomeKind: 'draft',
            status: 'queued',
          },
          {
            title: 'B',
            description: 'b',
            outcomeKind: 'campaign',
            status: 'ready',
          },
          {
            title: 'C',
            description: 'c',
            outcomeKind: 'landing_page',
            status: 'queued',
          },
          {
            title: 'D-overflow',
            description: 'd',
            outcomeKind: 'draft',
            status: 'queued',
          },
        ],
        seededMemory: [
          {
            label: '  Brand  voice ',
            value: 'friendly',
            publicSourceRef: 'https://example.com/about',
          },
        ],
      },
      runtime,
    )

    expect(record.holderRef).toBe('prospect-ref-001')
    expect(record.holderUserId).toBe('github:holder')
    expect(record.status).toBe('invited')
    // 1-3 starter workflows: the 4th is dropped.
    expect(record.starterWorkflows).toHaveLength(3)
    expect(record.starterWorkflows.map(w => w.title)).toEqual(['A', 'B', 'C'])
    // Whitespace is collapsed.
    expect(record.seededMemory[0]?.label).toBe('Brand voice')
  })

  test('public projection drops operator-only holder binding', () => {
    const record = makePrefilledWorkspaceRecord(
      {
        projectName: 'Project',
        holderUserId: 'github:holder',
        introReceipt: { summary: 's', publicSourceRefs: [] },
        seededMemory: [
          { label: 'l', value: 'v', publicSourceRef: 'https://src' },
        ],
      },
      runtime,
    )

    const projection = toPublicProjection(record)

    expect(projection).not.toHaveProperty('holderUserId')
    expect(projection).not.toHaveProperty('holderRef')
    expect(projection.projectName).toBe('Project')
    // Seeded-memory provenance is preserved in the public projection.
    expect(projection.seededMemory[0]?.publicSourceRef).toBe('https://src')
  })
})
