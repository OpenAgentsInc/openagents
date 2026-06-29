import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  buildPublicForumActivityEnvelope,
  handlePublicForumActivityApi,
  makeD1PublicForumActivityStore,
  PublicForumActivityEndpoint,
  type PublicForumActivityRecord,
  type PublicForumActivityStore,
} from './public-forum-activity-routes'

const sampleRecords: PublicForumActivityRecord[] = [
  {
    agentRef: 'agent:autopilot_abc',
    pylonRef: null,
    eventKind: 'forum_post',
    eventRef: 'topic_1',
    sourceRef: 'topic_1',
    topicRef: 'topic_1',
    sourceGeneratedAt: '2026-06-21T00:00:00.000Z',
    summary: 'Posted: Introducing my node',
  },
]

const fakeStore = (
  records: PublicForumActivityRecord[] = sampleRecords,
): PublicForumActivityStore => ({
  listRecentActivity: async () => records,
})

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

describe('handlePublicForumActivityApi (BF-1)', () => {
  it('returns a public-safe envelope of forum activity', async () => {
    const res = await run(
      handlePublicForumActivityApi(
        new Request('https://openagents.com/api/public/forum-activity'),
        { store: fakeStore(), nowIso: () => '2026-06-21T01:00:00.000Z' },
      ),
    )
    expect(res.status).toBe(200)
    const body = (await res.json()) as Record<string, unknown>
    expect(body.sourceUrl).toBe(PublicForumActivityEndpoint)
    expect(body.generatedAt).toBe('2026-06-21T01:00:00.000Z')
    expect(Array.isArray(body.activity)).toBe(true)
    expect((body.activity as unknown[]).length).toBe(1)
    const row = (body.activity as Record<string, unknown>[])[0]!
    expect(row.eventKind).toBe('forum_post')
    expect(row.sourceRef).toBe('topic_1')
    expect(row.agentRef).toBe('agent:autopilot_abc')
  })

  it('rejects a non-GET method with 405', async () => {
    const res = await run(
      handlePublicForumActivityApi(
        new Request('https://openagents.com/api/public/forum-activity', {
          method: 'POST',
        }),
        { store: fakeStore() },
      ),
    )
    expect(res.status).toBe(405)
  })

  it('rejects an out-of-range limit with 400', async () => {
    const res = await run(
      handlePublicForumActivityApi(
        new Request(
          'https://openagents.com/api/public/forum-activity?limit=9999',
        ),
        { store: fakeStore() },
      ),
    )
    expect(res.status).toBe(400)
  })

  it('returns an empty activity list when no store/db is available (fail-soft)', async () => {
    const envelope = await buildPublicForumActivityEnvelope({}, 50)
    expect(envelope.activity).toEqual([])
  })
})

// Minimal fake D1 that answers the two prepared queries (topics, replies) by
// inspecting the SQL text, so we can assert the store's public-safe mapping.
const fakeD1 = (
  topics: Array<Record<string, unknown>>,
  replies: Array<Record<string, unknown>>,
): D1Database =>
  ({
    prepare: (sql: string) => ({
      bind: () => ({
        all: async () => ({
          results: sql.includes("'topic' AS kind") ? topics : replies,
        }),
      }),
    }),
  }) as unknown as D1Database

describe('makeD1PublicForumActivityStore (BF-1 mapping)', () => {
  it('maps topics to forum_post and replies to forum_reply with public-safe summaries', async () => {
    const store = makeD1PublicForumActivityStore(
      fakeD1(
        [
          {
            kind: 'topic',
            event_ref: 'topic_1',
            topic_ref: 'topic_1',
            actor_ref: 'agent:a',
            title: 'Hello world',
            created_at: '2026-06-21T02:00:00.000Z',
          },
        ],
        [
          {
            kind: 'reply',
            event_ref: 'post_2',
            topic_ref: 'topic_1',
            actor_ref: 'agent:b',
            title: 'Hello world',
            created_at: '2026-06-21T03:00:00.000Z',
          },
        ],
      ),
    )
    const activity = await store.listRecentActivity(50)
    expect(activity.length).toBe(2)
    // Sorted by created_at desc → the reply (03:00) comes first.
    const reply = activity[0]!
    const topic = activity[1]!
    expect(reply.eventKind).toBe('forum_reply')
    expect(reply.eventRef).toBe('post_2')
    expect(reply.summary).toContain('Replied')
    expect(topic.eventKind).toBe('forum_post')
    expect(topic.summary).toContain('Posted')
    // Public-safe: pylonRef null (bridge resolves agent→pylon), no leakage.
    expect(topic.pylonRef).toBeNull()
  })
})
