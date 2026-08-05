import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, test } from 'vitest'

import { isKnownStartDocumentPath } from '../route-table'

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

// #9325: `/stage1` and `/landing-en` were unlinked surfaces whose purpose was
// validating the Effect Native framework, not shipping a product page. They are
// retired with the framework rather than converted to plain React.
describe('retired Effect Native landing surfaces', () => {
  test('/stage1 keeps only a compatibility redirect', () => {
    const stage1Route = readFileSync(path.resolve(import.meta.dirname, 'stage1.tsx'), 'utf8')

    expect(stage1Route).toContain("createFileRoute('/stage1')")
    expect(stage1Route).toContain("redirect({ to: '/splash' })")
    expect(stage1Route).not.toContain('component')
    expect(stage1Route).not.toContain('Stage1EffectNativePage')
    expect(stage1Route).not.toContain('@effect-native')
  })

  test('/landing-en keeps only a compatibility redirect', () => {
    const landingEnRoute = readFileSync(
      path.resolve(import.meta.dirname, 'landing-en.tsx'),
      'utf8',
    )

    expect(landingEnRoute).toContain("createFileRoute('/landing-en')")
    expect(landingEnRoute).toContain("redirect({ to: '/splash' })")
    expect(landingEnRoute).not.toContain('component')
    expect(landingEnRoute).not.toContain('LandingEnPage')
    expect(landingEnRoute).not.toContain('@effect-native')
  })

  test.each([
    '-stage1-effect-native-page.tsx',
    '-stage1-effect-native.test.tsx',
    '-stage1-effect-native-theme.ts',
    '-landing-en-page.tsx',
    '-landing-en.test.tsx',
  ])('the retired implementation module %s is gone', file => {
    expect(existsSync(path.resolve(import.meta.dirname, file))).toBe(false)
  })

  test('both retired paths stay owned by Start so the redirect actually runs', () => {
    // An unlisted document path is 302'd to `/` at the Worker edge before Start
    // is consulted, which would make the redirects above unreachable. Same
    // reason `/app`, `/new`, and `/code` stay listed after retirement.
    expect(isKnownStartDocumentPath('/stage1')).toBe(true)
    expect(isKnownStartDocumentPath('/landing-en')).toBe(true)
  })
})
