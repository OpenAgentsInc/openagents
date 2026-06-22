import { describe, expect, test } from 'vitest'

import {
  dispositionForOutcome,
  planCrmBatchWaves,
  runCrmBatch,
} from './crm-batch'
import { type CrmResendDeps, type CrmResendSender } from './crm-resend'

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

const cannedDb = (opts: { suppressed?: boolean; missingTemplate?: boolean } = {}): D1Database => {
  const firstFor = (q: string): Record<string, unknown> | null => {
    if (q.includes('FROM crm_contacts')) return contactRow
    if (q.includes('FROM crm_email_templates')) return opts.missingTemplate === true ? null : templateRow
    if (q.includes('FROM email_suppression_entries')) {
      return opts.suppressed === true ? { id: 's', scope: 'all' } : null
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

const runtime = { makeId: (p: string) => `${p}_1`, nowIso: () => '2026-06-22T00:00:00.000Z' }
const okSender: CrmResendSender = async () => ({ ok: true, providerMessageId: 'r1' })
const armed: CrmResendDeps = { enabled: true, fromEmail: 'x@x.com', sender: okSender }
const disabled: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }

describe('planCrmBatchWaves', () => {
  test('chunks ids into waves', () => {
    expect(planCrmBatchWaves(['a', 'b', 'c', 'd', 'e'], 2)).toEqual([['a', 'b'], ['c', 'd'], ['e']])
  })
  test('defaults a non-positive wave size', () => {
    expect(planCrmBatchWaves(['a'], 0)).toEqual([['a']])
  })
})

describe('dispositionForOutcome', () => {
  test('maps resend + gmail outcomes', () => {
    expect(dispositionForOutcome({ channel: 'resend', result: { kind: 'sent', message: {} as never } })).toBe('sent')
    expect(dispositionForOutcome({ channel: 'resend', result: { kind: 'dry_run', reason: 'send_disabled', subject: 's', toEmail: 't' } })).toBe('dry_run')
    expect(dispositionForOutcome({ channel: 'gmail_gws', kind: 'suppressed', reason: 'x', toEmail: 't' })).toBe('suppressed')
  })
})

describe('runCrmBatch — dry run (no writes)', () => {
  test('classifies would_send vs suppressed', async () => {
    const summary = await runCrmBatch(
      cannedDb(),
      { resend: disabled },
      { channel: 'gmail_gws', contactIds: ['c1', 'c2'], dryRun: true, templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(summary.dryRun).toBe(true)
    expect(summary.total).toBe(2)
    expect(summary.counts.would_send).toBe(2)
    expect(summary.counts.suppressed).toBe(0)
  })

  test('suppressed contacts are flagged in dry run', async () => {
    const summary = await runCrmBatch(
      cannedDb({ suppressed: true }),
      { resend: disabled },
      { channel: 'gmail_gws', contactIds: ['c1'], dryRun: true, templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(summary.counts.suppressed).toBe(1)
    expect(summary.counts.would_send).toBe(0)
  })

  test('a missing template fails that row without aborting the wave', async () => {
    const summary = await runCrmBatch(
      cannedDb({ missingTemplate: true }),
      { resend: disabled },
      { channel: 'gmail_gws', contactIds: ['c1', 'c2'], dryRun: true, templateSlug: 'nope', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(summary.counts.failed).toBe(2)
  })
})

describe('runCrmBatch — live', () => {
  test('gmail channel queues each contact', async () => {
    const summary = await runCrmBatch(
      cannedDb(),
      { resend: disabled },
      { channel: 'gmail_gws', contactIds: ['c1', 'c2', 'c3'], dryRun: false, templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(summary.counts.queued).toBe(3)
  })

  test('resend channel (armed) sends each contact', async () => {
    const summary = await runCrmBatch(
      cannedDb(),
      { resend: armed },
      { channel: 'resend', contactIds: ['c1', 'c2'], dryRun: false, templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(summary.counts.sent).toBe(2)
  })

  test('suppressed contacts are not sent (live)', async () => {
    const summary = await runCrmBatch(
      cannedDb({ suppressed: true }),
      { resend: armed },
      { channel: 'resend', contactIds: ['c1'], dryRun: false, templateSlug: 'welcome', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(summary.counts.suppressed).toBe(1)
    expect(summary.counts.sent).toBe(0)
  })
})
