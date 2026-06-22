import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type CrmResendDeps, type CrmResendSender } from './crm-resend'
import { makeCrmSendRoutes } from './crm-send-routes'

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
  const allFor = (q: string): Array<Record<string, unknown>> =>
    q.includes("channel = 'gmail_gws'") ? [messageRow] : []
  const statement = (q: string): D1PreparedStatement =>
    ({
      bind: () => statement(q),
      first: <T,>() => Promise.resolve(firstFor(q) as T | null),
      all: <T,>() =>
        Promise.resolve({ meta: {} as D1Meta, results: allFor(q) as unknown as Array<T>, success: true } as D1Result<T>),
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

type SendEnv = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext

const okSender: CrmResendSender = async () => ({ ok: true, providerMessageId: 'resend_1' })
const disabled: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }
const armed: CrmResendDeps = { enabled: true, fromEmail: 'x@x.com', sender: okSender }

const run = (
  admin: boolean,
  resend: CrmResendDeps,
  db: D1Database,
  request: Request,
): Promise<Response> => {
  const routes = makeCrmSendRoutes<SendEnv>({
    requireAdminApiToken: () => Promise.resolve(admin),
    resolveResendDeps: () => resend,
  })
  const effect = routes.routeCrmSendRequest(request, { OPENAGENTS_DB: db }, ctx)
  if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
  return Effect.runPromise(effect)
}

const sendReq = (body: unknown): Request =>
  new Request('https://openagents.com/api/operator/crm/contacts/crm_contact_1/send', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('unified CRM send route', () => {
  test('gmail_gws => 200 gmail_queued', async () => {
    const res = await run(true, disabled, cannedDb(), sendReq({ channel: 'gmail_gws', templateSlug: 'welcome' }))
    expect(res.status).toBe(200)
    const json = (await res.json()) as { outcome: { channel: string; kind?: string } }
    expect(json.outcome.channel).toBe('gmail_gws')
    expect(json.outcome.kind).toBe('gmail_queued')
  })

  test('resend disabled => 200 dry_run', async () => {
    const res = await run(true, disabled, cannedDb(), sendReq({ channel: 'resend', templateSlug: 'welcome' }))
    expect(res.status).toBe(200)
    const json = (await res.json()) as { outcome: { channel: string; result?: { kind: string } } }
    expect(json.outcome.result?.kind).toBe('dry_run')
  })

  test('resend armed => 200 sent', async () => {
    const res = await run(true, armed, cannedDb(), sendReq({ channel: 'resend', templateSlug: 'welcome' }))
    expect(res.status).toBe(200)
    const json = (await res.json()) as { outcome: { result?: { kind: string } } }
    expect(json.outcome.result?.kind).toBe('sent')
  })

  test('suppressed gmail_gws => 409', async () => {
    const res = await run(
      true,
      disabled,
      cannedDb({ suppressed: true }),
      sendReq({ channel: 'gmail_gws', templateSlug: 'welcome' }),
    )
    expect(res.status).toBe(409)
  })

  test('400 without templateSlug', async () => {
    const res = await run(true, disabled, cannedDb(), sendReq({ channel: 'resend' }))
    expect(res.status).toBe(400)
  })

  test('401 without admin', async () => {
    const res = await run(false, disabled, cannedDb(), sendReq({ channel: 'resend', templateSlug: 'welcome' }))
    expect(res.status).toBe(401)
  })
})

describe('Gmail executor queue route', () => {
  test('GET /gmail-queue lists queued gmail messages', async () => {
    const res = await run(
      true,
      disabled,
      cannedDb(),
      new Request('https://openagents.com/api/operator/crm/gmail-queue', { method: 'GET' }),
    )
    expect(res.status).toBe(200)
    const json = (await res.json()) as { messages: Array<{ id: string; status: string }> }
    expect(json.messages[0]?.id).toBe('crm_email_q')
    expect(json.messages[0]?.status).toBe('queued')
  })

  test('unrelated path passes through', () => {
    const routes = makeCrmSendRoutes<SendEnv>({
      requireAdminApiToken: () => Promise.resolve(true),
      resolveResendDeps: () => disabled,
    })
    const effect = routes.routeCrmSendRequest(
      new Request('https://openagents.com/api/operator/crm/contacts'),
      { OPENAGENTS_DB: cannedDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
