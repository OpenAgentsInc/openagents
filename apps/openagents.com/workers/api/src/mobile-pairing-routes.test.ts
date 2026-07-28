import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  APPLE_APP_SITE_ASSOCIATION_PATHS,
  MOBILE_PAIRING_APP_ID,
  MOBILE_PAIRING_PAGE_PATH,
  routeMobilePairingRequest,
} from './mobile-pairing-routes'

const run = (path: string, init?: RequestInit): Promise<Response> => {
  const request = new Request(`https://openagents.com${path}`, init)
  const effect = routeMobilePairingRequest(request)
  if (effect === undefined) throw new Error(`route did not match: ${path}`)
  return Effect.runPromise(effect)
}

describe('mobile pairing universal-link surfaces', () => {
  test('other paths pass through', () => {
    expect(
      routeMobilePairingRequest(new Request('https://openagents.com/pairing')),
    ).toBeUndefined()
    expect(
      routeMobilePairingRequest(new Request('https://openagents.com/pair/extra')),
    ).toBeUndefined()
    expect(
      routeMobilePairingRequest(
        new Request('https://openagents.com/.well-known/mcp.json'),
      ),
    ).toBeUndefined()
  })

  for (const path of APPLE_APP_SITE_ASSOCIATION_PATHS) {
    test(`GET ${path} serves the AASA binding /pair to the OpenAgents app`, async () => {
      const res = await run(path)
      expect(res.status).toBe(200)
      expect(res.headers.get('content-type')).toContain('application/json')
      const doc = (await res.json()) as {
        applinks: {
          apps: ReadonlyArray<never>
          details: Array<{
            appIDs: ReadonlyArray<string>
            components: Array<{ '/': string }>
          }>
        }
        webcredentials: { apps: ReadonlyArray<string> }
      }
      expect(doc.applinks.apps).toEqual([])
      expect(doc.applinks.details).toHaveLength(1)
      expect(doc.applinks.details[0]?.appIDs).toEqual([
        'HQWSG26L43.com.openagents.app',
      ])
      expect(
        doc.applinks.details[0]?.components.map(component => component['/']),
      ).toContain('/pair')
      expect(doc.webcredentials.apps).toEqual([MOBILE_PAIRING_APP_ID])
    })
  }

  test('GET /pair serves the fallback HTML page with app guidance', async () => {
    const res = await run(MOBILE_PAIRING_PAGE_PATH)
    expect(res.status).toBe(200)
    expect(res.headers.get('content-type')).toContain('text/html')
    const html = await res.text()
    expect(html).toContain('Pair your Omega desktop')
    expect(html).toContain('OpenAgents app')
    expect(html).toContain('TestFlight')
    expect(html).toContain('scan the desktop QR code again')
  })

  test('/pair never echoes or transmits the URL fragment', async () => {
    // Browsers never send the fragment, but guard the served page anyway: it
    // must not reference the fragment except for the bounded local hash check,
    // and must not ship any network-capable script.
    const res = await run(`${MOBILE_PAIRING_PAGE_PATH}`)
    const html = await res.text()
    // The only allowed hash usage is the local show/hide hint check.
    const hashReferences = html.match(/location\.hash/g) ?? []
    expect(hashReferences.length).toBeLessThanOrEqual(1)
    for (const forbidden of [
      'fetch(',
      'XMLHttpRequest',
      'navigator.sendBeacon',
      'WebSocket',
      'window.location =',
      'location.href =',
      'localStorage',
      'sessionStorage',
      'document.cookie',
    ]) {
      expect(html).not.toContain(forbidden)
    }
    // No external scripts and no form posts that could carry pairing material.
    expect(html).not.toContain('<script src')
    expect(html).not.toContain('<form')
  })

  test('HEAD is allowed and POST is method-not-allowed on both surfaces', async () => {
    const headRes = await run(MOBILE_PAIRING_PAGE_PATH, { method: 'HEAD' })
    expect(headRes.status).toBe(200)
    const pairPost = await run(MOBILE_PAIRING_PAGE_PATH, { method: 'POST' })
    expect(pairPost.status).toBe(405)
    const aasaPost = await run(APPLE_APP_SITE_ASSOCIATION_PATHS[0], {
      method: 'POST',
    })
    expect(aasaPost.status).toBe(405)
  })
})
