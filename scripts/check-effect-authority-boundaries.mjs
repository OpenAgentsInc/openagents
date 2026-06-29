#!/usr/bin/env bun

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { allowedEdges } from './effect-authority-boundary-allowlist.mjs'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

export const authorityRoots = [
  'apps/openagents.com/workers/api/src',
  'apps/openagents-world/src',
  'apps/pylon/src',
  'packages/agent-runtime-schema/src',
  'packages/atif/src',
  'packages/durable-stream/src',
  'packages/nip90/src',
  'packages/probe/packages/runtime/src',
  'packages/proof-replay/src',
  'packages/provider-account-schema/src',
  'packages/world-client/src',
  'packages/world-contract/src',
]

const ignoredDirs = new Set([
  '.git',
  '.turbo',
  '.wrangler',
  'build',
  'coverage',
  'dist',
  'node_modules',
  'target',
])

const sourceExtensions = new Set([
  '.js',
  '.jsx',
  '.mjs',
  '.cjs',
  '.ts',
  '.tsx',
  '.mts',
  '.cts',
])

const findingLabels = {
  'bare-catch': 'bare catch {}',
  'effect-run-promise': 'Effect.runPromise bridge',
  env: 'direct process.env/Bun.env',
  fetch: 'raw fetch',
  'json-parse-cast': 'JSON.parse near cast/manual narrowing',
}

const extensionOf = path => {
  const match = path.match(/\.[^.]+$/)
  return match?.[0] ?? ''
}

const isSourceFile = path => {
  if (!sourceExtensions.has(extensionOf(path))) return false
  if (/\.(test|spec)\.[cm]?[jt]sx?$/.test(path)) return false
  if (/\.test-support\.[cm]?[jt]sx?$/.test(path)) return false
  if (path.includes('/__tests__/') || path.includes('/fixtures/')) return false
  return true
}

const listFiles = dir => {
  let entries
  try {
    entries = readdirSync(dir, { withFileTypes: true })
  } catch {
    return []
  }

  return entries.flatMap(entry => {
    const path = join(dir, entry.name)

    if (entry.isDirectory()) {
      if (ignoredDirs.has(entry.name)) return []
      return listFiles(path)
    }

    if (!entry.isFile() || !isSourceFile(path)) return []
    return [path]
  })
}

const lineHasJsonCast = (lines, index) => {
  const window = lines.slice(index, index + 4).join('\n')
  return (
    /\bJSON\.parse\s*\(/.test(lines[index]) &&
    (/\bas\s+[A-Za-z_{]/.test(window) ||
      /\bsatisfies\s+[A-Za-z_{]/.test(window) ||
      /\btypeof\b/.test(window) ||
      /\bArray\.isArray\s*\(/.test(window))
  )
}

const lineStartsBareCatch = (lines, index) => {
  const line = lines[index]
  if (!/\bcatch\s*(?:\([^)]*\))?\s*\{/.test(line)) return false
  if (/\bcatch\s*(?:\([^)]*\))?\s*\{\s*\}/.test(line)) return true

  for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
    const trimmed = lines[cursor].trim()
    if (trimmed === '') continue
    return trimmed === '}'
  }

  return false
}

const rawFetchPattern = /(^|[^\w$.])fetch\s*\(/

const isFetchMethodSignature = line =>
  /^\s*(?:async\s+)?fetch\s*\([^)]*:\s*[^)]*\)\s*(?:[:{]|=>)/.test(line)

export const scanText = (text, path) => {
  const lines = text.split('\n')
  const findings = []

  lines.forEach((line, index) => {
    const trimmed = line.trim()
    if (
      trimmed.startsWith('//') ||
      trimmed.startsWith('*') ||
      trimmed.startsWith('/*')
    ) {
      return
    }

    const base = {
      line: trimmed.slice(0, 140),
      lineNumber: index + 1,
      path,
    }

    if (lineHasJsonCast(lines, index)) {
      findings.push({
        ...base,
        kind: 'json-parse-cast',
      })
    }

    if (/\b(?:process\.env|Bun\.env)\b/.test(line)) {
      findings.push({
        ...base,
        kind: 'env',
      })
    }

    if (lineStartsBareCatch(lines, index)) {
      findings.push({
        ...base,
        kind: 'bare-catch',
      })
    }

    if (rawFetchPattern.test(line) && !isFetchMethodSignature(line)) {
      findings.push({
        ...base,
        kind: 'fetch',
      })
    }

    if (/\bEffect\.runPromise(?:Exit)?\s*\(/.test(line)) {
      findings.push({
        ...base,
        kind: 'effect-run-promise',
      })
    }
  })

  return findings
}

const allowlistMatches = finding =>
  allowedEdges.find(edge => {
    if (edge.kind !== finding.kind) return false
    if (edge.path !== finding.path) return false
    if (edge.includes && !finding.line.includes(edge.includes)) return false
    return true
  })

export const scanAuthorityBoundaries = (roots = authorityRoots) => {
  const scannedFiles = []
  const findings = []
  const allowed = []

  for (const root of roots) {
    const absoluteRoot = join(repoRoot, root)
    if (!existsSync(absoluteRoot)) continue

    try {
      if (!statSync(absoluteRoot).isDirectory()) continue
    } catch {
      continue
    }

    for (const file of listFiles(absoluteRoot)) {
      const path = relative(repoRoot, file)
      scannedFiles.push(path)

      let text
      try {
        text = readFileSync(file, 'utf8')
      } catch {
        continue
      }

      for (const finding of scanText(text, path)) {
        const allowlistEntry = allowlistMatches(finding)
        if (allowlistEntry) {
          allowed.push({ ...finding, reason: allowlistEntry.reason })
        } else {
          findings.push(finding)
        }
      }
    }
  }

  return {
    allowed,
    findings,
    scannedFiles,
  }
}

const countByKind = findings =>
  findings.reduce((counts, finding) => {
    counts.set(finding.kind, (counts.get(finding.kind) ?? 0) + 1)
    return counts
  }, new Map())

const printSummary = (title, findings) => {
  console.log(title)

  if (findings.length === 0) {
    console.log('  none')
    return
  }

  for (const [kind, count] of [...countByKind(findings)].sort()) {
    console.log(`  ${findingLabels[kind] ?? kind}: ${count}`)
  }
}

const isMain =
  import.meta.url === `file://${process.argv[1]}` ||
  process.argv[1]?.endsWith('check-effect-authority-boundaries.mjs')

if (isMain) {
  const maxArg = process.argv.find(arg => arg.startsWith('--max='))
  const maxFindings = maxArg ? Number(maxArg.slice('--max='.length)) : 200
  const { allowed, findings, scannedFiles } = scanAuthorityBoundaries()
  const visibleFindings = findings.slice(0, maxFindings)

  console.log('Effect authority-boundary report (report-only)')
  console.log(`Scanned ${scannedFiles.length} source file(s).`)
  printSummary('Migration inventory:', findings)
  printSummary('Allowed raw edge inventory:', allowed)

  if (visibleFindings.length > 0) {
    console.log('')
    console.log(
      `First ${visibleFindings.length} finding(s), capped with --max=${maxFindings}:`,
    )
    for (const finding of visibleFindings) {
      console.log(
        `  ${finding.path}:${finding.lineNumber}: ${findingLabels[finding.kind]}: ${finding.line}`,
      )
    }
  }

  if (findings.length > visibleFindings.length) {
    console.log(
      `  ... ${findings.length - visibleFindings.length} additional finding(s) omitted`,
    )
  }

  console.log('')
  console.log(
    'Report-only: existing matches are migration inventory and do not fail this command.',
  )
}
