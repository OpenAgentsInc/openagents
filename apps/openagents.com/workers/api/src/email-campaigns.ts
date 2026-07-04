import { Schema as S } from 'effect'

// KS-8.11 (#8322): CrmEmailDatabase union — campaign/enrollment/send/
// preference/suppression writes mirror to Postgres fail-soft, and the
// SUPPRESSION COMPLIANCE GATE reads (isEmailSuppressed /
// readEmailPreferenceAllows) route through the flag-gated seam so the send
// path consults exactly one authoritative store per read.
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  crmEmailRead,
  mirrorCrmEmailRows,
} from './crm-email-domain-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const EmailCampaignStatus = S.Literals([
  'draft',
  'active',
  'paused',
  'archived',
])
export type EmailCampaignStatus = typeof EmailCampaignStatus.Type

export const EmailCampaignEnrollmentStatus = S.Literals([
  'active',
  'completed',
  'suppressed',
  'canceled',
])
export type EmailCampaignEnrollmentStatus =
  typeof EmailCampaignEnrollmentStatus.Type

export const EmailCampaignSendStatus = S.Literals([
  'scheduled',
  'claimed',
  'sent',
  'skipped',
  'suppressed',
  'failed',
  'canceled',
])
export type EmailCampaignSendStatus = typeof EmailCampaignSendStatus.Type

export const EmailSuppressionReason = S.Literals([
  'unsubscribe',
  'bounce',
  'complaint',
  'operator',
  'manual',
])
export type EmailSuppressionReason = typeof EmailSuppressionReason.Type

export const EmailSuppressionScope = S.Literals(['marketing', 'drip', 'all'])
export type EmailSuppressionScope = typeof EmailSuppressionScope.Type

export type EmailCampaignRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemEmailCampaignRuntime: EmailCampaignRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

export type EmailCampaignRecord = Readonly<{
  audience: string
  id: string
  metadataJson: string
  name: string
  slug: string
  sourceAuthorityRef: string
  status: EmailCampaignStatus
}>

export type EmailCampaignStepRecord = Readonly<{
  campaignId: string
  delaySeconds: number
  id: string
  lifecycleKind: string | null
  metadataJson: string
  name: string
  status: EmailCampaignStatus
  stepKey: string
  templateSlug: string
}>

export type EmailCampaignEnrollmentRecord = Readonly<{
  campaignId: string
  email: string
  id: string
  idempotencyKey: string
  metadataJson: string
  sourceAuthorityRef: string
  status: EmailCampaignEnrollmentStatus
  userId: string | null
}>

export type EmailCampaignSendRecord = Readonly<{
  campaignId: string
  dueAt: string
  email: string
  enrollmentId: string
  id: string
  idempotencyKey: string
  metadataJson: string
  sourceAuthorityRef: string
  status: EmailCampaignSendStatus
  stepId: string
  userId: string | null
}>

type EmailCampaignRow = Readonly<{
  audience: string
  id: string
  metadata_json: string
  name: string
  slug: string
  source_authority_ref: string
  status: EmailCampaignStatus
}>

type EmailCampaignStepRow = Readonly<{
  campaign_id: string
  delay_seconds: number
  id: string
  lifecycle_kind: string | null
  metadata_json: string
  name: string
  status: EmailCampaignStatus
  step_key: string
  template_slug: string
}>

type EmailCampaignEnrollmentRow = Readonly<{
  campaign_id: string
  email: string
  id: string
  idempotency_key: string
  metadata_json: string
  source_authority_ref: string
  status: EmailCampaignEnrollmentStatus
  user_id: string | null
}>

export type EmailSuppressionInput = Readonly<{
  email: string
  note?: string | null | undefined
  providerEventId?: string | null | undefined
  reason: EmailSuppressionReason
  scope: EmailSuppressionScope
  sourceAuthorityRef: string
}>

