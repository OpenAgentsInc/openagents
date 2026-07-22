/**
 * META-3 (#9182): the repeatable grading-baseline command. Reads the durable
 * Full Auto run reports and run-registry records from a userData root
 * (READ-ONLY -- it never syncs, quarantines, or mutates the stores), scores
 * every run against the D1-D7 autonomy rubric via `src/run-grading.ts`, and
 * writes a dated baseline artifact (JSON + Markdown) so later meta-agent
 * routing/decomposition changes are measurable against it.
 *
 * Usage (package script `pnpm --dir apps/openagents-desktop run grade-runs`):
 *   node --import tsx scripts/grade-runs.ts [options]
 *
 * Options:
 *   --user-data <path>   userData root holding full-auto/run-reports.json and
 *                        full-auto/runs.json (default: the app's own userData
 *                        dir, or OPENAGENTS_DESKTOP_USER_DATA)
 *   --out <dir>          artifact output directory (default:
 *                        <userData>/full-auto/grading -- local/private, never
 *                        a tracked repo path; real-run outputs stay local)
 *   --limit <n>          grade at most the n most recently updated runs
 *   --now <iso>          pinned timestamp for deterministic/fixture output
 *   --json               also print the baseline JSON to stdout
 *   --markdown           also print the Markdown report to stdout
 *
 * Measurement only: the artifact carries analysis-authority scores and typed
 * `not_measured` honesty. It cannot admit a release or a public claim, and
 * this command holds no dispatch or optimizer behavior.
 */
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"

import { resolveUserDataDir } from "./full-auto-control-client.ts"
import {
  buildFullAutoGradingBaseline,
  readFullAutoRunReports,
  readFullAutoRuns,
  renderFullAutoGradingBaselineMarkdown,
} from "../src/run-grading.ts"

const USAGE = `usage: grade-runs [--user-data <path>] [--out <dir>] [--limit <n>] [--now <iso>] [--json] [--markdown]`

const main = (): void => {
  const argv = [...process.argv.slice(2)]
  const takeOption = (name: string): string | undefined => {
    const index = argv.indexOf(name)
    if (index === -1) return undefined
    const value = argv[index + 1]
    argv.splice(index, 2)
    return value
  }
  const takeFlag = (name: string): boolean => {
    const index = argv.indexOf(name)
    if (index === -1) return false
    argv.splice(index, 1)
    return true
  }

  const userData = takeOption("--user-data")
  const outOption = takeOption("--out")
  const limitOption = takeOption("--limit")
  const nowOption = takeOption("--now")
  const printJson = takeFlag("--json")
  const printMarkdown = takeFlag("--markdown")
  if (argv.length > 0) {
    console.error(`unrecognized arguments: ${argv.join(" ")}\n${USAGE}`)
    process.exitCode = 1
    return
  }

  const now = (): Date => {
    if (nowOption === undefined) return new Date()
    const pinned = new Date(nowOption)
    if (Number.isNaN(pinned.getTime())) throw new Error(`--now is not a valid ISO timestamp: ${nowOption}`)
    return pinned
  }

  const userDataDir = resolveUserDataDir(userData)
  const reportsLoad = readFullAutoRunReports(userDataDir)
  const runsLoad = readFullAutoRuns(userDataDir)
  if (reportsLoad.issue === "missing") {
    console.error(`note: no run-report store at ${reportsLoad.filePath} -- grading zero runs`)
  } else if (reportsLoad.issue === "undecodable") {
    console.error(`note: run-report store at ${reportsLoad.filePath} failed decode; it was left untouched and zero runs are graded`)
  }
  if (runsLoad.issue === "undecodable") {
    console.error(`note: run-registry store at ${runsLoad.filePath} failed decode; grades degrade to run_record_unavailable`)
  }

  const runByRef = new Map(runsLoad.values.map((run) => [run.runRef, run]))
  const limit = limitOption === undefined ? undefined : Number.parseInt(limitOption, 10)
  if (limit !== undefined && (!Number.isInteger(limit) || limit <= 0)) {
    console.error(`--limit must be a positive integer\n${USAGE}`)
    process.exitCode = 1
    return
  }
  const reports = [...reportsLoad.values]
    .toSorted((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .slice(0, limit)

  const baseline = buildFullAutoGradingBaseline({
    entries: reports.map((report) => ({ report, run: runByRef.get(report.runRef) ?? null })),
    now,
  })

  const outDir = path.resolve(outOption ?? path.join(userDataDir, "full-auto", "grading"))
  mkdirSync(outDir, { recursive: true })
  const stamp = now().toISOString().replaceAll(":", "").replace(/\.\d{3}Z$/, "Z")
  const jsonPath = path.join(outDir, `full-auto-grading-baseline-${stamp}.json`)
  const markdownPath = path.join(outDir, `full-auto-grading-baseline-${stamp}.md`)
  writeFileSync(jsonPath, `${JSON.stringify(baseline, null, 2)}\n`, "utf8")
  const markdown = renderFullAutoGradingBaselineMarkdown(baseline)
  writeFileSync(markdownPath, markdown, "utf8")

  if (printJson) console.log(JSON.stringify(baseline, null, 2))
  if (printMarkdown) console.log(markdown)
  console.error(`graded ${baseline.runCount} run(s); baseline written to ${jsonPath} and ${markdownPath}`)
}

main()
