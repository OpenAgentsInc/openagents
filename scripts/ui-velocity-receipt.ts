import { execFileSync } from "node:child_process"

export type GitUiEntry = {
  hash: string
  date: Date
  subject: string
  prs: ReadonlyArray<number>
}

export type UiPullRequest = {
  number: number
  title?: string
  createdAt: string
  mergedAt: string | null
  url?: string
  files: ReadonlyArray<string>
  reviews?: ReadonlyArray<unknown>
}

export type UiVelocityWindowSummary = {
  start: string
  end: string
  gitUiFirstParentCommitCount: number
  uiPrCount: number
  uiPrsByPathPrefix: Record<string, number>
  directOrNoPrUiCommitCount: number
  cycleMinutes: {
    count: number
    average: number | null
    median: number | null
    p75: number | null
    min: number | null
    max: number | null
  }
}

export type UiVelocityReceipt = {
  schema: "openagents.ui_velocity_receipt.v1"
  measurementState: "measured" | "not_eligible"
  repo: string
  ref: string
  cutoff: string
  pathFilters: ReadonlyArray<string>
  windows: ReadonlyArray<UiVelocityWindowSummary>
  eligibility?: {
    eraStart: string
    requiredAgeDays: number
    actualAgeDays: number
    earliestEligibleCutoff: string
    reason: string
  }
}

type CliOptions = {
  repo: string
  ref: string
  cutoff: Date
  pathFilters: ReadonlyArray<string>
  windowDays: ReadonlyArray<number>
  eraStart?: Date
  requireEraDays?: number
}

export const normalizePathPrefix = (prefix: string): string => {
  const normalized = prefix.replace(/\\/g, "/").replace(/^\.\/+/, "")
  return normalized.endsWith("/") ? normalized : `${normalized}/`
}

