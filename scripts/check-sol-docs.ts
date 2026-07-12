#!/usr/bin/env bun

import { spawnSync } from "node:child_process"
import { readdir, readFile, stat, writeFile } from "node:fs/promises"
import { dirname, join, relative, resolve, sep } from "node:path"
import {
  buildSolDocumentManifest,
  serializeSolDocumentManifest,
  SOL_DOCUMENT_MANIFEST_PATH,
} from "./generate-sol-doc-manifest"

export const SNAPSHOT_PATH = "docs/sol/live-roadmap-issues.json"
export const SNAPSHOT_SCHEMA_VERSION = 1
export const MASTER_LINE_BUDGET = 800
export const SNAPSHOT_MAX_AGE_HOURS = 7 * 24
export const CANONICAL_PRODUCT_PROJECTION_HEADING = "### Canonical open product issue projection"

export const REMOVED_JULY_9_PATHS = [
  "docs/sol/2026-07-09-authority-trust-and-economics.md",
  "docs/sol/2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md",
  "docs/sol/2026-07-09-effect-native-strategic-importance.md",
  "docs/sol/2026-07-09-execution-sequence-and-critical-path.md",
  "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
  "docs/sol/2026-07-09-issue-triage.md",
  "docs/sol/2026-07-09-risks-tensions-and-decision-tests.md",
  "docs/sol/2026-07-09-roadmap-system-model.md",
  "docs/sol/2026-07-09-sarah-first-product-architecture.md",
] as const

export const REMOVED_SOL_ARCHIVE_PATHS = [
  ...REMOVED_JULY_9_PATHS,
  "docs/sol/2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md",
] as const

export type RoadmapIssue = {
  number: number
  title: string
  url: string
  state: "OPEN"
  labels: string[]
}

export type RoadmapIssueSnapshot = {
  schemaVersion: number
  generatedAt: string
  repository: string
  label: string
  excludedLabels: string[]
  maxAgeHours: number
  issues: RoadmapIssue[]
}

type CheckOptions = {
  root: string
  now?: Date
  snapshot?: RoadmapIssueSnapshot
}

const issueUrlPattern = /^https:\/\/github\.com\/OpenAgentsInc\/openagents\/issues\/(\d+)$/

function uniqueSorted(values: number[]): number[] {
  return [...new Set(values)].sort((left, right) => left - right)
}

function equalNumbers(left: number[], right: number[]): boolean {
  const a = uniqueSorted(left)
  const b = uniqueSorted(right)
  return a.length === b.length && a.every((value, index) => value === b[index])
}

function formatNumbers(values: number[]): string {
  return uniqueSorted(values).map((number) => `#${number}`).join(", ") || "(none)"
}

