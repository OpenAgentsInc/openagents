import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * PORTAL-1 (#8652): serve the /portal client-portal surface from the Cloud
 * Run monolith.
 *
 * The portal content is the React surface authored in
 * `apps/openagents.com/apps/start/src/routes/-portal-page.tsx` (state core in
 * `-portal-core.ts`). The monolith serves it exactly like the Sarah surface
 * (apps/sarah/src/server.ts): a static HTML shell at `/portal` plus a
 * deploy-built browser bundle at `/portal/app.js`
 * (deploy-cloudrun.sh -> dist-cloudrun/portal-ui/app.js). Dev builds the
 * bundle once from source on demand.
 *
 * The Worker API authority (/api/portal/*, portal-routes.ts) is untouched by
 * this file — this is presentation serving only.
 */
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const __dirname = fileURLToPath(new URL('.', import.meta.url))

/** Overridable for the Cloud Run monolith bundle (artifacts beside server.js). */
const PORTAL_UI_DIR =
  process.env.PORTAL_UI_DIR?.trim() || join(__dirname, 'portal-ui')

/** Source entry for the on-demand dev build (repo checkout layout). */
const PORTAL_ENTRY_SOURCE = join(
  __dirname,
  '../../../../apps/start/src/portal-entry.ts',
)

export const PORTAL_PAGE_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Client portal — OpenAgents</title>
<meta name="description" content="Your OpenAgents engagement at a glance: funnel status, content calendar, and approval queue. Private to your account." />
<style>
  /* The deploy step packs portal-entry.ts as JavaScript only (no Tailwind
     build reaches this standalone shell), so the surface's Tailwind/khala
     utility classes are inert here. These rules give the same markup a
     readable dark-surface baseline via the stable data-* hooks the portal
     component renders. The TanStack Start route loads the real stylesheet. */
  :root { color-scheme: dark; }
  html, body { min-height: 100%; margin: 0; }
  body {
    background: #05070d;
    color: #eef3ff;
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    line-height: 1.6;
  }
  [data-route="portal"] { display: block; min-height: 100vh; }
  [data-portal-root] {
    box-sizing: border-box;
    display: grid;
    gap: 1.5rem;
    margin: 0 auto;
    max-width: 64rem;
    padding: 3rem 1rem;
    width: 100%;
  }
  [data-portal-root] h1 { font-size: 1.25rem; }
  [data-portal-root] h2 { font-size: 1rem; }
  [data-portal-root] h3 { font-size: 1rem; }
  [data-portal-root] h1,
  [data-portal-root] h2,
  [data-portal-root] h3,
  [data-portal-root] p { margin: 0; }
  [data-portal-root] hr { border: 0; border-top: 1px solid #1f2b45; margin: 0; }
  [data-portal-root] section { display: grid; gap: 0.75rem; }
  [data-slot="card"] { border: 1px solid #1f2b45; background: #0b1220; }
  [data-portal-card] { display: grid; gap: 0.75rem; padding: 1.5rem; }
  [data-portal-item] { display: grid; gap: 0.5rem; padding: 1rem; }
  [data-portal-banner] {
    border: 1px solid #1f2b45;
    background: rgba(59, 130, 246, 0.1);
    padding: 0.5rem 0.75rem;
  }
  [data-portal-banner="warn"] {
    border-color: rgba(245, 158, 11, 0.4);
    background: rgba(245, 158, 11, 0.1);
  }
  [data-portal-banner="danger"] {
    border-color: rgba(248, 113, 113, 0.4);
    background: rgba(248, 113, 113, 0.1);
  }
  [data-portal-kpis], [data-portal-pair] {
    display: grid;
    gap: 0.75rem;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
  }
  [data-portal-kpi] { border: 1px solid #1f2b45; background: #0b1220; padding: 1rem; }
  [data-slot="badge"] {
    align-items: center;
    border: 1px solid #1f2b45;
    color: #8fb3ff;
    display: inline-flex;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.75rem;
    font-weight: 600;
    padding: 0.125rem 0.5rem;
    text-transform: uppercase;
  }
  [data-slot="button"], [data-portal-link] {
    align-items: center;
    background: transparent;
    border: 1px solid #1f2b45;
    color: #eef3ff;
    cursor: pointer;
    display: inline-flex;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.875rem;
    justify-content: center;
    min-height: 2.5rem;
    padding: 0.5rem 1rem;
    text-decoration: none;
    width: fit-content;
  }
  [data-portal-link="login"] { background: #eef3ff; border-color: #eef3ff; color: #05070d; }
  [data-portal-root] :focus-visible { outline: 2px solid #3b82f6; outline-offset: 2px; }
</style>
</head>
<body>
<div id="portal-root"></div>
<script type="module" src="/portal/app.js"></script>
</body>
</html>
`

// Production serves the deploy-built artifact from PORTAL_UI_DIR; dev builds
// once from source on demand (the Sarah serveAppBundle pattern).
let portalBundlePromise: Promise<string | null> | null = null

const servePortalBundle = async (): Promise<Response | null> => {
  const built = Runtime.file(join(PORTAL_UI_DIR, 'app.js'))
  if (await built.exists()) {
    return new Response(built.stream(), {
      headers: { 'content-type': 'application/javascript; charset=utf-8' },
    })
  }
  portalBundlePromise ??= (async () => {
    if (!(await Runtime.file(PORTAL_ENTRY_SOURCE).exists())) return null
    const result = await Runtime.build({
      entrypoints: [PORTAL_ENTRY_SOURCE],
      target: 'browser',
      minify: false,
    })
    if (!result.success || result.outputs.length === 0) {
      console.error('[portal] app bundle build failed', result.logs)
      return null
    }
    return await result.outputs[0]!.text()
  })()
  const code = await portalBundlePromise
  if (code === null) return null
  return new Response(code, {
    headers: { 'content-type': 'application/javascript; charset=utf-8' },
  })
}

/** Returns undefined for paths this surface does not own (API paths under
 * /api/portal/* are the Worker's — never intercepted here). */
export const handlePortalUiRequest = async (
  request: Request,
): Promise<Response | undefined> => {
  const url = new URL(request.url)
  const path = url.pathname
  if (path !== '/portal' && !path.startsWith('/portal/')) {
    return undefined
  }
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return new Response(JSON.stringify({ error: 'method_not_allowed' }), {
      status: 405,
      headers: {
        allow: 'GET, HEAD',
        'content-type': 'application/json',
      },
    })
  }
  if (path === '/portal' || path === '/portal/') {
    return new Response(PORTAL_PAGE_HTML, {
      headers: {
        'cache-control': 'no-store',
        'content-type': 'text/html; charset=utf-8',
      },
    })
  }
  if (path === '/portal/app.js') {
    const bundle = await servePortalBundle()
    if (bundle !== null) return bundle
    return new Response(
      JSON.stringify({ error: 'portal_bundle_unavailable' }),
      {
        status: 500,
        headers: { 'content-type': 'application/json' },
      },
    )
  }
  return new Response(JSON.stringify({ error: 'not_found' }), {
    status: 404,
    headers: { 'content-type': 'application/json' },
  })
}
