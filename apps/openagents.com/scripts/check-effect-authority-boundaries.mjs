#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDir = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(scriptDir, '../../..')

const authorityRoots = [
  {
    path: 'apps/openagents.com/workers/api/src',
    reason:
      'Worker routes and services decide auth, routing, payment, proof, settlement, and public projections.',
  },
  {
    path: 'apps/openagents.com/workers/api/scripts',
    reason:
      'Worker-adjacent operator scripts publish or verify authority evidence and must keep raw edges visible.',
  },
  {
    path: 'apps/pylon/src',
    reason:
      'Pylon owns local assignment execution, account materialization, proof closeout, and contributor capacity.',
  },
  {
    path: 'packages/atif/src',
    reason:
      'ATIF owns owner-only trace shape and redaction before any public-safe projection.',
  },
  {
    path: 'packages/world-contract/src',
    reason:
      'World contracts define public-safe command, delta, cursor, and projection schemas.',
  },
  {
    path: 'packages/world-client/src',
    reason:
      'World client mirrors live public projection state from transport boundaries into a read model.',
  },
  {
    path: 'packages/provider-account-schema/src',
    reason:
      'Provider-account schemas carry auth/account state across Worker, Pylon, and client boundaries.',
  },
  {
    path: 'packages/probe/packages/runtime/src',
    reason:
      'Probe runtime owns external-provider, contribution, telemetry, and verification contracts.',
  },
  {
    path: 'packages/nip90/src',
    reason:
      'NIP-90 package models labor/request/closeout contracts used by paid and no-spend work routing.',
  },
  {
    path: 'packages/agent-runtime-schema/src',
    reason:
      'Agent runtime schemas are the shared event boundary for raw executor telemetry.',
  },
]

const skippedDirectoryNames = new Set([
  'dist',
  'node_modules',
  '.git',
  '.wrangler',
  '.turbo',
  'coverage',
])

const sourceFilePattern = /\.(?:mjs|mts|ts|tsx)$/
const excludedFilePattern =
  /\.(?:test|test-support|story\.test|scene\.test)\.(?:ts|tsx)$/

const rawEdgeRules = [
  {
    category: 'raw-json-parse',
    description:
      'JSON.parse in authority paths should move behind Effect Schema decoders before domain logic consumes it.',
    find: (text, lines) =>
      matchesForRegex(text, /JSON\.parse\s*\(/g).map(match => {
        const window = lines
          .slice(Math.max(0, match.line - 1), Math.min(lines.length, match.line + 2))
          .join('\n')
        const hasCastOrManualNarrowing =
          /\bas\b|satisfies|Record<|unknown|typeof\s+|Array\.isArray|\bin\s+/.test(
            window,
          )
        return {
          ...match,
          detail: hasCastOrManualNarrowing
            ? 'JSON.parse with nearby cast/manual narrowing'
            : 'JSON.parse raw boundary',
        }
      }),
  },
  {
    category: 'direct-env-read',
    description:
      'process.env and Bun.env reads should be entry-edge config loading, then provided through services/layers.',
    find: text => matchesForRegex(text, /\b(?:process|Bun)\.env\b/g),
  },
  {
    category: 'bare-catch',
    description:
      'Bare catch blocks in authority paths erase whether failure was absent, malformed, transient, or a defect.',
    find: text =>
      matchesForRegex(
        text,
        /catch\s*(?:\([^)]*\))?\s*{\s*(?:(?:\/\/[^\n]*\n|\/\*[\s\S]*?\*\/|\s)*)}/g,
      ),
  },
  {
    category: 'raw-fetch',
    description:
      'Raw fetch should be an injected platform/client adapter with typed timeout, retry, and error mapping.',
    find: text => matchesForRegex(text, /\b(?:globalThis\.)?fetch\s*\(/g),
  },
  {
    category: 'effect-runpromise-bridge',
    description:
      'Effect.runPromise bridges should stay at CLI, Worker, test, or script entry edges.',
    find: text => matchesForRegex(text, /Effect\.runPromise(?:Exit)?\s*\(/g),
  },
]

const allowlist = [
  {
    categories: ['effect-runpromise-bridge'],
    path: 'apps/pylon/src/index.ts',
    reason:
      'Pylon CLI entrypoint is allowed to bridge the final composed program into the process runtime.',
  },
  {
    categories: ['direct-env-read'],
    path: 'apps/pylon/src/bootstrap.ts',
    reason:
      'Current Pylon bootstrap accepts process env as its transitional CLI config edge; migrate to a config service before moving this inward.',
  },
  {
    categories: ['effect-runpromise-bridge'],
    path: 'apps/openagents.com/workers/api/src/index.ts',
    reason:
      'Worker entry routing may bridge final request Effects while route internals migrate behind service boundaries.',
  },
  {
    categories: ['direct-env-read'],
    pathPrefix: 'apps/openagents.com/workers/api/scripts/',
    reason:
      'Operator scripts are process entrypoints; env reads are allowed only for loading operator tokens/config before typed requests.',
  },
  {
    categories: ['raw-fetch'],
    pathPrefix: 'apps/openagents.com/workers/api/scripts/',
    reason:
      'Operator scripts call public or operator HTTP APIs directly today; each call remains reported when not behind a Worker service.',
  },
  {
    categories: ['raw-json-parse'],
    path: 'apps/openagents.com/workers/api/src/json-boundary.ts',
    reason:
      'This is the named Worker JSON boundary helper; raw parse is intentional only here while callers migrate to schema-backed decoders.',
  },
  {
    categories: ['raw-json-parse'],
    path: 'packages/probe/packages/runtime/src/llm/openrouter.ts',
    reason:
      'OpenRouter client is a known good local example for typed retry/config/error discipline; raw JSON is kept visible until the SDK boundary is fully schema-decoded.',
  },
]

const read = path => readFileSync(path, 'utf8')

const listFiles = dir => {
  if (!existsSync(dir)) {
    return []
  }

  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name)

    if (entry.isDirectory()) {
      return skippedDirectoryNames.has(entry.name) ? [] : listFiles(path)
    }

    return [path]
  })
}

