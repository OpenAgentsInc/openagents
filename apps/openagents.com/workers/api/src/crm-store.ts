/**
 * Native CRM storage layer (epic #5980, sub-issue #5981).
 *
 * Pure D1 helpers over the tenant-scoped contact CRM tables added in migration
 * `0218_crm_contacts.sql`: contacts, accounts, lists (+memberships), activities,
 * engagement snapshots, opportunities (+roles), and import-run audit rows.
 *
 * Everything here is tenant-scoped: every read and write is filtered by
 * `tenantRef` so the same engine isolates OpenAgents' own outreach from each
 * customer's. No money movement, no email sending — this is the data model the
 * read APIs (this issue), the CSV importer (#5982), and the send channels
 * (#5983-#5985) build on.
 *
 * Style mirrors `email-campaigns.ts`: a small injectable `CrmRuntime`
 * (`makeId` + `nowIso`) and direct prepared statements, kept deterministic and
 * unit-testable against an in-memory D1 fake.
 */
import { Schema as S } from 'effect'

// KS-8.11 (#8322): every function here takes the `CrmEmailDatabase` union —
// a plain D1Database keeps working (no mirroring); when the caller passes
// the dual-write handle, each write converges its Postgres twin fail-soft
// (`mirrorCrmEmailRows` never throws) after the authoritative D1 write.
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  mirrorCrmEmailRows,
} from './crm-email-domain-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export class CrmStorageError extends S.TaggedErrorClass<CrmStorageError>()(
  'CrmStorageError',
  {
    operation: S.String,
  },
) {}

export type CrmRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const defaultCrmRuntime: CrmRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

/** The default tenant for OpenAgents' own outreach. Customers get their own. */
export const DEFAULT_CRM_TENANT_REF = 'tenant.openagents'

/** Normalize an email for the per-tenant dedupe key (trim + lowercase). */
export const normalizeCrmEmail = (email: string): string =>
  email.trim().toLowerCase()

// ---------------------------------------------------------------------------
// Projections (camelCase, public-to-operator shape)
// ---------------------------------------------------------------------------

export type CrmContact = Readonly<{
  id: string
  tenantRef: string
  primaryEmail: string
  secondaryEmail: string | null
  fullName: string | null
  firstName: string | null
  lastName: string | null
  jobTitle: string | null
  contactType: string
  relationshipStage: string
  lifecycleStage: string
  accountId: string | null
  portalAccessStatus: string
  engagementScore: number
  lastContactedAt: string | null
  lastEngagedAt: string | null
  lastRepliedAt: string | null
  externalSourceLabel: string | null
  externalSourceId: string | null
  notes: string | null
  createdAt: string
  updatedAt: string
}>

export type CrmAccount = Readonly<{
  id: string
  tenantRef: string
  name: string
  domain: string | null
  accountType: string
  status: string
  websiteUrl: string | null
  notes: string | null
  createdAt: string
  updatedAt: string
}>

export type CrmContactList = Readonly<{
  id: string
  tenantRef: string
  slug: string
  name: string
  description: string | null
  isSystem: boolean
  createdAt: string
  updatedAt: string
}>

export type CrmActivity = Readonly<{
  id: string
  tenantRef: string
  contactId: string
  accountId: string | null
  activityType: string
  subject: string | null
  summary: string | null
  occurredAt: string
  actorRef: string | null
  sourceSystem: string
  sourceRecordType: string | null
  sourceRecordId: string | null
  createdAt: string
}>

export type CrmEngagementSnapshot = Readonly<{
  contactId: string
  tenantRef: string
  lastEmailSentAt: string | null
  lastEmailOpenedAt: string | null
  lastEmailClickedAt: string | null
  lastEmailRepliedAt: string | null
  emailSentCount30d: number
  emailOpenCount30d: number
  emailClickCount30d: number
  engagementScore: number
  updatedAt: string
}>

export type CrmOpportunity = Readonly<{
  id: string
  tenantRef: string
  accountId: string | null
  name: string
  roundName: string | null
  stage: string
  status: string
  targetAmountCents: number | null
  expectedAmountCents: number | null
  convictionProbability: number | null
  targetCloseDate: string | null
  summary: string | null
  /** Opaque JSON bag (e.g. OB-5 `{ sourceRef, pipelineRef }` attribution
   * refs). Callers that need typed access should decode this themselves. */
  metadataJson: string
  createdAt: string
  updatedAt: string
}>

