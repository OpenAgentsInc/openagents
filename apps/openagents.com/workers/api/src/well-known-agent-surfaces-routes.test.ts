import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { PUBLIC_MCP_PATH } from './public-agent-mcp-discovery'
import {
  AI_CATALOG_PATH,
  MCP_MANIFEST_PATHS,
  routeWellKnownAgentSurfaceRequest,
} from './well-known-agent-surfaces-routes'

const run = (path: string, init?: RequestInit): Promise<Response> => {
  const request = new Request(`https://openagents.com${path}`, init)
  const effect = routeWellKnownAgentSurfaceRequest(request)
  if (effect === undefined) throw new Error(`route did not match: ${path}`)
  return Effect.runPromise(effect)
}

describe('well-known agent-discovery surfaces', () => {
  test('other paths pass through', () => {
    expect(
      routeWellKnownAgentSurfaceRequest(new Request('https://openagents.com/.well-known/openagents.json')),
    ).toBeUndefined()
  })

  for (const path of MCP_MANIFEST_PATHS) {
    test(`GET ${path} serves a valid MCP manifest pointing at the public MCP endpoint`, async () => {
      const res = await run(path)
      expect(res.status).toBe(200)
      expect(res.headers.get('content-type')).toContain('application/json')
      const doc = (await res.json()) as {
        name: string
        transport: string
        url: string
        tools: Array<{ name: string; ui?: string }>
      }
      expect(doc.name).toBe('openagents')
      expect(doc.transport).toBe('streamable-http')
      expect(doc.url).toBe(`https://openagents.com${PUBLIC_MCP_PATH}`)
      expect(doc.tools.map(t => t.name)).toContain('openagents.get_developer_resources')
      expect(
        doc.tools.find(t => t.name === 'openagents.get_developer_resources')?.ui,
      ).toBe('ui://openagents/developer-resources')
    })
  }

  test('GET /.well-known/ai-catalog.json serves a valid ARD catalog document', async () => {
    const res = await run(AI_CATALOG_PATH)
    expect(res.status).toBe(200)
    const doc = (await res.json()) as {
      specVersion: string
      host: { displayName: string; identifier: string }
      entries: Array<{ identifier: string; type: string; url: string }>
    }
    expect(doc.specVersion).toBe('1.0')
    expect(doc.host.displayName).toBe('OpenAgents')
    expect(doc.host.identifier).toBe('openagents.com')
    expect(doc.entries.length).toBeGreaterThanOrEqual(2)
    for (const entry of doc.entries) {
      expect(entry.identifier.startsWith('urn:air:')).toBe(true)
      expect(entry.url.startsWith('https://openagents.com')).toBe(true)
    }
  })

  test('POST is method-not-allowed for both surfaces', async () => {
    const mcpRes = await run(MCP_MANIFEST_PATHS[0], { method: 'POST' })
    expect(mcpRes.status).toBe(405)
    const catalogRes = await run(AI_CATALOG_PATH, { method: 'POST' })
    expect(catalogRes.status).toBe(405)
  })
})
