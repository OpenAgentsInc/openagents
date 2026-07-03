import { createHash } from "node:crypto"
import { readdir, readFile, writeFile } from "node:fs/promises"
import { dirname, extname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"

type Category =
  | "json-parse-cast"
  | "bare-catch"
  | "direct-env-read"
  | "date-now-in-logic"
  | "effect-run-promise"
  | "set-timeout-kill"

type Finding = {
  readonly id: string
  readonly category: Category
  readonly path: string
  readonly line: number
  readonly snippet: string
}

type RawFinding = Omit<Finding, "id">

type Allowlist = {
  readonly schema: "khala-code-architecture-scan-allowlist.v1"
  readonly description: string
  readonly counts: Partial<Record<Category, number>>
  readonly entries: readonly Finding[]
}

const scriptDir = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(scriptDir, "../../..")
const allowlistPath = join(scriptDir, "architecture-scan.allowlist.json")

const scanRoots = [
  "clients/khala-code-desktop",
  "packages/khala-tools",
] as const

const sourceExtensions = new Set([".ts", ".tsx"])

const ignoredPathParts = new Set([
  ".git",
  "dist",
  "node_modules",
  "coverage",
])

const scanRules = [
  {
    category: "json-parse-cast",
    regex: /JSON\.parse\([\s\S]{0,240}?\)\s+as\b/g,
  },
  {
    category: "bare-catch",
    regex: /\bcatch\s*(?:\([^)]*\)\s*)?\{\s*\}/g,
  },
  {
    category: "direct-env-read",
    regex: /\b(?:process|Bun)\.env\b|\bimport\.meta\.env\b/g,
  },
  {
    category: "date-now-in-logic",
    regex: /\bDate\.now\(\)/g,
  },
  {
    category: "effect-run-promise",
    regex: /\bEffect\.runPromise\(/g,
  },
  {
    category: "set-timeout-kill",
    regex: /\bsetTimeout\s*\([\s\S]{0,400}\b(?:killTree|process\.kill|kill|\.kill)\b/g,
  },
] satisfies readonly { readonly category: Category; readonly regex: RegExp }[]

const isSourceFile = (path: string): boolean => {
  if (!sourceExtensions.has(extname(path))) {
    return false
  }

  return (
    !path.endsWith(".d.ts") &&
    !/\.test\.tsx?$/.test(path) &&
    !/\.test-support\.tsx?$/.test(path) &&
    !/\.story\.test\.tsx?$/.test(path) &&
    !/\.scene\.test\.tsx?$/.test(path)
  )
}

const walk = async (root: string): Promise<readonly string[]> => {
  const files: string[] = []

  const visit = async (absoluteDir: string): Promise<void> => {
    const entries = await readdir(absoluteDir, { withFileTypes: true })

    for (const entry of entries) {
      if (ignoredPathParts.has(entry.name)) {
        continue
      }

      const absolutePath = join(absoluteDir, entry.name)
      if (entry.isDirectory()) {
        await visit(absolutePath)
      } else if (entry.isFile() && isSourceFile(absolutePath)) {
        files.push(absolutePath)
      }
    }
  }

  await visit(root)
  return files
}

const lineForIndex = (source: string, index: number): number =>
  source.slice(0, index).split("\n").length

const snippetForMatch = (matchText: string): string => {
  const text = matchText.replace(/\s+/g, " ").trim()
  return text.length > 180 ? `${text.slice(0, 177)}...` : text
}

const collectRawFindings = async (): Promise<readonly RawFinding[]> => {
  const findings: RawFinding[] = []

  for (const root of scanRoots) {
    const absoluteRoot = join(repoRoot, root)
    for (const absolutePath of await walk(absoluteRoot)) {
      const path = relative(repoRoot, absolutePath)
      const source = await readFile(absolutePath, "utf8")

      for (const rule of scanRules) {
        rule.regex.lastIndex = 0
        for (const match of source.matchAll(rule.regex)) {
          findings.push({
            category: rule.category,
            path,
            line: lineForIndex(source, match.index ?? 0),
            snippet: snippetForMatch(match[0] ?? ""),
          })
        }
      }
    }
  }

  return findings.sort(compareFinding)
}

const compareFinding = (left: RawFinding, right: RawFinding): number =>
  left.category.localeCompare(right.category) ||
  left.path.localeCompare(right.path) ||
  left.line - right.line ||
  left.snippet.localeCompare(right.snippet)

const stableId = (
  finding: RawFinding,
  occurrenceIndex: number,
): string =>
  createHash("sha256")
    .update(`${finding.category}\0${finding.path}\0${finding.snippet}\0${occurrenceIndex}`)
    .digest("hex")
    .slice(0, 16)

const withStableIds = (findings: readonly RawFinding[]): readonly Finding[] => {
  const occurrences = new Map<string, number>()

  return findings.map((finding) => {
    const key = `${finding.category}\0${finding.path}\0${finding.snippet}`
    const occurrenceIndex = occurrences.get(key) ?? 0
    occurrences.set(key, occurrenceIndex + 1)
    return {
      id: stableId(finding, occurrenceIndex),
      ...finding,
    }
  })
}

const countByCategory = (findings: readonly Finding[]): Partial<Record<Category, number>> => {
  const counts: Partial<Record<Category, number>> = {}
  for (const finding of findings) {
    counts[finding.category] = (counts[finding.category] ?? 0) + 1
  }
  return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right))) as Partial<
    Record<Category, number>
  >
}

