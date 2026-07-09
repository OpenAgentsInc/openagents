import { describe, expect, test, vi } from 'vitest'

import {
  type CrmResendDeps,
  type CrmResendSender,
  isCrmResendSendEnabled,
  resolveCrmResendIdentity,
  sendCrmEmailViaResend,
} from './crm-resend'

const contactRow = {
  created_at: '2026-06-22T00:00:00.000Z',
  first_name: 'Ada',
  full_name: 'Ada Lovelace',
  id: 'crm_contact_1',
  primary_email: 'ada@example.com',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}
const templateRow = {
  body_markdown_template: 'Hi {{ contact.first_name }}',
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_template_1',
  name: 'Welcome',
  slug: 'welcome',
  status: 'active',
  subject_template: 'Hello {{ contact.first_name }}',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}
const queuedMessageRow = {
  body_markdown: 'Hi Ada',
  channel: 'resend',
  contact_id: 'crm_contact_1',
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_email_q',
  status: 'queued',
  subject: 'Hello Ada',
  tenant_ref: 'tenant.openagents',
  to_email: 'ada@example.com',
  updated_at: '2026-06-22T00:00:00.000Z',
}

const cannedDb = (opts: { suppressed?: boolean } = {}): D1Database => {
  const firstFor = (query: string): Record<string, unknown> | null => {
    if (query.includes('FROM crm_contacts')) return contactRow
    if (query.includes('FROM crm_email_templates')) return templateRow
    if (query.includes('FROM email_suppression_entries')) {
      return opts.suppressed === true ? { id: 'sup', scope: 'all' } : null
    }
    if (query.includes('FROM email_preferences')) return null
    if (query.includes('FROM crm_email_messages')) return queuedMessageRow
    return null
  }
  const statement = (query: string): D1PreparedStatement =>
    ({
      bind: () => statement(query),
      first: <T,>() => Promise.resolve(firstFor(query) as T | null),
      all: <T,>() =>
        Promise.resolve({ meta: {} as D1Meta, results: [] as unknown as Array<T>, success: true } as D1Result<T>),
      run: () =>
        Promise.resolve({ meta: {} as D1Meta, results: [], success: true } as unknown as D1Result),
      raw: () => Promise.reject(new Error('raw')),
    }) as unknown as D1PreparedStatement
  return {
    batch: () => Promise.reject(new Error('batch')),
    dump: () => Promise.reject(new Error('dump')),
    exec: () => Promise.reject(new Error('exec')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session')
    },
  } as unknown as D1Database
}

const runtime = { makeId: (p: string) => `${p}_test`, nowIso: () => '2026-06-22T00:00:00.000Z' }

const okSender = (): { sender: CrmResendSender; calls: Array<unknown> } => {
  const calls: Array<unknown> = []
  return {
    calls,
    sender: async input => {
      calls.push(input)
      return { ok: true, providerMessageId: 'resend_123' }
    },
  }
}

const armed = (sender: CrmResendSender): CrmResendDeps => ({
  enabled: true,
  fromEmail: 'OpenAgents <chris+sites@openagents.com>',
  sender,
})

describe('isCrmResendSendEnabled', () => {
  test('off by default / for falsey values', () => {
    expect(isCrmResendSendEnabled(undefined)).toBe(false)
    expect(isCrmResendSendEnabled('false')).toBe(false)
    expect(isCrmResendSendEnabled('0')).toBe(false)
  })
  test('on for truthy strings', () => {
    expect(isCrmResendSendEnabled('1')).toBe(true)
    expect(isCrmResendSendEnabled('true')).toBe(true)
    expect(isCrmResendSendEnabled('ON')).toBe(true)
  })
})

