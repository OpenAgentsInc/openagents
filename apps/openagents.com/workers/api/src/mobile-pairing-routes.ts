/**
 * iOS Universal Link surfaces for OpenAgents mobile pairing.
 *
 * The Omega desktop shows a pairing QR that encodes
 * `https://openagents.com/pair#<base64url pairing bootstrap JSON>`. When iOS
 * Camera scans it, iOS resolves this domain's Apple App Site Association
 * (AASA) document and opens the OpenAgents app directly. When the app is not
 * installed, the browser lands on `/pair` and must see a short helpful page.
 *
 *   GET /.well-known/apple-app-site-association — AASA document binding the
 *                                                 `/pair` path to the app
 *   GET /apple-app-site-association             — legacy alias Apple's CDN
 *                                                 may still probe; same body
 *   GET /pair                                   — fallback HTML page for
 *                                                 browsers without the app
 *
 * The pairing payload lives in the URL FRAGMENT. Browsers never send the
 * fragment to the server, and this module must keep it that way: no handler
 * here reads, logs, stores, or echoes pairing material, and the `/pair` page
 * ships no script that transmits `location.hash` anywhere.
 */
import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'

type HttpResponse = globalThis.Response

/** Apple Team HQWSG26L43 + iOS bundle identifier com.openagents.app. */
export const MOBILE_PAIRING_APP_ID = 'HQWSG26L43.com.openagents.app'

export const APPLE_APP_SITE_ASSOCIATION_PATHS = [
  '/.well-known/apple-app-site-association',
  '/apple-app-site-association',
] as const

export const MOBILE_PAIRING_PAGE_PATH = '/pair'

// Modern `appIDs` + `components` AASA shape. `apps` must be present and empty
// at the applinks top level per Apple's format; `webcredentials` lets the app
// use associated-domain credentials for openagents.com.
const appleAppSiteAssociationDocument = () => ({
  applinks: {
    apps: [],
    details: [
      {
        appIDs: [MOBILE_PAIRING_APP_ID],
        components: [
          {
            '/': MOBILE_PAIRING_PAGE_PATH,
            comment: 'OpenAgents mobile pairing link',
          },
        ],
      },
    ],
  },
  webcredentials: {
    apps: [MOBILE_PAIRING_APP_ID],
  },
})

// The only script on this page checks `location.hash` LOCALLY to toggle the
// "pairing code detected" hint. It must never serialize, store, or transmit
// the fragment — the pairing payload stays on the device.
const pairingFallbackPageHtml = () =>
  `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="robots" content="noindex"><title>Pair your Omega desktop</title></head><body style="margin:0;background:#050505;color:#f1efe8;font-family:ui-sans-serif,system-ui,sans-serif;display:grid;min-height:100vh;place-items:center"><main style="width:min(420px,calc(100vw - 48px));border:1px solid #2b2a26;padding:28px;background:#0b0b0a"><p style="margin:0 0 10px;color:#8b8880;font-size:12px;text-transform:uppercase;letter-spacing:.12em">OpenAgents</p><h1 style="margin:0 0 12px;font-size:22px">Pair your Omega desktop</h1><p id="pair-hint" hidden style="margin:0 0 12px;color:#d6f6ff;line-height:1.55">Pairing code detected. It stays on this device.</p><p style="margin:0 0 12px;color:#c9c6bd;line-height:1.55">This link is meant to open the OpenAgents app on your phone. If the app is installed, opening this link on your phone opens the app and completes pairing.</p><p style="margin:0;color:#c9c6bd;line-height:1.55">If the app is not installed, install the OpenAgents app (TestFlight), then scan the desktop QR code again from inside the app or re-open this link.</p></main><script>if(location.hash.length>1){var h=document.getElementById("pair-hint");if(h)h.hidden=false}</script></body></html>`

// Public + cacheable like the sibling well-known discovery surfaces
// (`well-known-agent-surfaces-routes.ts`): Apple's AASA CDN and browsers can
// fetch these cheaply, and neither response varies by requester.
const renderGetOrHead = (
  request: Request,
  body: string,
  contentType: string,
): Effect.Effect<HttpResponse> =>
  Effect.sync(() => {
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      return methodNotAllowed(['GET', 'HEAD'])
    }
    const headers = new Headers({
      'cache-control': 'public, max-age=300',
      'content-type': contentType,
    })
    return new Response(request.method === 'HEAD' ? null : body, {
      headers,
      status: 200,
    })
  })

export const routeMobilePairingRequest = (
  request: Request,
  _env?: unknown,
  _ctx?: ExecutionContext,
): Effect.Effect<HttpResponse> | undefined => {
  const path = new URL(request.url).pathname
  if ((APPLE_APP_SITE_ASSOCIATION_PATHS as ReadonlyArray<string>).includes(path)) {
    return renderGetOrHead(
      request,
      JSON.stringify(appleAppSiteAssociationDocument(), null, 2),
      'application/json',
    )
  }
  if (path === MOBILE_PAIRING_PAGE_PATH) {
    return renderGetOrHead(
      request,
      pairingFallbackPageHtml(),
      'text/html; charset=utf-8',
    )
  }
  return undefined
}
