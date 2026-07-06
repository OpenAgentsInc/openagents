import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import { EmailAddress, ResendEmailSender, WorkerSecret } from './config'
import { makeOperatorEmailInspectionRoutes } from './operator-email-inspection-routes'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type StoredEmailMessage = Readonly<{
  action_submission_id: string | null
  created_at: string
  error_message: string | null
  error_name: string | null
  id: string
  idempotency_key: string
  kind: string
  provider: string | null
  provider_message_id: string | null
  source_authority_ref: string
  status: 'reserved' | 'rendered' | 'accepted' | 'failed' | 'draft_recorded'
  template_slug: string
  updated_at: string
}>

type StoredEmailDelivery = Readonly<{
  attempted_at: string
  completed_at: string | null
  error_message: string | null
  error_name: string | null
  id: string
  message_id: string
  provider: string
  provider_message_id: string | null
  status: string
}>

type StoredEventLink = Readonly<{
  assignment_id: string | null
  event_id: string
  event_type: string
  event_source: 'assignment' | 'site'
  message_id: string
  site_id: string | null
  software_order_id: string | null
}>

type StoredSiteEmailTarget = Readonly<{
  active_deployment_id: string | null
  active_version_id: string | null
  deployment_url: string | null
  display_name: string | null
  primary_email: string | null
  site_id: string
  site_title: string
  software_order_id: string | null
  target_user_id: string | null
  version_id: string | null
}>

type StoredAssignment = Readonly<{
  current_run_id: string | null
  goal_id: string | null
  id: string
  software_order_id: string | null
  visibility: 'private' | 'team' | 'public'
}>

class EmailInspectionStore {
  assignment: StoredAssignment | null = {
    current_run_id: 'agent_run_otec',
    goal_id: 'agent_goal_otec',
    id: 'adjutant_assignment_otec',
    software_order_id: 'software_order_otec',
    visibility: 'public',
  }
  deliveries: Array<StoredEmailDelivery> = [
    {
      attempted_at: '2026-06-05T12:00:00.000Z',
      completed_at: '2026-06-05T12:00:01.000Z',
      error_message: null,
      error_name: null,
      id: 'email_delivery_accepted',
      message_id: 'email_msg_site_deployed',
      provider: 'resend',
      provider_message_id: 'email_provider_site_deployed',
      status: 'accepted',
    },
    {
      attempted_at: '2026-06-05T12:05:00.000Z',
      completed_at: '2026-06-05T12:05:01.000Z',
      error_message:
        'Domain missing. Bearer secret-token-value-123456 should be redacted.',
      error_name: 'validation_error',
      id: 'email_delivery_failed',
      message_id: 'email_msg_failed',
      provider: 'resend',
      provider_message_id: null,
      status: 'failed',
    },
  ]
  links: Array<StoredEventLink> = [
    {
      assignment_id: null,
      event_id: 'site_event_deployed',
      event_source: 'site',
      event_type: 'adjutant.notification.deployed',
      message_id: 'email_msg_site_deployed',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
    },
    {
      assignment_id: 'adjutant_assignment_otec',
      event_id: 'assignment_event_deployed',
      event_source: 'assignment',
      event_type: 'adjutant.notification.deployed',
      message_id: 'email_msg_site_deployed',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
    },
    {
      assignment_id: 'adjutant_assignment_otec',
      event_id: 'assignment_event_skipped',
      event_source: 'assignment',
      event_type: 'adjutant.notification.review_ready',
      message_id: 'email_msg_skipped',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
    },
    {
      assignment_id: 'adjutant_assignment_otec',
      event_id: 'assignment_event_failed',
      event_source: 'assignment',
      event_type: 'adjutant.notification.review_ready',
      message_id: 'email_msg_failed',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
    },
  ]
  messages: Array<StoredEmailMessage> = [
    {
      action_submission_id: null,
      created_at: '2026-06-05T11:59:00.000Z',
      error_message: null,
      error_name: null,
      id: 'email_msg_site_deployed',
      idempotency_key:
        'order_sites_email:site_deployed:software_order_otec:adjutant_assignment_otec:site_project_otec:deployment_1',
      kind: 'operator_notification',
      provider: 'resend',
      provider_message_id: 'email_provider_site_deployed',
      source_authority_ref: 'system.order_sites_lifecycle_email.v1',
      status: 'accepted',
      template_slug: 'order_sites.site_deployed.v1',
      updated_at: '2026-06-05T12:00:01.000Z',
    },
    {
      action_submission_id: null,
      created_at: '2026-06-05T12:02:00.000Z',
      error_message: 'Resend email configuration is not set.',
      error_name: 'email_config_missing',
      id: 'email_msg_skipped',
      idempotency_key:
        'order_sites_email:review_ready:software_order_otec:adjutant_assignment_otec:site_project_otec:event_review_ready',
      kind: 'operator_notification',
      provider: 'resend',
      provider_message_id: null,
      source_authority_ref: 'system.order_sites_lifecycle_email.v1',
      status: 'failed',
      template_slug: 'order_sites.review_ready.v1',
      updated_at: '2026-06-05T12:02:01.000Z',
    },
    {
      action_submission_id: null,
      created_at: '2026-06-05T12:04:00.000Z',
      error_message:
        'Domain missing. Bearer secret-token-value-123456 should be redacted.',
      error_name: 'validation_error',
      id: 'email_msg_failed',
      idempotency_key:
        'order_sites_email:review_ready:software_order_otec:adjutant_assignment_otec:site_project_otec:event_failed',
      kind: 'operator_notification',
      provider: 'resend',
      provider_message_id: null,
      source_authority_ref: 'system.order_sites_lifecycle_email.v1',
      status: 'failed',
      template_slug: 'order_sites.review_ready.v1',
      updated_at: '2026-06-05T12:05:01.000Z',
    },
  ]
  target: StoredSiteEmailTarget | null = {
    active_deployment_id: 'site_deployment_otec',
    active_version_id: 'site_version_otec',
    deployment_url: 'https://sites.openagents.com/otec',
    display_name: 'Alex Customer',
    primary_email: 'alex.customer@example.com',
    site_id: 'site_project_otec',
    site_title: 'OTEC Floating Datacenter',
    software_order_id: 'software_order_otec',
    target_user_id: 'github:customer',
    version_id: 'site_version_otec',
  }
  sendCount = 0
}

