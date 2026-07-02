import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeArtanisForumUpdateWriter } from './artanis-operator-forum-update'

const nowIso = '2026-07-02T10:30:00.000Z'
const registeredArtanisUserId = 'user_artanis_registered'
const registeredArtanisActorRef = `agent:${registeredArtanisUserId}`
const publicProjectionJson = JSON.stringify({
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: [],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: ['artifact.forum.artanis.status'],
  safeReceiptRefs: [],
  trustTier: 'reviewed',
})
const legacyArtanisActorJson = JSON.stringify({
  actorId: 'agent_artanis',
  actorRef: 'agent:agent_artanis',
  displayName: 'Artanis',
  groupRefs: ['agents', 'openagents'],
  isAgent: true,
  slug: 'artanis',
})

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

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as Array<T>
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

const makeIdFactory = (ids: ReadonlyArray<string>) => {
  let index = 0
  return () => {
    const id = ids[index]
    index += 1
    if (id === undefined) {
      throw new Error('No test id left.')
    }
    return id
  }
}

const makeDb = (
  input: Readonly<{ registeredArtanis?: boolean }> = {},
): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(
    `CREATE TABLE users (
       id TEXT PRIMARY KEY,
       kind TEXT NOT NULL,
       display_name TEXT NOT NULL,
       primary_email TEXT,
       avatar_url TEXT,
       status TEXT NOT NULL,
       deleted_at TEXT,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE TABLE agent_profiles (
       user_id TEXT PRIMARY KEY,
       slug TEXT,
       metadata_json TEXT
     );
     CREATE TABLE agent_credentials (
       id TEXT PRIMARY KEY,
       user_id TEXT NOT NULL,
       openauth_user_id TEXT,
       token_prefix TEXT NOT NULL,
       status TEXT NOT NULL,
       revoked_at TEXT,
       expires_at TEXT,
       created_at TEXT NOT NULL
     );
     CREATE TABLE forum_forums (
       id TEXT PRIMARY KEY,
       board_id TEXT NOT NULL,
       category_id TEXT NOT NULL,
       description_ref TEXT,
       discoverability TEXT NOT NULL,
       visibility TEXT NOT NULL,
       slug TEXT NOT NULL,
       title TEXT NOT NULL,
       locked INTEGER NOT NULL,
       topic_count INTEGER NOT NULL,
       post_count INTEGER NOT NULL,
       latest_topic_id TEXT,
       latest_post_id TEXT,
       public_projection_json TEXT NOT NULL,
       archived_at TEXT,
       updated_at TEXT NOT NULL
     );
     CREATE TABLE forum_topics (
       id TEXT PRIMARY KEY,
       idempotency_key TEXT NOT NULL,
       forum_id TEXT NOT NULL,
       actor_ref TEXT NOT NULL,
       actor_json TEXT NOT NULL,
       slug TEXT NOT NULL,
       title TEXT NOT NULL,
       first_post_id TEXT NOT NULL,
       latest_post_id TEXT NOT NULL,
       post_count INTEGER NOT NULL,
       pin_state TEXT NOT NULL,
       state TEXT NOT NULL,
       score_ref TEXT,
       public_projection_json TEXT NOT NULL,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL,
       archived_at TEXT
     );
     CREATE TABLE forum_posts (
       id TEXT PRIMARY KEY,
       idempotency_key TEXT NOT NULL,
       topic_id TEXT NOT NULL,
       forum_id TEXT NOT NULL,
       actor_ref TEXT NOT NULL,
       actor_json TEXT NOT NULL,
       content_ref TEXT NOT NULL,
       parent_post_id TEXT,
       quote_post_id TEXT,
       post_number INTEGER NOT NULL,
       state TEXT NOT NULL,
       revision_ref TEXT,
       public_projection_json TEXT NOT NULL,
       receipt_refs_json TEXT NOT NULL,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL,
       archived_at TEXT
     );
     CREATE TABLE forum_post_bodies (
       post_id TEXT PRIMARY KEY,
       content_kind TEXT NOT NULL,
       body_text TEXT NOT NULL,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL,
       archived_at TEXT
     );
     CREATE TABLE forum_tip_recipient_wallets (
       actor_ref TEXT,
       archived_at TEXT
     );`,
  )

  raw
    .prepare(
      `INSERT INTO forum_forums (
         id, board_id, category_id, description_ref, discoverability,
         visibility, slug, title, locked, topic_count, post_count,
         latest_topic_id, latest_post_id, public_projection_json, archived_at,
         updated_at
       )
       VALUES (
         'forum_artanis', 'board_artanis', 'category_artanis', NULL, 'listed',
         'public', 'artanis', 'Artanis', 0, 1, 1, 'topic_status',
         'post_status_1', ?, NULL, ?
       )`,
    )
    .run(publicProjectionJson, nowIso)
  raw
    .prepare(
      `INSERT INTO forum_topics (
         id, idempotency_key, forum_id, actor_ref, actor_json, slug, title,
         first_post_id, latest_post_id, post_count, pin_state, state, score_ref,
         public_projection_json, created_at, updated_at, archived_at
       )
       VALUES (
         'topic_status', 'seed:artanis:status:v1', 'forum_artanis',
         'agent:agent_artanis', ?, 'artanis-status', 'Artanis status',
         'post_status_1', 'post_status_1', 1, 'announcement', 'open', NULL,
         ?, '2026-06-06T20:00:00.000Z', '2026-06-06T20:00:00.000Z', NULL
       )`,
    )
    .run(legacyArtanisActorJson, publicProjectionJson)
  raw
    .prepare(
      `INSERT INTO forum_posts (
         id, idempotency_key, topic_id, forum_id, actor_ref, actor_json,
         content_ref, parent_post_id, quote_post_id, post_number, state,
         revision_ref, public_projection_json, receipt_refs_json, created_at,
         updated_at, archived_at
       )
       VALUES (
         'post_status_1', 'seed:artanis:status:first-post:v1', 'topic_status',
         'forum_artanis', 'agent:agent_artanis', ?,
         'content.forum.artanis.status.first', NULL, NULL, 1, 'visible', NULL,
         ?, '[]', '2026-06-06T20:00:00.000Z',
         '2026-06-06T20:00:00.000Z', NULL
       )`,
    )
    .run(legacyArtanisActorJson, publicProjectionJson)
  raw
    .prepare(
      `INSERT INTO forum_post_bodies (
         post_id, content_kind, body_text, created_at, updated_at, archived_at
       )
       VALUES (
         'post_status_1', 'plain_text', 'Canonical Artanis status thread.',
         '2026-06-06T20:00:00.000Z', '2026-06-06T20:00:00.000Z', NULL
       )`,
    )
    .run()

  if (input.registeredArtanis !== false) {
    raw
      .prepare(
        `INSERT INTO users (
           id, kind, display_name, primary_email, avatar_url, status,
           deleted_at, created_at, updated_at
         )
         VALUES (?, 'agent', 'Artanis', NULL, NULL, 'active', NULL, ?, ?)`,
      )
      .run(
        registeredArtanisUserId,
        '2026-06-26T17:00:00.000Z',
        '2026-06-26T18:00:00.000Z',
      )
    raw
      .prepare(
        `INSERT INTO agent_profiles (user_id, slug, metadata_json)
         VALUES (?, 'artanis', ?)`,
      )
      .run(
        registeredArtanisUserId,
        JSON.stringify({ purpose: 'forum_posting' }),
      )
    raw
      .prepare(
        `INSERT INTO agent_credentials (
           id, user_id, openauth_user_id, token_prefix, status, revoked_at,
           expires_at, created_at
         )
         VALUES (
           'agent_credential_artanis_reissued', ?, NULL,
           'oa_agent_artanis_re', 'active', NULL, NULL,
           '2026-06-26T18:00:00.000Z'
         )`,
      )
      .run(registeredArtanisUserId)
  }

  return new SqliteD1(raw) as unknown as D1Database
}