export type EmailPreferenceInput = Readonly<{
  dripOptIn: boolean
  email: string
  marketingOptIn: boolean
  sourceAuthorityRef: string
  transactionalOptIn: boolean
  updatedByUserId?: string | null | undefined
  userId?: string | null | undefined
}>

const clampText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const normalizeEmail = (email: string): string =>
  clampText(email.toLowerCase(), 320)

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

export const makeEmailCampaignRecord = (
  input: Readonly<{
    audience: string
    metadata?: Record<string, string | number | boolean | null> | undefined
    name: string
    slug: string
    sourceAuthorityRef: string
    status?: EmailCampaignStatus | undefined
  }>,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): EmailCampaignRecord => ({
  audience: clampText(input.audience, 160),
  id: runtime.makeId('email_campaign'),
  metadataJson: metadataJson(input.metadata),
  name: clampText(input.name, 160),
  slug: clampText(input.slug, 120),
  sourceAuthorityRef: clampText(input.sourceAuthorityRef, 240),
  status: input.status ?? 'draft',
})

export const makeEmailCampaignStepRecord = (
  input: Readonly<{
    campaignId: string
    delaySeconds: number
    lifecycleKind?: string | null | undefined
    metadata?: Record<string, string | number | boolean | null> | undefined
    name: string
    status?: EmailCampaignStatus | undefined
    stepKey: string
    templateSlug: string
  }>,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): EmailCampaignStepRecord => ({
  campaignId: input.campaignId,
  delaySeconds: Math.max(0, Math.floor(input.delaySeconds)),
  id: runtime.makeId('email_campaign_step'),
  lifecycleKind:
    input.lifecycleKind === undefined || input.lifecycleKind === null
      ? null
      : clampText(input.lifecycleKind, 120),
  metadataJson: metadataJson(input.metadata),
  name: clampText(input.name, 160),
  status: input.status ?? 'draft',
  stepKey: clampText(input.stepKey, 120),
  templateSlug: clampText(input.templateSlug, 160),
})

export const makeEmailCampaignEnrollmentRecord = (
  input: Readonly<{
    campaignId: string
    email: string
    metadata?: Record<string, string | number | boolean | null> | undefined
    sourceAuthorityRef: string
    status?: EmailCampaignEnrollmentStatus | undefined
    userId?: string | null | undefined
  }>,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): EmailCampaignEnrollmentRecord => {
  const email = normalizeEmail(input.email)

  return {
    campaignId: input.campaignId,
    email,
    id: runtime.makeId('email_campaign_enrollment'),
    idempotencyKey: `email_campaign_enrollment:${input.campaignId}:${email}`,
    metadataJson: metadataJson(input.metadata),
    sourceAuthorityRef: clampText(input.sourceAuthorityRef, 240),
    status: input.status ?? 'active',
    userId: input.userId ?? null,
  }
}

export const makeEmailCampaignSendRecord = (
  input: Readonly<{
    campaignId: string
    dueAt: string
    email: string
    enrollmentId: string
    metadata?: Record<string, string | number | boolean | null> | undefined
    sourceAuthorityRef: string
    status?: EmailCampaignSendStatus | undefined
    stepId: string
    stepKey: string
    userId?: string | null | undefined
  }>,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): EmailCampaignSendRecord => {
  const email = normalizeEmail(input.email)

  return {
    campaignId: input.campaignId,
    dueAt: input.dueAt,
    email,
    enrollmentId: input.enrollmentId,
    id: runtime.makeId('email_campaign_send'),
    idempotencyKey: `email_campaign_send:${input.enrollmentId}:${input.stepKey}`,
    metadataJson: metadataJson(input.metadata),
    sourceAuthorityRef: clampText(input.sourceAuthorityRef, 240),
    status: input.status ?? 'scheduled',
    stepId: input.stepId,
    userId: input.userId ?? null,
  }
}

