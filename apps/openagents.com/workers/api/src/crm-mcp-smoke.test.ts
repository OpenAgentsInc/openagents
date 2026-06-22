/**
 * End-to-end CRM MCP smoke (epic #5991, sub-issue #5999).
 *
 * Drives the REAL transport (`crm-mcp-routes`) + REAL catalog (`crm-mcp`) over
 * the JSON-RPC wire, in-process (no network), with an in-memory D1 and three
 * principals (admin / scoped read-only / none). Proves the full client path and
 * the policy guarantees a real MCP client (Inspector / Claude Code / Codex)
 * would exercise — see docs/mcp/crm-mcp-client-runbook.md for the manual
 * external-client steps + config snippets.
 */
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmMcpCatalog } from './crm-mcp'
import { makeCrmMcpRoutes, type McpPrincipal } from './crm-mcp-routes'

const contactRow = {
  contact_type: 'investor',
  created_at: '2026-06-22T00:00:00.000Z',
  full_name: 'Ada Lovelace',
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
const commandRow = {
  approval_state: 'pending_approval',
  command_kind: 'send_email',
  contact_id: 'crm_contact_1',
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_cmd_1',
  payload_json: '{"channel":"gmail_gws","templateSlug":"welcome"}',
  result_json: '{}',
  status: 'proposed',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}

const cannedDb = (opts: { suppressed?: boolean } = {}): D1Database => {
  const firstFor = (q: string): Record<string, unknown> | null => {
    if (q.includes('FROM crm_contact_commands')) return commandRow
    if (q.includes('FROM crm_email_templates')) return templateRow
    if (q.includes('FROM email_suppression_entries')) {
      return opts.suppressed === true ? { id: 'sup', scope: 'all' } : null
    }
    if (q.includes('FROM email_preferences')) return null
    if (q.includes('FROM crm_contacts')) return contactRow
    return null
  }
  const allFor = (q: string): Array<Record<string, unknown>> =>
    q.includes('FROM crm_contacts') ? [contactRow] : []
  const statement = (q: string): D1PreparedStatement =>
    ({
      bind: () => statement(q),
      first: <T,>() => Promise.resolve(firstFor(q) as T | null),
      all: <T,>() =>
        Promise.resolve({ meta: {} as D1Meta, results: allFor(q) as unknown as Array<T>, success: true } as D1Result<T>),
      run: () => Promise.resolve({ meta: {} as D1Meta, results: [], success: true } as unknown as D1Result),
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

type SmokeEnv = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext

const grant = (authorityClass: 'operator_read' | 'workspace_write' | 'approval_resolution') => ({
  authorityClass,
  decision: 'granted' as const,
  grantRef: 'g',
  grantedAt: '2026-06-22T00:00:00.000Z',
  scopeRefs: [],
  sourceRefs: [],
  subjectRef: 's',
})
const adminPrincipal: McpPrincipal = {
  grants: [grant('operator_read'), grant('workspace_write'), grant('approval_resolution')],
  subjectRef: 'admin',
  tenantRef: 'tenant.openagents',
}
const readerPrincipal: McpPrincipal = {
  grants: [grant('operator_read')],
  subjectRef: 'reader',
  tenantRef: 'tenant.openagents',
}

// Maps a bearer token to a principal (admin / reader / none) — stands in for
// the real authenticate (admin token vs scoped grant vs unauthenticated).
const server = (db: D1Database) =>
  makeCrmMcpRoutes<SmokeEnv>({
    authenticate: request => {
      const auth = request.headers.get('authorization') ?? ''
      if (auth === 'Bearer admin') return Promise.resolve(adminPrincipal)
      if (auth === 'Bearer reader') return Promise.resolve(readerPrincipal)
      return Promise.resolve(null)
    },
    catalog: makeCrmMcpCatalog<SmokeEnv>({
      resolveResendDeps: () => ({ enabled: false, fromEmail: null, sender: null }),
    }),
  })

let nextId = 1
const call = (
  routes: ReturnType<typeof server>,
  db: D1Database,
  token: string | null,
  method: string,
  params?: unknown,
): Promise<Response> => {
  const headers: Record<string, string> = { 'content-type': 'application/json' }
  if (token !== null) headers.authorization = `Bearer ${token}`
  const request = new Request('https://openagents.com/api/mcp', {
    body: JSON.stringify({ ...(params === undefined ? {} : { params }), id: nextId++, jsonrpc: '2.0', method }),
    headers,
    method: 'POST',
  })
  const effect = routes.routeCrmMcpRequest(request, { OPENAGENTS_DB: db }, ctx)
  if (effect === undefined) throw new Error('route did not match')
  return Effect.runPromise(effect)
}

describe('CRM MCP end-to-end smoke', () => {
  test('admin client: initialize -> tools/list -> read -> render -> propose', async () => {
    const db = cannedDb()
    const routes = server(db)

    const init = await (await call(routes, db, 'admin', 'initialize')).json()
    expect((init as { result: { protocolVersion: string } }).result.protocolVersion).toBeTruthy()

    const list = (await (await call(routes, db, 'admin', 'tools/list')).json()) as {
      result: { tools: Array<{ name: string }> }
    }
    expect(list.result.tools).toHaveLength(21)

    const contacts = (await (
      await call(routes, db, 'admin', 'tools/call', { arguments: { limit: 5 }, name: 'crm.contacts.list' })
    ).json()) as { result: { isError?: boolean; structuredContent: Array<{ primaryEmail: string }> } }
    expect(contacts.result.isError).toBe(false)
    expect(contacts.result.structuredContent[0]?.primaryEmail).toBe('ada@example.com')

    const render = (await (
      await call(routes, db, 'admin', 'tools/call', {
        arguments: { contactId: 'crm_contact_1', template: 'welcome' },
        name: 'crm.contact.render',
      })
    ).json()) as { result: { structuredContent: { eligibility: { allowed: boolean }; message: { subject: string } } } }
    expect(render.result.structuredContent.eligibility.allowed).toBe(true)
    expect(render.result.structuredContent.message.subject).toBe('Hello Ada')

    const propose = (await (
      await call(routes, db, 'admin', 'tools/call', {
        arguments: { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome' },
        name: 'crm.send.command.propose',
      })
    ).json()) as { result: { structuredContent: { result: { status: string }; receipt: { kind: string } } } }
    expect(propose.result.structuredContent.result.status).toBe('proposed') // recorded; sends nothing
    expect(propose.result.structuredContent.receipt.kind).toBe('mutation')
  })

  test('scoped read-only client sees fewer tools and cannot approve (ungranted = absent)', async () => {
    const db = cannedDb()
    const routes = server(db)

    const list = (await (await call(routes, db, 'reader', 'tools/list')).json()) as {
      result: { tools: Array<{ name: string }> }
    }
    const names = list.result.tools.map(t => t.name)
    expect(names).toContain('crm.contacts.list')
    expect(names).toContain('crm.send.command.propose')
    expect(names).not.toContain('crm.send.command.approve')
    expect(names).not.toContain('crm.import.run')
    expect(names).not.toContain('crm.template.upsert')

    const approve = (await (
      await call(routes, db, 'reader', 'tools/call', {
        arguments: { commandId: 'crm_cmd_1' },
        name: 'crm.send.command.approve',
      })
    ).json()) as { result: { isError: boolean; content: Array<{ text: string }> } }
    expect(approve.result.isError).toBe(true)
    expect(approve.result.content[0]?.text).toContain('Unknown tool')
  })

  test('unauthenticated client is rejected (401)', async () => {
    const db = cannedDb()
    const routes = server(db)
    const res = await call(routes, db, null, 'initialize')
    expect(res.status).toBe(401)
  })

  test('a suppressed address is reported not-eligible (gate held end-to-end)', async () => {
    const db = cannedDb({ suppressed: true })
    const routes = server(db)
    const render = (await (
      await call(routes, db, 'admin', 'tools/call', {
        arguments: { contactId: 'crm_contact_1', template: 'welcome' },
        name: 'crm.contact.render',
      })
    ).json()) as { result: { structuredContent: { eligibility: { allowed: boolean; reason: string } } } }
    expect(render.result.structuredContent.eligibility.allowed).toBe(false)
    expect(render.result.structuredContent.eligibility.reason).toBe('all_suppressed')
  })

  test('revoking access (principal resolves to null) removes all access', async () => {
    const db = cannedDb()
    const routes = server(db)
    // A token that no longer maps to a principal behaves like a revoked grant.
    const res = await call(routes, db, 'revoked-token', 'tools/list')
    expect(res.status).toBe(401)
  })
})
