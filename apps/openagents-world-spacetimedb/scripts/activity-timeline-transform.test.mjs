import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import { describe, expect, it } from 'bun:test'

import {
  activityWorldReducerCounts,
  assertActivityWorldEventsMirrorTimeline,
  assertActivityWorldPlanPublicSafe,
  buildActivityTimelineWorldPlan,
} from './activity-timeline-transform.mjs'
import {
  assertNoDuplicateWorldEvents,
  assertWorldEventsAreSourced,
} from './tassadar-summary-transform.mjs'

const envelope = {
  schemaVersion: 'openagents.public_activity_timeline.v1',
  generatedAt: '2026-06-18T18:30:00.000Z',
  nextCursor: '2026-06-18T18:00:08.000Z:projection_gap:event.public.gap.1',
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 30,
    rebuildsOn: ['public_activity_timeline_read'],
  },
  range: {
    filterKinds: [],
    filterSources: [],
    from: null,
    limit: 3,
    since: null,
    to: null,
  },
  sourceLag: [
    {
      sourceKind: 'pylon_presence',
      status: 'current',
      latestSourceEventAt: '2026-06-18T18:00:02.000Z',
      observedAt: '2026-06-18T18:30:00.000Z',
      lagSeconds: 2,
      maxStalenessSeconds: 300,
      sourceRefs: ['route:/api/public/pylon-stats'],
      blockerRefs: [],
      caveatRefs: [],
    },
    {
      sourceKind: 'training_window',
      status: 'stale',
      latestSourceEventAt: '2026-06-18T18:00:06.000Z',
      observedAt: '2026-06-18T18:30:00.000Z',
      lagSeconds: 1440,
      maxStalenessSeconds: 300,
      sourceRefs: ['route:/api/public/tassadar-run-summary'],
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.activity_timeline.source_lag_exceeds_contract',
      ],
    },
  ],
  events: [
    {
      cursor: '2026-06-18T18:00:02.000Z:pylon_presence:event.public.pylon.1',
      eventRef: 'event.public.pylon.1',
      kind: 'pylon_heartbeat',
      sourceKind: 'pylon_presence',
      actorRef: 'pylon.public.worker1',
      targetRef: null,
      runRef: null,
      windowRef: null,
      refs: ['pylon.public.worker1'],
      sourceRefs: ['pylon.public.worker1', 'route:/api/public/pylon-stats'],
      blockerRefs: [],
      caveatRefs: [],
      ts: '2026-06-18T18:00:02.000Z',
      state: 'online',
      text: 'Pylon heartbeat observed in the public timeline.',
    },
    {
      cursor:
        '2026-06-18T18:00:06.000Z:training_window:event.public.work_claimed.1',
      eventRef: 'event.public.work_claimed.1',
      kind: 'work_claimed',
      sourceKind: 'training_window',
      actorRef: 'pylon.public.worker1',
      targetRef: 'training.window.public.timeline.w1',
      runRef: 'run.tassadar.executor.20260615',
      windowRef: 'training.window.public.timeline.w1',
      refs: [
        'pylon.public.worker1',
        'run.tassadar.executor.20260615',
        'training.window.public.timeline.w1',
      ],
      sourceRefs: ['training.lease.public.timeline.1'],
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.activity_timeline.claimed_work_is_not_accepted_or_paid',
      ],
      ts: '2026-06-18T18:00:06.000Z',
      state: 'active',
      text: 'Training work lease claimed by a public Pylon ref.',
    },
    {
      cursor: '2026-06-18T18:00:08.000Z:projection_gap:event.public.gap.1',
      eventRef: 'event.public.gap.1',
      kind: 'projection_gap',
      sourceKind: 'projection_gap',
      actorRef: null,
      targetRef: null,
      runRef: null,
      windowRef: null,
      refs: ['blocker.public.activity_timeline.training_store_missing'],
      sourceRefs: [],
      blockerRefs: ['blocker.public.activity_timeline.training_store_missing'],
      caveatRefs: [],
      ts: '2026-06-18T18:00:08.000Z',
      state: 'unavailable',
      text: 'Training source unavailable.',
    },
  ],
}

