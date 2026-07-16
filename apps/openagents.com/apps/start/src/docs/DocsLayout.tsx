import { Link, Outlet, useLocation, useNavigate } from '@tanstack/react-router'
import { ChevronDown, Menu, X } from 'lucide-react'
import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from 'react'

import { PublicHeader } from '../components/public-header'
import { docsNavigationDefinition } from './docs-navigation'
import { docsManifest } from './generated/docs-manifest.generated'
import { DocsSearch } from './DocsSearch'
import './docs.css'

type SidebarProps = Readonly<{
  activePath: string
  onNavigate?: () => void
}>

function DocsNavigationGroup({
  activePath,
  collapsed,
  label,
  onNavigate,
  slugs,
}: Readonly<SidebarProps & {
  collapsed: boolean
  label: string
  slugs: ReadonlyArray<string>
}>) {
  const containsActivePath = slugs.some(slug => {
    const entry = docsManifest.find(candidate => candidate.slug === slug)
    return entry?.path === activePath
  })
  const [open, setOpen] = useState(!collapsed || containsActivePath)

  useEffect(() => {
    if (containsActivePath) {
      setOpen(true)
    }
  }, [containsActivePath])

  const links = (
    <ul className="docs-nav-list">
      {slugs.map(slug => {
        const entry = docsManifest.find(candidate => candidate.slug === slug)
        if (entry === undefined) {
          return null
        }
        const active = entry.path === activePath
        const linkContent = entry.sidebarLabel
        return (
          <li key={entry.slug || 'index'}>
            {entry.slug === '' ? (
              <Link
                activeOptions={{ exact: true }}
                aria-current={active ? 'page' : undefined}
                className="docs-nav-link"
                onClick={onNavigate}
                preload="render"
                to="/docs"
              >
                {linkContent}
              </Link>
            ) : (
              <Link
                activeOptions={{ exact: true }}
                aria-current={active ? 'page' : undefined}
                className="docs-nav-link"
                onClick={onNavigate}
                params={{ _splat: entry.slug }}
                preload="render"
                to="/docs/$"
              >
                {linkContent}
              </Link>
            )}
          </li>
        )
      })}
    </ul>
  )

  if (!collapsed) {
    return (
      <section className="docs-nav-group">
        <h2 className="docs-nav-label">{label}</h2>
        {links}
      </section>
    )
  }

  return (
    <details
      className="docs-nav-details docs-nav-group"
      onToggle={event => setOpen(event.currentTarget.open)}
      open={open}
    >
      <summary className="docs-nav-summary">
        <span>{label}</span>
        <ChevronDown aria-hidden="true" className="docs-nav-chevron" size={15} />
      </summary>
      {links}
    </details>
  )
}

function DocsSidebar({ activePath, onNavigate }: SidebarProps) {
  return (
    <nav aria-label="Documentation">
      {docsNavigationDefinition.map(group => (
        <DocsNavigationGroup
          activePath={activePath}
          collapsed={group.collapsed}
          key={group.label}
          label={group.label}
          {...(onNavigate === undefined ? {} : { onNavigate })}
          slugs={group.slugs}
        />
      ))}
    </nav>
  )
}

export function DocsLayout() {
  const location = useLocation()
  const navigate = useNavigate()
  const drawerRef = useRef<HTMLDialogElement>(null)
  const drawerTriggerRef = useRef<HTMLButtonElement>(null)

  const closeDrawer = () => drawerRef.current?.close()

  const openDrawer = () => {
    const drawer = drawerRef.current
    if (drawer === null || drawer.open) {
      return
    }
    drawer.showModal()
  }

  const handleDocsNavigation = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (
      event.defaultPrevented ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return
    }
    const target = event.target
    if (!(target instanceof Element)) {
      return
    }
    const anchor = target.closest<HTMLAnchorElement>('a[href]')
    if (anchor === null || anchor.target === '_blank') {
      return
    }
    const url = new URL(anchor.href, window.location.href)
    if (url.origin !== window.location.origin || !url.pathname.startsWith('/docs')) {
      return
    }
    event.preventDefault()
    const hash = url.hash.replace(/^#/, '')
    if (url.pathname === '/docs' || url.pathname === '/docs/') {
      void navigate({ hash, to: '/docs' })
      return
    }
    void navigate({
      hash,
      params: { _splat: url.pathname.slice('/docs/'.length) },
      to: '/docs/$',
    })
  }

  return (
    <div className="docs-root" onClick={handleDocsNavigation}>
      <a className="docs-skip-link" href="#docs-content">Skip to documentation</a>
      <PublicHeader
        docsActive
        leading={
          <button
            aria-haspopup="dialog"
            aria-label="Open documentation navigation"
            className="docs-header-menu"
            onClick={openDrawer}
            ref={drawerTriggerRef}
            type="button"
          >
            <Menu aria-hidden="true" size={20} />
          </button>
        }
        utility={<DocsSearch />}
        variant="docs"
      />

      <div className="docs-shell">
        <aside className="docs-sidebar">
          <DocsSidebar activePath={location.pathname} />
        </aside>
        <main className="docs-main" id="docs-content">
          <Outlet />
        </main>
      </div>

      <dialog
        aria-label="Documentation navigation"
        className="docs-drawer"
        onClick={event => {
          if (event.currentTarget === event.target) {
            closeDrawer()
          }
        }}
        onClose={() => {
          drawerTriggerRef.current?.focus()
        }}
        ref={drawerRef}
      >
        <div>
          <div className="docs-drawer-header">
            <strong className="docs-drawer-title">Documentation</strong>
            <button className="docs-drawer-close" aria-label="Close documentation navigation" onClick={closeDrawer} type="button">
              <X aria-hidden="true" size={20} />
            </button>
          </div>
          <div className="docs-drawer-nav">
            <DocsSidebar activePath={location.pathname} onNavigate={closeDrawer} />
          </div>
        </div>
      </dialog>
    </div>
  )
}

export function DocsNotFound() {
  return (
    <div className="docs-not-found">
      <h1>Nothing at this path</h1>
      <p>That documentation page does not exist.</p>
      <Link to="/docs">Return to OpenAgents Docs</Link>
    </div>
  )
}
