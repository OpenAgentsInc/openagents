#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs"
import { basename, dirname, extname, resolve } from "node:path"
import { execFileSync } from "node:child_process"
import { fileURLToPath } from "node:url"

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")

const trackedAndUntrackedFiles = execFileSync(
  "git",
  ["ls-files", "-co", "--exclude-standard"],
  { cwd: repositoryRoot, encoding: "utf8" },
)
  .split("\n")
  .map(value => value.trim())
  .filter(Boolean)
  .filter(path => existsSync(resolve(repositoryRoot, path)))

const retiredPaths = [
  "clients",
  "apps/forge",
  "apps/nostr-relay",
  "apps/openagents-world",
]

const violations = []

for (const path of retiredPaths) {
  if (trackedAndUntrackedFiles.some(file => file === path || file.startsWith(`${path}/`))) {
    violations.push(`${path}: retired path contains files`)
  }
}

const activeExtensions = new Set([
  ".cjs",
  ".js",
  ".json",
  ".jsx",
  ".mjs",
  ".mts",
  ".rs",
  ".sh",
  ".toml",
  ".ts",
  ".tsx",
  ".yaml",
  ".yml",
])

const excludedActivePath = path =>
  path.startsWith("docs/") ||
  path.includes("/docs/") ||
  path.includes("/migrations/") ||
  path.includes("/fixtures/") ||
  path.includes("/proof/") ||
  path.includes("/proofs/") ||
  path.includes("/conformance/") ||
  path.endsWith(".test.ts") ||
  path.endsWith(".test.tsx") ||
  path.endsWith(".test.mjs") ||
  path.endsWith("typecheck-tests-baseline.json") ||
  path.endsWith("scripts/google-cloud-authority-guard.mjs") ||
  path.endsWith("scripts/check-effect-topology.mjs") ||
  path.endsWith("scripts/cloudrun/assert-self-contained-bundle.mjs") ||
  path.includes("packages/khala-sync-server/src/") && path.endsWith("-backfill.ts")

const forbiddenPatterns = [
  ["Cloudflare package", /@cloudflare\//],
  ["Cloudflare runtime import", /cloudflare:workers/],
  ["Wrangler operation", /\bwrangler\s+(?:deploy|d1|secret)\b/i],
  ["workers.dev origin", /\.workers\.dev\b/i],
  ["Cloudflare credential", /\b(?:CLOUDFLARE_[A-Z0-9_]+|CF_AIG_TOKEN)\b/],
  ["Cloudflare service lane", /\b(?:cloudflare_email|cloudflare_vpc_service)\b/i],
  ["retired owned service", /\b(?:relay|forge)\.openagents\.com\b/i],
  ["retired world service", /apps\/openagents-world/],
  ["retired SHC lane", /(?:\bshc\b|cloud[-_]shc|oa-shc|SHC_)/i],
]

for (const path of trackedAndUntrackedFiles) {
  if (basename(path) === "wrangler.jsonc") {
    violations.push(`${path}: Wrangler configuration is retired`)
    continue
  }
  if (!activeExtensions.has(extname(path)) || excludedActivePath(path)) continue

  const source = readFileSync(resolve(repositoryRoot, path), "utf8")
  for (const [label, pattern] of forbiddenPatterns) {
    if (pattern.test(source)) violations.push(`${path}: ${label}`)
  }
}

const authorityFiles = [
  "AGENTS.md",
  "INVARIANTS.md",
  "apps/openagents.com/AGENTS.md",
  "apps/openagents.com/INVARIANTS.md",
  "apps/openagents.com/README.md",
  "docs/adr/0014-use-google-cloud-as-the-sole-production-infrastructure.md",
  "packages/khala-sync-server/README.md",
]

for (const path of authorityFiles) {
  const source = readFileSync(resolve(repositoryRoot, path), "utf8")
  if (!/Google Cloud/.test(source)) {
    violations.push(`${path}: missing explicit Google Cloud authority`)
  }
}

const retiredCloudflareDecision = readFileSync(
  resolve(
    repositoryRoot,
    "docs/adr/0004-prefer-cloudflare-native-product-infrastructure.md",
  ),
  "utf8",
)
if (!/^status: "superseded"$/m.test(retiredCloudflareDecision)) {
  violations.push("ADR-0004 must remain superseded")
}

const googleCloudDecision = readFileSync(
  resolve(
    repositoryRoot,
    "docs/adr/0014-use-google-cloud-as-the-sole-production-infrastructure.md",
  ),
  "utf8",
)
if (!/^status: "accepted"$/m.test(googleCloudDecision)) {
  violations.push("ADR-0014 must remain accepted")
}
if (!/SHC was a limited pilot/.test(googleCloudDecision)) {
  violations.push("ADR-0014 must preserve the SHC limited-pilot correction")
}
if (
  !/Cloudflare remains the authoritative DNS provider/.test(
    googleCloudDecision,
  )
) {
  violations.push("ADR-0014 must preserve Cloudflare authoritative DNS")
}
if (!/DNS-only/.test(googleCloudDecision)) {
  violations.push("ADR-0014 must preserve the DNS-only Google Cloud target")
}

if (violations.length > 0) {
  console.error("Google Cloud authority guard failed:\n")
  for (const violation of violations) console.error(`- ${violation}`)
  process.exit(1)
}

console.log("Google Cloud authority guard passed")
