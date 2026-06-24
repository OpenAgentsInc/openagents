#!/usr/bin/env bun

// ---------------------------------------------------------------------------
// Provider-account / blueprint-export security-contract drift guard.
//
// Owner-ratified (forum 847d26de). The provider-account + blueprint-export
// security contracts — `ProviderSecretRef`, the provider-account secret-safety
// predicates, and the `IsPrivateDataSafe` private-data-safety predicate family
// — must have exactly ONE authority each, in the canonical packages:
//
//   packages/provider-account-schema   (ProviderSecretRef + secret predicates)
//   packages/blueprint-contracts        (IsPrivateDataSafe family + export seed)
//
// Every other location must IMPORT/RE-EXPORT from those packages, never
// redefine the contract. This guard fails the build on:
//   (a) any NEW duplicate contract authority — a file outside the canonical
//       packages that DEFINES one of the security-critical symbols, and
//   (b) residual drift — the known former-copy files must stay pure
//       re-exports (no local definitions).
//
// Run from apps/openagents.com (wired into `check:architecture`). It walks the
// whole monorepo (repo root is two levels up), skipping build/dep dirs and
// ignored local worktree mirrors that are not repository source.
// ---------------------------------------------------------------------------

import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join, relative } from 'node:path'

const repoRoot = join(import.meta.dir, '..', '..', '..')

// The canonical authority files. Definitions are allowed ONLY here.
const CANONICAL_FILES = new Set(
  [
    'packages/provider-account-schema/src/index.ts',
    'packages/provider-account-schema/src/runtime.ts',
    'packages/blueprint-contracts/src/index.ts',
  ].map(p => join(repoRoot, p)),
)

const SKIP_DIR = /(^|\/)(node_modules|\.git|\.claude|\.pylon-local|\.worktrees|dist|build|target|\.wrangler|\.turbo|coverage|\.next)(\/|$)/
const isSkippedPath = path => SKIP_DIR.test(relative(repoRoot, path))

const listFiles = dir =>
  readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name)
    if (isSkippedPath(path)) return []
    if (entry.isDirectory()) return listFiles(path)
    return /\.tsx?$/.test(path) && !/\.test\.tsx?$/.test(path) ? [path] : []
  })

// A symbol is "defined" (an authority) when a file DECLARES it, not merely
// imports/re-exports it. These patterns match top-level declarations.
const definitionPatterns = symbol => [
  new RegExp(`^export\\s+const\\s+${symbol}\\s*=`, 'm'),
  new RegExp(`^export\\s+function\\s+${symbol}\\b`, 'm'),
  new RegExp(`^export\\s+class\\s+${symbol}\\b`, 'm'),
  // local (non-export) definition that the file then re-exports
  new RegExp(`^(?:const|function|class)\\s+${symbol}\\b`, 'm'),
]

// The security-critical contract symbols that may have exactly one authority.
const GUARDED_SYMBOLS = [
  // provider-account secret contract
  'ProviderSecretRef',
  'containsProviderSecretMaterial',
  'isPublicSecretReference',
  'requirePublicSecretReference',
  'isPublicSecretRef',
  'containsSecretMaterial',
  'validateProbePublicProjection',
  'sanitizeProbePublicProjection',
  // blueprint IsPrivateDataSafe family + export-seed predicate
  'isBlueprintProjectionPrivateDataSafe',
  'blueprintContractExportSeedIsPrivateDataSafe',
  'blueprintPrivateFieldKey',
  'sanitizeBlueprintProjection',
]

// Files that were former copies and must stay PURE re-exports (no definitions
// of ANY guarded symbol). These paths anchor the residual-drift check; if a new
// definition reappears here, the build fails.
const FORMER_COPY_FILES = [
  'packages/probe/packages/runtime/src/contracts/provider-account.ts',
  'apps/pylon/packages/runtime/src/contracts/provider-account.ts',
  'packages/probe/packages/runtime/src/blueprint/contracts.ts',
  'apps/pylon/packages/runtime/src/blueprint/contracts.ts',
  'apps/openagents.com/workers/api/src/blueprint/exports/contract-export.ts',
].map(p => join(repoRoot, p))

const read = path => readFileSync(path, 'utf8')

const definesSymbol = (text, symbol) =>
  definitionPatterns(symbol).some(re => re.test(text))

const problems = []

// 1. Canonical files must exist (the single authority must be present).
for (const file of CANONICAL_FILES) {
  if (!existsSync(file)) {
    problems.push(
      `canonical contract authority missing: ${relative(repoRoot, file)}`,
    )
  }
}

// 2. Walk the whole repo: no file outside the canonical set may DEFINE a
//    guarded symbol. Re-exports (export { X } / export * from) are fine.
const allFiles = listFiles(repoRoot)

for (const file of allFiles) {
  if (CANONICAL_FILES.has(file)) continue
  const text = read(file)
  for (const symbol of GUARDED_SYMBOLS) {
    if (definesSymbol(text, symbol)) {
      problems.push(
        `duplicate contract authority: ${relative(repoRoot, file)} DEFINES ` +
          `"${symbol}". This security contract has one canonical home ` +
          `(packages/provider-account-schema or packages/blueprint-contracts). ` +
          `Import/re-export from there instead of redefining it.`,
      )
    }
  }
}

// 3. Residual-drift anchor: the known former-copy files must still exist and
//    must NOT define any guarded symbol (defends against a silent re-fork even
//    if the generic walk above is ever loosened).
for (const file of FORMER_COPY_FILES) {
  if (!existsSync(file)) {
    problems.push(
      `expected re-export shim missing: ${relative(repoRoot, file)} ` +
        `(former contract copy; should re-export the canonical package).`,
    )
    continue
  }
  const text = read(file)
  for (const symbol of GUARDED_SYMBOLS) {
    if (definesSymbol(text, symbol)) {
      problems.push(
        `residual contract drift: ${relative(repoRoot, file)} re-defines ` +
          `"${symbol}". This file must stay a pure re-export of the canonical ` +
          `contract package.`,
      )
    }
  }
}

console.log('Security-contract drift guard')
console.log('')
console.log(`Canonical authorities:`)
for (const file of CANONICAL_FILES) {
  console.log(`  ${relative(repoRoot, file)}`)
}
console.log('')
console.log(`Scanned ${allFiles.length} source files for ${GUARDED_SYMBOLS.length} guarded symbols.`)
console.log('')

if (problems.length > 0) {
  console.error('Security-contract drift guard FAILED:')
  for (const problem of problems) console.error(`- ${problem}`)
  process.exit(1)
}

console.log('Security-contract drift guard passed: one authority, no drift.')
