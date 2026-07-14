import { describe, expect, test } from 'vitest'
import { readFileSync } from 'node:fs'

import {
  checkVendorConsistency,
  extractCoreCatalogVersion,
  readManifest,
} from './check-effect-native-vendor.mjs'

describe('effect-native vendor guard', () => {
  test('the real vendored tree is internally consistent (no partial bump / catalog drift)', () => {
    const findings = checkVendorConsistency()
    // A non-empty findings list means a drifted or partially-bumped vendor
    // state — print it so the failure is actionable.
    expect(findings).toEqual([])
  })

  test('manifest pins a full commit and a catalog version', () => {
    const manifest = readManifest()
    expect(manifest.commit).toMatch(/^[0-9a-f]{40}$/)
    expect(manifest.catalogVersion).toMatch(/^effect-native\/v\d+$/)
    expect(Array.isArray(manifest.vendoredPackages)).toBe(true)
    expect(manifest.vendoredPackages.length).toBeGreaterThan(0)
  })

  test('the pinned DOM renderer carries the React projection boundary atomically', () => {
    const packageJson = JSON.parse(
      readFileSync(
        new URL('../packages/effect-native-render-dom/package.json', import.meta.url),
        'utf8',
      ),
    ) as {
      exports?: Record<string, string>
      peerDependencies?: Record<string, string>
    }
    const source = readFileSync(
      new URL('../packages/effect-native-render-dom/src/react.ts', import.meta.url),
      'utf8',
    )

    expect(packageJson.exports?.['./react']).toBe('./src/react.ts')
    expect(packageJson.peerDependencies).toMatchObject({
      react: '^19.0.0',
      'react-dom': '^19.0.0',
    })
    expect(source).toContain('makeReactDomRenderer')
    expect(source).toContain('useSyncExternalStore')
    expect(source).toContain('backend?: ReactDomBackend')
    expect(source).toContain('data-en-react-backend')
    expect(source).toContain('data-en-react-surface')
  })

  test('catalog-version extractor follows the alias hop and direct assignment', () => {
    const aliased = [
      'export const GraphCatalogVersion = "effect-native/v19" as const',
      'export const CatalogVersion = GraphCatalogVersion',
    ].join('\n')
    expect(extractCoreCatalogVersion(aliased)).toBe('effect-native/v19')

    const direct = 'export const CatalogVersion = "effect-native/v5" as const'
    expect(extractCoreCatalogVersion(direct)).toBe('effect-native/v5')
  })

  test('a partial bump (one package left at the old commit) is a hard failure', () => {
    const manifest = {
      commit: 'a'.repeat(40),
      catalogVersion: 'effect-native/v19',
      vendoredPackages: ['packages/effect-native-core'],
    }
    // Point at a temp app root where the package records a stale commit.
    // Simulated purely through the pure function's inputs: reuse the real
    // reader but with a manifest whose commit cannot match the on-disk value.
    const findings = checkVendorConsistency({ manifest })
    expect(findings.some((f) => f.kind === 'partial-bump' || f.kind === 'package')).toBe(
      true,
    )
  })

  test('a catalog-version mismatch is a hard failure', () => {
    const manifest = { ...readManifest(), catalogVersion: 'effect-native/v999' }
    const findings = checkVendorConsistency({ manifest })
    expect(findings.some((f) => f.kind === 'catalog')).toBe(true)
  })
})
