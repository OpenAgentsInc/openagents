import {
  type EmailCampaignRuntime,
  type EmailSuppressionReason,
  addEmailSuppression,
  systemEmailCampaignRuntime,
} from './email-campaigns'

export type EmailPolicyCategory = 'drip' | 'marketing' | 'transactional'

export type EmailPreferenceState = Readonly<{
  dripOptIn: boolean
  marketingOptIn: boolean
  transactionalOptIn: boolean
}>

export type EmailSendEligibility = Readonly<{
  allowed: boolean
  reason:
    | 'allowed'
    | 'all_suppressed'
    | 'drip_opted_out'
    | 'drip_suppressed'
    | 'marketing_opted_out'
    | 'marketing_suppressed'
    | 'transactional_opted_out'
}>

type PreferenceStateRow = Readonly<{
  drip_opt_in: number
  marketing_opt_in: number
  transactional_opt_in: number
}>

type SuppressionLookupRow = Readonly<{ id: string; scope: string }>

const normalizeEmail = (email: string): string =>
  email.trim().toLowerCase().replace(/\s+/g, '').slice(0, 320)

const sourceAuthority = (sourceAuthorityRef: string): string =>
  sourceAuthorityRef.trim().replace(/\s+/g, ' ').slice(0, 240)

export const defaultEmailPreferenceState = (): EmailPreferenceState => ({
  dripOptIn: true,
  marketingOptIn: true,
  transactionalOptIn: true,
})

export const readEmailPreferenceState = async (
  db: D1Database,
  email: string,
): Promise<EmailPreferenceState> => {
  const row = await db
    .prepare(
      `SELECT marketing_opt_in, drip_opt_in, transactional_opt_in
         FROM email_preferences
        WHERE email = ?
        LIMIT 1`,
    )
    .bind(normalizeEmail(email))
    .first<PreferenceStateRow>()

  return row === null
    ? defaultEmailPreferenceState()
    : {
        dripOptIn: row.drip_opt_in === 1,
        marketingOptIn: row.marketing_opt_in === 1,
        transactionalOptIn: row.transactional_opt_in === 1,
      }
}

export const upsertEmailPreferenceCategory = async (
  db: D1Database,
  input: Readonly<{
    category: EmailPolicyCategory
    email: string
    optIn: boolean
    sourceAuthorityRef: string
    updatedByUserId?: string | null | undefined
    userId?: string | null | undefined
  }>,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<void> => {
  const now = runtime.nowIso()
  const current = await readEmailPreferenceState(db, input.email)
  const next: EmailPreferenceState = {
    dripOptIn: input.category === 'drip' ? input.optIn : current.dripOptIn,
    marketingOptIn:
      input.category === 'marketing' ? input.optIn : current.marketingOptIn,
    transactionalOptIn:
      input.category === 'transactional'
        ? input.optIn
        : current.transactionalOptIn,
  }

  await db
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
      next.marketingOptIn ? 1 : 0,
      next.dripOptIn ? 1 : 0,
      next.transactionalOptIn ? 1 : 0,
      sourceAuthority(input.sourceAuthorityRef),
      input.updatedByUserId ?? null,
      now,
      now,
    )
    .run()
}

const readSuppression = async (
  db: D1Database,
  email: string,
  category: EmailPolicyCategory,
): Promise<'all' | 'category' | 'none'> => {
  const row = await db
    .prepare(
      `SELECT id, scope
         FROM email_suppression_entries
        WHERE email = ?
          AND active = 1
          AND archived_at IS NULL
          AND scope IN (?, 'all')
        LIMIT 1`,
    )
    .bind(
      normalizeEmail(email),
      category === 'transactional' ? 'all' : category,
    )
    .first<SuppressionLookupRow>()

  if (row === null) {
    return 'none'
  }

  return row.scope === 'all' ? 'all' : 'category'
}

export const readEmailSendEligibility = async (
  db: D1Database,
  input: Readonly<{
    category: EmailPolicyCategory
    email: string
  }>,
): Promise<EmailSendEligibility> => {
  const suppression = await readSuppression(db, input.email, input.category)

  if (suppression === 'all') {
    return { allowed: false, reason: 'all_suppressed' }
  }

  if (suppression === 'category') {
    return {
      allowed: false,
      reason:
        input.category === 'drip' ? 'drip_suppressed' : 'marketing_suppressed',
    }
  }

  const preference = await readEmailPreferenceState(db, input.email)

  if (input.category === 'drip' && !preference.dripOptIn) {
    return { allowed: false, reason: 'drip_opted_out' }
  }

  if (input.category === 'marketing' && !preference.marketingOptIn) {
    return { allowed: false, reason: 'marketing_opted_out' }
  }

  if (input.category === 'transactional' && !preference.transactionalOptIn) {
    return { allowed: false, reason: 'transactional_opted_out' }
  }

  return { allowed: true, reason: 'allowed' }
}

export const recordEmailUnsubscribe = async (
  db: D1Database,
  input: Readonly<{
    category: Exclude<EmailPolicyCategory, 'transactional'>
    email: string
    sourceAuthorityRef: string
    userId?: string | null | undefined
  }>,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<void> => {
  await upsertEmailPreferenceCategory(
    db,
    {
      category: input.category,
      email: input.email,
      optIn: false,
      sourceAuthorityRef: input.sourceAuthorityRef,
      userId: input.userId ?? null,
    },
    runtime,
  )
}

export const recordProviderEmailSuppression = async (
  db: D1Database,
  input: Readonly<{
    email: string
    providerEventId?: string | null | undefined
    reason: Extract<EmailSuppressionReason, 'bounce' | 'complaint'>
    sourceAuthorityRef: string
  }>,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<void> => {
  await addEmailSuppression(
    db,
    {
      email: input.email,
      providerEventId: input.providerEventId ?? null,
      reason: input.reason,
      scope: 'all',
      sourceAuthorityRef: input.sourceAuthorityRef,
    },
    runtime,
  )
}