export type CrmSourceImportRun = Readonly<{
  id: string
  tenantRef: string
  sourceLabel: string
  status: string
  totalRows: number
  importedRows: number
  updatedRows: number
  duplicateRows: number
  failedRows: number
  errorSummary: string | null
  createdAt: string
  updatedAt: string
}>

// ---------------------------------------------------------------------------
// Row decoders (defensive coercion from D1 result objects)
// ---------------------------------------------------------------------------

const str = (value: unknown): string =>
  value === null || value === undefined ? '' : String(value)
const nullableStr = (value: unknown): string | null =>
  value === null || value === undefined ? null : String(value)
const num = (value: unknown): number => {
  const parsed = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}
const nullableNum = (value: unknown): number | null => {
  if (value === null || value === undefined) return null
  const parsed = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

const decodeContact = (row: Record<string, unknown>): CrmContact => ({
  id: str(row.id),
  tenantRef: str(row.tenant_ref),
  primaryEmail: str(row.primary_email),
  secondaryEmail: nullableStr(row.secondary_email),
  fullName: nullableStr(row.full_name),
  firstName: nullableStr(row.first_name),
  lastName: nullableStr(row.last_name),
  jobTitle: nullableStr(row.job_title),
  contactType: str(row.contact_type),
  relationshipStage: str(row.relationship_stage),
  lifecycleStage: str(row.lifecycle_stage),
  accountId: nullableStr(row.account_id),
  portalAccessStatus: str(row.portal_access_status),
  engagementScore: num(row.engagement_score),
  lastContactedAt: nullableStr(row.last_contacted_at),
  lastEngagedAt: nullableStr(row.last_engaged_at),
  lastRepliedAt: nullableStr(row.last_replied_at),
  externalSourceLabel: nullableStr(row.external_source_label),
  externalSourceId: nullableStr(row.external_source_id),
  notes: nullableStr(row.notes),
  createdAt: str(row.created_at),
  updatedAt: str(row.updated_at),
})

const decodeAccount = (row: Record<string, unknown>): CrmAccount => ({
  id: str(row.id),
  tenantRef: str(row.tenant_ref),
  name: str(row.name),
  domain: nullableStr(row.domain),
  accountType: str(row.account_type),
  status: str(row.status),
  websiteUrl: nullableStr(row.website_url),
  notes: nullableStr(row.notes),
  createdAt: str(row.created_at),
  updatedAt: str(row.updated_at),
})

const decodeList = (row: Record<string, unknown>): CrmContactList => ({
  id: str(row.id),
  tenantRef: str(row.tenant_ref),
  slug: str(row.slug),
  name: str(row.name),
  description: nullableStr(row.description),
  isSystem: num(row.is_system) === 1,
  createdAt: str(row.created_at),
  updatedAt: str(row.updated_at),
})

const decodeActivity = (row: Record<string, unknown>): CrmActivity => ({
  id: str(row.id),
  tenantRef: str(row.tenant_ref),
  contactId: str(row.contact_id),
  accountId: nullableStr(row.account_id),
  activityType: str(row.activity_type),
  subject: nullableStr(row.subject),
  summary: nullableStr(row.summary),
  occurredAt: str(row.occurred_at),
  actorRef: nullableStr(row.actor_ref),
  sourceSystem: str(row.source_system),
  sourceRecordType: nullableStr(row.source_record_type),
  sourceRecordId: nullableStr(row.source_record_id),
  createdAt: str(row.created_at),
})

const decodeSnapshot = (
  row: Record<string, unknown>,
): CrmEngagementSnapshot => ({
  contactId: str(row.contact_id),
  tenantRef: str(row.tenant_ref),
  lastEmailSentAt: nullableStr(row.last_email_sent_at),
  lastEmailOpenedAt: nullableStr(row.last_email_opened_at),
  lastEmailClickedAt: nullableStr(row.last_email_clicked_at),
  lastEmailRepliedAt: nullableStr(row.last_email_replied_at),
  emailSentCount30d: num(row.email_sent_count_30d),
  emailOpenCount30d: num(row.email_open_count_30d),
  emailClickCount30d: num(row.email_click_count_30d),
  engagementScore: num(row.engagement_score),
  updatedAt: str(row.updated_at),
})

const decodeOpportunity = (row: Record<string, unknown>): CrmOpportunity => ({
  id: str(row.id),
  tenantRef: str(row.tenant_ref),
  accountId: nullableStr(row.account_id),
  name: str(row.name),
  roundName: nullableStr(row.round_name),
  stage: str(row.stage),
  status: str(row.status),
  targetAmountCents: nullableNum(row.target_amount_cents),
  expectedAmountCents: nullableNum(row.expected_amount_cents),
  convictionProbability: nullableNum(row.conviction_probability),
  targetCloseDate: nullableStr(row.target_close_date),
  summary: nullableStr(row.summary),
  metadataJson: str(row.metadata_json) === '' ? '{}' : str(row.metadata_json),
  createdAt: str(row.created_at),
  updatedAt: str(row.updated_at),
})

const decodeImportRun = (row: Record<string, unknown>): CrmSourceImportRun => ({
  id: str(row.id),
  tenantRef: str(row.tenant_ref),
  sourceLabel: str(row.source_label),
  status: str(row.status),
  totalRows: num(row.total_rows),
  importedRows: num(row.imported_rows),
  updatedRows: num(row.updated_rows),
  duplicateRows: num(row.duplicate_rows),
  failedRows: num(row.failed_rows),
  errorSummary: nullableStr(row.error_summary),
  createdAt: str(row.created_at),
  updatedAt: str(row.updated_at),
})

const runWrite = async (
  operation: string,
  fn: () => Promise<unknown>,
): Promise<void> => {
  try {
    await fn()
  } catch (error) {
    throw new CrmStorageError({ operation: `${operation}: ${String(error)}` })
  }
}

const queryAll = async <A>(
  operation: string,
  statement: D1PreparedStatement,
  decode: (row: Record<string, unknown>) => A,
): Promise<ReadonlyArray<A>> => {
  try {
    const result = await statement.all<Record<string, unknown>>()
    return (result.results ?? []).map(decode)
  } catch (error) {
    throw new CrmStorageError({ operation: `${operation}: ${String(error)}` })
  }
}

const queryFirst = async <A>(
  operation: string,
  statement: D1PreparedStatement,
  decode: (row: Record<string, unknown>) => A,
): Promise<A | null> => {
  try {
    const row = await statement.first<Record<string, unknown>>()
    return row === null ? null : decode(row)
  } catch (error) {
    throw new CrmStorageError({ operation: `${operation}: ${String(error)}` })
  }
}

// ---------------------------------------------------------------------------
// Contacts
// ---------------------------------------------------------------------------

export type UpsertCrmContactInput = Readonly<{
  tenantRef: string
  primaryEmail: string
  secondaryEmail?: string | null
  fullName?: string | null
  firstName?: string | null
  lastName?: string | null
  jobTitle?: string | null
  contactType?: string
  relationshipStage?: string
  lifecycleStage?: string
  accountId?: string | null
  portalAccessStatus?: string
  externalSourceLabel?: string | null
  externalSourceId?: string | null
  notes?: string | null
}>

export type UpsertCrmContactResult = Readonly<{
  contact: CrmContact
  created: boolean
}>

/**
 * Insert or update one contact, keyed on (tenantRef, normalized email). Used by
 * the CSV importer (#5982) and operator/agent writers. Returns whether a new
 * row was created so callers can keep honest imported-vs-updated counts.
 */
export const upsertCrmContact = async (
  db: CrmEmailDatabase,
  input: UpsertCrmContactInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<UpsertCrmContactResult> => {
  const email = normalizeCrmEmail(input.primaryEmail)
  const now = runtime.nowIso()

  const existing = await queryFirst(
    'crm.upsertContact.find',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_contacts WHERE tenant_ref = ? AND primary_email = ? LIMIT 1',
      )
      .bind(input.tenantRef, email),
    decodeContact,
  )

  if (existing !== null) {
    await runWrite('crm.upsertContact.update', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `UPDATE crm_contacts SET
             secondary_email = COALESCE(?, secondary_email),
             full_name = COALESCE(?, full_name),
             first_name = COALESCE(?, first_name),
             last_name = COALESCE(?, last_name),
             job_title = COALESCE(?, job_title),
             contact_type = COALESCE(?, contact_type),
             relationship_stage = COALESCE(?, relationship_stage),
             lifecycle_stage = COALESCE(?, lifecycle_stage),
             account_id = COALESCE(?, account_id),
             portal_access_status = COALESCE(?, portal_access_status),
             external_source_label = COALESCE(?, external_source_label),
             external_source_id = COALESCE(?, external_source_id),
             notes = COALESCE(?, notes),
             updated_at = ?
           WHERE id = ?`,
        )
        .bind(
          input.secondaryEmail ?? null,
          input.fullName ?? null,
          input.firstName ?? null,
          input.lastName ?? null,
          input.jobTitle ?? null,
          input.contactType ?? null,
          input.relationshipStage ?? null,
          input.lifecycleStage ?? null,
          input.accountId ?? null,
          input.portalAccessStatus ?? null,
          input.externalSourceLabel ?? null,
          input.externalSourceId ?? null,
          input.notes ?? null,
          now,
          existing.id,
        )
        .run(),
    )

    await mirrorCrmEmailRows(db, 'crm_contacts', 'id', [existing.id])

    const updated = await queryFirst(
      'crm.upsertContact.reread',
      crmEmailAuthorityDb(db)
        .prepare('SELECT * FROM crm_contacts WHERE id = ? LIMIT 1')
        .bind(existing.id),
      decodeContact,
    )
    return { contact: updated ?? existing, created: false }
  }

  const id = runtime.makeId('crm_contact')
  await runWrite('crm.upsertContact.insert', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_contacts (
           id, tenant_ref, primary_email, secondary_email, full_name,
           first_name, last_name, job_title, contact_type, relationship_stage,
           lifecycle_stage, account_id, portal_access_status, engagement_score,
           external_source_label, external_source_id, notes,
           metadata_json, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, '{}', ?, ?)`,
      )
      .bind(
        id,
        input.tenantRef,
        email,
        input.secondaryEmail ?? null,
        input.fullName ?? null,
        input.firstName ?? null,
        input.lastName ?? null,
        input.jobTitle ?? null,
        input.contactType ?? 'prospect',
        input.relationshipStage ?? 'new',
        input.lifecycleStage ?? 'lead',
        input.accountId ?? null,
        input.portalAccessStatus ?? 'none',
        input.externalSourceLabel ?? null,
        input.externalSourceId ?? null,
        input.notes ?? null,
        now,
        now,
      )
      .run(),
  )

  await mirrorCrmEmailRows(db, 'crm_contacts', 'id', [id])

  const created = await queryFirst(
    'crm.upsertContact.created',
    crmEmailAuthorityDb(db)
      .prepare('SELECT * FROM crm_contacts WHERE id = ? LIMIT 1')
      .bind(id),
    decodeContact,
  )
  if (created === null) {
    throw new CrmStorageError({
      operation: 'crm.upsertContact: row vanished after insert',
    })
  }
  return { contact: created, created: true }
}

export const getCrmContactById = (
  db: CrmEmailDatabase,
  tenantRef: string,
  id: string,
): Promise<CrmContact | null> =>
  queryFirst(
    'crm.getContact',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_contacts WHERE tenant_ref = ? AND id = ? AND archived_at IS NULL LIMIT 1',
      )
      .bind(tenantRef, id),
    decodeContact,
  )

export type ListCrmContactsQuery = Readonly<{
  limit?: number | undefined
  search?: string | null | undefined
}>

const clampLimit = (
  limit: number | undefined,
  fallback: number,
  max: number,
): number => {
  if (limit === undefined || !Number.isFinite(limit) || limit <= 0)
    return fallback
  return Math.min(Math.floor(limit), max)
}

export const listCrmContacts = (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: ListCrmContactsQuery = {},
): Promise<ReadonlyArray<CrmContact>> => {
  const limit = clampLimit(query.limit, 100, 500)
  const search = (query.search ?? '').trim().toLowerCase()

  if (search === '') {
    return queryAll(
      'crm.listContacts',
      crmEmailAuthorityDb(db)
        .prepare(
          'SELECT * FROM crm_contacts WHERE tenant_ref = ? AND archived_at IS NULL ORDER BY created_at DESC LIMIT ?',
        )
        .bind(tenantRef, limit),
      decodeContact,
    )
  }

  const like = `%${search}%`
  return queryAll(
    'crm.listContacts.search',
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT * FROM crm_contacts
           WHERE tenant_ref = ? AND archived_at IS NULL
             AND (LOWER(primary_email) LIKE ? OR LOWER(COALESCE(full_name, '')) LIKE ?)
           ORDER BY created_at DESC LIMIT ?`,
      )
      .bind(tenantRef, like, like, limit),
    decodeContact,
  )
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

