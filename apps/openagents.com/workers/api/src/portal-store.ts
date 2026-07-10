// PORTAL-1 (#8652): client portal engagement store.
//
// The Sell-in-Public revenue loop (docs/transcripts/247.md) onboards design
// partners into ENGAGEMENTS whose agent-drafted content items (A/B post
// pairs) await client approve/reject in /portal. This module owns the typed
// D1 storage boundary for `portal_engagements` + `portal_content_items`
// (migration 0315).
//
// Owner-scoping is the load-bearing rule: client reads resolve ONLY through
// the caller's verified session identity (`client_user_id`, or a
// case-insensitive `client_email` binding for pre-first-login engagements).
// There is no client-facing engagement-id lookup, so a client can never read
// another engagement. Decisions are immutable: `decided_at` and the unique
// `decision_receipt_ref` (`portal_content_decision:<opaque id>`) are written
// exactly once, following the 0308 `admin_credit_grants` receipt precedent.

import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/=#-]{0,240}$/
const BOUNDED_TEXT_MAX = 8_000
const BOUNDED_NAME_MAX = 200

export const PORTAL_ENGAGEMENT_STATUSES = [
  'preparing',
  'active',
  'paused',
  'closed',
] as const
export type PortalEngagementStatus =
  (typeof PORTAL_ENGAGEMENT_STATUSES)[number]

export const PORTAL_CONTENT_KINDS = ['post', 'email', 'ad'] as const
export type PortalContentKind = (typeof PORTAL_CONTENT_KINDS)[number]

export const PORTAL_CONTENT_VARIANTS = ['a', 'b'] as const
export type PortalContentVariant = (typeof PORTAL_CONTENT_VARIANTS)[number]

export const PORTAL_CONTENT_STATES = [
  'draft',
  'approved',
  'rejected',
  'published',
] as const
export type PortalContentState = (typeof PORTAL_CONTENT_STATES)[number]

export const PORTAL_DECISIONS = ['approve', 'reject'] as const
export type PortalDecision = (typeof PORTAL_DECISIONS)[number]

export class PortalValidationError extends Error {
  constructor(reason: string) {
    super(reason)
    this.name = 'PortalValidationError'
  }
}

const assertSafeRef = (field: string, value: string): void => {
  if (!SAFE_REF_PATTERN.test(value)) {
    throw new PortalValidationError(
      `${field} must be a bounded public-safe ref`,
    )
  }
}

const assertBoundedText = (
  field: string,
  value: string,
  max: number,
): void => {
  const trimmed = value.trim()
  if (trimmed.length === 0 || trimmed.length > max) {
    throw new PortalValidationError(
      `${field} must be non-empty and at most ${max} characters`,
    )
  }
}

const normalizedEmail = (value: string): string => value.trim().toLowerCase()

const isEmailShaped = (value: string): boolean =>
  /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value) && value.length <= 320

/**
 * Session user ids come in two shapes: provider refs (`github:14167547`,
 * SAFE_REF) and email-provider subjects (`email:chris@example.com`, whose
 * `@` SAFE_REF rejects). #8652 reopen: the original SAFE_REF-only guard made
 * every email-login client unable to read even their OWN engagement — the
 * exact audience the email binding exists for. All lookups bind the id as a
 * query parameter, so this validation is shape/bound defense only.
 */
const isClientUserIdShaped = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) ||
  (value.startsWith('email:') && isEmailShaped(value.slice('email:'.length)))

const assertClientUserId = (field: string, value: string): void => {
  if (!isClientUserIdShaped(value)) {
    throw new PortalValidationError(
      `${field} must be a bounded session user id`,
    )
  }
}

export type PortalEngagement = Readonly<{
  id: string
  name: string
  status: PortalEngagementStatus
  clientUserId: string | null
  clientEmail: string | null
  createdAt: string
  updatedAt: string
}>

export type PortalContentItem = Readonly<{
  id: string
  engagementId: string
  kind: PortalContentKind
  channel: string
  variant: PortalContentVariant
  pairRef: string | null
  title: string
  body: string
  state: PortalContentState
  decidedAt: string | null
  decisionReceiptRef: string | null
  createdAt: string
  updatedAt: string
}>

