import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_CANONICAL_TOPIC_IDS,
  ARTANIS_FORUM_DELIVERY_NO_EXTRA_AUTHORITY,
  ARTANIS_FORUM_ID,
  deliverArtanisForumPublicationIntent,
  deliverReadyArtanisForumPublications,
} from './artanis-forum-delivery'
import {
  ArtanisForumPublicationIntentRecord,
  exampleArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import { saveArtanisForumPublicationIntent } from './artanis-persistence'
import {
  ArtanisPersistenceTestStore,
  artanisPersistenceTestDb,
} from './test/artanis-persistence-fixture'

const nowIso = '2026-06-07T06:00:00.000Z'
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
const artanisActorJson = JSON.stringify({
  actorId: 'agent_artanis',
  actorRef: 'agent:agent_artanis',
  displayName: 'Artanis',
  groupRefs: ['agents', 'openagents'],
  isAgent: true,
  slug: 'artanis',
})
const registeredArtanisUserId = 'user_artanis_registered'
const registeredArtanisActorRef = `agent:${registeredArtanisUserId}`

type ForumRow = Readonly<{
  archived_at: string | null
  board_id: string
  category_id: string
  description_ref: string | null
  discoverability: 'listed' | 'unlisted' | 'hidden'
  id: string
  latest_post_id: string | null
  latest_topic_id: string | null
  locked: number
  post_count: number
  public_projection_json: string
  slug: string
  title: string
  topic_count: number
  visibility: 'public' | 'customer' | 'team' | 'private'
}>

type TopicRow = Readonly<{
  actor_json: string
  actor_ref: string
  archived_at: string | null
  created_at: string
  first_post_id: string
  forum_id: string
  id: string
  idempotency_key: string
  latest_post_id: string
  pin_state: 'normal' | 'sticky' | 'announcement'
  post_count: number
  public_projection_json: string
  score_ref: string | null
  slug: string
  state: 'open' | 'locked' | 'archived' | 'hidden'
  title: string
  updated_at: string
}>

type PostRow = Readonly<{
  actor_json: string
  actor_ref: string
  archived_at: string | null
  body_text: string | null
  content_ref: string
  created_at: string
  forum_id: string
  id: string
  idempotency_key: string
  parent_post_id: string | null
  post_number: number
  public_projection_json: string
  quote_post_id: string | null
  receipt_refs_json: string
  revision_ref: string | null
  state: 'visible' | 'edited' | 'tombstoned' | 'held_for_review' | 'hidden'
  topic_id: string
  updated_at: string
}>

type AgentForumIdentityRow = Readonly<{
  user_id: string
  display_name: string
  primary_email: string | null
  avatar_url: string | null
  status: 'active'
  user_created_at: string
  user_updated_at: string
  slug: string | null
  metadata_json: string | null
  credential_id: string
  openauth_user_id: string | null
  token_prefix: string
}>

class DeliveryStore {
  readonly artanis = new ArtanisPersistenceTestStore()
  registeredArtanis: AgentForumIdentityRow | null = {
    user_id: registeredArtanisUserId,
    display_name: 'Artanis',
    primary_email: null,
    avatar_url: null,
    status: 'active',
    user_created_at: '2026-06-26T17:00:00.000Z',
    user_updated_at: '2026-06-26T18:00:00.000Z',
    slug: 'artanis',
    metadata_json: JSON.stringify({ purpose: 'forum_posting' }),
    credential_id: 'agent_credential_artanis_reissued',
    openauth_user_id: null,
    token_prefix: 'oa_agent_artanis_re',
  }
  forums: Array<ForumRow> = [
    {
      archived_at: null,
      board_id: '88888888-1000-4000-8000-888888888888',
      category_id: '88888888-2000-4000-8000-888888888888',
      description_ref: 'content.forum.artanis.description',
      discoverability: 'listed',
      id: ARTANIS_FORUM_ID,
      latest_post_id: '88888888-5001-4001-8001-888888888888',
      latest_topic_id: ARTANIS_CANONICAL_TOPIC_IDS.status,
      locked: 0,
      post_count: 1,
      public_projection_json: publicProjectionJson,
      slug: 'artanis',
      title: 'Artanis',
      topic_count: 1,
      visibility: 'public',
    },
  ]
  topics: Array<TopicRow> = [
    {
      actor_json: artanisActorJson,
      actor_ref: 'agent:agent_artanis',
      archived_at: null,
      created_at: '2026-06-06T20:00:00.000Z',
      first_post_id: '88888888-5001-4001-8001-888888888888',
      forum_id: ARTANIS_FORUM_ID,
      id: ARTANIS_CANONICAL_TOPIC_IDS.status,
      idempotency_key: 'seed:artanis:status:v1',
      latest_post_id: '88888888-5001-4001-8001-888888888888',
      pin_state: 'announcement',
      post_count: 1,
      public_projection_json: publicProjectionJson,
      score_ref: 'score.forum.artanis.status',
      slug: 'artanis-status',
      state: 'open',
      title: 'Artanis status',
      updated_at: '2026-06-06T20:00:00.000Z',
    },
  ]
  posts: Array<PostRow> = [
    {
      actor_json: artanisActorJson,
      actor_ref: 'agent:agent_artanis',
      archived_at: null,
      body_text: 'Canonical Artanis status thread.',
      content_ref: 'content.forum.artanis.status.first',
      created_at: '2026-06-06T20:00:00.000Z',
      forum_id: ARTANIS_FORUM_ID,
      id: '88888888-5001-4001-8001-888888888888',
      idempotency_key: 'seed:artanis:status:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: publicProjectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: ARTANIS_CANONICAL_TOPIC_IDS.status,
      updated_at: '2026-06-06T20:00:00.000Z',
    },
  ]
}

class DeliveryStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: DeliveryStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('COALESCE(MAX(post_number)')) {
      const topicId = String(this.values[0])
      const postNumbers = this.store.posts
        .filter(post => post.topic_id === topicId && post.archived_at === null)
        .map(post => post.post_number)
      const postNumber = postNumbers.length === 0 ? 0 : Math.max(...postNumbers)