export const getCrmAccountById = (
  db: CrmEmailDatabase,
  tenantRef: string,
  id: string,
): Promise<CrmAccount | null> =>
  queryFirst(
    'crm.getAccount',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_accounts WHERE tenant_ref = ? AND id = ? AND archived_at IS NULL LIMIT 1',
      )
      .bind(tenantRef, id),
    decodeAccount,
  )

export const listCrmAccounts = (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: Readonly<{ limit?: number | undefined }> = {},
): Promise<ReadonlyArray<CrmAccount>> =>
  queryAll(
    'crm.listAccounts',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_accounts WHERE tenant_ref = ? AND archived_at IS NULL ORDER BY created_at DESC LIMIT ?',
      )
      .bind(tenantRef, clampLimit(query.limit, 100, 500)),
    decodeAccount,
  )

export type UpsertCrmAccountInput = Readonly<{
  tenantRef: string
  name: string
  domain?: string | null
  accountType?: string
  websiteUrl?: string | null
  notes?: string | null
}>

export const upsertCrmAccount = async (
  db: CrmEmailDatabase,
  input: UpsertCrmAccountInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmAccount> => {
  const now = runtime.nowIso()
  const existing = await queryFirst(
    'crm.upsertAccount.find',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_accounts WHERE tenant_ref = ? AND name = ? AND archived_at IS NULL LIMIT 1',
      )
      .bind(input.tenantRef, input.name),
    decodeAccount,
  )
  if (existing !== null) {
    return existing
  }
  const id = runtime.makeId('crm_account')
  await runWrite('crm.upsertAccount.insert', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_accounts (
           id, tenant_ref, name, domain, account_type, status, website_url,
           notes, metadata_json, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, 'active', ?, ?, '{}', ?, ?)`,
      )
      .bind(
        id,
        input.tenantRef,
        input.name,
        input.domain ?? null,
        input.accountType ?? 'company',
        input.websiteUrl ?? null,
        input.notes ?? null,
        now,
        now,
      )
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_accounts', 'id', [id])

  const created = await queryFirst(
    'crm.upsertAccount.created',
    crmEmailAuthorityDb(db)
      .prepare('SELECT * FROM crm_accounts WHERE id = ? LIMIT 1')
      .bind(id),
    decodeAccount,
  )
  if (created === null) {
    throw new CrmStorageError({
      operation: 'crm.upsertAccount: row vanished after insert',
    })
  }
  return created
}

// ---------------------------------------------------------------------------
// Contact lists
// ---------------------------------------------------------------------------

export const listCrmContactLists = (
  db: CrmEmailDatabase,
  tenantRef: string,
): Promise<ReadonlyArray<CrmContactList>> =>
  queryAll(
    'crm.listContactLists',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_contact_lists WHERE tenant_ref = ? AND archived_at IS NULL ORDER BY created_at DESC',
      )
      .bind(tenantRef),
    decodeList,
  )

export const upsertCrmContactList = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    slug: string
    name: string
    description?: string | null
    isSystem?: boolean
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmContactList> => {
  const now = runtime.nowIso()
  const existing = await queryFirst(
    'crm.upsertList.find',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_contact_lists WHERE tenant_ref = ? AND slug = ? LIMIT 1',
      )
      .bind(input.tenantRef, input.slug),
    decodeList,
  )
  if (existing !== null) {
    return existing
  }
  const id = runtime.makeId('crm_list')
  await runWrite('crm.upsertList.insert', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_contact_lists (
           id, tenant_ref, slug, name, description, is_system, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        id,
        input.tenantRef,
        input.slug,
        input.name,
        input.description ?? null,
        input.isSystem === true ? 1 : 0,
        now,
        now,
      )
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_contact_lists', 'id', [id])

  const created = await queryFirst(
    'crm.upsertList.created',
    crmEmailAuthorityDb(db)
      .prepare('SELECT * FROM crm_contact_lists WHERE id = ? LIMIT 1')
      .bind(id),
    decodeList,
  )
  if (created === null) {
    throw new CrmStorageError({
      operation: 'crm.upsertList: row vanished after insert',
    })
  }
  return created
}

export const addCrmContactListMembership = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    contactId: string
    listId: string
    source?: string
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<void> => {
  const now = runtime.nowIso()
  await runWrite('crm.addListMembership', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_contact_list_memberships (
           id, tenant_ref, contact_id, list_id, membership_status, source, created_at, updated_at
         ) VALUES (?, ?, ?, ?, 'active', ?, ?, ?)
         ON CONFLICT(contact_id, list_id) DO UPDATE SET
           membership_status = 'active', source = excluded.source, updated_at = excluded.updated_at`,
      )
      .bind(
        runtime.makeId('crm_membership'),
        input.tenantRef,
        input.contactId,
        input.listId,
        input.source ?? 'manual',
        now,
        now,
      )
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_contact_list_memberships', 'contact_id', [
    input.contactId,
  ])
}