export const insertEmailCampaign = async (
  db: CrmEmailDatabase,
  campaign: EmailCampaignRecord,
  now: string,
): Promise<void> => {
  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO email_campaigns
        (id, slug, name, audience, status, source_authority_ref,
         metadata_json, created_at, updated_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
       ON CONFLICT(slug) DO UPDATE SET
         name = excluded.name,
         audience = excluded.audience,
         status = excluded.status,
         source_authority_ref = excluded.source_authority_ref,
         metadata_json = excluded.metadata_json,
         updated_at = excluded.updated_at`,
    )
    .bind(
      campaign.id,
      campaign.slug,
      campaign.name,
      campaign.audience,
      campaign.status,
      campaign.sourceAuthorityRef,
      campaign.metadataJson,
      now,
      now,
    )
    .run()
  await mirrorCrmEmailRows(db, 'email_campaigns', 'slug', [campaign.slug])
}

const campaignFromRow = (row: EmailCampaignRow): EmailCampaignRecord => ({
  audience: row.audience,
  id: row.id,
  metadataJson: row.metadata_json,
  name: row.name,
  slug: row.slug,
  sourceAuthorityRef: row.source_authority_ref,
  status: row.status,
})

export const readEmailCampaignBySlug = async (
  db: CrmEmailDatabase,
  slug: string,
): Promise<EmailCampaignRecord | null> => {
  const row = await crmEmailAuthorityDb(db)
    .prepare(
      `SELECT id, slug, name, audience, status, source_authority_ref,
              metadata_json
         FROM email_campaigns
        WHERE slug = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(clampText(slug, 120))
    .first<EmailCampaignRow>()

  return row === null ? null : campaignFromRow(row)
}

export const insertEmailCampaignStep = async (
  db: CrmEmailDatabase,
  step: EmailCampaignStepRecord,
  now: string,
): Promise<void> => {
  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO email_campaign_steps
        (id, campaign_id, step_key, name, delay_seconds, template_slug,
         lifecycle_kind, status, metadata_json, created_at, updated_at,
         archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
       ON CONFLICT(campaign_id, step_key) DO UPDATE SET
         name = excluded.name,
         delay_seconds = excluded.delay_seconds,
         template_slug = excluded.template_slug,
         lifecycle_kind = excluded.lifecycle_kind,
         status = excluded.status,
         metadata_json = excluded.metadata_json,
         updated_at = excluded.updated_at`,
    )
    .bind(
      step.id,
      step.campaignId,
      step.stepKey,
      step.name,
      step.delaySeconds,
      step.templateSlug,
      step.lifecycleKind,
      step.status,
      step.metadataJson,
      now,
      now,
    )
    .run()
  await mirrorCrmEmailRows(db, 'email_campaign_steps', 'campaign_id', [
    step.campaignId,
  ])
}

const campaignStepFromRow = (
  row: EmailCampaignStepRow,
): EmailCampaignStepRecord => ({
  campaignId: row.campaign_id,
  delaySeconds: row.delay_seconds,
  id: row.id,
  lifecycleKind: row.lifecycle_kind,
  metadataJson: row.metadata_json,
  name: row.name,
  status: row.status,
  stepKey: row.step_key,
  templateSlug: row.template_slug,
})

export const listEmailCampaignSteps = async (
  db: CrmEmailDatabase,
  campaignId: string,
): Promise<ReadonlyArray<EmailCampaignStepRecord>> => {
  const result = await crmEmailAuthorityDb(db)
    .prepare(
      `SELECT id, campaign_id, step_key, name, delay_seconds, template_slug,
              lifecycle_kind, status, metadata_json
         FROM email_campaign_steps
        WHERE campaign_id = ?
          AND archived_at IS NULL
        ORDER BY delay_seconds ASC, step_key ASC`,
    )
    .bind(campaignId)
    .all<EmailCampaignStepRow>()

  return result.results.map(campaignStepFromRow)
}

export const insertEmailCampaignEnrollment = async (
  db: CrmEmailDatabase,
  enrollment: EmailCampaignEnrollmentRecord,
  now: string,
): Promise<void> => {
  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO email_campaign_enrollments
        (id, campaign_id, user_id, email, status, idempotency_key,
         source_authority_ref, metadata_json, enrolled_at, completed_at,
         canceled_at, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
       ON CONFLICT(idempotency_key) DO NOTHING`,
    )
    .bind(
      enrollment.id,
      enrollment.campaignId,
      enrollment.userId,
      enrollment.email,
      enrollment.status,
      enrollment.idempotencyKey,
      enrollment.sourceAuthorityRef,
      enrollment.metadataJson,
      now,
      now,
      now,
    )
    .run()
  await mirrorCrmEmailRows(
    db,
    'email_campaign_enrollments',
    'idempotency_key',
    [enrollment.idempotencyKey],
  )
}

const campaignEnrollmentFromRow = (
  row: EmailCampaignEnrollmentRow,
): EmailCampaignEnrollmentRecord => ({
  campaignId: row.campaign_id,
  email: row.email,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadataJson: row.metadata_json,
  sourceAuthorityRef: row.source_authority_ref,
  status: row.status,
  userId: row.user_id,
})

export const readEmailCampaignEnrollmentByIdempotencyKey = async (
  db: CrmEmailDatabase,
  idempotencyKey: string,
): Promise<EmailCampaignEnrollmentRecord | null> => {
  const row = await crmEmailAuthorityDb(db)
    .prepare(
      `SELECT id, campaign_id, user_id, email, status, idempotency_key,
              source_authority_ref, metadata_json
         FROM email_campaign_enrollments
        WHERE idempotency_key = ?
        LIMIT 1`,
    )
    .bind(clampText(idempotencyKey, 240))
    .first<EmailCampaignEnrollmentRow>()

  return row === null ? null : campaignEnrollmentFromRow(row)
}

export const insertEmailCampaignSend = async (
  db: CrmEmailDatabase,
  send: EmailCampaignSendRecord,
  now: string,
): Promise<void> => {
  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO email_campaign_sends
        (id, campaign_id, step_id, enrollment_id, user_id, email, due_at,
         status, idempotency_key, source_authority_ref, email_message_id,
         provider_event_id, error_name, error_message, metadata_json,
         claimed_at, sent_at, skipped_at, failed_at, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?,
         NULL, NULL, NULL, NULL, ?, ?)
       ON CONFLICT(idempotency_key) DO NOTHING`,
    )
    .bind(
      send.id,
      send.campaignId,
      send.stepId,
      send.enrollmentId,
      send.userId,
      send.email,
      send.dueAt,
      send.status,
      send.idempotencyKey,
      send.sourceAuthorityRef,
      send.metadataJson,
      now,
      now,
    )
    .run()
  await mirrorCrmEmailRows(db, 'email_campaign_sends', 'idempotency_key', [
    send.idempotencyKey,
  ])
}

export const upsertEmailPreference = async (
  db: CrmEmailDatabase,
  input: EmailPreferenceInput,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<void> => {
  const now = runtime.nowIso()

  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO email_preferences
        (id, user_id, email, marketing_opt_in, drip_opt_in,
         transactional_opt_in, source_authority_ref, updated_by_user_id,
         created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(email) DO UPDATE SET
         user_id = excluded.user_id,
         marketing_opt_in = excluded.marketing_opt_in,
         drip_opt_in = excluded.drip_opt_in,
         transactional_opt_in = excluded.transactional_opt_in,
         source_authority_ref = excluded.source_authority_ref,
         updated_by_user_id = excluded.updated_by_user_id,
         updated_at = excluded.updated_at`,
    )
    .bind(
      runtime.makeId('email_preference'),
      input.userId ?? null,
      normalizeEmail(input.email),
      input.marketingOptIn ? 1 : 0,
      input.dripOptIn ? 1 : 0,
      input.transactionalOptIn ? 1 : 0,
      clampText(input.sourceAuthorityRef, 240),
      input.updatedByUserId ?? null,
      now,
      now,
    )
    .run()
  await mirrorCrmEmailRows(db, 'email_preferences', 'email', [
    normalizeEmail(input.email),
  ])
}

