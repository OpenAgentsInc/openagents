import { describe, expect, test } from 'vitest'

import { canonicalWebRoute, trackNamedAnalyticsEvent } from './web-analytics'

describe('web analytics adapter', () => {
  test('maps paths to bounded route ids without query or user values', () => {
    expect(canonicalWebRoute('/')).toBe('/')
    expect(canonicalWebRoute('/blog/introducing-omega')).toBe('/blog/:slug')
    expect(canonicalWebRoute('/trace/private-id')).toBe('/trace/:id')
    expect(canonicalWebRoute('/unknown/private-id')).toBe('/other')
  })

  test('named events return immediately when transport is unavailable', () => {
    expect(trackNamedAnalyticsEvent('github_view', '/')).toBeUndefined()
  })
})
