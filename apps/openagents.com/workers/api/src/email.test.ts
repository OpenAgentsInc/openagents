import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import { EmailAddress, ResendEmailSender, WorkerSecret } from './config'
import {
  AutopilotDecisionEmailInput,
  type CloudflareEmailBinding,
  ORDER_SITES_TRANSACTIONAL_EMAIL_KINDS,
  OrderSitesTransactionalEmailInput,
  PrivateWorkspaceInviteEmailInput,
  SiteReferralOnboardingEmailInput,
  TargetedRemakeOutreachEmailInput,
  adjutantCustomerNotificationEmailHtml,
  adjutantCustomerNotificationEmailText,
  buildOrderSitesTransactionalEmailIdempotencyKey,
  makeEmailService,
  orderSitesTransactionalEmailHtml,
  orderSitesTransactionalEmailText,
  outOfCreditsEmailHtml,
  outOfCreditsEmailText,
  runOperatorEmailLedgerSmoke,
  sendOutOfCreditsEmail,
  sendRenderedEmailViaCloudflareBinding,
  sendRenderedEmailViaCloudflareBindingWithLedger,
  siteReferralOnboardingEmailHtml,
  siteReferralOnboardingEmailText,
  targetedRemakeOutreachEmailHtml,
  targetedRemakeOutreachEmailText,
} from './email'

const emailInput = {
  appOrigin: 'https://openagents.com',
  balanceFormatted: '$0.00',
  displayName: 'Chris <Admin>',
  idempotencyKey: 'billing:out-of-credits:user_test',
  to: 'chris@openagents.com',
}

const adjutantInput = {
  appOrigin: 'https://openagents.com',
  displayName: 'Alex <Customer>',
  idempotencyKey: 'adjutant_customer_notification:assignment:run:review_ready',
  orderId: 'software_order_otec',
  siteTitle: 'OTEC <Launch>',
  siteUrl: 'https://sites.openagents.com/otec',
  stage: 'review_ready' as const,
  to: 'alex.customer@example.com',
}

const lifecycleEmailInput = (
  lifecycleKind: (typeof ORDER_SITES_TRANSACTIONAL_EMAIL_KINDS)[number],
): OrderSitesTransactionalEmailInput =>
  new OrderSitesTransactionalEmailInput({
    appOrigin: 'https://openagents.com',
    assignmentId: 'adjutant_assignment_otec',
    artifactLabel: null,
    artifactUrl: null,
    customerSafeStatus: `customer-safe status for ${lifecycleKind}`,
    displayName: 'Alex <Customer>',
    eventRef: `event_${lifecycleKind}`,
    lifecycleKind,
    nextAction: `customer-safe next action for ${lifecycleKind}`,
    orderId: 'software_order_otec',
    revisionUrl:
      'https://sites.openagents.com/otec/versions/site_version_otec_20260605_revision_3',
    safeReason:
      lifecycleKind === 'unavailable_declined'
        ? 'The request needs a smaller first slice before OpenAgents can continue.'
        : null,
    siteId: 'site_otec',
    siteTitle: 'OTEC <Launch>',
    siteUrl: 'https://sites.openagents.com/otec',
    sourceAuthorityRefs: [
      'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md#epic-o',
    ],
    targetRefs: ['software_order_otec', 'site_otec'],
    to: 'alex.customer@example.com',
  })

const referralOnboardingInput = new SiteReferralOnboardingEmailInput({
  appOrigin: 'https://openagents.com',
  displayName: 'Alex <Customer>',
  idempotencyKey:
    'site_referral_onboarding:github:customer:referral_attribution_otec',
  sourceLabel: 'OTEC Floating Datacenter',
  sourceSiteUrl: 'https://sites.openagents.com/otec',
  to: 'alex.customer@example.com',
})

const targetedRemakeOutreachInput = new TargetedRemakeOutreachEmailInput({
  appOrigin: 'https://openagents.com',
  campaignId: 'targeted_site_campaign_texas_energy',
  conceptDisclosure:
    'This is an OpenAgents concept preview, not a site operated or endorsed by your organization.',
  displayName: 'Alex <Customer>',
  idempotencyKey:
    'targeted_remake_outreach:targeted_site_remake_preview_otec_1:operator_review_otec_1',
  meetingUrl: 'https://openagents.com/meet/otec-review',
  postalAddress: 'OpenAgents, 548 Market St, San Francisco, CA 94104',
  preferencesUrl: 'https://openagents.com/email/preferences',
  previewGenerationId: 'targeted_site_remake_preview_otec_1',
  previewUrl:
    'https://sites.openagents.com/concepts/targeted_site_campaign_texas_energy/otec-example',
  prospectId: 'targeted_site_prospect_otec',
  senderContact: 'chris+sites@openagents.com',
  senderName: 'Chris at OpenAgents',
  targetDomain: 'otec.example',
  targetName: 'OTEC Floating Datacenter',
  to: 'alex.customer@example.com',
  unsubscribeUrl: 'https://openagents.com/email/unsubscribe/targeted',
  valueProposition:
    'The preview focuses on clearer positioning, stronger calls to action, and a cleaner technical story.',
})

const privateWorkspaceInviteInput = new PrivateWorkspaceInviteEmailInput({
  acceptUrl:
    'https://openagents.com/api/team-workspace-invites/accept?token=invite_token',
  displayName: 'Alex <Customer>',
  expiresAt: '2026-06-19T12:00:00.000Z',
  idempotencyKey: 'team_workspace_invite:invite_1:1',
  inviteId: 'team_workspace_invite_1',
  projectId: 'team_project_1',
  teamId: 'team_1',
  to: 'alex.customer@example.com',
  workspaceLabel: 'Private <Workspace>',
})

const resendConfig = () => ({
  apiKey: Redacted.make(WorkerSecret.make('re_test')),
  fromEmail: ResendEmailSender.make('OpenAgents <billing@openagents.com>'),
  replyToEmail: EmailAddress.make('support@openagents.com'),
})

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const escapeFixture = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

type EmailMessageTestRow = Readonly<{
  created_at: string
  error_message: string | null
  error_name: string | null
  id: string
  idempotency_key: string
  kind: string
  provider: string | null
  provider_message_id: string | null
  source_authority_ref: string
  status: string
  target_user_id: string | null
  updated_at: string
}>

