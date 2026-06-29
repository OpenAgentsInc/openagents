import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import type { ResendEmailConfig } from './config'
import type { EmailCampaignRuntime } from './email-campaigns'
import {
  type EmailRuntime,
  SiteReferralOnboardingEmailInput,
  sendSiteReferralOnboardingEmailWithLedger,
  systemEmailRuntime,
} from './email'
import {
  type OnboardingDripEnrollmentResult,
  type OnboardingDripOrderState,
  enrollInOnboardingDrip,
} from './email-onboarding-drip'
import {
  type EmailPolicyCategory,
  type EmailSendEligibility,
  readEmailSendEligibility,
} from './email-preferences'
import { observedEffect } from './observability'
import type { ReferralConsumptionResult } from './site-referral-attribution-consumption'

export type SiteReferralOnboardingContext = Readonly<{
  attributionId: string
  campaignRef: string | null
  capturePath: string
  publicSlug: string
  publicSourceRef: string
  referralSourceId: string
  sourceLabel: string
  sourceSiteUrl: string | null
  target: string
}>

export type SiteReferralOnboardingTransactionalResult =
  | Readonly<{
      emailMessageId: string
      providerMessageId: string | null
      status: 'accepted'
    }>
  | Readonly<{
      emailMessageId: string
      errorMessage: string
      errorName: string | undefined
      status: 'failed'
    }>
  | Readonly<{
      reason: 'email_config_missing' | EmailSendEligibility['reason']
      status: 'skipped'
    }>

export type SiteReferralOnboardingResult =
  | Readonly<{
      reason: 'not_newly_consumed' | 'missing_referral_context'
      status: 'skipped'
    }>
  | Readonly<{
      context: SiteReferralOnboardingContext
      drip: OnboardingDripEnrollmentResult
      status: 'processed'
      transactional: SiteReferralOnboardingTransactionalResult
    }>

type SiteReferralOnboardingContextRow = Readonly<{
  attribution_id: string
  campaign_ref: string | null
  capture_path: string
  public_slug: string
  public_source_ref: string
  referral_source_id: string
  site_slug: string
  site_title: string
  source_label: string | null
  target: string
}>

export type SiteReferralOnboardingInput = Readonly<{
  appOrigin: string
  displayName: string
  email: string
  fetcher?: typeof fetch | undefined
  orderState: OnboardingDripOrderState
  referralResult: ReferralConsumptionResult
  resend?: ResendEmailConfig | undefined
  userId: string
  campaignRuntime?: EmailCampaignRuntime | undefined
  emailRuntime?: EmailRuntime | undefined
}>

const SAFE_SITE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{0,120}$/
const FORBIDDEN_PUBLIC_TEXT_PATTERN =
  /\b(provider[_ -]?account|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic)\b/i

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const sourceAuthorityRef = (attributionId: string): string =>
  compactText(`system.site_referral_onboarding.v1:${attributionId}`, 240)

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) &&
  !FORBIDDEN_PUBLIC_TEXT_PATTERN.test(value)

const safePublicText = (
  value: string | null | undefined,
  maxLength: number,
): string | null => {
  if (value === null || value === undefined) {
    return null
  }

  const compact = compactText(value, maxLength)

  return compact === '' || !textIsSafe(compact) ? null : compact
}

const sourceSiteUrl = (slug: string): string | null =>
  SAFE_SITE_SLUG_PATTERN.test(slug) && textIsSafe(slug)
    ? `https://sites.openagents.com/${slug}`
    : null

const contextFromRow = (
  row: SiteReferralOnboardingContextRow,
): SiteReferralOnboardingContext => {
  const sourceLabel =
    safePublicText(row.source_label, 160) ??
    safePublicText(row.site_title, 160) ??
    safePublicText(row.public_slug, 120) ??
    'an OpenAgents Site'

  return {
    attributionId: row.attribution_id,
    campaignRef: safePublicText(row.campaign_ref, 120),
    capturePath: safePublicText(row.capture_path, 40) ?? 'human',
    publicSlug: safePublicText(row.public_slug, 120) ?? 'site',
    publicSourceRef: safePublicText(row.public_source_ref, 190) ?? 'source',
    referralSourceId: row.referral_source_id,
    sourceLabel,
    sourceSiteUrl: sourceSiteUrl(row.site_slug),
    target: safePublicText(row.target, 40) ?? 'home',
  }
}

