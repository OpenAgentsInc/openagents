import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeEmailSequenceAuthoringRoutes } from './email-sequence-authoring-routes'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

// Queued-fixture D1 fake mirroring email-onboarding-drip.test.ts: reads come
// from nextFirst/nextAll in FIFO order; runs are recorded.
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
    return Promise.resolve({
      meta: {},
      results: (this.db.nextAll.shift() ?? []) as Array<T>,
      success: true,
    } as D1Result<T>)
  }

  first<T>(): Promise<T | null> {
    return Promise.resolve((this.db.nextFirst.shift() ?? null) as T | null)
  }

  run(): Promise<void> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve()
  }
}

class RecordingD1Database {
  readonly nextAll: Array<Array<unknown>> = []
  readonly nextFirst: Array<unknown | null> = []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

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
]

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

const adminSession: TestSession = {
  user: {
    email: 'chris@openagents.com',
    userId: 'github:operator',
  },
}

const runRoute = (
  session: TestSession | null,
  store: RecordingD1Database,
  request: Request,
  options: { hasAdminApiToken?: boolean } = {},
) => {
  const route = makeEmailSequenceAuthoringRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    requireAdminApiToken: () =>
      Promise.resolve(options.hasAdminApiToken === true),
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
  }).routeEmailSequenceAuthoringRequest(
    request,
    { OPENAGENTS_DB: store as unknown as D1Database },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const jsonBody = async (response: Response): Promise<any> => response.json()

const createRequest = (body: unknown): Request =>
  new Request('https://openagents.com/api/operator/email-sequences', {
    body: JSON.stringify(body),
    method: 'POST',
  })

const validCreateBody = {
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
  ],
}

describe('email sequence authoring routes', () => {
  test('returns undefined for unowned paths', () => {
    const route = makeEmailSequenceAuthoringRoutes({
      appendRefreshedSessionCookies: response => response,
      isOpenAgentsAdminEmail: () => true,
      requireBrowserSession: () => Promise.resolve(adminSession),
    }).routeEmailSequenceAuthoringRequest(
      new Request('https://openagents.com/api/operator/email-deliveries'),
      { OPENAGENTS_DB: new RecordingD1Database() as unknown as D1Database },
      executionContext(),
    )

    expect(route).toBeUndefined()
  })

  test('requires an admin session to create a sequence', async () => {
    const response = await runRoute(
      null,
      new RecordingD1Database(),
      createRequest(validCreateBody),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('forbids a non-admin session', async () => {
    const response = await runRoute(
      { user: { email: 'someone@example.com', userId: 'github:other' } },
      new RecordingD1Database(),
      createRequest(validCreateBody),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'forbidden' })
  })

  test('rejects non-POST methods', async () => {
    const response = await runRoute(
      adminSession,
      new RecordingD1Database(),
      new Request('https://openagents.com/api/operator/email-sequences', {
        method: 'GET',
      }),
    )

    expect(response.status).toBe(405)
  })

  test('rejects an invalid create payload with 400', async () => {
    const response = await runRoute(
      adminSession,
      new RecordingD1Database(),
      createRequest({ ...validCreateBody, slug: 'Not A Slug' }),
    )

    expect(response.status).toBe(400)
    const body = await jsonBody(response)
    expect(body.error).toBe('bad_request')
  })

  test('creates a sequence for an admin session', async () => {
    const store = new RecordingD1Database()
    store.nextFirst.push(campaignRow)
    store.nextAll.push(stepRows, stepRows)

    const response = await runRoute(
      adminSession,
      store,
      createRequest(validCreateBody),
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    const body = await jsonBody(response)
    expect(body.sequence.campaign.slug).toBe('welcome-nurture')
    expect(body.sequence.steps).toHaveLength(1)
  })

  test('creates a sequence with an admin API token (no browser session)', async () => {
    const store = new RecordingD1Database()
    store.nextFirst.push(campaignRow)
    store.nextAll.push(stepRows, stepRows)

    const response = await runRoute(null, store, createRequest(validCreateBody), {
      hasAdminApiToken: true,
    })

    expect(response.status).toBe(201)
  })

  test('updates sequence status', async () => {
    const store = new RecordingD1Database()
    const pausedRow = { ...campaignRow, status: 'paused' }
    store.nextFirst.push(campaignRow, pausedRow)
    store.nextAll.push(stepRows.map(step => ({ ...step, status: 'paused' })))

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        'https://openagents.com/api/operator/email-sequences/welcome-nurture/status',
        { body: JSON.stringify({ status: 'paused' }), method: 'POST' },
      ),
    )

    expect(response.status).toBe(200)
    const body = await jsonBody(response)
    expect(body.sequence.campaign.status).toBe('paused')
  })

  test('returns 404 when updating a missing sequence', async () => {
    const store = new RecordingD1Database()
    store.nextFirst.push(null)

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        'https://openagents.com/api/operator/email-sequences/missing/status',
        { body: JSON.stringify({ status: 'paused' }), method: 'POST' },
      ),
    )

    expect(response.status).toBe(404)
    await expect(response.json()).resolves.toEqual({
      error: 'email_sequence_not_found',
    })
  })

  test('enrolls a subscriber into a sequence', async () => {
    const store = new RecordingD1Database()
    store.nextFirst.push(
      campaignRow, // readEmailCampaignBySlug
      null, // isEmailSuppressed
      null, // readEmailPreferenceAllows
      {
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
    store.nextAll.push(stepRows)

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        'https://openagents.com/api/operator/email-sequences/welcome-nurture/enroll',
        {
          body: JSON.stringify({ email: 'lead@example.com' }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(201)
    const body = await jsonBody(response)
    expect(body.enrollment).toEqual({
      campaignId: 'email_campaign_welcome',
      enrollmentId: 'email_campaign_enrollment_welcome',
      scheduledSendCount: 1,
      status: 'enrolled',
    })
  })

  test('returns 200 with skipped enrollment for a suppressed subscriber', async () => {
    const store = new RecordingD1Database()
    store.nextFirst.push(campaignRow, { id: 'email_suppression_1' })

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        'https://openagents.com/api/operator/email-sequences/welcome-nurture/enroll',
        {
          body: JSON.stringify({ email: 'lead@example.com' }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = await jsonBody(response)
    expect(body.enrollment).toEqual({
      reason: 'drip_suppressed',
      status: 'skipped',
    })
  })
})
