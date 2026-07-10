import { isRedirect } from '@tanstack/react-router'
import { describe, expect, test } from 'vitest'

import {
  Route as CodeDownloadRoute,
  redirectRetiredCodeDownloadRoute,
} from './download'
import { Route as CodeRoute, redirectRetiredCodeRoute } from './index'

const captureRedirect = (execute: () => unknown) => {
  try {
    execute()
  } catch (error) {
    if (isRedirect(error)) {
      return error
    }

    throw error
  }

  throw new Error('Expected route guard to throw a redirect')
}

describe('retired Khala Code product routes', () => {
  test('/code permanently redirects to the OpenAgents landing page', () => {
    const result = captureRedirect(redirectRetiredCodeRoute)

    expect(result.status).toBe(308)
    expect(result.options.to).toBe('/')
    expect(result.options.replace).toBe(true)
    expect(CodeRoute.options).not.toHaveProperty('component')
    expect(CodeRoute.options).not.toHaveProperty('head')
  })

  test('/code/download permanently redirects to preserved promise history', () => {
    const result = captureRedirect(redirectRetiredCodeDownloadRoute)

    expect(result.status).toBe(308)
    expect(result.options.to).toBe('/promises')
    expect(result.options.replace).toBe(true)
    expect(CodeDownloadRoute.options).not.toHaveProperty('component')
    expect(CodeDownloadRoute.options).not.toHaveProperty('head')
  })
})
