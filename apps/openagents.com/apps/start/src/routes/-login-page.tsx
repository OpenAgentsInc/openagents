import { PublicFooter } from '@/components/public-footer'
import { PublicHeader } from '@/components/public-header'
import { GitBranch, Mail } from 'lucide-react'

import '../login.css'

export const DEFAULT_LOGIN_RETURN_TO = '/admin/analytics'

const providerHref = (
  provider: 'email' | 'github',
  returnTo?: string,
): string =>
  `/login/${provider}?returnTo=${encodeURIComponent(
    returnTo ?? DEFAULT_LOGIN_RETURN_TO,
  )}`

export function LoginPage({ returnTo }: Readonly<{ returnTo?: string }> = {}) {
  return (
    <div
      className="oa-login-page"
      data-persistent-scene-overlay="login"
      data-route="login"
    >
      <PublicHeader showLogin={false} />
      <main aria-labelledby="login-heading" className="oa-login-main">
        <div aria-hidden="true" className="oa-login-ambient" />
        <section className="oa-login-intro">
          <p className="oa-login-eyebrow">Early access</p>
          <h1 id="login-heading">Log In to OpenAgents</h1>
          <p className="oa-login-lede">
            If your account is approved for early access, continue with a secure
            one-time email code or GitHub.
          </p>
          <p className="oa-login-access-note">
            This is not open for public signup yet. Logging in verifies your
            identity; access remains limited to approved users.
          </p>
        </section>

        <section aria-label="Login options" className="oa-login-actions">
          <p className="oa-login-actions-label">Choose how to continue</p>
          <a
            className="oa-login-primary-action"
            href={providerHref('email', returnTo)}
          >
            <Mail aria-hidden="true" />
            <span>Continue with email</span>
          </a>
          <a
            className="oa-login-secondary-action"
            href={providerHref('github', returnTo)}
          >
            <GitBranch aria-hidden="true" />
            <span>Continue with GitHub</span>
          </a>
          <p className="oa-login-security-note">
            Email login uses a one-time code. Both options continue through the
            same secure OpenAgents login service.
          </p>
        </section>
      </main>
      <PublicFooter />
    </div>
  )
}
