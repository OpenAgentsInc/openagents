#!/usr/bin/env node

import { spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, relative, resolve, sep } from "node:path"

export const SOL_DOCUMENT_MANIFEST_PATH = "docs/sol/document-manifest.json"
export const SOL_DOCUMENT_POLICY_PATH = "docs/sol/document-manifest-policy.json"

type Disposition = {
  class: string
  owner: string
  disposition: string
  dispatch: boolean
}

const DOCUMENT_CLASSES = new Set([
  "authority", "contract", "index", "current-status", "receipt",
  "historical-analysis", "redirect", "tombstone", "backroom-export",
])

type ManifestPolicy = {
  schemaVersion: number
  inventoryReviewDate: string
  defaultOwner: string
  overrides: Record<string, Disposition>
}

export type SolDocumentManifestEntry = Disposition & {
  path: string
  sha256: string
  status: string
  snapshot: string
  reviewedAt: string
  reviewTrigger: string
  inboundLinks: string[]
  inboundSourceCount: number
  issueLinks: number[]
}

export type SolDocumentManifest = {
  schemaVersion: 1
  sourceRoot: "docs/sol"
  inventoryReviewDate: string
  sourceTreeSha256: string
  documents: SolDocumentManifestEntry[]
}

function normalized(path: string): string {
  return path.split(sep).join("/")
}

function gitMarkdownFiles(root: string): string[] {
  const result = spawnSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "--", "*.md"],
    { cwd: root, encoding: "utf8" },
  )
  if (result.status !== 0) throw new Error(`git ls-files failed: ${result.stderr.trim()}`)
  return [...new Set(result.stdout.split("\n").filter(Boolean))].sort()
}

