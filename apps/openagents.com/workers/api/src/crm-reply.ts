/**
 * OB-4 (#8561): Sarah reply routing — CRM-side plumbing.
 *
 * The spec (docs/fable/2026-07-07-sarah-sales-agent-spec.md, MASTER_ROADMAP
 * P1 Track C) names the Sarah repo's own email channel (S-8,
 * `OpenAgentsInc/sarah`) as the long-term producer of inbound reply events:
 * "Reply-To routes to Sarah's inbox ... if S-8 hasn't landed, v0 = inbound
 * webhook -> operator notification + CRM activity, upgraded when S-8 ships."
 *
 * S-8 lives in a separate private repo and was not reachable from this
 * change, so this module ships the v0 fallback + the CRM-side contract a
 * future S-8 integration (or a Resend/other inbound-email webhook) can call
 * unchanged: `recordCrmReplyEvent`. Every reply becomes both a
 * `crm_reply_events` row (this migration) and a `crm_activities` row (the
 * existing CRM activity ledger). Opt-out replies auto-suppress through the
 * existing suppression machinery (email-preferences.ts / email-campaigns.ts)
 * — the SAME gate `readEmailSendEligibility` checks before every future send,
 * so an opt-out here holds end to end without any new enforcement path.
 *
 * This module sends nothing and approves nothing — it only records inbound
 * events and (for opt-outs) writes suppression state. It does not touch the
 * approval-gated send authority in crm-command.ts / crm-send.ts.
 */
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
} from './crm-email-domain-store'
import { addEmailSuppression } from './email-campaigns'
import { recordEmailUnsubscribe } from './email-preferences'
import {
  type CrmRuntime,
  defaultCrmRuntime,
  normalizeCrmEmail,
  recordCrmActivity,
} from './crm-store'

/**
 * Bump to true once the Sarah repo's S-8 inbound email channel is wired to
 * call `recordCrmReplyEvent` directly (or an equivalent webhook is verified
 * as Sarah's own inbox). Until then every reply routes to the v0 fallback:
 * an operator-visible CRM activity + reply-event row, no live inbox handoff.
 */
export const SARAH_EMAIL_CHANNEL_LIVE = false

const OPT_OUT_PATTERN =
  /\b(unsubscribe|opt[\s-]?out|stop\s+email(?:ing|s)?|remove\s+me|take\s+me\s+off|do\s+not\s+contact|no\s+more\s+emails|please\s+stop)\b/i

/** Pure heuristic — no network/DB — kept separate so it is trivially unit-testable. */
export const detectCrmReplyOptOut = (
  input: Readonly<{ subject?: string | null | undefined; bodyText?: string | null | undefined }>,
): boolean => OPT_OUT_PATTERN.test(`${input.subject ?? ''} ${input.bodyText ?? ''}`)

const clamp = (value: string | null | undefined, max: number): string | null =>
  value === null || value === undefined
    ? null
    : value.trim().replace(/\s+/g, ' ').slice(0, max) || null

export type CrmReplyEventInput = Readonly<{
  tenantRef: string
  fromEmail: string
  subject?: string | null
  bodyText?: string | null
  inReplyToRef?: string | null
  provider?: string
  providerEventId?: string | null
}>

export type CrmReplyRoutedTo = 'sarah_inbox' | 'operator_notification'

export type CrmReplyEventResult = Readonly<{
  replyEventId: string
  contactId: string | null
  optOut: boolean
  routedTo: CrmReplyRoutedTo
  duplicate: boolean
}>

type ReplyEventRow = Readonly<{
  id: string
  contact_id: string | null
  opt_out: number
  routed_to: string
}>

/**
 * Record one inbound reply event: idempotent by (provider, providerEventId)
 * when given, matched to a CRM contact by email when one exists, logged as a
 * `crm_activities` row, and — when the reply reads as an opt-out — suppressed
 * through the existing suppression machinery (both the `marketing` category
 * opt-out and a hard `all`-scope suppression entry, so no future channel can
 * re-send to this address without a fresh explicit opt-in).
 */