type EmailDeliveryTestRow = Readonly<{
  error_message: string | null
  error_name: string | null
  id: string
  message_id: string
  provider: string
  provider_idempotency_key: string
  provider_message_id: string | null
  status: string
}>

const makeEmailLedgerD1 = (): Readonly<{
  deliveries: Array<EmailDeliveryTestRow>
  db: D1Database
  messagesByIdempotencyKey: Map<string, EmailMessageTestRow>
}> => {
  const deliveries: Array<EmailDeliveryTestRow> = []
  const messagesByIdempotencyKey = new Map<string, EmailMessageTestRow>()

  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => {
      let values: ReadonlyArray<unknown> = []

      function raw<T = unknown[]>(options: {
        columnNames: true
      }): Promise<[Array<string>, ...Array<T>]>
      function raw<T = unknown[]>(options?: {
        columnNames?: false
      }): Promise<Array<T>>
      function raw<T = unknown[]>(options?: {
        columnNames?: boolean
      }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
        return options?.columnNames === true
          ? Promise.resolve([[]])
          : Promise.resolve([])
      }

      const statement: D1PreparedStatement = {
        all: <T = Record<string, unknown>>() =>
          Promise.resolve(makeResult<T>()),
        bind: (...nextValues: ReadonlyArray<unknown>) => {
          values = nextValues

          return statement
        },
        first: <T = Record<string, unknown>>() => {
          if (query.includes('FROM email_messages')) {
            const row = messagesByIdempotencyKey.get(String(values[0]))

            return Promise.resolve(
              row === undefined ? null : jsonFixture<T>(row),
            )
          }

          return Promise.resolve(null)
        },
        raw,
        run: <T = Record<string, unknown>>() => {
          if (query.includes('INSERT INTO email_messages')) {
            const idempotencyKey = String(values[12])

            if (!messagesByIdempotencyKey.has(idempotencyKey)) {
              messagesByIdempotencyKey.set(idempotencyKey, {
                created_at: String(values[16]),
                error_message: null,
                error_name: null,
                id: String(values[0]),
                idempotency_key: idempotencyKey,
                kind: String(values[1]),
                provider: null,
                provider_message_id: null,
                source_authority_ref: String(values[13]),
                status: 'rendered',
                target_user_id: values[3] === null ? null : String(values[3]),
                updated_at: String(values[17]),
              })
            }
          }

          if (
            query.includes('UPDATE email_messages') &&
            query.includes("status = 'rendered'")
          ) {
            const idempotencyKey = String(values[15])
            const row = messagesByIdempotencyKey.get(idempotencyKey)

            if (row !== undefined && row.status !== 'accepted') {
              messagesByIdempotencyKey.set(idempotencyKey, {
                ...row,
                error_message: null,
                error_name: null,
                kind: String(values[0]),
                source_authority_ref: String(values[11]),
                status: 'rendered',
                target_user_id: values[2] === null ? null : String(values[2]),
                updated_at: String(values[14]),
              })
            }
          }

          if (
            query.includes('UPDATE email_messages') &&
            query.includes("status = 'accepted'")
          ) {
            const messageId = String(values[3])
            const row = Array.from(messagesByIdempotencyKey.values()).find(
              row => row.id === messageId,
            )

            if (row !== undefined) {
              messagesByIdempotencyKey.set(row.idempotency_key, {
                ...row,
                error_message: null,
                error_name: null,
                provider: String(values[0]),
                provider_message_id:
                  values[1] === null ? null : String(values[1]),
                status: 'accepted',
                updated_at: String(values[2]),
              })
            }
          }

          if (
            query.includes('UPDATE email_messages') &&
            query.includes("status = 'failed'")
          ) {
            const messageId = String(values[4])
            const row = Array.from(messagesByIdempotencyKey.values()).find(
              row => row.id === messageId,
            )

            if (row !== undefined) {
              messagesByIdempotencyKey.set(row.idempotency_key, {
                ...row,
                error_message: String(values[2]),
                error_name: String(values[1]),
                provider: String(values[0]),
                provider_message_id: null,
                status: 'failed',
                updated_at: String(values[3]),
              })
            }
          }

          if (query.includes('INSERT INTO email_deliveries')) {
            deliveries.push({
              error_message: values[7] === null ? null : String(values[7]),
              error_name: values[6] === null ? null : String(values[6]),
              id: String(values[0]),
              message_id: String(values[1]),
              provider: String(values[2]),
              provider_idempotency_key: String(values[4]),
              provider_message_id:
                values[3] === null ? null : String(values[3]),
              status: String(values[5]),
            })
          }

          return Promise.resolve(makeResult<T>())
        },
      }

      return statement
    },
    withSession: () => ({
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      getBookmark: () => null,
      prepare: query => db.prepare(query),
    }),
  }

  return {
    db,
    deliveries,
    messagesByIdempotencyKey,
  }
}

