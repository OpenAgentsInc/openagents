/**
 * Bun-API perimeter scan (BUN-1, openagents#8779) — the containment gate for
 * Option C of docs/fable/2026-07-13-bun-vs-vite-plus-analysis.md.
 *
 * Inventories `bun:*` / `"bun"` module imports and `Bun.*` global API usage
 * in production sources under apps/, packages/, and clients/. Existing usage
 * is grandfathered in scripts/bun-api-perimeter-allowlist.ts as a checked-in
 * burn-down list; named seam files carry explicit perimeter entries. The
 * scan FAILS (exit 1) on any NEW un-allowlisted usage, so Bun-specific APIs
 * can only enter production source through a deliberate allowlist diff.
 *
 * Root scripts/ are excluded by charter: the repo's script surface is Bun
 * (127/130 at analysis time) and is already inside the perimeter.
 *
 * Usage:
 *   bun run scan:bun-api-perimeter          # scan + enforce
 *   bun scripts/bun-api-perimeter-scan.ts --emit-allowlist
 *       # print a regenerated grandfathered list (for burn-down maintenance;
 *       # any additions must survive code review)
 *   bun scripts/bun-api-perimeter-scan.ts <root...>   # scan custom roots
 *       # (used by the fixture test; allowlist paths won't match, so any
 *       # usage in the custom roots is reported as NEW)
 *
 * Note: merges into the oxlint-plugin-openagents rule set when #8773 lands.
 */
import {
  bunApiGrandfathered,
  bunApiPerimeter,
} from "./bun-api-perimeter-allowlist"

type Category = "bun-import" | "bun-global"

type Finding = {
  readonly category: Category
  readonly path: string
  readonly line: number
  readonly snippet: string
}

const defaultRoots = ["apps", "packages", "clients"] as const

const sourceExtensions = new Set([
  ".cjs",
  ".cts",
  ".js",
  ".jsx",
  ".mjs",
  ".mts",
  ".ts",
  ".tsx",
])

const ignoredPathParts = new Set([
  ".git",
  ".expo",
  ".next",
  ".wrangler",
  "build",
  "coverage",
  "dist",
  "node_modules",
])

const categoryLabels = {
  "bun-import": "bun:* / \"bun\" module import",
  "bun-global": "Bun.* global API usage",
} satisfies Record<Category, string>

const relativePath = (path: string): string =>
  path.startsWith(`${process.cwd()}/`) ? path.slice(process.cwd().length + 1) : path

const hasSourceExtension = (path: string): boolean => {
  const dot = path.lastIndexOf(".")
  return dot >= 0 && sourceExtensions.has(path.slice(dot))
}

const isProductionSource = (path: string): boolean =>
  !/\.(test|spec)\.[cm]?[tj]sx?$/.test(path) &&
  !path.includes("/__tests__/") &&
  !path.includes("/fixtures/") &&
  !path.includes("/generated/")

const walk = async (root: string): Promise<readonly string[]> => {
  const files: string[] = []

  const visit = async (dir: string): Promise<void> => {
    let entries: string[]
    try {
      entries = await Array.fromAsync(new Bun.Glob("*").scan({ cwd: dir, onlyFiles: false }))
    } catch {
      return
    }

    for (const entry of entries) {
      if (ignoredPathParts.has(entry)) {
        continue
      }

      const path = `${dir}/${entry}`
      const stat = await Bun.file(path).stat()
      if (stat.isDirectory()) {
        await visit(path)
      } else if (stat.isFile() && hasSourceExtension(path) && isProductionSource(path)) {
        files.push(path)
      }
    }
  }

  await visit(root)
  return files
}

const isCommentLine = (line: string): boolean => {
  const trimmed = line.trim()
  return (
    trimmed.startsWith("//") ||
    trimmed.startsWith("*") ||
    trimmed.startsWith("/*") ||
    trimmed.startsWith("#")
  )
}

const normalizeSnippet = (line: string): string => line.trim().replace(/\s+/g, " ")

