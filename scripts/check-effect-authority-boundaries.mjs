#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join, relative } from 'node:path'

const root = process.cwd()
const allowlistPath = join(root, 'scripts/effect-authority-boundary-allowlist.json')

const scopes = [
  'apps/openagents.com/workers/api/src',
  'apps/pylon/src',
  'packages/agent-runtime-schema/src',
  'packages/atif/src',
  'packages/blueprint-contracts/src',
  'packages/durable-stream/src',
  'packages/mcp-contract/src',
  'packages/proof-replay/src',
  'packages/provider-account-schema/src',
  'packages/probe/packages/runtime/src',
  'packages/world-client/src',
  'packages/world-contract/src',
]

const ignoredPathFragments = [
  '/__fixtures__/',
  '/fixtures/',
  '/test/',
  '.generated.',
  '.test-support.',
  '.test.',
  '.spec.',
  '.d.ts',
]

const rules = [
  {
    id: 'raw-json-parse',
    label: 'raw JSON.parse boundary',
    test: (line, context) =>
      line.includes('JSON.parse(') &&
      /(\bas\b|typeof |Array\.isArray|Schema\.decode|Schema\.decodeUnknown|recordFromUnknown)/.test(
        `${line}\n${context.nextOne}\n${context.nextTwo}`,
      ),
  },
  {
    id: 'direct-env-read',
    label: 'direct process.env or Bun.env read',
    test: line => /\b(?:process|Bun)\.env\b/.test(line),
  },
  {
    id: 'bare-catch',
    label: 'bare catch block',
    test: line => /\bcatch\s*(?:\([^)]*\))?\s*\{\s*\}/.test(line),
  },
  {
    id: 'raw-fetch',
    label: 'raw fetch call',
    test: line =>
      /\b(?:await|return|=>|=|\?|:)\s*fetch\s*\(/.test(line) ||
      /\bfetch\s*\([^)]*,/.test(line),
  },
  {
    id: 'effect-run-promise-bridge',
    label: 'Effect.runPromise bridge',
    test: line => /\bEffect\.runPromise(?:Exit)?\s*\(/.test(line),
  },
]

function listFiles(directory) {
  if (!existsSync(directory)) return []
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) return listFiles(path)
    return [path]
  })
}

function readAllowlist() {
  if (!existsSync(allowlistPath)) return []
  const parsed = JSON.parse(readFileSync(allowlistPath, 'utf8'))
  if (!Array.isArray(parsed.entries)) {
    throw new Error('effect authority boundary allowlist must contain an entries array')
  }
  return parsed.entries.map((entry, index) => {
    for (const key of ['ruleId', 'path', 'reason']) {
      if (typeof entry[key] !== 'string' || entry[key].trim() === '') {
        throw new Error(`allowlist entry ${index} must include non-empty ${key}`)
      }
    }
    return {
      lineContains: typeof entry.lineContains === 'string' ? entry.lineContains : null,
      path: entry.path,
      reason: entry.reason,
      ruleId: entry.ruleId,
    }
  })
}

function isAllowed(finding, allowlist) {
  return allowlist.find(entry => {
    if (entry.ruleId !== finding.ruleId || entry.path !== finding.path) return false
    return entry.lineContains === null || finding.source.includes(entry.lineContains)
  })
}

const files = scopes
  .flatMap(scope => listFiles(join(root, scope)))
  .filter(path => /\.tsx?$/.test(path))
  .map(path => relative(root, path))
  .filter(path => !ignoredPathFragments.some(fragment => path.includes(fragment)))
  .sort()

const allowlist = readAllowlist()
const findings = []
const allowed = []

for (const path of files) {
  const lines = readFileSync(join(root, path), 'utf8').split('\n')
  lines.forEach((line, index) => {
    const trimmed = line.trim()
    if (
      trimmed === '' ||
      trimmed.startsWith('//') ||
      trimmed.startsWith('*') ||
      trimmed.startsWith('/*')
    ) {
      return
    }
    const context = {
      nextOne: lines[index + 1] ?? '',
      nextTwo: lines[index + 2] ?? '',
    }
    for (const rule of rules) {
      if (!rule.test(line, context)) continue
      const finding = {
        label: rule.label,
        line: index + 1,
        path,
        ruleId: rule.id,
        source: line.trim(),
      }
      const allowance = isAllowed(finding, allowlist)
      if (allowance) {
        allowed.push({ ...finding, reason: allowance.reason })
      } else {
        findings.push(finding)
      }
    }
  })
}

const byRule = new Map()
for (const finding of findings) {
  const bucket = byRule.get(finding.ruleId) ?? []
  bucket.push(finding)
  byRule.set(finding.ruleId, bucket)
}

if (process.argv.includes('--json')) {
  console.log(
    JSON.stringify(
      {
        allowedCount: allowed.length,
        findingCount: findings.length,
        findings,
        scannedFiles: files.length,
        scopes,
      },
      null,
      2,
    ),
  )
  process.exit(0)
}

console.log('Effect authority-boundary report (report-only)')
console.log(`Scanned files: ${files.length}`)
console.log(`Findings: ${findings.length}`)
console.log(`Allowlisted intentional edges: ${allowed.length}`)
console.log('')

for (const rule of rules) {
  const matches = byRule.get(rule.id) ?? []
  console.log(`${rule.id} (${rule.label}): ${matches.length}`)
  for (const finding of matches) {
    console.log(`  ${finding.path}:${finding.line} ${finding.source}`)
  }
  console.log('')
}

if (allowed.length > 0) {
  console.log('Allowlisted edges:')
  for (const finding of allowed) {
    console.log(`  ${finding.path}:${finding.line} ${finding.ruleId} - ${finding.reason}`)
  }
  console.log('')
}

console.log('Report-only: this command exits 0 so existing findings are migration inventory.')
