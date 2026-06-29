import { describe, expect, test } from 'vitest'

import {
  addEmailSuppression,
  insertEmailCampaign,
  insertEmailCampaignEnrollment,
  insertEmailCampaignSend,
  insertEmailCampaignStep,
  isEmailSuppressed,
  makeEmailCampaignEnrollmentRecord,
  makeEmailCampaignRecord,
  makeEmailCampaignSendRecord,
  makeEmailCampaignStepRecord,
  readEmailPreferenceAllows,
  upsertEmailPreference,
} from './email-campaigns'

class RecordingD1Statement {
  readonly bound: Array<unknown> = []

  constructor(
    private readonly db: RecordingD1Database,
    readonly query: string,
  ) {}

  bind(...values: Array<unknown>): RecordingD1Statement {
    this.bound.push(...values)

    return this
  }

  run(): Promise<void> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve()
  }

  first<T>(): Promise<T | null> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    return Promise.resolve(this.db.nextFirst as T | null)
  }
}

class RecordingD1Database {
  readonly lookups: Array<Readonly<{ query: string; values: Array<unknown> }>> =
    []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []
  nextFirst: unknown = null

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

const runtime = {
  makeId: (prefix: string) => `${prefix}_1`,
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

describe('email campaign records', () => {
  test('creates campaign, step, enrollment, and send records with idempotency keys', async () => {
    const db = new RecordingD1Database()
    const campaign = makeEmailCampaignRecord(
      {
        audience: 'new_signups',
        name: 'New signup onboarding',
        slug: 'new-signup-onboarding',
        sourceAuthorityRef: 'system.email_campaigns.v1',
        status: 'active',
      },
      runtime,
    )
    const step = makeEmailCampaignStepRecord(
      {
        campaignId: campaign.id,
        delaySeconds: 86400,
        lifecycleKind: 'onboarding_day_1',
        name: 'Day 1',
        status: 'active',
        stepKey: 'day_1',
        templateSlug: 'onboarding-day-1',
      },
      runtime,
    )
    const enrollment = makeEmailCampaignEnrollmentRecord(
      {
        campaignId: campaign.id,
        email: 'BEN@SILONES.COM',
        sourceAuthorityRef: 'order:software_order_1',
        userId: 'user_1',
      },
      runtime,
    )
    const send = makeEmailCampaignSendRecord(
      {
        campaignId: campaign.id,
        dueAt: '2026-06-06T12:00:00.000Z',
        email: enrollment.email,
        enrollmentId: enrollment.id,
        sourceAuthorityRef: 'email_campaign:new-signup-onboarding',
        stepId: step.id,
        stepKey: step.stepKey,
        userId: 'user_1',
      },
      runtime,
    )

    await insertEmailCampaign(db as unknown as D1Database, campaign, runtime.nowIso())
    await insertEmailCampaignStep(db as unknown as D1Database, step, runtime.nowIso())
    await insertEmailCampaignEnrollment(
      db as unknown as D1Database,
      enrollment,
      runtime.nowIso(),
    )
    await insertEmailCampaignSend(db as unknown as D1Database, send, runtime.nowIso())

    expect(enrollment.email).toBe('ben@silones.com')
    expect(enrollment.idempotencyKey).toBe(
      'email_campaign_enrollment:email_campaign_1:ben@silones.com',
    )
    expect(send.idempotencyKey).toBe(
      'email_campaign_send:email_campaign_enrollment_1:day_1',
    )
    expect(db.runs).toHaveLength(4)
    expect(db.runs[0]?.query).toContain('INSERT INTO email_campaigns')
    expect(db.runs[1]?.query).toContain('INSERT INTO email_campaign_steps')
    expect(db.runs[2]?.query).toContain(
      'INSERT INTO email_campaign_enrollments',
    )
    expect(db.runs[3]?.query).toContain('INSERT INTO email_campaign_sends')
    expect(JSON.stringify(db.runs)).not.toContain('provider_account')
    expect(JSON.stringify(db.runs)).not.toContain('auth_grant')
  })

  test('upserts preferences and reads preference/suppression state', async () => {
    const db = new RecordingD1Database()

    await upsertEmailPreference(
      db as unknown as D1Database,
      {
        dripOptIn: false,
        email: 'ben@silones.com',
        marketingOptIn: true,
        sourceAuthorityRef: 'customer.preference',
        transactionalOptIn: true,
        userId: 'user_1',
      },
      runtime,
    )
    await addEmailSuppression(
      db as unknown as D1Database,
      {
        email: 'ben@silones.com',
        reason: 'unsubscribe',
        scope: 'drip',
        sourceAuthorityRef: 'customer.unsubscribe',
      },
      runtime,
    )

    db.nextFirst = { id: 'email_suppression_1' }
    await expect(
      isEmailSuppressed(db as unknown as D1Database, 'BEN@SILONES.COM', 'drip'),
    ).resolves.toBe(true)
    db.nextFirst = {
      drip_opt_in: 0,
      marketing_opt_in: 1,
      transactional_opt_in: 1,
    }
    await expect(
      readEmailPreferenceAllows(
        db as unknown as D1Database,
        'ben@silones.com',
        'drip',
      ),
    ).resolves.toBe(false)

    expect(db.runs[0]?.query).toContain('INSERT INTO email_preferences')
    expect(db.runs[1]?.query).toContain(
      'INSERT INTO email_suppression_entries',
    )
    expect(db.lookups[0]?.values).toEqual(['ben@silones.com', 'drip'])
    expect(db.lookups[1]?.values).toEqual(['ben@silones.com'])
  })
})
