import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PUBLIC_MCP_PATH,
  PUBLIC_MCP_TOOLS,
  routePublicAgentMcpRequest,
} from './public-agent-mcp-routes'

const rpc = (method: string, params?: Record<string, unknown>) =>
  new Request(`https://openagents.com${PUBLIC_MCP_PATH}`, {
    body: JSON.stringify({ id: 1, jsonrpc: '2.0', method, params }),
    method: 'POST',
  })

const run = (request: Request): Promise<Response> => {
  const effect = routePublicAgentMcpRequest(request)
  if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
  return Effect.runPromise(effect)
}

describe('public agent MCP server (no auth)', () => {
  test('other paths pass through', () => {
    expect(
      routePublicAgentMcpRequest(new Request('https://openagents.com/api/mcp')),
    ).toBeUndefined()
  })

  test('GET is not allowed (transport is POST-only, no auth boundary to fail on)', async () => {
    const res = await run(new Request(`https://openagents.com${PUBLIC_MCP_PATH}`))
    expect(res.status).toBe(405)
  })

  test('initialize never returns 401 — this transport is intentionally unauthenticated', async () => {
    const res = await run(rpc('initialize'))
    expect(res.status).toBe(200)
    const body = (await res.json()) as {
      result: {
        capabilities: { extensions: { 'io.modelcontextprotocol/ui': { mimeTypes: string[] } } }
        serverInfo: { name: string }
      }
    }
    expect(body.result.serverInfo.name).toBe('openagents-public-mcp')
    expect(body.result.capabilities.extensions['io.modelcontextprotocol/ui'].mimeTypes).toEqual([
      'text/html;profile=mcp-app',
    ])
  })

  test('tools/list advertises both public tools with the UI resource on developer_resources', async () => {
    const res = await run(rpc('tools/list'))
    const body = (await res.json()) as {
      result: { tools: Array<{ name: string; _meta?: { ui?: { resourceUri: string } } }> }
    }
    expect(body.result.tools.map(t => t.name)).toEqual(
      PUBLIC_MCP_TOOLS.map(t => t.name),
    )
    const devResources = body.result.tools.find(
      t => t.name === 'openagents.get_developer_resources',
    )
    expect(devResources?._meta?.ui?.resourceUri).toBe('ui://openagents/developer-resources')
    const manifest = body.result.tools.find(
      t => t.name === 'openagents.get_capability_manifest',
    )
    expect(manifest?._meta).toBeUndefined()
  })

  test('tools/call openagents.get_developer_resources returns real public links', async () => {
    const res = await run(rpc('tools/call', { arguments: {}, name: 'openagents.get_developer_resources' }))
    const body = (await res.json()) as {
      result: { structuredContent: { agentOnboarding: string; sourceCode: string } }
    }
    expect(body.result.structuredContent.agentOnboarding).toBe('https://openagents.com/AGENTS.md')
    expect(body.result.structuredContent.sourceCode).toBe(
      'https://github.com/OpenAgentsInc/openagents',
    )
  })

  test('tools/call openagents.get_capability_manifest proxies the real manifest builder', async () => {
    const res = await run(rpc('tools/call', { arguments: {}, name: 'openagents.get_capability_manifest' }))
    const body = (await res.json()) as {
      result: { structuredContent: { schemaVersion: string; service: { name: string } } }
    }
    expect(body.result.structuredContent.schemaVersion).toBe('openagents.capabilities.v1')
    expect(body.result.structuredContent.service.name).toBe('OpenAgents Autopilot')
  })

  test('tools/call with an unknown tool name is a tool-level error, not a transport error', async () => {
    const res = await run(rpc('tools/call', { arguments: {}, name: 'openagents.does_not_exist' }))
    expect(res.status).toBe(200)
    const body = (await res.json()) as { result: { isError: boolean; content: Array<{ text: string }> } }
    expect(body.result.isError).toBe(true)
    expect(body.result.content[0]?.text).toContain('Unknown tool')
  })

  test('resources/list surfaces the ui:// developer-resources resource', async () => {
    const res = await run(rpc('resources/list'))
    const body = (await res.json()) as { result: { resources: Array<{ uri: string; mimeType: string }> } }
    expect(body.result.resources).toEqual([
      { mimeType: 'text/html;profile=mcp-app', name: 'OpenAgents developer resources card', uri: 'ui://openagents/developer-resources' },
    ])
  })

  test('resources/read returns an MCP Apps HTML document for the ui:// resource', async () => {
    const res = await run(rpc('resources/read', { uri: 'ui://openagents/developer-resources' }))
    const body = (await res.json()) as { result: { contents: Array<{ mimeType: string; text: string }> } }
    expect(body.result.contents[0]?.mimeType).toBe('text/html;profile=mcp-app')
    expect(body.result.contents[0]?.text).toContain('<html>')
    expect(body.result.contents[0]?.text).toContain('https://openagents.com/AGENTS.md')
    expect(body.result.contents[0]?.text).toContain('name="color-scheme" content="dark light"')
    expect(body.result.contents[0]?.text).toContain('http-equiv="Content-Security-Policy"')
  })

  test('resources/read on an unknown uri is a JSON-RPC error', async () => {
    const res = await run(rpc('resources/read', { uri: 'ui://openagents/nope' }))
    const body = (await res.json()) as { error: { message: string } }
    expect(body.error.message).toContain('Unknown resource')
  })
})
