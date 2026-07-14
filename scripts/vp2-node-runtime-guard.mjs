#!/usr/bin/env node

/**
 * VP-2 retained-runtime guard (#8796).
 *
 * Test-framework imports and repository automation move in VP-3/VP-4, and
 * production images move in VP-5. This gate protects the narrower VP-2
 * contract: retained application/package source entrypoints must not regain a
 * Bun global, Bun module import, or Bun shebang. The dual-runtime SQLite oracle
 * is the one named exception and is deleted in VP-6.
 */

import { execFileSync } from "node:child_process"
import { existsSync, readFileSync } from "node:fs"
import { extname, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const SOURCE_ROOT = /^(?:apps|clients|packages)\//
const SOURCE_PATH = /(?:^|\/)src\//
const SOURCE_EXTENSION = new Set([".cjs", ".cts", ".js", ".jsx", ".mjs", ".mts", ".ts", ".tsx"])
const EXCLUDED = /^(?:packages\/harness-conformance\/)|(?:^|\/)(?:dist|generated|test|tests|__tests__|conformance)(?:\/|$)|\.(?:test|spec|testkit)\.[cm]?[jt]sx?$/
const SQLITE_ORACLE = "packages/sqlite-runtime/src/bun-database.ts"

const patterns = [
  ["bun-global", /\bBun\s*\./],
  ["bun-module", /^(?:\s*(?:import|export)\b[^\n]*\bfrom\s*|\s*(?:import|require)\s*\()\s*["']bun(?::[^"']*)?["']/m],
  ["bun-shebang", /^#![^\n]*\benv\s+(?:-S\s+)?bun\b/m],
]

const trackedFiles = (root) =>
  execFileSync("git", ["ls-files", "-z"], { cwd: root, encoding: "utf8" })
    .split("\0")
    .filter(Boolean)

export const scanVp2NodeRuntime = (root) => {
  const findings = []
  for (const path of trackedFiles(root)) {
    if (
      path === SQLITE_ORACLE ||
      !SOURCE_ROOT.test(path) ||
      !SOURCE_PATH.test(path) ||
      EXCLUDED.test(path) ||
      !SOURCE_EXTENSION.has(extname(path))
    ) continue

    const absolute = resolve(root, path)
    if (!existsSync(absolute)) continue
    const source = readFileSync(absolute, "utf8")
    for (const [category, pattern] of patterns) {
      const match = pattern.exec(source)
      if (match === null) continue
      findings.push({
        category,
        path,
        line: source.slice(0, match.index).split("\n").length,
      })
    }
  }
  return findings
}

const isMain = process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)
if (isMain) {
  const root = resolve(process.argv[2] ?? process.cwd())
  const findings = scanVp2NodeRuntime(root)
  console.log(`VP-2 retained Node runtime guard: ${findings.length} violation(s)`)
  for (const finding of findings) {
    console.log(`${finding.category} ${finding.path}:${finding.line}`)
  }
  process.exitCode = findings.length === 0 ? 0 : 1
}
