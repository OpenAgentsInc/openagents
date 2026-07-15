#!/usr/bin/env node

import { readFileSync, readdirSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const expectedEffect = '4.0.0-beta.94'
const appRoot = resolve(fileURLToPath(new URL('..', import.meta.url)))
const repoRoot = resolve(appRoot, '..', '..')
const skipped = new Set(['.git', '.turbo', 'coverage', 'dist', 'node_modules', 'projects'])
const prohibited = new Set([
  '@effect/sql-d1',
  '@effect/sql-sqlite-do',
  'effect-cf',
])

const packageFiles = []
const visit = directory => {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (skipped.has(entry.name)) {
      continue
    }
    const path = join(directory, entry.name)
    if (entry.isDirectory()) {
      visit(path)
    } else if (entry.name === 'package.json') {
      packageFiles.push(path)
    }
  }
}
visit(repoRoot)

const problems = []
for (const path of packageFiles) {
  const manifest = JSON.parse(readFileSync(path, 'utf8'))
  for (const section of ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies']) {
    const dependencies = manifest[section] ?? {}
    const effect = dependencies.effect
    if (effect !== undefined && effect !== 'catalog:' && effect !== expectedEffect) {
      problems.push(`${relative(repoRoot, path)} ${section}.effect=${effect}`)
    }
    for (const dependency of Object.keys(dependencies)) {
      if (prohibited.has(dependency) || dependency.startsWith('@cloudflare/')) {
        problems.push(`${relative(repoRoot, path)} retains ${section}.${dependency}`)
      }
    }
  }
}

const workspace = readFileSync(join(repoRoot, 'pnpm-workspace.yaml'), 'utf8')
if (!workspace.includes(`  effect: ${expectedEffect}`)) {
  problems.push(`pnpm-workspace.yaml must catalog effect@${expectedEffect}`)
}

if (problems.length > 0) {
  console.error(['Effect/runtime topology check failed:', ...problems.map(problem => `- ${problem}`)].join('\n'))
  process.exit(1)
}

console.log(`Effect/runtime topology OK: effect@${expectedEffect}; no vendor-edge packages`)
