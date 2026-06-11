import { describe, expect, test } from 'vitest'

import {
  MemoryPackARuntimeEventRepository,
  TaskSupervisor,
  appendPackARuntimeEvent,
  appendScheduleEvent,
  projectPackASchedule,
  projectPackATask,
} from './autopilot-pack-a-runtime-supervision'

const at = '2026-06-11T22:00:00.000Z'

describe('Autopilot Pack A runtime supervision (Chronos)', () => {
  test('replays task lifecycle refs, terminal state, and projection staleness', async () => {
    const repository = new MemoryPackARuntimeEventRepository()
    const supervisor = new TaskSupervisor(repository)

    await supervisor.createTask({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      runRef: 'run.public.pack_a.scheduled_shc.1',
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      nowIso: at,
      refs: ['work_order.public.pack_a.overnight.1'],
    })
    await supervisor.startTask({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      nowIso: '2026-06-11T22:01:00.000Z',
    })
    await supervisor.appendOutput({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      outputRef: 'output.public.pack_a.scheduled_shc.cursor_1',
      cursor: 1,
      truncated: false,
      redacted: true,
      nowIso: '2026-06-11T22:02:00.000Z',
    })
    await supervisor.recordArtifact({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      artifactRef: 'artifact.public.pack_a.scheduled_shc.closeout',
      nowIso: '2026-06-11T22:03:00.000Z',
    })
    await supervisor.recordUsage({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      usageRef: 'usage.public.pack_a.scheduled_shc.tokens',
      nowIso: '2026-06-11T22:04:00.000Z',
    })
    await supervisor.completeTask({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      nowIso: '2026-06-11T22:05:00.000Z',
    })
    await supervisor.enqueueNotification({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      notificationRef: 'notification.public.pack_a.scheduled_shc.complete',
      nowIso: '2026-06-11T22:06:00.000Z',
    })
    await supervisor.deliverNotification({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      notificationRef: 'notification.public.pack_a.scheduled_shc.complete',
      nowIso: '2026-06-11T22:07:00.000Z',
    })

    const projection = projectPackATask(
      await repository.eventsForSubject(
        'task',
        'task.public.pack_a.scheduled_shc.1',
      ),
      '2026-06-11T22:08:00.000Z',
    )

    expect(projection).toMatchObject({
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      state: 'completed',
      runRef: 'run.public.pack_a.scheduled_shc.1',
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      outputRefs: ['output.public.pack_a.scheduled_shc.cursor_1'],
      artifactRefs: ['artifact.public.pack_a.scheduled_shc.closeout'],
      usageRefs: ['usage.public.pack_a.scheduled_shc.tokens'],
      notificationRefs: ['notification.public.pack_a.scheduled_shc.complete'],
      terminalState: 'completed',
      staleness: {
        composition: 'rebuilt_on_transition',
        maxStalenessSeconds: 0,
      },
      authority: {
        acceptedWorkAuthority: false,
        payoutAuthority: false,
        publicClaimAuthority: false,
      },
    })
    expect(projection.visibilitySplit).toEqual({
      storedEventVisibilities: ['public'],
      projectedVisibility: 'public',
    })
    expect(projection.eventCount).toBe(8)
  })

  test('rejects sequence gaps, raw public material, and duplicate delivered notifications', async () => {
    const repository = new MemoryPackARuntimeEventRepository()
    const supervisor = new TaskSupervisor(repository)

    await supervisor.createTask({
      taskRef: 'task.public.pack_a.rejects.1',
      nowIso: at,
    })

    await expect(appendPackARuntimeEvent(repository, {
      schema: 'openagents.autopilot.pack_a.runtime_event.v1',
      eventId: 'event.public.pack_a.rejects.gap',
      kind: 'task.started',
      subject: {
        kind: 'task',
        ref: 'task.public.pack_a.rejects.1',
        sequence: 3,
      },
      generatedAt: at,
      visibility: 'public',
      redactionClass: 'public_ref',
      refs: [],
      blockerRefs: [],
      task: {
        taskRef: 'task.public.pack_a.rejects.1',
      },
    })).rejects.toThrow('must append at 2')

    await expect(appendPackARuntimeEvent(repository, {
      schema: 'openagents.autopilot.pack_a.runtime_event.v1',
      eventId: 'event.public.pack_a.rejects.raw',
      kind: 'task.progress_recorded',
      subject: {
        kind: 'task',
        ref: 'task.public.pack_a.rejects.1',
        sequence: 2,
      },
      generatedAt: at,
      visibility: 'public',
      redactionClass: 'public_ref',
      refs: [],
      blockerRefs: [],
      summary: 'raw_prompt: /Users/example/private-source',
      task: {
        taskRef: 'task.public.pack_a.rejects.1',
      },
    })).rejects.toThrow('raw/private material')

    await supervisor.enqueueNotification({
      taskRef: 'task.public.pack_a.rejects.1',
      notificationRef: 'notification.public.pack_a.rejects.complete',
      nowIso: '2026-06-11T22:01:00.000Z',
    })
    await supervisor.deliverNotification({
      taskRef: 'task.public.pack_a.rejects.1',
      notificationRef: 'notification.public.pack_a.rejects.complete',
      nowIso: '2026-06-11T22:02:00.000Z',
    })

    await expect(supervisor.deliverNotification({
      taskRef: 'task.public.pack_a.rejects.1',
      notificationRef: 'notification.public.pack_a.rejects.complete',
      nowIso: '2026-06-11T22:03:00.000Z',
    })).rejects.toThrow('already delivered')
  })

  test('replay rejects illegal task transitions after terminal state', async () => {
    const repository = new MemoryPackARuntimeEventRepository()
    const supervisor = new TaskSupervisor(repository)

    await supervisor.createTask({
      taskRef: 'task.public.pack_a.illegal.1',
      nowIso: at,
    })
    await supervisor.startTask({
      taskRef: 'task.public.pack_a.illegal.1',
      nowIso: '2026-06-11T22:01:00.000Z',
    })
    await supervisor.completeTask({
      taskRef: 'task.public.pack_a.illegal.1',
      nowIso: '2026-06-11T22:02:00.000Z',
    })
    await supervisor.startTask({
      taskRef: 'task.public.pack_a.illegal.1',
      nowIso: '2026-06-11T22:03:00.000Z',
    })

    expect(() => projectPackATask(
      repository.eventsBySubject.get('task:task.public.pack_a.illegal.1') ?? [],
      '2026-06-11T22:04:00.000Z',
    )).toThrow('Illegal Pack A task transition: completed -> running')
  })

  test('records schedule fired, skipped, and continuation receipts without double fire', async () => {
    const repository = new MemoryPackARuntimeEventRepository()

    await appendScheduleEvent(repository, {
      kind: 'schedule.created',
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      ownerRef: 'owner.public.pack_a.operator',
      teamRef: 'team.public.openagents',
      nextRunAt: '2026-06-12T03:00:00.000Z',
      nowIso: at,
    })
    await appendScheduleEvent(repository, {
      kind: 'schedule.fired',
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      occurrenceRef: 'occurrence.public.pack_a.overnight.2026_06_12',
      taskRef: 'task.public.pack_a.scheduled_shc.1',
      runRef: 'run.public.pack_a.scheduled_shc.1',
      lastRunAt: '2026-06-12T03:00:00.000Z',
      nowIso: '2026-06-12T03:00:00.000Z',
    })

    await expect(appendScheduleEvent(repository, {
      kind: 'schedule.fired',
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      occurrenceRef: 'occurrence.public.pack_a.overnight.2026_06_12',
      taskRef: 'task.public.pack_a.duplicate.1',
      runRef: 'run.public.pack_a.duplicate.1',
      nowIso: '2026-06-12T03:00:01.000Z',
    })).rejects.toThrow('already fired')

    await appendScheduleEvent(repository, {
      kind: 'schedule.skipped',
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      occurrenceRef: 'occurrence.public.pack_a.overnight.2026_06_13',
      blockerRefs: ['blocker.public.pack_a.schedule.budget_ceiling'],
      nowIso: '2026-06-13T03:00:00.000Z',
    })
    await appendScheduleEvent(repository, {
      kind: 'schedule.continuation_queued',
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      occurrenceRef: 'occurrence.public.pack_a.overnight.continuation_1',
      taskRef: 'task.public.pack_a.continuation.1',
      nowIso: '2026-06-13T03:05:00.000Z',
    })

    const projection = projectPackASchedule(
      await repository.eventsForSubject(
        'schedule',
        'schedule.public.pack_a.overnight.1',
      ),
      '2026-06-13T03:06:00.000Z',
    )

    expect(projection).toMatchObject({
      scheduleRef: 'schedule.public.pack_a.overnight.1',
      state: 'active',
      ownerRef: 'owner.public.pack_a.operator',
      teamRef: 'team.public.openagents',
      nextRunAt: '2026-06-12T03:00:00.000Z',
      lastRunAt: '2026-06-12T03:00:00.000Z',
      occurrenceRefs: [
        'occurrence.public.pack_a.overnight.2026_06_12',
        'occurrence.public.pack_a.overnight.2026_06_13',
        'occurrence.public.pack_a.overnight.continuation_1',
      ],
      firedTaskRefs: ['task.public.pack_a.scheduled_shc.1'],
      continuationTaskRefs: ['task.public.pack_a.continuation.1'],
      blockerRefs: ['blocker.public.pack_a.schedule.budget_ceiling'],
      receiptRefs: [
        'receipt.public.pack_a.occurrence.public.pack_a.overnight.2026_06_12',
        'receipt.public.pack_a.occurrence.public.pack_a.overnight.2026_06_13',
        'receipt.public.pack_a.occurrence.public.pack_a.overnight.continuation_1',
      ],
      authority: {
        createsWorkAuthority: false,
        payoutAuthority: false,
        publicClaimAuthority: false,
      },
    })
  })

  test('replays decision-blocked task rows from the same event substrate', async () => {
    const repository = new MemoryPackARuntimeEventRepository()
    const supervisor = new TaskSupervisor(repository)

    await supervisor.createTask({
      taskRef: 'task.public.pack_a.decision_blocked.1',
      runRef: 'run.public.pack_a.decision_blocked.1',
      nowIso: at,
    })
    await supervisor.startTask({
      taskRef: 'task.public.pack_a.decision_blocked.1',
      nowIso: '2026-06-11T22:01:00.000Z',
    })
    await supervisor.waitForApproval({
      taskRef: 'task.public.pack_a.decision_blocked.1',
      blockerRefs: ['blocker.public.pack_a.approval_required'],
      nowIso: '2026-06-11T22:02:00.000Z',
    })

    const events = await repository.eventsForSubject(
      'task',
      'task.public.pack_a.decision_blocked.1',
    )
    const webProjection = projectPackATask(events, '2026-06-11T22:03:00.000Z')
    const apiProjection = projectPackATask(events, '2026-06-11T22:03:00.000Z')

    expect(webProjection).toEqual(apiProjection)
    expect(webProjection).toMatchObject({
      state: 'waiting',
      blockerRefs: ['blocker.public.pack_a.approval_required'],
    })
  })
})
