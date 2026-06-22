import { describe, expect, test } from 'vitest'

import { type CrmResendDeps, type CrmResendSender } from './crm-resend'
import { dispatchCrmSend } from './crm-send'

const contactRow = {
  created_at: '2026-06-22T00:00:00.000Z',
  first_name: 'Ada',
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
const messageRow = {
  body_markdown: 'Hi Ada',
  channel: 'gmail_gws',
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
  const firstFor = (q: string): Record<string, unknown> | null => {
    if (q.includes('FROM crm_contacts')) return contactRow
    if (q.includes('FROM crm_email_templates')) return templateRow
    if (q.includes('FROM email_suppression_entries')) {
      return opts.suppressed === true ? { id: 'sup', scope: 'all' } : null
    }
    if (q.includes('FROM email_preferences')) return null
    if (q.includes('FROM crm_email_messages')) return messageRow
    return null
  }
  const statement = (q: string): D1PreparedStatement =>
    ({
      bind: () => statement(q),
      first: <T,>() => Promise.resolve(firstFor(q) as T | null),
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
    prepare: (q: string) => statement(q),
    withSession: () => {
      throw new Error('session')
    },
  } as unknown as D1Database
}

const runtime = { makeId: (p: string) => `${p}_test`, nowIso: () => '2026-06-22T00:00:00.000Z' }
const okSender: CrmResendSender = async () => ({ ok: true, providerMessageId: 'resend_1' })
const armedResend: CrmResendDeps = { enabled: true, fromEmail: 'x@x.com', sender: okSender }
const disabledResend: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }

const req = (channel: 'gmail_gws' | 'resend') => ({
  channel,
  contactId: 'crm_contact_1',
  templateSlug: 'welcome',
  tenantRef: 'tenant.openagents',
})

describe('dispatchCrmSend — channel selection', () => {
  test('resend (armed) sends and tags the channel', async () => {
    const outcome = await dispatchCrmSend(cannedDb(), { resend: armedResend }, req('resend'), runtime)
    expect(outcome.channel).toBe('resend')
    if (outcome.channel === 'resend') expect(outcome.result.kind).toBe('sent')
  })

  test('resend (disabled) is a dry-run', async () => {
    const outcome = await dispatchCrmSend(cannedDb(), { resend: disabledResend }, req('resend'), runtime)
    expect(outcome.channel).toBe('resend')
    if (outcome.channel === 'resend') expect(outcome.result.kind).toBe('dry_run')
  })

  test('gmail_gws queues a ledger row + returns a send plan for the local executor', async () => {
    const outcome = await dispatchCrmSend(cannedDb(), { resend: disabledResend }, req('gmail_gws'), runtime)
    expect(outcome.channel).toBe('gmail_gws')
    if (outcome.channel === 'gmail_gws' && outcome.kind === 'gmail_queued') {
      expect(outcome.message.status).toBe('queued')
      expect(outcome.plan.toEmail).toBe('ada@example.com')
      expect(outcome.plan.subject).toBe('Hello Ada')
      expect(outcome.plan.bodyMarkdown).toBe('Hi Ada')
    } else {
      throw new Error('expected gmail_queued')
    }
  })
})

describe('dispatchCrmSend — shared suppression gate (both channels)', () => {
  test('resend suppressed', async () => {
    const outcome = await dispatchCrmSend(
      cannedDb({ suppressed: true }),
      { resend: armedResend },
      req('resend'),
      runtime,
    )
    if (outcome.channel === 'resend') expect(outcome.result.kind).toBe('suppressed')
  })

  test('gmail_gws suppressed (never queued)', async () => {
    const outcome = await dispatchCrmSend(
      cannedDb({ suppressed: true }),
      { resend: disabledResend },
      req('gmail_gws'),
      runtime,
    )
    expect(outcome.channel).toBe('gmail_gws')
    if (outcome.channel === 'gmail_gws') expect(outcome.kind).toBe('suppressed')
  })
})
