import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmCommandRoutes } from './crm-command-routes'
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

const makeDb = () => {
  const commands = new Map<string, Record<string, unknown>>()
  const statement = (query: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('FROM crm_contact_commands')) {
          return Promise.resolve((commands.get(String(bound[1] ?? '')) ?? null) as T | null)
        }
        if (query.includes('FROM crm_contacts')) return Promise.resolve(contactRow as T)
        if (query.includes('FROM crm_email_templates')) return Promise.resolve(templateRow as T)
        if (query.includes('FROM email_suppression_entries')) return Promise.resolve(null as T | null)
        if (query.includes('FROM email_preferences')) return Promise.resolve(null as T | null)
        if (query.includes('FROM crm_email_messages')) return Promise.resolve(messageRow as T)
        return Promise.resolve(null as T | null)
      },
      all: <T,>() =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: (query.includes('FROM crm_contact_commands')
            ? [...commands.values()]
            : []) as unknown as Array<T>,
          success: true,
        } as D1Result<T>),
      run: () => {
        if (query.includes('INSERT INTO crm_contact_commands')) {
          const [id, tenantRef, contactId, proposedByRef, payloadJson, createdAt, updatedAt] = bound
          commands.set(String(id), {
            approval_state: 'pending_approval',
            command_kind: 'send_email',
            contact_id: contactId,
            created_at: createdAt,
            id,
            payload_json: payloadJson,
            proposed_by_ref: proposedByRef,
            result_json: '{}',
            status: 'proposed',
            tenant_ref: tenantRef,
            updated_at: updatedAt,
          })
        }
        if (query.includes('UPDATE crm_contact_commands')) {
          const [status, approvalState, resultJson, updatedAt, , id] = bound
          const existing = commands.get(String(id))
          if (existing !== undefined) {
            commands.set(String(id), {
              ...existing,
              approval_state: approvalState,
              result_json: resultJson,
              status,
              updated_at: updatedAt,
            })
          }
        }
        return Promise.resolve({ meta: {} as D1Meta, results: [], success: true } as unknown as D1Result)
      },
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

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext
const disabledResend: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }

const routesFor = (admin: boolean, db: D1Database) => {
  const routes = makeCrmCommandRoutes<Env>({
    requireAdminApiToken: () => Promise.resolve(admin),
    resolveResendDeps: () => disabledResend,
  })
  return (request: Request): Promise<Response> => {
    const effect = routes.routeCrmCommandRequest(request, { OPENAGENTS_DB: db }, ctx)
    if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
    return Effect.runPromise(effect)
  }
}

const base = 'https://openagents.com'

const post = (path: string, body: unknown): Request =>
  new Request(`${base}${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('CRM command routes', () => {
  test('propose -> list -> approve happy path', async () => {
    const db = makeDb()
    const run = routesFor(true, db)

    const proposeRes = await run(
      post('/api/operator/crm/contacts/crm_contact_1/commands/send-email', {
        channel: 'gmail_gws',
        templateSlug: 'welcome',
      }),
    )
    expect(proposeRes.status).toBe(201)
    const { command } = (await proposeRes.json()) as { command: { id: string; status: string } }
    expect(command.status).toBe('proposed')

    const listRes = await run(new Request(`${base}/api/operator/crm/commands?status=proposed`))
    expect(listRes.status).toBe(200)
    const { commands } = (await listRes.json()) as { commands: Array<{ id: string }> }
    expect(commands.some(c => c.id === command.id)).toBe(true)

    const approveRes = await run(post(`/api/operator/crm/commands/${command.id}/approve`, {}))
    expect(approveRes.status).toBe(200)
    const { result } = (await approveRes.json()) as { result: { kind: string } }
    expect(result.kind).toBe('executed')
  })

  test('approve missing command => 404', async () => {
    const run = routesFor(true, makeDb())
    const res = await run(post('/api/operator/crm/commands/nope/approve', {}))
    expect(res.status).toBe(404)
  })

  test('reject marks rejected', async () => {
    const db = makeDb()
    const run = routesFor(true, db)
    const proposeRes = await run(
      post('/api/operator/crm/contacts/crm_contact_1/commands/send-email', {
        channel: 'resend',
        templateSlug: 'welcome',
      }),
    )
    const { command } = (await proposeRes.json()) as { command: { id: string } }
    const rejectRes = await run(post(`/api/operator/crm/commands/${command.id}/reject`, { reason: 'no' }))
    expect(rejectRes.status).toBe(200)
  })

  test('propose without templateSlug => 400', async () => {
    const run = routesFor(true, makeDb())
    const res = await run(post('/api/operator/crm/contacts/crm_contact_1/commands/send-email', {}))
    expect(res.status).toBe(400)
  })

  test('401 without admin', async () => {
    const run = routesFor(false, makeDb())
    const res = await run(new Request(`${base}/api/operator/crm/commands`))
    expect(res.status).toBe(401)
  })

  test('non-command path passes through', () => {
    const routes = makeCrmCommandRoutes<Env>({
      requireAdminApiToken: () => Promise.resolve(true),
      resolveResendDeps: () => disabledResend,
    })
    const effect = routes.routeCrmCommandRequest(
      new Request(`${base}/api/operator/crm/contacts`),
      { OPENAGENTS_DB: makeDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
