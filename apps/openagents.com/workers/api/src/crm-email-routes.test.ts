import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmEmailRoutes } from './crm-email-routes'

const contactRow = {
  contact_type: 'investor',
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

const messageRow = {
  body_markdown: 'Hi Ada',
  channel: 'gmail_gws',
  contact_id: 'crm_contact_1',
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_email_1',
  status: 'draft',
  subject: 'Hello Ada',
  tenant_ref: 'tenant.openagents',
  to_email: 'ada@example.com',
  updated_at: '2026-06-22T00:00:00.000Z',
}

// Query-routing fake: returns canned rows based on the table the query targets.
const cannedEmailDb = (opts: { suppressed?: boolean } = {}): D1Database => {
  const firstFor = (query: string): Record<string, unknown> | null => {
    if (query.includes('FROM crm_contacts')) return contactRow
    if (query.includes('FROM crm_email_templates')) return templateRow
    if (query.includes('FROM email_suppression_entries')) {
      return opts.suppressed === true ? { id: 'sup_1', scope: 'all' } : null
    }
    if (query.includes('FROM email_preferences')) return null
    if (query.includes('FROM crm_email_messages')) return messageRow
    return null
  }
  const allFor = (query: string): Array<Record<string, unknown>> => {
    if (query.includes('FROM crm_email_templates')) return [templateRow]
    if (query.includes('FROM crm_email_messages')) return [messageRow]
    return []
  }
  const statement = (query: string): D1PreparedStatement =>
    ({
      bind: () => statement(query),
      first: <T,>() => Promise.resolve(firstFor(query) as T | null),
      all: <T,>() =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: allFor(query) as unknown as Array<T>,
          success: true,
        } as D1Result<T>),
      run: () =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: [],
          success: true,
        } as unknown as D1Result),
      raw: () => Promise.reject(new Error('raw should not be used')),
    }) as unknown as D1PreparedStatement
  return {
    batch: () => Promise.reject(new Error('batch should not be used')),
    dump: () => Promise.reject(new Error('dump should not be used')),
    exec: () => Promise.reject(new Error('exec should not be used')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session should not be used')
    },
  } as unknown as D1Database
}

type TestEnv = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext

const run = (admin: boolean, db: D1Database, request: Request): Promise<Response> => {
  const routes = makeCrmEmailRoutes<TestEnv>({
    requireAdminApiToken: () => Promise.resolve(admin),
  })
  const effect = routes.routeCrmEmailRequest(request, { OPENAGENTS_DB: db }, ctx)
  if (effect === undefined) {
    throw new Error(`route did not match: ${request.url}`)
  }
  return Effect.runPromise(effect)
}

const base = 'https://openagents.com'

describe('CRM email templates', () => {
  test('GET lists templates', async () => {
    const res = await run(true, cannedEmailDb(), new Request(`${base}/api/operator/crm/templates`))
    expect(res.status).toBe(200)
    const json = (await res.json()) as { templates: Array<{ slug: string }> }
    expect(json.templates[0]?.slug).toBe('welcome')
  })

  test('POST upserts a template', async () => {
    const res = await run(
      true,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/templates`, {
        body: JSON.stringify({
          bodyMarkdownTemplate: 'Hi {{ contact.first_name }}',
          name: 'Welcome',
          slug: 'welcome',
          subjectTemplate: 'Hello',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
    )
    expect(res.status).toBe(200)
    const json = (await res.json()) as { template: { slug: string } }
    expect(json.template.slug).toBe('welcome')
  })

  test('POST with missing fields is a 400', async () => {
    const res = await run(
      true,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/templates`, {
        body: JSON.stringify({ slug: 'x' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
    )
    expect(res.status).toBe(400)
  })
})

describe('CRM contact render', () => {
  test('composes a personalized message and reports eligibility', async () => {
    const res = await run(
      true,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/contacts/crm_contact_1/render?template=welcome`),
    )
    expect(res.status).toBe(200)
    const json = (await res.json()) as {
      eligibility: { allowed: boolean }
      message: { subject: string; bodyMarkdown: string }
    }
    expect(json.message.subject).toBe('Hello Ada')
    expect(json.message.bodyMarkdown).toBe('Hi Ada')
    expect(json.eligibility.allowed).toBe(true)
  })

  test('reports not-eligible when the address is suppressed', async () => {
    const res = await run(
      true,
      cannedEmailDb({ suppressed: true }),
      new Request(`${base}/api/operator/crm/contacts/crm_contact_1/render?template=welcome`),
    )
    expect(res.status).toBe(200)
    const json = (await res.json()) as { eligibility: { allowed: boolean; reason: string } }
    expect(json.eligibility.allowed).toBe(false)
    expect(json.eligibility.reason).toBe('all_suppressed')
  })

  test('400 without a template param', async () => {
    const res = await run(
      true,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/contacts/crm_contact_1/render`),
    )
    expect(res.status).toBe(400)
  })
})

describe('CRM Gmail write-back', () => {
  test('records a draft message + activity', async () => {
    const res = await run(
      true,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/contacts/crm_contact_1/gmail-writeback`, {
        body: JSON.stringify({
          bodyMarkdown: 'Hi Ada',
          providerDraftId: 'draft_123',
          status: 'draft',
          subject: 'Hello Ada',
          toEmail: 'ada@example.com',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
    )
    expect(res.status).toBe(200)
    const json = (await res.json()) as { message: { id: string } }
    expect(json.message.id).toBe('crm_email_1')
  })

  test('refuses a sent write-back when suppressed (409)', async () => {
    const res = await run(
      true,
      cannedEmailDb({ suppressed: true }),
      new Request(`${base}/api/operator/crm/contacts/crm_contact_1/gmail-writeback`, {
        body: JSON.stringify({
          bodyMarkdown: 'Hi Ada',
          status: 'sent',
          subject: 'Hello Ada',
          toEmail: 'ada@example.com',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
    )
    expect(res.status).toBe(409)
    const json = (await res.json()) as { error: string }
    expect(json.error).toBe('suppressed')
  })

  test('400 when required fields are missing', async () => {
    const res = await run(
      true,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/contacts/crm_contact_1/gmail-writeback`, {
        body: JSON.stringify({ subject: 'x' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
    )
    expect(res.status).toBe(400)
  })
})

describe('CRM email ledger + gating', () => {
  test('GET /emails lists the contact send ledger', async () => {
    const res = await run(
      true,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/contacts/crm_contact_1/emails`),
    )
    expect(res.status).toBe(200)
    const json = (await res.json()) as { messages: Array<{ id: string }> }
    expect(json.messages[0]?.id).toBe('crm_email_1')
  })

  test('401 without an admin token', async () => {
    const res = await run(
      false,
      cannedEmailDb(),
      new Request(`${base}/api/operator/crm/templates`),
    )
    expect(res.status).toBe(401)
  })

  test('non-CRM-email paths pass through', () => {
    const routes = makeCrmEmailRoutes<TestEnv>({
      requireAdminApiToken: () => Promise.resolve(true),
    })
    const effect = routes.routeCrmEmailRequest(
      new Request(`${base}/api/operator/crm/contacts`),
      { OPENAGENTS_DB: cannedEmailDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