// ---------------------------------------------------------------------------
// Activities
// ---------------------------------------------------------------------------

export type RecordCrmActivityInput = Readonly<{
  tenantRef: string
  contactId: string
  accountId?: string | null
  activityType: string
  subject?: string | null
  summary?: string | null
  occurredAt?: string | null
  actorRef?: string | null
  sourceSystem?: string
  sourceRecordType?: string | null
  sourceRecordId?: string | null
}>

/**
 * Record one activity. When a (sourceRecordType, sourceRecordId) pair is given,
 * the insert is idempotent (`INSERT OR IGNORE`) so replayed provider events /
 * backfills never double-log.
 */
export const recordCrmActivity = async (
  db: CrmEmailDatabase,
  input: RecordCrmActivityInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<void> => {
  const now = runtime.nowIso()
  const id = runtime.makeId('crm_activity')
  await runWrite('crm.recordActivity', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT OR IGNORE INTO crm_activities (
           id, tenant_ref, contact_id, account_id, activity_type, subject,
           summary, occurred_at, actor_ref, source_system, source_record_type,
           source_record_id, metadata_json, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '{}', ?, ?)`,
      )
      .bind(
        id,
        input.tenantRef,
        input.contactId,
        input.accountId ?? null,
        input.activityType,
        input.subject ?? null,
        input.summary ?? null,
        input.occurredAt ?? now,
        input.actorRef ?? null,
        input.sourceSystem ?? 'crm',
        input.sourceRecordType ?? null,
        input.sourceRecordId ?? null,
        now,
        now,
      )
      .run(),
  )
  // No-op when the INSERT OR IGNORE deduped (no row with this fresh id):
  // the surviving row was mirrored when it was first recorded.
  await mirrorCrmEmailRows(db, 'crm_activities', 'id', [id])
}

export const listCrmActivitiesForContact = (
  db: CrmEmailDatabase,
  tenantRef: string,
  contactId: string,
  query: Readonly<{ limit?: number | undefined }> = {},
): Promise<ReadonlyArray<CrmActivity>> =>
  queryAll(
    'crm.listActivities',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_activities WHERE tenant_ref = ? AND contact_id = ? ORDER BY occurred_at DESC LIMIT ?',
      )
      .bind(tenantRef, contactId, clampLimit(query.limit, 100, 500)),
    decodeActivity,
  )

// ---------------------------------------------------------------------------
// Engagement snapshots
// ---------------------------------------------------------------------------

export const getCrmEngagementSnapshot = (
  db: CrmEmailDatabase,
  tenantRef: string,
  contactId: string,
): Promise<CrmEngagementSnapshot | null> =>
  queryFirst(
    'crm.getEngagementSnapshot',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_engagement_snapshots WHERE tenant_ref = ? AND contact_id = ? LIMIT 1',
      )
      .bind(tenantRef, contactId),
    decodeSnapshot,
  )

// ---------------------------------------------------------------------------
// Opportunities
// ---------------------------------------------------------------------------

export const listCrmOpportunities = (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: Readonly<{ limit?: number | undefined }> = {},
): Promise<ReadonlyArray<CrmOpportunity>> =>
  queryAll(
    'crm.listOpportunities',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_opportunities WHERE tenant_ref = ? AND archived_at IS NULL ORDER BY created_at DESC LIMIT ?',
      )
      .bind(tenantRef, clampLimit(query.limit, 100, 500)),
    decodeOpportunity,
  )

export const getCrmOpportunityById = (
  db: CrmEmailDatabase,
  tenantRef: string,
  id: string,
): Promise<CrmOpportunity | null> =>
  queryFirst(
    'crm.getOpportunity',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_opportunities WHERE tenant_ref = ? AND id = ? AND archived_at IS NULL LIMIT 1',
      )
      .bind(tenantRef, id),
    decodeOpportunity,
  )

// OB-5 (#8562): the reply -> conversation -> checkout -> settled-receipt
// pipeline stage vocabulary tracked on `crm_opportunities.stage`. The column
// has no CHECK constraint (free TEXT), so this is an application-level
// contract, not a schema one — `assertCrmSalesStage` below enforces it at the
// write boundary.
export const CRM_SALES_OPPORTUNITY_STAGES = [
  'sourced',
  'replied',
  'conversed',
  'quoted',
  'closed_won',
  'closed_lost',
] as const
export type CrmSalesOpportunityStage =
  (typeof CRM_SALES_OPPORTUNITY_STAGES)[number]

export const isCrmSalesOpportunityStage = (
  value: string,
): value is CrmSalesOpportunityStage =>
  (CRM_SALES_OPPORTUNITY_STAGES as ReadonlyArray<string>).includes(value)

const assertCrmSalesStage = (stage: string): CrmSalesOpportunityStage => {
  if (!isCrmSalesOpportunityStage(stage)) {
    throw new CrmStorageError({
      operation: `crm.opportunity: invalid sales stage "${stage}"`,
    })
  }
  return stage
}

export type CreateCrmOpportunityInput = Readonly<{
  tenantRef: string
  name: string
  accountId?: string | null
  stage?: CrmSalesOpportunityStage
  targetAmountCents?: number | null
  expectedAmountCents?: number | null
  summary?: string | null
  /** Opaque bag merged into `metadata_json` — OB-5 carries `{ sourceRef,
   * pipelineRef }` here so the LG-6 attribution chain dereferences from the
   * opportunity back to the segment/pipeline row that sourced it. */
  metadata?: Readonly<Record<string, unknown>>
}>

/** Create a fresh sales opportunity. Unlike `upsertCrmContact`, this always
 * inserts — callers that want find-or-create-by-name semantics should query
 * first (opportunities are per-deal, not per-entity, so there is no natural
 * dedupe key). */
export const createCrmOpportunity = async (
  db: CrmEmailDatabase,
  input: CreateCrmOpportunityInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmOpportunity> => {
  const now = runtime.nowIso()
  const id = runtime.makeId('crm_opportunity')
  const stage = assertCrmSalesStage(input.stage ?? 'sourced')
  const metadataJson = JSON.stringify(input.metadata ?? {})

  await runWrite('crm.createOpportunity', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_opportunities (
           id, tenant_ref, account_id, name, round_name, stage, status,
           target_amount_cents, expected_amount_cents, conviction_probability,
           target_close_date, summary, metadata_json, created_at, updated_at
         ) VALUES (?, ?, ?, ?, NULL, ?, 'open', ?, ?, NULL, NULL, ?, ?, ?, ?)`,
      )
      .bind(
        id,
        input.tenantRef,
        input.accountId ?? null,
        input.name,
        stage,
        input.targetAmountCents ?? null,
        input.expectedAmountCents ?? null,
        input.summary ?? null,
        metadataJson,
        now,
        now,
      )
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_opportunities', 'id', [id])

  const created = await getCrmOpportunityById(db, input.tenantRef, id)
  if (created === null) {
    throw new CrmStorageError({
      operation: 'crm.createOpportunity: row vanished after insert',
    })
  }
  return created
}

export type UpdateCrmOpportunityStageInput = Readonly<{
  tenantRef: string
  id: string
  stage: CrmSalesOpportunityStage
  /** Merged (shallow) into the existing `metadata_json` bag. */
  metadata?: Readonly<Record<string, unknown>>
}>

const CRM_SALES_STAGE_TERMINAL_STATUS: Readonly<
  Partial<Record<CrmSalesOpportunityStage, 'won' | 'lost'>>
> = {
  closed_won: 'won',
  closed_lost: 'lost',
}

/** Advance (or set) a sales opportunity's OB-5 stage. `status` mirrors
 * `closed_won`/`closed_lost` into the existing `open`/`won`/`lost` column so
 * older readers of `crm_opportunities.status` keep working unchanged. */
export const updateCrmOpportunityStage = async (
  db: CrmEmailDatabase,
  input: UpdateCrmOpportunityStageInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmOpportunity> => {
  const stage = assertCrmSalesStage(input.stage)
  const existing = await getCrmOpportunityById(db, input.tenantRef, input.id)
  if (existing === null) {
    throw new CrmStorageError({
      operation: `crm.updateOpportunityStage: not found ${input.id}`,
    })
  }

  const now = runtime.nowIso()
  const status = CRM_SALES_STAGE_TERMINAL_STATUS[stage] ?? 'open'
  const mergedMetadata =
    input.metadata === undefined
      ? existing.metadataJson
      : JSON.stringify({
          ...(JSON.parse(existing.metadataJson) as Record<string, unknown>),
          ...input.metadata,
        })

  await runWrite('crm.updateOpportunityStage', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE crm_opportunities
            SET stage = ?, status = ?, metadata_json = ?, updated_at = ?
          WHERE id = ? AND tenant_ref = ?`,
      )
      .bind(stage, status, mergedMetadata, now, input.id, input.tenantRef)
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_opportunities', 'id', [input.id])

  const updated = await getCrmOpportunityById(db, input.tenantRef, input.id)
  if (updated === null) {
    throw new CrmStorageError({
      operation: 'crm.updateOpportunityStage: row vanished after update',
    })
  }
  return updated
}

/** Idempotently attach a contact to an opportunity (e.g. the prospect who
 * replied and is now the opportunity's primary contact). */
export const upsertCrmOpportunityContactRole = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    opportunityId: string
    contactId: string
    roleType?: string
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<void> => {
  const now = runtime.nowIso()
  await runWrite('crm.upsertOpportunityContactRole', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_opportunity_contact_roles (
           id, tenant_ref, opportunity_id, contact_id, role_type, status,
           notes, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, 'active', NULL, ?, ?)
         ON CONFLICT(opportunity_id, contact_id) DO UPDATE SET
           role_type = excluded.role_type, status = 'active', updated_at = excluded.updated_at`,
      )
      .bind(
        runtime.makeId('crm_opp_role'),
        input.tenantRef,
        input.opportunityId,
        input.contactId,
        input.roleType ?? 'primary',
        now,
        now,
      )
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_opportunity_contact_roles', 'opportunity_id', [
    input.opportunityId,
  ])
}