export type PortalDecisionResult = Readonly<{
  item: PortalContentItem
  receiptRef: string
  alreadyDecided: boolean
}>

export type PortalCreateEngagementInput = Readonly<{
  name: string
  status?: PortalEngagementStatus | undefined
  clientEmail?: string | null | undefined
}>

export type PortalBindClientInput = Readonly<{
  engagementId: string
  clientUserId?: string | null | undefined
  clientEmail?: string | null | undefined
}>

export type PortalSeedContentItemInput = Readonly<{
  kind?: PortalContentKind | undefined
  channel: string
  variant?: PortalContentVariant | undefined
  pairRef?: string | null | undefined
  title: string
  body: string
}>

export type PortalRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemPortalRuntime: PortalRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

type PortalEngagementD1Row = Readonly<{
  id: string
  name: string
  status: string
  client_user_id: string | null
  client_email: string | null
  created_at: string
  updated_at: string
}>

type PortalContentItemD1Row = Readonly<{
  id: string
  engagement_id: string
  kind: string
  channel: string
  variant: string
  pair_ref: string | null
  title: string
  body: string
  state: string
  decided_at: string | null
  decision_receipt_ref: string | null
  created_at: string
  updated_at: string
}>

const engagementFromRow = (row: PortalEngagementD1Row): PortalEngagement => ({
  id: row.id,
  name: row.name,
  status: row.status as PortalEngagementStatus,
  clientUserId: row.client_user_id,
  clientEmail: row.client_email,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const contentItemFromRow = (
  row: PortalContentItemD1Row,
): PortalContentItem => ({
  id: row.id,
  engagementId: row.engagement_id,
  kind: row.kind as PortalContentKind,
  channel: row.channel,
  variant: row.variant as PortalContentVariant,
  pairRef: row.pair_ref,
  title: row.title,
  body: row.body,
  state: row.state as PortalContentState,
  decidedAt: row.decided_at,
  decisionReceiptRef: row.decision_receipt_ref,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

export type PortalStore = Readonly<{
  createEngagement: (
    input: PortalCreateEngagementInput,
    runtime?: PortalRuntime,
  ) => Promise<PortalEngagement>
  bindClient: (
    input: PortalBindClientInput,
    runtime?: PortalRuntime,
  ) => Promise<PortalEngagement | null>
  readEngagementById: (
    engagementId: string,
  ) => Promise<PortalEngagement | null>
  readEngagementForClient: (identity: {
    userId: string
    email: string | null
  }) => Promise<PortalEngagement | null>
  listContentItems: (
    engagementId: string,
  ) => Promise<ReadonlyArray<PortalContentItem>>
  readContentItemById: (itemId: string) => Promise<PortalContentItem | null>
  seedContentItems: (
    engagementId: string,
    items: ReadonlyArray<PortalSeedContentItemInput>,
    runtime?: PortalRuntime,
  ) => Promise<ReadonlyArray<PortalContentItem>>
  decideContentItem: (
    itemId: string,
    decision: PortalDecision,
    runtime?: PortalRuntime,
  ) => Promise<PortalDecisionResult>
}>

export const makeD1PortalStore = (db: D1Database): PortalStore => {
  const createEngagement = async (
    input: PortalCreateEngagementInput,
    runtime: PortalRuntime = systemPortalRuntime,
  ): Promise<PortalEngagement> => {
    assertBoundedText('name', input.name, BOUNDED_NAME_MAX)
    const status = input.status ?? 'preparing'
    if (!PORTAL_ENGAGEMENT_STATUSES.includes(status)) {
      throw new PortalValidationError('status must be a known engagement status')
    }
    const email =
      input.clientEmail == null || input.clientEmail.trim() === ''
        ? null
        : normalizedEmail(input.clientEmail)
    if (email !== null && !isEmailShaped(email)) {
      throw new PortalValidationError('clientEmail must be a valid email')
    }

    const id = runtime.makeId('portal_engagement')
    const nowIso = runtime.nowIso()
    await db
      .prepare(
        `INSERT INTO portal_engagements (
          id, name, status, client_user_id, client_email, created_at, updated_at
        ) VALUES (?, ?, ?, NULL, ?, ?, ?)`,
      )
      .bind(id, input.name.trim(), status, email, nowIso, nowIso)
      .run()

    const created = await readEngagementById(id)
    if (created === null) {
      throw new PortalValidationError('engagement insert did not persist')
    }
    return created
  }

  const bindClient = async (
    input: PortalBindClientInput,
    runtime: PortalRuntime = systemPortalRuntime,
  ): Promise<PortalEngagement | null> => {
    assertSafeRef('engagementId', input.engagementId)
    const userId =
      input.clientUserId == null || input.clientUserId.trim() === ''
        ? null
        : input.clientUserId.trim()
    if (userId !== null) {
      assertClientUserId('clientUserId', userId)
    }
    const email =
      input.clientEmail == null || input.clientEmail.trim() === ''
        ? null
        : normalizedEmail(input.clientEmail)
    if (email !== null && !isEmailShaped(email)) {
      throw new PortalValidationError('clientEmail must be a valid email')
    }
    if (userId === null && email === null) {
      throw new PortalValidationError(
        'bind requires clientUserId or clientEmail',
      )
    }

    const existing = await readEngagementById(input.engagementId)
    if (existing === null) return null

    await db
      .prepare(
        `UPDATE portal_engagements
           SET client_user_id = COALESCE(?, client_user_id),
               client_email = COALESCE(?, client_email),
               updated_at = ?
         WHERE id = ?`,
      )
      .bind(userId, email, runtime.nowIso(), input.engagementId)
      .run()

    return readEngagementById(input.engagementId)
  }

  const readEngagementById = async (
    engagementId: string,
  ): Promise<PortalEngagement | null> => {
    if (!SAFE_REF_PATTERN.test(engagementId)) return null
    const row = await db
      .prepare('SELECT * FROM portal_engagements WHERE id = ?')
      .bind(engagementId)
      .first<PortalEngagementD1Row>()
    return row === null ? null : engagementFromRow(row)
  }

  const readEngagementForClient = async (identity: {
    userId: string
    email: string | null
  }): Promise<PortalEngagement | null> => {
    if (!isClientUserIdShaped(identity.userId)) return null

    const byUser = await db
      .prepare(
        `SELECT * FROM portal_engagements
          WHERE client_user_id = ?
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(identity.userId)
      .first<PortalEngagementD1Row>()
    if (byUser !== null) return engagementFromRow(byUser)

    const email =
      identity.email == null || identity.email.trim() === ''
        ? null
        : normalizedEmail(identity.email)
    if (email === null || !isEmailShaped(email)) return null

    const byEmail = await db
      .prepare(
        `SELECT * FROM portal_engagements
          WHERE client_user_id IS NULL AND client_email = ?
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(email)
      .first<PortalEngagementD1Row>()
    return byEmail === null ? null : engagementFromRow(byEmail)
  }

  const listContentItems = async (
    engagementId: string,
  ): Promise<ReadonlyArray<PortalContentItem>> => {
    if (!SAFE_REF_PATTERN.test(engagementId)) return []
    const result = await db
      .prepare(
        `SELECT * FROM portal_content_items
          WHERE engagement_id = ?
          ORDER BY created_at ASC, pair_ref ASC, variant ASC`,
      )
      .bind(engagementId)
      .all<PortalContentItemD1Row>()
    return (result.results ?? []).map(contentItemFromRow)
  }

  const readContentItemById = async (
    itemId: string,
  ): Promise<PortalContentItem | null> => {
    if (!SAFE_REF_PATTERN.test(itemId)) return null
    const row = await db
      .prepare('SELECT * FROM portal_content_items WHERE id = ?')
      .bind(itemId)
      .first<PortalContentItemD1Row>()
    return row === null ? null : contentItemFromRow(row)
  }

  const seedContentItems = async (
    engagementId: string,
    items: ReadonlyArray<PortalSeedContentItemInput>,
    runtime: PortalRuntime = systemPortalRuntime,
  ): Promise<ReadonlyArray<PortalContentItem>> => {
    assertSafeRef('engagementId', engagementId)
    if (items.length === 0 || items.length > 50) {
      throw new PortalValidationError('items must contain 1-50 entries')
    }
    const engagement = await readEngagementById(engagementId)
    if (engagement === null) {
      throw new PortalValidationError('engagement not found')
    }

    const createdIds: Array<string> = []
    for (const item of items) {
      const kind = item.kind ?? 'post'
      if (!PORTAL_CONTENT_KINDS.includes(kind)) {
        throw new PortalValidationError('kind must be a known content kind')
      }
      const variant = item.variant ?? 'a'
      if (!PORTAL_CONTENT_VARIANTS.includes(variant)) {
        throw new PortalValidationError('variant must be a or b')
      }
      assertBoundedText('channel', item.channel, BOUNDED_NAME_MAX)
      assertBoundedText('title', item.title, BOUNDED_NAME_MAX)
      assertBoundedText('body', item.body, BOUNDED_TEXT_MAX)
      const pairRef =
        item.pairRef == null || item.pairRef.trim() === ''
          ? null
          : item.pairRef.trim()
      if (pairRef !== null) {
        assertSafeRef('pairRef', pairRef)
      }

      const id = runtime.makeId('portal_content')
      const nowIso = runtime.nowIso()
      await db
        .prepare(
          `INSERT INTO portal_content_items (
            id, engagement_id, kind, channel, variant, pair_ref,
            title, body, state, decided_at, decision_receipt_ref,
            created_at, updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'draft', NULL, NULL, ?, ?)`,
        )
        .bind(
          id,
          engagementId,
          kind,
          item.channel.trim(),
          variant,
          pairRef,
          item.title.trim(),
          item.body.trim(),
          nowIso,
          nowIso,
        )
        .run()
      createdIds.push(id)
    }

    const all = await listContentItems(engagementId)
    return all.filter((item) => createdIds.includes(item.id))
  }

  const decideContentItem = async (
    itemId: string,
    decision: PortalDecision,
    runtime: PortalRuntime = systemPortalRuntime,
  ): Promise<PortalDecisionResult> => {
    if (!PORTAL_DECISIONS.includes(decision)) {
      throw new PortalValidationError('decision must be approve or reject')
    }
    const item = await readContentItemById(itemId)
    if (item === null) {
      throw new PortalValidationError('content item not found')
    }

    const targetState: PortalContentState =
      decision === 'approve' ? 'approved' : 'rejected'

    if (item.state !== 'draft') {
      // Idempotent repeat of the SAME decision returns the existing receipt;
      // decisions never flip after the receipt is minted.
      if (item.state === targetState && item.decisionReceiptRef !== null) {
        return {
          item,
          receiptRef: item.decisionReceiptRef,
          alreadyDecided: true,
        }
      }
      throw new PortalValidationError(
        `content item already ${item.state}; decisions are immutable`,
      )
    }

    const receiptRef = `portal_content_decision:${runtime.makeId('pcd')}`
    const nowIso = runtime.nowIso()
    // Single-statement guarded write (the Cloud Run D1 HTTP bridge has no
    // atomic multi-statement batch): the WHERE state='draft' clause makes a
    // concurrent double-decide lose cleanly.
    const result = await db
      .prepare(
        `UPDATE portal_content_items
            SET state = ?, decided_at = ?, decision_receipt_ref = ?, updated_at = ?
          WHERE id = ? AND state = 'draft'`,
      )
      .bind(targetState, nowIso, receiptRef, nowIso, itemId)
      .run()

    const changed =
      (result.meta?.changes ?? (result.meta as { changed_db?: boolean } | undefined)?.changed_db ?? 1) !== 0
    if (!changed) {
      const latest = await readContentItemById(itemId)
      if (
        latest !== null &&
        latest.state === targetState &&
        latest.decisionReceiptRef !== null
      ) {
        return {
          item: latest,
          receiptRef: latest.decisionReceiptRef,
          alreadyDecided: true,
        }
      }
      throw new PortalValidationError(
        'content item was decided concurrently; decisions are immutable',
      )
    }

    const updated = await readContentItemById(itemId)
    if (updated === null || updated.decisionReceiptRef === null) {
      throw new PortalValidationError('decision write did not persist')
    }
    return {
      item: updated,
      receiptRef: updated.decisionReceiptRef,
      alreadyDecided: false,
    }
  }

  return {
    createEngagement,
    bindClient,
    readEngagementById,
    readEngagementForClient,
    listContentItems,
    readContentItemById,
    seedContentItems,
    decideContentItem,
  }
}
