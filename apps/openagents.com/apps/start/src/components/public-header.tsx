import { InternalLink } from '@/components/internal-link'
import {
  type PublicAuthState,
  publicAuthAction,
  publicLogoutAction,
  readPublicAuthState,
} from '@/lib/public-auth'
import { DOCS_URL, GITHUB_REPOSITORY_URL, X_URL } from '@/lib/public-site'
import { type ReactNode, useEffect, useState } from 'react'

import '../public-header.css'
import GithubMark from './launch-ui/logos/github'
import { XMark } from './x-mark'

type PublicHeaderProps = Readonly<{
  docsActive?: boolean
  leading?: ReactNode
  showLogin?: boolean
  showLogout?: boolean
  utility?: ReactNode
  variant?: 'default' | 'docs'
}>

export function PublicHeader({
  docsActive = false,
  leading,
  showLogin = true,
  showLogout = false,
  utility,
  variant = 'default',
}: PublicHeaderProps = {}) {
  const [authState, setAuthState] = useState<PublicAuthState>({
    tag: 'anonymous',
  })

  useEffect(() => {
    if (!showLogin) return

    let active = true
    void readPublicAuthState().then(state => {
      if (active) setAuthState(state)
    })
    return () => {
      active = false
    }
  }, [showLogin])

  const authAction = publicAuthAction(authState)
  const logoutAction = publicLogoutAction(authState, showLogout)

  return (
    <>
      <header
        className={
          variant === 'docs'
            ? 'oa-unified-header oa-unified-header--docs'
            : 'oa-unified-header'
        }
      >
        <nav aria-label="Primary navigation" className="oa-unified-nav">
          <div className="oa-unified-nav-left">
            {leading}
            <InternalLink
              aria-label="OpenAgents home"
              className="oa-unified-brand"
              href="/"
              preload="render"
            >
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
            <a
              aria-label="OpenAgents on X"
              className="oa-unified-icon-link oa-unified-x-link"
              href={X_URL}
              rel="noreferrer"
              target="_blank"
            >
              <XMark aria-hidden="true" />
            </a>
            <a
              aria-label="OpenAgents on GitHub"
              className="oa-unified-github-link oa-unified-icon-link"
              href={GITHUB_REPOSITORY_URL}
              rel="noreferrer"
              target="_blank"
            >
              <GithubMark aria-hidden="true" />
            </a>
            {showLogin ? (
              <InternalLink
                className="oa-unified-primary-link"
                href={authAction.href}
                preload="intent"
              >
                {authAction.label}
              </InternalLink>
            ) : null}
            {logoutAction ? (
              <a
                aria-label="Log Out of OpenAgents"
                className="oa-unified-logout-link"
                href={logoutAction.href}
              >
                {logoutAction.label}
              </a>
            ) : null}
          </div>
        </nav>
      </header>
      <div aria-hidden="true" className="oa-unified-header-spacer" />
    </>
  )
}