const bunImportPattern =
  /(?:\bfrom\s*|\bimport\s*\(\s*|\brequire(?:Module)?\s*\(\s*|^\s*import\s+)["'](?:bun:[a-z0-9-]+|bun)["']/

const bunGlobalPattern = /\bBun\.[A-Za-z_$]/

const findLineFindings = (
  lines: readonly string[],
  path: string,
  index: number,
): readonly Finding[] => {
  const line = lines[index] ?? ""
  if (isCommentLine(line)) {
    return []
  }

  const findings: Finding[] = []
  const snippet = normalizeSnippet(line)

  if (bunImportPattern.test(line)) {
    findings.push({ category: "bun-import", path, line: index + 1, snippet })
  }

  if (bunGlobalPattern.test(line)) {
    findings.push({ category: "bun-global", path, line: index + 1, snippet })
  }

  return findings
}

const collectFindings = async (roots: readonly string[]): Promise<readonly Finding[]> => {
  const findings: Finding[] = []

  for (const root of roots) {
    for (const absolutePath of await walk(root)) {
      const path = relativePath(absolutePath)
      const text = await Bun.file(absolutePath).text()
      const lines = text.split(/\r?\n/)

      for (let index = 0; index < lines.length; index += 1) {
        findings.push(...findLineFindings(lines, path, index))
      }
    }
  }

  return findings
}

const perimeterReason = (finding: Finding): string | undefined =>
  bunApiPerimeter.find(
    (entry) => entry.path === finding.path && entry.category === finding.category,
  )?.reason

const isGrandfathered = (finding: Finding): boolean =>
  bunApiGrandfathered.some(
    (entry) => entry.path === finding.path && entry.categories.includes(finding.category),
  )

const emitAllowlist = (findings: readonly Finding[]): void => {
  const byPath = new Map<string, Set<Category>>()
  for (const finding of findings) {
    if (perimeterReason(finding) !== undefined) {
      continue
    }
    const categories = byPath.get(finding.path) ?? new Set<Category>()
    categories.add(finding.category)
    byPath.set(finding.path, categories)
  }

  const paths = [...byPath.keys()].sort()
  console.log("export const bunApiGrandfathered = [")
  for (const path of paths) {
    const categories = [...(byPath.get(path) ?? [])].sort()
    const rendered = categories.map((category) => `"${category}"`).join(", ")
    console.log(`  { path: "${path}", categories: [${rendered}] },`)
  }
  console.log("] as const")
}

const printReport = (findings: readonly Finding[]): number => {
  const newViolations = findings.filter(
    (finding) => perimeterReason(finding) === undefined && !isGrandfathered(finding),
  )
  const perimeterCount = findings.filter(
    (finding) => perimeterReason(finding) !== undefined,
  ).length
  const grandfatheredFiles = new Set(
    findings
      .filter(
        (finding) => perimeterReason(finding) === undefined && isGrandfathered(finding),
      )
      .map((finding) => finding.path),
  )
  const staleEntries = bunApiGrandfathered.filter(
    (entry) =>
      !findings.some(
        (finding) =>
          finding.path === entry.path && entry.categories.includes(finding.category),
      ),
  )

  console.log("Bun-API perimeter scan")
  console.log("mode: enforce (new violations fail; grandfathered usage is a burn-down list)")
  console.log(`findings: ${findings.length}`)
  console.log(`perimeter (named seam) findings: ${perimeterCount}`)
  console.log(
    `grandfathered files: ${grandfatheredFiles.size} of ${bunApiGrandfathered.length} allowlisted`,
  )
  console.log(`new violations: ${newViolations.length}`)

  for (const entry of staleEntries) {
    console.log(
      `STALE allowlist entry (burned down? remove it): ${entry.path} [${entry.categories.join(", ")}]`,
    )
  }

  if (newViolations.length > 0) {
    for (const category of Object.keys(categoryLabels) as readonly Category[]) {
      const categoryViolations = newViolations.filter(
        (finding) => finding.category === category,
      )
      if (categoryViolations.length === 0) {
        continue
      }
      console.log("")
      console.log(`${categoryLabels[category]}: ${categoryViolations.length} NEW`)
      for (const finding of categoryViolations) {
        console.log(`NEW ${finding.path}:${finding.line} ${finding.snippet}`)
      }
    }
    console.log("")
    console.log(
      "Bun-specific APIs must stay behind named seams (see @openagentsinc/sqlite-runtime).",
    )
    console.log(
      "Either route through a seam or add a reviewed perimeter entry in scripts/bun-api-perimeter-allowlist.ts.",
    )
    return 1
  }

  return 0
}

const args = process.argv.slice(2)
const emit = args.includes("--emit-allowlist")
const rootArgs = args.filter((arg) => !arg.startsWith("--"))
const roots = rootArgs.length > 0 ? rootArgs : defaultRoots

const findings = await collectFindings(roots)
if (emit) {
  emitAllowlist(findings)
  process.exit(0)
}
process.exit(printReport(findings))
