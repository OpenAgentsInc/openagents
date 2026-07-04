import { Schema as S } from 'effect'

// KS-8.11 (#8322): list functions take the `CrmEmailDatabase` union — a
// plain D1Database keeps working (no mirroring); the dual-write handle
// converges the Postgres twins fail-soft after each authoritative D1 write.
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  mirrorCrmEmailRows,
} from './crm-email-domain-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const SubscriberListStatus = S.Literals(['active', 'paused', 'archived'])
export type SubscriberListStatus = typeof SubscriberListStatus.Type

export const ListSubscriberStatus = S.Literals([
  'active',
  'unsubscribed',
  'bounced',
])
export type ListSubscriberStatus = typeof ListSubscriberStatus.Type

export type NativeListsRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemNativeListsRuntime: NativeListsRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

export type SubscriberListRecord = Readonly<{
  id: string
  metadataJson: string
  name: string
  ownerUserId: string | null
  slug: string
  sourceAuthorityRef: string
  status: SubscriberListStatus
  teamId: string | null
}>

export type ListSubscriberRecord = Readonly<{
  email: string
  id: string
  idempotencyKey: string
  listId: string
  metadataJson: string
  sourceRef: string
  status: ListSubscriberStatus
}>

type SubscriberListRow = Readonly<{
  id: string
  metadata_json: string
  name: string
  owner_user_id: string | null
  slug: string
  source_authority_ref: string
  status: SubscriberListStatus
  team_id: string | null
}>

type ListSubscriberRow = Readonly<{
  email: string
  id: string
  idempotency_key: string
  list_id: string
  metadata_json: string
  source_ref: string
  status: ListSubscriberStatus
}>

const clampText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const normalizeEmail = (email: string): string =>
  clampText(email.toLowerCase(), 320)

const slugify = (value: string): string =>
  clampText(value, 120)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 120)

const metadataJson = (
  metadata: Record<string, string | number | boolean | null> | undefined,
): string =>
  JSON.stringify(
    Object.fromEntries(
      Object.entries(metadata ?? {})
        .slice(0, 20)
        .map(([key, value]) => [
          key
            .trim()
            .replace(/[^a-zA-Z0-9_.:-]/g, '_')
            .slice(0, 80),
          typeof value === 'string' ? clampText(value, 240) : value,
        ])
        .filter(([key]) => key !== ''),
    ),
  )

export const makeSubscriberListRecord = (
  input: Readonly<{
    metadata?: Record<string, string | number | boolean | null> | undefined
    name: string
    ownerUserId?: string | null | undefined
    slug?: string | undefined
    sourceAuthorityRef: string
    status?: SubscriberListStatus | undefined
    teamId?: string | null | undefined
  }>,
  runtime: NativeListsRuntime = systemNativeListsRuntime,
): SubscriberListRecord => {
  const slug = slugify(
    input.slug === undefined || input.slug.trim() === ''
      ? input.name
      : input.slug,
  )

  return {
    id: runtime.makeId('subscriber_list'),
    metadataJson: metadataJson(input.metadata),
    name: clampText(input.name, 160),
    ownerUserId: input.ownerUserId ?? null,
    slug,
    sourceAuthorityRef: clampText(input.sourceAuthorityRef, 240),
    status: input.status ?? 'active',
    teamId: input.teamId ?? null,
  }
}

export const makeListSubscriberRecord = (
  input: Readonly<{
    email: string
    listId: string
    metadata?: Record<string, string | number | boolean | null> | undefined
    sourceRef: string
    status?: ListSubscriberStatus | undefined
  }>,
  runtime: NativeListsRuntime = systemNativeListsRuntime,
): ListSubscriberRecord => {
  const email = normalizeEmail(input.email)

  return {
    email,
    id: runtime.makeId('list_subscriber'),
    idempotencyKey: `list_subscriber:${input.listId}:${email}`,
    listId: input.listId,
    metadataJson: metadataJson(input.metadata),
    sourceRef: clampText(input.sourceRef, 240),
    status: input.status ?? 'active',
  }
}

const listFromRow = (row: SubscriberListRow): SubscriberListRecord => ({
  id: row.id,
  metadataJson: row.metadata_json,
  name: row.name,
  ownerUserId: row.owner_user_id,
  slug: row.slug,
  sourceAuthorityRef: row.source_authority_ref,
  status: row.status,
  teamId: row.team_id,
})

const subscriberFromRow = (row: ListSubscriberRow): ListSubscriberRecord => ({
  email: row.email,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  listId: row.list_id,
  metadataJson: row.metadata_json,
  sourceRef: row.source_ref,
  status: row.status,
})

export type NativeListsServiceShape = Readonly<{
  addSubscriber: (
    input: Readonly<{
      email: string
      listId: string
      metadata?: Record<string, string | number | boolean | null> | undefined
      sourceRef: string
    }>,
  ) => Promise<
    Readonly<{ idempotent: boolean; subscriber: ListSubscriberRecord }>
  >
  createList: (
    input: Readonly<{
      metadata?: Record<string, string | number | boolean | null> | undefined
      name: string
      ownerUserId?: string | null | undefined
      slug?: string | undefined
      sourceAuthorityRef: string
      status?: SubscriberListStatus | undefined
      teamId?: string | null | undefined
    }>,
  ) => Promise<SubscriberListRecord>
  listSubscribers: (
    input: Readonly<{
      limit?: number | undefined
      listId: string
      status?: ListSubscriberStatus | undefined
    }>,
  ) => Promise<ReadonlyArray<ListSubscriberRecord>>
  readList: (listId: string) => Promise<SubscriberListRecord | undefined>
  unsubscribe: (
    input: Readonly<{ email: string; listId: string }>,
  ) => Promise<ListSubscriberRecord | undefined>
}>

