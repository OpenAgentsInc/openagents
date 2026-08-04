import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import {
  diamondHandsDeploymentEnabled,
  diamondHandsResponseHeaders,
  isDiamondHandsPath,
  isMarketDemoPath,
  marketDemoDeploymentEnabled,
  marketDemoResponseHeaders,
  startUiAssetRelativePath,
  startUiContentType,
} from './start-ui'

describe('Operation Diamond Hands static GPUI document', () => {
  test('fails closed unless a fresh deployment explicitly enables it', () => {
    expect(diamondHandsDeploymentEnabled(undefined)).toBe(false)
    expect(diamondHandsDeploymentEnabled('false')).toBe(false)
    expect(diamondHandsDeploymentEnabled('1')).toBe(false)
    expect(diamondHandsDeploymentEnabled('true')).toBe(true)

    const productionEnvironment = readFileSync(
      new URL('../../scripts/cloudrun/env-production.yaml', import.meta.url),
      'utf8',
    )
    expect(productionEnvironment).not.toContain(
      'OPENAGENTS_DIAMOND_HANDS_ENABLED',
    )
  })

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

describe('Market demo static GPUI document', () => {
  test('is env-gated and deliberately enabled by the production environment', () => {
    expect(marketDemoDeploymentEnabled(undefined)).toBe(false)
    expect(marketDemoDeploymentEnabled('false')).toBe(false)
    expect(marketDemoDeploymentEnabled('1')).toBe(false)
    expect(marketDemoDeploymentEnabled('true')).toBe(true)

    // Owner direction 2026-08-04: the swap demo ships live at /demo, so the
    // committed production environment declares the activation explicitly.
    const productionEnvironment = readFileSync(
      new URL('../../scripts/cloudrun/env-production.yaml', import.meta.url),
      'utf8',
    )
    expect(productionEnvironment).toContain(
      'OPENAGENTS_MARKET_DEMO_ENABLED: "true"',
    )
  })

  test.each([
    '/demo',
    '/demo/',
    '/demo/market_demo_web.js',
    '/demo/market_demo_web_bg.wasm',
  ])('recognizes %s as part of the isolated document', pathname => {
    expect(isMarketDemoPath(pathname)).toBe(true)
  })

  test.each(['/demo', '/demo/'])(
    'maps the %s document coordinate to its static entry',
    pathname => {
      expect(startUiAssetRelativePath(pathname)).toBe('demo/index.html')
    },
  )

  test('applies WebGPU/thread isolation and permits only the public relay connection', () => {
    const headers = marketDemoResponseHeaders('/demo')
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

  test('does not add demo policy to unrelated retained assets or the dh page', () => {
    expect(isMarketDemoPath('/assets/index.js')).toBe(false)
    expect(isMarketDemoPath('/dh')).toBe(false)
    expect(marketDemoResponseHeaders('/assets/index.js')).toEqual({})
  })
})
