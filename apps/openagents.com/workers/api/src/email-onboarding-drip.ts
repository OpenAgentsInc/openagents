import type { DripEmailKind } from '@openagentsinc/email-templates'

import {
  type EmailCampaignRecord,
  type EmailCampaignRuntime,
  type EmailCampaignStepRecord,
  insertEmailCampaign,
  insertEmailCampaignEnrollment,
  insertEmailCampaignSend,
  insertEmailCampaignStep,
  isEmailSuppressed,
  listEmailCampaignSteps,
  makeEmailCampaignEnrollmentRecord,
  makeEmailCampaignRecord,
  makeEmailCampaignSendRecord,
  makeEmailCampaignStepRecord,
  readEmailCampaignBySlug,
  readEmailCampaignEnrollmentByIdempotencyKey,
  readEmailPreferenceAllows,
  systemEmailCampaignRuntime,
} from './email-campaigns'
import { isoTimestampAfterIso } from './runtime-primitives'

export const ONBOARDING_DRIP_CAMPAIGN_SLUG = 'new-signup-onboarding'
export const ONBOARDING_DRIP_SOURCE_AUTHORITY =
  'system.email_onboarding_drip.v1'

export type OnboardingDripStepDefinition = Readonly<{
  delaySeconds: number
  kind: DripEmailKind
  name: string
  stepKey: string
}>

export const ONBOARDING_DRIP_STEPS = [
  {
    delaySeconds: 0,
    kind: 'signup_day_0',
    name: 'Day 0 welcome',
    stepKey: 'day_0',
  },
  {
    delaySeconds: 86_400,
    kind: 'signup_day_1',
    name: 'Day 1 request quality',
    stepKey: 'day_1',
  },
  {
    delaySeconds: 172_800,
    kind: 'signup_day_2',
    name: 'Day 2 revision loop',
    stepKey: 'day_2',
  },
] as const satisfies ReadonlyArray<OnboardingDripStepDefinition>

export type OnboardingDripOrderState = 'none' | 'active' | 'delivered'

export type OnboardingDripEnrollmentInput = Readonly<{
  displayName?: string | null | undefined
  email: string
  orderState: OnboardingDripOrderState
  referral?: Readonly<{
    attributionId: string
    referralSourceId: string
    sourceLabel: string
    sourceSiteUrl: string | null
  }> | null | undefined
  sourceAuthorityRef: string
  userId?: string | null | undefined
}>

export type OnboardingDripEnrollmentResult =
  | Readonly<{
      campaignId: string
      enrollmentId: string
      scheduledSendCount: number
      status: 'enrolled'
    }>
  | Readonly<{
      reason:
        | 'active_order'
        | 'delivered_order'
        | 'drip_preference_disabled'
        | 'drip_suppressed'
      status: 'skipped'
    }>

export type OnboardingDripCampaignDefinition = Readonly<{
  campaign: EmailCampaignRecord
  steps: ReadonlyArray<EmailCampaignStepRecord>
}>

const templateSlug = (kind: DripEmailKind): string => `drip.${kind}.v1`

const scheduledSendSourceAuthority = (stepKey: string): string =>
  `${ONBOARDING_DRIP_SOURCE_AUTHORITY}:send:${stepKey}`

export const seedOnboardingDripCampaign = async (
  db: D1Database,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<OnboardingDripCampaignDefinition> => {
  const now = runtime.nowIso()
  const candidateCampaign = makeEmailCampaignRecord(
    {
      audience: 'new_signups_without_active_or_delivered_orders',
      metadata: {
        campaignKind: 'signup_onboarding_drip',
        version: 1,
      },
      name: 'New signup onboarding drip',
      slug: ONBOARDING_DRIP_CAMPAIGN_SLUG,
      sourceAuthorityRef: ONBOARDING_DRIP_SOURCE_AUTHORITY,
      status: 'active',
    },
    runtime,
  )

  await insertEmailCampaign(db, candidateCampaign, now)

  const campaign =
    (await readEmailCampaignBySlug(db, ONBOARDING_DRIP_CAMPAIGN_SLUG)) ??
    candidateCampaign

  await Promise.all(
    ONBOARDING_DRIP_STEPS.map(step =>
      insertEmailCampaignStep(
        db,
        makeEmailCampaignStepRecord(
          {
            campaignId: campaign.id,
            delaySeconds: step.delaySeconds,
            lifecycleKind: step.kind,
            metadata: {
              campaignKind: 'signup_onboarding_drip',
              dripKind: step.kind,
              version: 1,
            },
            name: step.name,
            status: 'active',
            stepKey: step.stepKey,
            templateSlug: templateSlug(step.kind),
          },
          runtime,
        ),
        now,
      ),
    ),
  )

  const steps = await listEmailCampaignSteps(db, campaign.id)

  return { campaign, steps }
}

export const enrollInOnboardingDrip = async (
  db: D1Database,
  input: OnboardingDripEnrollmentInput,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<OnboardingDripEnrollmentResult> => {
  if (input.orderState === 'active') {
    return { reason: 'active_order', status: 'skipped' }
  }

  if (input.orderState === 'delivered') {
    return { reason: 'delivered_order', status: 'skipped' }
  }

  if (await isEmailSuppressed(db, input.email, 'drip')) {
    return { reason: 'drip_suppressed', status: 'skipped' }
  }

  if (!(await readEmailPreferenceAllows(db, input.email, 'drip'))) {
    return { reason: 'drip_preference_disabled', status: 'skipped' }
  }

  const now = runtime.nowIso()
  const definition = await seedOnboardingDripCampaign(db, runtime)
  const candidateEnrollment = makeEmailCampaignEnrollmentRecord(
    {
      campaignId: definition.campaign.id,
      email: input.email,
      metadata: {
        displayName: input.displayName ?? null,
        orderState: input.orderState,
        referralAttributionId: input.referral?.attributionId ?? null,
        referralSourceId: input.referral?.referralSourceId ?? null,
        referralSourceLabel: input.referral?.sourceLabel ?? null,
        referralSourceSiteUrl: input.referral?.sourceSiteUrl ?? null,
      },
      sourceAuthorityRef: input.sourceAuthorityRef,
      userId: input.userId ?? null,
    },
    runtime,
  )

  await insertEmailCampaignEnrollment(db, candidateEnrollment, now)

  const enrollment =
    (await readEmailCampaignEnrollmentByIdempotencyKey(
      db,
      candidateEnrollment.idempotencyKey,
    )) ?? candidateEnrollment

  await Promise.all(
    definition.steps.map(step =>
      insertEmailCampaignSend(
        db,
        makeEmailCampaignSendRecord(
          {
            campaignId: definition.campaign.id,
            dueAt: isoTimestampAfterIso(now, step.delaySeconds * 1_000),
            email: enrollment.email,
            enrollmentId: enrollment.id,
            metadata: {
              displayName: input.displayName ?? null,
              referralAttributionId: input.referral?.attributionId ?? null,
              referralSourceId: input.referral?.referralSourceId ?? null,
              referralSourceLabel: input.referral?.sourceLabel ?? null,
              referralSourceSiteUrl: input.referral?.sourceSiteUrl ?? null,
              templateSlug: step.templateSlug,
            },
            sourceAuthorityRef: scheduledSendSourceAuthority(step.stepKey),
            stepId: step.id,
            stepKey: step.stepKey,
            userId: enrollment.userId,
          },
          runtime,
        ),
        now,
      ),
    ),
  )

  return {
    campaignId: definition.campaign.id,
    enrollmentId: enrollment.id,
    scheduledSendCount: definition.steps.length,
    status: 'enrolled',
  }
}
