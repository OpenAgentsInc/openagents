#!/usr/bin/env bun

// Guard logic for the vendored @effect-native/* snapshot.
//
// The monorepo vendors unbuilt TypeScript from OpenAgentsInc/effect-native as
// workspace members (see apps/openagents.com/packages/effect-native-vendor.json
// and apps/openagents.com/packages/effect-native-render-rn/VENDORING.md). This
// module provides the pure checks that make a DRIFTED or PARTIALLY-BUMPED
// vendor state a hard RED test:
//
//   (a) every vendored package.json records the SAME upstream commit as the
//       manifest (a half-bumped set — one package left at the old SHA — fails);
//   (b) the vendored core's `CatalogVersion` literal matches the manifest's
//       recorded catalogVersion.
//
// Freshness vs. upstream is a separate, non-fatal WARNING — see
// check-effect-native-vendor-freshness.ts. Staleness is expected (upstream
// moves fast); a mismatched/partial vendor is the hard failure.

import { readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
// scripts/ -> apps/openagents.com
export const APP_ROOT = resolve(HERE, '..')
// apps/openagents.com -> monorepo root. The manifest's vendoredPackages paths
// are repo-root-relative, so package reads resolve from here.
export const REPO_ROOT = resolve(APP_ROOT, '..', '..')
export const MANIFEST_PATH = join(
  APP_ROOT,
  'packages',
  'effect-native-vendor.json',
)

export const readManifest = (manifestPath = MANIFEST_PATH) =>
  JSON.parse(readFileSync(manifestPath, 'utf8'))

/**
 * Resolve the effective `CatalogVersion` string literal from the vendored core
 * source. Upstream sets `CatalogVersion = <SomeAlias>` and `<SomeAlias> =
 * "effect-native/vN" as const`, so we follow one alias hop; a direct string
 * assignment is also supported.
 */
export const extractCoreCatalogVersion = (coreSource) => {
  const direct = coreSource.match(
    /export const CatalogVersion\s*=\s*"([^"]+)"/,
  )
  if (direct) return direct[1]

  const aliased = coreSource.match(/export const CatalogVersion\s*=\s*(\w+)/)
  if (!aliased) return null
  const aliasName = aliased[1]
  const literal = coreSource.match(
    new RegExp(`export const ${aliasName}\\s*=\\s*"([^"]+)"`),
  )
  return literal ? literal[1] : null
}

export const readPackageCommit = (packageDir, repoRoot = REPO_ROOT) => {
  const pkgPath = join(repoRoot, packageDir, 'package.json')
  const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'))
  return pkg.effectNativeVendor?.commit ?? null
}

/**
 * Returns a list of findings (empty === green). Each finding is a
 * `{ kind, message }` describing a drift/partial-bump/catalog mismatch.
 */
export const checkVendorConsistency = ({
  manifest = readManifest(),
  repoRoot = REPO_ROOT,
} = {}) => {
  const findings = []

  if (typeof manifest.commit !== 'string' || manifest.commit.length < 7) {
    findings.push({
      kind: 'manifest',
      message: `manifest.commit is missing or too short: ${JSON.stringify(manifest.commit)}`,
    })
  }
  if (
    typeof manifest.catalogVersion !== 'string' ||
    !manifest.catalogVersion.startsWith('effect-native/')
  ) {
    findings.push({
      kind: 'manifest',
      message: `manifest.catalogVersion is missing or malformed: ${JSON.stringify(manifest.catalogVersion)}`,
    })
  }

  const packages = Array.isArray(manifest.vendoredPackages)
    ? manifest.vendoredPackages
    : []
  if (packages.length === 0) {
    findings.push({
      kind: 'manifest',
      message: 'manifest.vendoredPackages is empty',
    })
  }

  for (const packageDir of packages) {
    let commit = null
    try {
      commit = readPackageCommit(packageDir, repoRoot)
    } catch (error) {
      findings.push({
        kind: 'package',
        message: `${packageDir}: cannot read package.json (${error instanceof Error ? error.message : String(error)})`,
      })
      continue
    }
    if (commit === null) {
      findings.push({
        kind: 'package',
        message: `${packageDir}: package.json has no effectNativeVendor.commit`,
      })
      continue
    }
    if (commit !== manifest.commit) {
      findings.push({
        kind: 'partial-bump',
        message: `${packageDir}: effectNativeVendor.commit ${commit} !== manifest.commit ${manifest.commit} (partial/drifted vendor)`,
      })
    }
  }

  // Catalog version literal in the vendored core must match the manifest.
  const corePackageDir = packages.find((p) => p.endsWith('effect-native-core'))
  if (corePackageDir) {
    try {
      const coreSource = readFileSync(
        join(repoRoot, corePackageDir, 'src', 'index.ts'),
        'utf8',
      )
      const literal = extractCoreCatalogVersion(coreSource)
      if (literal === null) {
        findings.push({
          kind: 'catalog',
          message: `${corePackageDir}: could not extract CatalogVersion literal from src/index.ts`,
        })
      } else if (literal !== manifest.catalogVersion) {
        findings.push({
          kind: 'catalog',
          message: `${corePackageDir}: core CatalogVersion ${literal} !== manifest.catalogVersion ${manifest.catalogVersion}`,
        })
      }
    } catch (error) {
      findings.push({
        kind: 'catalog',
        message: `${corePackageDir}: cannot read src/index.ts (${error instanceof Error ? error.message : String(error)})`,
      })
    }
  } else {
    findings.push({
      kind: 'manifest',
      message: 'manifest.vendoredPackages does not include effect-native-core',
    })
  }

  return findings
}

// Allow running directly as a fast gate too: `bun scripts/check-effect-native-vendor.mjs`
if (import.meta.main) {
  const findings = checkVendorConsistency()
  if (findings.length > 0) {
    console.error('effect-native vendor guard: FAIL')
    for (const f of findings) console.error(`  - [${f.kind}] ${f.message}`)
    process.exit(1)
  }
  console.log('effect-native vendor guard: OK (manifest + package commits + catalog version consistent)')
}
