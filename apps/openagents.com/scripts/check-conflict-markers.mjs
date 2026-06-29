#!/usr/bin/env bun

// Guard: fail the deploy gate if any unresolved Git merge-conflict marker is
// left in tracked source. A marker accidentally committed inside a TypeScript
// string array (e.g. the product-promises registry changelog) is a syntax
// error in code, but markers can also land in docs/JSON where nothing else in
// check:deploy would notice. This is a fast, format-agnostic line grep.

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

// Directories scanned, relative to apps/openagents.com. Covers the worker API
// source, the browser app source, all packages, and the docs tree.
export const scanRoots = ['workers/api/src', 'apps/web/src', 'packages', 'docs']

const ignoredDirs = new Set([
  'node_modules',
  'dist',
  'build',
  '.next',
  'coverage',
  '.turbo',
  '.wrangler',
  'target',
])

const listFiles = dir => {
  let entries
  try {
    entries = readdirSync(dir, { withFileTypes: true })
  } catch {
    return []
  }

  return entries.flatMap(entry => {
    if (entry.isDirectory()) {
      if (ignoredDirs.has(entry.name)) return []

      return listFiles(join(dir, entry.name))
    }

    if (!entry.isFile()) return []

    return [join(dir, entry.name)]
  })
}

// Build the marker matchers without writing literal marker lines at column 0 in
// this file, so the guard never trips on its own source.
const open = '<'.repeat(7)
const middle = '='.repeat(7)
const close = '>'.repeat(7)

// `<<<<<<< ` and `>>>>>>> ` always carry a branch/ref label after the marker,
// so they are unambiguous on their own. A bare `=======` line, however, is also
// a Markdown Setext heading underline, so it is only treated as a conflict
// divider when it sits inside an open `<<<<<<<` ... `>>>>>>>` block.
export const openPattern = new RegExp('^' + open + ' ')
export const middlePattern = new RegExp('^' + middle + '$')
export const closePattern = new RegExp('^' + close + ' ')

// Retained for tests: the unambiguous (always-a-marker) patterns.
export const markerPatterns = [openPattern, closePattern]

// Pure, testable: scan a single text body and report marker lines (1-based).
// `=======` is reported only when an unterminated `<<<<<<<` precedes it, which
// distinguishes a real conflict divider from a Markdown Setext heading rule.
export const scanText = (text, path = '<text>') => {
  const lines = text.split('\n')
  const findings = []
  let insideConflict = false

  lines.forEach((line, index) => {
    const finding = {
      line: line.slice(0, 80),
      lineNumber: index + 1,
      path,
    }

    if (openPattern.test(line)) {
      insideConflict = true
      findings.push(finding)
      return
    }

    if (closePattern.test(line)) {
      insideConflict = false
      findings.push(finding)
      return
    }

    if (middlePattern.test(line) && insideConflict) {
      findings.push(finding)
    }
  })

  return findings
}

// Walk the scan roots on disk and collect every marker finding.
export const scanForConflictMarkers = (roots = scanRoots) => {
  const findings = []

  for (const root of roots) {
    let exists = true
    try {
      statSync(root)
    } catch {
      exists = false
    }
    if (!exists) continue

    for (const path of listFiles(root)) {
      let text
      try {
        text = readFileSync(path, 'utf8')
      } catch {
        continue
      }

      findings.push(...scanText(text, relative('.', path)))
    }
  }

  return findings
}

const isMain =
  import.meta.url === `file://${process.argv[1]}` ||
  process.argv[1]?.endsWith('check-conflict-markers.mjs')

if (isMain) {
  const findings = scanForConflictMarkers()

  if (findings.length > 0) {
    console.error('check:conflict-markers FAILED')
    console.error(
      `Found ${findings.length} unresolved merge-conflict marker line(s):`,
    )
    for (const finding of findings) {
      console.error(`  ${finding.path}:${finding.lineNumber}: ${finding.line}`)
    }
    process.exit(1)
  }

  console.log(
    'check:conflict-markers OK (no merge-conflict markers in ' +
      scanRoots.join(', ') +
      ')',
  )
}
