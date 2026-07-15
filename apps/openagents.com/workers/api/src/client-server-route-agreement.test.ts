import { describe, expect, test } from 'vitest'

import {
  isKnownStartDocumentPath,
  knownDocumentPathPatterns as startDocumentPatterns,
} from '../../../apps/start/src/route-table'
import {
  shouldRedirectUnknownDocumentToHome,
  knownDocumentPathPatterns as workerDocumentPatterns,
} from './worker-routes'

const requestFor = (pathname: string) =>
  new Request(`https://openagents.com${pathname}`, {
    headers: { accept: 'text/html' },
    method: 'GET',
  })

describe('Start ⇄ Worker document route agreement (#8813)', () => {
  test('the Worker allowlist is the dependency-free Start allowlist', () => {
    expect(workerDocumentPatterns.map(pattern => pattern.source)).toEqual(
      startDocumentPatterns.map(pattern => pattern.source),
    )
  })

  test.each([
    '/app',
    '/astro',
    '/install',
    '/login',
    '/promises',
    '/forum',
    '/forum/f/product-promises',
    '/forum/t/topic-1',
    '/forum/receipts/receipt-1',
    '/privacy',
    '/terms',
    '/tanstack',
    '/agents/artanis',
    '/training/runs/run-1',
  ])('retained Start document %s is admitted by the Worker', pathname => {
    expect(isKnownStartDocumentPath(pathname)).toBe(true)
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor(pathname), pathname),
    ).toBe(false)
  })

  test('the public root stays out of the path-only table so auth.openagents.com cannot render it', () => {
    expect(isKnownStartDocumentPath('/')).toBe(false)
  })

  test.each([
    '/billing',
    '/checkout',
    '/sites',
    '/credits',
    '/some-retired-foldkit-route',
  ])('retired/non-MVP document %s is not owned by Start', pathname => {
    expect(isKnownStartDocumentPath(pathname)).toBe(false)
  })
})
