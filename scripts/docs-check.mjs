#!/usr/bin/env node

import { execSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"

function gitLsFiles() {
  const out = execSync("git ls-files -z", { stdio: ["ignore", "pipe", "inherit"] })
  return out
    .toString("utf8")
    .split("\0")
    .map((s) => s.trim())
    .filter(Boolean)
}

function isExternalLink(target) {
  return (
    target.startsWith("http://") ||
    target.startsWith("https://") ||
    target.startsWith("mailto:") ||
    target.startsWith("tel:")
  )
}

function resolveLink(fromFile, target) {
  const file = target.split("#")[0]
  if (!file) return null
  if (file.startsWith("/")) return null
  if (file.startsWith("~")) return null
  return path.resolve(path.dirname(fromFile), file)
}

function normalizePathCandidate(raw) {
  // Strip common line/anchor suffixes: foo.ts:12:3, foo.md#L10, etc.
  const noHash = raw.split("#")[0]
  const noLine = noHash.split(":")[0]
  return noLine
}

const tracked = gitLsFiles()

// Scope: repository knowledge base (docs/) + top-level entry points.
const mdFiles = tracked.filter(
  (f) =>
    (f === "README.md" || f === "AGENTS.md" || f.startsWith("docs/") && f.endsWith(".md")) &&
    !f.startsWith("docs/plans/archived/"),
)

/** @type {Array<{file: string, message: string}>} */
const errors = []

// 1) Validate markdown links resolve.
for (const file of mdFiles) {
  const content = fs.readFileSync(file, "utf8")

  const linkRe = /\[[^\]]*\]\(([^)]+)\)/g
  for (const match of content.matchAll(linkRe)) {
    const rawTarget = match[1].trim()
    const target = rawTarget.split(/\s+/)[0] // ignore optional title: (path "title")
    if (!target || target.startsWith("#") || isExternalLink(target)) continue

    const resolved = resolveLink(file, target)
    if (!resolved) continue

    if (!fs.existsSync(resolved)) {
      errors.push({ file, message: `broken link: (${target}) -> ${path.relative(process.cwd(), resolved)}` })
    }
  }
}

// 2) Validate in-repo doc pointers in backticks (docs/ + scripts/ only).
const pathPrefixes = ["docs/", "scripts/"]
const skipInlinePathCheckFiles = new Set([
  // Archive index intentionally references removed paths.
  "docs/RUST_DOCS_ARCHIVE_2026-02-11.md",
])
for (const file of mdFiles) {
  if (skipInlinePathCheckFiles.has(file)) continue
  const content = fs.readFileSync(file, "utf8")
  const codeRe = /`([^`]+)`/g
  for (const match of content.matchAll(codeRe)) {
    const raw = match[1]
    if (raw.includes(" ")) continue
    if (raw.includes("*")) continue

    const candidate = normalizePathCandidate(raw)
    if (!pathPrefixes.some((p) => candidate.startsWith(p))) continue
    if (!candidate.endsWith(".md") && !candidate.endsWith(".json") && !candidate.endsWith("/")) continue

    if (!fs.existsSync(candidate)) {
      errors.push({ file, message: `missing path: \`${raw}\`` })
    }
  }
}

// 3) Ensure each top-level docs/<dir>/ has a README.md (tracked).
const trackedSet = new Set(tracked)
const topDirs = new Set()
for (const f of tracked) {
  const m = f.match(/^docs\/([^/]+)\//)
  if (m) topDirs.add(m[1])
}

for (const dir of Array.from(topDirs).sort()) {
  const readme = `docs/${dir}/README.md`
  if (!trackedSet.has(readme)) {
    errors.push({ file: "docs", message: `missing index: ${readme}` })
  }
}

// 4) Guard canonical docs from stale active-surface claims.
const canonicalDocs = [
  "README.md",
  "AGENTS.md",
  "docs/README.md",
  "docs/PROJECT_OVERVIEW.md",
  "docs/AGENT_MAP.md",
  "docs/ROADMAP.md",
]

const historicalQualifierRe = /\b(legacy|historical|archived|removed|deleted|deprecated|non-canonical)\b/i
const staleSurfaceRules = [
  { pattern: /apps\/mobile\//, label: "apps/mobile/" },
  { pattern: /apps\/desktop\//, label: "apps/desktop/" },
  { pattern: /apps\/inbox-autopilot\//, label: "apps/inbox-autopilot/" },
  { pattern: /apps\/openagents-runtime\//, label: "apps/openagents-runtime/" },
]

const staleRuntimeClaimRules = [
  { pattern: /\bLaravel 12 \+ Inertia \+ React\b/, label: "Laravel web runtime claim" },
  { pattern: /\bElixir runtime\b/i, label: "Elixir runtime claim" },
  { pattern: /\bmix phx\.server\b/, label: "mix runtime command claim" },
]

for (const file of canonicalDocs) {
  if (!trackedSet.has(file)) continue
  const lines = fs.readFileSync(file, "utf8").split("\n")
  lines.forEach((line, index) => {
    for (const rule of staleSurfaceRules) {
      if (rule.pattern.test(line) && !historicalQualifierRe.test(line)) {
        errors.push({
          file,
          message: `stale surface reference without historical qualifier at line ${index + 1}: ${rule.label}`,
        })
      }
    }
    for (const rule of staleRuntimeClaimRules) {
      if (rule.pattern.test(line) && !historicalQualifierRe.test(line)) {
        errors.push({
          file,
          message: `stale runtime claim at line ${index + 1}: ${rule.label}`,
        })
      }
    }
  })
}

if (errors.length) {
  const byFile = new Map()
  for (const e of errors) {
    const arr = byFile.get(e.file) ?? []
    arr.push(e.message)
    byFile.set(e.file, arr)
  }

  console.error(`docs-check failed: ${errors.length} issue(s)\n`)
  for (const [file, msgs] of byFile.entries()) {
    console.error(file)
    for (const msg of msgs) console.error(`  - ${msg}`)
    console.error("")
  }
  process.exit(1)
}

console.log("docs-check: OK")
