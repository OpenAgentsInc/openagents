import { describe, expect, test } from 'vitest'

import {
  readSiteReferralOnboardingContext,
  sendSiteReferralOnboardingForConsumption,
} from './site-referral-onboarding'

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
      results: this.query.includes('FROM email_campaign_steps')
        ? (stepRows as Array<T>)
        : [],
      success: true,
    } as unknown as D1Result<T>)
  }

  first<T>(): Promise<T | null> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    if (this.query.includes('FROM referral_attributions')) {
      return Promise.resolve(this.db.contextRow as T)
    }

    if (this.query.includes('FROM email_suppression_entries')) {
      return Promise.resolve(this.db.suppressed ? ({ id: 'suppressed' } as T) : null)
    }

    if (this.query.includes('FROM email_preferences')) {
      return Promise.resolve(this.db.dripOptedOut ? (optedOutPreference as T) : null)
    }

    if (this.query.includes('FROM email_campaigns')) {
      return Promise.resolve(campaignRow as T)
    }

    if (this.query.includes('FROM email_campaign_enrollments')) {
      return Promise.resolve(enrollmentRow as T)
    }

    return Promise.resolve(null)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve({
      meta: { changes: 1 },
      results: [],
      success: true,
    } as unknown as D1Result<T>)
  }
}

class RecordingD1Database {
  readonly lookups: Array<Readonly<{ query: string; values: Array<unknown> }>> =
    []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []
  dripOptedOut = false
  suppressed = false

  contextRow = {
    attribution_id: 'referral_attribution_otec',
    campaign_ref: 'site-launch',
    capture_path: 'human',
    public_slug: 'otec',
    public_source_ref: 'site_ref_otec_ben',
    referral_source_id: 'site_referral_source_otec',
    site_slug: 'otec',
    site_title: 'OTEC Floating Datacenter',
    source_label: 'Bearer gho_abcdefghijklmnopqrstuvwxyz',
    target: 'order',
  }

  batch<T = unknown>(statements: Array<D1PreparedStatement>): Promise<Array<D1Result<T>>> {
    return Promise.all(statements.map(statement => statement.run<T>()))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.resolve(new ArrayBuffer(0))
  }

  exec(): Promise<D1ExecResult> {
    return Promise.resolve({ count: 0, duration: 0 })
  }

  prepare(query: string): D1PreparedStatement {
    return new RecordingD1Statement(this, query) as unknown as D1PreparedStatement
  }

  withSession(): D1DatabaseSession {
    throw new Error('D1 session should not be used')
  }
}

const campaignRow = {
  audience: 'new_signups_without_active_or_delivered_orders',
  id: 'email_campaign_existing',
  metadata_json: '{}',
  name: 'New signup onboarding drip',
  slug: 'new-signup-onboarding',
  source_authority_ref: 'system.email_onboarding_drip.v1',
  status: 'active',
}

const stepRows = [
  {
    campaign_id: campaignRow.id,
    delay_seconds: 0,
    id: 'email_campaign_step_day_0',
    lifecycle_kind: 'signup_day_0',
    metadata_json: '{}',
    name: 'Day 0 welcome',
    status: 'active',
    step_key: 'day_0',
    template_slug: 'drip.signup_day_0.v1',
  },
  {
    campaign_id: campaignRow.id,
    delay_seconds: 86_400,
    id: 'email_campaign_step_day_1',
    lifecycle_kind: 'signup_day_1',
    metadata_json: '{}',
    name: 'Day 1 request quality',
    status: 'active',
    step_key: 'day_1',
    template_slug: 'drip.signup_day_1.v1',
  },
]

const enrollmentRow = {
  campaign_id: campaignRow.id,
  email: 'alex@example.com',
  id: 'email_campaign_enrollment_existing',
  idempotency_key:
    'email_campaign_enrollment:email_campaign_existing:alex@example.com',
  metadata_json: '{}',
  source_authority_ref:
    'system.site_referral_onboarding.v1:referral_attribution_otec',
  status: 'active',
  user_id: 'github:1',
}

const optedOutPreference = {
  drip_opt_in: 0,
  marketing_opt_in: 1,
  transactional_opt_in: 1,
}

const campaignRuntime = {
  makeId: (prefix: string) => `${prefix}_generated`,
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

describe('site referral onboarding hook', () => {
  test('sanitizes unsafe source labels while reading public context', async () => {
    const db = new RecordingD1Database()
    const context = await readSiteReferralOnboardingContext(
      db as unknown as D1Database,
      'referral_attribution_otec',
    )

    expect(context).toEqual(
      expect.objectContaining({
        attributionId: 'referral_attribution_otec',
        publicSlug: 'otec',
        referralSourceId: 'site_referral_source_otec',
        sourceLabel: 'OTEC Floating Datacenter',
        sourceSiteUrl: 'https://sites.openagents.com/otec',
      }),
    )
    expect(JSON.stringify(context)).not.toContain('gho_')
  })

  test('enrolls consumed referred users with referral metadata and skips transactional send without config', async () => {
    const db = new RecordingD1Database()

    const result = await sendSiteReferralOnboardingForConsumption(
      db as unknown as D1Database,
      {
        appOrigin: 'https://openagents.com',
        campaignRuntime,
        displayName: 'Alex Customer',
        email: 'alex@example.com',
        orderState: 'none',
        referralResult: {
          _tag: 'consumed',
          attributionId: 'referral_attribution_otec',
        },
        userId: 'github:1',
      },
    )

    expect(result).toEqual(
      expect.objectContaining({
        status: 'processed',
        transactional: {
          reason: 'email_config_missing',
          status: 'skipped',
        },
      }),
    )
    expect(result.status === 'processed' ? result.drip : null).toEqual({
      campaignId: 'email_campaign_existing',
      enrollmentId: 'email_campaign_enrollment_existing',
      scheduledSendCount: 2,
      status: 'enrolled',
    })

    const enrollmentRun = db.runs.find(run =>
      run.query.includes('INSERT INTO email_campaign_enrollments'),
    )
    const sendRuns = db.runs.filter(run =>
      run.query.includes('INSERT INTO email_campaign_sends'),
    )
    const enrollmentMetadata = JSON.parse(String(enrollmentRun?.values[7]))

    expect(enrollmentMetadata).toMatchObject({
      referralAttributionId: 'referral_attribution_otec',
      referralSourceId: 'site_referral_source_otec',
      referralSourceLabel: 'OTEC Floating Datacenter',
      referralSourceSiteUrl: 'https://sites.openagents.com/otec',
    })
    expect(sendRuns).toHaveLength(2)
    expect(JSON.stringify(sendRuns)).toContain('referral_attribution_otec')
    expect(JSON.stringify(db.runs)).not.toMatch(
      /provider_account|auth_grant|token_hash|gho_/i,
    )
  })

  test('does not schedule drip records for suppressed referred users', async () => {
    const db = new RecordingD1Database()
    db.suppressed = true

    const result = await sendSiteReferralOnboardingForConsumption(
      db as unknown as D1Database,
      {
        appOrigin: 'https://openagents.com',
        campaignRuntime,
        displayName: 'Alex Customer',
        email: 'alex@example.com',
        orderState: 'none',
        referralResult: {
          _tag: 'consumed',
          attributionId: 'referral_attribution_otec',
        },
        userId: 'github:1',
      },
    )

    expect(result.status === 'processed' ? result.drip : null).toEqual({
      reason: 'drip_suppressed',
      status: 'skipped',
    })
    expect(
      db.runs.some(run => run.query.includes('INSERT INTO email_campaign_sends')),
    ).toBe(false)
  })
})