export const parsePrNumbersFromSubject = (
  subject: string,
): ReadonlyArray<number> =>
  [
    ...subject.matchAll(/#(\d+)|origin\/pr\/(\d+)/g),
  ]
    .map((match) => Number(match[1] ?? match[2]))
    .filter((number) => Number.isSafeInteger(number))

export const pathMatchesAnyPrefix = (
  filePath: string,
  pathFilters: ReadonlyArray<string>,
): boolean => {
  const normalizedPath = filePath.replace(/\\/g, "/").replace(/^\.\/+/, "")
  return pathFilters
    .map(normalizePathPrefix)
    .some((prefix) => normalizedPath.startsWith(prefix))
}

export const parseGitUiLog = (log: string): ReadonlyArray<GitUiEntry> =>
  log
    .trim()
    .split(/\n/)
    .filter(Boolean)
    .map((line) => {
      const [hash, date, ...rest] = line.split("\t")
      const subject = rest.join("\t")

      return {
        hash,
        date: new Date(date),
        subject,
        prs: parsePrNumbersFromSubject(subject),
      }
    })

export const quantile = (
  values: ReadonlyArray<number>,
  q: number,
): number | null => {
  if (values.length === 0) {
    return null
  }

  const sorted = values.slice().sort((a, b) => a - b)
  const pos = (sorted.length - 1) * q
  const base = Math.floor(pos)
  const rest = pos - base
  const next = sorted[base + 1]

  return next === undefined ? sorted[base] : sorted[base] + rest * (next - sorted[base])
}

export const roundOneDecimal = (value: number | null): number | null =>
  value === null ? null : Math.round(value * 10) / 10

export const summarizeUiVelocityWindow = (input: {
  cutoff: Date
  start: Date
  entries: ReadonlyArray<GitUiEntry>
  pullRequests: ReadonlyArray<UiPullRequest>
  pathFilters: ReadonlyArray<string>
}): UiVelocityWindowSummary => {
  const normalizedFilters = input.pathFilters.map(normalizePathPrefix)
  const pullRequests = input.pullRequests.filter((pr) => {
    if (pr.mergedAt === null) {
      return false
    }

    const merged = new Date(pr.mergedAt)
    return merged >= input.start && merged < input.cutoff
  })

  const allEntries = input.entries.filter(
    (entry) => entry.date >= input.start && entry.date < input.cutoff,
  )
  const directEntries = allEntries.filter((entry) => entry.prs.length === 0)
  const cycles = pullRequests.map(
    (pr) => (new Date(pr.mergedAt as string).getTime() - new Date(pr.createdAt).getTime()) /
      60_000,
  )
  const cycleTotal = cycles.reduce((total, value) => total + value, 0)

  return {
    start: input.start.toISOString(),
    end: input.cutoff.toISOString(),
    gitUiFirstParentCommitCount: allEntries.length,
    uiPrCount: pullRequests.length,
    uiPrsByPathPrefix: Object.fromEntries(
      normalizedFilters.map((prefix) => [
        prefix,
        pullRequests.filter((pr) =>
          pr.files.some((file) => pathMatchesAnyPrefix(file, [prefix])),
        ).length,
      ]),
    ),
    directOrNoPrUiCommitCount: directEntries.length,
    cycleMinutes: {
      count: cycles.length,
      average: cycles.length === 0 ? null : roundOneDecimal(cycleTotal / cycles.length),
      median: roundOneDecimal(quantile(cycles, 0.5)),
      p75: roundOneDecimal(quantile(cycles, 0.75)),
      min: cycles.length === 0 ? null : roundOneDecimal(Math.min(...cycles)),
      max: cycles.length === 0 ? null : roundOneDecimal(Math.max(...cycles)),
    },
  }
}

export const buildNotEligibleReceipt = (input: {
  repo: string
  ref: string
  cutoff: Date
  pathFilters: ReadonlyArray<string>
  eraStart: Date
  requiredAgeDays: number
}): UiVelocityReceipt => {
  const actualAgeDays =
    (input.cutoff.getTime() - input.eraStart.getTime()) / (24 * 60 * 60 * 1000)
  const earliestEligibleCutoff = new Date(
    input.eraStart.getTime() + input.requiredAgeDays * 24 * 60 * 60 * 1000,
  )

  return {
    schema: "openagents.ui_velocity_receipt.v1",
    measurementState: "not_eligible",
    repo: input.repo,
    ref: input.ref,
    cutoff: input.cutoff.toISOString(),
    pathFilters: input.pathFilters.map(normalizePathPrefix),
    windows: [],
    eligibility: {
      eraStart: input.eraStart.toISOString(),
      requiredAgeDays: input.requiredAgeDays,
      actualAgeDays: roundOneDecimal(actualAgeDays) ?? 0,
      earliestEligibleCutoff: earliestEligibleCutoff.toISOString(),
      reason:
        "The React-era comparison must not run until the required trailing window is wholly after the React-era dependency anchor.",
    },
  }
}

export const buildMeasuredReceipt = (input: {
  repo: string
  ref: string
  cutoff: Date
  pathFilters: ReadonlyArray<string>
  windowDays: ReadonlyArray<number>
  entries: ReadonlyArray<GitUiEntry>
  pullRequests: ReadonlyArray<UiPullRequest>
}): UiVelocityReceipt => ({
  schema: "openagents.ui_velocity_receipt.v1",
  measurementState: "measured",
  repo: input.repo,
  ref: input.ref,
  cutoff: input.cutoff.toISOString(),
  pathFilters: input.pathFilters.map(normalizePathPrefix),
  windows: input.windowDays.map((days) =>
    summarizeUiVelocityWindow({
      cutoff: input.cutoff,
      start: new Date(input.cutoff.getTime() - days * 24 * 60 * 60 * 1000),
      entries: input.entries,
      pullRequests: input.pullRequests,
      pathFilters: input.pathFilters,
    }),
  ),
})

const requireValue = (
  args: ReadonlyArray<string>,
  index: number,
  flag: string,
): string => {
  const value = args[index + 1]
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}`)
  }
  return value
}

const parseCliOptions = (args: ReadonlyArray<string>): CliOptions => {
  let repo = "OpenAgentsInc/openagents"
  let ref = "HEAD"
  let cutoff: Date | undefined
  let pathFilters: ReadonlyArray<string> | undefined
  let windowDays: ReadonlyArray<number> = [30, 60]
  let eraStart: Date | undefined
  let requireEraDays: number | undefined

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    switch (arg) {
      case "--repo":
        repo = requireValue(args, index, arg)
        index += 1
        break
      case "--ref":
        ref = requireValue(args, index, arg)
        index += 1
        break
      case "--cutoff":
        cutoff = new Date(requireValue(args, index, arg))
        index += 1
        break
      case "--paths":
        pathFilters = requireValue(args, index, arg)
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean)
        index += 1
        break
      case "--window-days":
        windowDays = requireValue(args, index, arg)
          .split(",")
          .map((value) => Number(value.trim()))
          .filter((value) => Number.isFinite(value) && value > 0)
        index += 1
        break
      case "--era-start":
        eraStart = new Date(requireValue(args, index, arg))
        index += 1
        break
      case "--require-era-days":
        requireEraDays = Number(requireValue(args, index, arg))
        index += 1
        break
      default:
        throw new Error(`Unknown argument: ${arg}`)
    }
  }

  if (cutoff === undefined || Number.isNaN(cutoff.getTime())) {
    throw new Error("--cutoff must be an ISO timestamp")
  }
  if (pathFilters === undefined || pathFilters.length === 0) {
    throw new Error("--paths must provide at least one comma-separated path prefix")
  }
  if (windowDays.length === 0) {
    throw new Error("--window-days must provide at least one positive day count")
  }
  if (eraStart !== undefined && Number.isNaN(eraStart.getTime())) {
    throw new Error("--era-start must be an ISO timestamp")
  }
  if (requireEraDays !== undefined && (!Number.isFinite(requireEraDays) || requireEraDays <= 0)) {
    throw new Error("--require-era-days must be a positive number")
  }

  return {
    repo,
    ref,
    cutoff,
    pathFilters,
    windowDays,
    eraStart,
    requireEraDays,
  }
}

const readGitEntries = (options: CliOptions): ReadonlyArray<GitUiEntry> => {
  const maxDays = Math.max(...options.windowDays)
  const start = new Date(options.cutoff.getTime() - maxDays * 24 * 60 * 60 * 1000)
  const log = execFileSync("git", [
    "log",
    "--first-parent",
    `--since=${start.toISOString()}`,
    `--until=${options.cutoff.toISOString()}`,
    "--format=%H%x09%cI%x09%s",
    options.ref,
    "--",
    ...options.pathFilters.map(normalizePathPrefix),
  ], {
    encoding: "utf8",
    maxBuffer: 50 * 1024 * 1024,
  })

  return parseGitUiLog(log)
}

const readPullRequests = (
  repo: string,
  entries: ReadonlyArray<GitUiEntry>,
  pathFilters: ReadonlyArray<string>,
): ReadonlyArray<UiPullRequest> => {
  const prNumbers = [...new Set(entries.flatMap((entry) => entry.prs))]
    .sort((a, b) => a - b)
  const pullRequests: Array<UiPullRequest> = []

  for (const number of prNumbers) {
    try {
      const json = execFileSync("gh", [
        "pr",
        "view",
        String(number),
        "--repo",
        repo,
        "--json",
        "number,title,createdAt,mergedAt,url,reviews,files",
      ], {
        encoding: "utf8",
        maxBuffer: 20 * 1024 * 1024,
        stdio: ["ignore", "pipe", "pipe"],
      })
      const pr = JSON.parse(json) as {
        number: number
        title?: string
        createdAt: string
        mergedAt: string | null
        url?: string
        reviews?: ReadonlyArray<unknown>
        files?: ReadonlyArray<{ path: string }>
      }
      const files = (pr.files ?? []).map((file) => file.path)

      if (files.some((file) => pathMatchesAnyPrefix(file, pathFilters))) {
        pullRequests.push({
          ...pr,
          files: files.filter((file) => pathMatchesAnyPrefix(file, pathFilters)),
        })
      }
    } catch {
      // Historical squash subjects can contain issue refs or stale rewritten refs.
    }
  }

  return pullRequests
}

export const runCli = (args: ReadonlyArray<string>): UiVelocityReceipt => {
  const options = parseCliOptions(args)

  if (options.eraStart !== undefined && options.requireEraDays !== undefined) {
    const actualAgeMs = options.cutoff.getTime() - options.eraStart.getTime()
    const requiredAgeMs = options.requireEraDays * 24 * 60 * 60 * 1000
    if (actualAgeMs < requiredAgeMs) {
      return buildNotEligibleReceipt({
        repo: options.repo,
        ref: options.ref,
        cutoff: options.cutoff,
        pathFilters: options.pathFilters,
        eraStart: options.eraStart,
        requiredAgeDays: options.requireEraDays,
      })
    }
  }

  const entries = readGitEntries(options)
  const pullRequests = readPullRequests(options.repo, entries, options.pathFilters)

  return buildMeasuredReceipt({
    repo: options.repo,
    ref: options.ref,
    cutoff: options.cutoff,
    pathFilters: options.pathFilters,
    windowDays: options.windowDays,
    entries,
    pullRequests,
  })
}

if (import.meta.main) {
  try {
    console.log(JSON.stringify(runCli(process.argv.slice(2)), null, 2))
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  }
}