export const addEmailSuppression = async (
  db: CrmEmailDatabase,
  input: EmailSuppressionInput,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<void> => {
  const now = runtime.nowIso()
  const id = runtime.makeId('email_suppression')

  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO email_suppression_entries
        (id, email, reason, scope, active, source_authority_ref,
         provider_event_id, note, created_at, updated_at, archived_at)
       VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      normalizeEmail(input.email),
      input.reason,
      input.scope,
      clampText(input.sourceAuthorityRef, 240),
      input.providerEventId ?? null,
      input.note === undefined || input.note === null
        ? null
        : clampText(input.note, 240),
      now,
      now,
    )
    .run()
  await mirrorCrmEmailRows(db, 'email_suppression_entries', 'id', [id])
}

type SuppressionLookupRow = Readonly<{ id: string }>
type PreferenceLookupRow = Readonly<{
  drip_opt_in: number
  marketing_opt_in: number
  transactional_opt_in: number
}>

export const isEmailSuppressed = async (
  db: CrmEmailDatabase,
  email: string,
  scope: EmailSuppressionScope,
): Promise<boolean> => {
  const normalized = normalizeEmail(email)

  // COMPLIANCE GATE (KS-8.11): flag-routed so the send path reads exactly
  // one authoritative suppression store per read. Both implementations
  // answer the SAME question: any active, non-archived entry whose scope is
  // this scope or 'all'.
  return crmEmailRead(
    db,
    'email_suppression_entries.isSuppressed',
    [normalized],
    async () => {
      const row = await crmEmailAuthorityDb(db)
        .prepare(
          `SELECT id
             FROM email_suppression_entries
            WHERE email = ?
              AND active = 1
              AND archived_at IS NULL
              AND scope IN (?, 'all')
            LIMIT 1`,
        )
        .bind(normalized, scope)
        .first<SuppressionLookupRow>()

      return row !== null
    },
    async postgres => {
      const rows = await postgres.selectRowsByKey(
        'email_suppression_entries',
        'email',
        [normalized],
      )

      return rows.some(
        row =>
          Number(row.active) === 1 &&
          (row.archived_at === null || row.archived_at === undefined) &&
          (row.scope === scope || row.scope === 'all'),
      )
    },
  )
}

