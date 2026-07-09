/**
 * CRM Resend channel — the scalable, Worker-side send path (epic #5980,
 * sub-issue #5984), sibling to the local Gmail/gws channel (#5983).
 *
 * HONEST ARMING: the sender is INERT by default. It only sends when BOTH the
 * `CRM_RESEND_SEND_ENABLED` flag is on AND Resend is configured (api key +
 * from address). Disabled => a dry-run plan that NEVER calls Resend. This
 * mirrors `email-sequence-send-service.ts` and the owner mandate: no faked
 * greens, no live send until armed + the sending domain is verified.
 *
 * The "prove" half (a live send->deliver receipt with bounce/complaint
 * handling) is owner/secret-gated: it needs a real RESEND_API_KEY and a verified
 * sending domain. `scripts/crm-resend-smoke.mjs` drives that once armed.
 *
 * Composition + ledger are shared with the Gmail channel via `crm-email.ts`;
 * suppression/unsubscribe reuse the shared `readEmailSendEligibility` gate.
 */
import { Redacted } from 'effect'

import {
  composeCrmEmailForContact,
  CrmEmailError,
  type CrmEmailMessage,
  recordCrmEmailMessage,
  updateCrmEmailMessageDelivery,
} from './crm-email'
// KS-8.11 (#8322): union type so the dual-write handle flows through.
import { type CrmEmailDatabase } from './crm-email-domain-store'
import { type CrmRuntime, defaultCrmRuntime, recordCrmActivity } from './crm-store'
import { readEmailSendEligibility } from './email-preferences'

export const isCrmResendSendEnabled = (value: string | undefined): boolean =>
  value !== undefined && ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())

/**
 * OB-1 (#8558): resolve the effective CRM sender identity. The CRM send path
 * uses its own from/reply-to (`Sarah <sarah@openagents.com>`) when configured,
 * so the shared Sites transactional identity (RESEND_FROM_EMAIL) is never
 * changed as a side effect. When the CRM overrides are absent it falls back to
 * the shared Resend from/reply-to.
 */
export const resolveCrmResendIdentity = (
  shared: Readonly<{ fromEmail: string; replyToEmail?: string | undefined }>,
  override: Readonly<{
    fromEmail?: string | undefined
    replyToEmail?: string | undefined
  }>,
): Readonly<{ fromEmail: string; replyToEmail?: string | undefined }> => ({
  fromEmail: override.fromEmail ?? shared.fromEmail,
  replyToEmail: override.replyToEmail ?? shared.replyToEmail,
})

export type CrmResendSenderInput = Readonly<{
  from: string
  to: string
  subject: string
  html: string
  text: string
  idempotencyKey: string
  replyTo?: string | undefined
}>

export type CrmResendSenderResult =
  | Readonly<{ ok: true; providerMessageId: string | null }>
  | Readonly<{ ok: false; errorMessage: string; errorName?: string | undefined }>

export type CrmResendSender = (input: CrmResendSenderInput) => Promise<CrmResendSenderResult>

/**
 * Production Resend HTTP sender. Mirrors the existing transactional sender in
 * `email.ts` (POST https://api.resend.com/emails with an Idempotency-Key).
 */
export const makeCrmResendSender = (
  config: Readonly<{ apiKey: Redacted.Redacted<string>; replyTo?: string | undefined }>,
  fetcher: typeof fetch = fetch,
): CrmResendSender => {
  return async input => {
    try {
      const response = await fetcher('https://api.resend.com/emails', {
        body: JSON.stringify({
          from: input.from,
          html: input.html,
          subject: input.subject,
          text: input.text,
          to: [input.to],
          ...(input.replyTo === undefined && config.replyTo === undefined
            ? {}
            : { reply_to: input.replyTo ?? config.replyTo }),
        }),
        headers: {
          Authorization: `Bearer ${Redacted.value(config.apiKey)}`,
          'content-type': 'application/json',
          'Idempotency-Key': input.idempotencyKey,
        },
        method: 'POST',
      })
      const payload = (await response.json().catch(() => ({}))) as Record<string, unknown>
      if (!response.ok) {
        const errorBody = payload as { message?: unknown; name?: unknown }
        return {
          errorMessage:
            typeof errorBody.message === 'string' ? errorBody.message : `resend ${response.status}`,
          errorName: typeof errorBody.name === 'string' ? errorBody.name : 'resend_rejected',
          ok: false,
        }
      }
      return {
        ok: true,
        providerMessageId: typeof payload.id === 'string' ? payload.id : null,
      }
    } catch (error) {
      return {
        errorMessage: error instanceof Error ? error.message : String(error),
        errorName: error instanceof Error ? error.name : 'resend_fetch_error',
        ok: false,
      }
    }
  }
}

