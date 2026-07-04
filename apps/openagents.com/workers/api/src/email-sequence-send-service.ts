// EMAIL-SEQUENCE SEND-SERVICE INTEGRATION (#4983/#4984; promise
// autopilot_sites.native_email_sequences.v1, yellow).
//
// The shared multi-step dispatcher (email-campaign-dispatcher.ts) only knows
// how to render the three onboarding DRIP templates: dripKindFromRow() returns
// null for any operator-authored sequence step, so every authored-sequence send
// is marked `unsupported_campaign_template` and skipped. That is the open
// blocker `blocker.product_promises.email_send_service_integration_missing`:
// authored sequences can be created, enrolled, and SCHEDULED, but there is no
// send-service path that turns a due authored-sequence send into an actual email.
//
// This module is that integration seam, and it is INERT by default. It does
// NOT touch the live dispatcher and changes nothing on the running Worker until
// explicitly armed:
//
//   - It classifies a due authored-sequence send row into a typed
//     `EmailSequenceSendPlan` (the send-service contract: who/what/which
//     template/idempotency key).
//   - Gated by an additive, default-OFF feature flag
//     (`EMAIL_SEQUENCE_SEND_ENABLED`, mirroring isSiteFormCaptureEnabled). When
//     the flag is OFF — the default and the only state on the live Worker — the
//     service returns a `dry_run` outcome and NEVER invokes the injected sender.
//   - The sender is INJECTED, so even the armed path can be exercised in tests
//     against a fake. The real Worker wiring of a live sender + deliverability
//     proof + bounce/complaint handling stays owner/product-gated and is the
//     remaining receipt for green (the deliverability blocker is NOT cleared
//     here, and the promise stays yellow — green is owner-signed per
//     proof.claim_upgrade_receipts.v1).
//
// No new migration: reuses the migration-0063 campaign send tables and the
// existing email send ledger via the injected sender.

// KS-8.11 (#8322): union type so the dual-write handle flows through.
import { type CrmEmailDatabase } from './crm-email-domain-store'
import { Effect } from 'effect'

import {
  type CloudflareEmailBinding,
  type EmailLedgerSendResult,
  type EmailRuntime,
  RenderedEmail,
  sendRenderedEmailViaCloudflareBindingWithLedger,
  systemEmailRuntime,
} from './email'

export const EMAIL_SEQUENCE_SEND_PROMISE_ID =
  'autopilot_sites.native_email_sequences.v1'

// The single blocker this integration clears once landed (the send-service
// seam now exists and is exercised). Deliverability and customer self-serve are
// NOT touched here and stay owner/product-gated.
export const EMAIL_SEQUENCE_SEND_BLOCKER_CLEARED =
  'blocker.product_promises.email_send_service_integration_missing'

// Additive, default-OFF feature flag. Mirrors isSiteFormCaptureEnabled:
// absent or any non-truthy value => disabled (INERT). When disabled the send
// service plans every send as a dry-run and never calls the sender.
export const isEmailSequenceSendEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// The minimal, send-service-facing view of a due authored-sequence send. Built
// from the dispatcher's DueCampaignSendRow but kept independent so this module
// has no dependency on the dispatcher internals.
export type EmailSequenceSendRow = Readonly<{
  campaignId: string
  email: string
  enrollmentId: string
  idempotencyKey: string
  sendId: string
  sourceAuthorityRef: string
  stepId: string
  stepKey: string
  templateSlug: string
  // Optional, already-parsed metadata (e.g. displayName). Never required.
  displayName?: string | null
  userId?: string | null
}>

// Typed send-service contract derived from a send row. This is what an armed
// sender receives; it carries no transport, vendor, or secret material.
export type EmailSequenceSendPlan = Readonly<{
  campaignId: string
  displayName: string
  enrollmentId: string
  idempotencyKey: string
  sendId: string
  sourceAuthorityRef: string
  stepId: string
  stepKey: string
  templateSlug: string
  to: string
  userId: string | null
}>

export type EmailSequenceCloudflareSenderConfig = Readonly<{
  appOrigin: string
  fromEmail: string
  replyToEmail?: string | undefined
}>

// An injected sender. The real implementation wires the email ledger
// (sendRenderedEmailViaCloudflareBindingWithLedger / a per-template sender). In
// the default INERT state this is never called.
export type EmailSequenceSender = (
  plan: EmailSequenceSendPlan,
) => Effect.Effect<EmailLedgerSendResult, never>

export type EmailSequenceSendOutcome =
  | Readonly<{
      kind: 'dry_run'
      plan: EmailSequenceSendPlan
      reason: 'send_disabled'
    }>
  | Readonly<{
      kind: 'sent'
      plan: EmailSequenceSendPlan
      result: EmailLedgerSendResult & { ok: true }
    }>
  | Readonly<{
      kind: 'failed'
      plan: EmailSequenceSendPlan
      result: EmailLedgerSendResult & { ok: false }
    }>

// Build the typed send-service plan for a due authored-sequence send. Pure and
// total — it never sends, it only shapes the contract.
export const planEmailSequenceSend = (
  row: EmailSequenceSendRow,
): EmailSequenceSendPlan => ({
  campaignId: row.campaignId,
  displayName:
    typeof row.displayName === 'string' && row.displayName.trim() !== ''
      ? row.displayName.trim()
      : 'there',
  enrollmentId: row.enrollmentId,
  idempotencyKey: row.idempotencyKey,
  sendId: row.sendId,
  sourceAuthorityRef: row.sourceAuthorityRef,
  stepId: row.stepId,
  stepKey: row.stepKey,
  templateSlug: row.templateSlug,
  to: row.email,
  userId: row.userId ?? null,
})

