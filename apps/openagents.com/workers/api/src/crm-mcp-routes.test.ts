import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CRM_MCP_PROTOCOL_VERSION,
  type CrmMcpCatalog,
  emptyCrmMcpCatalog,
  makeCrmMcpRoutes,
} from './crm-mcp-routes'

type TestEnv = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext
const env: TestEnv = { OPENAGENTS_DB: {} as unknown as D1Database }

const testPrincipal = {
  grants: [
    {
      authorityClass: 'operator_read' as const,
      decision: 'granted' as const,
      grantRef: 'g',
      grantedAt: '2026-06-22T00:00:00.000Z',
      scopeRefs: [],
      sourceRefs: [],
      subjectRef: 'admin',
    },
  ],
  subjectRef: 'admin',
  tenantRef: 'tenant.openagents',
}

const run = (
  admin: boolean,
  catalog: CrmMcpCatalog<TestEnv>,
  request: Request,
): Promise<Response> => {
  const routes = makeCrmMcpRoutes<TestEnv>({
    authenticate: () => Promise.resolve(admin ? testPrincipal : null),
    catalog,
  })
  const effect = routes.routeCrmMcpRequest(request, env, ctx)
  if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
  return Effect.runPromise(effect)
}

const rpc = (body: unknown): Request =>
  new Request('https://openagents.com/api/mcp', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const req = (id: unknown, method: string, params?: unknown) => ({
  ...(params === undefined ? {} : { params }),
  id,
  jsonrpc: '2.0',
  method,
})

describe('CRM MCP transport — lifecycle', () => {
  test('initialize returns protocol version, capabilities, and server info', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), rpc(req(1, 'initialize')))
    expect(res.status).toBe(200)
    const json = (await res.json()) as {
      result: { protocolVersion: string; capabilities: Record<string, unknown>; serverInfo: { name: string } }
    }
    expect(json.result.protocolVersion).toBe(CRM_MCP_PROTOCOL_VERSION)
    expect(json.result.capabilities).toHaveProperty('tools')
    expect(json.result.capabilities).toHaveProperty('resources')
    expect(json.result.serverInfo.name).toBe('openagents-crm-mcp')
  })

  test('ping returns an empty result', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), rpc(req(2, 'ping')))
    const json = (await res.json()) as { result: unknown }
    expect(json.result).toEqual({})
  })

  test('notifications/initialized returns 202 with no body', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), rpc({ jsonrpc: '2.0', method: 'notifications/initialized' }))
    expect(res.status).toBe(202)
  })
})

describe('CRM MCP transport — tools', () => {
  test('tools/list returns the catalog tools', async () => {
    const catalog: CrmMcpCatalog<TestEnv> = {
      ...emptyCrmMcpCatalog<TestEnv>(),
      listTools: () =>
        Promise.resolve([
          { description: 'List contacts', inputSchema: { type: 'object' }, name: 'crm.contacts.list', title: 'List contacts' },
        ]),
    }
    const res = await run(true, catalog, rpc(req(3, 'tools/list')))
    const json = (await res.json()) as { result: { tools: Array<{ name: string }> } }
    expect(json.result.tools[0]?.name).toBe('crm.contacts.list')
  })

  test('tools/call dispatches to the catalog', async () => {
    const catalog: CrmMcpCatalog<TestEnv> = {
      ...emptyCrmMcpCatalog<TestEnv>(),
      callTool: (_e, _r, _principal, name) =>
        Promise.resolve({ content: [{ text: `called ${name}`, type: 'text' as const }] }),
    }
    const res = await run(true, catalog, rpc(req(4, 'tools/call', { arguments: {}, name: 'crm.contacts.list' })))
    const json = (await res.json()) as { result: { content: Array<{ text: string }>; isError?: boolean } }
    expect(json.result.content[0]?.text).toBe('called crm.contacts.list')
    expect(json.result.isError).toBeUndefined()
  })

  test('tools/call on an unknown tool returns an isError result (not a transport error)', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), rpc(req(5, 'tools/call', { name: 'crm.nope' })))
    expect(res.status).toBe(200)
    const json = (await res.json()) as { result: { isError: boolean; content: Array<{ text: string }> } }
    expect(json.result.isError).toBe(true)
    expect(json.result.content[0]?.text).toContain('Unknown tool')
  })

  test('tools/call without a name is invalid params', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), rpc(req(6, 'tools/call', {})))
    const json = (await res.json()) as { error: { code: number } }
    expect(json.error.code).toBe(-32602)
  })
})

describe('CRM MCP transport — errors + auth', () => {
  test('401 without an admin token', async () => {
    const res = await run(false, emptyCrmMcpCatalog(), rpc(req(7, 'initialize')))
    expect(res.status).toBe(401)
  })

  test('405 on a non-POST method', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), new Request('https://openagents.com/api/mcp', { method: 'GET' }))
    expect(res.status).toBe(405)
  })

  test('unknown method is method-not-found', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), rpc(req(8, 'frobnicate')))
    const json = (await res.json()) as { error: { code: number } }
    expect(json.error.code).toBe(-32601)
  })

  test('malformed JSON-RPC is a parse error', async () => {
    const res = await run(true, emptyCrmMcpCatalog(), rpc({ not: 'jsonrpc' }))
    const json = (await res.json()) as { error: { code: number } }
    expect(json.error.code).toBe(-32700)
  })

  test('non-/api/mcp path passes through', () => {
    const routes = makeCrmMcpRoutes<TestEnv>({
      catalog: emptyCrmMcpCatalog(),
      authenticate: () => Promise.resolve(testPrincipal),
    })
    const effect = routes.routeCrmMcpRequest(
      new Request('https://openagents.com/api/operator/crm/contacts', { method: 'POST' }),
      env,
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