export type CrmResendDeps = Readonly<{
  enabled: boolean
  fromEmail: string | null
  sender: CrmResendSender | null
}>

export type CrmResendSendInput = Readonly<{
  tenantRef: string
  contactId: string
  templateSlug: string
  sendReason?: string | null
}>

export type CrmResendSendResult =
  | Readonly<{ kind: 'dry_run'; reason: 'send_disabled'; toEmail: string; subject: string }>
  | Readonly<{ kind: 'not_configured'; reason: string }>
  | Readonly<{ kind: 'suppressed'; reason: string; toEmail: string }>
  | Readonly<{ kind: 'sent'; message: CrmEmailMessage }>
  | Readonly<{ kind: 'failed'; message: CrmEmailMessage; errorMessage: string }>

/**
 * Compose, gate, and (only when armed) send one CRM email via Resend, recording
 * the result in the CRM ledger. Throws `CrmEmailError` only for a missing
 * contact/template (the route maps that to 422).
 */
export const sendCrmEmailViaResend = async (
  db: CrmEmailDatabase,
  deps: CrmResendDeps,
  input: CrmResendSendInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmResendSendResult> => {
  const composed = await composeCrmEmailForContact(db, {
    contactId: input.contactId,
    templateSlug: input.templateSlug,
    tenantRef: input.tenantRef,
  })

  const eligibility = await readEmailSendEligibility(db, {
    category: 'marketing',
    email: composed.toEmail,
  })
  if (!eligibility.allowed) {
    return { kind: 'suppressed', reason: eligibility.reason, toEmail: composed.toEmail }
  }

  // INERT unless armed.
  if (!deps.enabled) {
    return {
      kind: 'dry_run',
      reason: 'send_disabled',
      subject: composed.subject,
      toEmail: composed.toEmail,
    }
  }
  if (deps.sender === null || deps.fromEmail === null) {
    return { kind: 'not_configured', reason: 'resend api key / from address missing' }
  }

  // Record a queued ledger row first so we have a stable id + idempotency key.
  const queued = await recordCrmEmailMessage(
    db,
    {
      bodyHtml: composed.bodyHtml,
      bodyMarkdown: composed.bodyMarkdown,
      channel: 'resend',
      contactId: input.contactId,
      fromEmail: deps.fromEmail,
      sendReason: input.sendReason ?? 'crm_resend_outreach',
      status: 'queued',
      subject: composed.subject,
      templateId: composed.template.id,
      tenantRef: input.tenantRef,
      toEmail: composed.toEmail,
    },
    runtime,
  )

  const result = await deps.sender({
    from: deps.fromEmail,
    html: composed.bodyHtml,
    idempotencyKey: queued.id,
    subject: composed.subject,
    text: composed.bodyMarkdown,
    to: composed.toEmail,
  })

  if (!result.ok) {
    await updateCrmEmailMessageDelivery(
      db,
      { errorMessage: result.errorMessage, id: queued.id, status: 'failed', tenantRef: input.tenantRef },
      runtime,
    )
    const failed = { ...queued, errorMessage: result.errorMessage, status: 'failed' }
    return { errorMessage: result.errorMessage, kind: 'failed', message: failed }
  }

  await updateCrmEmailMessageDelivery(
    db,
    {
      id: queued.id,
      providerMessageId: result.providerMessageId,
      status: 'sent',
      tenantRef: input.tenantRef,
    },
    runtime,
  )
  await recordCrmActivity(
    db,
    {
      activityType: 'email_sent',
      contactId: input.contactId,
      sourceRecordId: queued.id,
      sourceRecordType: 'crm_email_message',
      sourceSystem: 'resend',
      subject: composed.subject,
      summary: 'Sent via Resend',
      tenantRef: input.tenantRef,
    },
    runtime,
  )
  const sent = {
    ...queued,
    providerMessageId: result.providerMessageId,
    status: 'sent',
  }
  return { kind: 'sent', message: sent }
}

/** Re-export for the route so it doesn't import crm-email just for the error. */
export { CrmEmailError }
