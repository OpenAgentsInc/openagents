import { describe, expect, test } from 'vitest'

import { routeSharedAgentSurface } from './server'

const request = (path: string, init?: RequestInit): Request =>
  new Request(`https://openagents.com${path}`, init)

describe('Start shared agent surfaces', () => {
  test('serves the API Worker MCP manifest source', async () => {
    const response = await routeSharedAgentSurface(
      request('/.well-known/mcp.json'),
    )

    expect(response?.status).toBe(200)
    expect(response?.headers.get('content-type')).toContain('application/json')
    const body = (await response?.json()) as {
      name: string
      transport: string
      url: string
      tools: ReadonlyArray<{ name: string }>
    }
    expect(body.name).toBe('openagents')
    expect(body.transport).toBe('streamable-http')
    expect(body.url).toBe('https://openagents.com/api/agent-mcp')
    expect(body.tools.map(tool => tool.name)).toContain(
      'openagents.get_developer_resources',
    )
  })

  test('serves the API Worker ARD catalog source', async () => {
    const response = await routeSharedAgentSurface(
      request('/.well-known/ai-catalog.json'),
    )

    expect(response?.status).toBe(200)
    const body = (await response?.json()) as {
      host: { displayName: string }
      specVersion: string
    }
    expect(body.specVersion).toBe('1.0')
    expect(body.host.displayName).toBe('OpenAgents')
  })

  test('serves robots, sitemap, and llms from the shared Worker helpers', async () => {
    const robots = await routeSharedAgentSurface(request('/robots.txt'))
    const sitemap = await routeSharedAgentSurface(request('/sitemap.xml'))
    const llms = await routeSharedAgentSurface(request('/llms.txt'))

    expect(await robots?.text()).toContain('Sitemap: https://openagents.com/sitemap.xml')
    expect(await sitemap?.text()).toContain(
      '<loc>https://openagents.com/.well-known/mcp.json</loc>',
    )
    expect(await llms?.text()).toContain('openagents/khala')
  })

  test('passes through normal Start routes', async () => {
    await expect(routeSharedAgentSurface(request('/business'))).resolves.toBeUndefined()
  })
})
