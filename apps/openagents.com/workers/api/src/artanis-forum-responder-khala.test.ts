import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_REGISTERED_ACTOR_REF,
  ARTANIS_RESPONDER_KHALA_MODEL,
  runArtanisResponderScan,
} from './artanis-forum-responder'
import type {
  InferenceRequest,
  InferenceResult,
} from './inference/provider-adapter'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
    return { results }
  }

  async run(): Promise<{ success: true }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(
    `CREATE TABLE forum_forums (
       id TEXT PRIMARY KEY,
       slug TEXT NOT NULL
     );
     CREATE TABLE forum_topics (
       id TEXT PRIMARY KEY,
       forum_id TEXT NOT NULL,
       first_post_id TEXT NOT NULL,
       title TEXT NOT NULL,
       actor_ref TEXT NOT NULL,
       state TEXT NOT NULL,
       created_at TEXT NOT NULL
     );
     CREATE TABLE forum_post_bodies (
       post_id TEXT PRIMARY KEY,
       body_text TEXT NOT NULL
     );`,
  )
  raw.exec(migration('0161_artanis_responder.sql'))
  raw.exec(migration('0213_artanis_responder_asker_provenance.sql'))
  return new SqliteD1(raw) as unknown as D1Database
}

const seedTopic = async (
  db: D1Database,
  input: Readonly<{
    actorRef?: string
    bodyText?: string
    title?: string
    topicId?: string
  }> = {},
): Promise<void> => {
  const topicId = input.topicId ?? 'topic_pylon_help'
  const firstPostId = `${topicId}_first_post`

  await db
    .prepare(
      `INSERT INTO artanis_responder_state
       (id, scan_cursor_iso, responses_today, responses_day, updated_at)
       VALUES (1, '2026-06-24T00:00:00.000Z', 0, '2026-06-25', '2026-06-25T00:00:00.000Z')`,
    )
    .run()
  await db
    .prepare(
      `INSERT INTO forum_forums (id, slug) VALUES ('forum_agents', 'agents')`,
    )
    .run()
  await db
    .prepare(
      `INSERT INTO forum_topics
       (id, forum_id, first_post_id, title, actor_ref, state, created_at)
       VALUES (?, 'forum_agents', ?, ?, ?, 'open', ?)`,
    )
    .bind(
      topicId,
      firstPostId,
      input.title ?? 'Can my Pylon join training runs?',
      input.actorRef ?? 'agent:external_contributor',
      '2026-06-25T01:00:00.000Z',
    )
    .run()
  await db
    .prepare(`INSERT INTO forum_post_bodies (post_id, body_text) VALUES (?, ?)`)
    .bind(
      firstPostId,
      input.bodyText ??
        'I have a GPU Pylon online. How do I know whether it can join a training run?',
    )
    .run()
}

describe('Artanis forum responder Khala routing', () => {
  test('uses openagents/khala as the default reasoning path without requiring Gemini', async () => {
    const db = makeDb()
    await seedTopic(db)
    const requests: InferenceRequest[] = []

    const outcome = await runArtanisResponderScan(db, {
      artanisActorRefs: ['agent:artanis'],
      geminiApiKey: null,
      khalaClient: request => {
        requests.push(request)
        return Effect.succeed({
          content: JSON.stringify({
            candidates: [
              {
                questionClass: 'training_run',
                respond: true,
                topicId: 'topic_pylon_help',
              },
            ],
          }),
          finishReason: 'stop',
          servedModel: ARTANIS_RESPONDER_KHALA_MODEL,
          usage: {
            completionTokens: 12,
            promptTokens: 40,
            totalTokens: 52,
          },
        } satisfies InferenceResult)
      },
      nowIso: '2026-06-25T02:00:00.000Z',
    })

    expect(outcome.proposed).toBe(1)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.model).toBe(ARTANIS_RESPONDER_KHALA_MODEL)
    expect(requests[0]?.stream).toBe(false)

    const action = await db
      .prepare(
        `SELECT state, question_class, proposal_json
           FROM artanis_responder_actions
          WHERE topic_id = 'topic_pylon_help'`,
      )
      .first<{
        proposal_json: string
        question_class: string
        state: string
      }>()
    expect(action?.state).toBe('proposed')
    expect(action?.question_class).toBe('training_run')
    expect(JSON.parse(action?.proposal_json ?? '{}')).toMatchObject({
      servedVia: 'openagents_khala',
    })
  })

  test('filters the legacy seeded Artanis actor before mind classification', async () => {
    const db = makeDb()
    await seedTopic(db, {
      actorRef: 'agent:agent_artanis',
      bodyText: 'Artanis internal status should not ask Artanis for a reply.',
      title: 'Artanis internal status',
      topicId: 'topic_legacy_artanis_self',
    })
    const requests: InferenceRequest[] = []

    const outcome = await runArtanisResponderScan(db, {
      artanisActorRefs: [ARTANIS_REGISTERED_ACTOR_REF],
      geminiApiKey: null,
      khalaClient: request => {
        requests.push(request)
        return Effect.succeed({
          content: '{"candidates":[]}',
          finishReason: 'stop',
          servedModel: ARTANIS_RESPONDER_KHALA_MODEL,
          usage: {
            completionTokens: 0,
            promptTokens: 0,
            totalTokens: 0,
          },
        } satisfies InferenceResult)
      },
      nowIso: '2026-06-25T02:00:00.000Z',
    })

    expect(outcome).toMatchObject({
      proposed: 0,
      scanned: 0,
      skipped: 0,
    })
    expect(requests).toHaveLength(0)
  })
})
