import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmBatchRoutes } from './crm-batch-routes'
import { type CrmResendDeps } from './crm-resend'

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

const cannedDb = (): D1Database => {
  const firstFor = (q: string): Record<string, unknown> | null => {
    if (q.includes('FROM crm_contacts')) return contactRow
    if (q.includes('FROM crm_email_templates')) return templateRow
    if (q.includes('FROM email_suppression_entries')) return null
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

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext
const disabled: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }

const run = (admin: boolean, request: Request): Promise<Response> => {
  const routes = makeCrmBatchRoutes<Env>({
    requireAdminApiToken: () => Promise.resolve(admin),
    resolveResendDeps: () => disabled,
  })
  const effect = routes.routeCrmBatchRequest(request, { OPENAGENTS_DB: cannedDb() }, ctx)
  if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
  return Effect.runPromise(effect)
}

const batchReq = (body: unknown): Request =>
  new Request('https://openagents.com/api/operator/crm/send-batch', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('CRM batch route', () => {
  test('defaults to dry-run (no dryRun field) and classifies would_send', async () => {
    const res = await run(true, batchReq({ contactIds: ['c1', 'c2'], templateSlug: 'welcome' }))
    expect(res.status).toBe(200)
    const json = (await res.json()) as { summary: { dryRun: boolean; counts: { would_send: number } } }
    expect(json.summary.dryRun).toBe(true)
    expect(json.summary.counts.would_send).toBe(2)
  })

  test('live gmail batch (dryRun:false) queues each contact', async () => {
    const res = await run(
      true,
      batchReq({ channel: 'gmail_gws', contactIds: ['c1'], dryRun: false, templateSlug: 'welcome' }),
    )
    expect(res.status).toBe(200)
    const json = (await res.json()) as { summary: { counts: { queued: number } } }
    expect(json.summary.counts.queued).toBe(1)
  })

  test('400 without contactIds', async () => {
    const res = await run(true, batchReq({ templateSlug: 'welcome' }))
    expect(res.status).toBe(400)
  })

  test('400 without templateSlug', async () => {
    const res = await run(true, batchReq({ contactIds: ['c1'] }))
    expect(res.status).toBe(400)
  })

  test('401 without admin', async () => {
    const res = await run(false, batchReq({ contactIds: ['c1'], templateSlug: 'welcome' }))
    expect(res.status).toBe(401)
  })

  test('non-batch path passes through', () => {
    const routes = makeCrmBatchRoutes<Env>({
      requireAdminApiToken: () => Promise.resolve(true),
      resolveResendDeps: () => disabled,
    })
    const effect = routes.routeCrmBatchRequest(
      new Request('https://openagents.com/api/operator/crm/contacts'),
      { OPENAGENTS_DB: cannedDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
