import {
  AutopilotDecisionEmailKind,
  AutopilotDecisionTemplateProps,
  DripTemplateProps,
  ORDER_SITES_LIFECYCLE_EMAIL_KINDS,
  OrderSitesLifecycleTemplateProps,
  PrivateWorkspaceInviteTemplateProps,
  renderAutopilotDecisionEmail,
  renderDripEmail,
  renderOrderSitesLifecycleEmail,
  renderPrivateWorkspaceInviteEmail,
} from '@openagentsinc/email-templates'
import {
  containsProviderSecretMaterial,
  redactProviderAccountSecretMaterial,
} from '@openagentsinc/provider-account-schema'
import { Context, Effect, Layer, Redacted, Schema as S } from 'effect'

import {
  type ResendEmailConfig,
  ResendEmailSender,
  WorkerSecret,
} from './config'
import { parseJsonRecord, parseJsonWithSchema } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const EmailMessageId = S.String.pipe(S.brand('EmailMessageId'))
export type EmailMessageId = typeof EmailMessageId.Type

export const EmailDeliveryId = S.String.pipe(S.brand('EmailDeliveryId'))
export type EmailDeliveryId = typeof EmailDeliveryId.Type

export const EmailProvider = S.Literals([
  'resend',
  'gmail',
  'cloudflare_email',
])
export type EmailProvider = typeof EmailProvider.Type

export const EmailKind = S.Literals([
  'prelaunch_invitation',
  'billing_out_of_credits',
  'operator_notification',
  'crm_transactional',
])
export type EmailKind = typeof EmailKind.Type

export const EmailIntentStatus = S.Literals([
  'reserved',
  'rendered',
  'accepted',
  'failed',
  'draft_recorded',
])
export type EmailIntentStatus = typeof EmailIntentStatus.Type

export const EmailDeliveryStatus = S.Literals([
  'queued',
  'accepted',
  'failed',
  'unknown_external_state',
])
export type EmailDeliveryStatus = typeof EmailDeliveryStatus.Type

export const EmailDraftStatus = S.Literals([
  'draft_requested',
  'draft_created',
  'draft_failed',
  'sent_from_draft',
  'abandoned',
])
export type EmailDraftStatus = typeof EmailDraftStatus.Type

export class EmailTag extends S.Class<EmailTag>('EmailTag')({
  name: S.String,
  value: S.String,
}) {}

export class RenderedEmail extends S.Class<RenderedEmail>('RenderedEmail')({
  from: S.String,
  html: S.String,
  idempotencyKey: S.String,
  kind: EmailKind,
  metadataJson: S.String,
  replyTo: S.optionalKey(S.String),
  subject: S.String,
  tags: S.Array(EmailTag),
  templateContextJson: S.String,
  templateSlug: S.String,
  text: S.String,
  to: S.String,
}) {}

export class EmailProviderAccepted extends S.TaggedClass<EmailProviderAccepted>()(
  'EmailProviderAccepted',
  {
    provider: EmailProvider,
    providerMessageId: S.NullOr(S.String),
  },
) {}

export class EmailProviderRejected extends S.TaggedClass<EmailProviderRejected>()(
  'EmailProviderRejected',
  {
    errorMessage: S.String,
    errorName: S.String,
    provider: EmailProvider,
  },
) {}

export const EmailProviderResult = S.Union([
  EmailProviderAccepted,
  EmailProviderRejected,
])
export type EmailProviderResult = typeof EmailProviderResult.Type

export class EmailServiceError extends S.TaggedErrorClass<EmailServiceError>()(
  'EmailServiceError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export type SendOutOfCreditsEmailInput = Readonly<{
  appOrigin: string
  balanceFormatted: string
  displayName: string
  idempotencyKey: string
  to: string
}>

export type AdjutantCustomerNotificationStage =
  | 'deployed'
  | 'input_needed'
  | 'review_ready'
  | 'unavailable'

export type SendAdjutantCustomerNotificationInput = Readonly<{
  appOrigin: string
  displayName: string
  idempotencyKey: string
  orderId: string
  siteTitle: string | null
  siteUrl: string | null
  stage: AdjutantCustomerNotificationStage
  to: string
}>

export class AutopilotDecisionEmailInput extends S.Class<AutopilotDecisionEmailInput>(
  'AutopilotDecisionEmailInput',
)({
  appOrigin: S.String,
  displayName: S.String,
  idempotencyKey: S.String,
  kind: AutopilotDecisionEmailKind,
  to: S.String,
  workOrderRef: S.String,
}) {}

export class DripCampaignEmailInput extends S.Class<DripCampaignEmailInput>(
  'DripCampaignEmailInput',
)({
  appOrigin: S.String,
  displayName: S.String,
  idempotencyKey: S.String,
  kind: S.Literals(['signup_day_0', 'signup_day_1', 'signup_day_2']),
  managePreferencesUrl: S.String,
  to: S.String,
}) {}

export class SiteReferralOnboardingEmailInput extends S.Class<SiteReferralOnboardingEmailInput>(
  'SiteReferralOnboardingEmailInput',
)({
  appOrigin: S.String,
  displayName: S.String,
  idempotencyKey: S.String,
  sourceLabel: S.String,
  sourceSiteUrl: S.NullOr(S.String),
  to: S.String,
}) {}

export class TargetedRemakeOutreachEmailInput extends S.Class<TargetedRemakeOutreachEmailInput>(
  'TargetedRemakeOutreachEmailInput',
)({
  appOrigin: S.String,
  campaignId: S.String,
  conceptDisclosure: S.String,
  displayName: S.String,
  idempotencyKey: S.String,
  meetingUrl: S.String,
  postalAddress: S.String,
  preferencesUrl: S.String,
  previewGenerationId: S.String,
  previewUrl: S.String,
  prospectId: S.NullOr(S.String),
  senderContact: S.String,
  senderName: S.String,
  targetDomain: S.String,
  targetName: S.String,
  to: S.String,
  unsubscribeUrl: S.String,
  valueProposition: S.String,
}) {}

export class PrivateWorkspaceInviteEmailInput extends S.Class<PrivateWorkspaceInviteEmailInput>(
  'PrivateWorkspaceInviteEmailInput',
)({
  acceptUrl: S.String,
  displayName: S.String,
  expiresAt: S.String,
  idempotencyKey: S.String,
  inviteId: S.String,
  projectId: S.NullOr(S.String),
  teamId: S.String,
  to: S.String,
  workspaceLabel: S.String,
}) {}

export const OrderSitesTransactionalEmailKind = S.Literals([
  'order_received',
  'scoping_started',
  'repository_source_needed',
  'autopilot_queued',
  'autopilot_running',
  'review_ready',
  'site_saved_version_ready',
  'site_deployed',
  'customer_input_needed',
  'unavailable_declined',
  'delivered',
  'adjustment_received',
  'adjustment_completed',
])
export type OrderSitesTransactionalEmailKind =
  typeof OrderSitesTransactionalEmailKind.Type

export const ORDER_SITES_TRANSACTIONAL_EMAIL_KINDS = [
  ...ORDER_SITES_LIFECYCLE_EMAIL_KINDS,
] as const satisfies ReadonlyArray<OrderSitesTransactionalEmailKind>

export class OrderSitesTransactionalEmailInput extends S.Class<OrderSitesTransactionalEmailInput>(
  'OrderSitesTransactionalEmailInput',
)({
  appOrigin: S.String,
  assignmentId: S.optionalKey(S.String),
  artifactLabel: S.NullOr(S.String),
  artifactUrl: S.NullOr(S.String),
  displayName: S.String,
  eventRef: S.optionalKey(S.String),
  idempotencyKey: S.optionalKey(S.String),
  lifecycleKind: OrderSitesTransactionalEmailKind,
  nextAction: S.String,
  orderId: S.String,
  safeReason: S.NullOr(S.String),
  revisionUrl: S.NullOr(S.String),
  siteId: S.optionalKey(S.String),
  siteTitle: S.NullOr(S.String),
  siteUrl: S.NullOr(S.String),
  statusPageUrl: S.optionalKey(S.String),
  customerSafeStatus: S.String,
  sourceAuthorityRefs: S.optionalKey(S.Array(S.String)),
  targetRefs: S.optionalKey(S.Array(S.String)),
  to: S.String,
}) {}

export type EmailIntentContext = Readonly<{
  actionSubmissionId?: string | undefined
  actorUserId?: string | undefined
  metadata?: Record<string, unknown> | undefined
  sourceAuthorityRef?: string | undefined
  targetUserId?: string | undefined
}>

export type SendEmailResult =
  | Readonly<{ id: string | null; ok: true }>
  | Readonly<{ errorMessage: string; errorName?: string; ok: false }>

export type EmailLedgerSendResult =
  | Readonly<{
      emailMessageId: string
      ok: true
      providerMessageId: string | null
    }>
  | Readonly<{
      emailMessageId: string
      errorMessage: string
      errorName?: string | undefined
      ok: false
    }>

export type OperatorEmailLedgerSmokeMode = 'dry_run' | 'send'

export type OperatorEmailLedgerSmokeInput = Readonly<{
  appOrigin: string
  idempotencyKey: string
  mode?: OperatorEmailLedgerSmokeMode | undefined
  to: string
}>

export type OperatorEmailLedgerSmokeResult = Readonly<{
  configStatus: 'present' | 'missing'
  emailMessageId: string
  errorMessage: string | null
  errorName: string | null
  idempotencyKey: string
  mode: OperatorEmailLedgerSmokeMode
  provider: 'resend'
  providerMessageId: string | null
  status: 'accepted' | 'dry_run' | 'failed' | 'skipped'
  templateSlug: string
}>

type CloudflareEmailSendMessage = Readonly<{
  from: string
  headers?: Readonly<Record<string, string>>
  html: string
  replyTo?: string
  subject: string
  text: string
  to: string
}>

export type CloudflareEmailBinding = Readonly<{
  send: (message: CloudflareEmailSendMessage) => Promise<unknown>
}>

export type EmailRuntime = Readonly<{
  nowIso: () => string
  randomId: (prefix: string) => string
}>

export const systemEmailRuntime: EmailRuntime = {
  nowIso: currentIsoTimestamp,
  randomId: compactRandomId,
}

export type EmailMessageRecord = Readonly<{
  createdAt: string
  errorMessage: string | null
  errorName: string | null
  id: EmailMessageId
  idempotencyKey: string
  kind: EmailKind
  provider: EmailProvider | null
  providerMessageId: string | null
  status: EmailIntentStatus
  updatedAt: string
}>

