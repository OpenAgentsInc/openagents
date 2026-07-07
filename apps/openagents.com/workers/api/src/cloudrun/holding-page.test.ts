import { describe, expect, it } from 'bun:test'

import { holdingPageInterception } from './holding-page'

const u = (href: string) => new URL(href)

describe('holdingPageInterception', () => {
  it('serves the placeholder at the public-host root', async () => {
    const r = holdingPageInterception(u('https://openagents.com/'))
    expect(r).toBeInstanceOf(Response)
    expect(r?.status).toBe(200)
    expect(await r!.text()).toContain('be right back')
    // Also matches www.
    expect(holdingPageInterception(u('https://www.openagents.com/'))?.status).toBe(200)
  })

  it('does NOT block or redirect any other public page route', () => {
    for (const p of ['/forum', '/forum/u/user_x/lathe', '/settings', '/agents/artanis', '/stats', '/billing']) {
      expect(holdingPageInterception(u(`https://openagents.com${p}`))).toBeUndefined()
    }
  })

  it('passes through API, assets, and machine files at the root host', () => {
    for (const p of ['/api/forum', '/assets/app.js', '/AGENTS.md', '/favicon.ico', '/.well-known/x']) {
      expect(holdingPageInterception(u(`https://openagents.com${p}`))).toBeUndefined()
    }
  })

  it('never affects the auth issuer host or direct Cloud Run URLs', () => {
    expect(holdingPageInterception(u('https://auth.openagents.com/'))).toBeUndefined()
    expect(holdingPageInterception(u('https://auth.openagents.com/authorize?x=1'))).toBeUndefined()
    expect(
      holdingPageInterception(u('https://openagents-monolith-abc.us-central1.run.app/')),
    ).toBeUndefined()
  })
})