describe('public activity SpacetimeDB projection transform', () => {
  it('maps timeline events into sourced world_event reducer calls', () => {
    const plan = buildActivityTimelineWorldPlan(envelope, {
      sourceUrl: 'https://openagents.com/api/public/activity-timeline?limit=3',
    })
    const counts = activityWorldReducerCounts(plan)
    const worldEvents = plan.calls.filter(call => call.reducer === 'append_world_event')

    expect(plan.bridgeRef).toBe('bridge.public-activity-timeline')
    expect(plan.sourceNextCursor).toBe(envelope.nextCursor)
    expect(counts.append_world_event).toBe(envelope.events.length)
    expect(counts.record_projection_cursor).toBe(1)
    expect(worldEvents[0]?.args[1]).toBe('run.public_activity_timeline')
    expect(worldEvents[1]?.args[1]).toBe('run.tassadar.executor.20260615')
    expect(worldEvents[2]?.args[4]).toBe(
      'blocker.public.activity_timeline.training_store_missing',
    )
    assertNoDuplicateWorldEvents(plan)
    assertWorldEventsAreSourced(plan)
    assertActivityWorldEventsMirrorTimeline(plan, envelope)
    assertActivityWorldPlanPublicSafe(plan)
  })

  it('preserves source refs, caveats, generatedAt, and expiry in event summaries', () => {
    const plan = buildActivityTimelineWorldPlan(envelope)
    const workEvent = plan.calls
      .filter(call => call.reducer === 'append_world_event')
      .find(call => call.args[2] === 'work_claimed')
    const summary = JSON.parse(workEvent.args[6])

    expect(summary).toMatchObject({
      authority: 'worker_d1_public_projection_only',
      eventRef: 'event.public.work_claimed.1',
      generatedAt: envelope.generatedAt,
      kind: 'work_claimed',
      sourceLagStatus: 'stale',
    })
    expect(summary.sourceRefs).toEqual(['training.lease.public.timeline.1'])
    expect(summary.caveatRefs).toEqual([
      'caveat.public.activity_timeline.claimed_work_is_not_accepted_or_paid',
    ])
    expect(summary.expiresAt).toBe('2026-06-18T18:30:30.000Z')
  })

  it('is deterministic for replaying the same public timeline envelope', () => {
    const first = buildActivityTimelineWorldPlan(envelope)
    const second = buildActivityTimelineWorldPlan(envelope)

    expect(second).toEqual(first)
  })

  it('rejects private material before a bridge plan can be applied', () => {
    const unsafe = {
      ...envelope,
      events: [
        {
          ...envelope.events[0],
          text: 'raw_prompt customer@example.com sk-test-private',
        },
      ],
    }

    expect(() => buildActivityTimelineWorldPlan(unsafe)).toThrow(
      /private material/,
    )
  })

  it('supports source-file dry-run output for bridge checks', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'activity-world-'))
    const fixture = join(dir, 'timeline.json')
    await writeFile(fixture, `${JSON.stringify(envelope, null, 2)}\n`)

    const result = spawnSync(process.execPath, [
      'apps/openagents-world-spacetimedb/scripts/project-activity-timeline.mjs',
      '--source-file',
      fixture,
    ], {
      cwd: new URL('../../..', import.meta.url),
      encoding: 'utf8',
    })

    expect(result.status).toBe(0)
    const body = JSON.parse(result.stdout)
    expect(body.applyVm).toBe(false)
    expect(body.reducerCounts.append_world_event).toBe(envelope.events.length)
    expect(body.reducerCounts.record_projection_cursor).toBe(1)
    expect(body.sourceNextCursor).toBe(envelope.nextCursor)
  })
})
