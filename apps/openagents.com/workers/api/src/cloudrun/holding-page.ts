/**
 * Temporary public-site holding page (owner request, 2026-07-06).
 *
 * While the openagents.com public product surface is intentionally offline
 * during the Cloudflare -> Google Cloud cutover, the public site shows a
 * minimal "be right back" page and every other public PAGE route redirects
 * to it. Machine/API/asset surfaces are untouched so the mobile app, Khala
 * Sync, the OpenAuth issuer (separate host), and agent onboarding keep
 * working.
 *
 * Scope: applies ONLY to the public web hosts (openagents.com / www). The
 * auth issuer host (auth.openagents.com) and direct Cloud Run URLs are never
 * affected here. Remove this front controller to restore the full site.
 */

const PUBLIC_WEB_HOSTS = new Set(['openagents.com', 'www.openagents.com'])

/** Paths that must keep working even while the site is in holding mode:
 * the whole API surface, well-known metadata, static assets (anything with a
 * file extension, e.g. .js/.css/.png/.ico/.map/.woff2), and the machine-facing
 * agent/onboarding files. */
const MACHINE_FILES = new Set([
  '/AGENTS.md',
  '/HEARTBEAT.md',
  '/RULES.md',
  '/skill.json',
  '/robots.txt',
  '/sitemap.xml',
  '/favicon.ico',
])

const hasFileExtension = (pathname: string): boolean =>
  /\.[a-z0-9]+$/i.test(pathname)

const isPassThroughPath = (pathname: string): boolean =>
  pathname.startsWith('/api/') ||
  pathname.startsWith('/.well-known/') ||
  pathname.startsWith('/_') ||
  hasFileExtension(pathname) ||
  MACHINE_FILES.has(pathname)

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
  }
  .wrap { padding: 24px; }
  h1 {
    margin: 0;
    font-size: clamp(40px, 9vw, 88px);
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  p {
    margin: 14px 0 0;
    font-size: clamp(16px, 3.4vw, 22px);
    color: #7f94b5;
    letter-spacing: 0.01em;
  }
</style>
</head>
<body>
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
 * If the request is for the public web site and should be held, returns the
 * holding-page response (for `/`) or a 302 redirect to `/` (for any other
 * page route). Returns `undefined` to let the normal handler run (API,
 * assets, well-known, machine files, the auth host, direct Cloud Run URLs).
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
  if (isPassThroughPath(url.pathname)) {
    return undefined
  }
  return Response.redirect('https://openagents.com/', 302)
}
