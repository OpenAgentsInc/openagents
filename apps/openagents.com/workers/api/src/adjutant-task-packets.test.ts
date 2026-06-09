import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { AdjutantAssignment } from './adjutant-assignments'
import type { AdjutantResearchBrief } from './adjutant-research-briefs'
import { buildAdjutantTaskPacket } from './adjutant-task-packets'

const assignment = (): AdjutantAssignment => ({
  id: 'adjutant_assignment_otec',
  softwareOrderId: 'software_order_otec',
  siteId: 'site_project_otec',
  goalId: 'agent_goal_otec',
  currentRunId: null,
  teamId: 'team_openagents_core',
  projectId: 'project_adjutant',
  agentId: 'adjutant',
  assignedByUserId: 'github:14167547',
  assignmentKind: 'site_generation',
  status: 'draft',
  visibility: 'public',
  taskSpecPath: null,
  commitSha: null,
  objective:
    'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
  createdAt: '2026-06-05T00:00:00.000Z',
  updatedAt: '2026-06-05T00:00:00.000Z',
  completedAt: null,
  blockedAt: null,
  archivedAt: null,
})

const researchBrief = (
  overrides: Partial<AdjutantResearchBrief> = {},
): AdjutantResearchBrief => ({
  id: 'adjutant_research_brief_otec',
  assignmentId: 'adjutant_assignment_otec',
  enrichmentRunId: 'exa_enrichment_run_otec',
  status: 'approved',
  summary: 'Approved public evidence supports OTEC and SWAC context.',
  groundedFacts: ['OTEC uses ocean temperature gradients for energy.'],
  suggestedSections: ['Explain OTEC and SWAC infrastructure together.'],
  unknowns: ['Confirm customer-specific deployment constraints.'],
  claimsNeedingReview: ['Gigawatt-scale claims need careful sourcing.'],
  sourceCards: [
    {
      id: 'exa_enrichment_source_otec',
      title: 'OTEC overview',
      url: 'https://example.com/otec',
      domain: 'example.com',
      highlightText: 'Ocean thermal energy conversion context.',
    },
  ],
  createdByUserId: 'github:operator',
  reviewedByUserId: 'github:operator',
  reviewReason: 'Approved for public launch smoke.',
  approvedAt: '2026-06-05T00:00:00.000Z',
  rejectedAt: null,
  createdAt: '2026-06-05T00:00:00.000Z',
  updatedAt: '2026-06-05T00:00:00.000Z',
  archivedAt: null,
  ...overrides,
})

describe('buildAdjutantTaskPacket research brief context', () => {
  test('includes an approved research brief with sourced context', async () => {
    const packet = await Effect.runPromise(
      buildAdjutantTaskPacket({
        assignment: assignment(),
        commitSha: 'abcdef1',
        researchBrief: researchBrief(),
        site: {
          id: 'site_project_otec',
          slug: 'otec',
          title: 'OTEC Floating Datacenter',
        },
      }),
    )

    expect(packet.markdown).toContain('## Approved Research Brief')
    expect(packet.markdown).toContain(
      '- researchBriefId: adjutant_research_brief_otec',
    )
    expect(packet.markdown).toContain(
      'OTEC uses ocean temperature gradients for energy.',
    )
    expect(packet.markdown).toContain('OTEC overview: https://example.com/otec')
  })

  test('includes explicit visual asset requirements when the objective asks for images', async () => {
    const packet = await Effect.runPromise(
      buildAdjutantTaskPacket({
        assignment: {
          ...assignment(),
          objective:
            'Revise the OTEC Site and add images of ocean infrastructure.',
        },
        commitSha: 'abcdef1',
        site: {
          id: 'site_project_otec',
          slug: 'otec',
          title: 'OTEC Floating Datacenter',
        },
      }),
    )

    expect(packet.markdown).toContain('## Visual Asset Requirements')
    expect(packet.markdown).toContain(
      'image: required from customer_request',
    )
    expect(packet.markdown).toContain(
      'CSS-only diagrams do not satisfy requested image media',
    )
  })

  test('rejects unapproved research briefs before dispatch', async () => {
    await expect(
      Effect.runPromise(
        buildAdjutantTaskPacket({
          assignment: assignment(),
          commitSha: 'abcdef1',
          researchBrief: researchBrief({
            approvedAt: null,
            status: 'needs_review',
          }),
          site: null,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantTaskPacketValidationError',
    })
  })

  test('still rejects secret-shaped approved brief material', async () => {
    await expect(
      Effect.runPromise(
        buildAdjutantTaskPacket({
          assignment: assignment(),
          commitSha: 'abcdef1',
          researchBrief: researchBrief({
            groundedFacts: ['Bearer sk-secret123456789 must not dispatch.'],
          }),
          site: null,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantTaskPacketUnsafe',
    })
  })
})