      return Promise.resolve({ post_number: postNumber } as T)
    }

    if (this.query.includes('FROM forum_forums')) {
      const forumRef = String(this.values[0])
      const slugRef = String(this.values[1] ?? this.values[0])
      const forum =
        this.store.forums.find(
          row =>
            row.archived_at === null &&
            (row.id === forumRef || row.slug === slugRef),
        ) ?? null

      return Promise.resolve(forum as T | null)
    }

    if (this.query.includes('FROM forum_topics')) {
      const topicId = String(this.values[0])
      const topic =
        this.store.topics.find(
          row => row.archived_at === null && row.id === topicId,
        ) ?? null

      return Promise.resolve(topic as T | null)
    }

    if (this.query.includes('FROM forum_posts')) {
      const value = String(this.values[0])
      const post =
        this.store.posts.find(
          row =>
            row.archived_at === null &&
            (this.query.includes('forum_posts.idempotency_key = ?')
              ? row.idempotency_key === value
              : row.id === value),
        ) ?? null

      return Promise.resolve(post as T | null)
    }

    if (
      this.query.includes('FROM agent_profiles') &&
      this.query.includes('agent_credentials')
    ) {
      const slug = String(this.values[0])
      const row =
        this.store.registeredArtanis !== null &&
        this.store.registeredArtanis.slug === slug
          ? this.store.registeredArtanis
          : null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_tip_recipient_wallets')) {
      // The Artanis actor has no seeded tip-recipient wallet in this fixture,
      // so delivery resolves to the missing-readiness projection and proceeds.
      return Promise.resolve(null)
    }

    return Promise.reject(new Error(`Unexpected first query: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO forum_posts')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.posts.every(row => row.idempotency_key !== idempotencyKey)
      ) {
        this.store.posts.push({
          actor_json: String(this.values[5]),
          actor_ref: String(this.values[4]),
          archived_at: null,
          body_text: null,
          content_ref: String(this.values[6]),
          created_at: String(this.values[11]),
          forum_id: String(this.values[3]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          parent_post_id:
            this.values[7] === null ? null : String(this.values[7]),
          post_number: Number(this.values[9]),
          public_projection_json: String(this.values[10]),
          quote_post_id:
            this.values[8] === null ? null : String(this.values[8]),
          receipt_refs_json: '[]',
          revision_ref: null,
          state: 'visible',
          topic_id: String(this.values[2]),
          updated_at: String(this.values[12]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_post_bodies')) {
      const postId = String(this.values[0])
      const index = this.store.posts.findIndex(row => row.id === postId)

      if (index !== -1) {
        const existing = this.store.posts[index]!
        this.store.posts[index] = {
          ...existing,
          body_text: String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_topics')) {
      const topicId = String(this.values[2])
      const index = this.store.topics.findIndex(row => row.id === topicId)

      if (index !== -1) {
        const existing = this.store.topics[index]!
        this.store.topics[index] = {
          ...existing,
          latest_post_id: String(this.values[0]),
          post_count: existing.post_count + 1,
          updated_at: String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_forums')) {
      const forumId = String(this.values[3])
      const index = this.store.forums.findIndex(row => row.id === forumId)

      if (index !== -1) {
        const existing = this.store.forums[index]!
        this.store.forums[index] = {
          ...existing,
          latest_post_id: String(this.values[1]),
          latest_topic_id: String(this.values[0]),
          post_count: existing.post_count + 1,
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run query: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<T[] | [string[], ...T[]]> {
    return options?.columnNames === true
      ? Promise.resolve([[]] as [string[], ...T[]])
      : Promise.resolve([] as T[])
  }
}

const deliveryDb = (store: DeliveryStore): D1Database => {
  const artanisDb = artanisPersistenceTestDb(store.artanis)

  return {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run())) as Promise<
        Array<D1Result<T>>
      >,
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) =>
      query.includes('artanis_')
        ? artanisDb.prepare(query)
        : new DeliveryStatement(query, store),
    withSession: () => deliveryDb(store),
  } as unknown as D1Database
}

const runtime = {
  makeId: () => '88888888-6001-4001-8001-888888888888',
  nowEpochMillis: () => Date.parse(nowIso),
  nowIso: () => nowIso,
}

const intentRecord = (
  overrides: Partial<ArtanisForumPublicationIntentRecord> = {},
): ArtanisForumPublicationIntentRecord =>
  S.decodeUnknownSync(ArtanisForumPublicationIntentRecord)({
    ...exampleArtanisForumPublicationQueue().intents[0]!,
    ...overrides,
  })

const persistIntent = (
  store: DeliveryStore,
  intent: ArtanisForumPublicationIntentRecord,
) =>
  Effect.runPromise(
    saveArtanisForumPublicationIntent(deliveryDb(store), intent, nowIso),
  )

const persistedForumIntent = (store: DeliveryStore) =>
  JSON.parse(
    store.artanis.rows('artanis_forum_publication_intents')[0]!.record_json,
  ) as Record<string, unknown>

describe('Artanis Forum delivery', () => {
  test('delivers a ready persisted status intent into the canonical Forum topic', async () => {
    const store = new DeliveryStore()
    const db = deliveryDb(store)

    await persistIntent(store, intentRecord())

    const result = await Effect.runPromise(
      deliverReadyArtanisForumPublications(db, { runtime }),
    )
    const delivered = result.delivered[0]!

    expect(result.checkedIntentRefs).toEqual([
      'forum.public.artanis.status_intent.20260607T0121',
    ])
    expect(delivered).toMatchObject({
      forbiddenAuthority: ARTANIS_FORUM_DELIVERY_NO_EXTRA_AUTHORITY,
      idempotent: false,
      postNumber: 2,
      postRef: 'post.public.forum.artanis.status.2',
      publicUrl: `/forum/t/${ARTANIS_CANONICAL_TOPIC_IDS.status}#post-${delivered.postId}`,
    })
    expect(store.posts).toHaveLength(2)
    expect(store.posts[1]).toMatchObject({
      actor_ref: registeredArtanisActorRef,
      body_text:
        'Artanis status update: Pylon v0.2 release work is active, Model Lab evidence is being gathered, and public proofs will be linked as they are accepted.',
      idempotency_key: 'artanis-forum:status:20260607T0121:v1',
      post_number: 2,
      topic_id: ARTANIS_CANONICAL_TOPIC_IDS.status,
    })
    expect(store.topics[0]).toMatchObject({
      latest_post_id: delivered.postId,
      post_count: 2,
    })
    expect(store.forums[0]).toMatchObject({
      latest_post_id: delivered.postId,
      post_count: 2,
    })
    expect(persistedForumIntent(store)).toMatchObject({
      deliveredAtIso: nowIso,
      deliveryReceiptRefs: [
        'receipt.public.artanis.forum_delivery.forum_public_artanis_status_intent_20260607t0121',
      ],
      deliveryState: 'delivered',
      postRef: 'post.public.forum.artanis.status.2',
    })
  })

  test('fails closed when the registered Artanis Forum identity is unavailable', async () => {
    const store = new DeliveryStore()
    store.registeredArtanis = null
    const db = deliveryDb(store)

    await persistIntent(store, intentRecord())

    await expect(
      Effect.runPromise(deliverReadyArtanisForumPublications(db, { runtime })),
    ).rejects.toMatchObject({
      _tag: 'ArtanisForumDeliveryError',
      kind: 'identity_unavailable',
    })
    expect(store.posts).toHaveLength(1)
  })

  test('collapses duplicate delivery retries to the existing Forum post ref', async () => {
    const store = new DeliveryStore()
    const db = deliveryDb(store)
    const intent = intentRecord()

    await persistIntent(store, intent)
    await Effect.runPromise(
      deliverArtanisForumPublicationIntent(db, intent, runtime),
    )

    const retry = await Effect.runPromise(
      deliverArtanisForumPublicationIntent(db, intent, {
        ...runtime,
        makeId: () => 'duplicate-post-id',
      }),
    )

    expect(retry).toMatchObject({
      idempotent: true,
      postId: '88888888-6001-4001-8001-888888888888',
      postRef: 'post.public.forum.artanis.status.2',
    })
    expect(store.posts).toHaveLength(2)
  })

  test('fails closed for locked, hidden, and archived target topics', async () => {
    await Promise.all(
      (['locked', 'hidden', 'archived'] as const).map(async state => {
        const store = new DeliveryStore()
        const db = deliveryDb(store)
        store.topics[0] = { ...store.topics[0]!, state }

        await persistIntent(store, intentRecord())

        await expect(
          Effect.runPromise(
            deliverReadyArtanisForumPublications(db, { runtime }),
          ),
        ).rejects.toMatchObject({
          _tag: 'ArtanisForumDeliveryError',
          kind: 'target_blocked',
        })
        expect(store.posts).toHaveLength(1)
      }),
    )
  })

  test('fails closed for unsafe body text before writing a Forum post', async () => {
    const store = new DeliveryStore()
    const db = deliveryDb(store)
    const unsafeIntent = intentRecord({
      bodyText:
        'Artanis status update includes raw prompt data and sk-secret material.',
    })

    await expect(
      Effect.runPromise(
        deliverArtanisForumPublicationIntent(db, unsafeIntent, runtime),
      ),
    ).rejects.toMatchObject({
      _tag: 'ArtanisForumDeliveryError',
      kind: 'unsafe_intent',
    })
    expect(store.posts).toHaveLength(1)
  })

  test('fails closed when the idempotency key belongs to another post payload', async () => {
    const store = new DeliveryStore()
    const db = deliveryDb(store)
    const intent = intentRecord()

    store.posts.push({
      ...store.posts[0]!,
      body_text: 'Different Artanis payload.',
      id: '88888888-6002-4002-8002-888888888888',
      idempotency_key: intent.idempotencyKey,
      parent_post_id: store.posts[0]!.id,
      post_number: 2,
    })
    await persistIntent(store, intent)

    await expect(
      Effect.runPromise(
        deliverArtanisForumPublicationIntent(db, intent, runtime),
      ),
    ).rejects.toMatchObject({
      _tag: 'ArtanisForumDeliveryError',
      kind: 'existing_post_conflict',
    })
    expect(persistedForumIntent(store)).toMatchObject({
      deliveryState: 'ready',
      postRef: null,
    })
  })
})