// ---------------------------------------------------------------------------
// Import-run audit
// ---------------------------------------------------------------------------

export const startCrmSourceImportRun = async (
  db: CrmEmailDatabase,
  input: Readonly<{ tenantRef: string; sourceLabel: string }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<string> => {
  const id = runtime.makeId('crm_import')
  const now = runtime.nowIso()
  await runWrite('crm.startImportRun', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_source_import_runs (
           id, tenant_ref, source_label, status, created_at, updated_at
         ) VALUES (?, ?, ?, 'running', ?, ?)`,
      )
      .bind(id, input.tenantRef, input.sourceLabel, now, now)
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_source_import_runs', 'id', [id])
  return id
}

export const completeCrmSourceImportRun = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    id: string
    status: 'completed' | 'failed'
    totalRows: number
    importedRows: number
    updatedRows: number
    duplicateRows: number
    failedRows: number
    errorSummary?: string | null
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<void> => {
  await runWrite('crm.completeImportRun', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE crm_source_import_runs SET
           status = ?, total_rows = ?, imported_rows = ?, updated_rows = ?,
           duplicate_rows = ?, failed_rows = ?, error_summary = ?, updated_at = ?
         WHERE id = ?`,
      )
      .bind(
        input.status,
        input.totalRows,
        input.importedRows,
        input.updatedRows,
        input.duplicateRows,
        input.failedRows,
        input.errorSummary ?? null,
        runtime.nowIso(),
        input.id,
      )
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_source_import_runs', 'id', [input.id])
}

export const getCrmSourceImportRun = (
  db: CrmEmailDatabase,
  tenantRef: string,
  id: string,
): Promise<CrmSourceImportRun | null> =>
  queryFirst(
    'crm.getImportRun',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_source_import_runs WHERE tenant_ref = ? AND id = ? LIMIT 1',
      )
      .bind(tenantRef, id),
    decodeImportRun,
  )

export const listCrmSourceImportRuns = (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: Readonly<{ limit?: number | undefined }> = {},
): Promise<ReadonlyArray<CrmSourceImportRun>> =>
  queryAll(
    'crm.listImportRuns',
    crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_source_import_runs WHERE tenant_ref = ? ORDER BY created_at DESC LIMIT ?',
      )
      .bind(tenantRef, clampLimit(query.limit, 50, 200)),
    decodeImportRun,
  )
