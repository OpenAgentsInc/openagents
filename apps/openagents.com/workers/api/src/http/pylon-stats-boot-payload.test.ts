import { describe, expect, test } from 'vitest'

import {
  PYLON_STATS_BOOT_SCRIPT_ID,
  injectPylonStatsBootPayload,
  injectPylonStatsBootPayloadIntoAssetResponse,
} from './pylon-stats-boot-payload'

describe('pylon stats boot payload', () => {
  test('injects public stats JSON before the app shell closes', async () => {
    const response = await injectPylonStatsBootPayloadIntoAssetResponse(
      new Request('https://openagents.com/pylons', {
        headers: { accept: 'text/html' },
      }),
      {},
      new Response('<!doctype html><html><body><div id="root"></div></body></html>', {
        headers: { 'content-type': 'text/html; charset=utf-8' },
      }),
      async () =>
        JSON.stringify({
          available: true,
          publicRealSatsSettled24h: 150_000,
          pylonsOnlineNow: 4,
          trainingModelProgressContributors: 3,
        }),
    )

    const html = await response.text()

    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(html).toContain(`id="${PYLON_STATS_BOOT_SCRIPT_ID}"`)
    expect(html).toContain('"publicRealSatsSettled24h":150000')
    expect(html.indexOf(PYLON_STATS_BOOT_SCRIPT_ID)).toBeLessThan(
      html.indexOf('</body>'),
    )
  })

  test('does not inject into the landing homepage at / (kept lean + cacheable)', async () => {
    const appShell = '<!doctype html><div id="root"></div>'
    const response = await injectPylonStatsBootPayloadIntoAssetResponse(
      new Request('https://openagents.com/', {
        headers: { accept: 'text/html' },
      }),
      {},
      new Response(appShell, {
        headers: { 'content-type': 'text/html; charset=utf-8' },
      }),
      async () => JSON.stringify({ available: true, pylonsOnlineNow: 4 }),
    )

    // / is the landing 3D scene now — it must stay a small, cacheable shell with
    // no injected pylon-stats payload and no no-store header.
    expect(await response.text()).toBe(appShell)
    expect(response.headers.get('cache-control')).not.toBe('no-store')
  })

  test('does not inject into unrelated document routes', async () => {
    const appShell = '<!doctype html><div id="root"></div>'
    const response = await injectPylonStatsBootPayloadIntoAssetResponse(
      new Request('https://openagents.com/stats', {
        headers: { accept: 'text/html' },
      }),
      {},
      new Response(appShell, {
        headers: { 'content-type': 'text/html; charset=utf-8' },
      }),
      async () => JSON.stringify({ available: true, pylonsOnlineNow: 4 }),
    )

    expect(await response.text()).toBe(appShell)
  })

  test('escapes JSON so a snapshot cannot close the script tag', () => {
    const html = injectPylonStatsBootPayload(
      '<!doctype html><body></body>',
      JSON.stringify({ status: '</script><script>alert(1)</script>' }),
    )

    expect(html).toContain('\\u003c/script>')
    expect(html).not.toContain('</script><script>alert')
  })
})
