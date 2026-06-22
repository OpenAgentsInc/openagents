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

const cannedDb = (): D1Database => {
  const firstFor = (q: string): Record<string, unknown> | null =>
    q.includes('FROM crm_contacts') ? contactRow : null
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

describe('CRM MCP read catalog — listing', () => {
  test('exposes 15 read tools with valid names + input schemas', async () => {
    const tools = await catalog.listTools(env, req)
    expect(tools).toHaveLength(15)
    for (const tool of tools) {
      expect(() => assertValidOpenAgentsMcpName(tool.name)).not.toThrow()
      expect(tool.inputSchema).toHaveProperty('type', 'object')
      expect(tool.annotations).toMatchObject({ readOnlyHint: true })
    }
    expect(tools.map(t => t.name)).toContain('crm.contacts.list')
    expect(tools.map(t => t.name)).toContain('crm.contact.render')
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
    const outcome = await catalog.callTool(env, req, 'crm.contacts.list', { limit: 10 })
    expect(outcome.isError).toBe(false)
    expect(Array.isArray(outcome.structuredContent)).toBe(true)
    const list = outcome.structuredContent as Array<{ primaryEmail: string }>
    expect(list[0]?.primaryEmail).toBe('ada@example.com')
    expect(outcome.content[0]?.text).toContain('ada@example.com')
  })

  test('crm.contact.get resolves a contact by id', async () => {
    const outcome = await catalog.callTool(env, req, 'crm.contact.get', { contactId: 'crm_contact_1' })
    const contact = outcome.structuredContent as { id: string }
    expect(contact.id).toBe('crm_contact_1')
  })

  test('crm.contact.get without contactId is a tool error (rejects)', async () => {
    await expect(catalog.callTool(env, req, 'crm.contact.get', {})).rejects.toThrow('contactId is required')
  })

  test('an unknown tool rejects with unknown_tool', async () => {
    await expect(catalog.callTool(env, req, 'crm.nope', {})).rejects.toThrow('unknown_tool')
  })
})