describe('Artanis Forum update writer', () => {
  test('creates topics as the registered Artanis Forum identity', async () => {
    const db = makeDb()
    const writer = makeArtanisForumUpdateWriter({
      db,
      makeId: makeIdFactory(['topic_registered_update', 'post_registered_first']),
      nowEpochMillis: () => Date.parse(nowIso),
      nowIso: () => nowIso,
    })

    const result = await Effect.runPromise(
      writer({
        action: 'create_topic',
        bodyText: 'Registered Artanis topic update.',
        forumRef: 'artanis',
        idempotencyKey: 'artanis-operator-update:registered-topic:v1',
        title: 'Registered Artanis topic',
        topicRef: undefined,
      }),
    )

    expect(result).toMatchObject({
      idempotent: false,
      postId: 'post_registered_first',
      topicId: 'topic_registered_update',
    })
    const topic = await db
      .prepare(
        `SELECT actor_ref, actor_json, title
           FROM forum_topics
          WHERE id = 'topic_registered_update'`,
      )
      .first<{ actor_json: string; actor_ref: string; title: string }>()
    expect(topic?.actor_ref).toBe(registeredArtanisActorRef)
    expect(topic?.title).toBe('Registered Artanis topic')
    expect(JSON.parse(topic?.actor_json ?? '{}')).toMatchObject({
      actorId: registeredArtanisUserId,
      actorRef: registeredArtanisActorRef,
      displayName: 'Artanis',
      isAgent: true,
      slug: 'artanis',
    })

    const retry = await Effect.runPromise(
      writer({
        action: 'create_topic',
        bodyText: 'Registered Artanis topic update.',
        forumRef: 'artanis',
        idempotencyKey: 'artanis-operator-update:registered-topic:v1',
        title: 'Registered Artanis topic',
        topicRef: undefined,
      }),
    )
    expect(retry).toMatchObject({
      idempotent: true,
      postId: 'post_registered_first',
      topicId: 'topic_registered_update',
    })
  })

  test('replies as the registered Artanis Forum identity', async () => {
    const db = makeDb()
    const writer = makeArtanisForumUpdateWriter({
      db,
      makeId: makeIdFactory(['post_registered_reply']),
      nowEpochMillis: () => Date.parse(nowIso),
      nowIso: () => nowIso,
    })

    const result = await Effect.runPromise(
      writer({
        action: 'reply',
        bodyText: 'Registered Artanis progress update.',
        forumRef: 'artanis',
        idempotencyKey: 'artanis-operator-update:registered-reply:v1',
        title: undefined,
        topicRef: 'topic_status',
      }),
    )

    expect(result).toMatchObject({
      idempotent: false,
      postId: 'post_registered_reply',
      topicId: 'topic_status',
    })
    const post = await db
      .prepare(
        `SELECT actor_ref, actor_json
           FROM forum_posts
          WHERE id = 'post_registered_reply'`,
      )
      .first<{ actor_json: string; actor_ref: string }>()
    expect(post?.actor_ref).toBe(registeredArtanisActorRef)
    expect(JSON.parse(post?.actor_json ?? '{}')).toMatchObject({
      actorId: registeredArtanisUserId,
      actorRef: registeredArtanisActorRef,
      displayName: 'Artanis',
      isAgent: true,
      slug: 'artanis',
    })
  })

  test('fails closed when the registered Artanis identity is missing', async () => {
    const db = makeDb({ registeredArtanis: false })
    const writer = makeArtanisForumUpdateWriter({
      db,
      makeId: makeIdFactory(['post_should_not_be_used']),
      nowEpochMillis: () => Date.parse(nowIso),
      nowIso: () => nowIso,
    })

    await expect(
      Effect.runPromise(
        writer({
          action: 'reply',
          bodyText: 'This should not publish.',
          forumRef: 'artanis',
          idempotencyKey: 'artanis-operator-update:missing-identity:v1',
          title: undefined,
          topicRef: 'topic_status',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'ArtanisForumUpdateWriterError',
      reason: expect.stringContaining('Registered Artanis Forum identity'),
    })

    const count = await db
      .prepare(`SELECT COUNT(*) AS count FROM forum_posts`)
      .first<{ count: number }>()
    expect(count?.count).toBe(1)
  })
})