const SUBSCRIBER_LIST_LIMIT = 500

export const makeNativeListsService = (
  database: CrmEmailDatabase,
  runtime: NativeListsRuntime = systemNativeListsRuntime,
): NativeListsServiceShape => {
  // KS-8.11 (#8322): D1 stays the write/read authority; when `database` is
  // the dual-write handle each write below also converges its Postgres twin
  // fail-soft.
  const db = crmEmailAuthorityDb(database)

  return {
    addSubscriber: async input => {
      const now = runtime.nowIso()
      const record = makeListSubscriberRecord(
        {
          email: input.email,
          listId: input.listId,
          metadata: input.metadata,
          sourceRef: input.sourceRef,
        },
        runtime,
      )

      await db
        .prepare(
          `INSERT INTO list_subscribers
          (id, list_id, email, status, source_ref, idempotency_key,
           metadata_json, subscribed_at, unsubscribed_at, bounced_at,
           created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
         ON CONFLICT(idempotency_key) DO NOTHING`,
        )
        .bind(
          record.id,
          record.listId,
          record.email,
          record.status,
          record.sourceRef,
          record.idempotencyKey,
          record.metadataJson,
          now,
          now,
          now,
        )
        .run()

      await mirrorCrmEmailRows(
        database,
        'list_subscribers',
        'idempotency_key',
        [record.idempotencyKey],
      )

      const stored = await db
        .prepare(
          `SELECT id, list_id, email, status, source_ref, idempotency_key,
                metadata_json
           FROM list_subscribers
          WHERE idempotency_key = ?
          LIMIT 1`,
        )
        .bind(record.idempotencyKey)
        .first<ListSubscriberRow>()

      if (stored === null) {
        return { idempotent: false, subscriber: record }
      }

      return {
        idempotent: stored.id !== record.id,
        subscriber: subscriberFromRow(stored),
      }
    },
    createList: async input => {
      const now = runtime.nowIso()
      const record = makeSubscriberListRecord(input, runtime)

      await db
        .prepare(
          `INSERT INTO subscriber_lists
          (id, owner_user_id, team_id, slug, name, status,
           source_authority_ref, metadata_json, created_at, updated_at,
           archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
         ON CONFLICT(slug) DO UPDATE SET
           name = excluded.name,
           status = excluded.status,
           source_authority_ref = excluded.source_authority_ref,
           metadata_json = excluded.metadata_json,
           updated_at = excluded.updated_at`,
        )
        .bind(
          record.id,
          record.ownerUserId,
          record.teamId,
          record.slug,
          record.name,
          record.status,
          record.sourceAuthorityRef,
          record.metadataJson,
          now,
          now,
        )
        .run()

      await mirrorCrmEmailRows(database, 'subscriber_lists', 'slug', [
        record.slug,
      ])

      const stored = await db
        .prepare(
          `SELECT id, owner_user_id, team_id, slug, name, status,
                source_authority_ref, metadata_json
           FROM subscriber_lists
          WHERE slug = ?
          LIMIT 1`,
        )
        .bind(record.slug)
        .first<SubscriberListRow>()

      return stored === null ? record : listFromRow(stored)
    },
    listSubscribers: async input => {
      const limit = Math.max(
        1,
        Math.min(SUBSCRIBER_LIST_LIMIT, Math.floor(input.limit ?? 200)),
      )
      const result =
        input.status === undefined
          ? await db
              .prepare(
                `SELECT id, list_id, email, status, source_ref, idempotency_key,
                      metadata_json
                 FROM list_subscribers
                WHERE list_id = ?
                ORDER BY updated_at DESC
                LIMIT ?`,
              )
              .bind(input.listId, limit)
              .all<ListSubscriberRow>()
          : await db
              .prepare(
                `SELECT id, list_id, email, status, source_ref, idempotency_key,
                      metadata_json
                 FROM list_subscribers
                WHERE list_id = ?
                  AND status = ?
                ORDER BY updated_at DESC
                LIMIT ?`,
              )
              .bind(input.listId, input.status, limit)
              .all<ListSubscriberRow>()

      return result.results.map(subscriberFromRow)
    },
    readList: async listId => {
      const row = await db
        .prepare(
          `SELECT id, owner_user_id, team_id, slug, name, status,
                source_authority_ref, metadata_json
           FROM subscriber_lists
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
        )
        .bind(listId)
        .first<SubscriberListRow>()

      return row === null ? undefined : listFromRow(row)
    },
    unsubscribe: async input => {
      const now = runtime.nowIso()
      const email = normalizeEmail(input.email)

      await db
        .prepare(
          `UPDATE list_subscribers
            SET status = 'unsubscribed',
                unsubscribed_at = ?,
                updated_at = ?
          WHERE list_id = ?
            AND email = ?
            AND status <> 'unsubscribed'`,
        )
        .bind(now, now, input.listId, email)
        .run()

      const row = await db
        .prepare(
          `SELECT id, list_id, email, status, source_ref, idempotency_key,
                metadata_json
           FROM list_subscribers
          WHERE list_id = ?
            AND email = ?
          LIMIT 1`,
        )
        .bind(input.listId, email)
        .first<ListSubscriberRow>()

      if (row !== null) {
        await mirrorCrmEmailRows(database, 'list_subscribers', 'id', [row.id])
      }

      return row === null ? undefined : subscriberFromRow(row)
    },
  }
}
