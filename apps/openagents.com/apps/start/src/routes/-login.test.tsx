import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { LoginPage } from './-login-page'

describe('Start login route', () => {
  test('server-renders the OpenAuth provider links', () => {
    const html = renderToStaticMarkup(<LoginPage />)

    expect(html).toContain('data-route="login"')
    expect(html).toContain('data-persistent-scene-overlay="login"')
    expect(html).toContain('Early access')
    expect(html).toContain('Log in to OpenAgents')
    expect(html).toContain('aria-label="Login options"')
    expect(html).toContain('If your account is approved for early access')
    expect(html).toContain('href="/login/email?returnTo=%2Fadmin%2Fanalytics"')
    expect(html).toContain('Continue with email')
    expect(html).toContain('href="/login/github?returnTo=%2Fadmin%2Fanalytics"')
    expect(html).toContain('Continue with GitHub')
    expect(html).toContain('not open for public signup yet')
    expect(html).toContain('access remains limited to approved users')
    expect(html).not.toContain('Omega early access')
    expect(html).not.toMatch(/Sign[- ]?in/i)
    expect(html).not.toContain('href="/login"')
    expect(html).not.toContain('>Log In</a>')
  })

  test('carries a protected return target to both providers', () => {
    const html = renderToStaticMarkup(<LoginPage returnTo="/admin/analytics" />)

    expect(html).toContain('href="/login/email?returnTo=%2Fadmin%2Fanalytics"')
    expect(html).toContain('href="/login/github?returnTo=%2Fadmin%2Fanalytics"')
  })
})
