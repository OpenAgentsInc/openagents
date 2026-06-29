import { effectAuthorityBoundaryAllowlist } from "./effect-authority-boundary-allowlist"

type Category =
  | "json-parse-cast"
  | "direct-env-read"
  | "bare-catch"
  | "raw-fetch"
  | "effect-run-promise"

type Finding = {
  readonly category: Category
  readonly path: string
  readonly line: number
  readonly snippet: string
  readonly allowedReason?: string
}

const authorityRoots = [
  "apps/openagents.com/workers/api",
  "apps/pylon/src",
  "packages/atif",
  "packages/mcp-contract",
  "packages/nip90",
  "packages/proof-replay",
  "packages/provider-account-schema",
  "packages/world-client",
  "packages/world-contract",
] as const

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
  ".wrangler",
  "dist",
  "node_modules",
  "coverage",
  "scripts",
])

const categoryLabels = {
  "json-parse-cast": "JSON.parse followed by cast/manual narrowing",
  "direct-env-read": "direct process.env/Bun.env read",
  "bare-catch": "bare catch block",
  "raw-fetch": "raw fetch call",
  "effect-run-promise": "Effect.runPromise bridge",
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

const allowlistReason = (finding: Omit<Finding, "allowedReason">): string | undefined => {
  const entry = effectAuthorityBoundaryAllowlist.find(
    (candidate) =>
      candidate.category === finding.category &&
      candidate.path === finding.path &&
      finding.snippet.includes(candidate.pattern),
  )

  return entry?.reason
}

const findJsonParseCast = (
  lines: readonly string[],
  path: string,
  index: number,
): Finding | undefined => {
  const line = lines[index] ?? ""
  if (isCommentLine(line) || !line.includes("JSON.parse")) {
    return undefined
  }

  const windowText = lines.slice(index, index + 4).join("\n")
  const suspiciousCast =
    /\)\s+as\s+[A-Za-z_{]/.test(windowText) ||
    /\bas\s+(Record|unknown|any|Readonly|Array|[A-Z][A-Za-z0-9_]+)/.test(windowText) ||
    /:\s*(Record|unknown|any|Readonly|Array|[A-Z][A-Za-z0-9_]+)\s*=/.test(windowText)

  if (!suspiciousCast) {
    return undefined
  }

  return {
    category: "json-parse-cast",
    path,
    line: index + 1,
    snippet: normalizeSnippet(line),
  }
}

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

  if (/\b(process\.env|Bun\.env)\b/.test(line)) {
    findings.push({ category: "direct-env-read", path, line: index + 1, snippet })
  }

  if (/catch\s*\{\s*\}/.test(line) || /catch\s*\{\s*$/.test(line)) {
    const nextMeaningful = lines
      .slice(index + 1, index + 5)
      .map((candidate) => candidate.trim())
      .find((candidate) => candidate.length > 0 && !candidate.startsWith("//"))
    if (nextMeaningful === "}" || /catch\s*\{\s*\}/.test(line)) {
      findings.push({ category: "bare-catch", path, line: index + 1, snippet })
    }
  }

  if (/\bfetch\s*\(/.test(line)) {
    findings.push({ category: "raw-fetch", path, line: index + 1, snippet })
  }

  if (/\bEffect\.runPromise\s*\(/.test(line)) {
    findings.push({ category: "effect-run-promise", path, line: index + 1, snippet })
  }

  const jsonParseCast = findJsonParseCast(lines, path, index)
  if (jsonParseCast) {
    findings.push(jsonParseCast)
  }

  return findings
}

const collectFindings = async (): Promise<readonly Finding[]> => {
  const findings: Finding[] = []

  for (const root of authorityRoots) {
    for (const absolutePath of await walk(root)) {
      const path = relativePath(absolutePath)
      const text = await Bun.file(absolutePath).text()
      const lines = text.split(/\r?\n/)

      for (let index = 0; index < lines.length; index += 1) {
        for (const finding of findLineFindings(lines, path, index)) {
          findings.push({
            ...finding,
            allowedReason: allowlistReason(finding),
          })
        }
      }
    }
  }

  return findings
}

const printReport = (findings: readonly Finding[]): void => {
  const byCategory = new Map<Category, readonly Finding[]>()
  for (const category of Object.keys(categoryLabels) as readonly Category[]) {
    byCategory.set(
      category,
      findings.filter((finding) => finding.category === category),
    )
  }

  console.log("Effect authority-boundary scan")
  console.log("mode: report-only")
  console.log(`scanned roots: ${authorityRoots.join(", ")}`)
  console.log(`findings: ${findings.length}`)

  for (const [category, categoryFindings] of byCategory) {
    if (categoryFindings.length === 0) {
      continue
    }

    const allowed = categoryFindings.filter((finding) => finding.allowedReason).length
    console.log("")
    console.log(`${categoryLabels[category]}: ${categoryFindings.length} (${allowed} allowed)`)

    for (const finding of categoryFindings) {
      const marker = finding.allowedReason ? "ALLOWLISTED" : "MIGRATE"
      console.log(
        `${marker} ${finding.path}:${finding.line} ${finding.snippet}`,
      )
      if (finding.allowedReason) {
        console.log(`  reason: ${finding.allowedReason}`)
      }
    }
  }

  if (findings.length === 0) {
    console.log("No suspicious authority-boundary operations found.")
  }
}

const findings = await collectFindings()
printReport(findings)
