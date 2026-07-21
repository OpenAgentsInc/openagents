/**
 * /aisdk/docs reader (owner-directed addition, 2026-07-21). Renders the
 * build-time-compiled Markdown from the repository `docs/ai-sdk/` tree.
 * Same presentation model as the /docs reader: generated, escaped HTML with
 * no raw-HTML passthrough, injected into a styled article.
 */
import { InternalLink } from '@/components/internal-link'

import type { AisdkDocsPage } from '../aisdk/aisdk-content'
import { aisdkDocsManifest } from '../aisdk/generated/aisdk-manifest.generated'
import { AISDK_GITHUB_URL } from './-aisdk-page'
import { PublicSiteShell } from './-public-site'
import './-aisdk.css'

export function AisdkDocsPageView({ page }: Readonly<{ page: AisdkDocsPage }>) {
  return (
    <div data-route="aisdk-docs">
      <PublicSiteShell>
        <div className="oa-aisdk-docs-page">
          <div className="oa-aisdk-docs-shell">
            <nav aria-label="AI SDK documentation" className="oa-aisdk-docs-nav">
              <p className="oa-aisdk-docs-nav-title">
                <InternalLink href="/aisdk" preload="render">AI SDK</InternalLink>
                {' '}/ docs
              </p>
              {aisdkDocsManifest.map(entry => (
                <InternalLink
                  aria-current={entry.slug === page.slug ? 'page' : undefined}
                  href={entry.path}
                  key={entry.path}
                  preload="render"
                >
                  {entry.sidebarLabel}
                </InternalLink>
              ))}
            </nav>
            <article className="oa-aisdk-doc">
              <h1>{page.title}</h1>
              {/* Compiled at build time from docs/ai-sdk by
                  scripts/generate-aisdk-docs.ts — escaped output, raw HTML
                  rejected at compile time. */}
              <section dangerouslySetInnerHTML={{ __html: page.html }} />
              <footer className="oa-aisdk-doc-footer">
                <p>
                  Source:{' '}
                  <a href={page.editUrl} rel="noreferrer" target="_blank">docs/ai-sdk in the OpenAgents repository</a>
                  {' · '}
                  <a href={AISDK_GITHUB_URL} rel="noreferrer" target="_blank">SDK repository</a>
                </p>
              </footer>
            </article>
          </div>
        </div>
      </PublicSiteShell>
    </div>
  )
}

export function AisdkDocsNotFound() {
  return (
    <div data-route="aisdk-docs-not-found">
      <PublicSiteShell>
        <div className="oa-container oa-aisdk-notfound">
          <h1>Page not found</h1>
          <p>
            That AI SDK document does not exist. Start from the{' '}
            <InternalLink href="/aisdk/docs" preload="render">documentation overview</InternalLink>
            {' '}or the <InternalLink href="/aisdk" preload="render">AI SDK page</InternalLink>.
          </p>
        </div>
      </PublicSiteShell>
    </div>
  )
}
