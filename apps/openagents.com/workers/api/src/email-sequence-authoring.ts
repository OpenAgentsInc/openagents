import { Schema as S } from 'effect'

import {
  type EmailCampaignEnrollmentRecord,
  type EmailCampaignRecord,
  type EmailCampaignRuntime,
  type EmailCampaignStatus,
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

// Operator-facing authoring layer over the existing multi-step scheduled email
// engine (email-campaigns.ts + email-campaign-dispatcher.ts). This module lets
// an operator author and manage nurture/sales SEQUENCES (campaigns) and their
// ordered steps, then enroll subscribers. It reuses the campaign/step/
// enrollment/send primitives and does NOT duplicate the dispatcher: scheduled
// sends produced here are picked up by dispatchDueEmailCampaignSends exactly
// like the onboarding drip. No new migration — reuses migration 0063 tables.

export const EMAIL_SEQUENCE_SOURCE_AUTHORITY =
  'operator.email_sequence_authoring.v1'

const SEQUENCE_STATUS_VALUES = [
  'draft',
  'active',
  'paused',
  'archived',
] as const satisfies ReadonlyArray<EmailCampaignStatus>

// Lifecycle transitions an operator may apply to an authored sequence.
// 'draft' is the authored-but-not-running state; 'active' enables dispatch;
// 'paused' halts new dispatch; 'archived' retires the sequence.
const EmailSequenceStatusSchema = S.Literals(SEQUENCE_STATUS_VALUES)

const SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{1,118}[a-z0-9]$/

const EmailSequenceSlug = S.String.check(
  S.isPattern(SLUG_PATTERN),
  S.isMaxLength(120),
)

const EmailSequenceStepInput = S.Struct({
  delaySeconds: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  lifecycleKind: S.optionalKey(S.NullishOr(S.NonEmptyString)),
  name: S.NonEmptyString,
  stepKey: S.NonEmptyString,
  templateSlug: S.NonEmptyString,
})
export type EmailSequenceStepInput = typeof EmailSequenceStepInput.Type

export const CreateEmailSequenceRequest = S.Struct({
  audience: S.NonEmptyString,
  name: S.NonEmptyString,
  slug: EmailSequenceSlug,
  status: S.optionalKey(EmailSequenceStatusSchema),
  steps: S.NonEmptyArray(EmailSequenceStepInput),
})
export type CreateEmailSequenceRequest = typeof CreateEmailSequenceRequest.Type

export const decodeCreateEmailSequenceRequest = S.decodeUnknownSync(
  CreateEmailSequenceRequest,
)

export const UpdateEmailSequenceStatusRequest = S.Struct({
  status: EmailSequenceStatusSchema,
})
export type UpdateEmailSequenceStatusRequest =
  typeof UpdateEmailSequenceStatusRequest.Type

export const decodeUpdateEmailSequenceStatusRequest = S.decodeUnknownSync(
  UpdateEmailSequenceStatusRequest,
)

export const EnrollSubscriberRequest = S.Struct({
  displayName: S.optionalKey(S.NullishOr(S.String)),
  email: S.NonEmptyString.check(S.isIncludes('@')),
  userId: S.optionalKey(S.NullishOr(S.String)),
})
export type EnrollSubscriberRequest = typeof EnrollSubscriberRequest.Type

export const decodeEnrollSubscriberRequest = S.decodeUnknownSync(
  EnrollSubscriberRequest,
)

export type EmailSequenceDefinition = Readonly<{
  campaign: EmailCampaignRecord
  steps: ReadonlyArray<EmailCampaignStepRecord>
}>

export type EnrollSubscriberResult =
  | Readonly<{
      campaignId: string
      enrollmentId: string
      scheduledSendCount: number
      status: 'enrolled'
    }>
  | Readonly<{
      reason: 'drip_preference_disabled' | 'drip_suppressed'
      status: 'skipped'
    }>

const stepSourceAuthority = (slug: string, stepKey: string): string =>
  `${EMAIL_SEQUENCE_SOURCE_AUTHORITY}:${slug}:send:${stepKey}`

const sequenceSourceAuthority = (
  operatorUserId: string,
  slug: string,
): string => `${EMAIL_SEQUENCE_SOURCE_AUTHORITY}:${operatorUserId}:${slug}`

// Create (or upsert) an authored sequence and its ordered steps. The campaign
// `slug` is the stable identity; re-creating with the same slug updates the
// campaign and steps in place via the existing ON CONFLICT upserts. Steps that
// were authored previously but are absent from this request are archived so the
// dispatcher stops scheduling them for new enrollments.
export const createEmailSequence = async (
  db: D1Database,
  operatorUserId: string,
  request: CreateEmailSequenceRequest,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<EmailSequenceDefinition> => {
  const now = runtime.nowIso()
  const sourceAuthorityRef = sequenceSourceAuthority(
    operatorUserId,
    request.slug,
  )
  const candidateCampaign = makeEmailCampaignRecord(
    {
      audience: request.audience,
      metadata: {
        authoredByUserId: operatorUserId,
        campaignKind: 'operator_authored_sequence',
        version: 1,
      },
      name: request.name,
      slug: request.slug,
      sourceAuthorityRef,
      status: request.status ?? 'draft',
    },
    runtime,
  )

  await insertEmailCampaign(db, candidateCampaign, now)

  const campaign =
    (await readEmailCampaignBySlug(db, request.slug)) ?? candidateCampaign

  const requestedStepKeys = new Set<string>()

  await Promise.all(
    request.steps.map(step => {
      const record = makeEmailCampaignStepRecord(
        {
          campaignId: campaign.id,
          delaySeconds: step.delaySeconds,
          lifecycleKind: step.lifecycleKind ?? null,
          metadata: {
            campaignKind: 'operator_authored_sequence',
            version: 1,
          },
          name: step.name,
          status: campaign.status,
          stepKey: step.stepKey,
          templateSlug: step.templateSlug,
        },
        runtime,
      )

      requestedStepKeys.add(record.stepKey)

      return insertEmailCampaignStep(db, record, now)
    }),
  )

  await archiveRemovedSequenceSteps(db, campaign.id, requestedStepKeys, now)

  const steps = await listEmailCampaignSteps(db, campaign.id)

  return { campaign, steps }
}

const archiveRemovedSequenceSteps = async (
  db: D1Database,
  campaignId: string,
  keepStepKeys: ReadonlySet<string>,
  now: string,
): Promise<void> => {
  const existing = await listEmailCampaignSteps(db, campaignId)
  const stale = existing.filter(step => !keepStepKeys.has(step.stepKey))

  await Promise.all(
    stale.map(step =>
      db
        .prepare(
          `UPDATE email_campaign_steps
              SET status = 'archived',
                  archived_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(now, now, step.id)
        .run(),
    ),
  )
}

// Update the lifecycle status of an authored sequence. Activating or pausing
// the campaign also propagates to its non-archived steps so the dispatcher's
// scheduled sends honor the operator decision for future enrollments. Returns
// null when no campaign with the slug exists.
export const updateEmailSequenceStatus = async (
  db: D1Database,
  slug: string,
  request: UpdateEmailSequenceStatusRequest,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<EmailSequenceDefinition | null> => {
  const existing = await readEmailCampaignBySlug(db, slug)

  if (existing === null) {
    return null
  }

  const now = runtime.nowIso()

  await db
    .prepare(
      `UPDATE email_campaigns
          SET status = ?,
              archived_at = ?,
              updated_at = ?
        WHERE id = ?
          AND archived_at IS NULL`,
    )
    .bind(
      request.status,
      request.status === 'archived' ? now : null,
      now,
      existing.id,
    )
    .run()

  await db
    .prepare(
      `UPDATE email_campaign_steps
          SET status = ?,
              archived_at = ?,
              updated_at = ?
        WHERE campaign_id = ?
          AND archived_at IS NULL`,
    )
    .bind(
      request.status,
      request.status === 'archived' ? now : null,
      now,
      existing.id,
    )
    .run()

  const campaign = await readEmailCampaignBySlug(db, slug)

  if (campaign === null) {
    // Archived: campaign is now hidden from readEmailCampaignBySlug. Report the
    // operator-requested terminal state without re-reading the archived row.
    return {
      campaign: { ...existing, status: request.status },
      steps: [],
    }
  }

  const steps = await listEmailCampaignSteps(db, campaign.id)

  return { campaign, steps }
}

// Enroll a subscriber into an authored sequence and schedule the per-step
// sends. Honors drip suppression and preference exactly like the onboarding
// drip path. Enrollment and sends are idempotent (existing ON CONFLICT keys),
// so repeated enrollment for the same email is safe. Returns null when no
// campaign with the slug exists.
export const enrollSubscriberInSequence = async (
  db: D1Database,
  slug: string,
  request: EnrollSubscriberRequest,
  operatorUserId: string,
  runtime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<EnrollSubscriberResult | null> => {
  const campaign = await readEmailCampaignBySlug(db, slug)

  if (campaign === null) {
    return null
  }

  if (await isEmailSuppressed(db, request.email, 'drip')) {
    return { reason: 'drip_suppressed', status: 'skipped' }
  }

  if (!(await readEmailPreferenceAllows(db, request.email, 'drip'))) {
    return { reason: 'drip_preference_disabled', status: 'skipped' }
  }

  const now = runtime.nowIso()
  const sourceAuthorityRef = sequenceSourceAuthority(operatorUserId, slug)
  const candidateEnrollment: EmailCampaignEnrollmentRecord =
    makeEmailCampaignEnrollmentRecord(
      {
        campaignId: campaign.id,
        email: request.email,
        metadata: {
          displayName: request.displayName ?? null,
          enrolledByUserId: operatorUserId,
        },
        sourceAuthorityRef,
        userId: request.userId ?? null,
      },
      runtime,
    )

  await insertEmailCampaignEnrollment(db, candidateEnrollment, now)

  const enrollment =
    (await readEmailCampaignEnrollmentByIdempotencyKey(
      db,
      candidateEnrollment.idempotencyKey,
    )) ?? candidateEnrollment

  const steps = await listEmailCampaignSteps(db, campaign.id)

  await Promise.all(
    steps.map(step =>
      insertEmailCampaignSend(
        db,
        makeEmailCampaignSendRecord(
          {
            campaignId: campaign.id,
            dueAt: isoTimestampAfterIso(now, step.delaySeconds * 1_000),
            email: enrollment.email,
            enrollmentId: enrollment.id,
            metadata: {
              displayName: request.displayName ?? null,
              templateSlug: step.templateSlug,
            },
            sourceAuthorityRef: stepSourceAuthority(slug, step.stepKey),
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
    campaignId: campaign.id,
    enrollmentId: enrollment.id,
    scheduledSendCount: steps.length,
    status: 'enrolled',
  }
}

export const projectEmailSequenceDefinition = (
  definition: EmailSequenceDefinition,
) => ({
  campaign: {
    audience: definition.campaign.audience,
    id: definition.campaign.id,
    name: definition.campaign.name,
    slug: definition.campaign.slug,
    status: definition.campaign.status,
  },
  steps: definition.steps.map(step => ({
    delaySeconds: step.delaySeconds,
    lifecycleKind: step.lifecycleKind,
    name: step.name,
    status: step.status,
    stepKey: step.stepKey,
    templateSlug: step.templateSlug,
  })),
})
