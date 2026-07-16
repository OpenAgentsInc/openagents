import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, test } from 'vitest'

describe('retired app landing route', () => {
  test('keeps only compatibility redirects and removes the Launch UI app implementation', () => {
    const appRoute = readFileSync(path.resolve(import.meta.dirname, 'app.tsx'), 'utf8')
    const newRoute = readFileSync(path.resolve(import.meta.dirname, 'new.tsx'), 'utf8')
    const loginPage = readFileSync(path.resolve(import.meta.dirname, '-login-page.tsx'), 'utf8')

    expect(appRoute).toContain("redirect({ to: '/splash' })")
    expect(newRoute).toContain("redirect({ to: '/splash' })")
    expect(appRoute).not.toContain('launch-ui')
    expect(appRoute).not.toContain('LandingPage')
    expect(loginPage).not.toContain("'/app'")
    expect(existsSync(path.resolve(import.meta.dirname, '-app-account.tsx'))).toBe(false)
  })
})
