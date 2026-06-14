import { describe, expect, test } from 'vitest'

import {
  createEmailSequence,
  decodeCreateEmailSequenceRequest,
  decodeEnrollSubscriberRequest,
  decodeUpdateEmailSequenceStatusRequest,
  enrollSubscriberInSequence,
  updateEmailSequenceStatus,
} from './email-sequence-authoring'

// Recording fake D1 mirrored from email-onboarding-drip.test.ts: each prepare()
// returns a statement that records runs/lookups; reads are served from queued
// nextFirst/nextAll fixtures in FIFO order.
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

const sequenceRequest = decodeCreateEmailSequenceRequest({
  audience: 'sales_qualified_leads',
  name: 'Welcome nurture',
  slug: 'welcome-nurture',
  status: 'active',
  steps: [
    {
      delaySeconds: 0,
      name: 'Day 0 intro',
      stepKey: 'day_0',
      templateSlug: 'sequence.welcome.day_0.v1',
    },
    {
      delaySeconds: 86_400,
      lifecycleKind: 'nurture_day_1',
      name: 'Day 1 value',
      stepKey: 'day_1',
      templateSlug: 'sequence.welcome.day_1.v1',
    },
  ],
})

const campaignRow = {
  audience: 'sales_qualified_leads',
  id: 'email_campaign_welcome',
  metadata_json: '{}',
  name: 'Welcome nurture',
  slug: 'welcome-nurture',
  source_authority_ref:
    'operator.email_sequence_authoring.v1:github:operator:welcome-nurture',
  status: 'active',
}

const stepRows = [
  {
    campaign_id: campaignRow.id,
    delay_seconds: 0,
    id: 'email_campaign_step_day_0',
    lifecycle_kind: null,
    metadata_json: '{}',
    name: 'Day 0 intro',
    status: 'active',
    step_key: 'day_0',
    template_slug: 'sequence.welcome.day_0.v1',
  },
  {
    campaign_id: campaignRow.id,
    delay_seconds: 86_400,
    id: 'email_campaign_step_day_1',
    lifecycle_kind: 'nurture_day_1',
    metadata_json: '{}',
    name: 'Day 1 value',
    status: 'active',
    step_key: 'day_1',
    template_slug: 'sequence.welcome.day_1.v1',
  },
]

describe('email sequence authoring schema', () => {
  test('rejects an invalid slug', () => {
    expect(() =>
      decodeCreateEmailSequenceRequest({
        audience: 'leads',
        name: 'Bad',
        slug: 'Not A Slug',
        steps: [
          {
            delaySeconds: 0,
            name: 'x',
            stepKey: 'x',
            templateSlug: 'x',
          },
        ],
      }),
    ).toThrow()
  })

  test('rejects an empty step list', () => {
    expect(() =>
      decodeCreateEmailSequenceRequest({
        audience: 'leads',
        name: 'No steps',
        slug: 'no-steps',
        steps: [],
      }),
    ).toThrow()
  })

  test('rejects a negative delay', () => {
    expect(() =>
      decodeCreateEmailSequenceRequest({
        audience: 'leads',
        name: 'Neg',
        slug: 'neg-delay',
        steps: [
          {
            delaySeconds: -1,
            name: 'x',
            stepKey: 'x',
            templateSlug: 'x',
          },
        ],
      }),
    ).toThrow()
  })

  test('rejects an enroll request without an @ email', () => {
    expect(() =>
      decodeEnrollSubscriberRequest({ email: 'no-at-sign' }),
    ).toThrow()
  })

  test('accepts a valid status update', () => {
    expect(decodeUpdateEmailSequenceStatusRequest({ status: 'paused' })).toEqual(
      { status: 'paused' },
    )
  })
})

describe('createEmailSequence', () => {
  test('upserts a campaign and its ordered steps reusing 0063 tables', async () => {
    const db = new RecordingD1Database()
    // readEmailCampaignBySlug after insert, then listEmailCampaignSteps in
    // archiveRemovedSequenceSteps, then final listEmailCampaignSteps.
    db.nextFirst.push(campaignRow)
    db.nextAll.push(stepRows, stepRows)

    const definition = await createEmailSequence(
      db as unknown as D1Database,
      'github:operator',
      sequenceRequest,
      runtime,
    )

    expect(definition.campaign.id).toBe('email_campaign_welcome')
    expect(definition.steps.map(step => step.stepKey)).toEqual([
      'day_0',
      'day_1',
    ])
    expect(
      db.runs.filter(run =>
        run.query.includes('INSERT INTO email_campaigns'),
      ),
    ).toHaveLength(1)
    expect(
      db.runs.filter(run =>
        run.query.includes('INSERT INTO email_campaign_steps'),
      ),
    ).toHaveLength(2)
    // No new tables touched beyond the migration 0063 set.
    expect(JSON.stringify(db.runs)).not.toContain('email_sequences')
  })

  test('archives previously authored steps that are no longer present', async () => {
    const db = new RecordingD1Database()
    const staleStep = {
      campaign_id: campaignRow.id,
      delay_seconds: 259_200,
      id: 'email_campaign_step_day_3',
      lifecycle_kind: null,
      metadata_json: '{}',
      name: 'Day 3 stale',
      status: 'active',
      step_key: 'day_3',
      template_slug: 'sequence.welcome.day_3.v1',
    }
    db.nextFirst.push(campaignRow)
    db.nextAll.push([...stepRows, staleStep], stepRows)

    await createEmailSequence(
      db as unknown as D1Database,
      'github:operator',
      sequenceRequest,
      runtime,
    )

    const archiveRuns = db.runs.filter(
      run =>
        run.query.includes('UPDATE email_campaign_steps') &&
        run.query.includes("status = 'archived'"),
    )
    expect(archiveRuns).toHaveLength(1)
    expect(archiveRuns[0]?.values).toContain('email_campaign_step_day_3')
  })
})

