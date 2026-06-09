import { describe, expect, test } from 'vitest'

import {
  ArtanisProductionLaunchGateUnsafe,
  assertArtanisContinuousAutonomyClaimAllowed,
  exampleArtanisProductionLaunchGateRecord,
  projectArtanisProductionLaunchGate,
} from './artanis-production-launch-gate'

const nowIso = '2026-06-06T21:30:00.000Z'

describe('Artanis production launch gate', () => {
  test('allows bounded status-projection claims while blocking autonomy authority', () => {
    const projection = projectArtanisProductionLaunchGate(
      exampleArtanisProductionLaunchGateRecord(nowIso),
      nowIso,
    )

    expect(projection).toMatchObject({
      agentRef: 'agent.public.artanis',
      canClaimBoundedStatusProjection: true,
      canClaimContinuouslyRunning: false,
      dispatchAuthorityAllowed: false,
      environmentRef: 'env.production.openagents.worker',
      failedOrPendingRequiredCount: 0,
      forumAutoPublishAllowed: false,
      gateRef: 'gate.public.artanis.production_launch.v1',
      providerMutationAuthorityAllowed: false,
      settlementAuthorityAllowed: false,
      state: 'ready',
      stateLabel: 'Ready for controlled production enablement',
      walletSpendAuthorityAllowed: false,
    })
    expect(projection.blockerRefs).toEqual([])
    expect(projection.blockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.production_e2e_smoke.blocked',
    )
    expect(projection.blockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.scheduled_runner.blocked',
    )
    expect(projection.requiredIssueRefs).toEqual(
      expect.arrayContaining([
        'issue:#511',
        'issue:#512',
        'issue:#403',
        'issue:#405',
        'issue:#406',
        'issue:#407',
        'issue:#408',
        'issue:#410',
        'issue:#411',
        'issue:#412',
        'issue:#413',
        'issue:#414',
      ]),
    )
    expect(projection.testRefs).toEqual(
      expect.arrayContaining([
        'test:workers/api/src/artanis-production-launch-gate.test.ts',
        'test:workers/api/src/artanis-gepa-production-smoke.test.ts',
        'test:workers/api/src/artanis-gepa-scheduled-runner-proof.test.ts',
        'test:workers/api/src/artanis-scheduled-runner.test.ts',
        'test:workers/api/src/artanis-public-report.test.ts',
      ]),
    )
    expect(projection.routeRefs).toEqual(
      expect.arrayContaining([
        'route:/artanis',
        'route:/autopilot',
        'route:/api/operator/artanis/console',
        'route:/api/operator/artanis/approval-gates/{gateRef}/approve',
        'route:/api/operator/artanis/approval-gates/{gateRef}/reject',
        'route:/api/public/artanis/report',
        'route:/api/public/pylon-stats',
      ]),
    )
    expect(projection.verificationTargetRefs).toEqual(
      expect.arrayContaining([
        'route:/artanis',
        'route:/api/public/artanis/report',
        'route:/api/public/pylon-stats',
        'route:/api/operator/artanis/console',
        'route:/api/operator/artanis/approval-gates',
        'route:/autopilot',
        'signal.public.artanis.health_staleness',
        'topic.public.forum.artanis.status',
      ]),
    )
    expect(projection.runbookCommandRefs).toEqual(
      expect.arrayContaining([
        'runbook.public.artanis.production_launch.check',
        'runbook.public.artanis.production_launch.disable',
        'runbook.public.artanis.production_launch.enable',
        'runbook.public.artanis.production_launch.pause',
        'runbook.public.artanis.production_launch.recover',
        'runbook.public.artanis.production_launch.revoke',
      ]),
    )
    expect(projection.rollbackRefs).toEqual(
      expect.arrayContaining([
        'rollback.public.artanis.dispatch_mistake',
        'rollback.public.artanis.payment_reward_mistake',
        'rollback.public.artanis.public_claim_mistake',
        'rollback.public.artanis.publication_mistake',
      ]),
    )
    expect(() =>
      assertArtanisContinuousAutonomyClaimAllowed(
        projection,
        'Artanis is continuously running autonomously.',
      ),
    ).toThrow(ArtanisProductionLaunchGateUnsafe)
    expect(() =>
      assertArtanisContinuousAutonomyClaimAllowed(
        projection,
        'Artanis has a public evidence surface and operator-gated launch path.',
      ),
    ).not.toThrow()
    expect(JSON.stringify(projection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('blocks continuous-autonomy claims if scheduled runner evidence regresses', () => {
    const record = exampleArtanisProductionLaunchGateRecord(nowIso)
    const blockedRecord = {
      ...record,
      checks: record.checks.map(check =>
        check.category === 'scheduled_runner'
          ? { ...check, status: 'blocked' as const }
          : check,
      ),
    }
    const projection = projectArtanisProductionLaunchGate(blockedRecord, nowIso)

    expect(projection).toMatchObject({
      canClaimBoundedStatusProjection: false,
      canClaimContinuouslyRunning: false,
      failedOrPendingRequiredCount: 1,
      state: 'blocked',
      stateLabel: 'Blocked before autonomous public claims',
    })
    expect(projection.blockerRefs).toContain(
      'blocker.public.artanis.launch_gate.scheduled_runner.blocked',
    )
    expect(() =>
      assertArtanisContinuousAutonomyClaimAllowed(
        projection,
        'Artanis is continuously running autonomously.',
      ),
    ).toThrow(ArtanisProductionLaunchGateUnsafe)
  })

  test('rejects gates missing required verification targets', () => {
    const record = exampleArtanisProductionLaunchGateRecord(nowIso)

    expect(() =>
      projectArtanisProductionLaunchGate(
        {
          ...record,
          verificationTargets: record.verificationTargets.filter(
            target => target.targetRef !== 'route:/autopilot',
          ),
        },
        nowIso,
      ),
    ).toThrow(ArtanisProductionLaunchGateUnsafe)
  })

  test('rejects concrete runbook commands that expose literal secrets', () => {
    const record = exampleArtanisProductionLaunchGateRecord(nowIso)

    expect(() =>
      projectArtanisProductionLaunchGate(
        {
          ...record,
          runbookCommands: record.runbookCommands.map(command =>
            command.kind === 'check'
              ? {
                  ...command,
                  command:
                    'curl -H "Authorization: Bearer sk-live-should-not-appear" https://openagents.com/api/operator/artanis/console',
                }
              : command,
          ),
        },
        nowIso,
      ),
    ).toThrow(ArtanisProductionLaunchGateUnsafe)
  })
})
