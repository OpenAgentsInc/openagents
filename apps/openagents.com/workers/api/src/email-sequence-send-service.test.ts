import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import type { CloudflareEmailBinding, EmailLedgerSendResult } from './email'
import {
  type EmailSequenceSendPlan,
  type EmailSequenceSendRow,
  isEmailSequenceSendEnabled,
  makeCloudflareEmailSequenceSender,
  makeEmailSequenceSendService,
  planEmailSequenceSend,
  renderEmailSequenceSend,
} from './email-sequence-send-service'

const baseRow: EmailSequenceSendRow = {
  campaignId: 'campaign-1',
  email: 'lead@example.com',
  enrollmentId: 'enrollment-1',
  idempotencyKey: 'idem-1',
  sendId: 'send-1',
  sourceAuthorityRef: 'operator.email_sequence_authoring.v1:welcome:send:step-1',
  stepId: 'step-1',
  stepKey: 'welcome',
  templateSlug: 'sequence.welcome.v1',
  displayName: 'Ada',
  userId: 'user-1',
}

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

  first<T>(): Promise<T | null> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    if (this.query.includes('FROM email_messages')) {
      return Promise.resolve({
        created_at: '2026-06-28T12:00:00.000Z',
        error_message: null,
        error_name: null,
        id: 'email_msg_sequence_fixed',
        idempotency_key: this.bound[0],
        kind: 'crm_transactional',
        provider: null,
        provider_message_id: null,
        status: 'rendered',
        updated_at: '2026-06-28T12:00:00.000Z',
      } as T)
    }

    return Promise.resolve(null)
  }

  run(): Promise<D1Result> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve({ meta: { changes: 1 }, success: true } as D1Result)
  }
}