function extractSection(markdown: string, heading: string): string {
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

function markdownTargets(markdown: string): string[] {
  const targets: string[] = []
  const pattern = /(?<!!)\[[^\]]*\]\((<[^>]+>|[^)\s]+)(?:\s+["'][^)]*)?\)/g
  for (const match of markdown.matchAll(pattern)) {
    const raw = match[1]
    if (!raw || /^(?:https?:|mailto:|data:|#)/i.test(raw)) continue
    targets.push(raw.startsWith("<") && raw.endsWith(">") ? raw.slice(1, -1) : raw)
  }
  return targets
}

function resolveTarget(root: string, source: string, rawTarget: string): string | undefined {
  const target = rawTarget.split("#", 1)[0]?.split("?", 1)[0]
  if (!target) return undefined
  let decoded = target
  try {
    decoded = decodeURIComponent(target)
  } catch {
    return undefined
  }
  const absolute = decoded.startsWith("/")
    ? resolve(root, `.${decoded}`)
    : resolve(root, dirname(source), decoded)
  const repoPath = normalized(relative(root, absolute))
  return repoPath.startsWith("../") ? undefined : repoPath
}

function localMarkdownTargets(root: string, source: string, markdown: string): string[] {
  return markdownTargets(markdown)
    .map((target) => resolveTarget(root, source, target))
    .filter((target): target is string => Boolean(target?.endsWith(".md")))
}

function sectionTargets(root: string, indexPath: string, markdown: string, heading: string): string[] {
  return localMarkdownTargets(root, indexPath, extractSection(markdown, heading))
}

function issueNumbers(markdown: string): number[] {
  return [...new Set(
    [...markdown.matchAll(/(?:https:\/\/github\.com\/OpenAgentsInc\/openagents\/issues\/|(?<![\w./-])#)(\d{3,})\b/g)]
      .map((match) => Number(match[1])),
  )].sort((left, right) => left - right)
}

function metadata(markdown: string, field: string): string | undefined {
  const header = markdown.split("\n").slice(0, 35).join("\n")
  return header.match(new RegExp(`^- ${field}:\\s*(.+)$`, "m"))?.[1]?.trim()
}

function reviewTrigger(disposition: string): string {
  if (disposition === "archive-candidate") return "before archive import, link migration, or deletion"
  if (disposition.includes("tombstone")) return "only on explicit owner revival"
  if (disposition.includes("evidence") || disposition.includes("historical")) {
    return "only on proof invalidation, reopen, provenance correction, or archive selection"
  }
  if (disposition.includes("index")) return "whenever indexed membership, path, or classification changes"
  if (disposition.includes("redirect")) return "whenever its replacement target changes"
  if (disposition.includes("manifest")) return "before any governed archive transition"
  return "whenever the owning authority, contract, issue state, or acceptance boundary changes"
}

function classifiedIssuePaths(root: string): Map<string, Disposition> {
  const indexPath = "docs/sol/issues/README.md"
  const index = readFileSync(resolve(root, indexPath), "utf8")
  const sections: Array<[string, string, string]> = [
    ["## Live issue sources", "contract", "retain-live-source"],
    ["## Closed proof and implementation sources", "receipt", "retain-evidence"],
    ["## Closed non-revival tombstones", "tombstone", "retain-tombstone"],
    ["## Architecture reference", "historical-analysis", "retain-reference"],
  ]
  const result = new Map<string, Disposition>()
  for (const [heading, documentClass, disposition] of sections) {
    const section = extractSection(index, heading)
    for (const path of sectionTargets(root, indexPath, index, heading)) {
      if (result.has(path)) throw new Error(`${path} appears in multiple issue classifications`)
      const basename = path.split("/").at(-1) ?? path
      const owningLine = section.split("\n").find((line) => line.includes(`./${basename}`)) ?? ""
      const ownerIssue = issueNumbers(owningLine)[0]
      result.set(path, {
        class: documentClass,
        owner: ownerIssue ? `Sol issue #${ownerIssue}` : "Sol issue source",
        disposition,
        dispatch: false,
      })
    }
  }
  return result
}

function receiptPaths(root: string): Map<string, string> {
  const path = "docs/sol/receipts/README.md"
  const markdown = readFileSync(resolve(root, path), "utf8")
  const result = new Map<string, string>()
  for (const target of localMarkdownTargets(root, path, markdown)) {
    const basename = target.split("/").at(-1) ?? target
    const row = markdown.split("\n").find((line) => line.includes(`../${basename}`)) ?? ""
    const ownerIssue = issueNumbers(row)[0]
    result.set(target, ownerIssue ? `Sol proof / issue #${ownerIssue}` : "Sol proof")
  }
  return result
}

function derivedDisposition(
  path: string,
  markdown: string,
  policy: ManifestPolicy,
  issueClasses: Map<string, Disposition>,
  receipts: Map<string, string>,
): Disposition {
  const override = policy.overrides[path]
  if (override) return override
  const issueClass = issueClasses.get(path)
  if (issueClass) return issueClass
  if (path === "docs/sol/issues/README.md") {
    return { class: "index", owner: policy.defaultOwner, disposition: "retain-active-index", dispatch: false }
  }
  if (path === "docs/sol/receipts/README.md") {
    return { class: "index", owner: "Sol proof", disposition: "retain-active-index", dispatch: false }
  }
  if (path.startsWith("docs/sol/decisions/")) {
    return { class: "contract", owner: "Sol owner decisions", disposition: "retain-decision", dispatch: false }
  }
  if (receipts.has(path)) {
    return { class: "receipt", owner: receipts.get(path) ?? "Sol proof", disposition: "retain-evidence", dispatch: false }
  }
  throw new Error(`unclassified Sol document: ${path}; add declared/index classification or a reviewed policy override`)
}

export function buildSolDocumentManifest(root: string): SolDocumentManifest {
  const absoluteRoot = resolve(root)
  const policy = JSON.parse(
    readFileSync(resolve(absoluteRoot, SOL_DOCUMENT_POLICY_PATH), "utf8"),
  ) as ManifestPolicy
  if (policy.schemaVersion !== 1) throw new Error("document manifest policy schemaVersion must be 1")
  for (const [path, classification] of Object.entries(policy.overrides)) {
    if (!DOCUMENT_CLASSES.has(classification.class)) {
      throw new Error(`${path} has unsupported class ${classification.class}`)
    }
  }

  const allMarkdown = gitMarkdownFiles(absoluteRoot)
    .filter((path) => existsSync(resolve(absoluteRoot, path)))
  const solDocuments = allMarkdown.filter((path) => path.startsWith("docs/sol/"))
  const issueClasses = classifiedIssuePaths(absoluteRoot)
  const receipts = receiptPaths(absoluteRoot)
  const inbound = new Map<string, Set<string>>(solDocuments.map((path) => [path, new Set()]))

  for (const source of allMarkdown) {
    const markdown = readFileSync(resolve(absoluteRoot, source), "utf8")
    for (const target of localMarkdownTargets(absoluteRoot, source, markdown)) {
      if (target !== source && inbound.has(target)) inbound.get(target)!.add(source)
    }
  }

  const documents = solDocuments.map((path): SolDocumentManifestEntry => {
    const markdown = readFileSync(resolve(absoluteRoot, path), "utf8")
    const classification = derivedDisposition(path, markdown, policy, issueClasses, receipts)
    const source = metadata(markdown, "Source snapshot")
      ?? metadata(markdown, "Snapshot")
      ?? metadata(markdown, "Base")
      ?? metadata(markdown, "Updated")
      ?? metadata(markdown, "Date")
      ?? "current checked-in content"
    return {
      path,
      ...classification,
      sha256: createHash("sha256").update(markdown).digest("hex"),
      status: metadata(markdown, "Final disposition")
        ?? metadata(markdown, "Status")
        ?? classification.disposition,
      snapshot: source,
      reviewedAt: policy.inventoryReviewDate,
      reviewTrigger: reviewTrigger(classification.disposition),
      inboundLinks: [...(inbound.get(path) ?? [])].sort(),
      inboundSourceCount: inbound.get(path)?.size ?? 0,
      issueLinks: issueNumbers(markdown),
    }
  })

  const paths = documents.map((document) => document.path)
  if (new Set(paths).size !== paths.length) throw new Error("document manifest contains duplicate paths")
  for (const path of Object.keys(policy.overrides)) {
    if (!paths.includes(path)) throw new Error(`document manifest policy override targets missing ${path}`)
  }
  const sourceTreeSha256 = createHash("sha256")
    .update(documents.map((document) => `${document.path}\0${document.sha256}`).join("\n"))
    .digest("hex")
  return {
    schemaVersion: 1,
    sourceRoot: "docs/sol",
    inventoryReviewDate: policy.inventoryReviewDate,
    sourceTreeSha256,
    documents,
  }
}

export function serializeSolDocumentManifest(manifest: SolDocumentManifest): string {
  return `${JSON.stringify(manifest, null, 2)}\n`
}

function repositoryRoot(): string {
  const result = spawnSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" })
  if (result.status !== 0) throw new Error("run generate-sol-doc-manifest inside the repository")
  return result.stdout.trim()
}

if (import.meta.main) {
  const root = repositoryRoot()
  const output = serializeSolDocumentManifest(buildSolDocumentManifest(root))
  const path = resolve(root, SOL_DOCUMENT_MANIFEST_PATH)
  if (process.argv.includes("--check")) {
    if (!existsSync(path) || readFileSync(path, "utf8") !== output) {
      console.error(`${SOL_DOCUMENT_MANIFEST_PATH} is stale; run bun run generate:sol-doc-manifest`)
      process.exit(1)
    }
    console.log(`check:sol-doc-manifest OK (${JSON.parse(output).documents.length} documents)`)
  } else {
    writeFileSync(path, output)
    console.log(`generated ${SOL_DOCUMENT_MANIFEST_PATH} (${JSON.parse(output).documents.length} documents)`)
  }
}