describe('resolveCrmResendIdentity (OB-1 #8558)', () => {
  const shared = {
    fromEmail: 'OpenAgents <chris+sites@openagents.com>',
    replyToEmail: 'chris+sites@openagents.com',
  }

  test('falls back to the shared Resend identity when no CRM override', () => {
    expect(resolveCrmResendIdentity(shared, {})).toEqual({
      fromEmail: 'OpenAgents <chris+sites@openagents.com>',
      replyToEmail: 'chris+sites@openagents.com',
    })
  })

  test('uses the CRM-specific identity when the override is present', () => {
    expect(
      resolveCrmResendIdentity(shared, {
        fromEmail: 'Sarah <sarah@openagents.com>',
        replyToEmail: 'sarah@openagents.com',
      }),
    ).toEqual({
      fromEmail: 'Sarah <sarah@openagents.com>',
      replyToEmail: 'sarah@openagents.com',
    })
  })

  test('overrides from-email while inheriting the shared reply-to', () => {
    expect(
      resolveCrmResendIdentity(shared, {
        fromEmail: 'Sarah <sarah@openagents.com>',
      }),
    ).toEqual({
      fromEmail: 'Sarah <sarah@openagents.com>',
      replyToEmail: 'chris+sites@openagents.com',
    })
  })

  test('never mutates the shared Sites identity (no side effects)', () => {
    resolveCrmResendIdentity(shared, {
      fromEmail: 'Sarah <sarah@openagents.com>',
      replyToEmail: 'sarah@openagents.com',
    })
    expect(shared.fromEmail).toBe('OpenAgents <chris+sites@openagents.com>')
    expect(shared.replyToEmail).toBe('chris+sites@openagents.com')
  })
})

describe('sendCrmEmailViaResend — INERT by default', () => {
  test('disabled deps => dry_run, sender never called', async () => {
    const { calls, sender } = okSender()
    const result = await sendCrmEmailViaResend(
      cannedDb(),
      { enabled: false, fromEmail: 'x@x.com', sender },
      { contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(result.kind).toBe('dry_run')
    expect(calls).toHaveLength(0)
  })

  test('enabled but unconfigured => not_configured', async () => {
    const result = await sendCrmEmailViaResend(
      cannedDb(),
      { enabled: true, fromEmail: null, sender: null },
      { contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(result.kind).toBe('not_configured')
  })
})

describe('sendCrmEmailViaResend — gated + armed', () => {
  test('suppressed address is not sent', async () => {
    const { calls, sender } = okSender()
    const result = await sendCrmEmailViaResend(
      cannedDb({ suppressed: true }),
      armed(sender),
      { contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(result.kind).toBe('suppressed')
    expect(calls).toHaveLength(0)
  })

  test('armed + eligible => sends, records provider id, idempotency = message id', async () => {
    const { calls, sender } = okSender()
    const result = await sendCrmEmailViaResend(
      cannedDb(),
      armed(sender),
      { contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(result.kind).toBe('sent')
    if (result.kind === 'sent') {
      expect(result.message.providerMessageId).toBe('resend_123')
      expect(result.message.status).toBe('sent')
    }
    expect(calls).toHaveLength(1)
    expect((calls[0] as { idempotencyKey: string }).idempotencyKey).toBe('crm_email_q')
    expect((calls[0] as { to: string }).to).toBe('ada@example.com')
    expect((calls[0] as { subject: string }).subject).toBe('Hello Ada')
  })

  test('provider rejection => failed', async () => {
    const failing: CrmResendSender = async () => ({ errorMessage: 'domain not verified', ok: false })
    const result = await sendCrmEmailViaResend(
      cannedDb(),
      armed(failing),
      { contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(result.kind).toBe('failed')
    if (result.kind === 'failed') {
      expect(result.errorMessage).toContain('domain not verified')
    }
  })
})

describe('makeCrmResendSender (HTTP shape)', () => {
  test('POSTs to Resend with Idempotency-Key and parses the id', async () => {
    const { makeCrmResendSender } = await import('./crm-resend')
    const { Redacted } = await import('effect')
    const fetcher = vi.fn(async () =>
      new Response(JSON.stringify({ id: 'resend_abc' }), { status: 200 }),
    ) as unknown as typeof fetch
    const sender = makeCrmResendSender({ apiKey: Redacted.make('re_test') }, fetcher)
    const result = await sender({
      from: 'a@x.com',
      html: '<p>hi</p>',
      idempotencyKey: 'idem_1',
      subject: 's',
      text: 'hi',
      to: 'b@x.com',
    })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.providerMessageId).toBe('resend_abc')
    const call = (fetcher as unknown as { mock: { calls: Array<Array<unknown>> } }).mock.calls[0]
    expect(call?.[0]).toBe('https://api.resend.com/emails')
    const init = call?.[1] as RequestInit
    expect((init.headers as Record<string, string>)['Idempotency-Key']).toBe('idem_1')
  })
})
