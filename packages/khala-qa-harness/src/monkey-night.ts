import { mkdir, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { Effect } from "effect"

import {
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  mergeKhalaCodeQaMonkeyCoverage,
  runKhalaCodeQaSeededMonkey,
  type KhalaCodeQaMonkeyRunReport,
} from "./index.js"

type MonkeyNightSummary = {
  readonly schema: "khala_code_qa_seeded_monkey_night.v1"
  readonly generatedAt: string
  readonly mode: "fleet_cockpit_night"
  readonly runs: number
  readonly passed: number
  readonly failed: number
  readonly falseFailureRate: number
  readonly seeds: readonly string[]
  readonly failedSeeds: readonly string[]
  readonly coverageLedgerPath: string
}

const optionValue = (name: string): string | undefined => {
  const index = process.argv.indexOf(name)
  return index === -1 ? undefined : process.argv[index + 1]
}

const intOption = (name: string, fallback: number): number => {
  const value = optionValue(name)
  if (value === undefined) return fallback
  const parsed = Number.parseInt(value, 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const stringOption = (name: string, fallback: string): string =>
  optionValue(name) ?? fallback

const seedFor = (prefix: string, index: number): string =>
  `${prefix}-${String(index + 1).padStart(3, "0")}`

export const runKhalaCodeQaMonkeyNight = async (options: {
  readonly artifactDir?: string
  readonly runs?: number
  readonly seedPrefix?: string
  readonly steps?: number
} = {}): Promise<MonkeyNightSummary> => {
  const artifactDir = options.artifactDir ?? "artifacts"
  const runs = options.runs ?? 100
  const seedPrefix = options.seedPrefix ?? "t6.8-night"
  const steps = options.steps ?? 64
  const reports: KhalaCodeQaMonkeyRunReport[] = []

  for (let index = 0; index < runs; index += 1) {
    const seed = seedFor(seedPrefix, index)
    const report = await Effect.runPromise(
      runKhalaCodeQaSeededMonkey({
        driver: makeKhalaCodeRpcQaDriver({
          baseUrl: "http://fixture.local",
          fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
          now: () => "2026-07-01T00:00:00.000Z",
        }),
        options: {
          mode: "fleet_cockpit_night",
          seed,
          steps,
        },
      }),
    )
    reports.push(report)
  }

  const coverageLedger = mergeKhalaCodeQaMonkeyCoverage(reports)
  const generatedAt = new Date().toISOString()
  const coverageLedgerPath = join(artifactDir, "monkey-night-coverage-ledger.json")
  const summaryPath = join(artifactDir, "monkey-night-report.json")
  const failedSeeds = reports
    .filter((report) => report.status === "fail")
    .map((report) => report.seed)
  const summary: MonkeyNightSummary = {
    coverageLedgerPath,
    failed: failedSeeds.length,
    failedSeeds,
    falseFailureRate: runs === 0 ? 0 : failedSeeds.length / runs,
    generatedAt,
    mode: "fleet_cockpit_night",
    passed: reports.length - failedSeeds.length,
    runs,
    schema: "khala_code_qa_seeded_monkey_night.v1",
    seeds: reports.map((report) => report.seed),
  }

  await mkdir(dirname(coverageLedgerPath), { recursive: true })
  await writeFile(coverageLedgerPath, `${JSON.stringify(coverageLedger, null, 2)}\n`)
  await writeFile(summaryPath, `${JSON.stringify({ ...summary, reports }, null, 2)}\n`)
  return summary
}

if (import.meta.main) {
  const artifactDir = stringOption("--artifact-dir", "artifacts")
  const runs = intOption("--runs", 100)
  const seedPrefix = stringOption("--seed-prefix", "t6.8-night")
  const steps = intOption("--steps", 64)
  const summary = await runKhalaCodeQaMonkeyNight({ artifactDir, runs, seedPrefix, steps })
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`)
}
