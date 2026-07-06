import { describe, expect, test } from 'bun:test'

import { holdingPageInterception } from './holding-page'

describe('holdingPageInterception', () => {
  const u = (s: string): URL => new URL(s)

  test('serves the be-right-back page at the public root', () => {
    const r = holdingPageInterception(u('https://openagents.com/'))
    expect(r?.status).toBe(200)
  })

  test('redirects other public page routes to home', () => {
    for (const p of ['/stats', '/khala', '/code', '/business', '/docs/x', '/anything']) {
      const r = holdingPageInterception(u(`https://openagents.com${p}`))
      expect(r?.status).toBe(302)
      expect(r?.headers.get('location')).toBe('https://openagents.com/')
    }
  })

  test('passes API, sync, well-known, assets and machine files through', () => {
    for (const p of [
      '/api/mobile/session',
      '/api/sync/bootstrap',
      '/api/public/khala-tokens-served',
      '/.well-known/oauth-authorization-server',
      '/_expo/static/js/app.js',
      '/assets/index-abc.css',
      '/favicon.ico',
      '/AGENTS.md',
      '/robots.txt',
    ]) {
      expect(holdingPageInterception(u(`https://openagents.com${p}`))).toBeUndefined()
    }
  })

  test('never touches the auth issuer host or direct Cloud Run URLs', () => {
    expect(holdingPageInterception(u('https://auth.openagents.com/authorize?x=1'))).toBeUndefined()
    expect(holdingPageInterception(u('https://auth.openagents.com/'))).toBeUndefined()
    expect(
      holdingPageInterception(u('https://openagents-monolith-abc.us-central1.run.app/stats')),
    ).toBeUndefined()
  })
})