const preferenceAllowsScope = (
  row: Readonly<{
    drip_opt_in: number
    marketing_opt_in: number
  }>,
  scope: EmailSuppressionScope,
): boolean => {
  switch (scope) {
    case 'marketing':
      return row.marketing_opt_in === 1
    case 'drip':
      return row.drip_opt_in === 1
    case 'all':
      return row.marketing_opt_in === 1 && row.drip_opt_in === 1
  }
}

export const readEmailPreferenceAllows = async (
  db: CrmEmailDatabase,
  email: string,
  scope: EmailSuppressionScope,
): Promise<boolean> => {
  const normalized = normalizeEmail(email)

  // COMPLIANCE GATE (KS-8.11): flag-routed alongside isEmailSuppressed.
  return crmEmailRead(
    db,
    'email_preferences.allows',
    [normalized],
    async () => {
      const row = await crmEmailAuthorityDb(db)
        .prepare(
          `SELECT marketing_opt_in, drip_opt_in, transactional_opt_in
             FROM email_preferences
            WHERE email = ?
            LIMIT 1`,
        )
        .bind(normalized)
        .first<PreferenceLookupRow>()

      return row === null ? true : preferenceAllowsScope(row, scope)
    },
    async postgres => {
      const rows = await postgres.selectRowsByKey(
        'email_preferences',
        'email',
        [normalized],
      )
      const row = rows[0]

      return row === undefined
        ? true
        : preferenceAllowsScope(
            {
              drip_opt_in: Number(row.drip_opt_in),
              marketing_opt_in: Number(row.marketing_opt_in),
            },
            scope,
          )
    },
  )
}
