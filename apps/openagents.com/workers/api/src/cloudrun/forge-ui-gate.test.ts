import { describe, expect, test, vi } from 'vitest'

import { gateForgeDocumentRequest } from './forge-ui-gate'

const serve = () =>
  Promise.resolve(
    new Response('<html>Forge</html>', {
      headers: { 'content-type': 'text/html' },
    }),
  )

describe('Forge document membership gate', () => {
  test('serves the admitted public repository without a member check', async () => {
    const checkMembership = vi.fn(
      async () => new Response(null, { status: 401 }),
    )
    const response = await gateForgeDocumentRequest(
      new Request(
        'https://openagents.com/forge/tenant.openagents/omega?view=tree&ref=refs%2Fheads%2Fmain',
      ),
      checkMembership,
      serve,
    )

    expect(response?.status).toBe(200)
    expect(await response?.text()).toContain('Forge')
    expect(checkMembership).not.toHaveBeenCalled()
  })

  test.each([
    '/forge/tenant.openagents/omega-private',
    '/forge/tenant.openagents/omega/settings',
    '/forge/tenant.openagents/other',
  ])('keeps the member gate for %s', async (pathname) => {
    const checkMembership = vi.fn(
      async () => new Response(null, { status: 401 }),
    )
    const response = await gateForgeDocumentRequest(
      new Request(`https://openagents.com${pathname}`),
      checkMembership,
      serve,
    )

    expect(response?.status).toBe(302)
    expect(checkMembership).toHaveBeenCalledOnce()
  })

  test('redirects a signed-out Forge visitor and preserves the return path', async () => {
    const response = await gateForgeDocumentRequest(
      new Request('https://openagents.com/forge/repositories?tab=mine'),
      async () => new Response(null, { status: 401 }),
      serve,
    )

    expect(response?.status).toBe(302)
    expect(response?.headers.get('location')).toBe(
      '/login?returnTo=%2Fforge%2Frepositories%3Ftab%3Dmine',
    )
    expect(response?.headers.get('cache-control')).toBe('no-store')
  })

  test('refuses an uninvited or revoked visitor before serving the shell', async () => {
    const serveDocument = vi.fn(serve)
    const response = await gateForgeDocumentRequest(
      new Request('https://openagents.com/forge'),
      async () => new Response(null, { status: 403 }),
      serveDocument,
    )

    expect(response?.status).toBe(403)
    expect(response?.headers.get('cache-control')).toBe('no-store')
    expect(serveDocument).not.toHaveBeenCalled()
  })

  test('serves an invited member and keeps a refreshed session cookie', async () => {
    const response = await gateForgeDocumentRequest(
      new Request('https://openagents.com/forge/project/core'),
      async (_request, tenantRef) =>
        new Response(null, {
          headers: { 'set-cookie': 'oa_session=renewed; Path=/; HttpOnly' },
          status: tenantRef === 'tenant.openagents' ? 204 : 403,
        }),
      serve,
    )

    expect(response?.status).toBe(200)
    expect(await response?.text()).toContain('Forge')
    expect(response?.headers.get('set-cookie')).toContain('oa_session=renewed')
  })

  test('does not gate a non-Forge document', async () => {
    const checkMembership = vi.fn(
      async () => new Response(null, { status: 403 }),
    )
    const response = await gateForgeDocumentRequest(
      new Request('https://openagents.com/forum'),
      checkMembership,
      serve,
    )

    expect(response?.status).toBe(200)
    expect(checkMembership).not.toHaveBeenCalled()
  })
})
