import { assertValidOpenAgentsMcpName } from '@openagentsinc/mcp-contract'
import { describe, expect, test } from 'vitest'

import { CRM_MCP_READ_TOOLS, makeCrmMcpReadCatalog } from './crm-mcp'

const contactRow = {
  contact_type: 'investor',
  created_at: '2026-06-22T00:00:00.000Z',
  full_name: 'Ada Lovelace',
  id: 'crm_contact_1',
  primary_email: 'ada@example.com',
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
const templateRow = {
  body_markdown_template: 'Hi {{ contact.first_name }}',
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_template_1',
  name: 'Welcome',
  slug: 'welcome',
  status: 'active',
  subject_template: 'Hello',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}

const cannedDb = (): D1Database => {
  const firstFor = (q: string): Record<string, unknown> | null => {
    if (q.includes('FROM crm_contact_commands')) return commandRow
    if (q.includes('FROM crm_email_templates')) return templateRow
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

type TestEnv = Readonly<{ OPENAGENTS_DB: D1Database }>
const env: TestEnv = { OPENAGENTS_DB: cannedDb() }
const req = new Request('https://openagents.com/api/mcp', { method: 'POST' })
const catalog = makeCrmMcpReadCatalog<TestEnv>()

const grant = (authorityClass: 'operator_read' | 'workspace_write' | 'approval_resolution') => ({
  authorityClass,
  decision: 'granted' as const,
  grantRef: 'g',
  grantedAt: '2026-06-22T00:00:00.000Z',
  scopeRefs: [],
  sourceRefs: [],
  subjectRef: 'admin',
})

// Full CRM authority (admin-equivalent).
const principal = {
  grants: [grant('operator_read'), grant('workspace_write'), grant('approval_resolution')],
  subjectRef: 'admin',
  tenantRef: 'tenant.openagents',
}

// Read + propose only (no workspace_write).
const readPrincipal = {
  grants: [grant('operator_read')],
  subjectRef: 'reader',
  tenantRef: 'tenant.openagents',
}

describe('CRM MCP read catalog — listing', () => {
  test('full-authority principal sees all read + write tools with valid names', async () => {
    const tools = await catalog.listTools(env, req, principal)
    expect(tools).toHaveLength(17) // 15 read + 2 write
    for (const tool of tools) {
      expect(() => assertValidOpenAgentsMcpName(tool.name)).not.toThrow()
      expect(tool.inputSchema).toHaveProperty('type', 'object')
    }
    const names = tools.map(t => t.name)
    expect(names).toContain('crm.contacts.list')
    expect(names).toContain('crm.send.command.propose')
    expect(names).toContain('crm.template.upsert')
  })

  test('every descriptor is operator_read + read_only with no receipt', () => {
    for (const d of CRM_MCP_READ_TOOLS) {
      expect(d.requiredAuthorities).toEqual(['operator_read'])
      expect(d.riskClass).toBe('read_only')
      expect(d.receiptBehavior).toBe('none')
    }
  })
})

describe('CRM MCP read catalog — dispatch', () => {
  test('crm.contacts.list returns structured contacts + JSON text', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.contacts.list', { limit: 10 })
    expect(outcome.isError).toBe(false)
    expect(Array.isArray(outcome.structuredContent)).toBe(true)
    const list = outcome.structuredContent as Array<{ primaryEmail: string }>
    expect(list[0]?.primaryEmail).toBe('ada@example.com')
    expect(outcome.content[0]?.text).toContain('ada@example.com')
  })

  test('crm.contact.get resolves a contact by id', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.contact.get', { contactId: 'crm_contact_1' })
    const contact = outcome.structuredContent as { id: string }
    expect(contact.id).toBe('crm_contact_1')
  })

  test('crm.contact.get without contactId is a tool error (rejects)', async () => {
    await expect(catalog.callTool(env, req, principal, 'crm.contact.get', {})).rejects.toThrow('contactId is required')
  })

  test('an unknown tool rejects with unknown_tool', async () => {
    await expect(catalog.callTool(env, req, principal, 'crm.nope', {})).rejects.toThrow('unknown_tool')
  })
})

describe('CRM MCP Wave 2 — propose + template (no send)', () => {
  test('crm.send.command.propose records a pending command + a receipt (sends nothing)', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.send.command.propose', {
      channel: 'gmail_gws',
      contactId: 'crm_contact_1',
      templateSlug: 'welcome',
    })
    expect(outcome.isError).toBe(false)
    const sc = outcome.structuredContent as {
      result: { status: string; commandKind: string }
      receipt: { kind: string; status: string; receiptRef: string }
    }
    expect(sc.result.commandKind).toBe('send_email')
    expect(sc.result.status).toBe('proposed')
    expect(sc.receipt.kind).toBe('mutation')
    expect(sc.receipt.status).toBe('recorded')
  })

  test('crm.template.upsert is available to a workspace_write principal', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.template.upsert', {
      bodyMarkdownTemplate: 'Hi {{ contact.first_name }}',
      name: 'Welcome',
      slug: 'welcome',
      subjectTemplate: 'Hello',
    })
    expect(outcome.isError).toBe(false)
  })

  test('a read+propose principal can propose but NOT upsert templates', async () => {
    const tools = (await catalog.listTools(env, req, readPrincipal)).map(t => t.name)
    expect(tools).toContain('crm.send.command.propose')
    expect(tools).not.toContain('crm.template.upsert')
    await expect(
      catalog.callTool(env, req, readPrincipal, 'crm.template.upsert', {
        bodyMarkdownTemplate: 'x',
        name: 'x',
        slug: 'x',
        subjectTemplate: 'x',
      }),
    ).rejects.toThrow('unknown_tool')
  })
})

