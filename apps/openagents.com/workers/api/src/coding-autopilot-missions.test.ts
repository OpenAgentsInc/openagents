import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CodingAutopilotMissionProjection,
  CodingAutopilotMissionRecord,
  CodingAutopilotMissionUnsafe,
  codingAutopilotMissionProjectionHasPrivateMaterial,
  exampleCodingAutopilotMissionRecord,
  projectCodingAutopilotMissionRecord,
} from './coding-autopilot-missions'

const nowIso = '2026-06-06T21:05:00.000Z'

describe('Coding on Autopilot mission records', () => {
  test('projects a Site mission for public, customer, team, and operator audiences', () => {
    const record = exampleCodingAutopilotMissionRecord()
    const publicProjection = projectCodingAutopilotMissionRecord(record, 'public', nowIso)
    const customerProjection = projectCodingAutopilotMissionRecord(record, 'customer', nowIso)
    const teamProjection = projectCodingAutopilotMissionRecord(record, 'team', nowIso)
    const operatorProjection = projectCodingAutopilotMissionRecord(record, 'operator', nowIso)

    expect(S.decodeUnknownSync(CodingAutopilotMissionRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(CodingAutopilotMissionProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      audience: 'public',
      latestBriefingRef: 'briefing.continuation.otec_revision_4.latest',
      missionRef: 'mission.otec_revision_4',
      status: 'waiting_for_review',
      statusLabel: 'Waiting for review',
      updatedAtDisplay: '5 minutes ago',
      workKind: 'site',
    })
    expect(publicProjection.customerRefs).toEqual([])
    expect(publicProjection.teamRefs).toEqual([])
    expect(publicProjection.workroomRefs).toEqual([])
    expect(publicProjection.assignmentRefs).toEqual([])
    expect(publicProjection.routeScorecardRefs).toEqual([])
    expect(publicProjection.accountLeaseRefs).toEqual([])
    expect(publicProjection.budgetRefs).toEqual([])
    expect(customerProjection.customerRefs).toEqual(['customer_ref.order_otec'])
    expect(customerProjection.workroomRefs).toEqual(['workroom.otec_site_revision_4'])
    expect(customerProjection.routeScorecardRefs).toEqual([])
    expect(teamProjection.teamRefs).toEqual(['team_ref.sites_fulfillment'])
    expect(teamProjection.routeScorecardRefs).toEqual([
      'route_scorecard.codex_container_to_site_build',
    ])
    expect(operatorProjection.accountLeaseRefs).toEqual([
      'account_lease.codex_3.run_otec_revision_4',
    ])
    expect(operatorProjection.budgetRefs).toEqual([
      'budget.internal_free_beta.otec_revision_4',
    ])
  })

  test('does not expose raw timestamps in mission projections', () => {
    const projection = projectCodingAutopilotMissionRecord(
      exampleCodingAutopilotMissionRecord(),
      'customer',
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toContain('2026-06-06T21:00:00.000Z')
    expect(serialized).not.toContain('2026-06-06T20:15:00.000Z')
    expect(codingAutopilotMissionProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('filters account lease, route, and budget refs outside allowed audiences', () => {
    const record = {
      ...exampleCodingAutopilotMissionRecord(),
      budgetRefs: [
        'budget.customer_visible_estimate',
        'budget_private.provider_runtime_spend',
      ],
      routeScorecardRefs: [
        'route_scorecard.public_safe_summary',
      ],
    }
    const publicProjection = projectCodingAutopilotMissionRecord(record, 'public', nowIso)
    const customerProjection = projectCodingAutopilotMissionRecord(record, 'customer', nowIso)
    const teamProjection = projectCodingAutopilotMissionRecord(record, 'team', nowIso)

    expect(publicProjection.routeScorecardRefs).toEqual([])
    expect(publicProjection.budgetRefs).toEqual([])
    expect(customerProjection.routeScorecardRefs).toEqual([])
    expect(customerProjection.budgetRefs).toEqual([])
    expect(teamProjection.routeScorecardRefs).toEqual([
      'route_scorecard.public_safe_summary',
    ])
    expect(teamProjection.accountLeaseRefs).toEqual([])
  })

  test('lets Coding on Autopilot workrooms carry Probe GEPA route scorecards for team and operator audiences', () => {
    const record = {
      ...exampleCodingAutopilotMissionRecord(),
      routeScorecardRefs: [
        'route_scorecard.probe_gepa.live_stage0.demo_1',
        'route_scorecard.probe_gepa.live_stage0.demo_2',
      ],
      workKind: 'coding' as const,
      workroomRefs: ['workroom.coding_autopilot.probe_gepa.live_stage0'],
    }
    const publicProjection = projectCodingAutopilotMissionRecord(record, 'public', nowIso)
    const teamProjection = projectCodingAutopilotMissionRecord(record, 'team', nowIso)
    const operatorProjection = projectCodingAutopilotMissionRecord(record, 'operator', nowIso)

    expect(publicProjection.routeScorecardRefs).toEqual([])
    expect(teamProjection.routeScorecardRefs).toEqual([
      'route_scorecard.probe_gepa.live_stage0.demo_1',
      'route_scorecard.probe_gepa.live_stage0.demo_2',
    ])
    expect(teamProjection.workroomRefs).toEqual([
      'workroom.coding_autopilot.probe_gepa.live_stage0',
    ])
    expect(operatorProjection.routeScorecardRefs).toEqual(
      teamProjection.routeScorecardRefs,
    )
  })

  test('rejects raw runner logs, provider account refs, private repo refs, customer emails, and secrets', () => {
    expect(() =>
      projectCodingAutopilotMissionRecord({
        ...exampleCodingAutopilotMissionRecord(),
        artifactRefs: ['raw_runner_log.otec_revision_4'],
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotMissionUnsafe)
    expect(() =>
      projectCodingAutopilotMissionRecord({
        ...exampleCodingAutopilotMissionRecord(),
        routeScorecardRefs: ['provider_account.codex_private'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotMissionUnsafe)
    expect(() =>
      projectCodingAutopilotMissionRecord({
        ...exampleCodingAutopilotMissionRecord(),
        objectiveStackRefs: ['private_repo:https://github.com/customer/private-repo'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotMissionUnsafe)
    expect(() =>
      projectCodingAutopilotMissionRecord({
        ...exampleCodingAutopilotMissionRecord(),
        customerRefs: ['ben@example.com'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotMissionUnsafe)
    expect(() =>
      projectCodingAutopilotMissionRecord({
        ...exampleCodingAutopilotMissionRecord(),
        blockerRefs: ['secret:provider-token'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotMissionUnsafe)
  })
})
