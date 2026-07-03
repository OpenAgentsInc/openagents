/**
 * Crawl-discovery surfaces — `GET /robots.txt` and `GET /sitemap.xml` (orank
 * agent-readiness gap: "Developer resource discoverability"). Neither path
 * was previously registered, so both fell through to the SPA shell and
 * served the homepage HTML instead of a crawl directive / URL index. Search
 * engines and agent crawlers use these to find the developer-facing surfaces
 * (`AGENTS.md`, `llms.txt`, the OpenAPI spec, the well-known manifests) that
 * are otherwise only reachable by already knowing the exact path.
 */
import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'

type HttpResponse = globalThis.Response

const ORIGIN = 'https://openagents.com'

// Public developer- and agent-facing pages worth indexing. Kept small and
// honest: only real, stable, publicly reachable paths.
const SITEMAP_PATHS: ReadonlyArray<string> = [
  '/',
  '/AGENTS.md',
  '/llms.txt',
  '/agents.md',
  '/ai.md',
  '/skill.md',
  '/docs/api',
  '/docs/product-promises',
  '/forum',
  '/forum/f/product-promises',
  '/api/openapi.json',
  '/.well-known/openagents.json',
  '/.well-known/mcp.json',
  '/.well-known/ai-catalog.json',
]

const robotsTxt = (): string => `User-agent: *
Allow: /

Sitemap: ${ORIGIN}/sitemap.xml
`

const sitemapXml = (): string => {
  const urls = SITEMAP_PATHS.map(
    path => `  <url><loc>${ORIGIN}${path}</loc></url>`,
  ).join('\n')
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`
}

const renderTextSurface = (
  request: Request,
  body: string,
  contentType: string,
): HttpResponse => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return methodNotAllowed(['GET', 'HEAD'])
  }
  const headers = new Headers({
    'cache-control': 'public, max-age=300',
    'content-type': contentType,
  })
  return new Response(request.method === 'HEAD' ? null : body, { headers, status: 200 })
}

export const routeSiteCrawlSurfaceRequest = (
  request: Request,
  _env?: unknown,
  _ctx?: ExecutionContext,
): Effect.Effect<HttpResponse> | undefined => {
  const path = new URL(request.url).pathname
  if (path === '/robots.txt') {
    return Effect.sync(() => renderTextSurface(request, robotsTxt(), 'text/plain; charset=utf-8'))
  }
  if (path === '/sitemap.xml') {
    return Effect.sync(() =>
      renderTextSurface(request, sitemapXml(), 'application/xml; charset=utf-8'),
    )
  }
  return undefined
}
