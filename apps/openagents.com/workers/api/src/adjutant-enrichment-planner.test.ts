import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { AdjutantAssignment } from './adjutant-assignments'
import { makeAdjutantEnrichmentPlanner } from './adjutant-enrichment-planner'
import { teamAdjutantIntentFromBody } from './index'

const assignment = (
  overrides: Partial<AdjutantAssignment> = {},
): AdjutantAssignment => ({
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
  ...overrides,
})

const otecOrder = {
  id: 'software_order_otec',
  repositoryDefaultBranch: 'main',
  repositoryFullName: 'bensilone/openagents',
  repositoryHtmlUrl: 'https://github.com/bensilone/openagents',
  repositoryName: 'openagents',
  repositoryOwner: 'bensilone',
  repositoryPrivate: false,
  request:
    'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
}

const otecSite = {
  id: 'site_project_otec',
  slug: 'otec',
  sourceRepositoryName: 'openagents',
  sourceRepositoryOwner: 'bensilone',
  sourceRepositoryProvider: 'github' as const,
  sourceRepositoryRef: 'main',
  title: 'OTEC Floating Datacenter',
}

describe('AdjutantEnrichmentPlanner', () => {
  test('refuses context-free assignments instead of inferring from prompt keywords', async () => {
    const planner = makeAdjutantEnrichmentPlanner()

    await expect(
      Effect.runPromise(
        planner.buildPlan({
          assignment: assignment({
            siteId: null,
            softwareOrderId: null,
            taskSpecPath: null,
          }),
          order: null,
          site: null,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantEnrichmentPlannerValidationError',
    })
  })

  test('plans topic and repository search from explicit assignment context', async () => {
    const planner = makeAdjutantEnrichmentPlanner()
    const plan = await Effect.runPromise(
      planner.buildPlan({
        assignment: assignment(),
        order: otecOrder,
        site: otecSite,
      }),
    )

    expect(plan.planId).toBe('exa_plan_adjutant_assignment_otec')
    expect(plan.searchTasks.map(task => task.sourceCategory)).toEqual([
      'topic_web',
      'repository',
    ])
    expect(plan.searchTasks[0]?.query).toContain('OTEC powered')
    expect(plan.searchTasks[1]).toMatchObject({
      category: 'github',
      includeDomains: ['github.com'],
      sourceCategory: 'repository',
      urls: ['https://github.com/bensilone/openagents'],
    })
    expect(plan.searchTasks.some(task => task.category === 'people')).toBe(
      false,
    )
  })

  test('does not infer Ben identity enrichment from the OTEC order text alone', async () => {
    const parserIntent = teamAdjutantIntentFromBody(
      '@adjutant Build the Ben OTEC website from the order',
    )
    const planner = makeAdjutantEnrichmentPlanner()
    const plan = await Effect.runPromise(
      planner.buildPlan({
        assignment: assignment(),
        order: {
          ...otecOrder,
          request: 'Ben wants an OTEC and ocean infrastructure website.',
          repositoryFullName: null,
          repositoryHtmlUrl: null,
          repositoryName: null,
          repositoryOwner: null,
        },
        site: otecSite,
      }),
    )

    expect(parserIntent).toEqual({
      schemaVersion: 'openagents.team_chat.adjutant_intent.v1',
      prompt: 'Build the Ben OTEC website from the order',
    })
    expect(plan.searchTasks.some(task => task.category === 'people')).toBe(
      false,
    )
  })

  test('creates people-search and contents tasks only for approved explicit public profile refs', async () => {
    const planner = makeAdjutantEnrichmentPlanner()
    const plan = await Effect.runPromise(
      planner.buildPlan({
        assignment: assignment(),
        explicitSourceRefs: [
          {
            id: 'source_ref_ben_profile',
            kind: 'github_profile',
            label: 'Ben public GitHub profile',
            status: 'approved',
            url: 'https://github.com/bensilone',
          },
          {
            id: 'source_ref_private_guess',
            kind: 'x_profile',
            label: 'Unreviewed social guess',
            status: 'proposed',
            url: 'https://x.com/example',
          },
        ],
        order: otecOrder,
        site: otecSite,
      }),
    )

    expect(plan.contentsTasks).toEqual([
      expect.objectContaining({
        sourceCategory: 'github_profile',
        sourceRefId: 'source_ref_ben_profile',
        urls: ['https://github.com/bensilone'],
      }),
    ])
    expect(plan.searchTasks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          category: 'people',
          sourceCategory: 'github_profile',
          sourceRefId: 'source_ref_ben_profile',
        }),
      ]),
    )
    expect(plan.blockedSources).toEqual([
      {
        reason: 'Source ref has not been approved for enrichment.',
        sourceRefId: 'source_ref_private_guess',
        sourceRefKind: 'x_profile',
        url: 'https://x.com/example',
      },
    ])
  })

  test('blocks rejected and internal-only source refs', async () => {
    const planner = makeAdjutantEnrichmentPlanner()
    const plan = await Effect.runPromise(
      planner.buildPlan({
        assignment: assignment(),
        explicitSourceRefs: [
          {
            id: 'source_ref_internal',
            kind: 'personal_site',
            status: 'internal_only',
            url: 'https://example.com/internal',
          },
          {
            id: 'source_ref_rejected',
            kind: 'generic_url',
            status: 'rejected',
            url: 'https://example.com/rejected',
          },
        ],
        order: otecOrder,
        site: otecSite,
      }),
    )

    expect(plan.contentsTasks).toHaveLength(0)
    expect(plan.blockedSources.map(source => source.reason)).toEqual([
      'Source ref is internal-only and cannot be used for public evidence enrichment.',
      'Source ref was rejected by an operator.',
    ])
  })
})
