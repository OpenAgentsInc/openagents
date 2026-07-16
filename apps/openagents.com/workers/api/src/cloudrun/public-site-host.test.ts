import { describe, expect, test } from 'vitest'

import { isPublicSiteRootRequest } from './public-site-host'
import {
  isStartDocumentRequestPath,
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

  test('routes only the QA board API through the Start server seam', () => {
    expect(isStartServerRequestPath('/api/public/qa-board')).toBe(true)
    expect(isStartServerRequestPath('/api/public/qa-board/')).toBe(false)
    expect(isStartServerRequestPath('/api/auth/session')).toBe(false)
    expect(isStartServerRequestPath('/api/portal/session')).toBe(false)
  })
})
