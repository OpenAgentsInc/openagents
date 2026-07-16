import { InternalLink } from '@/components/internal-link'
import { DOCS_URL, DOWNLOAD_URL, GITHUB_REPOSITORY_URL } from '@/lib/public-site'
import type { ReactNode } from 'react'

import GithubMark from './launch-ui/logos/github'
import '../public-header.css'

type PublicHeaderProps = Readonly<{
  docsActive?: boolean
  leading?: ReactNode
  utility?: ReactNode
  variant?: 'default' | 'docs'
}>

export function PublicHeader({ docsActive = false, leading, utility, variant = 'default' }: PublicHeaderProps = {}) {
  return (
    <>
      <header className={variant === 'docs' ? 'oa-unified-header oa-unified-header--docs' : 'oa-unified-header'}>
        <nav aria-label="Primary navigation" className="oa-unified-nav">
          <div className="oa-unified-nav-left">
            {leading}
            <InternalLink aria-label="OpenAgents home" className="oa-unified-brand" href="/" preload="render">
              OpenAgents
            </InternalLink>
            <InternalLink
              {...(docsActive ? { 'aria-current': 'page' as const } : {})}
              className="oa-unified-docs-link"
              href={DOCS_URL}
              preload="render"
            >
              Docs
            </InternalLink>
          </div>
          <div className="oa-unified-nav-actions">
            {utility}
            <a aria-label="OpenAgents on GitHub" className="oa-unified-github-link" href={GITHUB_REPOSITORY_URL} rel="noreferrer" target="_blank">
              <GithubMark aria-hidden="true" />
            </a>
            <InternalLink className="oa-unified-download-link" href={DOWNLOAD_URL} preload="render">
              Download
            </InternalLink>
          </div>
        </nav>
      </header>
      <div aria-hidden="true" className="oa-unified-header-spacer" />
    </>
  )
}