describe('emails', () => {
  test('renders out-of-credits text with the billing link', () => {
    expect(outOfCreditsEmailText(emailInput)).toContain(
      'https://openagents.com/billing',
    )
    expect(outOfCreditsEmailText(emailInput)).toContain('$0.00')
  })

  test('escapes display names in the out-of-credits html', () => {
    expect(outOfCreditsEmailHtml(emailInput)).toContain('Chris &lt;Admin&gt;')
    expect(outOfCreditsEmailHtml(emailInput)).not.toContain('Chris <Admin>')
  })

  test('sends Resend REST requests with idempotency and reply-to headers', async () => {
    const requests: Array<Request> = []
    const fetcher: typeof fetch = async (input, init) => {
      requests.push(input instanceof Request ? input : new Request(input, init))

      return new Response(JSON.stringify({ id: 'email_test' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }

    const result = await Effect.runPromise(
      sendOutOfCreditsEmail(resendConfig(), emailInput, fetcher),
    )

    expect(result).toEqual({ id: 'email_test', ok: true })
    expect(requests).toHaveLength(1)
    const request = requests[0]

    expect(request).toBeDefined()
    expect(request!.headers.get('Idempotency-Key')).toBe(
      emailInput.idempotencyKey,
    )
    const body = await request!.json()
    expect(body).toMatchObject({
      from: 'OpenAgents <billing@openagents.com>',
      reply_to: 'support@openagents.com',
      subject: 'OpenAgents Autopilot credits exhausted',
      to: ['chris@openagents.com'],
    })
  })

  test('maps malformed Resend error payloads to the default email error', async () => {
    const fetcher: typeof fetch = async () =>
      new Response('{bad json', {
        headers: { 'content-type': 'application/json' },
        status: 500,
      })

    const result = await Effect.runPromise(
      sendOutOfCreditsEmail(
        {
          apiKey: Redacted.make(WorkerSecret.make('re_test')),
          fromEmail: ResendEmailSender.make(
            'OpenAgents <billing@openagents.com>',
          ),
        },
        emailInput,
        fetcher,
      ),
    )

    expect(result).toEqual({
      errorMessage: 'Resend email request failed.',
      errorName: 'resend_error',
      ok: false,
    })
  })

  test('sends rendered email through the Cloudflare Email binding adapter', async () => {
    const sentMessages: Array<Parameters<CloudflareEmailBinding['send']>[0]> =
      []
    const binding: CloudflareEmailBinding = {
      send: message => {
        sentMessages.push(message)

        return Promise.resolve({ messageId: 'cf_email_test' })
      },
    }
    const rendered = await Effect.runPromise(
      makeEmailService().renderOutOfCreditsEmail(resendConfig(), emailInput),
    )

    const result = await Effect.runPromise(
      sendRenderedEmailViaCloudflareBinding(binding, rendered),
    )

    expect(result).toMatchObject({
      _tag: 'EmailProviderAccepted',
      provider: 'cloudflare_email',
      providerMessageId: 'cf_email_test',
    })
    expect(sentMessages).toEqual([
      expect.objectContaining({
        from: 'OpenAgents <billing@openagents.com>',
        html: expect.stringContaining('Autopilot credits are exhausted'),
        replyTo: 'support@openagents.com',
        subject: 'OpenAgents Autopilot credits exhausted',
        text: expect.stringContaining('Your OpenAgents Autopilot credits'),
        to: 'chris@openagents.com',
      }),
    ])
    expect(sentMessages[0]!.headers).toEqual({
      'X-OpenAgents-Idempotency-Key': emailInput.idempotencyKey,
    })
  })

  test('maps Cloudflare Email binding errors to bounded provider failures', async () => {
    const error = Object.assign(new Error('recipient is not allowed'), {
      code: 'E_RECIPIENT_NOT_ALLOWED',
    })
    const binding: CloudflareEmailBinding = {
      send: () => Promise.reject(error),
    }
    const rendered = await Effect.runPromise(
      makeEmailService().renderOutOfCreditsEmail(resendConfig(), emailInput),
    )

    const result = await Effect.runPromise(
      sendRenderedEmailViaCloudflareBinding(binding, rendered),
    )

    expect(result).toMatchObject({
      _tag: 'EmailProviderRejected',
      errorMessage: 'recipient is not allowed',
      errorName: 'E_RECIPIENT_NOT_ALLOWED',
      provider: 'cloudflare_email',
    })
  })

  test('renders out-of-credits email through the Effect service', async () => {
    const rendered = await Effect.runPromise(
      makeEmailService().renderOutOfCreditsEmail(resendConfig(), emailInput),
    )

    expect(rendered.kind).toBe('billing_out_of_credits')
    expect(rendered.templateSlug).toBe('billing.out_of_credits.v1')
    expect(rendered.tags.map(tag => tag.value)).toEqual([
      'billing',
      'out_of_credits',
    ])
    expect(JSON.parse(rendered.metadataJson)).toEqual({
      policy: 'system.billing_out_of_credits.v1',
    })
  })

  test('renders site referral onboarding without payout promises', async () => {
    const text = siteReferralOnboardingEmailText(referralOnboardingInput)
    const html = siteReferralOnboardingEmailHtml(referralOnboardingInput)
    const rendered = await Effect.runPromise(
      makeEmailService().renderSiteReferralOnboardingEmail(
        resendConfig(),
        referralOnboardingInput,
      ),
    )

    expect(text).toContain('OTEC Floating Datacenter')
    expect(text).toContain('https://sites.openagents.com/otec')
    expect(text).toContain('https://openagents.com/order')
    expect(`${text}\n${html}`).not.toMatch(
      /payout|earnings|revshare|settlement|guaranteed credit/i,
    )
    expect(html).toContain('<meta name="color-scheme" content="light" />')
    expect(html).toContain('Alex &lt;Customer&gt;')
    expect(rendered.kind).toBe('crm_transactional')
    expect(rendered.templateSlug).toBe('site_referral.onboarding.v1')
    expect(JSON.parse(rendered.metadataJson)).toEqual({
      emailSubtype: 'site_referral_onboarding',
      policy: 'system.site_referral_onboarding.v1',
    })
  })

  test('renders targeted remake outreach with preview, meeting, unsubscribe, and light-safe html', async () => {
    const text = targetedRemakeOutreachEmailText(targetedRemakeOutreachInput)
    const html = targetedRemakeOutreachEmailHtml(targetedRemakeOutreachInput)
    const rendered = await Effect.runPromise(
      makeEmailService().renderTargetedRemakeOutreachEmail(
        resendConfig(),
        targetedRemakeOutreachInput,
      ),
    )

    expect(text).toContain(targetedRemakeOutreachInput.previewUrl)
    expect(text).toContain(targetedRemakeOutreachInput.meetingUrl)
    expect(text).toContain(targetedRemakeOutreachInput.unsubscribeUrl)
    expect(text).toContain(targetedRemakeOutreachInput.conceptDisclosure)
    expect(html).toContain('<meta name="color-scheme" content="light" />')
    expect(html).toContain('background:#fbfaf6 !important')
    expect(html).toContain('color:#17211f !important')
    expect(html).toContain('Alex &lt;Customer&gt;')
    expect(html).not.toContain('Alex <Customer>')
    expect(rendered.kind).toBe('operator_notification')
    expect(rendered.templateSlug).toBe('targeted_remake.outreach.v1')
    expect(rendered.tags.map(tag => tag.value)).toEqual([
      'targeted_remake',
      'concept_preview_outreach',
    ])
    expect(JSON.parse(rendered.metadataJson)).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      emailSubtype: 'targeted_remake_outreach',
      policy: 'system.targeted_remake_outreach_email.v1',
      previewGenerationId: 'targeted_site_remake_preview_otec_1',
    })
  })

  test('rejects targeted remake outreach with private or bypass-shaped material', async () => {
    await expect(
      Effect.runPromise(
        makeEmailService().renderTargetedRemakeOutreachEmail(
          resendConfig(),
          new TargetedRemakeOutreachEmailInput({
            ...targetedRemakeOutreachInput,
            valueProposition: 'Use provider_payload_raw_1 and captcha bypass.',
          }),
        ),
      ),
    ).rejects.toMatchObject({
      operation: 'EmailService.renderTargetedRemakeOutreachEmail',
    })
  })

  test('renders and ledgers the Autopilot decision-required email', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const decisionInput = new AutopilotDecisionEmailInput({
      appOrigin: 'https://openagents.com',
      displayName: 'Alex <Customer>',
      idempotencyKey:
        'autopilot:decision_required:autopilot_work_order.decision_test_1',
      kind: 'decision_required',
      to: 'alex.customer@example.com',
      workOrderRef: 'autopilot_work_order.decision_test_1',
    })
    const requests: Array<Request> = []
    const fetcher: typeof fetch = async (input, init) => {
      requests.push(input instanceof Request ? input : new Request(input, init))

      return new Response(JSON.stringify({ id: 'email_decision_test' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }
    const runtime = {
      nowIso: () => '2026-06-11T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_fixed`,
    }

    const rendered = await Effect.runPromise(
      makeEmailService().renderAutopilotDecisionNotificationEmail(
        resendConfig(),
        decisionInput,
      ),
    )

    expect(rendered.kind).toBe('crm_transactional')
    expect(rendered.templateSlug).toBe('autopilot_decisions.decision_required.v1')
    expect(rendered.text).toContain('https://openagents.com/decisions')
    expect(rendered.text).toContain(
      'Work order: autopilot_work_order.decision_test_1',
    )

    const result = await Effect.runPromise(
      makeEmailService().sendAutopilotDecisionEmailWithLedger(
        db,
        resendConfig(),
        decisionInput,
        {
          sourceAuthorityRef: 'system.autopilot_decision_notification.v1',
          targetUserId: 'github:autopilot-owner',
        },
        fetcher,
        runtime,
      ),
    )

    expect(result).toMatchObject({
      ok: true,
      providerMessageId: 'email_decision_test',
    })
    expect(requests[0]?.headers.get('Idempotency-Key')).toBe(
      decisionInput.idempotencyKey,
    )
    expect(messagesByIdempotencyKey.get(decisionInput.idempotencyKey)).toMatchObject({
      kind: 'crm_transactional',
      source_authority_ref: 'system.autopilot_decision_notification.v1',
      status: 'accepted',
      target_user_id: 'github:autopilot-owner',
    })
    expect(deliveries).toEqual([
      expect.objectContaining({
        provider_idempotency_key: decisionInput.idempotencyKey,
        provider_message_id: 'email_decision_test',
        status: 'accepted',
      }),
    ])
  })

  test('rejects Autopilot decision email input containing secret-shaped material', async () => {
    await expect(
      Effect.runPromise(
        makeEmailService().renderAutopilotDecisionNotificationEmail(
          resendConfig(),
          new AutopilotDecisionEmailInput({
            appOrigin: 'https://openagents.com',
            displayName: 'Alex Customer',
            idempotencyKey: 'autopilot:decision_required:unsafe',
            kind: 'decision_required',
            to: 'alex.customer@example.com',
            workOrderRef: 'autopilot_work_order.access_token.leak',
          }),
        ),
      ),
    ).rejects.toMatchObject({
      operation: 'EmailService.renderAutopilotDecisionEmail',
    })
  })

  test('records message and delivery rows around Resend sends', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const fetcher: typeof fetch = async () =>
      new Response(JSON.stringify({ id: 'email_ledger_test' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    const runtime = {
      nowIso: () => '2026-06-04T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_fixed`,
    }

    const result = await Effect.runPromise(
      makeEmailService().sendOutOfCreditsEmailWithLedger(
        db,
        resendConfig(),
        emailInput,
        {
          metadata: { balanceCents: 0 },
          sourceAuthorityRef: 'system.billing_out_of_credits.v1',
          targetUserId: 'github:user',
        },
        fetcher,
        runtime,
      ),
    )

    expect(result).toEqual({ id: 'email_ledger_test', ok: true })
    const message = messagesByIdempotencyKey.get(emailInput.idempotencyKey)

    expect(message).toMatchObject({
      idempotency_key: emailInput.idempotencyKey,
      kind: 'billing_out_of_credits',
      provider: 'resend',
      provider_message_id: 'email_ledger_test',
      source_authority_ref: 'system.billing_out_of_credits.v1',
      status: 'accepted',
      target_user_id: 'github:user',
    })
    expect(deliveries).toEqual([
      expect.objectContaining({
        message_id: 'email_msg_fixed',
        provider: 'resend',
        provider_idempotency_key: emailInput.idempotencyKey,
        provider_message_id: 'email_ledger_test',
        status: 'accepted',
      }),
    ])
  })

  test('sends targeted remake outreach through EmailService ledger and Resend idempotency', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const requests: Array<Request> = []
    const fetcher: typeof fetch = async (input, init) => {
      requests.push(input instanceof Request ? input : new Request(input, init))

      return new Response(JSON.stringify({ id: 'email_targeted_remake_test' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }
    const runtime = {
      nowIso: () => '2026-06-05T19:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_targeted_fixed`,
    }

    const result = await Effect.runPromise(
      makeEmailService().sendTargetedRemakeOutreachEmailWithLedger(
        db,
        resendConfig(),
        targetedRemakeOutreachInput,
        {
          actorUserId: 'operator_chris',
          metadata: {
            operatorReviewEventId: 'targeted_site_operator_review_otec_1',
          },
          sourceAuthorityRef: 'source_pack_otec_1',
        },
        fetcher,
        runtime,
      ),
    )

    expect(result).toEqual({
      emailMessageId: 'email_msg_targeted_fixed',
      ok: true,
      providerMessageId: 'email_targeted_remake_test',
    })
    expect(requests).toHaveLength(1)
    expect(requests[0]!.headers.get('Idempotency-Key')).toBe(
      targetedRemakeOutreachInput.idempotencyKey,
    )
    const body = await requests[0]!.json()
    expect(body).toMatchObject({
      from: 'OpenAgents <billing@openagents.com>',
      reply_to: 'support@openagents.com',
      subject: 'A concept Site for OTEC Floating Datacenter',
      to: ['alex.customer@example.com'],
    })
    expect(messagesByIdempotencyKey.get(targetedRemakeOutreachInput.idempotencyKey))
      .toMatchObject({
        kind: 'operator_notification',
        provider: 'resend',
        provider_message_id: 'email_targeted_remake_test',
        source_authority_ref: 'source_pack_otec_1',
        status: 'accepted',
      })
    expect(deliveries).toEqual([
      expect.objectContaining({
        message_id: 'email_msg_targeted_fixed',
        provider: 'resend',
        provider_idempotency_key: targetedRemakeOutreachInput.idempotencyKey,
        provider_message_id: 'email_targeted_remake_test',
        status: 'accepted',
      }),
    ])
  })

  test('records Cloudflare Email binding sends through the existing email ledger', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const sentMessages: Array<Parameters<CloudflareEmailBinding['send']>[0]> =
      []
    const binding: CloudflareEmailBinding = {
      send: message => {
        sentMessages.push(message)

        return Promise.resolve({ messageId: 'cf_email_ledger_test' })
      },
    }
    const runtime = {
      nowIso: () => '2026-06-16T15:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_cloudflare_fixed`,
    }
    const rendered = await Effect.runPromise(
      makeEmailService().renderOutOfCreditsEmail(resendConfig(), emailInput),
    )

    const result = await Effect.runPromise(
      sendRenderedEmailViaCloudflareBindingWithLedger(
        db,
        binding,
        rendered,
        {
          metadata: { smoke: 'cloudflare_email_provider_adapter' },
          sourceAuthorityRef: 'system.cloudflare_email_provider_adapter.v1',
        },
        runtime,
      ),
    )

    expect(result).toEqual({
      emailMessageId: 'email_msg_cloudflare_fixed',
      ok: true,
      providerMessageId: 'cf_email_ledger_test',
    })
    expect(sentMessages).toHaveLength(1)
    expect(
      messagesByIdempotencyKey.get(emailInput.idempotencyKey),
    ).toMatchObject({
      idempotency_key: emailInput.idempotencyKey,
      kind: 'billing_out_of_credits',
      provider: 'cloudflare_email',
      provider_message_id: 'cf_email_ledger_test',
      source_authority_ref: 'system.cloudflare_email_provider_adapter.v1',
      status: 'accepted',
    })
    expect(deliveries).toEqual([
      expect.objectContaining({
        message_id: 'email_msg_cloudflare_fixed',
        provider: 'cloudflare_email',
        provider_idempotency_key: emailInput.idempotencyKey,
        provider_message_id: 'cf_email_ledger_test',
        status: 'accepted',
      }),
    ])
  })

  test('renders Adjutant customer notifications without leaking html input', () => {
    expect(adjutantCustomerNotificationEmailText(adjutantInput)).toContain(
      'https://openagents.com/order',
    )
    expect(adjutantCustomerNotificationEmailText(adjutantInput)).toContain(
      'https://sites.openagents.com/otec',
    )
    expect(adjutantCustomerNotificationEmailHtml(adjutantInput)).toContain(
      'Alex &lt;Customer&gt;',
    )
    expect(adjutantCustomerNotificationEmailHtml(adjutantInput)).toContain(
      'OTEC &lt;Launch&gt; is ready for review',
    )
    expect(adjutantCustomerNotificationEmailHtml(adjutantInput)).not.toContain(
      'Alex <Customer>',
    )
  })

  test('renders every order/Sites lifecycle subtype with customer-safe status and next action', async () => {
    const service = makeEmailService()

    for (const lifecycleKind of ORDER_SITES_TRANSACTIONAL_EMAIL_KINDS) {
      const input = lifecycleEmailInput(lifecycleKind)
      const rendered = await Effect.runPromise(
        service.renderOrderSitesTransactionalEmail(resendConfig(), input),
      )

      expect(rendered.kind).toBe('operator_notification')
      expect(rendered.templateSlug).toBe(`order_sites.${lifecycleKind}.v1`)
      expect(rendered.subject.length).toBeGreaterThan(0)
      expect(rendered.text).toContain(input.customerSafeStatus)
      expect(rendered.text).toContain(input.nextAction)
      expect(rendered.html).toContain(escapeFixture(input.customerSafeStatus))
      expect(rendered.html).toContain(escapeFixture(input.nextAction))
      expect(rendered.html).toContain('Alex &lt;Customer&gt;')
      expect(rendered.html).toContain('OTEC &lt;Launch&gt;')
      expect(rendered.html).not.toContain('Alex <Customer>')

      const metadata = JSON.parse(rendered.metadataJson)
      expect(metadata).toMatchObject({
        emailSubtype: 'order_sites_lifecycle',
        lifecycleKind,
        orderId: input.orderId,
        policy: 'system.order_sites_lifecycle_email.v1',
        siteId: input.siteId,
      })
      const context = JSON.parse(rendered.templateContextJson)
      expect(context).toMatchObject({
        customerSafeStatus: input.customerSafeStatus,
        lifecycleKind,
        nextAction: input.nextAction,
      })
      expect(rendered.tags.map(tag => tag.value)).toEqual([
        'order_sites',
        lifecycleKind,
      ])
      const serialized = JSON.stringify(rendered).toLowerCase()
      expect(serialized).not.toContain('provider_account')
      expect(serialized).not.toContain('auth_grant')
      expect(serialized).not.toContain('runner raw log')
      expect(serialized).not.toContain('private operator note')
      expect(serialized).not.toContain('raw exa')
    }
  })

  test('renders order/Sites lifecycle text and html helpers with links', () => {
    const input = lifecycleEmailInput('site_deployed')

    expect(orderSitesTransactionalEmailText(input)).toContain(
      'Status: customer-safe status for site_deployed',
    )
    expect(orderSitesTransactionalEmailText(input)).toContain(
      'Next action: customer-safe next action for site_deployed',
    )
    expect(orderSitesTransactionalEmailText(input)).toContain(
      'https://sites.openagents.com/otec',
    )
    expect(orderSitesTransactionalEmailText(input)).toContain(
      'Latest revision: https://sites.openagents.com/otec/versions/site_version_otec_20260605_revision_3',
    )
    expect(orderSitesTransactionalEmailHtml(input)).toContain('View status')
    expect(orderSitesTransactionalEmailHtml(input)).toContain('Live Site:')
    expect(orderSitesTransactionalEmailHtml(input)).toContain(
      'Latest revision:',
    )
    expect(orderSitesTransactionalEmailHtml(input)).not.toContain(
      'OpenAgents email ledger',
    )
    expect(orderSitesTransactionalEmailHtml(input)).toContain(
      'https://openagents.com/order?orderId=software_order_otec',
    )
  })

  test('renders non-Sites review artifact lifecycle email links', async () => {
    const baseInput = lifecycleEmailInput('review_ready')
    const input = new OrderSitesTransactionalEmailInput({
      appOrigin: baseInput.appOrigin,
      artifactLabel: 'Review pull request',
      artifactUrl: 'https://github.com/customer/app/pull/7',
      customerSafeStatus: baseInput.customerSafeStatus,
      displayName: baseInput.displayName,
      eventRef: 'fulfillment_artifact_pr',
      lifecycleKind: baseInput.lifecycleKind,
      nextAction: baseInput.nextAction,
      orderId: baseInput.orderId,
      revisionUrl: null,
      safeReason: baseInput.safeReason,
      siteTitle: 'Customer app PR',
      siteUrl: null,
      to: baseInput.to,
      ...(baseInput.assignmentId === undefined
        ? {}
        : { assignmentId: baseInput.assignmentId }),
      ...(baseInput.sourceAuthorityRefs === undefined
        ? {}
        : { sourceAuthorityRefs: baseInput.sourceAuthorityRefs }),
      ...(baseInput.targetRefs === undefined
        ? {}
        : { targetRefs: baseInput.targetRefs }),
    })
    const rendered = await Effect.runPromise(
      makeEmailService().renderOrderSitesTransactionalEmail(
        resendConfig(),
        input,
      ),
    )

    expect(rendered.text).toContain(
      'Review pull request: https://github.com/customer/app/pull/7',
    )
    expect(rendered.html).toContain('Review pull request:')
    expect(rendered.html).toContain('https://github.com/customer/app/pull/7')
    expect(rendered.text).toContain(
      'Order status: https://openagents.com/order?orderId=software_order_otec',
    )
    expect(rendered.text).not.toContain('Latest revision:')
    expect(JSON.parse(rendered.metadataJson)).toMatchObject({
      artifactLabel: 'Review pull request',
      artifactUrl: 'https://github.com/customer/app/pull/7',
      lifecycleKind: 'review_ready',
      orderId: 'software_order_otec',
    })
    expect(buildOrderSitesTransactionalEmailIdempotencyKey(input)).toBe(
      'order_sites_email:review_ready:software_order_otec:adjutant_assignment_otec:none:fulfillment_artifact_pr',
    )
  })

  test('builds deterministic order/Sites lifecycle idempotency keys', () => {
    const input = lifecycleEmailInput('review_ready')

    expect(buildOrderSitesTransactionalEmailIdempotencyKey(input)).toBe(
      'order_sites_email:review_ready:software_order_otec:adjutant_assignment_otec:site_otec:event_review_ready',
    )
    expect(
      buildOrderSitesTransactionalEmailIdempotencyKey(
        new OrderSitesTransactionalEmailInput({
          ...input,
          assignmentId: 'assignment with spaces',
          eventRef: 'event/with/slashes',
        }),
      ),
    ).toBe(
      'order_sites_email:review_ready:software_order_otec:assignment%20with%20spaces:site_otec:event%2Fwith%2Fslashes',
    )
  })

  test('rejects secret-shaped order/Sites lifecycle email inputs', async () => {
    await expect(
      Effect.runPromise(
        makeEmailService().renderOrderSitesTransactionalEmail(
          resendConfig(),
          new OrderSitesTransactionalEmailInput({
            ...lifecycleEmailInput('customer_input_needed'),
            nextAction:
              'Paste OPENCODE_AUTH_CONTENT or sk-proj-secret into the reply.',
          }),
        ),
      ),
    ).rejects.toMatchObject({
      message:
        'Order/Sites lifecycle email input contains secret-shaped material.',
      operation: 'EmailService.renderOrderSitesTransactionalEmail',
    })
  })

  test('rejects secret-shaped site referral onboarding input', async () => {
    await expect(
      Effect.runPromise(
        makeEmailService().renderSiteReferralOnboardingEmail(
          resendConfig(),
          new SiteReferralOnboardingEmailInput({
            ...referralOnboardingInput,
            sourceLabel: 'provider_account auth_grant token_hash',
          }),
        ),
      ),
    ).rejects.toMatchObject({
      message:
        'Site referral onboarding email input contains secret-shaped material.',
      operation: 'EmailService.renderSiteReferralOnboardingEmail',
    })
  })

  test('keeps site referral onboarding sends idempotent through the ledger', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    let fetchCount = 0
    const fetcher: typeof fetch = async () => {
      fetchCount += 1

      return new Response(
        JSON.stringify({ id: 'email_site_referral_onboarding_test' }),
        {
          headers: { 'content-type': 'application/json' },
          status: 200,
        },
      )
    }
    const runtime = {
      nowIso: () => '2026-06-05T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_site_referral`,
    }
    const service = makeEmailService()

    const first = await Effect.runPromise(
      service.sendSiteReferralOnboardingEmailWithLedger(
        db,
        resendConfig(),
        referralOnboardingInput,
        {
          metadata: {
            referralAttributionId: 'referral_attribution_otec',
            referralSourceId: 'site_referral_source_otec',
          },
          sourceAuthorityRef:
            'system.site_referral_onboarding.v1:referral_attribution_otec',
          targetUserId: 'github:customer',
        },
        fetcher,
        runtime,
      ),
    )
    const second = await Effect.runPromise(
      service.sendSiteReferralOnboardingEmailWithLedger(
        db,
        resendConfig(),
        referralOnboardingInput,
        {
          metadata: {
            referralAttributionId: 'referral_attribution_otec',
            referralSourceId: 'site_referral_source_otec',
          },
          sourceAuthorityRef:
            'system.site_referral_onboarding.v1:referral_attribution_otec',
          targetUserId: 'github:customer',
        },
        fetcher,
        runtime,
      ),
    )

    expect(first).toEqual({
      emailMessageId: 'email_msg_site_referral',
      ok: true,
      providerMessageId: 'email_site_referral_onboarding_test',
    })
    expect(second).toEqual(first)
    expect(fetchCount).toBe(1)
    expect(
      messagesByIdempotencyKey.get(referralOnboardingInput.idempotencyKey),
    ).toEqual(
      expect.objectContaining({
        kind: 'crm_transactional',
        provider: 'resend',
        provider_message_id: 'email_site_referral_onboarding_test',
        status: 'accepted',
        target_user_id: 'github:customer',
      }),
    )
    expect(deliveries).toHaveLength(1)
  })

  test('keeps Adjutant customer notification sends idempotent through the ledger', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    let fetchCount = 0
    const fetcher: typeof fetch = async () => {
      fetchCount += 1

      return new Response(
        JSON.stringify({ id: 'email_adjutant_ledger_test' }),
        {
          headers: { 'content-type': 'application/json' },
          status: 200,
        },
      )
    }
    const runtime = {
      nowIso: () => '2026-06-04T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_fixed`,
    }
    const service = makeEmailService()

    const first = await Effect.runPromise(
      service.sendAdjutantCustomerNotificationWithLedger(
        db,
        resendConfig(),
        adjutantInput,
        {
          sourceAuthorityRef:
            'docs/2026-06-05-adjutant-sites-supervisor-audit.md#16',
          targetUserId: 'github:customer',
        },
        fetcher,
        runtime,
      ),
    )
    const second = await Effect.runPromise(
      service.sendAdjutantCustomerNotificationWithLedger(
        db,
        resendConfig(),
        adjutantInput,
        {
          sourceAuthorityRef:
            'docs/2026-06-05-adjutant-sites-supervisor-audit.md#16',
          targetUserId: 'github:customer',
        },
        fetcher,
        runtime,
      ),
    )

    expect(first).toEqual({
      emailMessageId: 'email_msg_fixed',
      ok: true,
      providerMessageId: 'email_adjutant_ledger_test',
    })
    expect(second).toEqual(first)
    expect(fetchCount).toBe(1)
    expect(messagesByIdempotencyKey.get(adjutantInput.idempotencyKey)).toEqual(
      expect.objectContaining({
        kind: 'operator_notification',
        provider: 'resend',
        provider_message_id: 'email_adjutant_ledger_test',
        status: 'accepted',
        target_user_id: 'github:customer',
      }),
    )
    expect(deliveries).toHaveLength(1)
  })

  test('renders private workspace invite email with escaped labels', async () => {
    const service = makeEmailService()
    const rendered = await Effect.runPromise(
      service.renderPrivateWorkspaceInviteEmail(
        resendConfig(),
        privateWorkspaceInviteInput,
      ),
    )

    expect(rendered.kind).toBe('operator_notification')
    expect(rendered.templateSlug).toBe('team_workspace_invite.v1')
    expect(rendered.text).toContain(privateWorkspaceInviteInput.acceptUrl)
    expect(rendered.html).toContain('Alex &lt;Customer&gt;')
    expect(rendered.html).toContain('Private &lt;Workspace&gt;')
    expect(rendered.html).not.toContain('Alex <Customer>')
    expect(rendered.html).not.toContain('Private <Workspace>')
  })

  test('sends private workspace invite email idempotently through the ledger', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const runtime = {
      nowIso: () => '2026-06-16T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_workspace_invite`,
    }
    let fetchCount = 0
    const fetcher: typeof fetch = async () => {
      fetchCount += 1

      return new Response(JSON.stringify({ id: 'email_workspace_invite' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }
    const service = makeEmailService()

    const first = await Effect.runPromise(
      service.sendPrivateWorkspaceInviteEmailWithLedger(
        db,
        resendConfig(),
        privateWorkspaceInviteInput,
        {
          sourceAuthorityRef: 'system.private_workspace_invite_email.test',
        },
        fetcher,
        runtime,
      ),
    )
    const second = await Effect.runPromise(
      service.sendPrivateWorkspaceInviteEmailWithLedger(
        db,
        resendConfig(),
        privateWorkspaceInviteInput,
        {
          sourceAuthorityRef: 'system.private_workspace_invite_email.test',
        },
        fetcher,
        runtime,
      ),
    )

    expect(first).toEqual({
      emailMessageId: 'email_msg_workspace_invite',
      ok: true,
      providerMessageId: 'email_workspace_invite',
    })
    expect(second).toEqual(first)
    expect(fetchCount).toBe(1)
    expect(
      messagesByIdempotencyKey.get(privateWorkspaceInviteInput.idempotencyKey),
    ).toMatchObject({
      kind: 'operator_notification',
      provider: 'resend',
      provider_message_id: 'email_workspace_invite',
      source_authority_ref: 'system.private_workspace_invite_email.test',
      status: 'accepted',
    })
    expect(deliveries).toHaveLength(1)
  })

  test('records private workspace invite provider rejection without raw provider payloads', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const runtime = {
      nowIso: () => '2026-06-16T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_workspace_invite_failed`,
    }
    const service = makeEmailService()

    const result = await Effect.runPromise(
      service.sendPrivateWorkspaceInviteEmailWithLedger(
        db,
        resendConfig(),
        new PrivateWorkspaceInviteEmailInput({
          ...privateWorkspaceInviteInput,
          idempotencyKey: 'team_workspace_invite:invite_1:failed',
        }),
        undefined,
        async () =>
          new Response(
            JSON.stringify({
              message:
                'Domain is not verified. Bearer secret-token-value-123456 should not appear.',
              name: 'validation_error',
            }),
            {
              headers: { 'content-type': 'application/json' },
              status: 422,
            },
          ),
        runtime,
      ),
    )

    expect(result).toMatchObject({
      errorMessage:
        'Domain is not verified. Bearer [REDACTED] should not appear.',
      errorName: 'validation_error',
      ok: false,
    })
    expect(messagesByIdempotencyKey.get(result.emailMessageId)).toBeUndefined()
    expect(
      messagesByIdempotencyKey.get('team_workspace_invite:invite_1:failed'),
    ).toMatchObject({
      error_message:
        'Domain is not verified. Bearer [REDACTED] should not appear.',
      error_name: 'validation_error',
      status: 'failed',
    })
    expect(deliveries).toEqual([
      expect.objectContaining({
        error_message:
          'Domain is not verified. Bearer [REDACTED] should not appear.',
        error_name: 'validation_error',
        status: 'failed',
      }),
    ])
    expect(JSON.stringify(result)).not.toContain('secret-token-value')
    expect(JSON.stringify(deliveries)).not.toContain('secret-token-value')
  })

  test('operator smoke records an email_config_missing skip as a ledger failure', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const runtime = {
      nowIso: () => '2026-06-05T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_smoke_missing`,
    }

    const result = await Effect.runPromise(
      runOperatorEmailLedgerSmoke(
        db,
        undefined,
        {
          appOrigin: 'https://openagents.com',
          idempotencyKey: 'operator-email-smoke:missing-config',
          mode: 'send',
          to: 'ops@example.com',
        },
        async () => {
          throw new Error('fetch should not be called without config')
        },
        runtime,
      ),
    )

    expect(result).toEqual({
      configStatus: 'missing',
      emailMessageId: 'email_msg_smoke_missing',
      errorMessage: 'Resend email configuration is not set.',
      errorName: 'email_config_missing',
      idempotencyKey: 'operator-email-smoke:missing-config',
      mode: 'send',
      provider: 'resend',
      providerMessageId: null,
      status: 'skipped',
      templateSlug: 'adjutant.customer_notification.v1',
    })
    expect(messagesByIdempotencyKey.get(result.idempotencyKey)).toMatchObject({
      error_name: 'email_config_missing',
      provider: 'resend',
      status: 'failed',
    })
    expect(deliveries).toHaveLength(0)
  })

  test('operator smoke sends through Resend and records one delivery', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const runtime = {
      nowIso: () => '2026-06-05T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_smoke_success`,
    }
    let fetchCount = 0
    const fetcher: typeof fetch = async () => {
      fetchCount += 1

      return new Response(JSON.stringify({ id: 'email_operator_smoke' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }

    const result = await Effect.runPromise(
      runOperatorEmailLedgerSmoke(
        db,
        resendConfig(),
        {
          appOrigin: 'https://openagents.com',
          idempotencyKey: 'operator-email-smoke:send-success',
          mode: 'send',
          to: 'ops@example.com',
        },
        fetcher,
        runtime,
      ),
    )

    expect(result).toMatchObject({
      configStatus: 'present',
      errorMessage: null,
      errorName: null,
      providerMessageId: 'email_operator_smoke',
      status: 'accepted',
    })
    expect(fetchCount).toBe(1)
    expect(messagesByIdempotencyKey.get(result.idempotencyKey)).toMatchObject({
      provider_message_id: 'email_operator_smoke',
      status: 'accepted',
    })
    expect(deliveries).toEqual([
      expect.objectContaining({
        provider_message_id: 'email_operator_smoke',
        status: 'accepted',
      }),
    ])
  })

  test('operator smoke records redacted provider failure state', async () => {
    const { db, deliveries, messagesByIdempotencyKey } = makeEmailLedgerD1()
    const runtime = {
      nowIso: () => '2026-06-05T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_smoke_failed`,
    }

    const result = await Effect.runPromise(
      runOperatorEmailLedgerSmoke(
        db,
        resendConfig(),
        {
          appOrigin: 'https://openagents.com',
          idempotencyKey: 'operator-email-smoke:send-failed',
          mode: 'send',
          to: 'ops@example.com',
        },
        async () =>
          new Response(
            JSON.stringify({
              message:
                'Domain is not verified. Bearer secret-token-value-123456 should not appear.',
              name: 'validation_error',
            }),
            {
              headers: { 'content-type': 'application/json' },
              status: 422,
            },
          ),
        runtime,
      ),
    )

    expect(result).toMatchObject({
      configStatus: 'present',
      errorMessage:
        'Domain is not verified. Bearer [REDACTED] should not appear.',
      errorName: 'validation_error',
      providerMessageId: null,
      status: 'failed',
    })
    expect(messagesByIdempotencyKey.get(result.idempotencyKey)).toMatchObject({
      error_message:
        'Domain is not verified. Bearer [REDACTED] should not appear.',
      error_name: 'validation_error',
      status: 'failed',
    })
    expect(deliveries).toEqual([
      expect.objectContaining({
        error_message:
          'Domain is not verified. Bearer [REDACTED] should not appear.',
        error_name: 'validation_error',
        status: 'failed',
      }),
    ])
    expect(JSON.stringify(result)).not.toContain('secret-token-value')
    expect(JSON.stringify(messagesByIdempotencyKey)).not.toContain(
      'secret-token-value',
    )
  })

  test('operator smoke idempotency does not double-send', async () => {
    const { db, deliveries } = makeEmailLedgerD1()
    const runtime = {
      nowIso: () => '2026-06-05T12:00:00.000Z',
      randomId: (prefix: string) => `${prefix}_smoke_idempotent`,
    }
    let fetchCount = 0
    const fetcher: typeof fetch = async () => {
      fetchCount += 1

      return new Response(JSON.stringify({ id: 'email_operator_once' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }
    const input = {
      appOrigin: 'https://openagents.com',
      idempotencyKey: 'operator-email-smoke:idempotent',
      mode: 'send' as const,
      to: 'ops@example.com',
    }

    const first = await Effect.runPromise(
      runOperatorEmailLedgerSmoke(db, resendConfig(), input, fetcher, runtime),
    )
    const second = await Effect.runPromise(
      runOperatorEmailLedgerSmoke(db, resendConfig(), input, fetcher, runtime),
    )

    expect(first).toEqual(second)
    expect(fetchCount).toBe(1)
    expect(deliveries).toHaveLength(1)
  })
})