class RecordingD1Database {
  readonly lookups: Array<Readonly<{ query: string; values: Array<unknown> }>> =
    []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

const runtime = {
  nowIso: () => '2026-06-28T12:00:00.000Z',
  randomId: (prefix: string) => `${prefix}_sequence_fixed`,
}

describe('isEmailSequenceSendEnabled', () => {
  test('defaults to disabled (INERT) when unset or non-truthy', () => {
    expect(isEmailSequenceSendEnabled(undefined)).toBe(false)
    expect(isEmailSequenceSendEnabled('')).toBe(false)
    expect(isEmailSequenceSendEnabled('0')).toBe(false)
    expect(isEmailSequenceSendEnabled('false')).toBe(false)
    expect(isEmailSequenceSendEnabled('off')).toBe(false)
  })

  test('is enabled only for explicit truthy values', () => {
    expect(isEmailSequenceSendEnabled('1')).toBe(true)
    expect(isEmailSequenceSendEnabled('true')).toBe(true)
    expect(isEmailSequenceSendEnabled('yes')).toBe(true)
    expect(isEmailSequenceSendEnabled(' ON ')).toBe(true)
  })
})

describe('planEmailSequenceSend', () => {
  test('shapes the typed send-service plan from a send row', () => {
    const plan = planEmailSequenceSend(baseRow)

    expect(plan).toEqual({
      campaignId: 'campaign-1',
      displayName: 'Ada',
      enrollmentId: 'enrollment-1',
      idempotencyKey: 'idem-1',
      sendId: 'send-1',
      sourceAuthorityRef:
        'operator.email_sequence_authoring.v1:welcome:send:step-1',
      stepId: 'step-1',
      stepKey: 'welcome',
      templateSlug: 'sequence.welcome.v1',
      to: 'lead@example.com',
      userId: 'user-1',
    })
  })

  test('falls back to a neutral display name and null user', () => {
    const plan = planEmailSequenceSend({
      ...baseRow,
      displayName: '   ',
      userId: null,
    })

    expect(plan.displayName).toBe('there')
    expect(plan.userId).toBeNull()
  })
})

describe('makeEmailSequenceSendService (INERT default)', () => {
  test('never calls the sender when disabled, returns a dry-run', async () => {
    let calls = 0
    const service = makeEmailSequenceSendService({
      isEnabled: () => false,
      send: () => {
        calls += 1

        return Effect.succeed({
          emailMessageId: 'm',
          ok: true,
          providerMessageId: 'p',
        })
      },
    })

    const outcome = await Effect.runPromise(service.dispatchSequenceSend(baseRow))

    expect(calls).toBe(0)
    expect(outcome.kind).toBe('dry_run')
    if (outcome.kind === 'dry_run') {
      expect(outcome.reason).toBe('send_disabled')
      expect(outcome.plan.to).toBe('lead@example.com')
    }
  })

  test('delegates to the injected sender only when armed (sent)', async () => {
    const seen: EmailSequenceSendPlan[] = []
    const service = makeEmailSequenceSendService({
      isEnabled: () => true,
      send: plan => {
        seen.push(plan)

        return Effect.succeed({
          emailMessageId: 'message-1',
          ok: true,
          providerMessageId: 'provider-1',
        } satisfies EmailLedgerSendResult)
      },
    })

    const outcome = await Effect.runPromise(service.dispatchSequenceSend(baseRow))

    expect(seen).toHaveLength(1)
    expect(seen[0]?.idempotencyKey).toBe('idem-1')
    expect(outcome.kind).toBe('sent')
    if (outcome.kind === 'sent') {
      expect(outcome.result.providerMessageId).toBe('provider-1')
    }
  })

  test('reports a failed outcome when an armed send fails', async () => {
    const service = makeEmailSequenceSendService({
      isEnabled: () => true,
      send: () =>
        Effect.succeed({
          emailMessageId: 'message-1',
          errorMessage: 'vendor rejected',
          errorName: 'send_failed',
          ok: false,
        } satisfies EmailLedgerSendResult),
    })

    const outcome = await Effect.runPromise(service.dispatchSequenceSend(baseRow))

    expect(outcome.kind).toBe('failed')
    if (outcome.kind === 'failed') {
      expect(outcome.result.errorMessage).toBe('vendor rejected')
    }
  })
})

describe('Cloudflare email sequence sender', () => {
  test('renders a receipt-first transactional sequence email', () => {
    const rendered = renderEmailSequenceSend(
      {
        appOrigin: 'https://openagents.com',
        fromEmail: 'OpenAgents Sites <sites@openagents.com>',
        replyToEmail: 'support@openagents.com',
      },
      planEmailSequenceSend(baseRow),
    )

    expect(rendered.kind).toBe('crm_transactional')
    expect(rendered.from).toBe('OpenAgents Sites <sites@openagents.com>')
    expect(rendered.replyTo).toBe('support@openagents.com')
    expect(rendered.templateSlug).toBe('sequence.welcome.v1')
    expect(rendered.text).toContain(
      'https://openagents.com/email/preferences',
    )
    expect(rendered.metadataJson).toContain(
      'autopilot_sites.native_email_sequences.v1',
    )
  })

  test('sends through the Cloudflare binding and records ledger receipts', async () => {
    const db = new RecordingD1Database()
    const sentMessages: Array<Parameters<CloudflareEmailBinding['send']>[0]> =
      []
    const binding: CloudflareEmailBinding = {
      send: message => {
        sentMessages.push(message)

        return Promise.resolve({ messageId: 'cf_sequence_1' })
      },
    }
    const sender = makeCloudflareEmailSequenceSender(
      db as unknown as D1Database,
      binding,
      {
        appOrigin: 'https://openagents.com',
        fromEmail: 'OpenAgents Sites <sites@openagents.com>',
      },
      runtime,
    )

    const result = await Effect.runPromise(sender(planEmailSequenceSend(baseRow)))

    expect(result).toEqual({
      emailMessageId: 'email_msg_sequence_fixed',
      ok: true,
      providerMessageId: 'cf_sequence_1',
    })
    expect(sentMessages).toHaveLength(1)
    expect(sentMessages[0]?.to).toBe('lead@example.com')
    expect(sentMessages[0]?.from).toBe(
      'OpenAgents Sites <sites@openagents.com>',
    )
    expect(
      db.runs.some(run => run.query.includes('INSERT INTO email_messages')),
    ).toBe(true)
    expect(
      db.runs.some(run => run.query.includes('INSERT INTO email_deliveries')),
    ).toBe(true)
  })
})
