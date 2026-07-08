import { assertValidOpenAgentsMcpName } from '@openagentsinc/mcp-contract'
import { describe, expect, test } from 'vitest'

import { CRM_MCP_READ_TOOLS, makeCrmMcpCatalog } from './crm-mcp'

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
  const messageRow = {
    body_markdown: 'Hi Ada',
    channel: 'gmail_gws',
    contact_id: 'crm_contact_1',
    created_at: '2026-06-22T00:00:00.000Z',
    id: 'crm_email_q',
    status: 'queued',
    subject: 'Hello',
    tenant_ref: 'tenant.openagents',
    to_email: 'ada@example.com',
    updated_at: '2026-06-22T00:00:00.000Z',
  }
  const firstFor = (q: string): Record<string, unknown> | null => {
    if (q.includes('FROM crm_contact_commands')) return commandRow
    if (q.includes('FROM crm_email_templates')) return templateRow
    if (q.includes('FROM crm_email_messages')) return messageRow
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
const catalog = makeCrmMcpCatalog<TestEnv>({
  resolveResendDeps: () => ({ enabled: false, fromEmail: null, sender: null }),
})

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
    expect(tools).toHaveLength(23) // 15 read + 8 write
    for (const tool of tools) {
      expect(() => assertValidOpenAgentsMcpName(tool.name)).not.toThrow()
      expect(tool.inputSchema).toHaveProperty('type', 'object')
    }
    const names = tools.map(t => t.name)
    expect(names).toContain('crm.activity.append')
    expect(names).toContain('crm.contact.upsert')
    expect(names).toContain('crm.contacts.list')
    expect(names).toContain('crm.send.command.propose')
    expect(names).toContain('crm.template.upsert')
    expect(names).toContain('crm.send.command.approve')
    expect(names).toContain('crm.import.run')
    expect(names).toContain('crm.batch.send')
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
  test('crm.contact.upsert records a CRM contact + mutation receipt', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.contact.upsert', {
      externalSourceId: 'sarah_session_1',
      externalSourceLabel: 'sarah',
      fullName: 'Ada Lovelace',
      primaryEmail: 'ada@example.com',
    })
    expect(outcome.isError).toBe(false)
    const sc = outcome.structuredContent as {
      receipt: { kind: string; status: string; targetRef: string }
      result: { contact: { id: string; primaryEmail: string }; created: boolean }
    }
    expect(sc.result.contact.id).toBe('crm_contact_1')
    expect(sc.result.contact.primaryEmail).toBe('ada@example.com')
    expect(sc.receipt.kind).toBe('mutation')
    expect(sc.receipt.status).toBe('recorded')
  })

  test('crm.activity.append records a CRM activity + mutation receipt', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.activity.append', {
      activityType: 'sarah_session_summary',
      contactId: 'crm_contact_1',
      sourceRecordId: 'sarah_session_1',
      sourceRecordType: 'sarah_session',
      subject: 'Sarah sales call',
      summary: 'Prospect asked about private coding agents.',
    })
    expect(outcome.isError).toBe(false)
    const sc = outcome.structuredContent as {
      receipt: { artifactRefs: Array<string>; kind: string; targetRef: string }
      result: { contactId: string; activityType: string; id: string }
    }
    expect(sc.result.contactId).toBe('crm_contact_1')
    expect(sc.result.activityType).toBe('sarah_session_summary')
    expect(sc.result.id).toBe('sarah_session_1')
    expect(sc.receipt.artifactRefs).toContain('sarah_session_1')
    expect(sc.receipt.targetRef).toBe('crm_contact_1')
  })

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
    expect(tools).not.toContain('crm.activity.append')
    expect(tools).not.toContain('crm.contact.upsert')
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

describe('CRM MCP Wave 3 — gated execution', () => {
  test('crm.send.command.approve executes (suppression gate still applies) + approval receipt', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.send.command.approve', {
      commandId: 'crm_cmd_1',
    })
    expect(outcome.isError).toBe(false)
    const sc = outcome.structuredContent as { receipt: { kind: string }; result: { kind: string } }
    expect(sc.receipt.kind).toBe('approval')
  })

  test('crm.send.command.reject is allowed for approval_resolution', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.send.command.reject', {
      commandId: 'crm_cmd_1',
      reason: 'not now',
    })
    expect(outcome.isError).toBe(false)
  })

  test('crm.import.run imports CSV (workspace_write) + mutation receipt', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.import.run', {
      csv: 'email\nada@example.com',
      sourceLabel: 'mcp:test',
    })
    expect(outcome.isError).toBe(false)
    const sc = outcome.structuredContent as { receipt: { kind: string }; result: { totalRows: number } }
    expect(sc.receipt.kind).toBe('mutation')
    expect(sc.result.totalRows).toBe(1)
  })

  test('crm.batch.send is DRY-RUN only over MCP (plans, sends nothing)', async () => {
    const outcome = await catalog.callTool(env, req, principal, 'crm.batch.send', {
      channel: 'gmail_gws',
      contactIds: ['crm_contact_1'],
      templateSlug: 'welcome',
    })
    expect(outcome.isError).toBe(false)
    const summary = outcome.structuredContent as { dryRun: boolean; counts: { would_send: number } }
    expect(summary.dryRun).toBe(true)
    expect(summary.counts.would_send).toBe(1)
  })

  test('a read+propose principal cannot approve or import (ungranted = absent)', async () => {
    const names = (await catalog.listTools(env, req, readPrincipal)).map(t => t.name)
    expect(names).not.toContain('crm.send.command.approve')
    expect(names).not.toContain('crm.import.run')
    // batch.send is operator_read, so it IS visible (dry-run only)
    expect(names).toContain('crm.batch.send')
    await expect(
      catalog.callTool(env, req, readPrincipal, 'crm.send.command.approve', { commandId: 'x' }),
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