export function extractSection(markdown: string, heading: string): string {
  const lines = markdown.split("\n")
  const start = lines.findIndex((line) => line.trim() === heading)
  if (start < 0) return ""
  const level = heading.match(/^#+/)?.[0].length ?? 6
  let end = lines.length
  for (let index = start + 1; index < lines.length; index += 1) {
    const nextLevel = lines[index]?.match(/^#+/)?.[0].length
    if (nextLevel && nextLevel <= level) {
      end = index
      break
    }
  }
  return lines.slice(start + 1, end).join("\n")
}

export function extractIssueNumbers(markdown: string): number[] {
  const numbers: number[] = []
  for (const match of markdown.matchAll(/(?:\/issues\/|#)(\d{3,})\b/g)) {
    numbers.push(Number(match[1]))
  }
  return uniqueSorted(numbers)
}

export function extractIssueTableNumbers(markdown: string): number[] {
  return extractIssueNumbers(
    markdown.split("\n").filter((line) => line.trimStart().startsWith("|")).join("\n"),
  )
}

function extractMarkdownTargets(markdown: string): string[] {
  const targets: string[] = []
  const pattern = /(?<!!)\[[^\]]*\]\((<[^>]+>|[^)\s]+)(?:\s+["'][^)]*)?\)/g
  for (const match of markdown.matchAll(pattern)) {
    const raw = match[1]
    if (!raw) continue
    targets.push(raw.startsWith("<") && raw.endsWith(">") ? raw.slice(1, -1) : raw)
  }
  return targets
}

function sectionLocalMarkdownTargets(markdown: string): string[] {
  return extractMarkdownTargets(markdown)
    .map((target) => target.split("#", 1)[0] ?? "")
    .filter((target) => target.startsWith("./") && target.endsWith(".md"))
    .map((target) => target.slice(2))
}

export function validateSnapshot(
  snapshot: RoadmapIssueSnapshot,
  now = new Date(),
): string[] {
  const errors: string[] = []
  if (snapshot.schemaVersion !== SNAPSHOT_SCHEMA_VERSION) {
    errors.push(`snapshot schemaVersion must be ${SNAPSHOT_SCHEMA_VERSION}`)
  }
  if (snapshot.repository !== "OpenAgentsInc/openagents") {
    errors.push("snapshot repository must be OpenAgentsInc/openagents")
  }
  if (snapshot.label !== "roadmap:sol") {
    errors.push("snapshot label must be roadmap:sol")
  }
  if (!Array.isArray(snapshot.excludedLabels) || !snapshot.excludedLabels.includes("area:docs")) {
    errors.push("snapshot excludedLabels must include area:docs to keep the product projection non-recursive")
  }
  if (snapshot.maxAgeHours !== SNAPSHOT_MAX_AGE_HOURS) {
    errors.push(`snapshot maxAgeHours must be ${SNAPSHOT_MAX_AGE_HOURS}`)
  }

  const generatedAt = new Date(snapshot.generatedAt)
  if (Number.isNaN(generatedAt.getTime())) {
    errors.push("snapshot generatedAt must be an ISO timestamp")
  } else {
    const ageMs = now.getTime() - generatedAt.getTime()
    if (ageMs < -5 * 60 * 1000) errors.push("snapshot generatedAt is in the future")
    if (ageMs > snapshot.maxAgeHours * 60 * 60 * 1000) {
      errors.push(`snapshot is older than ${snapshot.maxAgeHours} hours`)
    }
  }

  if (!Array.isArray(snapshot.issues) || snapshot.issues.length === 0) {
    errors.push("snapshot issues must be a non-empty array")
    return errors
  }

  const seen = new Set<number>()
  let previous = -1
  for (const issue of snapshot.issues) {
    if (!Number.isInteger(issue.number) || issue.number <= 0) {
      errors.push("snapshot issue numbers must be positive integers")
      continue
    }
    if (seen.has(issue.number)) errors.push(`snapshot repeats issue #${issue.number}`)
    seen.add(issue.number)
    if (issue.number < previous) errors.push("snapshot issues must be sorted by number")
    previous = issue.number
    if (!issue.title?.trim()) errors.push(`snapshot issue #${issue.number} has no title`)
    const urlNumber = issue.url?.match(issueUrlPattern)?.[1]
    if (Number(urlNumber) !== issue.number) errors.push(`snapshot issue #${issue.number} has a non-canonical URL`)
    if (issue.state !== "OPEN") errors.push(`snapshot issue #${issue.number} is not OPEN`)
    if (!issue.labels?.includes(snapshot.label)) errors.push(`snapshot issue #${issue.number} lacks ${snapshot.label}`)
    for (const excluded of snapshot.excludedLabels) {
      if (issue.labels?.includes(excluded)) errors.push(`snapshot issue #${issue.number} has excluded label ${excluded}`)
    }
  }
  return errors
}

export function validateMaster(master: string, snapshot: RoadmapIssueSnapshot): string[] {
  const errors: string[] = []
  const lines = master.split("\n").length
  if (lines > MASTER_LINE_BUDGET) errors.push(`master has ${lines} lines; budget is ${MASTER_LINE_BUDGET}`)
  if ((master.match(/^- Revision: \d+$/gm) ?? []).length !== 1) {
    errors.push("master must declare exactly one numeric Revision metadata field")
  }
  const projection = extractSection(master, CANONICAL_PRODUCT_PROJECTION_HEADING)
  if (!projection) {
    errors.push("master is missing the canonical open product issue projection")
  } else {
    const actual = extractIssueTableNumbers(projection)
    const expected = snapshot.issues.map((issue) => issue.number)
    if (!equalNumbers(actual, expected)) {
      errors.push(`master projection ${formatNumbers(actual)} differs from snapshot ${formatNumbers(expected)}`)
    }
  }
  return errors
}

export function validateRevisionPins(activeDocuments: Record<string, string>): string[] {
  const errors: string[] = []
  for (const [path, markdown] of Object.entries(activeDocuments)) {
    if (/\b(?:Master Roadmap )?Revision\s+\d+\b/i.test(markdown)) {
      errors.push(`${path} hard-codes a master revision`)
    }
  }
  return errors
}

export function validateQueueOwnership(documents: Record<string, string>): string[] {
  const owners = Object.entries(documents).filter(([, markdown]) =>
    markdown.includes(CANONICAL_PRODUCT_PROJECTION_HEADING),
  )
  const errors: string[] = []
  if (owners.length !== 1 || owners[0]?.[0] !== "docs/sol/MASTER_ROADMAP.md") {
    errors.push(`canonical open product issue projection must be owned only by docs/sol/MASTER_ROADMAP.md; found ${owners.map(([path]) => path).join(", ") || "none"}`)
  }
  for (const [path, markdown] of Object.entries(documents)) {
    const header = markdown.split("\n").slice(0, 20).join("\n")
    if (/Class:.*(?:historical|receipt|redirect)|Status:.*(?:retired|superseded|historical)/i.test(header) && /^## Start here\b/m.test(markdown)) {
      errors.push(`${path} is historical/superseded but declares a Start here section`)
    }
  }
  const dispatchIndex = documents["docs/sol/README.md"] ?? ""
  const readingOrder = extractSection(dispatchIndex, "## Dispatch-safe reading order")
  if (!readingOrder) {
    errors.push("docs/sol/README.md is missing the dispatch-safe reading order")
  } else {
    for (const target of sectionLocalMarkdownTargets(readingOrder)) {
      const path = `docs/sol/${target}`
      const header = (documents[path] ?? "").split("\n").slice(0, 20).join("\n")
      if (!documents[path]) errors.push(`dispatch-safe reading order targets missing ${path}`)
      if (/Class:.*(?:historical|receipt|redirect)|Status:.*(?:retired|superseded|historical)/i.test(header)) {
        errors.push(`dispatch-safe reading order lists historical/superseded ${path}`)
      }
    }
  }
  return errors
}

export function validateCleanAgentReading(
  documents: Record<string, string>,
  snapshot: RoadmapIssueSnapshot,
): string[] {
  const errors: string[] = []
  const index = documents["docs/sol/README.md"] ?? ""
  const directTargets = new Set(sectionLocalMarkdownTargets(index))
  for (const target of ["MASTER_ROADMAP.md", "CLAIM_PROTOCOL.md", "receipts/README.md"]) {
    if (!directTargets.has(target)) errors.push(`clean-agent index cannot reach ${target} in one link`)
  }

  const master = documents["docs/sol/MASTER_ROADMAP.md"] ?? ""
  for (const heading of [
    "## Owner decisions",
    "## Non-goals and non-revival boundary",
    "## Proof vocabulary",
    "## Current execution order",
  ]) {
    if (!extractSection(master, heading)) errors.push(`clean-agent master is missing ${heading}`)
  }
  const projection = extractIssueTableNumbers(
    extractSection(master, CANONICAL_PRODUCT_PROJECTION_HEADING),
  )
  if (!equalNumbers(projection, snapshot.issues.map((issue) => issue.number))) {
    errors.push("clean-agent product issue set differs from the pinned artifact")
  }
  const proof = extractSection(master, "## Proof vocabulary")
  for (const rung of ["code-landed", "fixture-proven", "deployed/distributed", "live-proven", "owner-accepted", "closed"]) {
    if (!proof.includes(rung)) errors.push(`clean-agent proof vocabulary is missing ${rung}`)
  }
  const order = extractSection(master, "## Current execution order")
  if (!/#\d{3,}/.test(order)) errors.push("clean-agent current execution order has no issue-backed next action")

  const claims = documents["docs/sol/CLAIM_PROTOCOL.md"] ?? ""
  if (!/```text\nCLAIM\n/.test(claims)) errors.push("clean-agent claim protocol lacks CLAIM shape")
  if (!/```text\nCLAIM-RELEASE\n/.test(claims)) errors.push("clean-agent claim protocol lacks CLAIM-RELEASE shape")
  return errors
}

export function validateIssueIndex(
  index: string,
  issueSourceFiles: string[],
  snapshot: RoadmapIssueSnapshot,
): string[] {
  const errors: string[] = []
  const classifiedSections = [
    "## Live issue sources",
    "## Closed proof and implementation sources",
    "## Closed non-revival tombstones",
    "## Architecture reference",
  ]
  const counts = new Map<string, number>()
  for (const heading of classifiedSections) {
    const section = extractSection(index, heading)
    if (!section) errors.push(`issue index is missing ${heading}`)
    for (const target of sectionLocalMarkdownTargets(section)) {
      counts.set(target, (counts.get(target) ?? 0) + 1)
    }
  }
  for (const file of issueSourceFiles.sort()) {
    const count = counts.get(file) ?? 0
    if (count !== 1) errors.push(`issue source ${file} is classified ${count} times; expected exactly once`)
  }
  for (const [file] of counts) {
    if (!issueSourceFiles.includes(file)) errors.push(`issue index classifies missing source ${file}`)
  }

  const sourceIssues = extractIssueNumbers(extractSection(index, "## Live issue sources"))
  const receiptIssues = extractIssueNumbers(extractSection(index, "## Live issues represented by receipts"))
  const planIssues = extractIssueNumbers(extractSection(index, "## Live issues represented by an owning plan"))
  const representedIssues = [...sourceIssues, ...receiptIssues, ...planIssues]
  const actual = uniqueSorted(representedIssues)
  if (representedIssues.length !== actual.length) {
    errors.push("issue index represents a live issue in more than one coverage section")
  }
  const expected = snapshot.issues.map((issue) => issue.number)
  if (!equalNumbers(actual, expected)) {
    errors.push(`issue index live set ${formatNumbers(actual)} differs from snapshot ${formatNumbers(expected)}`)
  }
  const closedIssues = uniqueSorted([
    ...extractIssueNumbers(extractSection(index, "## Closed proof and implementation sources")),
    ...extractIssueNumbers(extractSection(index, "## Closed non-revival tombstones")),
  ])
  for (const number of expected) {
    if (closedIssues.includes(number)) errors.push(`open issue #${number} is classified as closed`)
  }
  return errors
}

export function validatePolicy(activeDocuments: Record<string, string>): string[] {
  const errors: string[] = []
  const combined = Object.entries(activeDocuments)
    .map(([path, markdown]) => `\nFILE ${path}\n${markdown}`)
    .join("\n")
  if (!/nothing gates on physical Android/i.test(combined)) {
    errors.push("active authority must state that nothing gates on physical Android")
  }
  for (const [path, markdown] of Object.entries(activeDocuments)) {
    const physicalAndroidRevival = markdown.split("\n").some((line) => {
      const assertsGate = /(?:requires?\s+(?:a\s+)?physical Android|must\s+(?:use|pass on)\s+(?:a\s+)?physical Android|blocked\s+(?:on|until)\s+(?:a\s+)?physical Android)/i.test(line)
      const explicitDenial = /\b(?:no|not|nothing|never|without)\b[^.]{0,100}\b(?:gate|gates|gating|require|requires|required|blocked)\b[^.]{0,100}physical Android/i.test(line)
        || /physical Android[^.]{0,100}\b(?:not required|non-gating|never required)\b/i.test(line)
      return assertsGate && !explicitDenial
    })
    if (physicalAndroidRevival) {
      errors.push(`${path} revives physical Android as a gate`)
    }
    if (/(?:pause|paused|defer|deferred|block|blocked|remove|removed|disable|disabled)[^.\n]{0,100}persona-neutral voice/i.test(markdown)) {
      errors.push(`${path} pauses or removes persona-neutral voice`)
    }
  }
  return errors
}

export function validateReceiptIndex(index: string): string[] {
  const errors: string[] = []
  const lines = index.split("\n")
  const tableHeaders = lines.filter((line, indexNumber) =>
    /^\|/.test(line) && /^\|\s*---/.test(lines[indexNumber + 1] ?? ""),
  )
  for (const header of tableHeaders) {
    if (!/Evidence snapshot/.test(header) || !/Proof rung/.test(header) || !/Final disposition/.test(header)) {
      errors.push(`receipt table lacks snapshot, proof rung, or final disposition: ${header}`)
    }
  }
  if (tableHeaders.length < 4) errors.push(`receipt index has ${tableHeaders.length} evidence tables; expected 4`)
  const indexedRows = lines.filter((line) => /^\|/.test(line) && /\]\(\.\.\/[^)]+\.md\)/.test(line))
  if (indexedRows.length < 25) errors.push(`receipt index has ${indexedRows.length} evidence rows; expected at least 25`)
  for (const [indexNumber, row] of indexedRows.entries()) {
    const cells = row.split("|").slice(1, -1).map((cell) => cell.trim())
    if (cells.some((cell) => !cell)) errors.push(`receipt index row ${indexNumber + 1} has an empty field`)
    if (!/\d{4}-\d{2}-\d{2}/.test(row)) errors.push(`receipt index row ${indexNumber + 1} has no dated snapshot`)
  }
  return errors
}

export function validateArchiveManifest(manifest: string): string[] {
  const errors: string[] = []
  const required = [
    "Source repository: `OpenAgentsInc/openagents`",
    "Destination repository: `OpenAgentsInc/backroom`",
    "archive/openagents-sol-docs-2026-07-12/july9/",
    "Backroom import: `dec8ae52`",
    "OpenAgents link migration and source removal: `b62ad88136`",
    "Backroom final bidirectional receipt: `b9645456`",
    "OpenAgents completed manifest: `c608527eda`",
  ]
  for (const token of required) {
    if (!manifest.includes(token)) errors.push(`archive manifest is missing provenance token: ${token}`)
  }
  const hashes = manifest.match(/`[a-f0-9]{64}`/g) ?? []
  if (new Set(hashes).size !== REMOVED_JULY_9_PATHS.length) {
    errors.push(`archive manifest must contain ${REMOVED_JULY_9_PATHS.length} unique SHA-256 hashes`)
  }
  for (const path of REMOVED_JULY_9_PATHS) {
    if (!manifest.includes(`\`${path.replace("docs/sol/", "")}\``)) {
      errors.push(`archive manifest does not inventory ${path}`)
    }
  }
  return errors
}

export function validatePreparedArchiveManifest(manifest: string): string[] {
  const errors: string[] = []
  const required = [
    "Source repository: `OpenAgentsInc/openagents`",
    "Destination repository: `OpenAgentsInc/backroom`",
    "archive/openagents-sol-docs-2026-07-12/july10-delegation/",
    "`2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`",
    "`177c38a9b41c5817c13fcf69ae529c55d8a60a0f2d039740bb81593b80abed2a`",
    "| 769 | 41044 |",
    "[`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)",
    "[`2026-07-12-documentation-cleanup-audit-and-retirement-plan.md`](./2026-07-12-documentation-cleanup-audit-and-retirement-plan.md)",
    "Backroom import: `9c710a93`",
  ]
  for (const token of required) {
    if (!manifest.includes(token)) errors.push(`prepared archive manifest is missing: ${token}`)
  }
  const pending = /Status: Backroom import complete; OpenAgents source removal pending/.test(manifest)
  const complete = /Status: archive\/import\/link migration\/source removal complete/.test(manifest)
  if (!pending && !complete) {
    errors.push("delegation archive manifest must declare pending-removal or completed state")
  }
  if (complete) {
    for (const pattern of [
      /OpenAgents link migration and source removal: `[0-9a-f]{8,40}`/,
      /Backroom final bidirectional receipt: `[0-9a-f]{8,40}`/,
    ]) {
      if (!pattern.test(manifest)) errors.push("completed delegation archive manifest lacks final commit provenance")
    }
  }
  const candidateRows = manifest.split("\n").filter((line) =>
    line.startsWith("| `2026-07-10-") && /`[a-f0-9]{64}`/.test(line),
  )
  if (candidateRows.length !== 1) {
    errors.push(`prepared archive manifest must select exactly one July 10 candidate; found ${candidateRows.length}`)
  }
  if (!/Receipts, failures, decisions, transcripts,\s+tombstones, issue sources, and the cutover dependency contract are excluded\./m.test(manifest)) {
    errors.push("prepared archive manifest must explicitly exclude retained evidence and contracts")
  }
  if (
    !/Backroom import is not pushed and hash-verified, no OpenAgents source\s+deletion begins/m.test(manifest)
    && !/Backroom import `9c710a93` (?:is|was) pushed and hash-verified/m.test(manifest)
  ) {
    errors.push("prepared archive manifest must retain the Backroom-first deletion gate")
  }
  return errors
}

async function markdownFilesUnder(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  const files: string[] = []
  for (const entry of entries) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) files.push(...await markdownFilesUnder(path))
    if (entry.isFile() && entry.name.endsWith(".md")) files.push(path)
  }
  return files
}

export async function validateMarkdownLinks(root: string, files: string[]): Promise<string[]> {
  const errors: string[] = []
  for (const file of files) {
    const markdown = await readFile(file, "utf8")
    for (const rawTarget of extractMarkdownTargets(markdown)) {
      if (/^(?:https?:|mailto:|data:)/i.test(rawTarget) || rawTarget.startsWith("#")) continue
      const targetWithoutAnchor = rawTarget.split("#", 1)[0]?.split("?", 1)[0] ?? ""
      if (!targetWithoutAnchor) continue
      let decoded = targetWithoutAnchor
      try {
        decoded = decodeURIComponent(targetWithoutAnchor)
      } catch {
        errors.push(`${relative(root, file)} has an invalid encoded link: ${rawTarget}`)
        continue
      }
      const absolute = decoded.startsWith("/")
        ? resolve(root, `.${decoded}`)
        : resolve(dirname(file), decoded)
      const repoRelative = relative(root, absolute).split(sep).join("/")
      if (repoRelative.startsWith("../")) {
        errors.push(`${relative(root, file)} links outside the repository: ${rawTarget}`)
        continue
      }
      if (REMOVED_SOL_ARCHIVE_PATHS.includes(repoRelative as typeof REMOVED_SOL_ARCHIVE_PATHS[number])) {
        errors.push(`${relative(root, file)} links to removed archive source ${repoRelative}`)
        continue
      }
      try {
        await stat(absolute)
      } catch {
        errors.push(`${relative(root, file)} has a broken internal link: ${rawTarget}`)
      }
    }
  }
  return errors
}

export function compareLiveIssues(snapshot: RoadmapIssueSnapshot, live: RoadmapIssue[]): string[] {
  const errors: string[] = []
  const expected = snapshot.issues
  if (!equalNumbers(expected.map((issue) => issue.number), live.map((issue) => issue.number))) {
    errors.push(`live GitHub set ${formatNumbers(live.map((issue) => issue.number))} differs from snapshot ${formatNumbers(expected.map((issue) => issue.number))}`)
    return errors
  }
  const liveByNumber = new Map(live.map((issue) => [issue.number, issue]))
  for (const issue of expected) {
    const actual = liveByNumber.get(issue.number)
    if (!actual) continue
    if (actual.title !== issue.title) errors.push(`snapshot title for #${issue.number} differs from live GitHub`)
    if (actual.url !== issue.url) errors.push(`snapshot URL for #${issue.number} differs from live GitHub`)
  }
  return errors
}

export function readLiveRoadmapIssues(snapshot: RoadmapIssueSnapshot): RoadmapIssue[] {
  const result = spawnSync("gh", [
    "issue", "list",
    "--repo", snapshot.repository,
    "--state", "open",
    "--label", snapshot.label,
    "--limit", "100",
    "--json", "number,title,url,state,labels",
  ], { encoding: "utf8" })
  if (result.status !== 0) {
    throw new Error(`gh issue list failed: ${result.stderr.trim() || "unknown error"}`)
  }
  const raw = JSON.parse(result.stdout) as Array<RoadmapIssue & { labels: Array<string | { name: string }> }>
  return raw
    .map((issue) => ({
      ...issue,
      state: "OPEN" as const,
      labels: issue.labels.map((label) => typeof label === "string" ? label : label.name),
    }))
    .filter((issue) => !snapshot.excludedLabels.some((label) => issue.labels.includes(label)))
    .sort((left, right) => left.number - right.number)
}

export async function collectSolDocErrors(options: CheckOptions): Promise<string[]> {
  const root = resolve(options.root)
  const solRoot = join(root, "docs/sol")
  const snapshot = options.snapshot ?? JSON.parse(await readFile(join(root, SNAPSHOT_PATH), "utf8")) as RoadmapIssueSnapshot
  const [master, issueIndex, receiptIndex, archiveManifest, preparedArchiveManifest] = await Promise.all([
    readFile(join(solRoot, "MASTER_ROADMAP.md"), "utf8"),
    readFile(join(solRoot, "issues/README.md"), "utf8"),
    readFile(join(solRoot, "receipts/README.md"), "utf8"),
    readFile(join(solRoot, "2026-07-12-july9-doctrine-extraction-and-backroom-manifest.md"), "utf8"),
    readFile(join(solRoot, "2026-07-12-july10-delegation-backroom-preparation-manifest.md"), "utf8"),
  ])
  const allMarkdownFiles = await markdownFilesUnder(solRoot)
  const allDocuments: Record<string, string> = {}
  for (const file of allMarkdownFiles) {
    allDocuments[relative(root, file).split(sep).join("/")] = await readFile(file, "utf8")
  }
  const activePaths = [
    "docs/sol/README.md",
    "docs/sol/OPERATING_MODEL.md",
    "docs/sol/SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md",
    "docs/sol/CLAIM_PROTOCOL.md",
    "docs/sol/CHALLENGE_LEDGER.md",
    "docs/sol/2026-07-10-r1-r2-identity-sync-contract.md",
    "docs/sol/2026-07-10-terra-execution-lane.md",
    "docs/sol/2026-07-11-openagents-coding-cutover-issue-plan.md",
    "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
    "docs/sol/issues/README.md",
    "docs/sol/receipts/README.md",
    "docs/sol/decisions/2026-07-10-greenfield-clients-and-sarah-removal.md",
  ]
  const activeDocuments = Object.fromEntries(activePaths.map((path) => [path, allDocuments[path] ?? ""]))
  const issueSourceFiles = (await readdir(join(solRoot, "issues")))
    .filter((file) => file.endsWith(".md") && file !== "README.md")

  const errors = [
    ...validateSnapshot(snapshot, options.now),
    ...validateMaster(master, snapshot),
    ...validateRevisionPins(activeDocuments),
    ...validateQueueOwnership(allDocuments),
    ...validateCleanAgentReading(allDocuments, snapshot),
    ...validateIssueIndex(issueIndex, issueSourceFiles, snapshot),
    ...validatePolicy({ "docs/sol/MASTER_ROADMAP.md": master, ...activeDocuments }),
    ...validateReceiptIndex(receiptIndex),
    ...validateArchiveManifest(archiveManifest),
    ...validatePreparedArchiveManifest(preparedArchiveManifest),
    ...await validateMarkdownLinks(root, allMarkdownFiles),
  ]
  try {
    const expectedManifest = buildSolDocumentManifest(root)
    const expectedBytes = serializeSolDocumentManifest(expectedManifest)
    const actualBytes = await readFile(join(root, SOL_DOCUMENT_MANIFEST_PATH), "utf8")
    if (actualBytes !== expectedBytes) {
      errors.push(`${SOL_DOCUMENT_MANIFEST_PATH} differs from deterministic generation`)
    }
    const dispatchOwners = expectedManifest.documents.filter((document) => document.dispatch)
    if (dispatchOwners.length !== 1 || dispatchOwners[0]?.path !== "docs/sol/MASTER_ROADMAP.md") {
      errors.push("document manifest must mark only the master as dispatch-capable")
    }
    for (const document of expectedManifest.documents) {
      if (
        !document.path || !document.class || !document.owner || !document.reviewedAt
        || !document.disposition || !document.status || !document.snapshot
        || !document.reviewTrigger || !/^[0-9a-f]{64}$/.test(document.sha256)
      ) {
        errors.push(`document manifest has an incomplete row for ${document.path || "unknown path"}`)
      }
      if (!Array.isArray(document.inboundLinks) || !Array.isArray(document.issueLinks)) {
        errors.push(`document manifest has invalid link lists for ${document.path}`)
      }
    }
  } catch (error) {
    errors.push(`document manifest validation failed: ${error instanceof Error ? error.message : String(error)}`)
  }
  for (const path of REMOVED_SOL_ARCHIVE_PATHS) {
    try {
      await stat(join(root, path))
      errors.push(`removed archived source exists in product repository: ${path}`)
    } catch {
      // Absence is the required state.
    }
  }
  return errors
}

async function main(): Promise<void> {
  const args = process.argv.slice(2)
  const rootIndex = args.indexOf("--root")
  const root = rootIndex >= 0 && args[rootIndex + 1] ? resolve(args[rootIndex + 1]!) : process.cwd()
  const snapshotPath = join(root, SNAPSHOT_PATH)
  let snapshot = JSON.parse(await readFile(snapshotPath, "utf8")) as RoadmapIssueSnapshot

  if (args.includes("--refresh-live")) {
    const issues = readLiveRoadmapIssues(snapshot)
    snapshot = { ...snapshot, generatedAt: new Date().toISOString(), issues }
    await writeFile(snapshotPath, `${JSON.stringify(snapshot, null, 2)}\n`)
    console.log(`[sol-docs] refreshed ${SNAPSHOT_PATH} with ${issues.length} product issues`)
  }

  const errors = await collectSolDocErrors({ root, snapshot })
  if (args.includes("--live") || args.includes("--refresh-live")) {
    try {
      errors.push(...compareLiveIssues(snapshot, readLiveRoadmapIssues(snapshot)))
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error))
    }
  }

  if (errors.length > 0) {
    console.error(`[sol-docs] FAILED with ${errors.length} error(s):`)
    for (const error of errors) console.error(`- ${error}`)
    process.exitCode = 1
    return
  }
  console.log(`[sol-docs] OK: ${snapshot.issues.length} product issues, offline freshness, classifications, receipts, policies, archive provenance, and links`)
}

if (import.meta.main) await main()
