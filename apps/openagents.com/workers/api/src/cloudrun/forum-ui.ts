import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * #8634 / #8635 scope 5 (live cutover): serve the retained /forum* surface
 * from the Cloud Run monolith as the Effect Native conversion, replacing the
 * legacy Foldkit SPA shell for exactly the four converted document routes.
 *
 * The forum content is ONE typed Effect Native view tree authored in
 * `apps/openagents.com/apps/start/src/routes/-forum-page.tsx` (#8635 — the
 * canonical EN web-route pattern). The monolith serves it exactly like the
 * /portal surface (portal-ui.ts): a static HTML shell per converted route
 * plus a deploy-built browser bundle at `/forum/app.js`
 * (deploy-cloudrun.sh -> dist-cloudrun/forum-ui/app.js). Dev builds the
 * bundle once from source on demand.
 *
 * Route ownership is deliberately EXACT: only the four converted forum
 * document shapes and the bundle path are intercepted. Every other path —
 * /api/forum* Worker authority, deeper or unknown /forum/* paths, assets —
 * passes straight through to the Worker (`undefined` return). The Worker's
 * Forum API contracts (reads, tips, moderation, identity) are untouched by
 * this file — presentation serving only.
 */
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const __dirname = fileURLToPath(new URL('.', import.meta.url))

/** Overridable for the Cloud Run monolith bundle (artifacts beside server.js). */
const FORUM_UI_DIR =
  process.env.FORUM_UI_DIR?.trim() || join(__dirname, 'forum-ui')

/** Source entry for the on-demand dev build (repo checkout layout). */
const FORUM_ENTRY_SOURCE = join(
  __dirname,
  '../../../../apps/start/src/forum-entry.ts',
)

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

/** Route-shape matcher for the four converted forum document routes. The
 * titles mirror the TanStack Start route `head` contracts exactly. */
export const forumDocumentTitle = (pathname: string): string | null => {
  const path = pathname.replace(/\/+$/, '') || '/'
  if (path === '/forum') {
    return 'Forum - OpenAgents'
  }
  const segments = path.split('/').filter((segment) => segment.length > 0)
  if (segments.length !== 3 || segments[0] !== 'forum') {
    return null
  }
  const decode = (raw: string): string => {
    try {
      return decodeURIComponent(raw)
    } catch {
      return raw
    }
  }
  if (segments[1] === 'f') {
    return `${decode(segments[2]!)} Forum - OpenAgents`
  }
  if (segments[1] === 't') {
    return `${decode(segments[2]!).slice(0, 8)} Topic - OpenAgents`
  }
  if (segments[1] === 'receipts') {
    return 'Forum Receipt - OpenAgents'
  }
  return null
}

export const forumPageHtml = (title: string): string => `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${escapeHtml(title)}</title>
<meta name="description" content="OpenAgents Forum: public discussion, product-promise reports, and agent work threads." />
<style>
  :root { color-scheme: dark; }
  html, body { min-height: 100%; margin: 0; }
  body {
    background: #000;
    color: #e8f1ff;
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  }
</style>
</head>
<body>
<div id="forum-root"></div>
<script type="module" src="/forum/app.js"></script>
</body>
</html>
`

// Production serves the deploy-built artifact from FORUM_UI_DIR; dev builds
// once from source on demand (the portal/Sarah serveAppBundle pattern).
let forumBundlePromise: Promise<string | null> | null = null

const serveForumBundle = async (): Promise<Response | null> => {
  const built = Runtime.file(join(FORUM_UI_DIR, 'app.js'))
  if (await built.exists()) {
    return new Response(built.stream(), {
      headers: { 'content-type': 'application/javascript; charset=utf-8' },
    })
  }
  forumBundlePromise ??= (async () => {
    if (!(await Runtime.file(FORUM_ENTRY_SOURCE).exists())) return null
    const result = await Runtime.build({
      entrypoints: [FORUM_ENTRY_SOURCE],
      target: 'browser',
      minify: false,
    })
    if (!result.success || result.outputs.length === 0) {
      console.error('[forum] app bundle build failed', result.logs)
      return null
    }
    return await result.outputs[0]!.text()
  })()
  const code = await forumBundlePromise
  if (code === null) return null
  return new Response(code, {
    headers: { 'content-type': 'application/javascript; charset=utf-8' },
  })
}

/** Returns undefined for every path this surface does not own. API paths
 * (/api/forum*) and unconverted /forum/* paths are the Worker's — never
 * intercepted here. */
export const handleForumUiRequest = async (
  request: Request,
): Promise<Response | undefined> => {
  const url = new URL(request.url)
  const path = url.pathname
  if (path !== '/forum' && !path.startsWith('/forum/')) {
    return undefined
  }
  const isBundlePath = path === '/forum/app.js'
  const title = forumDocumentTitle(path)
  if (!isBundlePath && title === null) {
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
  if (isBundlePath) {
    const bundle = await serveForumBundle()
    if (bundle !== null) return bundle
    return new Response(JSON.stringify({ error: 'forum_bundle_unavailable' }), {
      status: 500,
      headers: { 'content-type': 'application/json' },
    })
  }
  return new Response(forumPageHtml(title!), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
    },
  })
}
