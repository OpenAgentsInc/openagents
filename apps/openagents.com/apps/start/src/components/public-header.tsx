import { InternalLink } from '@/components/internal-link'
import { DOCS_URL, GITHUB_REPOSITORY_URL } from '@/lib/public-site'

import GithubMark from './launch-ui/logos/github'
import '../public-header.css'

export function PublicHeader() {
  return (
    <header className="oa-unified-header">
      <nav aria-label="Primary navigation" className="oa-unified-nav">
        <div className="oa-unified-nav-left">
          <InternalLink aria-label="OpenAgents home" className="oa-unified-brand" href="/" preload="render">
            OpenAgents
          </InternalLink>
          <InternalLink className="oa-unified-docs-link" href={DOCS_URL} preload="render">
            Docs
          </InternalLink>
        </div>
        <div className="oa-unified-nav-actions">
          <InternalLink className="oa-unified-app-link" href="/app" preload="intent">
            Open app
          </InternalLink>
          <a aria-label="OpenAgents on GitHub" className="oa-unified-github-link" href={GITHUB_REPOSITORY_URL} rel="noreferrer" target="_blank">
            <GithubMark aria-hidden="true" />
          </a>
          <InternalLink className="oa-unified-download-link" href="/install" preload="render">
            Download
          </InternalLink>
        </div>
      </nav>
    </header>
  )
}
