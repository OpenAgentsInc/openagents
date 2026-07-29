import { describe, expect, test } from 'vitest'

import { isPublicSiteRootRequest } from './public-site-host'
import {
  isStartDocumentRequestPath,
  isStartServerRequest,
  isStartServerRequestPath,
} from './start-ui'

describe('public Start homepage host boundary', () => {
  test.each([
    'https://openagents.com/',
    'https://www.openagents.com/',
  ])('admits the apex website root %s', href => {
    expect(isPublicSiteRootRequest(new URL(href))).toBe(true)
  })

  test.each([
    'https://auth.openagents.com/',
    'https://openagents.com/login',
    'https://openagents.com/api/auth/session',
  ])('does not claim auth, API, or non-root requests %s', href => {
    expect(isPublicSiteRootRequest(new URL(href))).toBe(false)
  })

  test('admits root only through the explicit apex dispatch', () => {
    expect(isStartDocumentRequestPath('/', true)).toBe(true)
    expect(isStartDocumentRequestPath('/')).toBe(false)
  })

  test('routes only the admitted Start server APIs through the seam', () => {
    expect(isStartServerRequestPath('/api/public/qa-board')).toBe(true)
    expect(isStartServerRequestPath('/api/public/qa-board/')).toBe(false)
    // DIST-10 (#8923): Desktop download resolver + verified artifact redirect
    // are Start-served and must cross the Cloud Run adapter (QA-4 lesson).
    expect(isStartServerRequestPath('/api/public/desktop-download')).toBe(true)
    expect(isStartServerRequestPath('/api/public/desktop-download/artifact')).toBe(true)
    expect(isStartServerRequestPath('/api/public/product-download')).toBe(true)
    expect(isStartServerRequestPath('/api/public/desktop-download/')).toBe(false)
    expect(isStartServerRequestPath('/api/public/desktop-download/other')).toBe(false)
    expect(isStartServerRequestPath('/api/public/product-download/')).toBe(false)
    expect(
      isStartServerRequestPath(
        '/internal/v1/repositories/tenant.openagents/omega/web-read-asset/crates/zed/icon.png',
      ),
    ).toBe(true)
    expect(
      isStartServerRequestPath(
        '/internal/v1/repositories/tenant.openagents/omega/web-read',
      ),
    ).toBe(false)
    expect(
      isStartServerRequestPath(
        '/internal/v1/repositories/tenant.openagents/omega/web-read-asset',
      ),
    ).toBe(false)
    expect(isStartServerRequestPath('/_serverFn/forge-repository-read')).toBe(
      true,
    )
    expect(isStartServerRequestPath('/_serverFn/')).toBe(false)
    expect(
      isStartServerRequestPath('/_serverFn/forge-repository-read/other'),
    ).toBe(false)
    expect(isStartServerRequestPath('/api/auth/session')).toBe(false)
    expect(isStartServerRequestPath('/api/portal/session')).toBe(false)
  })

  test('admits POST only for exact Start server-function paths', () => {
    expect(
      isStartServerRequest(
        new Request(
          'https://openagents.com/_serverFn/forge-repository-read',
          { method: 'POST' },
        ),
      ),
    ).toBe(true)
    expect(
      isStartServerRequest(
        new Request(
          'https://openagents.com/_serverFn/forge-repository-read',
          { method: 'PUT' },
        ),
      ),
    ).toBe(false)
    expect(
      isStartServerRequest(
        new Request('https://openagents.com/forge/tenant.openagents/omega', {
          method: 'POST',
        }),
      ),
    ).toBe(false)
  })
})
