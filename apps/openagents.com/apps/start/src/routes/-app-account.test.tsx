import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test, vi } from 'vitest'

import { AppAccount, readAppSessionIdentity } from './-app-account'

describe('authenticated app account control', () => {
  test('parses the real authenticated session projection', async () => {
    const fetchFn = vi.fn(() =>
      Promise.resolve(
        Response.json({
          authenticated: true,
          bootstrap: {
            session: {
              avatarUrl: 'https://avatars.example/chris.png',
              email: 'chris@openagents.com',
              login: 'chris',
              name: 'Christopher David',
            },
          },
        }),
      ),
    ) as unknown as typeof fetch

    await expect(readAppSessionIdentity(fetchFn)).resolves.toEqual({
      avatarUrl: 'https://avatars.example/chris.png',
      email: 'chris@openagents.com',
      login: 'chris',
      name: 'Christopher David',
    })
    expect(fetchFn).toHaveBeenCalledWith(
      '/api/auth/session',
      expect.objectContaining({ cache: 'no-store', credentials: 'include' }),
    )
  })

  test('renders avatar, explicit signed-in state, and logout', () => {
    const html = renderToStaticMarkup(
      <AppAccount
        initialIdentity={{
          avatarUrl: 'https://avatars.example/chris.png',
          email: 'chris@openagents.com',
          login: 'chris',
          name: 'Christopher David',
        }}
      />,
    )

    expect(html).toContain('data-app-account="signed-in"')
    expect(html).toContain('Signed in as Christopher David')
    expect(html).toContain('https://avatars.example/chris.png')
    expect(html).toContain('Signed in · @chris')
    expect(html).toContain('href="/logout"')
    expect(html).toContain('Log out')
  })

  test('fails closed to a login action when the session projection is absent', async () => {
    const fetchFn = vi.fn(() =>
      Promise.resolve(Response.json({ authenticated: false })),
    ) as unknown as typeof fetch

    await expect(readAppSessionIdentity(fetchFn)).resolves.toBeNull()
    const html = renderToStaticMarkup(<AppAccount initialIdentity={null} />)
    expect(html).toContain('Session expired · Log in')
    expect(html).toContain('href="/login?returnTo=%2Fapp"')
  })
})