export const recordCrmReplyEvent = async (
  db: CrmEmailDatabase,
  input: CrmReplyEventInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmReplyEventResult> => {
  const provider = input.provider ?? 'inbound_webhook'
  const providerEventId = input.providerEventId ?? null

  if (providerEventId !== null) {
    const existing = await crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id, contact_id, opt_out, routed_to FROM crm_reply_events
          WHERE provider = ? AND provider_event_id = ? LIMIT 1`,
      )
      .bind(provider, providerEventId)
      .first<ReplyEventRow>()
    if (existing !== null) {
      return {
        contactId: existing.contact_id,
        duplicate: true,
        optOut: existing.opt_out === 1,
        replyEventId: existing.id,
        routedTo: existing.routed_to === 'sarah_inbox' ? 'sarah_inbox' : 'operator_notification',
      }
    }
  }

  const email = normalizeCrmEmail(input.fromEmail)
  const contactRow = await crmEmailAuthorityDb(db)
    .prepare(
      'SELECT id FROM crm_contacts WHERE tenant_ref = ? AND primary_email = ? LIMIT 1',
    )
    .bind(input.tenantRef, email)
    .first<{ id: string }>()
  const contactId = contactRow?.id ?? null

  const optOut = detectCrmReplyOptOut({ bodyText: input.bodyText, subject: input.subject })
  const routedTo: CrmReplyRoutedTo = SARAH_EMAIL_CHANNEL_LIVE
    ? 'sarah_inbox'
    : 'operator_notification'

  const id = runtime.makeId('crm_reply_event')
  const now = runtime.nowIso()
  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO crm_reply_events (
         id, tenant_ref, contact_id, from_email, subject, body_text,
         in_reply_to_ref, provider, provider_event_id, opt_out, routed_to,
         created_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      id,
      input.tenantRef,
      contactId,
      email,
      clamp(input.subject, 500),
      clamp(input.bodyText, 5000),
      clamp(input.inReplyToRef, 320),
      provider,
      providerEventId,
      optOut ? 1 : 0,
      routedTo,
      now,
    )
    .run()

  if (contactId !== null) {
    await recordCrmActivity(
      db,
      {
        activityType: 'email_reply',
        actorRef: email,
        contactId,
        sourceRecordId: providerEventId ?? id,
        sourceRecordType: 'crm_reply_event',
        sourceSystem: routedTo === 'sarah_inbox' ? 'sarah_inbox' : 'inbound_email_v0',
        subject: clamp(input.subject, 500),
        summary: optOut
          ? 'Reply detected as an opt-out request; suppression recorded.'
          : clamp(input.bodyText, 500),
        tenantRef: input.tenantRef,
      },
      runtime,
    )
  }

  if (optOut) {
    await recordEmailUnsubscribe(
      db,
      { category: 'marketing', email, sourceAuthorityRef: `crm.reply_event:${id}` },
      runtime,
    )
    await addEmailSuppression(
      db,
      { email, reason: 'unsubscribe', scope: 'all', sourceAuthorityRef: `crm.reply_event:${id}` },
      runtime,
    )
  }

  return { contactId, duplicate: false, optOut, replyEventId: id, routedTo }
}

export type ListCrmReplyEventsQuery = Readonly<{
  contactId?: string | null
  limit?: number | undefined
}>

export type CrmReplyEvent = Readonly<{
  id: string
  tenantRef: string
  contactId: string | null
  fromEmail: string
  subject: string | null
  bodyText: string | null
  inReplyToRef: string | null
  provider: string
  providerEventId: string | null
  optOut: boolean
  routedTo: CrmReplyRoutedTo
  createdAt: string
}>

const decodeReplyEvent = (row: Record<string, unknown>): CrmReplyEvent => ({
  bodyText: row.body_text === null || row.body_text === undefined ? null : String(row.body_text),
  contactId: row.contact_id === null || row.contact_id === undefined ? null : String(row.contact_id),
  createdAt: String(row.created_at ?? ''),
  fromEmail: String(row.from_email ?? ''),
  id: String(row.id ?? ''),
  inReplyToRef:
    row.in_reply_to_ref === null || row.in_reply_to_ref === undefined
      ? null
      : String(row.in_reply_to_ref),
  optOut: Number(row.opt_out) === 1,
  provider: String(row.provider ?? ''),
  providerEventId:
    row.provider_event_id === null || row.provider_event_id === undefined
      ? null
      : String(row.provider_event_id),
  routedTo: row.routed_to === 'sarah_inbox' ? 'sarah_inbox' : 'operator_notification',
  subject: row.subject === null || row.subject === undefined ? null : String(row.subject),
  tenantRef: String(row.tenant_ref ?? ''),
})

export const listCrmReplyEvents = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: ListCrmReplyEventsQuery = {},
): Promise<ReadonlyArray<CrmReplyEvent>> => {
  const limit =
    query.limit === undefined || !Number.isFinite(query.limit) || query.limit <= 0
      ? 100
      : Math.min(Math.floor(query.limit), 500)

  const statement =
    query.contactId === undefined || query.contactId === null || query.contactId.trim() === ''
      ? crmEmailAuthorityDb(db)
          .prepare(
            'SELECT * FROM crm_reply_events WHERE tenant_ref = ? ORDER BY created_at DESC LIMIT ?',
          )
          .bind(tenantRef, limit)
      : crmEmailAuthorityDb(db)
          .prepare(
            'SELECT * FROM crm_reply_events WHERE tenant_ref = ? AND contact_id = ? ORDER BY created_at DESC LIMIT ?',
          )
          .bind(tenantRef, query.contactId.trim(), limit)

  const result = await statement.all<Record<string, unknown>>()
  return (result.results ?? []).map(decodeReplyEvent)
}
