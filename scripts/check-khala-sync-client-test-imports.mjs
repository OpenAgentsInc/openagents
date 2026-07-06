#!/usr/bin/env node
import { readdirSync, readFileSync, statSync } from "node:fs"
import { dirname, extname, join, normalize, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const packageRoot = join(repoRoot, "packages", "khala-sync-client")
const srcRoot = join(packageRoot, "src")

const walk = (dir) => {
  const out = []
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    const stat = statSync(full)
    if (stat.isDirectory()) out.push(...walk(full))
    else if (stat.isFile()) out.push(full)
  }
  return out
}

const normalizePath = (path) => normalize(path)

const sourceFiles = new Set(
  walk(srcRoot)
    .filter((file) => extname(file) === ".ts")
    .filter((file) => !file.endsWith(".test.ts"))
    .filter((file) => !file.endsWith(".testkit.ts"))
    .map(normalizePath),
)

const testFiles = walk(srcRoot)
  .filter((file) => file.endsWith(".test.ts"))
  .map(normalizePath)

const resolveImport = (fromFile, specifier) => {
  if (!specifier.startsWith(".")) return null
  const base = resolve(dirname(fromFile), specifier)
  const candidates = []
  if (specifier.endsWith(".js")) {
    candidates.push(`${base.slice(0, -3)}.ts`)
  } else if (specifier.endsWith(".ts")) {
    candidates.push(base)
  } else {
    candidates.push(`${base}.ts`, join(base, "index.ts"))
  }
  return candidates.map(normalizePath).find((candidate) => sourceFiles.has(candidate)) ?? null
}

const staticImportPattern =
  /^\s*import\s+(?!type\b)(?:[\s\S]*?)\s+from\s+["']([^"']+)["']/gm
const sideEffectImportPattern = /^\s*import\s+["']([^"']+)["']/gm
const dynamicImportPattern = /import\s*\(\s*["']([^"']+)["']\s*\)/g

const covered = new Set()
for (const testFile of testFiles) {
  const text = readFileSync(testFile, "utf8")
  for (const pattern of [
    staticImportPattern,
    sideEffectImportPattern,
    dynamicImportPattern,
  ]) {
    pattern.lastIndex = 0
    let match
    while ((match = pattern.exec(text)) !== null) {
      const resolved = resolveImport(testFile, match[1])
      if (resolved !== null) covered.add(resolved)
    }
  }
}

const missing = [...sourceFiles]
  .filter((file) => !covered.has(file))
  .map((file) => relative(repoRoot, file))
  .sort()

if (missing.length > 0) {
  console.error(
    "khala-sync-client test-import guard FAILED: source modules without direct non-type test imports:",
  )
  for (const file of missing) console.error(`- ${file}`)
  console.error(
    "Add a focused test import for each module, or split non-runtime test helpers into *.testkit.ts.",
  )
  process.exit(1)
}

console.log(
  `khala-sync-client test-import guard passed: ${sourceFiles.size} source modules covered by ${testFiles.length} test files.`,
)