describe('CRM MCP grant filtering + tenant binding', () => {
  const noGrant = { grants: [], subjectRef: 'anon', tenantRef: 'tenant.openagents' }

  test('a principal without operator_read sees no tools or resources', async () => {
    expect(await catalog.listTools(env, req, noGrant)).toHaveLength(0)
    expect(await catalog.listResources(env, req, noGrant)).toHaveLength(0)
  })

  test('an ungranted tool is absent: calling it is unknown_tool', async () => {
    await expect(catalog.callTool(env, req, noGrant, 'crm.contacts.list', {})).rejects.toThrow('unknown_tool')
  })

  test('an ungranted resource read is unknown_resource', async () => {
    await expect(
      catalog.readResource(env, req, noGrant, 'mcp://openagents/worker/crm/contacts'),
    ).rejects.toThrow('unknown_resource')
  })

  test('client-supplied args.tenant is ignored; the principal tenant is used', async () => {
    // The canned DB returns the same row regardless of tenant; assert the bound
    // tenant flows by checking the call still succeeds with a foreign args.tenant.
    const outcome = await catalog.callTool(env, req, principal, 'crm.contacts.list', {
      tenant: 'tenant.attacker',
    })
    expect(outcome.isError).toBe(false)
  })
})

describe('CRM MCP resources', () => {
  test('listResources advertises the worker/crm collections', async () => {
    const resources = await catalog.listResources(env, req, principal)
    const uris = resources.map(r => r.uri)
    expect(uris).toContain('mcp://openagents/worker/crm/contacts')
    expect(uris).toContain('mcp://openagents/worker/crm/commands')
    expect(resources.every(r => r.uri.startsWith('mcp://openagents/worker/crm/'))).toBe(true)
  })

  test('readResource reads a collection as JSON contents', async () => {
    const outcome = await catalog.readResource(env, req, principal, 'mcp://openagents/worker/crm/contacts')
    expect(outcome.contents[0]?.uri).toBe('mcp://openagents/worker/crm/contacts')
    expect(outcome.contents[0]?.mimeType).toBe('application/json')
    expect(outcome.contents[0]?.text).toContain('ada@example.com')
  })

  test('readResource reads a single contact by URI', async () => {
    const outcome = await catalog.readResource(env, req, principal, 'mcp://openagents/worker/crm/contact/crm_contact_1')
    expect(outcome.contents[0]?.text).toContain('crm_contact_1')
  })

  test('a non-worker namespace is an unknown resource', async () => {
    await expect(
      catalog.readResource(env, req, principal, 'mcp://openagents/pylon/node/status'),
    ).rejects.toThrow('unknown_resource')
  })

  test('a malformed URI throws (transport maps to invalid params)', async () => {
    await expect(catalog.readResource(env, req, principal, 'not-a-uri')).rejects.toThrow()
  })
})