export const readSiteReferralOnboardingContext = async (
  db: D1Database,
  attributionId: string,
): Promise<SiteReferralOnboardingContext | null> => {
  const row = await db
    .prepare(
      `SELECT referral_attributions.id AS attribution_id,
              referral_attributions.capture_path,
              referral_attributions.target,
              site_referral_sources.id AS referral_source_id,
              site_referral_sources.public_slug,
              site_referral_sources.public_source_ref,
              site_referral_sources.campaign_ref,
              site_referral_sources.source_label,
              site_projects.slug AS site_slug,
              site_projects.title AS site_title
         FROM referral_attributions
         JOIN site_referral_sources
           ON site_referral_sources.id = referral_attributions.referral_source_id
          AND site_referral_sources.archived_at IS NULL
         JOIN site_projects
           ON site_projects.id = site_referral_sources.site_id
          AND site_projects.archived_at IS NULL
        WHERE referral_attributions.id = ?
          AND referral_attributions.archived_at IS NULL
        LIMIT 1`,
    )
    .bind(attributionId)
    .first<SiteReferralOnboardingContextRow>()

  return row === null ? null : contextFromRow(row)
}

const sendTransactionalReferralOnboarding = async (
  db: D1Database,
  input: SiteReferralOnboardingInput,
  context: SiteReferralOnboardingContext,
): Promise<SiteReferralOnboardingTransactionalResult> => {
  const eligibility = await readEmailSendEligibility(db, {
    category: 'transactional' satisfies EmailPolicyCategory,
    email: input.email,
  })

  if (!eligibility.allowed) {
    return {
      reason: eligibility.reason,
      status: 'skipped',
    }
  }

  if (input.resend === undefined) {
    return {
      reason: 'email_config_missing',
      status: 'skipped',
    }
  }

  const result = await observedEffect(
    'Email.sendSiteReferralOnboardingEmailWithLedger',
    sendSiteReferralOnboardingEmailWithLedger(
      db,
      input.resend,
      new SiteReferralOnboardingEmailInput({
        appOrigin: input.appOrigin,
        displayName: input.displayName,
        idempotencyKey: `site_referral_onboarding:${input.userId}:${context.attributionId}`,
        sourceLabel: context.sourceLabel,
        sourceSiteUrl: context.sourceSiteUrl,
        to: input.email,
      }),
      {
        metadata: {
          campaignRef: context.campaignRef,
          capturePath: context.capturePath,
          emailSubtype: 'site_referral_onboarding',
          publicSlug: context.publicSlug,
          publicSourceRef: context.publicSourceRef,
          referralAttributionId: context.attributionId,
          referralSourceId: context.referralSourceId,
          target: context.target,
        },
        sourceAuthorityRef: sourceAuthorityRef(context.attributionId),
        targetUserId: input.userId,
      },
      input.fetcher,
      input.emailRuntime ?? systemEmailRuntime,
    ),
  )

  return result.ok
    ? {
        emailMessageId: result.emailMessageId,
        providerMessageId: result.providerMessageId,
        status: 'accepted',
      }
    : {
        emailMessageId: result.emailMessageId,
        errorMessage: result.errorMessage,
        errorName: result.errorName,
        status: 'failed',
      }
}

export const sendSiteReferralOnboardingForConsumption = async (
  db: D1Database,
  input: SiteReferralOnboardingInput,
): Promise<SiteReferralOnboardingResult> => {
  if (input.referralResult._tag !== 'consumed') {
    return { reason: 'not_newly_consumed', status: 'skipped' }
  }

  const context = await readSiteReferralOnboardingContext(
    db,
    input.referralResult.attributionId,
  )

  if (context === null) {
    return { reason: 'missing_referral_context', status: 'skipped' }
  }

  const transactional = await sendTransactionalReferralOnboarding(
    db,
    input,
    context,
  )
  const drip = await enrollInOnboardingDrip(
    db,
    {
      displayName: input.displayName,
      email: input.email,
      orderState: input.orderState,
      referral: {
        attributionId: context.attributionId,
        referralSourceId: context.referralSourceId,
        sourceLabel: context.sourceLabel,
        sourceSiteUrl: context.sourceSiteUrl,
      },
      sourceAuthorityRef: sourceAuthorityRef(context.attributionId),
      userId: input.userId,
    },
    input.campaignRuntime,
  )

  return {
    context,
    drip,
    status: 'processed',
    transactional,
  }
}
