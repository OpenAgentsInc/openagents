import { Search, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import { decodeDocsSearchIndex, type DocsSearchRecord } from './content-schema'

const loadSearchEngine = async () => {
  const orama = await import('@orama/orama')
  const response = await fetch('/docs/search.json', {
    headers: { accept: 'application/json' },
  })
  if (!response.ok) {
    throw new Error(`Search index returned ${response.status}`)
  }
  const records = decodeDocsSearchIndex(await response.json())
  const database = await orama.create({
    schema: {
      body: 'string',
      description: 'string',
      headings: 'string',
      id: 'string',
      path: 'string',
      title: 'string',
    },
  })
  await orama.insertMultiple(database, [...records])
  return { database, orama, records }
}

type SearchEngine = Awaited<ReturnType<typeof loadSearchEngine>>

const searchEngineCache: { promise?: Promise<SearchEngine> } = {}

const getSearchEngine = (): Promise<SearchEngine> => {
  searchEngineCache.promise ??= loadSearchEngine()
  return searchEngineCache.promise
}

export function DocsSearch() {
  const dialogRef = useRef<HTMLDialogElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<ReadonlyArray<DocsSearchRecord>>([])
  const [status, setStatus] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')

  const openSearch = () => {
    const dialog = dialogRef.current
    if (dialog === null || dialog.open) {
      return
    }
    dialog.showModal()
    setStatus('loading')
    void getSearchEngine()
      .then(() => {
        setStatus('ready')
        inputRef.current?.focus()
      })
      .catch(() => setStatus('error'))
  }

  useEffect(() => {
    const controller = new AbortController()
    window.addEventListener('keydown', event => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        openSearch()
      }
    }, { signal: controller.signal })
    return () => controller.abort()
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    if (query.trim() === '') {
      setResults([])
      return () => controller.abort()
    }
    void getSearchEngine()
      .then(async ({ database, orama, records }) => {
        const response = await orama.search(database, {
          boost: { headings: 2, title: 4 },
          limit: 8,
          properties: ['title', 'description', 'headings', 'body'],
          term: query,
        })
        const matchedPaths = response.hits.map(hit => String(hit.document['path']))
        const matchedRecords = matchedPaths.flatMap(path => {
          const record = records.find(candidate => candidate.path === path)
          return record === undefined ? [] : [record]
        })
        if (!controller.signal.aborted) {
          setResults(matchedRecords)
          setStatus('ready')
        }
      })
      .catch(() => {
        if (!controller.signal.aborted) {
          setStatus('error')
        }
      })
    return () => controller.abort()
  }, [query])

  const closeSearch = () => dialogRef.current?.close()

  return (
    <>
      <button
        aria-haspopup="dialog"
        className="docs-search-trigger"
        onClick={openSearch}
        type="button"
      >
        <Search aria-hidden="true" size={16} strokeWidth={1.8} />
        <span className="docs-search-label">Search</span>
        <kbd className="docs-search-shortcut">⌘K</kbd>
      </button>
      <dialog
        aria-label="Search documentation"
        className="docs-search-dialog"
        onClick={event => {
          if (event.currentTarget === event.target) {
            closeSearch()
          }
        }}
        onClose={() => {
          setQuery('')
          setResults([])
        }}
        ref={dialogRef}
      >
        <div>
          <div className="docs-search-header">
            <strong className="docs-search-title">Search documentation</strong>
            <button className="docs-search-close" aria-label="Close search" onClick={closeSearch} type="button">
              <X aria-hidden="true" size={18} />
            </button>
          </div>
          <label className="docs-search-form">
            <Search aria-hidden="true" size={18} strokeWidth={1.8} />
            <span className="sr-only">Search documentation</span>
            <input
              autoComplete="off"
              className="docs-search-input"
              onChange={event => setQuery(event.currentTarget.value)}
              placeholder="Search documentation"
              ref={inputRef}
              type="search"
              value={query}
            />
          </label>
          <div aria-live="polite">
            {status === 'loading' && <p className="docs-search-empty">Loading search…</p>}
            {status === 'error' && (
              <p className="docs-search-empty">Search is unavailable. Browse the navigation instead.</p>
            )}
            {status === 'ready' && query.trim() === '' && (
              <p className="docs-search-empty">Search pages, headings, and documentation text.</p>
            )}
            {status === 'ready' && query.trim() !== '' && results.length === 0 && (
              <p className="docs-search-empty">No documentation matched “{query}”.</p>
            )}
            {results.length > 0 && (
              <ul className="docs-search-results">
                {results.map(result => (
                  <li key={result.id}>
              <a
                className="docs-search-result-link"
                href={result.path}
                onClick={closeSearch}
              >
                      <strong className="docs-search-result-title">{result.title}</strong>
                      <span className="docs-search-result-description">{result.description}</span>
              </a>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      </dialog>
    </>
  )
}
