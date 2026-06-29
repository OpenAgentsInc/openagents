import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CUSTOMER_ONE_COHORT_PROJECTION_VERSION,
  type CustomerOneCohortPrivateRow,
  CustomerOneCohortProjection,
  projectCustomerOneCohort,
} from './customer-one-cohort-projection'

const generatedAt = '2026-06-17T20:00:00.000Z'

const completedRow = (index: number): CustomerOneCohortPrivateRow => ({
  artifactRef: `artifact.customer-one.team-${index}.delivery.v1`,
  completionBundleRef: `completion.customer-one.team-${index}.bundle.v1`,
  privacyReviewRef: `privacy.customer-one.team-${index}.review.v1`,
  reviewRef: `review.customer-one.team-${index}.human.v1`,
  routingRef: `routing.customer-one.team-${index}.owned-node.v1`,
  runRef: `run.customer-one.team-${index}.primary.v1`,
  state: 'loop_completed',
  teamCohortRef: `cohort.team.${index}.v1`,
  templateRef: 'forge.template.ecommerce.inventory_campaign.v1',
  updatedAt: generatedAt,
  verificationRef: `verification.customer-one.team-${index}.smoke.v1`,
  verticalRef: 'vertical.ecommerce.v1',
  workspaceRef: `workspace.customer-one.team-${index}.v1`,
})

describe('customer one cohort projection', () => {
  test('projects an empty awaiting-source state without fake completions', () => {
    const projection = projectCustomerOneCohort({
      generatedAt,
      rows: [],
    })

    expect(projection).toEqual({
      authority: 'evidence_only',
      blockerRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
      caveatRefs: [],
      cohortProjectionVersion: CUSTOMER_ONE_COHORT_PROJECTION_VERSION,
      counts: {
        blocked: 0,
        candidate: 0,
        deferred: 0,
        delivery_reviewed: 0,
        first_run_started: 0,
        invited: 0,
        loop_completed: 0,
        workspace_seeded: 0,
      },
      gate: {
        reasonRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
        state: 'blocked',
      },
      generatedAt,
      rows: [],
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
        rebuildsOn: ['cohort_row_written', 'privacy_review_recorded'],
      },
      target: {
        maximumTargetTeams: 5,
        minimumCompletedTeams: 3,
      },
    })
  })

  test('projects partial rows with generic labels and blocked gate', () => {
    const projection = projectCustomerOneCohort({
      generatedAt,
      rows: [
        {
          candidateRef: 'candidate.customer-one.team-alpha.v1',
          state: 'candidate',
          teamCohortRef: 'cohort.team.alpha.v1',
          updatedAt: generatedAt,
          verticalRef: 'vertical.legal.v1',
        },
        {
          routingRef: 'routing.customer-one.team-beta.fallback.v1',
          runRef: 'run.customer-one.team-beta.primary.v1',
          state: 'first_run_started',
          teamCohortRef: 'cohort.team.beta.v1',
          updatedAt: generatedAt,
          workspaceRef: 'workspace.customer-one.team-beta.v1',
        },
      ],
    })

    expect(projection.counts).toMatchObject({
      candidate: 1,
      first_run_started: 1,
      loop_completed: 0,
    })
    expect(projection.gate).toEqual({
      reasonRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
      state: 'blocked',
    })
    expect(projection.rows.map(row => row.displayLabel)).toEqual([
      'Team 1',
      'Team 2',
    ])
    expect(projection.rows[0]).toMatchObject({
      teamCohortRef: 'cohort.team.alpha.v1',
      verticalRef: 'vertical.legal.v1',
    })
  })

  test('does not count completed rows without completion and privacy refs', () => {
    const projection = projectCustomerOneCohort({
      generatedAt,
      rows: [
        {
          completionBundleRef: 'completion.customer-one.team-gamma.bundle.v1',
          state: 'loop_completed',
          teamCohortRef: 'cohort.team.gamma.v1',
          updatedAt: generatedAt,
        },
        completedRow(1),
      ],
    })

    expect(projection.counts.loop_completed).toBe(1)
    expect(projection.gate.state).toBe('blocked')
    expect(projection.rows[0]).toMatchObject({
      countsTowardD3Completion: false,
      blockerRefs: [
        'customer-one-cohort-blocker:cohort.team.gamma.v1:missing-privacy-review',
      ],
    })
    expect(projection.rows[1]?.countsTowardD3Completion).toBe(true)
  })

  test('opens the gate only after three counted completion bundles', () => {
    const projection = projectCustomerOneCohort({
      generatedAt,
      rows: [completedRow(1), completedRow(2), completedRow(3)],
    })

    expect(projection.counts.loop_completed).toBe(3)
    expect(projection.gate).toEqual({
      reasonRefs: [],
      state: 'ready',
    })
    expect(projection.blockerRefs).toEqual([])
    expect(projection.rows.every(row => row.countsTowardD3Completion)).toBe(
      true,
    )
  })

  test('projection output decodes through the public schema', () => {
    const projection = projectCustomerOneCohort({
      generatedAt,
      rows: [completedRow(1)],
    })

    expect(
      S.decodeUnknownSync(CustomerOneCohortProjection)(projection),
    ).toEqual(projection)
  })

  test('rejects unsafe private material before public projection', () => {
    expect(() =>
      projectCustomerOneCohort({
        generatedAt,
        rows: [
          {
            state: 'candidate',
            teamCohortRef: 'cohort.team.private.v1',
            updatedAt: generatedAt,
            workspaceRef: '/Users/operator/private-repo',
          },
        ],
      }),
    ).toThrow(/private cohort material/)

    expect(() =>
      projectCustomerOneCohort({
        generatedAt,
        rows: [
          {
            sourceUrl: 'https://customer.example/private-workspace',
            state: 'candidate',
            teamCohortRef: 'cohort.team.source.v1',
            updatedAt: generatedAt,
          } as unknown as CustomerOneCohortPrivateRow,
        ],
      }),
    ).toThrow(/private cohort material/)

    expect(() =>
      projectCustomerOneCohort({
        generatedAt,
        rows: [
          {
            caveatRefs: ['raw prompt: build the customer pipeline'],
            state: 'candidate',
            teamCohortRef: 'cohort.team.prompt.v1',
            updatedAt: generatedAt,
          },
        ],
      }),
    ).toThrow(/private cohort material/)

    expect(() =>
      projectCustomerOneCohort({
        generatedAt,
        rows: [
          {
            state: 'candidate',
            teamCohortRef: 'acme-team',
            updatedAt: generatedAt,
          },
        ],
      }),
    ).toThrow(/opaque cohort\.team/)
  })
})
