import { describe, expect, test } from 'vitest'

import {
  diamondHandsResponseHeaders,
  isDiamondHandsPath,
  startUiAssetRelativePath,
  startUiContentType,
} from './start-ui'

describe('Operation Diamond Hands static GPUI document', () => {
  test.each([
    '/dh',
    '/dh/',
    '/dh/diamond_hands_web.js',
    '/dh/diamond_hands_web_bg.wasm',
  ])('recognizes %s as part of the isolated document', pathname => {
    expect(isDiamondHandsPath(pathname)).toBe(true)
  })

  test('serves WebAssembly with its required media type', () => {
    expect(startUiContentType('diamond_hands_web_bg.wasm')).toBe(
      'application/wasm',
    )
  })

  test.each(['/dh', '/dh/'])(
    'maps the %s document coordinate to its static entry',
    pathname => {
      expect(startUiAssetRelativePath(pathname)).toBe('dh/index.html')
    },
  )

  test('applies WebGPU/thread isolation and permits only the public relay connection', () => {
    const headers = diamondHandsResponseHeaders('/dh')
    expect(headers['cross-origin-opener-policy']).toBe('same-origin')
    expect(headers['cross-origin-embedder-policy']).toBe('require-corp')
    expect(headers['cross-origin-resource-policy']).toBe('same-origin')
    expect(headers['content-security-policy']).toContain(
      'connect-src https://relay.openagents.com wss://relay.openagents.com',
    )
    expect(headers['content-security-policy']).toContain(
      "worker-src 'self' blob:",
    )
  })

  test('does not add project-page policy to unrelated retained assets', () => {
    expect(isDiamondHandsPath('/assets/index.js')).toBe(false)
    expect(diamondHandsResponseHeaders('/assets/index.js')).toEqual({})
  })
})