describe('updateEmailSequenceStatus', () => {
  test('returns null when the sequence does not exist', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(null)

    const result = await updateEmailSequenceStatus(
      db as unknown as D1Database,
      'missing',
      { status: 'paused' },
      runtime,
    )

    expect(result).toBeNull()
    expect(db.runs).toHaveLength(0)
  })

  test('pauses the campaign and its steps', async () => {
    const db = new RecordingD1Database()
    const pausedRow = { ...campaignRow, status: 'paused' }
    const pausedSteps = stepRows.map(step => ({ ...step, status: 'paused' }))
    db.nextFirst.push(campaignRow, pausedRow)
    db.nextAll.push(pausedSteps)

    const result = await updateEmailSequenceStatus(
      db as unknown as D1Database,
      'welcome-nurture',
      { status: 'paused' },
      runtime,
    )

    expect(result?.campaign.status).toBe('paused')
    expect(result?.steps.every(step => step.status === 'paused')).toBe(true)
    const campaignUpdate = db.runs.find(run =>
      run.query.includes('UPDATE email_campaigns'),
    )
    expect(campaignUpdate?.values).toContain('paused')
    const stepUpdate = db.runs.find(run =>
      run.query.includes('UPDATE email_campaign_steps'),
    )
    expect(stepUpdate?.values).toContain('paused')
  })

  test('archives the campaign and reports the terminal state', async () => {
    const db = new RecordingD1Database()
    // First read returns the live campaign; after archive the slug read returns
    // null (archived_at filter hides it).
    db.nextFirst.push(campaignRow, null)

    const result = await updateEmailSequenceStatus(
      db as unknown as D1Database,
      'welcome-nurture',
      { status: 'archived' },
      runtime,
    )

    expect(result?.campaign.status).toBe('archived')
    expect(result?.steps).toEqual([])
    const campaignUpdate = db.runs.find(run =>
      run.query.includes('UPDATE email_campaigns'),
    )
    // archived_at is the now timestamp for archive.
    expect(campaignUpdate?.values).toContain('2026-06-05T12:00:00.000Z')
  })
})

describe('enrollSubscriberInSequence', () => {
  test('returns null when the sequence does not exist', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(null)

    const result = await enrollSubscriberInSequence(
      db as unknown as D1Database,
      'missing',
      { email: 'lead@example.com' },
      'github:operator',
      runtime,
    )

    expect(result).toBeNull()
  })

  test('skips a drip-suppressed subscriber without scheduling', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(campaignRow, { id: 'email_suppression_1' })

    const result = await enrollSubscriberInSequence(
      db as unknown as D1Database,
      'welcome-nurture',
      { email: 'lead@example.com' },
      'github:operator',
      runtime,
    )

    expect(result).toEqual({ reason: 'drip_suppressed', status: 'skipped' })
    expect(
      db.runs.filter(run => run.query.includes('INSERT INTO')),
    ).toHaveLength(0)
  })

  test('skips a drip-opted-out subscriber', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(campaignRow, null, {
      drip_opt_in: 0,
      marketing_opt_in: 1,
      transactional_opt_in: 1,
    })

    const result = await enrollSubscriberInSequence(
      db as unknown as D1Database,
      'welcome-nurture',
      { email: 'lead@example.com' },
      'github:operator',
      runtime,
    )

    expect(result).toEqual({
      reason: 'drip_preference_disabled',
      status: 'skipped',
    })
  })

  test('enrolls an eligible subscriber and schedules per-step sends', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(
      campaignRow, // readEmailCampaignBySlug
      null, // isEmailSuppressed
      null, // readEmailPreferenceAllows (no preference row => allowed)
      {
        // readEmailCampaignEnrollmentByIdempotencyKey
        campaign_id: campaignRow.id,
        email: 'lead@example.com',
        id: 'email_campaign_enrollment_welcome',
        idempotency_key:
          'email_campaign_enrollment:email_campaign_welcome:lead@example.com',
        metadata_json: '{}',
        source_authority_ref:
          'operator.email_sequence_authoring.v1:github:operator:welcome-nurture',
        status: 'active',
        user_id: null,
      },
    )
    db.nextAll.push(stepRows) // listEmailCampaignSteps for sends

    const result = await enrollSubscriberInSequence(
      db as unknown as D1Database,
      'welcome-nurture',
      { displayName: 'Lead', email: 'LEAD@example.com' },
      'github:operator',
      runtime,
    )

    expect(result).toEqual({
      campaignId: 'email_campaign_welcome',
      enrollmentId: 'email_campaign_enrollment_welcome',
      scheduledSendCount: 2,
      status: 'enrolled',
    })

    const sendRuns = db.runs.filter(run =>
      run.query.includes('INSERT INTO email_campaign_sends'),
    )
    expect(sendRuns).toHaveLength(2)
    // dueAt (index 6) honors per-step delaySeconds from now.
    expect(sendRuns.map(run => run.values[6])).toEqual([
      '2026-06-05T12:00:00.000Z',
      '2026-06-06T12:00:00.000Z',
    ])
    // idempotency_key (index 8) is per enrollment+stepKey.
    expect(sendRuns.map(run => run.values[8])).toEqual([
      'email_campaign_send:email_campaign_enrollment_welcome:day_0',
      'email_campaign_send:email_campaign_enrollment_welcome:day_1',
    ])
  })
})