const categories = new Set<Category>([
  "json-parse-cast",
  "bare-catch",
  "direct-env-read",
  "date-now-in-logic",
  "effect-run-promise",
  "set-timeout-kill",
])

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

const isCategory = (value: unknown): value is Category =>
  typeof value === "string" && categories.has(value as Category)

const isFinding = (value: unknown): value is Finding => {
  if (!isRecord(value)) {
    return false
  }

  return (
    typeof value.id === "string" &&
    isCategory(value.category) &&
    typeof value.path === "string" &&
    typeof value.line === "number" &&
    typeof value.snippet === "string"
  )
}

const isAllowlist = (value: unknown): value is Allowlist => {
  if (!isRecord(value)) {
    return false
  }

  return (
    value.schema === "khala-code-architecture-scan-allowlist.v1" &&
    typeof value.description === "string" &&
    isRecord(value.counts) &&
    Array.isArray(value.entries) &&
    value.entries.every(isFinding)
  )
}

const readAllowlist = async (): Promise<Allowlist> => {
  const raw = await readFile(allowlistPath, "utf8")
  const parsed: unknown = JSON.parse(raw)
  if (!isAllowlist(parsed)) {
    throw new Error(`Unsupported architecture scan allowlist schema in ${allowlistPath}`)
  }
  return parsed
}

const makeAllowlist = (findings: readonly Finding[]): Allowlist => ({
  schema: "khala-code-architecture-scan-allowlist.v1",
  description:
    "Grandfathered Khala Code architecture scan findings. Run `bun run --cwd clients/khala-code-desktop scan:architecture -- --update-allowlist` after removing a finding, review the count decrease, and keep verify green.",
  counts: countByCategory(findings),
  entries: findings,
})

const formatFinding = (finding: Finding): string =>
  `${finding.category} ${finding.path}:${finding.line} ${finding.id}\n  ${finding.snippet}`

const assertAllowlistCounts = (allowlist: Allowlist): readonly string[] => {
  const actual = countByCategory(allowlist.entries)
  const expected = allowlist.counts
  const categories = new Set([...Object.keys(actual), ...Object.keys(expected)])
  const errors: string[] = []

  for (const category of Array.from(categories).sort()) {
    const actualCount = actual[category as Category] ?? 0
    const expectedCount = expected[category as Category] ?? 0
    if (actualCount !== expectedCount) {
      errors.push(`${category}: counts says ${expectedCount}, entries contain ${actualCount}`)
    }
  }

  return errors
}

const main = async (): Promise<void> => {
  const args = new Set(process.argv.slice(2))
  const findings = withStableIds(await collectRawFindings())

  if (args.has("--update-allowlist")) {
    const next = `${JSON.stringify(makeAllowlist(findings), null, 2)}\n`
    await writeFile(allowlistPath, next)
    console.log(`Updated ${relative(repoRoot, allowlistPath)} with ${findings.length} findings.`)
    return
  }

  const allowlist = await readAllowlist()
  const countErrors = assertAllowlistCounts(allowlist)
  const currentIds = new Set(findings.map((finding) => finding.id))
  const allowedIds = new Set(allowlist.entries.map((finding) => finding.id))
  const newFindings = findings.filter((finding) => !allowedIds.has(finding.id))
  const staleFindings = allowlist.entries.filter((finding) => !currentIds.has(finding.id))

  if (countErrors.length === 0 && newFindings.length === 0 && staleFindings.length === 0) {
    console.log(
      `Khala Code architecture scan passed: ${findings.length} grandfathered findings, zero new violations.`,
    )
    return
  }

  console.error("Khala Code architecture scan failed.")
  for (const error of countErrors) {
    console.error(`\nAllowlist count mismatch: ${error}`)
  }

  if (newFindings.length > 0) {
    console.error(`\nNew architecture violations (${newFindings.length}):`)
    for (const finding of newFindings) {
      console.error(formatFinding(finding))
    }
  }

  if (staleFindings.length > 0) {
    console.error(`\nStale allowlist entries (${staleFindings.length}); shrink the ratchet:`)
    for (const finding of staleFindings) {
      console.error(formatFinding(finding))
    }
  }

  console.error(
    "\nRun `bun run --cwd clients/khala-code-desktop scan:architecture -- --update-allowlist` only after reviewing intentional debt removal or baseline changes.",
  )
  process.exit(1)
}

await main()
