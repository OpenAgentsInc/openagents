import { InternalLink } from '@/components/internal-link'
import {
  DISCORD_URL,
  DOCS_URL,
  DOWNLOAD_URL,
  GITHUB_REPOSITORY_URL,
  STACKER_NEWS_URL,
  X_URL,
} from '@/lib/public-site'

import '../public-footer.css'

export function PublicFooter() {
  return (
    <footer className="oa-public-footer">
      <div className="oa-public-footer-main">
        <div className="oa-public-footer-brand">
          <InternalLink href="/" preload="render">
            OpenAgents
          </InternalLink>
          <p>A local-first workroom for durable, reviewable Codex work.</p>
        </div>
        <nav aria-label="Product links">
          <strong>Product</strong>
          <InternalLink href={DOCS_URL} preload="render">
            Docs
          </InternalLink>
          <InternalLink href="/blog" preload="render">
            Blog
          </InternalLink>
          <InternalLink href={DOWNLOAD_URL} preload="render">
            Download
          </InternalLink>
        </nav>
        <nav aria-label="Community links">
          <strong>Community</strong>
          <a href={GITHUB_REPOSITORY_URL} rel="noreferrer" target="_blank">
            GitHub
          </a>
          <a href={X_URL} rel="noreferrer" target="_blank">
            X
          </a>
          <a href={DISCORD_URL}>Discord</a>
          <a href={STACKER_NEWS_URL} rel="noreferrer" target="_blank">
            Stacker News
          </a>
        </nav>
        <nav aria-label="Legal links">
          <strong>Legal</strong>
          <InternalLink href="/privacy" preload="render">
            Privacy
          </InternalLink>
          <InternalLink href="/terms" preload="render">
            Terms
          </InternalLink>
        </nav>
      </div>
      <div className="oa-public-footer-bottom">
        <span>© 2026 OpenAgents, Inc.</span>
        <span>Open source · local first · evidence backed</span>
      </div>
    </footer>
  )
}
