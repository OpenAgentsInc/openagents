import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmMcpDiscoveryRoutes } from './crm-mcp-discovery-routes'

const ctx = {} as ExecutionContext
const routes = makeCrmMcpDiscoveryRoutes()

const run = (request: Request): Promise<Response> => {
  const effect = routes.routeCrmMcpDiscoveryRequest(request, {}, ctx)
  if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
  return Effect.runPromise(effect)
}

describe('CRM MCP discovery doc', () => {
  test('GET /.well-known/openagents-mcp.json advertises the server, transport, and public tool catalog', async () => {
    const res = await run(new Request('https://openagents.com/.well-known/openagents-mcp.json'))
    expect(res.status).toBe(200)
    const doc = (await res.json()) as {
      server: { name: string }
      transport: { endpoint: string; kind: string }
      tools: Array<{ name: string; requiredAuthorities: string[] }>
      resources: Array<{ uri: string }>
    }
    expect(doc.server.name).toBe('openagents-crm-mcp')
    expect(doc.transport.endpoint).toBe('/api/mcp')
    expect(doc.transport.kind).toBe('streamable_http')
    expect(doc.tools.length).toBeGreaterThanOrEqual(21)
    expect(doc.tools.map(t => t.name)).toContain('crm.contacts.list')
    expect(doc.resources.map(r => r.uri)).toContain('mcp://openagents/worker/crm/contacts')
  })

  test('does not leak data — tools carry only metadata (name/title/authorities/risk)', async () => {
    const res = await run(new Request('https://openagents.com/.well-known/openagents-mcp.json'))
    const doc = (await res.json()) as { tools: Array<Record<string, unknown>> }
    for (const tool of doc.tools) {
      expect(Object.keys(tool).sort()).toEqual(
        ['name', 'requiredAuthorities', 'riskClass', 'summary', 'title'].sort(),
      )
    }
  })

  test('non-GET is a 405', async () => {
    const res = await run(
      new Request('https://openagents.com/.well-known/openagents-mcp.json', { method: 'POST' }),
    )
    expect(res.status).toBe(405)
  })

  test('other paths pass through', () => {
    const effect = routes.routeCrmMcpDiscoveryRequest(
      new Request('https://openagents.com/api/mcp', { method: 'GET' }),
      {},
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
