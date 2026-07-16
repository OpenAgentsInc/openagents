import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { LoginPage } from './-login-page'

describe('Start login route', () => {
  test('server-renders the login form and provider link', () => {
    const html = renderToStaticMarkup(<LoginPage />)

    expect(html).toContain('data-route="login"')
    expect(html).toContain('data-persistent-scene-overlay="login"')
    expect(html).toContain('Log in to OpenAgents')
    expect(html).toContain('action="/login/email"')
    expect(html).toContain('method="get"')
    expect(html).toContain('name="email"')
    expect(html).toContain('you@example.com')
    expect(html).toContain('Email me a code')
    expect(html).toContain('href="/login/github"')
    expect(html).toContain('Log in with GitHub')
  })
})