// CFG-4 Domain 2 (#8519): the customer `users` profile serves from the
// Postgres identity handle — backed by the same fixture target.
const inspectionIdentityDb = (store: EmailInspectionStore) => ({
  batch: () => Promise.resolve(),
  query: (sql: string, params: ReadonlyArray<unknown> = []) => {
    const target = store.target
    return Promise.resolve(
      sql.includes('FROM users') &&
        target !== null &&
        target.target_user_id !== null &&
        params.map(String).includes(target.target_user_id)
        ? [
            {
              avatar_url: null,
              created_at: '2026-06-01T00:00:00.000Z',
              deleted_at: null,
              display_name: target.display_name,
              github_id: null,
              github_username: null,
              id: target.target_user_id,
              kind: 'human',
              primary_email: target.primary_email,
              status: 'active',
            },
          ]
        : [],
    )
  },
})

class EmailInspectionStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: EmailInspectionStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM email_messages')) {
      const idempotencyKey = String(this.values[0])
      const message = this.store.messages.find(
        item => item.idempotency_key === idempotencyKey,
      )

      return Promise.resolve((message as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_projects')) {
      // CFG-4 Domain 2 (#8519): the D1 half no longer carries the users
      // fields; it returns the order's user id and the identity handle
      // (inspectionIdentityDb) serves the profile.
      const target = this.store.target
      if (target === null) return Promise.resolve(null)
      const {
        display_name: _displayName,
        primary_email: _primaryEmail,
        target_user_id,
        ...rest
      } = target
      return Promise.resolve({
        ...rest,
        order_user_id: target_user_id,
      } as T)
    }

    if (this.query.includes('FROM adjutant_assignments')) {
      return Promise.resolve(this.store.assignment as T | null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO email_messages')) {
      const [
        id,
        kind,
        ,
        ,
        ,
        ,
        ,
        ,
        ,
        ,
        templateSlug,
        ,
        idempotencyKey,
        sourceAuthorityRef,
        ,
        ,
        createdAt,
        updatedAt,
      ] = this.values

      if (
        this.store.messages.some(
          message => message.idempotency_key === idempotencyKey,
        )
      ) {
        return Promise.resolve({ success: true } as D1Result<T>)
      }

      this.store.messages.unshift({
        action_submission_id: null,
        created_at: String(createdAt),
        error_message: null,
        error_name: null,
        id: String(id),
        idempotency_key: String(idempotencyKey),
        kind: String(kind),
        provider: null,
        provider_message_id: null,
        source_authority_ref: String(sourceAuthorityRef),
        status: 'rendered',
        template_slug: String(templateSlug),
        updated_at: String(updatedAt),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE email_messages') &&
      this.query.includes("status = 'rendered'")
    ) {
      const idempotencyKey = String(this.values.at(-1))

      this.store.messages = this.store.messages.map(message =>
        message.idempotency_key === idempotencyKey &&
        message.status !== 'accepted'
          ? {
              ...message,
              status: 'rendered',
              updated_at: String(this.values.at(-2)),
            }
          : message,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE email_messages') &&
      this.query.includes("status = 'accepted'")
    ) {
      const [provider, providerMessageId, updatedAt, messageId] = this.values

      this.store.messages = this.store.messages.map(message =>
        message.id === messageId
          ? {
              ...message,
              error_message: null,
              error_name: null,
              provider: String(provider),
              provider_message_id:
                providerMessageId === null ? null : String(providerMessageId),
              status: 'accepted',
              updated_at: String(updatedAt),
            }
          : message,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE email_messages') &&
      this.query.includes("status = 'failed'")
    ) {
      const [provider, , errorMessage, updatedAt, messageId] = this.values

      this.store.messages = this.store.messages.map(message =>
        message.id === messageId
          ? {
              ...message,
              error_message: errorMessage === null ? null : String(errorMessage),
              error_name: 'resend_error',
              provider: String(provider),
              provider_message_id: null,
              status: 'failed',
              updated_at: String(updatedAt),
            }
          : message,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO email_deliveries')) {
      const [
        id,
        messageId,
        provider,
        providerMessageId,
        ,
        status,
        errorName,
        errorMessage,
        ,
        attemptedAt,
        completedAt,
      ] = this.values

      this.store.deliveries.unshift({
        attempted_at: String(attemptedAt),
        completed_at: completedAt === null ? null : String(completedAt),
        error_message: errorMessage === null ? null : String(errorMessage),
        error_name: errorName === null ? null : String(errorName),
        id: String(id),
        message_id: String(messageId),
        provider: String(provider),
        provider_message_id:
          providerMessageId === null ? null : String(providerMessageId),
        status: String(status),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO adjutant_assignment_events')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        ,
        ,
        ,
        ,
        ,
        ,
        emailMessageId,
      ] = this.values

      this.store.links.unshift({
        assignment_id: String(assignmentId),
        event_id: String(id),
        event_source: 'assignment',
        event_type: 'adjutant.notification.review_ready',
        message_id: String(emailMessageId),
        site_id: String(siteId),
        software_order_id:
          softwareOrderId === null ? null : String(softwareOrderId),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_events')) {
      const [id, siteId, , , , , , , emailMessageId] = this.values

      this.store.links.unshift({
        assignment_id: null,
        event_id: String(id),
        event_source: 'site',
        event_type: 'adjutant.notification.review_ready',
        message_id: String(emailMessageId),
        site_id: String(siteId),
        software_order_id: this.store.target?.software_order_id ?? null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM email_messages')) {
      const siteId = this.values.find(
        value => typeof value === 'string' && value.startsWith('site_project_'),
      ) as string | undefined
      const softwareOrderId = this.values.find(
        value => typeof value === 'string' && value.startsWith('software_order_'),
      ) as string | undefined
      const messageIds = new Set(
        this.store.links
          .filter(
            link =>
              (siteId === undefined || link.site_id === siteId) &&
              (softwareOrderId === undefined ||
                link.software_order_id === softwareOrderId),
          )
          .map(link => link.message_id),
      )
      const results = this.store.messages
        .filter(message => messageIds.has(message.id))
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at))

      return Promise.resolve({
        results: results as Array<T>,
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('FROM email_deliveries')) {
      const messageId = String(this.values[0])
      const results = this.store.deliveries
        .filter(delivery => delivery.message_id === messageId)
        .sort((left, right) => right.attempted_at.localeCompare(left.attempted_at))

      return Promise.resolve({
        results: results as Array<T>,
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('FROM site_events')) {
      const messageId = String(this.values[0])
      const results = this.store.links.filter(link => link.message_id === messageId)

      return Promise.resolve({
        results: results as Array<T>,
        success: true,
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.resolve([])
  }
}

const db = (store: EmailInspectionStore): D1Database =>
  ({
    batch: () => Promise.reject(new Error('batch not used')),
    dump: () => Promise.reject(new Error('dump not used')),
    exec: () => Promise.reject(new Error('exec not used')),
    prepare: (query: string) => new EmailInspectionStatement(query, store),
    withSession: () => {
      throw new Error('sessions not used')
    },
  }) as unknown as D1Database

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
  store: EmailInspectionStore,
  request: Request,
  options: { hasAdminApiToken?: boolean; withResendConfig?: boolean } = {},
) => {
  const route = makeOperatorEmailInspectionRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    getAppOrigin: () => 'https://openagents.com',
    getResendEmailConfig: () =>
      options.withResendConfig === true
        ? {
            apiKey: Redacted.make(WorkerSecret.make('resend-test-key')),
            fromEmail: ResendEmailSender.make(
              'OpenAgents <sites@openagents.com>',
            ),
            replyToEmail: EmailAddress.make('support@openagents.com'),
          }
        : undefined,
    emailFetcher: async () => {
      store.sendCount += 1

      return new Response(JSON.stringify({ id: 'email_review_ready_test' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    },
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    requireAdminApiToken: () =>
      Promise.resolve(options.hasAdminApiToken === true),
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
  }).routeOperatorEmailInspectionRequest(
    request,
    {
      IDENTITY_DB: inspectionIdentityDb(store),
      OPENAGENTS_DB: db(store),
    },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

describe('operator email inspection routes', () => {
  test('requires an admin browser session', async () => {
    const response = await runRoute(
      null,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries?softwareOrderId=software_order_otec',
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('requires admin session for review-ready smoke route', async () => {
    const response = await runRoute(
      null,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries/review-ready-smoke',
        {
          body: JSON.stringify({ siteId: 'site_project_otec' }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('dry-runs review-ready Site revision email with rendered template metadata', async () => {
    const response = await runRoute(
      adminSession,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries/review-ready-smoke',
        {
          body: JSON.stringify({ dryRun: true, siteId: 'site_project_otec' }),
          method: 'POST',
        },
      ),
      { withResendConfig: true },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      smoke: expect.objectContaining({
        dryRun: true,
        emailStatus: 'skipped',
        idempotencyKey:
          'order_sites_email:review_ready:software_order_otec:adjutant_assignment_otec:site_project_otec:site_version_otec',
        rendered: {
          subject: 'OTEC Floating Datacenter is ready for review',
          templateSlug: 'order_sites.review_ready.v1',
        },
        skipReason: 'dry_run',
        siteId: 'site_project_otec',
        softwareOrderId: 'software_order_otec',
        versionId: 'site_version_otec',
      }),
    })
  })

  test('dry-runs review-ready Site revision email with admin API token', async () => {
    const response = await runRoute(
      null,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries/review-ready-smoke',
        {
          body: JSON.stringify({
            dryRun: true,
            siteId: 'site_project_otec',
          }),
          method: 'POST',
        },
      ),
      { hasAdminApiToken: true, withResendConfig: true },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      smoke: expect.objectContaining({
        dryRun: true,
        emailStatus: 'skipped',
        siteId: 'site_project_otec',
        skipReason: 'dry_run',
      }),
    })
  })

  test('surfaces missing Resend config for review-ready smoke without sending', async () => {
    const response = await runRoute(
      adminSession,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries/review-ready-smoke',
        {
          body: JSON.stringify({ siteId: 'site_project_otec' }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      smoke: expect.objectContaining({
        dryRun: false,
        emailStatus: 'skipped',
        idempotencyKey:
          'order_sites_email:review_ready:software_order_otec:adjutant_assignment_otec:site_project_otec:site_version_otec',
        skipReason: 'email_config_missing',
      }),
    })
  })

  test('sends review-ready smoke through the ledger idempotently', async () => {
    const store = new EmailInspectionStore()
    const request = () =>
      new Request(
        'https://openagents.com/api/operator/email-deliveries/review-ready-smoke',
        {
          body: JSON.stringify({ siteId: 'site_project_otec' }),
          method: 'POST',
        },
      )

    const first = await runRoute(adminSession, store, request(), {
      withResendConfig: true,
    })
    const second = await runRoute(adminSession, store, request(), {
      withResendConfig: true,
    })

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    const firstBody = await first.json()
    const secondBody = await second.json()

    expect(store.sendCount).toBe(1)
    expect(firstBody).toEqual({
      smoke: expect.objectContaining({
        dryRun: false,
        emailMessageId: expect.any(String),
        emailStatus: 'accepted',
        idempotencyKey:
          'order_sites_email:review_ready:software_order_otec:adjutant_assignment_otec:site_project_otec:site_version_otec',
        providerMessageId: 'email_review_ready_test',
      }),
    })
    expect(secondBody).toEqual({
      smoke: expect.objectContaining({
        dryRun: false,
        emailStatus: 'accepted',
        providerMessageId: 'email_review_ready_test',
      }),
    })
    expect(
      store.links.filter(
        link => link.event_type === 'adjutant.notification.review_ready',
      ).length,
    ).toBeGreaterThanOrEqual(2)
  })

  test('inspects order-scoped delivery status without exposing bodies or secrets', async () => {
    const response = await runRoute(
      adminSession,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries?softwareOrderId=software_order_otec',
      ),
    )

    expect(response.status).toBe(200)
    const body = await response.json()

    expect(body).toEqual({
      inspection: expect.objectContaining({
        messages: [
          expect.objectContaining({
            deliveryAttempts: 1,
            emailMessageId: 'email_msg_failed',
            errorMessage:
              'Domain missing. Bearer [REDACTED] should be redacted.',
            latestDelivery: expect.objectContaining({
              errorMessage:
                'Domain missing. Bearer [REDACTED] should be redacted.',
              status: 'failed',
            }),
            relatedAssignmentIds: ['adjutant_assignment_otec'],
            relatedOrderIds: ['software_order_otec'],
            relatedSiteIds: ['site_project_otec'],
            status: 'failed',
          }),
          expect.objectContaining({
            emailMessageId: 'email_msg_skipped',
            skippedReason: 'email_config_missing',
            status: 'skipped',
          }),
          expect.objectContaining({
            deliveryAttempts: 1,
            emailMessageId: 'email_msg_site_deployed',
            providerMessageId: 'email_provider_site_deployed',
            status: 'accepted',
          }),
        ],
        summary: {
          accepted: 1,
          failed: 1,
          messageCount: 3,
          skipped: 1,
          suppressed: 0,
        },
      }),
    })
    expect(JSON.stringify(body)).not.toContain('text_body')
    expect(JSON.stringify(body)).not.toContain('html_body')
    expect(JSON.stringify(body)).not.toContain('secret-token-value-123456')
  })

  test('inspects Site-scoped delivery status', async () => {
    const response = await runRoute(
      adminSession,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries?siteId=site_project_otec',
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        inspection: expect.objectContaining({
          messages: expect.arrayContaining([
            expect.objectContaining({
              emailMessageId: 'email_msg_site_deployed',
              eventRefs: expect.arrayContaining([
                'site:adjutant.notification.deployed:site_event_deployed',
              ]),
            }),
          ]),
          scope: { siteId: 'site_project_otec' },
        }),
      }),
    )
  })

  test('returns empty inspection for scopes with no messages', async () => {
    const response = await runRoute(
      adminSession,
      new EmailInspectionStore(),
      new Request(
        'https://openagents.com/api/operator/email-deliveries?softwareOrderId=software_order_missing',
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      inspection: {
        messages: [],
        scope: { softwareOrderId: 'software_order_missing' },
        summary: {
          accepted: 0,
          failed: 0,
          messageCount: 0,
          skipped: 0,
          suppressed: 0,
        },
      },
    })
  })

  test('rejects missing inspection scope', async () => {
    const response = await runRoute(
      adminSession,
      new EmailInspectionStore(),
      new Request('https://openagents.com/api/operator/email-deliveries'),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'email_inspection_invalid_scope',
      reason: 'siteId or softwareOrderId is required.',
    })
  })
})
