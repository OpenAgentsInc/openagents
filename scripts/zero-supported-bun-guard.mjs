#!/usr/bin/env node

import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"
import path from "node:path"
import process from "node:process"

const root = path.resolve(process.argv[2] ?? ".")

const tracked = execFileSync("git", ["ls-files", "-z"], {
  cwd: root,
  encoding: "utf8",
})
  .split("\0")
  .filter(Boolean)

const historicalPrefixes = [
  "docs/transcripts/",
  "docs/fable/",
  "docs/reference/",
  "docs/research/",
  "docs/sarah/",
  "assurance/receipts/",
  "apps/pylon/docs/proofs/",
]

const historicalSegments = ["/proofs/", "/receipts/", "/fixtures/", "/conformance/"]

const reviewedCompatibilityPaths = new Set([
  ".gitignore",
  "packages/pylon-core/src/custody/harness-maintenance.ts",
  "packages/pylon-core/src/custody/harness-maintenance.test.ts",
  "packages/pylon-core/src/executor/workspace-materializer.ts",
  "packages/pylon-core/src/executor/workspace-materializer.test.ts",
  "pnpm-lock.yaml",
])

const reviewedNegativePaths = new Set([
  "scripts/bun-api-perimeter-allowlist.ts",
  "scripts/bun-api-perimeter-scan.test.ts",
  "scripts/bun-api-perimeter-scan.ts",
  "scripts/node-vp-cutover-inventory.mjs",
  "scripts/node-vp-cutover-inventory.test.mjs",
  "scripts/vp2-node-runtime-guard.mjs",
  "scripts/vp2-node-runtime-guard.test.mjs",
  "scripts/effect-authority-boundary-allowlist.ts",
  "scripts/effect-authority-boundary-scan.ts",
  "scripts/zero-supported-bun-guard.mjs",
  "scripts/zero-supported-bun-guard.test.mjs",
  "apps/openagents-desktop/scripts/release-preflight.ts",
  "apps/openagents-desktop/tests/electron-boundary.test.ts",
  "apps/pylon/tests/gcloud-setup-script.test.ts",
  "packages/behavior-contracts/src/sarah-retired.ts",
  "packages/khala-sync-server/src/test/local-postgres.ts",
])

const authoritativeNames = new Set(["README.md", "INSTALL.md", "AGENTS.md", "INVARIANTS.md"])
const executableExtensions = new Set([
  ".bash",
  ".cjs",
  ".js",
  ".jsx",
  ".mjs",
  ".plist",
  ".service",
  ".sh",
  ".ts",
  ".tsx",
  ".zsh",
])

const isHistorical = (file) =>
  historicalPrefixes.some((prefix) => file.startsWith(prefix)) ||
  historicalSegments.some((segment) => file.includes(segment))

const isSupportedSurface = (file) => {
  const basename = path.posix.basename(file)
  if (file === "docs/DEPLOYMENT.md") return true
  if (authoritativeNames.has(basename)) return true
  if (basename === "package.json" || basename === "pnpm-workspace.yaml") return true
  if (basename === "Dockerfile" || basename.startsWith("Dockerfile.")) return true
  return executableExtensions.has(path.posix.extname(file))
}

const isTestFixture = (file) =>
  /(?:^|\/)tests?\//u.test(file) || /\.test\.[^.]+$/u.test(file)

const signatures = [
  { name: "Bun shebang", pattern: /^#!.*\bbun\b/imu },
  { name: "Bun global API", pattern: /\bBun\s*\./u },
  { name: "Bun module import", pattern: /(?:from\s*|import\s*\(|require\s*\()\s*["']bun(?::[^"']*)?["']/u },
  { name: "Bun ambient type", pattern: /(?:types|reference\s+types)\s*[=:]\s*["']bun["']/iu },
  {
    name: "Bun command",
    pattern:
      /(?:^|[\s`"'=;|&(])(?:bunx(?=$|[\s`"';|&)])|bun\s+(?:run|test|install|pm|publish|build|x|--[\w-]+|[^\s`"']+\.(?:[cm]?[jt]sx?))\b)/mu,
  },
  { name: "Bun install layout", pattern: /(?:^|[/~])\.bun(?:\/|$)/imu },
  { name: "Bun installer", pattern: /\bbun\.sh\b|\boven-sh\/bun\b/iu },
  { name: "Bun package authority", pattern: /"packageManager"\s*:\s*"bun@|"bun"\s*:\s*"|"@types\/bun"|"electrobun"/iu },
  {
    name: "Bun child process",
    pattern: /(?:spawn|spawnSync|execFile|execFileSync)\s*\(\s*["']bun["']|(?:args|command|executable)\s*:\s*\[?\s*["']bun["']/u,
  },
]

const wholeFileSignatures = [
  {
    name: "Bun argv",
    pattern: /\[\s*["']bun["']\s*,\s*["'](?:run|test|install|pm|publish|build|x|--[^"']+|[^"']+\.[cm]?[jt]sx?)["']/mu,
  },
  { name: "Bun executable override", pattern: /OPENAGENTS_BUN_PATH/u },
]

const violations = []
for (const file of tracked) {
  if (!isSupportedSurface(file) || isHistorical(file)) continue
  if (reviewedCompatibilityPaths.has(file) || reviewedNegativePaths.has(file)) continue

  let contents
  try {
    contents = readFileSync(path.join(root, file), "utf8")
  } catch {
    continue
  }

  if (!isTestFixture(file)) {
    for (const signature of wholeFileSignatures) {
      if (signature.pattern.test(contents)) {
        violations.push(`${file}: ${signature.name}`)
      }
    }
  }

  const lines = contents.split("\n")
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]
    for (const signature of signatures) {
      if (
        isTestFixture(file) &&
        (signature.name === "Bun command" || signature.name === "Bun child process")
      ) {
        continue
      }
      signature.pattern.lastIndex = 0
      if (signature.pattern.test(line)) {
        violations.push(`${file}:${index + 1}: ${signature.name}: ${line.trim()}`)
        break
      }
    }
  }
}

if (violations.length > 0) {
  console.error("Unsupported Bun references found on supported repository surfaces:")
  for (const violation of violations) console.error(`- ${violation}`)
  console.error("\nHistorical evidence, negative fixtures, and third-party compatibility require named policy entries.")
  process.exit(1)
}

console.log(`zero-supported-bun: checked ${tracked.length} tracked files; no supported Bun path remains`)
