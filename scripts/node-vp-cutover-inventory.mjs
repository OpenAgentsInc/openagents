#!/usr/bin/env node

import { createHash } from "node:crypto"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { basename, relative, resolve, sep } from "node:path"
import { pathToFileURL } from "node:url"
import { spawnSync } from "node:child_process"

export const INVENTORY_SCHEMA = "openagents.node_vp_cutover_inventory.v1"
export const SCANNER_VERSION = 1
export const DEFAULT_BASELINE = "docs/sol/evidence/node-vp-cutover-inventory.v1.json"

const SELF_EXCLUDED_PATHS = new Set([
  DEFAULT_BASELINE,
  "docs/sol/document-manifest.json",
  "docs/sol/live-roadmap-issues.json",
  "scripts/node-vp-cutover-inventory.mjs",
  "scripts/node-vp-cutover-inventory.test.mjs",
])

const CONTENT_RULES = [
  { id: "bun-api", pattern: /\bBun\.[A-Za-z_$][A-Za-z0-9_$]*/g },
  { id: "bun-test", pattern: /\bvite-plus/test\b/g },
  { id: "bun-module", pattern: /\bbun:[a-z0-9_-]+\b|["']bun["']/gi },
  { id: "bun-shebang", pattern: /^#![^\n]*\bbun\b/gm },
  {
    id: "bun-command",
    pattern: /(^|[\s"'`:=,(])(?:bun|bunx)(?=\s|$|["'`,)])/gim,
  },
  {
    id: "direct-tool",
    pattern: /\b(?:vite-plus|vitest|vite|oxlint|oxfmt|tsdown|turbo)\b/gi,
  },
  {
    id: "money-negative-contract",
    pattern: /\b(?:paymentMode|settlementState|payoutClaimAllowed)\b/g,
  },
  {
    id: "money-authority",
    pattern:
      /\b(?:payments?|wallets?|treasury|billing|payouts?|invoices?|lightning)\b/gi,
  },
  {
    id: "money-spark-authority",
    pattern: /\b(?:SparkWallet|sparkWallet|spark-bun-storage|@buildonspark)\b/g,
  },
  {
    id: "money-rail-dependency",
    pattern: /(?:@moneydevkit\/|@breeztech\/breez-sdk-spark|\bstripe\b|\brevenuecat\b)/gi,
  },
  {
    id: "money-secret-binding",
    pattern: /\b(?:MDK|SPARK|STRIPE|REVENUECAT|L402)_[A-Z0-9_]+\b/g,
  },
  {
    id: "money-positive-mode",
    pattern:
      /\bpaymentMode\s*[:=]\s*["'](?:paid|metered|buyer_funded)["']|\bpayoutClaimAllowed\s*[:=]\s*true\b|\bsettlementState\s*[:=]\s*["'](?:pending|recorded|settled)["']/gi,
  },
]

const normalizePath = (path) => path.split(sep).join("/")

const normalizeLine = (line) => line.trim().replace(/\s+/g, " ")

const signatureFor = (value) =>
  createHash("sha256").update(value).digest("hex")

const isHistoricalPath = (path) =>
  path.startsWith("docs/transcripts/") ||
  path.startsWith("docs/fable/") ||
  path.startsWith("docs/research/") ||
  path.includes("/receipts/") ||
  /(?:^|\/)[^/]*receipt[^/]*\.md$/i.test(path) ||
  /(?:^|\/)archive(?:s|d)?\//.test(path)

const isTestPath = (path) =>
  /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(path) ||
  path.includes("/__tests__/") ||
  path.includes("/fixtures/") ||
  path.includes("/__fixtures__/") ||
  path.startsWith("tests/") ||
  path.includes("/tests/")

const isPackageAuthorityPath = (path) =>
  basename(path) === "package.json" ||
  basename(path) === "bun.lock" ||
  basename(path) === "bunfig.toml" ||
  path === "package.json" ||
  path === "bun.lock" ||
  path.startsWith(".githooks/") ||
  path.startsWith("scripts/") ||
  /(?:^|\/)(?:vite|vitest|oxlint|oxfmt|tsdown|turbo)\.config\./.test(path)

const isReleaseOrImagePath = (path) =>
  /(?:^|\/)(?:Dockerfile[^/]*|Containerfile[^/]*|docker-compose[^/]*)$/i.test(path) ||
  /(?:^|\/)(?:release|releases|publish|deploy)(?:\/|\.|-)/i.test(path) ||
  /(?:^|\/)(?:cloudrun|container|image)(?:\/|\.|-)/i.test(path)

const isPaymentNamedPath = (path) =>
  /(?:^|[\/_.-])(?:payment|payments|wallet|wallets|treasury|billing|credit|credits|payout|payouts|settlement|settlements|invoice|invoices|lightning|mdk-tips-buffer|mdk-treasury|nip90|labor-market)(?:[\/_.-]|$)/i.test(
    path,
  ) || path.includes("spark-bun-storage")

const isAppliedMigrationPath = (path) =>
  path.startsWith("migrations/") || path.includes("/migrations/")

const isMoneyEvidencePath = (path) =>
  isAppliedMigrationPath(path) ||
  /(?:^|\/)backfill-(?:billing|treasury|token-ledger)\./i.test(path)

const isNonEconomicMoneyFalsePositive = (path) =>
  /(?:^|\/)(?:ercot|energy|power-grid|grid-operations)(?:\/|\.|-)/i.test(path)

const isDeprecatedClientPath = (path) =>
  path.startsWith("clients/khala-code-desktop/") ||
  path.startsWith("clients/khala-mobile/") ||
  path.startsWith("clients/khala-ios/")

const isAuthoritativeDocPath = (path) =>
  path === "AGENTS.md" ||
  path === "INVARIANTS.md" ||
  path === "README.md" ||
  path.startsWith("docs/sol/") ||
  path.startsWith("docs/promises/") ||
  path === "docs/DEPLOYMENT.md" ||
  /(?:^|\/)(?:AGENTS|INVARIANTS|README)\.md$/.test(path)

const isActiveCodeOrConfigPath = (path) =>
  /^(?:apps|packages|clients|crates|scripts|fixtures|specs)\//.test(path) ||
  path === "package.json" ||
  path.endsWith(".json") ||
  path.endsWith(".toml") ||
  path.endsWith(".yaml") ||
  path.endsWith(".yml") ||
  /(?:^|\/)(?:Dockerfile|Containerfile)/.test(path)

const shouldApplyContentRule = (rule, path, line) => {
  if (!rule.startsWith("money-")) return true
  if (rule === "money-authority") {
    if (path.startsWith("docs/promises/") || isPaymentNamedPath(path)) return true
    if (!isActiveCodeOrConfigPath(path)) return false
    return /\b(?:import|export|from|require|route|router|endpoint|binding|service|repository|store|handler|schema|table|queue|cron)\b/i.test(line)
  }
  if (rule === "money-rail-dependency") {
    if (basename(path) === "package.json" || basename(path).endsWith(".lock")) return true
    return isActiveCodeOrConfigPath(path) && /\b(?:import|from|require|dependency|package|binding|service)\b/i.test(line)
  }
  if (rule === "money-secret-binding") {
    return isActiveCodeOrConfigPath(path) && !path.endsWith(".md")
  }
  if (rule === "money-positive-mode" || rule === "money-negative-contract") {
    return isActiveCodeOrConfigPath(path) && !path.endsWith(".md")
  }
  return isActiveCodeOrConfigPath(path) || path.startsWith("docs/promises/")
}

export const classifyFinding = ({ category, path }) => {
  if (category === "historical-transcript-archive") {
    return { phase: "historical", disposition: "retain-read-only" }
  }

  if (isHistoricalPath(path)) {
    return { phase: "historical", disposition: "retain-read-only" }
  }

  if (category === "money-negative-contract") {
    return { phase: "VP-1", disposition: "retain-negative-contract" }
  }

  if (category === "migration-history") {
    return { phase: "VP-1", disposition: "retain-immutable-migration" }
  }

  if (
    category === "money-authority" ||
    category === "money-spark-authority" ||
    category === "money-rail-dependency" ||
    category === "money-secret-binding" ||
    category === "money-positive-mode" ||
    category === "money-path-surface"
  ) {
    if (isNonEconomicMoneyFalsePositive(path)) {
      return { phase: "false-positive", disposition: "non-economic-vocabulary" }
    }
    if (isMoneyEvidencePath(path)) {
      return { phase: "VP-1", disposition: "retain-read-only-evidence" }
    }
    if (isAuthoritativeDocPath(path)) {
      return { phase: "VP-1", disposition: "update-or-withdraw-authority" }
    }
    return { phase: "VP-1", disposition: "decommission-delete" }
  }

  if (category === "runtime-image") {
    if (isPaymentNamedPath(path)) {
      return { phase: "VP-1", disposition: "decommission-delete" }
    }
    return { phase: "VP-5", disposition: "convert-runtime-image" }
  }

  if (category === "release-surface") {
    if (isPaymentNamedPath(path)) {
      return { phase: "VP-1", disposition: "decommission-delete" }
    }
    return { phase: "VP-5", disposition: "stabilize-release-path" }
  }

  if (category === "hook-surface") {
    return { phase: "VP-4", disposition: "replace-hook-authority" }
  }

  if (category === "bun-manifest" || category === "bun-package-authority") {
    return { phase: "VP-4", disposition: "replace-workspace-authority" }
  }

  if (category === "direct-tool") {
    if (isTestPath(path)) {
      return { phase: "VP-3", disposition: "prove-test-parity" }
    }
    return { phase: "VP-4", disposition: "replace-with-vite-plus" }
  }

  if (
    category === "bun-api" ||
    category === "bun-test" ||
    category === "bun-module" ||
    category === "bun-shebang" ||
    category === "bun-command"
  ) {
    if (isPaymentNamedPath(path)) {
      return { phase: "VP-1", disposition: "decommission-delete" }
    }
    if (isTestPath(path) || category === "bun-test") {
      return { phase: "VP-3", disposition: "migrate-test-runtime" }
    }
    if (isDeprecatedClientPath(path)) {
      return { phase: "VP-6", disposition: "delete-deprecated-bun-path" }
    }
    if (isReleaseOrImagePath(path)) {
      return { phase: "VP-5", disposition: "convert-release-runtime" }
    }
    if (isAuthoritativeDocPath(path)) {
      return { phase: "VP-6", disposition: "update-authoritative-doc" }
    }
    if (isPackageAuthorityPath(path)) {
      return { phase: "VP-4", disposition: "replace-tooling-runtime" }
    }
    return { phase: "VP-2", disposition: "port-retained-runtime" }
  }

  throw new Error(`unclassified inventory category ${category} at ${path}`)
}

const git = (root, args) => {
  const result = spawnSync("git", args, { cwd: root, encoding: "utf8" })
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || `git ${args.join(" ")} failed`)
  }
  return result.stdout
}

const trackedPaths = (root) =>
  git(root, ["ls-files", "-z", "--cached"])
    .split("\0")
    .filter(Boolean)
    .map(normalizePath)
    .sort()

const isLikelyBinary = (bytes) => bytes.subarray(0, 8192).includes(0)

const findingKey = (finding) =>
  [finding.path, finding.category, finding.phase, finding.disposition].join("\0")

const addFinding = (groups, finding) => {
  const classified = classifyFinding(finding)
  const complete = { ...finding, ...classified }
  const key = findingKey(complete)
  const group = groups.get(key) ?? {
    path: complete.path,
    category: complete.category,
    phase: complete.phase,
    disposition: complete.disposition,
    matches: 0,
    signatures: new Map(),
  }
  group.matches += 1
  group.signatures.set(
    complete.signature,
    (group.signatures.get(complete.signature) ?? 0) + 1,
  )
  groups.set(key, group)
}

const serializeGroups = (groups) =>
  [...groups.values()]
    .map((group) => ({
      path: group.path,
      category: group.category,
      phase: group.phase,
      disposition: group.disposition,
      matches: group.matches,
      signatures: Object.fromEntries([...group.signatures.entries()].sort()),
    }))
    .sort((left, right) => findingKey(left).localeCompare(findingKey(right)))

export const collectInventory = (root = process.cwd()) => {
  const absoluteRoot = resolve(root)
  const groups = new Map()
  const skipped = { binary: 0, selfGenerated: 0, transcriptFiles: 0 }

  addFinding(groups, {
    path: "docs/transcripts/",
    category: "historical-transcript-archive",
    signature: signatureFor("preserved historical transcript archive"),
  })

  for (const path of trackedPaths(absoluteRoot)) {
    if (SELF_EXCLUDED_PATHS.has(path)) {
      skipped.selfGenerated += 1
      continue
    }
    if (path.startsWith("docs/transcripts/")) {
      skipped.transcriptFiles += 1
      continue
    }

    if (basename(path) === "bun.lock" || basename(path) === "bunfig.toml") {
      addFinding(groups, {
        path,
        category: "bun-manifest",
        signature: signatureFor(`bun-manifest:${path}`),
      })
    }
    if (isAppliedMigrationPath(path)) {
      const migrationBytes = readFileSync(resolve(absoluteRoot, path))
      addFinding(groups, {
        path,
        category: "migration-history",
        signature: signatureFor(migrationBytes),
      })
    }
    if (isPaymentNamedPath(path)) {
      addFinding(groups, {
        path,
        category: "money-path-surface",
        signature: signatureFor(`money-path:${path}`),
      })
    }
    if (/^(?:.*\/)?\.githooks\//.test(path) || path.startsWith(".githooks/")) {
      addFinding(groups, {
        path,
        category: "hook-surface",
        signature: signatureFor(`hook:${path}`),
      })
    }
    if (/(?:^|\/)(?:Dockerfile[^/]*|Containerfile[^/]*|docker-compose[^/]*)$/i.test(path)) {
      addFinding(groups, {
        path,
        category: "runtime-image",
        signature: signatureFor(`runtime-image:${path}`),
      })
    }
    if (/(?:^|\/)(?:release|releases|publish|deploy)(?:\/|\.|-)/i.test(path)) {
      addFinding(groups, {
        path,
        category: "release-surface",
        signature: signatureFor(`release:${path}`),
      })
    }

    const absolutePath = resolve(absoluteRoot, path)
    if (!existsSync(absolutePath)) continue
    const bytes = readFileSync(absolutePath)
    if (isLikelyBinary(bytes)) {
      skipped.binary += 1
      continue
    }

    if (basename(path) === "package.json") {
      try {
        const manifest = JSON.parse(bytes.toString("utf8"))
        const dependencyBlocks = [
          manifest.dependencies,
          manifest.devDependencies,
          manifest.optionalDependencies,
          manifest.peerDependencies,
        ].filter(Boolean)
        const bunAuthorities = []
        if (typeof manifest.packageManager === "string" && manifest.packageManager.startsWith("bun@")) {
          bunAuthorities.push(`packageManager:${manifest.packageManager}`)
        }
        if (manifest.engines && Object.hasOwn(manifest.engines, "bun")) {
          bunAuthorities.push(`engines.bun:${manifest.engines.bun}`)
        }
        for (const dependencies of dependencyBlocks) {
          for (const name of ["bun-types", "@types/bun"]) {
            if (Object.hasOwn(dependencies, name)) bunAuthorities.push(`dependency:${name}`)
          }
        }
        for (const authority of bunAuthorities) {
          addFinding(groups, {
            path,
            category: "bun-package-authority",
            signature: signatureFor(authority),
          })
        }
      } catch (error) {
        throw new Error(`invalid package manifest ${path}: ${error instanceof Error ? error.message : String(error)}`)
      }
    }

    const lines = bytes.toString("utf8").split(/\r?\n/)
    for (const line of lines) {
      const normalized = normalizeLine(line)
      if (!normalized) continue
      for (const rule of CONTENT_RULES) {
        if (!shouldApplyContentRule(rule.id, path, line)) continue
        rule.pattern.lastIndex = 0
        const matches = [...line.matchAll(rule.pattern)]
        for (let index = 0; index < matches.length; index += 1) {
          addFinding(groups, {
            path,
            category: rule.id,
            signature: signatureFor(`${rule.id}:${normalized}`),
          })
        }
      }
    }
  }

  const entries = serializeGroups(groups)
  const phaseCounts = {}
  const categoryCounts = {}
  for (const entry of entries) {
    phaseCounts[entry.phase] = (phaseCounts[entry.phase] ?? 0) + entry.matches
    categoryCounts[entry.category] =
      (categoryCounts[entry.category] ?? 0) + entry.matches
  }

  return {
    schema: INVENTORY_SCHEMA,
    scannerVersion: SCANNER_VERSION,
    coverage: {
      trackedText: "all Git-tracked text files; binary files are counted and skipped",
      transcriptArchive: "collapsed to one historical read-only entry",
      exclusions: [...SELF_EXCLUDED_PATHS].sort(),
      skipped,
    },
    summary: {
      entries: entries.length,
      matches: entries.reduce((sum, entry) => sum + entry.matches, 0),
      phaseCounts: Object.fromEntries(Object.entries(phaseCounts).sort()),
      categoryCounts: Object.fromEntries(Object.entries(categoryCounts).sort()),
    },
    entries,
  }
}

export const createBaseline = (inventory, sourceCommit) => ({
  ...inventory,
  sourceCommit,
  policy:
    "Burn-down only: a current signature count may decrease or disappear; a new path/category/signature or increased multiplicity fails.",
})

export const compareWithBaseline = (inventory, baseline) => {
  const errors = []
  if (baseline.schema !== INVENTORY_SCHEMA) {
    errors.push(`baseline schema must be ${INVENTORY_SCHEMA}`)
  }
  if (baseline.scannerVersion !== SCANNER_VERSION) {
    errors.push(`baseline scannerVersion must be ${SCANNER_VERSION}`)
  }

  const baselineByKey = new Map(
    (baseline.entries ?? []).map((entry) => [findingKey(entry), entry]),
  )
  for (const entry of inventory.entries) {
    const previous = baselineByKey.get(findingKey(entry))
    if (!previous) {
      errors.push(`NEW ${entry.path} [${entry.category} -> ${entry.phase}/${entry.disposition}]`)
      continue
    }
    for (const [signature, count] of Object.entries(entry.signatures)) {
      const previousCount = previous.signatures?.[signature] ?? 0
      if (count > previousCount) {
        errors.push(
          `GROWTH ${entry.path} [${entry.category}] signature ${signature.slice(0, 12)} ${previousCount} -> ${count}`,
        )
      }
    }
  }

  const currentByKey = new Map(
    inventory.entries.map((entry) => [findingKey(entry), entry]),
  )
  for (const previous of baseline.entries ?? []) {
    if (previous.category !== "migration-history") continue
    const current = currentByKey.get(findingKey(previous))
    if (!current) {
      errors.push(`MIGRATION_REMOVED ${previous.path}`)
      continue
    }
    if (JSON.stringify(current.signatures) !== JSON.stringify(previous.signatures)) {
      errors.push(`MIGRATION_CHANGED ${previous.path}`)
    }
  }
  return errors
}

const parseArgs = (argv) => {
  const options = { root: process.cwd(), baseline: DEFAULT_BASELINE }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === "--root") options.root = argv[++index]
    else if (arg === "--baseline") options.baseline = argv[++index]
    else if (arg === "--source-commit") options.sourceCommit = argv[++index]
    else if (arg === "--write-baseline") options.writeBaseline = true
    else if (arg === "--json") options.json = true
    else if (arg === "--check") options.check = true
    else throw new Error(`unknown argument: ${arg}`)
  }
  return options
}

const main = () => {
  const options = parseArgs(process.argv.slice(2))
  const root = resolve(options.root)
  const inventory = collectInventory(root)
  const baselinePath = resolve(root, options.baseline)

  if (options.writeBaseline) {
    const sourceCommit = options.sourceCommit ?? git(root, ["rev-parse", "HEAD"]).trim()
    const baseline = createBaseline(inventory, sourceCommit)
    writeFileSync(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`)
    console.log(
      `[node-vp-inventory] wrote ${relative(root, baselinePath)}: ${inventory.summary.entries} entries / ${inventory.summary.matches} matches`,
    )
    return
  }

  if (options.json) {
    console.log(JSON.stringify(inventory, null, 2))
    return
  }

  if (!options.check && !existsSync(baselinePath)) {
    throw new Error(`baseline not found: ${relative(root, baselinePath)}`)
  }
  const baseline = JSON.parse(readFileSync(baselinePath, "utf8"))
  const errors = compareWithBaseline(inventory, baseline)
  console.log(
    `[node-vp-freeze] ${inventory.summary.entries} entries / ${inventory.summary.matches} matches; baseline ${baseline.sourceCommit ?? "unknown"}`,
  )
  if (errors.length > 0) {
    for (const error of errors) console.error(error)
    process.exitCode = 1
    return
  }
  console.log("[node-vp-freeze] OK: no new Bun, direct-tool, money-authority, hook, release, or runtime-image surface")
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main()
}
