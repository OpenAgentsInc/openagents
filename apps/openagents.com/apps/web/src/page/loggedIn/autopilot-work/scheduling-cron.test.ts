import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeSchedulingCronInput,
  projectForgeSchedulingCron,
} from './scheduling-cron'

const baseInput = {
  generatedAt: '2026-06-18T02:20:00.000Z',
  snapshotRef: 'scheduling-cron-snapshot.public.work_1',
  versionRef: 'scheduling-cron-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge scheduling and cron projection', () => {
  test('projects active schedule evidence as refs-only non-authoritative state', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          adapterPreferenceRefs: ['adapter-preference.public.pylon.local'],
          budgetPolicyRefs: ['budget-policy.public.overnight.cap'],
          freshness: 'fresh',
          nextRunRefs: ['next-run.public.2026-06-18T09:00:00Z'],
          notificationPolicyRefs: ['notification-policy.public.completion'],
          ownerRefs: ['owner.public.user_1'],
          permissionPolicyRefs: ['permission-policy.public.recurring.safe'],
          providerPreferenceRefs: ['provider-preference.public.own_pylon'],
          repoRefs: ['repo.public.openagents'],
          retentionPolicyRefs: ['retention-policy.public.schedule_receipts'],
          scheduleRef: 'schedule.public.work_1.overnight',
          status: 'active',
          teamRefs: ['team.public.autopilot'],
          timezoneRefs: ['timezone.public.America/Chicago'],
          triggerKind: 'one_shot',
          workOrderTemplateRefs: ['work-template.public.overnight'],
          workspaceRefs: ['workspace.public.repo'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      active: 1,
      failed: 0,
      fired: 0,
      paused: 0,
      schedules: 1,
      skipped: 0,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      budgetMutationAuthority: false,
      continuationApprovalAuthority: false,
      credentialMutationAuthority: false,
      deploymentAuthority: false,
      notificationSendAuthority: false,
      providerMutationAuthority: false,
      publicClaimAuthority: false,
      scheduleCreateAuthority: false,
      scheduleDeleteAuthority: false,
      schedulePauseAuthority: false,
      scheduleResumeAuthority: false,
      scheduleUpdateAuthority: false,
      schedulerEnqueueAuthority: false,
      schedulerFireAuthority: false,
      settlementAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing scheduling state as empty', () => {
    const view = projectForgeSchedulingCron({
      generatedAt: '2026-06-18T02:20:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.schedules).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale schedule evidence', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          freshness: 'stale',
          scheduleRef: 'schedule.public.stale',
          status: 'paused',
          triggerKind: 'maintenance',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:stale-schedule-evidence:schedule.public.stale',
    )
  })

  test('blocks active schedules without budget permission and workspace policy refs', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          freshness: 'fresh',
          nextRunRefs: ['next-run.public.active'],
          scheduleRef: 'schedule.public.missing_policy',
          status: 'active',
          triggerKind: 'one_shot',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:active-schedule-policy-missing:schedule.public.missing_policy',
    )
  })

  test('blocks active schedules without next-run refs', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          budgetPolicyRefs: ['budget-policy.public.safe'],
          freshness: 'fresh',
          permissionPolicyRefs: ['permission-policy.public.safe'],
          scheduleRef: 'schedule.public.missing_next',
          status: 'active',
          triggerKind: 'one_shot',
          workspaceRefs: ['workspace.public.safe'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:active-schedule-next-run-missing:schedule.public.missing_next',
    )
  })

  test('blocks continuation schedules without continuation policy refs', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          freshness: 'fresh',
          scheduleRef: 'schedule.public.continuation',
          status: 'paused',
          triggerKind: 'continuation',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:continuation-policy-missing:schedule.public.continuation',
    )
  })

  test('blocks fired schedules without fire and run receipts', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          fireReceiptRefs: ['schedule-fire-receipt.public.work_1'],
          freshness: 'fresh',
          lastRunRefs: ['last-run.public.work_1'],
          scheduleRef: 'schedule.public.fired',
          status: 'fired',
          triggerKind: 'one_shot',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:fired-schedule-run-receipt-missing:schedule.public.fired',
    )
  })

  test('blocks skipped schedules without skip receipts', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          freshness: 'fresh',
          scheduleRef: 'schedule.public.skipped',
          status: 'skipped',
          triggerKind: 'one_shot',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:skipped-schedule-receipt-missing:schedule.public.skipped',
    )
  })

  test('blocks terminal schedules without failure or cancel receipts', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          freshness: 'fresh',
          scheduleRef: 'schedule.public.failed',
          status: 'failed',
          triggerKind: 'one_shot',
        },
        {
          freshness: 'fresh',
          scheduleRef: 'schedule.public.cancelled',
          status: 'cancelled',
          triggerKind: 'one_shot',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:failed-schedule-receipt-missing:schedule.public.failed',
    )
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:cancelled-schedule-receipt-missing:schedule.public.cancelled',
    )
  })

  test('blocks recurring schedules without no-double-fire evidence', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      schedules: [
        {
          freshness: 'fresh',
          scheduleRef: 'schedule.public.recurring',
          status: 'paused',
          triggerKind: 'recurring',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:recurring-no-double-fire-evidence-missing:schedule.public.recurring',
    )
  })

  test('blocks populated schedule entries without snapshot refs', () => {
    const view = projectForgeSchedulingCron({
      generatedAt: '2026-06-18T02:20:00.000Z',
      schedules: [
        {
          freshness: 'fresh',
          scheduleRef: 'schedule.public.no_snapshot',
          status: 'paused',
          triggerKind: 'one_shot',
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.no_snapshot:missing-scheduling-cron-snapshot-ref',
    )
  })

  test('omits unsafe private scheduling material before projection', () => {
    const view = projectForgeSchedulingCron({
      ...baseInput,
      blockerRefs: [
        'schedule-blocker.public.safe',
        'raw schedule /Users/christopher/schedule.json',
      ],
      schedules: [
        {
          adapterPreferenceRefs: ['adapter-preference.public.safe'],
          budgetPolicyRefs: ['budget-policy.public.safe'],
          freshness: 'fresh',
          nextRunRefs: ['next-run.public.safe'],
          notificationPolicyRefs: ['notification-policy.public.safe'],
          ownerRefs: ['owner.public.safe', 'customer data private'],
          permissionPolicyRefs: ['permission-policy.public.safe'],
          providerPreferenceRefs: ['provider-preference.public.safe'],
          repoRefs: ['repo.public.safe'],
          retentionPolicyRefs: ['retention-policy.public.safe'],
          scheduleRef: 'schedule.public.safe',
          status: 'active',
          teamRefs: ['team.public.safe'],
          timezoneRefs: ['timezone.public.America/Chicago'],
          triggerKind: 'one_shot',
          workOrderTemplateRefs: [
            'work-template.public.safe',
            'cron body /Users/christopher/private.txt',
          ],
          workspaceRefs: ['workspace.public.safe', 'provider payload sk-private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.schedules[0]?.ownerRefs).toEqual(['owner.public.safe'])
    expect(view.schedules[0]?.workspaceRefs).toEqual(['workspace.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-scheduling-cron-blocker:work.public.work_1:unsafe-scheduling-cron-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw schedule')
    expect(payload).not.toContain('cron body')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('customer data')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T02:20:00.000Z',
      schedulingCron: {
        generatedAt: '2026-06-18T02:21:00.000Z',
        schedules: [
          {
            budgetPolicyRefs: ['budget-policy.public.work_2'],
            freshness: 'fresh',
            nextRunRefs: ['next-run.public.work_2'],
            permissionPolicyRefs: ['permission-policy.public.work_2'],
            scheduleRef: 'schedule.public.work_2',
            status: 'active',
            triggerKind: 'one_shot',
            workspaceRefs: ['workspace.public.work_2'],
          },
        ],
        snapshotRef: 'scheduling-cron-snapshot.public.work_2',
        versionRef: 'scheduling-cron-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeSchedulingCronInput(work)).toEqual({
      generatedAt: '2026-06-18T02:21:00.000Z',
      schedules: [
        {
          budgetPolicyRefs: ['budget-policy.public.work_2'],
          freshness: 'fresh',
          nextRunRefs: ['next-run.public.work_2'],
          permissionPolicyRefs: ['permission-policy.public.work_2'],
          scheduleRef: 'schedule.public.work_2',
          status: 'active',
          triggerKind: 'one_shot',
          workspaceRefs: ['workspace.public.work_2'],
        },
      ],
      snapshotRef: 'scheduling-cron-snapshot.public.work_2',
      versionRef: 'scheduling-cron-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
