/**
 * Public-site homepage placeholder (owner request, 2026-07-06; scoped to `/`
 * only on 2026-07-07).
 *
 * The public web hosts (openagents.com / www.openagents.com) show a minimal
 * "be right back" placeholder at the ROOT PATH `/` ONLY. Every other route —
 * all other page routes (/forum, /settings, /agents/{ref}, ...), the API
 * surface, assets, well-known metadata, machine files, the auth issuer host,
 * and direct Cloud Run URLs — passes straight through to the normal handler.
 *
 * (Earlier this front controller also 302'd every non-`/` public page route to
 * the placeholder during the Cloudflare -> Google Cloud cutover; that blanket
 * redirect has been removed so the rest of the site works while the homepage
 * intentionally still shows the placeholder.)
 *
 * Remove this front controller + its wiring in `server.ts` to restore the real
 * homepage too.
 */

const PUBLIC_WEB_HOSTS = new Set(['openagents.com', 'www.openagents.com'])

const HOLDING_PAGE_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<meta name="robots" content="noindex" />
<title>OpenAgents</title>
<style>
  :root { color-scheme: dark; }
  html, body { height: 100%; margin: 0; }
  body {
    background: #05070d;
    color: #e8ecf4;
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    overflow: hidden;
  }
  /* Still capture of the live landing 3D glow-grid scene, behind a center
   * vignette that mutes the baked wordmark and keeps the blue bloom ambiance. */
  .bg {
    position: fixed;
    inset: 0;
    background-image:
      radial-gradient(circle at center, rgba(5,7,13,0.92) 0%, rgba(5,7,13,0.66) 55%, rgba(5,7,13,0.5) 100%),
      url('/holding-bg.jpg');
    background-size: cover;
    background-position: center;
    z-index: 0;
  }
  .wrap { position: relative; z-index: 1; padding: 24px; }
  h1 {
    margin: 0;
    font-size: clamp(40px, 9vw, 88px);
    font-weight: 650;
    letter-spacing: -0.02em;
    text-shadow: 0 2px 40px rgba(0,0,0,0.6);
  }
  p {
    margin: 14px 0 0;
    font-size: clamp(16px, 3.4vw, 22px);
    color: #9db4d8;
    letter-spacing: 0.01em;
    text-shadow: 0 1px 20px rgba(0,0,0,0.6);
  }
</style>
</head>
<body>
  <div class="bg"></div>
  <div class="wrap">
    <h1>OpenAgents</h1>
    <p>be right back</p>
  </div>
</body>
</html>
`

const holdingPageResponse = (): Response =>
  new Response(HOLDING_PAGE_HTML, {
    status: 200,
    headers: {
      'content-type': 'text/html; charset=utf-8',
      'cache-control': 'no-store',
    },
  })

/**
 * Serves the placeholder ONLY for the public web hosts' root path (`/`).
 * Returns `undefined` for every other request (host, path, API, asset, …) so
 * the normal handler runs — no other route is blocked or redirected.
 */
export const holdingPageInterception = (
  url: URL,
): Response | undefined => {
  if (!PUBLIC_WEB_HOSTS.has(url.hostname)) {
    return undefined
  }
  if (url.pathname === '/' || url.pathname === '') {
    return holdingPageResponse()
  }
  return undefined
}
