#!/usr/bin/env node

/**
 * ST-2 (#8508): zero-test-imports guard for @openagentsinc/khala-sync-client.
 *
 * Fails when any `src/` module is imported by ZERO test files. This is the
 * exact hole behind the 2026-07-06 mobile WebSocket-auth incident:
 * `src/transport.ts` — the file that talks to real servers — shipped for
 * weeks with no test importing it, so its query-token auth convention was
 * never asserted against anything
 * (docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md §R2).
 *
 * Mechanism (kept deliberately simple, same family as the
 * apps/openagents.com `check-*` deploy-gate scripts):
 *
 * 1. Import diff — collect every `src/**` `*.ts` module, parse the import
 *    specifiers out of every `*.test.ts` file, resolve the relative ones,
 *    and fail listing any module no test references.
 * 2. Seam-module tier — the incident survived rule 1 (session.test.ts
 *    always imported `KhalaSyncTransportError` from transport.js while the
 *    production factory went unexercised), so modules that talk to real
 *    networks additionally require a DEDICATED co-located `<name>.test.ts`
 *    that imports them, and every value they export (const/function/class)
 *    must be referenced by at least one test file.
 *
 * Wired into: this package's `check:test-import-coverage` script and the
 * apps/openagents.com `check:deploy` chain (so it gates deploys).
 */

import { readdirSync, readFileSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const srcRoot = join(packageRoot, 'src')

/**
 * Modules that CANNOT be imported by a bun test, with the reason on record.
 * Keep this list justified and short — every entry is an untested-network-
 * boundary risk until proven otherwise.
 */
const ALLOWLIST = new Map([
  [
    'web/sqlite-wasm-worker.ts',
    'storage-worker entrypoint: the only module that imports the ' +
      '@sqlite.org/sqlite-wasm bundle; importing it under bun test would ' +
      'pull the WASM runtime outside a worker. Its composition parts ' +
      '(wasm-driver, worker-runtime, worker-server) are all test-imported.',
  ],
])

/**
 * Network-boundary seam modules. A type-or-error import from another test
 * is NOT enough for these — that exact loophole is how transport.ts stayed
 * effectively untested through the incident. Each must have a dedicated
 * co-located `<name>.test.ts` importing it, and every exported value must
 * appear in at least one test file.
 */
const SEAM_MODULES = ['transport.ts']

const walk = dir => {
  const files = []
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) {
      files.push(...walk(path))
    } else if (entry.isFile() && entry.name.endsWith('.ts')) {
      files.push(path)
    }
  }
  return files
}

const isTestFile = path => path.endsWith('.test.ts')

const importSpecifiers = source => {
  const specifiers = []
  const patterns = [
    /(?:^|[^\w.])import\s+[^"'()]*?from\s*["']([^"']+)["']/gm, // import x from "..."
    /(?:^|[^\w.])import\s*["']([^"']+)["']/gm, // bare side-effect import "..."
    /(?:^|[^\w.])export\s+[^"']*?from\s*["']([^"']+)["']/gm, // re-export from "..."
    /import\s*\(\s*["']([^"']+)["']\s*\)/gm, // dynamic import("...")
  ]
  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) {
      specifiers.push(match[1])
    }
  }
  return specifiers
}

/** Resolve a relative specifier from a test file to a src/*.ts module path. */
const resolveRelative = (fromFile, specifier) => {
  const base = resolve(dirname(fromFile), specifier)
  const candidates = [
    base.replace(/\.js$/, '.ts'),
    base.endsWith('.ts') ? base : `${base}.ts`,
    join(base, 'index.ts'),
  ]
  return candidates
}

const main = () => {
  const allFiles = walk(srcRoot)
  const modules = allFiles.filter(path => !isTestFile(path))
  const testFiles = allFiles.filter(isTestFile)

  if (testFiles.length === 0) {
    console.error(
      '✘ khala-sync-client test-import coverage: no *.test.ts files found ' +
        'under src/ — the package has lost its entire test suite.',
    )
    process.exit(1)
  }

  const imported = new Set()
  for (const testFile of testFiles) {
    const source = readFileSync(testFile, 'utf8')
    for (const specifier of importSpecifiers(source)) {
      if (!specifier.startsWith('.')) continue
      for (const candidate of resolveRelative(testFile, specifier)) {
        imported.add(candidate)
      }
    }
  }

  const uncovered = []
  for (const module of modules) {
    const rel = relative(srcRoot, module)
    if (ALLOWLIST.has(rel)) continue
    if (!imported.has(module)) {
      uncovered.push(rel)
    }
  }

  // Seam-module tier (see header): dedicated test file + exported-value refs.
  const seamFailures = []
  const allTestSource = testFiles
    .map(testFile => readFileSync(testFile, 'utf8'))
    .join('\n')
  for (const rel of SEAM_MODULES) {
    const modulePath = join(srcRoot, rel)
    const dedicatedTest = modulePath.replace(/\.ts$/, '.test.ts')
    if (!testFiles.includes(dedicatedTest)) {
      seamFailures.push(
        `src/${rel}: missing its dedicated test file ` +
          `src/${rel.replace(/\.ts$/, '.test.ts')} (network-boundary seam ` +
          'modules require one; an incidental type/error import from ' +
          'another test does not count).',
      )
      continue
    }
    const dedicatedSource = readFileSync(dedicatedTest, 'utf8')
    const importsModule = importSpecifiers(dedicatedSource).some(specifier => {
      if (!specifier.startsWith('.')) return false
      return resolveRelative(dedicatedTest, specifier).includes(modulePath)
    })
    if (!importsModule) {
      seamFailures.push(
        `src/${rel}: its dedicated test file exists but does not import it.`,
      )
    }
    const moduleSource = readFileSync(modulePath, 'utf8')
    const exportedValues = [
      ...moduleSource.matchAll(
        /^export\s+(?:const|function|class)\s+([A-Za-z0-9_$]+)/gm,
      ),
    ].map(match => match[1])
    for (const name of exportedValues) {
      if (!allTestSource.includes(name)) {
        seamFailures.push(
          `src/${rel}: exported value \`${name}\` is referenced by no test file.`,
        )
      }
    }
  }

  if (seamFailures.length > 0) {
    console.error(
      '✘ khala-sync-client test-import coverage (seam tier): the modules ' +
        'that talk to real servers need dedicated, exercising tests (#8508):',
    )
    for (const failure of seamFailures) {
      console.error(`  - ${failure}`)
    }
    process.exit(1)
  }

  if (uncovered.length > 0) {
    console.error(
      '✘ khala-sync-client test-import coverage: the following src/ modules ' +
        'are imported by ZERO test files. Untested modules on this ' +
        'network-boundary package are how the 2026-07-06 WebSocket-auth ' +
        'incident shipped (#8508). Add a test that imports each module ' +
        '(directly exercising it), or — only with a recorded justification — ' +
        'add it to the ALLOWLIST in this script:',
    )
    for (const rel of uncovered.sort()) {
      console.error(`  - src/${rel}`)
    }
    process.exit(1)
  }

  console.log(
    `✔ khala-sync-client test-import coverage: ${modules.length} src modules, ` +
      `${testFiles.length} test files, 0 uncovered, ` +
      `${SEAM_MODULES.length} seam module(s) with dedicated tests ` +
      `(${ALLOWLIST.size} allowlisted: ${[...ALLOWLIST.keys()].join(', ')}).`,
  )
}

main()
