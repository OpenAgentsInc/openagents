/**
 * Unified two-channel CRM send abstraction (epic #5980, sub-issue #5985).
 *
 * One entry point — `dispatchCrmSend` — over BOTH channels, with the
 * suppression/unsubscribe/preference gate enforced ONCE for either channel and
 * every attempt written to the same `crm_email_messages` ledger:
 *
 *   channel 'resend'    -> sends server-side now (crm-resend.ts).
 *   channel 'gmail_gws' -> records a `queued` ledger row + returns a send plan;
 *                          the LOCAL executor (#5987) sends as the operator's
 *                          mailbox via `gws` and writes the outcome back. Gmail
 *                          OAuth can't live in a Worker, so transport stays
 *                          local — but gating + ledger are unified here.
 */
import {
  composeCrmEmailForContact,
  CrmEmailError,
  type CrmEmailMessage,
  type CrmSendChannel,
  recordCrmEmailMessage,
} from './crm-email'
// KS-8.11 (#8322): the dispatch seam takes the `CrmEmailDatabase` union so
// the dual-write handle flows through to the gate reads + ledger writes.
import { type CrmEmailDatabase } from './crm-email-domain-store'
import {
  type CrmResendDeps,
  type CrmResendSendResult,
  sendCrmEmailViaResend,
} from './crm-resend'
import { type CrmRuntime, defaultCrmRuntime } from './crm-store'
import { readEmailSendEligibility } from './email-preferences'

export type CrmSendRequest = Readonly<{
  tenantRef: string
  contactId: string
  templateSlug: string
  channel: CrmSendChannel
  sendReason?: string | null
}>

export type CrmGmailSendPlan = Readonly<{
  bodyHtml: string
  bodyMarkdown: string
  subject: string
  toEmail: string
}>

export type CrmSendOutcome =
  | Readonly<{ channel: 'resend'; result: CrmResendSendResult }>
  | Readonly<{
      channel: 'gmail_gws'
      kind: 'gmail_queued'
      message: CrmEmailMessage
      plan: CrmGmailSendPlan
    }>
  | Readonly<{ channel: 'gmail_gws'; kind: 'suppressed'; reason: string; toEmail: string }>

export type CrmDispatchDeps = Readonly<{
  resend: CrmResendDeps
}>

/**
 * Dispatch one CRM send over the selected channel. Throws `CrmEmailError` only
 * for a missing contact/template (the route maps that to 422).
 */
export const dispatchCrmSend = async (
  db: CrmEmailDatabase,
  deps: CrmDispatchDeps,
  request: CrmSendRequest,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmSendOutcome> => {
  if (request.channel === 'resend') {
    const result = await sendCrmEmailViaResend(
      db,
      deps.resend,
      {
        contactId: request.contactId,
        sendReason: request.sendReason ?? null,
        templateSlug: request.templateSlug,
        tenantRef: request.tenantRef,
      },
      runtime,
    )
    return { channel: 'resend', result }
  }

  // gmail_gws: compose + gate here (shared), queue for the local executor.
  const composed = await composeCrmEmailForContact(db, {
    contactId: request.contactId,
    templateSlug: request.templateSlug,
    tenantRef: request.tenantRef,
  })
  const eligibility = await readEmailSendEligibility(db, {
    category: 'marketing',
    email: composed.toEmail,
  })
  if (!eligibility.allowed) {
    return {
      channel: 'gmail_gws',
      kind: 'suppressed',
      reason: eligibility.reason,
      toEmail: composed.toEmail,
    }
  }

  const message = await recordCrmEmailMessage(
    db,
    {
      bodyHtml: composed.bodyHtml,
      bodyMarkdown: composed.bodyMarkdown,
      channel: 'gmail_gws',
      contactId: request.contactId,
      sendReason: request.sendReason ?? 'crm_gmail_outreach',
      status: 'queued',
      subject: composed.subject,
      templateId: composed.template.id,
      tenantRef: request.tenantRef,
      toEmail: composed.toEmail,
    },
    runtime,
  )

  return {
    channel: 'gmail_gws',
    kind: 'gmail_queued',
    message,
    plan: {
      bodyHtml: composed.bodyHtml,
      bodyMarkdown: composed.bodyMarkdown,
      subject: composed.subject,
      toEmail: composed.toEmail,
    },
  }
}

export { CrmEmailError }
