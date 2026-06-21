import { describe, expect, it } from 'bun:test'

import {
  DEFAULT_FORUM_ACTIVITY_RUN_REF,
  buildForumActivityWorldPlan,
  forumActivityEnvelopeHasUnsafeMaterial,
  forumWorldReducerCounts,
} from './forum-activity-transform.mjs'
import {
  assertNoDuplicateWorldEvents,
  assertWorldEventsAreSourced,
} from './tassadar-summary-transform.mjs'

const envelope = {
  generatedAt: '2026-06-21T18:30:00.000Z',
  sourceUrl: '/api/public/forum-activity',
  staleness: { composition: 'live_at_read', maxStalenessSeconds: 0 },
  activity: [
    {
      agentRef: 'agent:autopilot_abc',
      pylonRef: null,
      eventKind: 'forum_post',
      eventRef: 'topic_1',
      sourceRef: 'topic_1',
      topicRef: 'topic_1',
      sourceGeneratedAt: '2026-06-21T18:00:00.000Z',
      summary: 'Posted: Introducing my node',
    },
    {
      agentRef: 'agent:other_def',
      pylonRef: null,
      eventKind: 'forum_reply',
      eventRef: 'post_2',
      sourceRef: 'post_2',
      topicRef: 'topic_1',
      sourceGeneratedAt: '2026-06-21T18:10:00.000Z',
      summary: 'Replied: welcome',
    },
  ],
}

describe('buildForumActivityWorldPlan (BF-2)', () => {
  it('maps each forum activity row to an append_world_event with the right fields', () => {
    const plan = buildForumActivityWorldPlan(envelope)
    const worldEvents = plan.calls.filter(c => c.reducer === 'append_world_event')
    expect(worldEvents.length).toBe(2)
    const [post, reply] = worldEvents
    // args: [event_ref, run_ref, event_kind, entity_ref, source_ref, src_at, summary]
    expect(post.args[2]).toBe('forum_post')
    expect(post.args[3]).toBe('agent:autopilot_abc') // entity_ref = agentRef
    expect(post.args[4]).toBe('topic_1') // source_ref (dereferenceable)
    expect(post.args[1]).toBe(DEFAULT_FORUM_ACTIVITY_RUN_REF)
    expect(reply.args[2]).toBe('forum_reply')
    // event_ref is the idempotent world hash, not the raw forum id.
    expect(post.args[0]).toContain('world_event.forum_activity.')
  })

  it('appends a projection cursor and counts reducers', () => {
    const plan = buildForumActivityWorldPlan(envelope)
    const counts = forumWorldReducerCounts(plan)
    expect(counts.append_world_event).toBe(2)
    expect(counts.record_projection_cursor).toBe(1)
  })

  it('is idempotent: the same envelope yields identical, deterministic event_refs', () => {
    const a = buildForumActivityWorldPlan(envelope)
    const b = buildForumActivityWorldPlan(envelope)
    const refsOf = plan =>
      plan.calls
        .filter(c => c.reducer === 'append_world_event')
        .map(c => c.args[0])
    expect(refsOf(a)).toEqual(refsOf(b))
    assertNoDuplicateWorldEvents(a)
    assertWorldEventsAreSourced(a)
  })

  it('skips unknown-kind and malformed rows', () => {
    const plan = buildForumActivityWorldPlan({
      ...envelope,
      activity: [
        { eventKind: 'totally_unknown', eventRef: 'x', sourceRef: 'x', agentRef: 'a' },
        { eventKind: 'forum_post', eventRef: '', sourceRef: 'y', agentRef: 'a' },
        ...envelope.activity,
      ],
    })
    expect(
      plan.calls.filter(c => c.reducer === 'append_world_event').length,
    ).toBe(2)
  })

  it('rejects envelopes carrying private material (token / spark address)', () => {
    const unsafe = {
      ...envelope,
      activity: [
        {
          ...envelope.activity[0],
          summary: 'leak oa_agent_secrettoken',
        },
      ],
    }
    expect(forumActivityEnvelopeHasUnsafeMaterial(unsafe)).toBe(true)
    expect(() => buildForumActivityWorldPlan(unsafe)).toThrow(/private material/)
  })

  it('rejects a malformed envelope (no activity array)', () => {
    expect(() => buildForumActivityWorldPlan({ generatedAt: 'x' })).toThrow(
      /activity array/,
    )
  })
})
