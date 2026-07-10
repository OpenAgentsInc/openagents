// PORTAL-1 (#8652): /portal monolith serving shell.
import { describe, expect, test } from 'vitest'

import { PORTAL_PAGE_HTML, handlePortalUiRequest } from './portal-ui'

describe('portal ui serving (PORTAL-1 #8652)', () => {
  test('does not own non-portal paths', async () => {
    for (const path of ['/', '/sarah', '/api/portal/engagement', '/portals']) {
      expect(
        await handlePortalUiRequest(
          new Request(`https://openagents.com${path}`),
        ),
      ).toBeUndefined()
    }
  })

  test('serves the portal HTML shell at /portal', async () => {
    const response = await handlePortalUiRequest(
      new Request('https://openagents.com/portal'),
    )
    expect(response?.status).toBe(200)
    expect(response?.headers.get('content-type')).toContain('text/html')
    const html = await response!.text()
    expect(html).toContain('id="portal-root"')
    expect(html).toContain('/portal/app.js')
    expect(html).toBe(PORTAL_PAGE_HTML)
  })

  test('rejects non-GET methods', async () => {
    const response = await handlePortalUiRequest(
      new Request('https://openagents.com/portal', { method: 'POST' }),
    )
    expect(response?.status).toBe(405)
    expect(response?.headers.get('allow')).toBe('GET, HEAD')
  })

  test('unknown portal asset paths answer 404', async () => {
    const response = await handlePortalUiRequest(
      new Request('https://openagents.com/portal/nope.js'),
    )
    expect(response?.status).toBe(404)
  })
})
