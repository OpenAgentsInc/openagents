import { describe, expect, test } from 'vitest'

import {
  ONBOARDING_DRIP_CAMPAIGN_SLUG,
  ONBOARDING_DRIP_STEPS,
  enrollInOnboardingDrip,
  seedOnboardingDripCampaign,
} from './email-onboarding-drip'

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

  all<T>(): Promise<D1Result<T>> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    return Promise.resolve({
      meta: {},
      results: (this.db.nextAll.shift() ?? []) as Array<T>,
      success: true,
    } as D1Result<T>)
  }

  first<T>(): Promise<T | null> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    return Promise.resolve((this.db.nextFirst.shift() ?? null) as T | null)
  }

  run(): Promise<void> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve()
  }
}

class RecordingD1Database {
  readonly lookups: Array<Readonly<{ query: string; values: Array<unknown> }>> =
    []
  readonly nextAll: Array<Array<unknown>> = []
  readonly nextFirst: Array<unknown | null> = []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

const runtime = {
  makeId: (prefix: string) => `${prefix}_id`,
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

const campaignRow = {
  audience: 'new_signups_without_active_or_delivered_orders',
  id: 'email_campaign_existing',
  metadata_json: '{}',
  name: 'New signup onboarding drip',
  slug: ONBOARDING_DRIP_CAMPAIGN_SLUG,
  source_authority_ref: 'system.email_onboarding_drip.v1',
  status: 'active',
}

const stepRows = ONBOARDING_DRIP_STEPS.map(step => ({
  campaign_id: campaignRow.id,
  delay_seconds: step.delaySeconds,
  id: `email_campaign_step_${step.stepKey}`,
  lifecycle_kind: step.kind,
  metadata_json: '{}',
  name: step.name,
  status: 'active',
  step_key: step.stepKey,
  template_slug: `drip.${step.kind}.v1`,
}))

describe('email onboarding drip', () => {
  test('seeds active day 0/day 1/day 2 campaign steps', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(campaignRow)
    db.nextAll.push(stepRows)

    const definition = await seedOnboardingDripCampaign(
      db as unknown as D1Database,
      runtime,
    )

    expect(definition.campaign.id).toBe('email_campaign_existing')
    expect(definition.steps.map(step => step.delaySeconds)).toEqual([
      0, 86_400, 172_800,
    ])
    expect(
      db.runs.filter(run => run.query.includes('email_campaign_steps')),
    ).toHaveLength(3)
    expect(JSON.stringify(db.runs)).toContain('drip.signup_day_0.v1')
    expect(JSON.stringify(db.runs)).toContain('drip.signup_day_1.v1')
    expect(JSON.stringify(db.runs)).toContain('drip.signup_day_2.v1')
  })

  test('enrolls an eligible signup and schedules three idempotent sends', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(null, null, campaignRow, {
      campaign_id: campaignRow.id,
      email: 'ben@silones.com',
      id: 'email_campaign_enrollment_existing',
      idempotency_key:
        'email_campaign_enrollment:email_campaign_existing:ben@silones.com',
      metadata_json: '{}',
      source_authority_ref: 'signup:user_1',
      status: 'active',
      user_id: 'user_1',
    })
    db.nextAll.push(stepRows)

    const result = await enrollInOnboardingDrip(
      db as unknown as D1Database,
      {
        displayName: 'Ben',
        email: 'BEN@SILONES.COM',
        orderState: 'none',
        sourceAuthorityRef: 'signup:user_1',
        userId: 'user_1',
      },
      runtime,
    )

    expect(result).toEqual({
      campaignId: 'email_campaign_existing',
      enrollmentId: 'email_campaign_enrollment_existing',
      scheduledSendCount: 3,
      status: 'enrolled',
    })

    const sendRuns = db.runs.filter(run =>
      run.query.includes('INSERT INTO email_campaign_sends'),
    )
    expect(sendRuns).toHaveLength(3)
    expect(sendRuns.map(run => run.values[6])).toEqual([
      '2026-06-05T12:00:00.000Z',
      '2026-06-06T12:00:00.000Z',
      '2026-06-07T12:00:00.000Z',
    ])
    expect(sendRuns.map(run => run.values[8])).toEqual([
      'email_campaign_send:email_campaign_enrollment_existing:day_0',
      'email_campaign_send:email_campaign_enrollment_existing:day_1',
      'email_campaign_send:email_campaign_enrollment_existing:day_2',
    ])
    expect(JSON.stringify(sendRuns)).not.toContain('provider_account')
    expect(JSON.stringify(sendRuns)).not.toContain('auth_grant')
  })

  test('skips active and delivered order states before scheduling', async () => {
    const activeDb = new RecordingD1Database()
    const deliveredDb = new RecordingD1Database()

    await expect(
      enrollInOnboardingDrip(
        activeDb as unknown as D1Database,
        {
          email: 'ben@silones.com',
          orderState: 'active',
          sourceAuthorityRef: 'signup:user_1',
          userId: 'user_1',
        },
        runtime,
      ),
    ).resolves.toEqual({ reason: 'active_order', status: 'skipped' })
    await expect(
      enrollInOnboardingDrip(
        deliveredDb as unknown as D1Database,
        {
          email: 'ben@silones.com',
          orderState: 'delivered',
          sourceAuthorityRef: 'signup:user_1',
          userId: 'user_1',
        },
        runtime,
      ),
    ).resolves.toEqual({ reason: 'delivered_order', status: 'skipped' })

    expect(activeDb.runs).toHaveLength(0)
    expect(deliveredDb.runs).toHaveLength(0)
  })

  test('skips suppressed or opted-out drip recipients', async () => {
    const suppressedDb = new RecordingD1Database()
    suppressedDb.nextFirst.push({ id: 'email_suppression_1' })
    const preferenceDb = new RecordingD1Database()
    preferenceDb.nextFirst.push(null, {
      drip_opt_in: 0,
      marketing_opt_in: 1,
      transactional_opt_in: 1,
    })

    await expect(
      enrollInOnboardingDrip(
        suppressedDb as unknown as D1Database,
        {
          email: 'ben@silones.com',
          orderState: 'none',
          sourceAuthorityRef: 'signup:user_1',
          userId: 'user_1',
        },
        runtime,
      ),
    ).resolves.toEqual({ reason: 'drip_suppressed', status: 'skipped' })
    await expect(
      enrollInOnboardingDrip(
        preferenceDb as unknown as D1Database,
        {
          email: 'ben@silones.com',
          orderState: 'none',
          sourceAuthorityRef: 'signup:user_1',
          userId: 'user_1',
        },
        runtime,
      ),
    ).resolves.toEqual({
      reason: 'drip_preference_disabled',
      status: 'skipped',
    })

    expect(suppressedDb.runs).toHaveLength(0)
    expect(preferenceDb.runs).toHaveLength(0)
  })
})