type EmailMessageRow = Readonly<{
  created_at: string
  error_message: string | null
  error_name: string | null
  id: string
  idempotency_key: string
  kind: EmailKind
  provider: EmailProvider | null
  provider_message_id: string | null
  status: EmailIntentStatus
  updated_at: string
}>

export type EmailServiceShape = Readonly<{
  recordDraft: (
    db: D1Database,
    input: Readonly<{
      context?: EmailIntentContext | undefined
      provider: EmailProvider
      providerDraftId: string
      providerMessageId?: string | undefined
      providerThreadId?: string | undefined
      rendered: RenderedEmail
    }>,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailMessageRecord, EmailServiceError>
  renderOutOfCreditsEmail: (
    config: ResendEmailConfig,
    input: SendOutOfCreditsEmailInput,
  ) => Effect.Effect<RenderedEmail>
  renderAdjutantCustomerNotificationEmail: (
    config: ResendEmailConfig,
    input: SendAdjutantCustomerNotificationInput,
  ) => Effect.Effect<RenderedEmail>
  renderAutopilotDecisionNotificationEmail: (
    config: ResendEmailConfig,
    input: AutopilotDecisionEmailInput,
  ) => Effect.Effect<RenderedEmail, EmailServiceError>
  renderDripCampaignEmail: (
    config: ResendEmailConfig,
    input: DripCampaignEmailInput,
  ) => Effect.Effect<RenderedEmail>
  renderSiteReferralOnboardingEmail: (
    config: ResendEmailConfig,
    input: SiteReferralOnboardingEmailInput,
  ) => Effect.Effect<RenderedEmail, EmailServiceError>
  renderTargetedRemakeOutreachEmail: (
    config: ResendEmailConfig,
    input: TargetedRemakeOutreachEmailInput,
  ) => Effect.Effect<RenderedEmail, EmailServiceError>
  renderPrivateWorkspaceInviteEmail: (
    config: ResendEmailConfig,
    input: PrivateWorkspaceInviteEmailInput,
  ) => Effect.Effect<RenderedEmail, EmailServiceError>
  buildOrderSitesTransactionalEmailIdempotencyKey: (
    input: OrderSitesTransactionalEmailInput,
  ) => string
  renderOrderSitesTransactionalEmail: (
    config: ResendEmailConfig,
    input: OrderSitesTransactionalEmailInput,
  ) => Effect.Effect<RenderedEmail, EmailServiceError>
  sendOrderSitesTransactionalEmailWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: OrderSitesTransactionalEmailInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  sendAdjutantCustomerNotificationWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: SendAdjutantCustomerNotificationInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  sendAutopilotDecisionEmailWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: AutopilotDecisionEmailInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  sendDripCampaignEmailWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: DripCampaignEmailInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  sendSiteReferralOnboardingEmailWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: SiteReferralOnboardingEmailInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  sendTargetedRemakeOutreachEmailWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: TargetedRemakeOutreachEmailInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  sendPrivateWorkspaceInviteEmailWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: PrivateWorkspaceInviteEmailInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  reserveMessage: (
    db: D1Database,
    rendered: RenderedEmail,
    context?: EmailIntentContext | undefined,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailMessageRecord, EmailServiceError>
  sendOutOfCreditsEmail: (
    config: ResendEmailConfig,
    input: SendOutOfCreditsEmailInput,
    fetcher?: typeof fetch,
  ) => Effect.Effect<SendEmailResult>
  sendOutOfCreditsEmailWithLedger: (
    db: D1Database,
    config: ResendEmailConfig,
    input: SendOutOfCreditsEmailInput,
    context?: EmailIntentContext | undefined,
    fetcher?: typeof fetch,
    runtime?: EmailRuntime,
  ) => Effect.Effect<SendEmailResult, EmailServiceError>
  sendRenderedEmail: (
    config: ResendEmailConfig,
    rendered: RenderedEmail,
    fetcher?: typeof fetch,
  ) => Effect.Effect<EmailProviderResult>
  sendRenderedEmailViaCloudflareBinding: (
    binding: CloudflareEmailBinding,
    rendered: RenderedEmail,
  ) => Effect.Effect<EmailProviderResult>
  sendRenderedEmailViaCloudflareBindingWithLedger: (
    db: D1Database,
    binding: CloudflareEmailBinding,
    rendered: RenderedEmail,
    context?: EmailIntentContext | undefined,
    runtime?: EmailRuntime,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
}>

export class EmailService extends Context.Service<
  EmailService,
  EmailServiceShape
>()('@openagentsinc/autopilot-omega/EmailService') {}

const ResendSendResponse = S.Struct({
  id: S.optionalKey(S.Unknown),
  message: S.optionalKey(S.Unknown),
  name: S.optionalKey(S.Unknown),
})
type ResendSendResponse = typeof ResendSendResponse.Type

const decodeResendSendResponse = (text: string): ResendSendResponse => {
  if (text.trim() === '') {
    return {}
  }

  try {
    return parseJsonWithSchema(ResendSendResponse, text)
  } catch {
    return {}
  }
}

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const compactText = (value: string, maxLength: number): string => {
  const compact = value.replace(/\s+/g, ' ').trim()

  return compact.length <= maxLength
    ? compact
    : `${compact.slice(0, Math.max(0, maxLength - 3))}...`
}

const jsonValue = (value: unknown): string => JSON.stringify(value ?? {})

const emailServiceError = (
  operation: string,
  error: unknown,
): EmailServiceError =>
  new EmailServiceError({
    message: error instanceof Error ? error.message : String(error),
    operation,
  })

const billingUrl = (origin: string): string =>
  new URL('/billing', origin).toString()

const orderUrl = (origin: string): string =>
  new URL('/order', origin).toString()

const siteLabel = (input: SendAdjutantCustomerNotificationInput): string =>
  input.siteTitle === null || input.siteTitle.trim() === ''
    ? 'your OpenAgents Site'
    : input.siteTitle

export const outOfCreditsEmailText = (
  input: SendOutOfCreditsEmailInput,
): string =>
  [
    `Hi ${input.displayName},`,
    '',
    `Your OpenAgents Autopilot credits have reached ${input.balanceFormatted}.`,
    'Active Autopilot runs were stopped so container usage does not keep accruing.',
    '',
    `Add credits here: ${billingUrl(input.appOrigin)}`,
    '',
    'OpenAgents',
  ].join('\n')

export const outOfCreditsEmailHtml = (
  input: SendOutOfCreditsEmailInput,
): string => {
  const escapedBillingUrl = escapeHtml(billingUrl(input.appOrigin))

  return `<!doctype html>
<html>
  <body style="margin:0;background:#050607;color:#f7f8f8;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;">
      <p style="margin:0 0 28px;color:#8a8f98;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#f7f8f8;font-size:28px;font-weight:600;line-height:1.2;">Autopilot credits are exhausted.</h1>
      <p style="margin:18px 0 0;color:#d0d6e0;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, your OpenAgents Autopilot credits have reached ${escapeHtml(input.balanceFormatted)}.</p>
      <p style="margin:18px 0 0;color:#d0d6e0;font-size:15px;line-height:1.6;">Active Autopilot runs were stopped so container usage does not keep accruing.</p>
      <p style="margin:28px 0 0;">
        <a href="${escapedBillingUrl}" style="display:inline-block;border-radius:999px;background:#f7f8f8;color:#050607;font-size:14px;font-weight:600;text-decoration:none;padding:11px 18px;">Add credits</a>
      </p>
      <p style="margin:28px 0 0;color:#8a8f98;font-size:13px;line-height:1.5;">If you already added credits, refresh OpenAgents and start a new Autopilot run.</p>
    </div>
  </body>
</html>`
}

const adjutantNotificationSubject = (
  input: SendAdjutantCustomerNotificationInput,
): string =>
  input.stage === 'review_ready'
    ? `${siteLabel(input)} is ready for review`
    : input.stage === 'deployed'
      ? `${siteLabel(input)} is live`
      : input.stage === 'input_needed'
        ? `OpenAgents needs input for ${siteLabel(input)}`
        : `OpenAgents could not continue ${siteLabel(input)}`

const adjutantNotificationLead = (
  input: SendAdjutantCustomerNotificationInput,
): string =>
  input.stage === 'review_ready'
    ? 'Autopilot saved a new Site version and the OpenAgents team is reviewing it before release.'
    : input.stage === 'deployed'
      ? 'The OpenAgents team deployed your Site.'
      : input.stage === 'input_needed'
        ? 'Autopilot needs more input before the Site work can continue.'
        : 'Autopilot could not continue this Site run right now.'

export const adjutantCustomerNotificationEmailText = (
  input: SendAdjutantCustomerNotificationInput,
): string =>
  [
    `Hi ${input.displayName},`,
    '',
    adjutantNotificationLead(input),
    '',
    input.siteUrl === null ? null : `Live Site: ${input.siteUrl}`,
    `Order: ${orderUrl(input.appOrigin)}`,
    '',
    'OpenAgents',
  ]
    .filter((line): line is string => line !== null)
    .join('\n')

export const adjutantCustomerNotificationEmailHtml = (
  input: SendAdjutantCustomerNotificationInput,
): string => {
  const escapedOrderUrl = escapeHtml(orderUrl(input.appOrigin))
  const escapedSiteUrl =
    input.siteUrl === null ? null : escapeHtml(input.siteUrl)

  return `<!doctype html>
<html>
  <body style="margin:0;background:#050607;color:#f7f8f8;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;">
      <p style="margin:0 0 28px;color:#8a8f98;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#f7f8f8;font-size:28px;font-weight:600;line-height:1.2;">${escapeHtml(adjutantNotificationSubject(input))}</h1>
      <p style="margin:18px 0 0;color:#d0d6e0;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, ${escapeHtml(adjutantNotificationLead(input))}</p>
      ${
        escapedSiteUrl === null
          ? ''
          : `<p style="margin:18px 0 0;color:#d0d6e0;font-size:15px;line-height:1.6;">Live Site: <a href="${escapedSiteUrl}" style="color:#f7f8f8;">${escapedSiteUrl}</a></p>`
      }
      <p style="margin:28px 0 0;">
        <a href="${escapedOrderUrl}" style="display:inline-block;border-radius:999px;background:#f7f8f8;color:#050607;font-size:14px;font-weight:600;text-decoration:none;padding:11px 18px;">View order</a>
      </p>
    </div>
  </body>
</html>`
}

export const buildOrderSitesTransactionalEmailIdempotencyKey = (
  input: OrderSitesTransactionalEmailInput,
): string => {
  const encode = (value: string | undefined): string =>
    encodeURIComponent(value ?? 'none')

  return [
    'order_sites_email',
    input.lifecycleKind,
    encode(input.orderId),
    encode(input.assignmentId),
    encode(input.siteId),
    encode(input.eventRef),
  ].join(':')
}

const lifecycleSecretCheckPayload = (
  input: OrderSitesTransactionalEmailInput,
): unknown => ({
  assignmentId: input.assignmentId,
  artifactLabel: input.artifactLabel,
  artifactUrl: input.artifactUrl,
  customerSafeStatus: input.customerSafeStatus,
  displayName: input.displayName,
  eventRef: input.eventRef,
  nextAction: input.nextAction,
  orderId: input.orderId,
  safeReason: input.safeReason,
  revisionUrl: input.revisionUrl,
  siteId: input.siteId,
  siteTitle: input.siteTitle,
  siteUrl: input.siteUrl,
  sourceAuthorityRefs: input.sourceAuthorityRefs,
  statusPageUrl: input.statusPageUrl,
  targetRefs: input.targetRefs,
  to: input.to,
})

const LIFECYCLE_EMAIL_FORBIDDEN_PATTERNS: ReadonlyArray<RegExp> = [
  /provider[_ -]?account/i,
  /auth[_ -]?grant/i,
  /runner raw log/i,
  /private operator note/i,
  /raw exa/i,
  /OPENCODE_AUTH_CONTENT/i,
  /auth\.json/i,
  /access_token/i,
  /refresh_token/i,
  /device_auth_id/i,
  /code_verifier/i,
]

const assertOrderSitesTransactionalEmailInputSafe = (
  input: OrderSitesTransactionalEmailInput,
): Effect.Effect<void, EmailServiceError> => {
  const json = JSON.stringify(lifecycleSecretCheckPayload(input))

  return containsProviderSecretMaterial(json) ||
    LIFECYCLE_EMAIL_FORBIDDEN_PATTERNS.some(pattern => pattern.test(json))
    ? Effect.fail(
        new EmailServiceError({
          message:
            'Order/Sites lifecycle email input contains secret-shaped material.',
          operation: 'EmailService.renderOrderSitesTransactionalEmail',
        }),
      )
    : Effect.void
}

const orderSitesTemplateProps = (
  input: OrderSitesTransactionalEmailInput,
): OrderSitesLifecycleTemplateProps =>
  new OrderSitesLifecycleTemplateProps({
    appOrigin: input.appOrigin,
    artifactLabel: input.artifactLabel,
    artifactUrl: input.artifactUrl,
    customerSafeStatus: input.customerSafeStatus,
    displayName: input.displayName,
    lifecycleKind: input.lifecycleKind,
    nextAction: input.nextAction,
    orderId: input.orderId,
    safeReason: input.safeReason,
    revisionUrl: input.revisionUrl,
    siteTitle: input.siteTitle,
    siteUrl: input.siteUrl,
    ...(input.statusPageUrl === undefined
      ? {}
      : { statusPageUrl: input.statusPageUrl }),
  })

export const orderSitesTransactionalEmailText = (
  input: OrderSitesTransactionalEmailInput,
): string => renderOrderSitesLifecycleEmail(orderSitesTemplateProps(input)).text

export const orderSitesTransactionalEmailHtml = (
  input: OrderSitesTransactionalEmailInput,
): string => renderOrderSitesLifecycleEmail(orderSitesTemplateProps(input)).html

const resendErrorMessage = (payload: ResendSendResponse): string => {
  if (typeof payload.message === 'string' && payload.message.trim() !== '') {
    return compactText(
      redactProviderAccountSecretMaterial(payload.message),
      500,
    )
  }

  return 'Resend email request failed.'
}

const resendErrorName = (payload: ResendSendResponse): string =>
  typeof payload.name === 'string' && payload.name.trim() !== ''
    ? compactText(redactProviderAccountSecretMaterial(payload.name), 120)
    : 'resend_error'

const providerResultToSendEmailResult = (
  result: EmailProviderResult,
): SendEmailResult =>
  result._tag === 'EmailProviderAccepted'
    ? {
        id: result.providerMessageId,
        ok: true,
      }
    : {
        errorMessage: result.errorMessage,
        errorName: result.errorName,
        ok: false,
      }

const renderedOutOfCreditsEmail = (
  config: ResendEmailConfig,
  input: SendOutOfCreditsEmailInput,
): RenderedEmail =>
  new RenderedEmail({
    from: config.fromEmail,
    html: outOfCreditsEmailHtml(input),
    idempotencyKey: input.idempotencyKey,
    kind: 'billing_out_of_credits',
    metadataJson: jsonValue({
      policy: 'system.billing_out_of_credits.v1',
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: 'OpenAgents Autopilot credits exhausted',
    tags: [
      new EmailTag({ name: 'category', value: 'billing' }),
      new EmailTag({ name: 'event', value: 'out_of_credits' }),
    ],
    templateContextJson: jsonValue({
      balanceFormatted: input.balanceFormatted,
      billingUrl: billingUrl(input.appOrigin),
      displayName: input.displayName,
    }),
    templateSlug: 'billing.out_of_credits.v1',
    text: outOfCreditsEmailText(input),
    to: input.to,
  })

const renderedAdjutantCustomerNotificationEmail = (
  config: ResendEmailConfig,
  input: SendAdjutantCustomerNotificationInput,
): RenderedEmail =>
  new RenderedEmail({
    from: config.fromEmail,
    html: adjutantCustomerNotificationEmailHtml(input),
    idempotencyKey: input.idempotencyKey,
    kind: 'operator_notification',
    metadataJson: jsonValue({
      emailSubtype: 'order_sites_lifecycle',
      lifecycleKind: lifecycleKindFromAdjutantStage(input.stage),
      orderId: input.orderId,
      policy: 'system.adjutant.customer_notification.v1',
      siteUrl: input.siteUrl,
      stage: input.stage,
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: adjutantNotificationSubject(input),
    tags: [
      new EmailTag({ name: 'category', value: 'adjutant' }),
      new EmailTag({ name: 'event', value: input.stage }),
    ],
    templateContextJson: jsonValue({
      displayName: input.displayName,
      orderId: input.orderId,
      orderUrl: orderUrl(input.appOrigin),
      siteTitle: input.siteTitle,
      siteUrl: input.siteUrl,
      stage: input.stage,
    }),
    templateSlug: 'adjutant.customer_notification.v1',
    text: adjutantCustomerNotificationEmailText(input),
    to: input.to,
  })

const renderedDripCampaignEmail = (
  config: ResendEmailConfig,
  input: DripCampaignEmailInput,
): RenderedEmail => {
  const renderedTemplate = renderDripEmail(
    new DripTemplateProps({
      appOrigin: input.appOrigin,
      displayName: input.displayName,
      kind: input.kind,
      managePreferencesUrl: input.managePreferencesUrl,
    }),
  )

  return new RenderedEmail({
    from: config.fromEmail,
    html: renderedTemplate.html,
    idempotencyKey: input.idempotencyKey,
    kind: 'crm_transactional',
    metadataJson: jsonValue({
      campaignKind: 'signup_onboarding_drip',
      dripKind: input.kind,
      emailSubtype: 'campaign_drip',
      policy: 'system.email_onboarding_drip.v1',
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: renderedTemplate.subject,
    tags: [
      new EmailTag({ name: 'category', value: 'drip' }),
      new EmailTag({ name: 'event', value: input.kind }),
    ],
    templateContextJson: jsonValue(renderedTemplate.templateContext),
    templateSlug: renderedTemplate.templateSlug,
    text: renderedTemplate.text,
    to: input.to,
  })
}

const autopilotDecisionSecretCheckPayload = (
  input: AutopilotDecisionEmailInput,
): unknown => ({
  displayName: input.displayName,
  kind: input.kind,
  to: input.to,
  workOrderRef: input.workOrderRef,
})

const assertAutopilotDecisionEmailInputSafe = (
  input: AutopilotDecisionEmailInput,
): Effect.Effect<void, EmailServiceError> => {
  const json = JSON.stringify(autopilotDecisionSecretCheckPayload(input))

  return containsProviderSecretMaterial(json) ||
    LIFECYCLE_EMAIL_FORBIDDEN_PATTERNS.some(pattern => pattern.test(json))
    ? Effect.fail(
        new EmailServiceError({
          message:
            'Autopilot decision email input contains secret-shaped material.',
          operation: 'EmailService.renderAutopilotDecisionEmail',
        }),
      )
    : Effect.void
}

const renderedAutopilotDecisionEmail = (
  config: ResendEmailConfig,
  input: AutopilotDecisionEmailInput,
): RenderedEmail => {
  const renderedTemplate = renderAutopilotDecisionEmail(
    new AutopilotDecisionTemplateProps({
      appOrigin: input.appOrigin,
      displayName: input.displayName,
      kind: input.kind,
      workOrderRef: input.workOrderRef,
    }),
  )

  return new RenderedEmail({
    from: config.fromEmail,
    html: renderedTemplate.html,
    idempotencyKey: input.idempotencyKey,
    kind: 'crm_transactional',
    metadataJson: jsonValue({
      decisionKind: input.kind,
      emailSubtype: 'autopilot_decision_queue',
      policy: 'system.autopilot_decision_notification.v1',
      workOrderRef: input.workOrderRef,
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: renderedTemplate.subject,
    tags: [
      new EmailTag({ name: 'category', value: 'autopilot_decisions' }),
      new EmailTag({ name: 'event', value: input.kind }),
    ],
    templateContextJson: jsonValue(renderedTemplate.templateContext),
    templateSlug: renderedTemplate.templateSlug,
    text: renderedTemplate.text,
    to: input.to,
  })
}

const siteReferralOrderUrl = (input: SiteReferralOnboardingEmailInput): string =>
  orderUrl(input.appOrigin)

const siteReferralOnboardingSubject = (
  input: SiteReferralOnboardingEmailInput,
): string => `Start your OpenAgents request from ${input.sourceLabel}`

const siteReferralOnboardingLead = (
  input: SiteReferralOnboardingEmailInput,
): string =>
  `You came to OpenAgents from ${input.sourceLabel}. You can submit a software request, ask for a hosted Site, and review follow-up revisions from your order page.`

export const siteReferralOnboardingEmailText = (
  input: SiteReferralOnboardingEmailInput,
): string =>
  [
    `Hi ${input.displayName},`,
    '',
    siteReferralOnboardingLead(input),
    '',
    input.sourceSiteUrl === null ? null : `Source Site: ${input.sourceSiteUrl}`,
    `Order page: ${siteReferralOrderUrl(input)}`,
    '',
    'OpenAgents',
  ]
    .filter((line): line is string => line !== null)
    .join('\n')

export const siteReferralOnboardingEmailHtml = (
  input: SiteReferralOnboardingEmailInput,
): string => {
  const escapedOrderUrl = escapeHtml(siteReferralOrderUrl(input))
  const escapedSourceSiteUrl =
    input.sourceSiteUrl === null ? null : escapeHtml(input.sourceSiteUrl)

  return `<!doctype html>
<html>
  <head>
    <meta name="color-scheme" content="light" />
    <meta name="supported-color-schemes" content="light" />
  </head>
  <body bgcolor="#fbfaf6" style="margin:0;background:#fbfaf6 !important;color:#17211f !important;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;background:#fbfaf6 !important;color:#17211f !important;">
      <p style="margin:0 0 28px;color:#5e6b68 !important;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#17211f !important;font-size:28px;font-weight:600;line-height:1.2;">${escapeHtml(siteReferralOnboardingSubject(input))}</h1>
      <p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, ${escapeHtml(siteReferralOnboardingLead(input))}</p>
      ${
        escapedSourceSiteUrl === null
          ? ''
          : `<p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Source Site: <a href="${escapedSourceSiteUrl}" style="color:#11384c !important;">${escapedSourceSiteUrl}</a></p>`
      }
      <p style="margin:28px 0 0;">
        <a href="${escapedOrderUrl}" style="display:inline-block;border-radius:999px;background:#11384c !important;color:#ffffff !important;font-size:14px;font-weight:600;text-decoration:none;padding:11px 18px;">Open order page</a>
      </p>
      <p style="margin:24px 0 0;color:#5e6b68 !important;font-size:13px;line-height:1.6;">OpenAgents keeps the current beta focused on reviewable work, clear revisions, and follow-up comments.</p>
    </div>
  </body>
</html>`
}

const SITE_REFERRAL_ONBOARDING_FORBIDDEN_PATTERNS: ReadonlyArray<RegExp> = [
  /provider[_ -]?account/i,
  /auth[_ -]?grant/i,
  /runner raw log/i,
  /private operator note/i,
  /OPENCODE_AUTH_CONTENT/i,
  /auth\.json/i,
  /access_token/i,
  /refresh_token/i,
  /device_auth_id/i,
  /code_verifier/i,
  /token_hash/i,
  /private_key/i,
  /wallet_secret/i,
  /mdk_access_token/i,
]

const assertSiteReferralOnboardingEmailInputSafe = (
  input: SiteReferralOnboardingEmailInput,
): Effect.Effect<void, EmailServiceError> => {
  const json = JSON.stringify(input)

  return containsProviderSecretMaterial(json) ||
    SITE_REFERRAL_ONBOARDING_FORBIDDEN_PATTERNS.some(pattern =>
      pattern.test(json),
    )
    ? Effect.fail(
        new EmailServiceError({
          message:
            'Site referral onboarding email input contains secret-shaped material.',
          operation: 'EmailService.renderSiteReferralOnboardingEmail',
        }),
      )
    : Effect.void
}

const renderedSiteReferralOnboardingEmail = (
  config: ResendEmailConfig,
  input: SiteReferralOnboardingEmailInput,
): RenderedEmail =>
  new RenderedEmail({
    from: config.fromEmail,
    html: siteReferralOnboardingEmailHtml(input),
    idempotencyKey: input.idempotencyKey,
    kind: 'crm_transactional',
    metadataJson: jsonValue({
      emailSubtype: 'site_referral_onboarding',
      policy: 'system.site_referral_onboarding.v1',
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: siteReferralOnboardingSubject(input),
    tags: [
      new EmailTag({ name: 'category', value: 'referral' }),
      new EmailTag({ name: 'event', value: 'referred_signup' }),
    ],
    templateContextJson: jsonValue({
      displayName: input.displayName,
      orderUrl: siteReferralOrderUrl(input),
      sourceLabel: input.sourceLabel,
      sourceSiteUrl: input.sourceSiteUrl,
    }),
    templateSlug: 'site_referral.onboarding.v1',
    text: siteReferralOnboardingEmailText(input),
    to: input.to,
  })

const targetedRemakeOutreachSubject = (
  input: TargetedRemakeOutreachEmailInput,
): string => `A concept Site for ${input.targetName}`

const targetedRemakeOutreachLead = (
  input: TargetedRemakeOutreachEmailInput,
): string =>
  `OpenAgents prepared a concept Site preview for ${input.targetName} at ${input.targetDomain}. ${input.valueProposition}`

export const targetedRemakeOutreachEmailText = (
  input: TargetedRemakeOutreachEmailInput,
): string =>
  [
    `Hi ${input.displayName},`,
    '',
    targetedRemakeOutreachLead(input),
    '',
    input.conceptDisclosure,
    '',
    `Preview: ${input.previewUrl}`,
    `Book a review: ${input.meetingUrl}`,
    '',
    `${input.senderName}`,
    input.senderContact,
    input.postalAddress,
    '',
    `Unsubscribe: ${input.unsubscribeUrl}`,
    `Manage preferences: ${input.preferencesUrl}`,
    '',
    'OpenAgents',
  ].join('\n')

export const targetedRemakeOutreachEmailHtml = (
  input: TargetedRemakeOutreachEmailInput,
): string => {
  const escapedPreviewUrl = escapeHtml(input.previewUrl)
  const escapedMeetingUrl = escapeHtml(input.meetingUrl)
  const escapedUnsubscribeUrl = escapeHtml(input.unsubscribeUrl)
  const escapedPreferencesUrl = escapeHtml(input.preferencesUrl)

  return `<!doctype html>
<html>
  <head>
    <meta name="color-scheme" content="light" />
    <meta name="supported-color-schemes" content="light" />
  </head>
  <body bgcolor="#fbfaf6" style="margin:0;background:#fbfaf6 !important;color:#17211f !important;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:600px;margin:0 auto;padding:40px 24px;background:#fbfaf6 !important;color:#17211f !important;">
      <p style="margin:0 0 28px;color:#5e6b68 !important;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#17211f !important;font-size:28px;font-weight:600;line-height:1.2;">${escapeHtml(targetedRemakeOutreachSubject(input))}</h1>
      <p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, ${escapeHtml(targetedRemakeOutreachLead(input))}</p>
      <p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">${escapeHtml(input.conceptDisclosure)}</p>
      <p style="margin:28px 0 0;">
        <a href="${escapedPreviewUrl}" style="display:inline-block;border-radius:999px;background:#11384c !important;color:#ffffff !important;font-size:14px;font-weight:600;text-decoration:none;padding:11px 18px;">Open concept preview</a>
      </p>
      <p style="margin:16px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Preview: <a href="${escapedPreviewUrl}" style="color:#11384c !important;">${escapedPreviewUrl}</a></p>
      <p style="margin:10px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Book a review: <a href="${escapedMeetingUrl}" style="color:#11384c !important;">${escapedMeetingUrl}</a></p>
      <p style="margin:28px 0 0;color:#5e6b68 !important;font-size:13px;line-height:1.6;">${escapeHtml(input.senderName)}<br>${escapeHtml(input.senderContact)}<br>${escapeHtml(input.postalAddress)}</p>
      <p style="margin:20px 0 0;color:#5e6b68 !important;font-size:12px;line-height:1.6;"><a href="${escapedUnsubscribeUrl}" style="color:#5e6b68 !important;">Unsubscribe</a> · <a href="${escapedPreferencesUrl}" style="color:#5e6b68 !important;">Manage preferences</a></p>
    </div>
  </body>
</html>`
}

const TARGETED_REMAKE_OUTREACH_FORBIDDEN_PATTERNS: ReadonlyArray<RegExp> = [
  /provider[_ -]?account/i,
  /provider[_ -]?payload/i,
  /raw[_ -]?payload/i,
  /browser[_ -]?log/i,
  /auth[_ -]?grant/i,
  /runner raw log/i,
  /private operator note/i,
  /OPENCODE_AUTH_CONTENT/i,
  /auth\.json/i,
  /access_token/i,
  /refresh_token/i,
  /device_auth_id/i,
  /code_verifier/i,
  /token_hash/i,
  /private_key/i,
  /wallet_secret/i,
  /mdk_access_token/i,
  /payment_preimage/i,
  /payment_secret/i,
  /bypass/i,
  /captcha/i,
  /headless stealth/i,
]

const assertTargetedRemakeOutreachEmailInputSafe = (
  input: TargetedRemakeOutreachEmailInput,
): Effect.Effect<void, EmailServiceError> => {
  const json = JSON.stringify(input)

  return containsProviderSecretMaterial(json) ||
    TARGETED_REMAKE_OUTREACH_FORBIDDEN_PATTERNS.some(pattern =>
      pattern.test(json),
    )
    ? Effect.fail(
        new EmailServiceError({
          message:
            'Targeted remake outreach email input contains secret-shaped material.',
          operation: 'EmailService.renderTargetedRemakeOutreachEmail',
        }),
      )
    : Effect.void
}

const renderedTargetedRemakeOutreachEmail = (
  config: ResendEmailConfig,
  input: TargetedRemakeOutreachEmailInput,
): RenderedEmail =>
  new RenderedEmail({
    from: config.fromEmail,
    html: targetedRemakeOutreachEmailHtml(input),
    idempotencyKey: input.idempotencyKey,
    kind: 'operator_notification',
    metadataJson: jsonValue({
      campaignId: input.campaignId,
      emailSubtype: 'targeted_remake_outreach',
      policy: 'system.targeted_remake_outreach_email.v1',
      previewGenerationId: input.previewGenerationId,
      prospectId: input.prospectId,
      targetDomain: input.targetDomain,
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: targetedRemakeOutreachSubject(input),
    tags: [
      new EmailTag({ name: 'category', value: 'targeted_remake' }),
      new EmailTag({ name: 'event', value: 'concept_preview_outreach' }),
    ],
    templateContextJson: jsonValue({
      appOrigin: input.appOrigin,
      campaignId: input.campaignId,
      conceptDisclosure: input.conceptDisclosure,
      displayName: input.displayName,
      meetingUrl: input.meetingUrl,
      preferencesUrl: input.preferencesUrl,
      previewGenerationId: input.previewGenerationId,
      previewUrl: input.previewUrl,
      prospectId: input.prospectId,
      senderContact: input.senderContact,
      senderName: input.senderName,
      targetDomain: input.targetDomain,
      targetName: input.targetName,
      unsubscribeUrl: input.unsubscribeUrl,
    }),
    templateSlug: 'targeted_remake.outreach.v1',
    text: targetedRemakeOutreachEmailText(input),
    to: input.to,
  })

const renderedPrivateWorkspaceInviteEmail = (
  config: ResendEmailConfig,
  input: PrivateWorkspaceInviteEmailInput,
): RenderedEmail => {
  const renderedTemplate = renderPrivateWorkspaceInviteEmail(
    new PrivateWorkspaceInviteTemplateProps({
      acceptUrl: input.acceptUrl,
      displayName: input.displayName,
      expiresAt: input.expiresAt,
      workspaceLabel: input.workspaceLabel,
    }),
  )

  return new RenderedEmail({
    from: config.fromEmail,
    html: renderedTemplate.html,
    idempotencyKey: input.idempotencyKey,
    kind: 'operator_notification',
    metadataJson: jsonValue({
      emailSubtype: 'private_workspace_invite',
      inviteId: input.inviteId,
      policy: 'system.private_workspace_invite_email.v1',
      projectId: input.projectId,
      teamId: input.teamId,
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: renderedTemplate.subject,
    tags: [
      new EmailTag({ name: 'category', value: 'workspace' }),
      new EmailTag({ name: 'event', value: 'private_invite' }),
    ],
    templateContextJson: jsonValue(renderedTemplate.templateContext),
    templateSlug: renderedTemplate.templateSlug,
    text: renderedTemplate.text,
    to: input.to,
  })
}

const renderedOrderSitesTransactionalEmail = (
  config: ResendEmailConfig,
  input: OrderSitesTransactionalEmailInput,
): RenderedEmail => {
  const idempotencyKey =
    input.idempotencyKey ??
    buildOrderSitesTransactionalEmailIdempotencyKey(input)
  const renderedTemplate = renderOrderSitesLifecycleEmail(
    orderSitesTemplateProps(input),
  )
  const sourceAuthorityRefs = input.sourceAuthorityRefs ?? []
  const targetRefs = input.targetRefs ?? []

  return new RenderedEmail({
    from: config.fromEmail,
    html: renderedTemplate.html,
    idempotencyKey,
    kind: 'operator_notification',
    metadataJson: jsonValue({
      assignmentId: input.assignmentId,
      artifactLabel: input.artifactLabel,
      artifactUrl: input.artifactUrl,
      emailSubtype: 'order_sites_lifecycle',
      eventRef: input.eventRef,
      lifecycleKind: input.lifecycleKind,
      orderId: input.orderId,
      policy: 'system.order_sites_lifecycle_email.v1',
      safeReason: input.safeReason,
      siteId: input.siteId,
      siteUrl: input.siteUrl,
      sourceAuthorityRefs,
      targetRefs,
    }),
    ...(config.replyToEmail === undefined
      ? {}
      : { replyTo: config.replyToEmail }),
    subject: renderedTemplate.subject,
    tags: [
      new EmailTag({ name: 'category', value: 'order_sites' }),
      new EmailTag({ name: 'event', value: input.lifecycleKind }),
    ],
    templateContextJson: jsonValue({
      ...renderedTemplate.templateContext,
      assignmentId: input.assignmentId,
      eventRef: input.eventRef,
      siteId: input.siteId,
      sourceAuthorityRefs,
      targetRefs,
    }),
    templateSlug: renderedTemplate.templateSlug,
    text: renderedTemplate.text,
    to: input.to,
  })
}

const lifecycleKindFromAdjutantStage = (
  stage: AdjutantCustomerNotificationStage,
): OrderSitesTransactionalEmailKind =>
  stage === 'deployed'
    ? 'site_deployed'
    : stage === 'input_needed'
      ? 'customer_input_needed'
      : stage === 'review_ready'
        ? 'review_ready'
        : 'unavailable_declined'

const operatorSmokeNotificationInput = (
  input: OperatorEmailLedgerSmokeInput,
): SendAdjutantCustomerNotificationInput => ({
  appOrigin: input.appOrigin,
  displayName: 'OpenAgents Operator',
  idempotencyKey: input.idempotencyKey,
  orderId: 'operator_email_smoke',
  siteTitle: 'OpenAgents email smoke',
  siteUrl: null,
  stage: 'review_ready',
  to: input.to,
})

const missingResendSmokeConfig = (): ResendEmailConfig => ({
  apiKey: Redacted.make(WorkerSecret.make('resend_config_missing')),
  fromEmail: ResendEmailSender.make(
    'OpenAgents <email-config-missing@openagents.local>',
  ),
})

const emailMessageRecordFromRow = (row: EmailMessageRow): EmailMessageRecord =>
  ({
    createdAt: row.created_at,
    errorMessage: row.error_message,
    errorName: row.error_name,
    id: EmailMessageId.make(row.id),
    idempotencyKey: row.idempotency_key,
    kind: row.kind,
    provider: row.provider,
    providerMessageId: row.provider_message_id,
    status: row.status,
    updatedAt: row.updated_at,
  }) satisfies EmailMessageRecord

const reserveEmailMessage = (
  db: D1Database,
  rendered: RenderedEmail,
  context: EmailIntentContext | undefined = undefined,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailMessageRecord, EmailServiceError> =>
  Effect.tryPromise({
    try: async () => {
      const now = runtime.nowIso()
      const id = runtime.randomId('email_msg')
      const metadataJson = jsonValue({
        ...(context?.metadata ?? {}),
        rendered: parseJsonRecord(rendered.metadataJson) ?? {},
      })

      await db
        .prepare(
          `INSERT INTO email_messages
            (id, kind, actor_user_id, target_user_id, to_email, from_email,
             reply_to_email, subject, text_body, html_body, template_slug,
             template_context_json, status, idempotency_key,
             source_authority_ref, action_submission_id, metadata_json,
             created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'rendered', ?, ?, ?, ?, ?, ?)
           ON CONFLICT(idempotency_key) DO NOTHING`,
        )
        .bind(
          id,
          rendered.kind,
          context?.actorUserId ?? null,
          context?.targetUserId ?? null,
          rendered.to,
          rendered.from,
          rendered.replyTo ?? null,
          rendered.subject,
          rendered.text,
          rendered.html,
          rendered.templateSlug,
          rendered.templateContextJson,
          rendered.idempotencyKey,
          context?.sourceAuthorityRef ?? 'system.email.policy.v1',
          context?.actionSubmissionId ?? null,
          metadataJson,
          now,
          now,
        )
        .run()

      await db
        .prepare(
          `UPDATE email_messages
           SET kind = ?,
               actor_user_id = ?,
               target_user_id = ?,
               to_email = ?,
               from_email = ?,
               reply_to_email = ?,
               subject = ?,
               text_body = ?,
               html_body = ?,
               template_slug = ?,
               template_context_json = ?,
               status = 'rendered',
               source_authority_ref = ?,
               action_submission_id = ?,
               metadata_json = ?,
               error_name = NULL,
               error_message = NULL,
               updated_at = ?
           WHERE idempotency_key = ? AND status != 'accepted'`,
        )
        .bind(
          rendered.kind,
          context?.actorUserId ?? null,
          context?.targetUserId ?? null,
          rendered.to,
          rendered.from,
          rendered.replyTo ?? null,
          rendered.subject,
          rendered.text,
          rendered.html,
          rendered.templateSlug,
          rendered.templateContextJson,
          context?.sourceAuthorityRef ?? 'system.email.policy.v1',
          context?.actionSubmissionId ?? null,
          metadataJson,
          now,
          rendered.idempotencyKey,
        )
        .run()

      const row = await db
        .prepare(
          `SELECT id, kind, status, provider, provider_message_id,
                  error_name, error_message, idempotency_key, created_at,
                  updated_at
           FROM email_messages
           WHERE idempotency_key = ?`,
        )
        .bind(rendered.idempotencyKey)
        .first<EmailMessageRow>()

      return row
    },
    catch: error => emailServiceError('EmailService.reserveMessage', error),
  }).pipe(
    Effect.flatMap(row =>
      row === null
        ? Effect.fail(
            new EmailServiceError({
              message: 'Email message reservation was not persisted.',
              operation: 'EmailService.reserveMessage',
            }),
          )
        : Effect.succeed(emailMessageRecordFromRow(row)),
    ),
    Effect.withSpan('EmailService.reserveMessage'),
  )

const markEmailMessageAccepted = (
  db: D1Database,
  messageId: EmailMessageId,
  provider: EmailProvider,
  providerMessageId: string | null,
  runtime: EmailRuntime,
): Effect.Effect<void, EmailServiceError> =>
  Effect.tryPromise({
    try: async () => {
      await db
        .prepare(
          `UPDATE email_messages
           SET status = 'accepted',
               provider = ?,
               provider_message_id = ?,
               error_name = NULL,
               error_message = NULL,
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(provider, providerMessageId, runtime.nowIso(), messageId)
        .run()
    },
    catch: error =>
      emailServiceError('EmailService.markMessageAccepted', error),
  }).pipe(Effect.withSpan('EmailService.markMessageAccepted'))

const markEmailMessageFailed = (
  db: D1Database,
  messageId: EmailMessageId,
  provider: EmailProvider,
  result: EmailProviderRejected,
  runtime: EmailRuntime,
): Effect.Effect<void, EmailServiceError> =>
  Effect.tryPromise({
    try: async () => {
      await db
        .prepare(
          `UPDATE email_messages
           SET status = 'failed',
               provider = ?,
               provider_message_id = NULL,
               error_name = ?,
               error_message = ?,
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(
          provider,
          compactText(result.errorName, 120),
          compactText(result.errorMessage, 500),
          runtime.nowIso(),
          messageId,
        )
        .run()
    },
    catch: error => emailServiceError('EmailService.markMessageFailed', error),
  }).pipe(Effect.withSpan('EmailService.markMessageFailed'))

const recordEmailDelivery = (
  db: D1Database,
  messageId: EmailMessageId,
  rendered: RenderedEmail,
  result: EmailProviderResult,
  runtime: EmailRuntime,
): Effect.Effect<void, EmailServiceError> =>
  Effect.tryPromise({
    try: async () => {
      const now = runtime.nowIso()
      const accepted = result._tag === 'EmailProviderAccepted'
      const providerMessageId = accepted ? result.providerMessageId : null
      const errorName = accepted ? null : compactText(result.errorName, 120)
      const errorMessage = accepted
        ? null
        : compactText(result.errorMessage, 500)

      await db
        .prepare(
          `INSERT INTO email_deliveries
            (id, message_id, provider, provider_message_id,
             provider_thread_id, provider_request_id, provider_idempotency_key,
             status, error_name, error_message, provider_payload_summary_json,
             attempted_at, completed_at, created_at, updated_at)
           VALUES (?, ?, ?, ?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          runtime.randomId('email_del'),
          messageId,
          result.provider,
          providerMessageId,
          rendered.idempotencyKey,
          accepted ? 'accepted' : 'failed',
          errorName,
          errorMessage,
          jsonValue(
            accepted
              ? { providerMessageId }
              : { errorName, errorMessage, provider: result.provider },
          ),
          now,
          now,
          now,
          now,
        )
        .run()
    },
    catch: error => emailServiceError('EmailService.recordDelivery', error),
  }).pipe(Effect.withSpan('EmailService.recordDelivery'))

const recordDraft = (
  db: D1Database,
  input: Readonly<{
    context?: EmailIntentContext | undefined
    provider: EmailProvider
    providerDraftId: string
    providerMessageId?: string | undefined
    providerThreadId?: string | undefined
    rendered: RenderedEmail
  }>,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailMessageRecord, EmailServiceError> =>
  Effect.gen(function* () {
    const message = yield* reserveEmailMessage(
      db,
      input.rendered,
      input.context,
      runtime,
    )
    const now = runtime.nowIso()

    yield* Effect.tryPromise({
      try: async () => {
        await db
          .prepare(
            `UPDATE email_messages
             SET status = 'draft_recorded',
                 provider = ?,
                 provider_draft_id = ?,
                 provider_message_id = ?,
                 provider_thread_id = ?,
                 error_name = NULL,
                 error_message = NULL,
                 updated_at = ?
             WHERE id = ?`,
          )
          .bind(
            input.provider,
            input.providerDraftId,
            input.providerMessageId ?? null,
            input.providerThreadId ?? null,
            now,
            message.id,
          )
          .run()

        await db
          .prepare(
            `INSERT INTO email_drafts
              (id, message_id, provider, provider_draft_id,
               provider_message_id, provider_thread_id, status,
               provenance_json, created_at, updated_at, sent_at)
             VALUES (?, ?, ?, ?, ?, ?, 'draft_created', ?, ?, ?, NULL)
             ON CONFLICT(provider, provider_draft_id) DO UPDATE
             SET message_id = excluded.message_id,
                 provider_message_id = excluded.provider_message_id,
                 provider_thread_id = excluded.provider_thread_id,
                 status = 'draft_created',
                 provenance_json = excluded.provenance_json,
                 updated_at = excluded.updated_at`,
          )
          .bind(
            runtime.randomId('email_draft'),
            message.id,
            input.provider,
            input.providerDraftId,
            input.providerMessageId ?? null,
            input.providerThreadId ?? null,
            jsonValue(input.context?.metadata ?? {}),
            now,
            now,
          )
          .run()
      },
      catch: error => emailServiceError('EmailService.recordDraft', error),
    })

    const record: EmailMessageRecord = {
      ...message,
      provider: input.provider,
      providerMessageId: input.providerMessageId ?? null,
      status: 'draft_recorded',
      updatedAt: now,
    }

    return record
  }).pipe(Effect.withSpan('EmailService.recordDraft'))

const sendRenderedEmailToResend = (
  config: ResendEmailConfig,
  rendered: RenderedEmail,
  fetcher: typeof fetch = fetch,
): Effect.Effect<EmailProviderResult> =>
  Effect.tryPromise({
    try: async () => {
      const apiKey = Redacted.value(config.apiKey)
      const response = await fetcher('https://api.resend.com/emails', {
        body: JSON.stringify({
          from: rendered.from,
          html: rendered.html,
          ...(rendered.replyTo === undefined
            ? {}
            : { reply_to: rendered.replyTo }),
          subject: rendered.subject,
          tags: rendered.tags.map(tag => ({
            name: tag.name,
            value: tag.value,
          })),
          text: rendered.text,
          to: [rendered.to],
        }),
        headers: {
          Authorization: `Bearer ${apiKey}`,
          'content-type': 'application/json',
          'Idempotency-Key': rendered.idempotencyKey,
        },
        method: 'POST',
      })
      const text = await response.text()
      const payload = decodeResendSendResponse(text)

      if (!response.ok) {
        return new EmailProviderRejected({
          errorMessage: resendErrorMessage(payload),
          errorName: resendErrorName(payload),
          provider: 'resend',
        })
      }

      return new EmailProviderAccepted({
        provider: 'resend',
        providerMessageId: typeof payload.id === 'string' ? payload.id : null,
      })
    },
    catch: error =>
      new EmailProviderRejected({
        errorMessage: error instanceof Error ? error.message : String(error),
        errorName: error instanceof Error ? error.name : 'resend_fetch_error',
        provider: 'resend',
      }),
  }).pipe(
    Effect.catch(error => Effect.succeed(error)),
    Effect.withSpan('EmailService.sendRenderedEmail'),
  )

const cloudflareEmailProviderMessageId = (response: unknown): string | null => {
  if (response === null || typeof response !== 'object') {
    return null
  }

  const record = response as Readonly<Record<string, unknown>>
  const messageId = record.messageId ?? record.id

  return typeof messageId === 'string' && messageId.trim() !== ''
    ? messageId
    : null
}

const cloudflareEmailErrorName = (error: unknown): string =>
  error !== null &&
  typeof error === 'object' &&
  'code' in error &&
  typeof (error as Readonly<{ code?: unknown }>).code === 'string'
    ? compactText(String((error as Readonly<{ code: string }>).code), 120)
    : error instanceof Error
      ? compactText(error.name, 120)
      : 'cloudflare_email_send_error'

const cloudflareEmailErrorMessage = (error: unknown): string =>
  error instanceof Error
    ? compactText(error.message, 500)
    : compactText(String(error), 500)

const renderedEmailToCloudflareMessage = (
  rendered: RenderedEmail,
): CloudflareEmailSendMessage => ({
  from: rendered.from,
  headers: {
    'X-OpenAgents-Idempotency-Key': compactText(
      rendered.idempotencyKey,
      2048,
    ),
  },
  html: rendered.html,
  ...(rendered.replyTo === undefined ? {} : { replyTo: rendered.replyTo }),
  subject: rendered.subject,
  text: rendered.text,
  to: rendered.to,
})

const sendRenderedEmailViaCloudflareBindingEffect = (
  binding: CloudflareEmailBinding,
  rendered: RenderedEmail,
): Effect.Effect<EmailProviderResult> =>
  Effect.tryPromise({
    try: async () => {
      const response = await binding.send(
        renderedEmailToCloudflareMessage(rendered),
      )

      return new EmailProviderAccepted({
        provider: 'cloudflare_email',
        providerMessageId: cloudflareEmailProviderMessageId(response),
      })
    },
    catch: error =>
      new EmailProviderRejected({
        errorMessage: cloudflareEmailErrorMessage(error),
        errorName: cloudflareEmailErrorName(error),
        provider: 'cloudflare_email',
      }),
  }).pipe(
    Effect.catch(error => Effect.succeed(error)),
    Effect.withSpan('EmailService.sendRenderedEmailViaCloudflareBinding'),
  )

const sendRenderedEmailViaCloudflareBindingWithLedgerEffect = (
  db: D1Database,
  binding: CloudflareEmailBinding,
  rendered: RenderedEmail,
  context: EmailIntentContext | undefined = undefined,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  Effect.gen(function* () {
    const message = yield* reserveEmailMessage(db, rendered, context, runtime)

    if (message.status === 'accepted') {
      return {
        emailMessageId: message.id,
        ok: true as const,
        providerMessageId: message.providerMessageId,
      }
    }

    const result = yield* sendRenderedEmailViaCloudflareBindingEffect(
      binding,
      rendered,
    )

    yield* result._tag === 'EmailProviderAccepted'
      ? markEmailMessageAccepted(
          db,
          message.id,
          result.provider,
          result.providerMessageId,
          runtime,
        )
      : markEmailMessageFailed(
          db,
          message.id,
          result.provider,
          result,
          runtime,
        )
    yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

    if (result._tag === 'EmailProviderAccepted') {
      return {
        emailMessageId: message.id,
        ok: true as const,
        providerMessageId: result.providerMessageId,
      }
    }

    return {
      emailMessageId: message.id,
      errorMessage: result.errorMessage,
      errorName: result.errorName,
      ok: false as const,
    }
  }).pipe(
    Effect.withSpan(
      'EmailService.sendRenderedEmailViaCloudflareBindingWithLedger',
    ),
  )

export const makeEmailService = (): EmailServiceShape => {
  const renderOutOfCreditsEmail = Effect.fn(
    'EmailService.renderOutOfCreditsEmail',
  )((config: ResendEmailConfig, input: SendOutOfCreditsEmailInput) =>
    Effect.succeed(renderedOutOfCreditsEmail(config, input)),
  )

  const renderAdjutantCustomerNotificationEmail = Effect.fn(
    'EmailService.renderAdjutantCustomerNotificationEmail',
  )((config: ResendEmailConfig, input: SendAdjutantCustomerNotificationInput) =>
    Effect.succeed(renderedAdjutantCustomerNotificationEmail(config, input)),
  )

  const renderDripCampaignEmail = Effect.fn(
    'EmailService.renderDripCampaignEmail',
  )((config: ResendEmailConfig, input: DripCampaignEmailInput) =>
    Effect.succeed(renderedDripCampaignEmail(config, input)),
  )

  const renderAutopilotDecisionNotificationEmail = Effect.fn(
    'EmailService.renderAutopilotDecisionNotificationEmail',
  )((config: ResendEmailConfig, input: AutopilotDecisionEmailInput) =>
    Effect.gen(function* () {
      yield* assertAutopilotDecisionEmailInputSafe(input)

      return renderedAutopilotDecisionEmail(config, input)
    }),
  )

  const renderSiteReferralOnboardingEmail = Effect.fn(
    'EmailService.renderSiteReferralOnboardingEmail',
  )((config: ResendEmailConfig, input: SiteReferralOnboardingEmailInput) =>
    Effect.gen(function* () {
      yield* assertSiteReferralOnboardingEmailInputSafe(input)

      return renderedSiteReferralOnboardingEmail(config, input)
    }),
  )

  const renderOrderSitesTransactionalEmail = Effect.fn(
    'EmailService.renderOrderSitesTransactionalEmail',
  )((config: ResendEmailConfig, input: OrderSitesTransactionalEmailInput) =>
    Effect.gen(function* () {
      yield* assertOrderSitesTransactionalEmailInputSafe(input)

      return renderedOrderSitesTransactionalEmail(config, input)
    }),
  )

  const renderTargetedRemakeOutreachEmail = Effect.fn(
    'EmailService.renderTargetedRemakeOutreachEmail',
  )((config: ResendEmailConfig, input: TargetedRemakeOutreachEmailInput) =>
    Effect.gen(function* () {
      yield* assertTargetedRemakeOutreachEmailInputSafe(input)

      return renderedTargetedRemakeOutreachEmail(config, input)
    }),
  )

  const renderPrivateWorkspaceInviteEmail = Effect.fn(
    'EmailService.renderPrivateWorkspaceInviteEmail',
  )((config: ResendEmailConfig, input: PrivateWorkspaceInviteEmailInput) =>
    Effect.succeed(renderedPrivateWorkspaceInviteEmail(config, input)),
  )

  const sendRenderedEmail = Effect.fn('EmailService.sendRenderedEmail')(
    sendRenderedEmailToResend,
  )

  const sendOutOfCreditsEmailEffect = Effect.fn(
    'EmailService.sendOutOfCreditsEmail',
  )(
    (
      config: ResendEmailConfig,
      input: SendOutOfCreditsEmailInput,
      fetcher?: typeof fetch,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderOutOfCreditsEmail(config, input)
        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        return providerResultToSendEmailResult(result)
      }),
  )

  const sendOutOfCreditsEmailWithLedger = Effect.fn(
    'EmailService.sendOutOfCreditsEmailWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: SendOutOfCreditsEmailInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderOutOfCreditsEmail(config, input)
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            sourceAuthorityRef:
              context?.sourceAuthorityRef ?? 'system.billing_out_of_credits.v1',
          },
          runtime,
        )
        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        if (result._tag === 'EmailProviderAccepted') {
          yield* markEmailMessageAccepted(
            db,
            message.id,
            result.provider,
            result.providerMessageId,
            runtime,
          )
        } else {
          yield* markEmailMessageFailed(
            db,
            message.id,
            result.provider,
            result,
            runtime,
          )
        }
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        return providerResultToSendEmailResult(result)
      }),
  )

  const sendAdjutantCustomerNotificationWithLedger = Effect.fn(
    'EmailService.sendAdjutantCustomerNotificationWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: SendAdjutantCustomerNotificationInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderAdjutantCustomerNotificationEmail(
          config,
          input,
        )
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            sourceAuthorityRef:
              context?.sourceAuthorityRef ??
              'system.adjutant.customer_notification.v1',
          },
          runtime,
        )

        if (message.status === 'accepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: message.providerMessageId,
          }
        }

        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        yield* result._tag === 'EmailProviderAccepted'
          ? markEmailMessageAccepted(
              db,
              message.id,
              result.provider,
              result.providerMessageId,
              runtime,
            )
          : markEmailMessageFailed(
              db,
              message.id,
              result.provider,
              result,
              runtime,
            )
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        if (result._tag === 'EmailProviderAccepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: result.providerMessageId,
          }
        }

        return {
          emailMessageId: message.id,
          errorMessage: result.errorMessage,
          errorName: result.errorName,
          ok: false as const,
        }
      }),
  )

  const sendDripCampaignEmailWithLedger = Effect.fn(
    'EmailService.sendDripCampaignEmailWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: DripCampaignEmailInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderDripCampaignEmail(config, input)
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            metadata: {
              ...(context?.metadata ?? {}),
              dripKind: input.kind,
            },
            sourceAuthorityRef:
              context?.sourceAuthorityRef ?? 'system.email_onboarding_drip.v1',
          },
          runtime,
        )

        if (message.status === 'accepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: message.providerMessageId,
          }
        }

        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        yield* result._tag === 'EmailProviderAccepted'
          ? markEmailMessageAccepted(
              db,
              message.id,
              result.provider,
              result.providerMessageId,
              runtime,
            )
          : markEmailMessageFailed(
              db,
              message.id,
              result.provider,
              result,
              runtime,
            )
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        if (result._tag === 'EmailProviderAccepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: result.providerMessageId,
          }
        }

        return {
          emailMessageId: message.id,
          errorMessage: result.errorMessage,
          errorName: result.errorName,
          ok: false as const,
        }
      }),
  )

  const sendSiteReferralOnboardingEmailWithLedger = Effect.fn(
    'EmailService.sendSiteReferralOnboardingEmailWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: SiteReferralOnboardingEmailInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderSiteReferralOnboardingEmail(config, input)
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            metadata: {
              ...(context?.metadata ?? {}),
              emailSubtype: 'site_referral_onboarding',
            },
            sourceAuthorityRef:
              context?.sourceAuthorityRef ??
              'system.site_referral_onboarding.v1',
          },
          runtime,
        )

        if (message.status === 'accepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: message.providerMessageId,
          }
        }

        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        yield* result._tag === 'EmailProviderAccepted'
          ? markEmailMessageAccepted(
              db,
              message.id,
              result.provider,
              result.providerMessageId,
              runtime,
            )
          : markEmailMessageFailed(
              db,
              message.id,
              result.provider,
              result,
              runtime,
            )
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        if (result._tag === 'EmailProviderAccepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: result.providerMessageId,
          }
        }

        return {
          emailMessageId: message.id,
          errorMessage: result.errorMessage,
          errorName: result.errorName,
          ok: false as const,
        }
      }),
  )

  const sendAutopilotDecisionEmailWithLedger = Effect.fn(
    'EmailService.sendAutopilotDecisionEmailWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: AutopilotDecisionEmailInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderAutopilotDecisionNotificationEmail(
          config,
          input,
        )
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            metadata: {
              ...(context?.metadata ?? {}),
              decisionKind: input.kind,
              workOrderRef: input.workOrderRef,
            },
            sourceAuthorityRef:
              context?.sourceAuthorityRef ??
              'system.autopilot_decision_notification.v1',
          },
          runtime,
        )

        if (message.status === 'accepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: message.providerMessageId,
          }
        }

        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        yield* result._tag === 'EmailProviderAccepted'
          ? markEmailMessageAccepted(
              db,
              message.id,
              result.provider,
              result.providerMessageId,
              runtime,
            )
          : markEmailMessageFailed(
              db,
              message.id,
              result.provider,
              result,
              runtime,
            )
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        if (result._tag === 'EmailProviderAccepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: result.providerMessageId,
          }
        }

        return {
          emailMessageId: message.id,
          errorMessage: result.errorMessage,
          errorName: result.errorName,
          ok: false as const,
        }
      }),
  )

  const sendOrderSitesTransactionalEmailWithLedger = Effect.fn(
    'EmailService.sendOrderSitesTransactionalEmailWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: OrderSitesTransactionalEmailInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderOrderSitesTransactionalEmail(
          config,
          input,
        )
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            metadata: {
              ...(context?.metadata ?? {}),
              lifecycleKind: input.lifecycleKind,
            },
            sourceAuthorityRef:
              context?.sourceAuthorityRef ??
              'system.order_sites_lifecycle_email.v1',
          },
          runtime,
        )

        if (message.status === 'accepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: message.providerMessageId,
          }
        }

        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        yield* result._tag === 'EmailProviderAccepted'
          ? markEmailMessageAccepted(
              db,
              message.id,
              result.provider,
              result.providerMessageId,
              runtime,
            )
          : markEmailMessageFailed(
              db,
              message.id,
              result.provider,
              result,
              runtime,
            )
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        if (result._tag === 'EmailProviderAccepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: result.providerMessageId,
          }
        }

        return {
          emailMessageId: message.id,
          errorMessage: result.errorMessage,
          errorName: result.errorName,
          ok: false as const,
        }
      }),
  )

  const sendTargetedRemakeOutreachEmailWithLedger = Effect.fn(
    'EmailService.sendTargetedRemakeOutreachEmailWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: TargetedRemakeOutreachEmailInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderTargetedRemakeOutreachEmail(config, input)
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            metadata: {
              ...(context?.metadata ?? {}),
              campaignId: input.campaignId,
              emailSubtype: 'targeted_remake_outreach',
              previewGenerationId: input.previewGenerationId,
              prospectId: input.prospectId,
            },
            sourceAuthorityRef:
              context?.sourceAuthorityRef ??
              'system.targeted_remake_outreach_email.v1',
          },
          runtime,
        )

        if (message.status === 'accepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: message.providerMessageId,
          }
        }

        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        yield* result._tag === 'EmailProviderAccepted'
          ? markEmailMessageAccepted(
              db,
              message.id,
              result.provider,
              result.providerMessageId,
              runtime,
            )
          : markEmailMessageFailed(
              db,
              message.id,
              result.provider,
              result,
              runtime,
            )
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        if (result._tag === 'EmailProviderAccepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: result.providerMessageId,
          }
        }

        return {
          emailMessageId: message.id,
          errorMessage: result.errorMessage,
          errorName: result.errorName,
          ok: false as const,
        }
      }),
  )

  const sendPrivateWorkspaceInviteEmailWithLedger = Effect.fn(
    'EmailService.sendPrivateWorkspaceInviteEmailWithLedger',
  )(
    (
      db: D1Database,
      config: ResendEmailConfig,
      input: PrivateWorkspaceInviteEmailInput,
      context: EmailIntentContext | undefined = undefined,
      fetcher: typeof fetch = fetch,
      runtime: EmailRuntime = systemEmailRuntime,
    ) =>
      Effect.gen(function* () {
        const rendered = yield* renderPrivateWorkspaceInviteEmail(config, input)
        const message = yield* reserveEmailMessage(
          db,
          rendered,
          {
            ...context,
            metadata: {
              ...(context?.metadata ?? {}),
              emailSubtype: 'private_workspace_invite',
              inviteId: input.inviteId,
              projectId: input.projectId,
              teamId: input.teamId,
            },
            sourceAuthorityRef:
              context?.sourceAuthorityRef ??
              'system.private_workspace_invite_email.v1',
          },
          runtime,
        )

        if (message.status === 'accepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: message.providerMessageId,
          }
        }

        const result = yield* sendRenderedEmail(config, rendered, fetcher)

        yield* result._tag === 'EmailProviderAccepted'
          ? markEmailMessageAccepted(
              db,
              message.id,
              result.provider,
              result.providerMessageId,
              runtime,
            )
          : markEmailMessageFailed(
              db,
              message.id,
              result.provider,
              result,
              runtime,
            )
        yield* recordEmailDelivery(db, message.id, rendered, result, runtime)

        if (result._tag === 'EmailProviderAccepted') {
          return {
            emailMessageId: message.id,
            ok: true as const,
            providerMessageId: result.providerMessageId,
          }
        }

        return {
          emailMessageId: message.id,
          errorMessage: result.errorMessage,
          errorName: result.errorName,
          ok: false as const,
        }
      }),
  )

  return {
    buildOrderSitesTransactionalEmailIdempotencyKey:
      buildOrderSitesTransactionalEmailIdempotencyKey,
    recordDraft,
    renderAdjutantCustomerNotificationEmail,
    renderAutopilotDecisionNotificationEmail,
    renderDripCampaignEmail,
    renderOrderSitesTransactionalEmail,
    renderOutOfCreditsEmail,
    renderPrivateWorkspaceInviteEmail,
    renderSiteReferralOnboardingEmail,
    renderTargetedRemakeOutreachEmail,
    reserveMessage: reserveEmailMessage,
    sendAdjutantCustomerNotificationWithLedger,
    sendAutopilotDecisionEmailWithLedger,
    sendDripCampaignEmailWithLedger,
    sendOrderSitesTransactionalEmailWithLedger,
    sendOutOfCreditsEmail: sendOutOfCreditsEmailEffect,
    sendOutOfCreditsEmailWithLedger,
    sendPrivateWorkspaceInviteEmailWithLedger,
    sendRenderedEmail,
    sendRenderedEmailViaCloudflareBinding:
      sendRenderedEmailViaCloudflareBindingEffect,
    sendRenderedEmailViaCloudflareBindingWithLedger:
      sendRenderedEmailViaCloudflareBindingWithLedgerEffect,
    sendSiteReferralOnboardingEmailWithLedger,
    sendTargetedRemakeOutreachEmailWithLedger,
  }
}

export const runOperatorEmailLedgerSmoke = (
  db: D1Database,
  config: ResendEmailConfig | undefined,
  input: OperatorEmailLedgerSmokeInput,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<OperatorEmailLedgerSmokeResult, EmailServiceError> =>
  Effect.gen(function* () {
    const service = makeEmailService()
    const mode = input.mode ?? 'dry_run'
    const notification = operatorSmokeNotificationInput(input)

    if (config === undefined) {
      const rendered = yield* service.renderAdjutantCustomerNotificationEmail(
        missingResendSmokeConfig(),
        notification,
      )
      const message = yield* service.reserveMessage(
        db,
        rendered,
        {
          metadata: {
            configStatus: 'missing',
            mode,
            smoke: 'operator_email_ledger_smoke',
          },
          sourceAuthorityRef: 'system.operator_email_ledger_smoke.v1',
        },
        runtime,
      )

      if (message.status !== 'accepted') {
        yield* markEmailMessageFailed(
          db,
          message.id,
          'resend',
          new EmailProviderRejected({
            errorMessage: 'Resend email configuration is not set.',
            errorName: 'email_config_missing',
            provider: 'resend',
          }),
          runtime,
        )
      }

      return {
        configStatus: 'missing' as const,
        emailMessageId: message.id,
        errorMessage: 'Resend email configuration is not set.',
        errorName: 'email_config_missing',
        idempotencyKey: input.idempotencyKey,
        mode,
        provider: 'resend' as const,
        providerMessageId: null,
        status: 'skipped' as const,
        templateSlug: rendered.templateSlug,
      }
    }

    const rendered = yield* service.renderAdjutantCustomerNotificationEmail(
      config,
      notification,
    )

    if (mode === 'dry_run') {
      const message = yield* service.reserveMessage(
        db,
        rendered,
        {
          metadata: {
            configStatus: 'present',
            mode,
            smoke: 'operator_email_ledger_smoke',
          },
          sourceAuthorityRef: 'system.operator_email_ledger_smoke.v1',
        },
        runtime,
      )

      return {
        configStatus: 'present' as const,
        emailMessageId: message.id,
        errorMessage: message.errorMessage,
        errorName: message.errorName,
        idempotencyKey: input.idempotencyKey,
        mode,
        provider: 'resend' as const,
        providerMessageId: message.providerMessageId,
        status:
          message.status === 'accepted'
            ? ('accepted' as const)
            : ('dry_run' as const),
        templateSlug: rendered.templateSlug,
      }
    }

    const result = yield* service.sendAdjutantCustomerNotificationWithLedger(
      db,
      config,
      notification,
      {
        metadata: {
          configStatus: 'present',
          mode,
          smoke: 'operator_email_ledger_smoke',
        },
        sourceAuthorityRef: 'system.operator_email_ledger_smoke.v1',
      },
      fetcher,
      runtime,
    )

    return result.ok
      ? {
          configStatus: 'present' as const,
          emailMessageId: result.emailMessageId,
          errorMessage: null,
          errorName: null,
          idempotencyKey: input.idempotencyKey,
          mode,
          provider: 'resend' as const,
          providerMessageId: result.providerMessageId,
          status: 'accepted' as const,
          templateSlug: rendered.templateSlug,
        }
      : {
          configStatus: 'present' as const,
          emailMessageId: result.emailMessageId,
          errorMessage: compactText(result.errorMessage, 500),
          errorName: compactText(result.errorName ?? 'resend_error', 120),
          idempotencyKey: input.idempotencyKey,
          mode,
          provider: 'resend' as const,
          providerMessageId: null,
          status: 'failed' as const,
          templateSlug: rendered.templateSlug,
        }
  })

export const EmailServiceLive = Layer.succeed(EmailService, makeEmailService())

const defaultEmailService = makeEmailService()

export const sendOutOfCreditsEmail = (
  config: ResendEmailConfig,
  input: SendOutOfCreditsEmailInput,
  fetcher: typeof fetch = fetch,
): Effect.Effect<SendEmailResult> =>
  defaultEmailService.sendOutOfCreditsEmail(config, input, fetcher)

export const sendOutOfCreditsEmailWithLedger = (
  db: D1Database,
  config: ResendEmailConfig,
  input: SendOutOfCreditsEmailInput,
  context?: EmailIntentContext | undefined,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<SendEmailResult, EmailServiceError> =>
  defaultEmailService.sendOutOfCreditsEmailWithLedger(
    db,
    config,
    input,
    context,
    fetcher,
    runtime,
  )

export const renderOrderSitesTransactionalEmail = (
  config: ResendEmailConfig,
  input: OrderSitesTransactionalEmailInput,
): Effect.Effect<RenderedEmail, EmailServiceError> =>
  defaultEmailService.renderOrderSitesTransactionalEmail(config, input)

export const sendOrderSitesTransactionalEmailWithLedger = (
  db: D1Database,
  config: ResendEmailConfig,
  input: OrderSitesTransactionalEmailInput,
  context?: EmailIntentContext | undefined,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  defaultEmailService.sendOrderSitesTransactionalEmailWithLedger(
    db,
    config,
    input,
    context,
    fetcher,
    runtime,
  )

export const sendRenderedEmailViaCloudflareBinding = (
  binding: CloudflareEmailBinding,
  rendered: RenderedEmail,
): Effect.Effect<EmailProviderResult> =>
  defaultEmailService.sendRenderedEmailViaCloudflareBinding(binding, rendered)

export const sendRenderedEmailViaCloudflareBindingWithLedger = (
  db: D1Database,
  binding: CloudflareEmailBinding,
  rendered: RenderedEmail,
  context?: EmailIntentContext | undefined,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  defaultEmailService.sendRenderedEmailViaCloudflareBindingWithLedger(
    db,
    binding,
    rendered,
    context,
    runtime,
  )

export const sendAdjutantCustomerNotificationWithLedger = (
  db: D1Database,
  config: ResendEmailConfig,
  input: SendAdjutantCustomerNotificationInput,
  context?: EmailIntentContext | undefined,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  defaultEmailService.sendAdjutantCustomerNotificationWithLedger(
    db,
    config,
    input,
    context,
    fetcher,
    runtime,
  )

export const sendAutopilotDecisionEmailWithLedger = (
  db: D1Database,
  config: ResendEmailConfig,
  input: AutopilotDecisionEmailInput,
  context?: EmailIntentContext | undefined,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  defaultEmailService.sendAutopilotDecisionEmailWithLedger(
    db,
    config,
    input,
    context,
    fetcher,
    runtime,
  )

export const sendDripCampaignEmailWithLedger = (
  db: D1Database,
  config: ResendEmailConfig,
  input: DripCampaignEmailInput,
  context?: EmailIntentContext | undefined,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  defaultEmailService.sendDripCampaignEmailWithLedger(
    db,
    config,
    input,
    context,
    fetcher,
    runtime,
  )

export const sendSiteReferralOnboardingEmailWithLedger = (
  db: D1Database,
  config: ResendEmailConfig,
  input: SiteReferralOnboardingEmailInput,
  context?: EmailIntentContext | undefined,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  defaultEmailService.sendSiteReferralOnboardingEmailWithLedger(
    db,
    config,
    input,
    context,
    fetcher,
    runtime,
  )

export const sendPrivateWorkspaceInviteEmailWithLedger = (
  db: D1Database,
  config: ResendEmailConfig,
  input: PrivateWorkspaceInviteEmailInput,
  context?: EmailIntentContext | undefined,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<EmailLedgerSendResult, EmailServiceError> =>
  defaultEmailService.sendPrivateWorkspaceInviteEmailWithLedger(
    db,
    config,
    input,
    context,
    fetcher,
    runtime,
  )