export type EmailSequenceSendServiceDependencies = Readonly<{
  // Per-call gate. Read from the live request/cron environment so arming is a
  // configuration change, not a redeploy. Defaults OFF (INERT).
  isEnabled: () => boolean
  // The armed sender. Only invoked when isEnabled() returns true.
  send: EmailSequenceSender
}>

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const compactLabel = (value: string): string =>
  value
    .trim()
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .slice(0, 120)

const managePreferencesUrl = (appOrigin: string): string =>
  `${appOrigin.replace(/\/+$/, '')}/email/preferences`

export const renderEmailSequenceSend = (
  config: EmailSequenceCloudflareSenderConfig,
  plan: EmailSequenceSendPlan,
): RenderedEmail => {
  const sequenceLabel = compactLabel(plan.stepKey) || 'update'
  const preferencesUrl = managePreferencesUrl(config.appOrigin)
  const subject = `OpenAgents Sites: ${sequenceLabel}`
  const text = [
    `Hi ${plan.displayName},`,
    '',
    `This is the ${sequenceLabel} message from your OpenAgents Sites sequence.`,
    '',
    `Manage email preferences: ${preferencesUrl}`,
    '',
    'OpenAgents',
  ].join('\n')
  const html = `<!doctype html>
<html>
  <body style="margin:0;background:#050607;color:#f7f8f8;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;">
      <p style="margin:0 0 28px;color:#8a8f98;font-size:14px;">OpenAgents Sites</p>
      <h1 style="margin:0;color:#f7f8f8;font-size:26px;font-weight:600;line-height:1.25;">${escapeHtml(sequenceLabel)}</h1>
      <p style="margin:18px 0 0;color:#d0d6e0;font-size:15px;line-height:1.6;">Hi ${escapeHtml(plan.displayName)}, this is the ${escapeHtml(sequenceLabel)} message from your OpenAgents Sites sequence.</p>
      <p style="margin:28px 0 0;color:#8a8f98;font-size:13px;line-height:1.5;"><a href="${escapeHtml(preferencesUrl)}" style="color:#f7f8f8;">Manage email preferences</a></p>
    </div>
  </body>
</html>`

  return new RenderedEmail({
    from: config.fromEmail,
    html,
    idempotencyKey: plan.idempotencyKey,
    kind: 'crm_transactional',
    metadataJson: JSON.stringify({
      campaignId: plan.campaignId,
      enrollmentId: plan.enrollmentId,
      promiseId: EMAIL_SEQUENCE_SEND_PROMISE_ID,
      sendId: plan.sendId,
      stepId: plan.stepId,
      stepKey: plan.stepKey,
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject,
    tags: [
      { name: 'promise', value: EMAIL_SEQUENCE_SEND_PROMISE_ID },
      { name: 'campaign_id', value: plan.campaignId },
      { name: 'step_key', value: plan.stepKey },
    ],
    templateContextJson: JSON.stringify({
      displayName: plan.displayName,
      managePreferencesUrl: preferencesUrl,
      stepKey: plan.stepKey,
    }),
    templateSlug: plan.templateSlug,
    text,
    to: plan.to,
  })
}

export const makeCloudflareEmailSequenceSender = (
  db: CrmEmailDatabase,
  binding: CloudflareEmailBinding,
  config: EmailSequenceCloudflareSenderConfig,
  runtime: EmailRuntime = systemEmailRuntime,
): EmailSequenceSender => plan =>
  sendRenderedEmailViaCloudflareBindingWithLedger(
    db,
    binding,
    renderEmailSequenceSend(config, plan),
    {
      metadata: {
        campaignId: plan.campaignId,
        enrollmentId: plan.enrollmentId,
        stepId: plan.stepId,
        stepKey: plan.stepKey,
      },
      sourceAuthorityRef: plan.sourceAuthorityRef,
      targetUserId: plan.userId ?? undefined,
    },
    runtime,
  ).pipe(
    Effect.catch(error =>
      Effect.succeed({
        emailMessageId: plan.sendId,
        errorMessage: error.message,
        errorName: error.operation,
        ok: false as const,
      }),
    ),
  )

// The send-service integration. dispatchSequenceSend turns a due
// authored-sequence send into either a dry-run (INERT default; the sender is
// never called) or — only when armed — a real send through the injected sender.
//
// This is deliberately NOT chained into the live dispatcher: it is the wired,
// tested seam that the operator/owner can arm, with the deliverability proof
// and live-sender wiring tracked as the remaining green receipt.
export const makeEmailSequenceSendService = (
  dependencies: EmailSequenceSendServiceDependencies,
) => ({
  dispatchSequenceSend: (
    row: EmailSequenceSendRow,
  ): Effect.Effect<EmailSequenceSendOutcome, never> => {
    const plan = planEmailSequenceSend(row)

    if (!dependencies.isEnabled()) {
      return Effect.succeed({ kind: 'dry_run', plan, reason: 'send_disabled' })
    }

    return dependencies.send(plan).pipe(
      Effect.map(result =>
        result.ok
          ? { kind: 'sent', plan, result }
          : { kind: 'failed', plan, result },
      ),
    )
  },
})
