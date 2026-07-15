import { ArrowLeft, ArrowRight, Check, Clipboard, ExternalLink, RotateCcw } from 'lucide-react'
import { useState, type MouseEvent as ReactMouseEvent } from 'react'

import type { DocsPage as DocsPageModel } from './content-schema'

export function DocsPage({ page }: Readonly<{ page: DocsPageModel }>) {
  const [markdownCopyStatus, setMarkdownCopyStatus] = useState<'idle' | 'copied' | 'failed'>('idle')

  const copyMarkdown = async () => {
    try {
      const response = await fetch(page.rawMarkdownUrl, {
        headers: { accept: 'text/markdown' },
      })
      if (!response.ok) {
        throw new Error(`Raw Markdown returned ${response.status}`)
      }
      await navigator.clipboard.writeText(await response.text())
      setMarkdownCopyStatus('copied')
    } catch {
      setMarkdownCopyStatus('failed')
    }
  }

  const handleArticleClick = (event: ReactMouseEvent<HTMLElement>) => {
    const target = event.target
    if (!(target instanceof Element)) {
      return
    }

    const copyButton = target.closest<HTMLButtonElement>('[data-docs-copy-code]')
    if (copyButton !== null) {
      const code = copyButton.closest('.docs-code')?.querySelector('code')?.textContent
      if (code !== undefined && code !== null) {
        void navigator.clipboard.writeText(code).then(() => {
          copyButton.textContent = 'Copied'
          copyButton.setAttribute('aria-label', 'Code copied')
        }).catch(() => {
          copyButton.textContent = 'Copy failed'
        })
      }
      return
    }

  }

  return (
    <div className="docs-page-grid">
      <article className="docs-article">
        <nav aria-label="Breadcrumb" className="docs-breadcrumb">
          {page.group}
        </nav>
        <header>
          <h1>{page.title}</h1>
          <p className="docs-description">{page.description}</p>
          <div className="docs-page-actions">
            <a className="docs-action-link" href={page.editUrl} rel="noreferrer" target="_blank">
              <ExternalLink aria-hidden="true" size={15} />
              Edit on GitHub
            </a>
            <button className="docs-action-button" onClick={copyMarkdown} type="button">
              {markdownCopyStatus === 'copied'
                ? <Check aria-hidden="true" size={15} />
                : markdownCopyStatus === 'failed'
                  ? <RotateCcw aria-hidden="true" size={15} />
                  : <Clipboard aria-hidden="true" size={15} />}
              {markdownCopyStatus === 'copied'
                ? 'Markdown copied'
                : markdownCopyStatus === 'failed'
                  ? 'Try copy again'
                  : 'Copy Markdown'}
            </button>
            <a className="docs-action-link" href="#docs-content">Back to top</a>
          </div>
        </header>
        <section
          className="docs-prose"
          dangerouslySetInnerHTML={{ __html: page.html }}
          onClick={handleArticleClick}
        />

        <footer>
          <nav aria-label="Documentation pagination" className="docs-pagination">
            {page.previous === undefined ? <span /> : (
              <a className="docs-pagination-link" href={page.previous.path}>
                <span className="docs-pagination-direction"><ArrowLeft aria-hidden="true" size={14} /> Previous</span>
                <span className="docs-pagination-title">{page.previous.title}</span>
              </a>
            )}
            {page.next === undefined ? <span /> : (
              <a className="docs-pagination-link" href={page.next.path}>
                <span className="docs-pagination-direction">Next <ArrowRight aria-hidden="true" size={14} /></span>
                <span className="docs-pagination-title">{page.next.title}</span>
              </a>
            )}
          </nav>
        </footer>
      </article>

      {page.headings.length > 0 && (
        <aside className="docs-toc">
          <h2 className="docs-toc-title">On this page</h2>
          <nav aria-label="On this page">
            <ul className="docs-toc-list">
              {page.headings.map(heading => (
                <li key={heading.id}>
                  <a className="docs-toc-link" data-level={heading.depth} href={`#${heading.id}`}>
                    {heading.text}
                  </a>
                </li>
              ))}
            </ul>
          </nav>
        </aside>
      )}
    </div>
  )
}