const lineForIndex = (text, index) => text.slice(0, index).split('\n').length

const matchesForRegex = (text, regex) =>
  Array.from(text.matchAll(regex)).map(match => ({
    column: match.index - text.lastIndexOf('\n', match.index - 1),
    line: lineForIndex(text, match.index),
    snippet: firstLine(match[0]),
  }))

const firstLine = value => value.replace(/\s+/g, ' ').trim().slice(0, 160)

const isAllowed = finding =>
  allowlist.find(entry => {
    const categoryMatches = entry.categories.includes(finding.category)
    const pathMatches =
      (entry.path !== undefined && entry.path === finding.path) ||
      (entry.pathPrefix !== undefined && finding.path.startsWith(entry.pathPrefix))

    return categoryMatches && pathMatches
  })

const files = authorityRoots
  .flatMap(root => listFiles(resolve(repoRoot, root.path)))
  .filter(path => sourceFilePattern.test(path))
  .filter(path => !excludedFilePattern.test(path))
  .filter(path => !path.includes('/test/'))
  .sort()

const findings = files.flatMap(file => {
  const text = read(file)
  const lines = text.split('\n')
  const path = relative(repoRoot, file)

  return rawEdgeRules.flatMap(rule =>
    rule.find(text, lines).map(match => ({
      ...match,
      category: rule.category,
      description: rule.description,
      path,
    })),
  )
})

const decoratedFindings = findings.map(finding => ({
  ...finding,
  allowlistEntry: isAllowed(finding),
}))

const activeFindings = decoratedFindings.filter(
  finding => finding.allowlistEntry === undefined,
)
const allowedFindings = decoratedFindings.filter(
  finding => finding.allowlistEntry !== undefined,
)

const byCategory = rawEdgeRules.map(rule => ({
  ...rule,
  active: activeFindings.filter(finding => finding.category === rule.category),
  allowed: allowedFindings.filter(finding => finding.category === rule.category),
}))

console.log('Effect authority-boundary report-only scan')
console.log('')
console.log('Declared authority roots:')
authorityRoots.forEach(root => {
  console.log(`- ${root.path}`)
  console.log(`  ${root.reason}`)
})
console.log('')
console.log(`Files scanned: ${files.length}`)
console.log(`Migration inventory findings: ${activeFindings.length}`)
console.log(`Allowlisted raw edges: ${allowedFindings.length}`)
console.log('')

byCategory.forEach(category => {
  console.log(`${category.category}: ${category.active.length} migration item(s), ${category.allowed.length} allowlisted`)
  console.log(`  ${category.description}`)

  if (category.active.length === 0) {
    console.log('  migration inventory: none')
  } else {
    category.active.forEach(finding => {
      const detail = finding.detail === undefined ? '' : ` (${finding.detail})`
      console.log(
        `  ${finding.path}:${finding.line}:${finding.column}${detail}: ${finding.snippet}`,
      )
    })
  }

  if (category.allowed.length > 0) {
    console.log('  allowlisted edges:')
    category.allowed.forEach(finding => {
      console.log(
        `  ${finding.path}:${finding.line}:${finding.column}: ${finding.snippet}`,
      )
      console.log(`    reason: ${finding.allowlistEntry.reason}`)
    })
  }

  console.log('')
})

console.log(
  'Report-only mode: existing findings are migration inventory and do not fail this command.',
)
