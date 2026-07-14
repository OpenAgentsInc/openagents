/**
 * OpenAgents Desktop startup-timing benchmark (measure-constantly discipline).
 *
 * Runs the real Electron app N times in the deterministic startup-marks mode
 * (fixture wiring — no network, no live ~/.codex scan), each with a fresh
 * temp userData root, and collects the milestone chain the main process writes:
 *
 *   process start -> app.whenReady -> window created -> window ready-to-show
 *   -> renderer boot -> renderer first paint -> shell mounted (interactive)
 *   -> capability ready (runtime-gateway bootstrap)
 *
 * All marks are ms from process start. A warmup run is discarded (cold V8/OS
 * cache), then it reports median + p95 per mark over the measured runs and
 * writes a privacy-safe JSON receipt (timings only — no paths, no user data).
 *
 * Usage:
 *   bun scripts/startup-bench.ts [--runs N] [--warmup W] [--out FILE] [--label L] [--no-build]
 *
 * Default receipt: benchmarks/startup/latest.json
 *
 * Real-wiring variant (2026-07-13 startup incident): launch the app manually
 * with OPENAGENTS_DESKTOP_STARTUP_TRACE=<file> (and optionally
 * OPENAGENTS_DESKTOP_USER_DATA=<profile>, OPENAGENTS_DESKTOP_STARTUP_TRACE_SHOTS=<dir>)
 * to record the same milestone chain — plus historyHydrated — WITHOUT fixture
 * substitution, against a real profile and real ~/.codex. Timings only; the
 * measured profile is never deleted.
 */
import { mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { spawn } from "node:child_process"
import { buildDesktop } from "./build.ts"

const appRoot = path.resolve(import.meta.dir, "..")

type Args = Readonly<{ runs: number; warmup: number; out: string; label: string | null; build: boolean }>

const parseArgs = (argv: readonly string[]): Args => {
  let runs = 7
  let warmup = 1
  let out = path.join(appRoot, "benchmarks", "startup", "latest.json")
  let label: string | null = null
  let build = true
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === "--runs") runs = Math.max(1, Number(argv[++i]))
    else if (arg === "--warmup") warmup = Math.max(0, Number(argv[++i]))
    else if (arg === "--out") out = path.resolve(String(argv[++i]))
    else if (arg === "--label") label = String(argv[++i])
    else if (arg === "--no-build") build = false
  }
  return { runs, warmup, out, label, build }
}

const MARK_ORDER = [
  "mainModuleEvaluated",
  "appWhenReady",
  "windowCreated",
  "syncHostOpened",
  "sessionVaultRecovered",
  "windowReadyToShow",
  "rendererBootStart",
  "firstPaint",
  "shellMounted",
  "historyHydrated",
  "capabilityReady",
] as const
type MarkName = (typeof MARK_ORDER)[number]

/**
 * Startup budgets (2026-07-13 incident contract
 * `openagents_desktop.startup.window_first_no_blank_frame.v1`): fixture-mode
 * medians must stay inside these bounds or the bench exits non-zero. Bounds
 * carry generous machine headroom over the measured ~500/700 ms medians —
 * they exist to catch ordering regressions (hydration or network back on the
 * pre-paint path), not micro-noise.
 */
const BUDGETS_MS: Partial<Record<MarkName, number>> = {
  windowReadyToShow: 1500,
  shellMounted: 2500,
}
type MarkRecord = Partial<Record<MarkName, number | null>>

const electronBin = path.join(appRoot, "node_modules", ".bin", "electron")

const runOnce = async (): Promise<MarkRecord> => {
  const userData = mkdtempSync(path.join(tmpdir(), "oa-desktop-bench-userdata-"))
  const marksFile = path.join(mkdtempSync(path.join(tmpdir(), "oa-desktop-bench-marks-")), "marks.json")
  try {
    await new Promise<void>((resolve, reject) => {
      const child = spawn(electronBin, ["."], {
        cwd: appRoot,
        env: {
          ...process.env,
          OPENAGENTS_DESKTOP_STARTUP_MARKS: marksFile,
          OPENAGENTS_DESKTOP_USER_DATA: userData,
        },
        stdio: ["ignore", "ignore", "inherit"],
      })
      const timer = setTimeout(() => {
        child.kill("SIGKILL")
        reject(new Error("startup-bench run timed out after 60s"))
      }, 60_000)
      child.once("error", (error) => { clearTimeout(timer); reject(error) })
      child.once("exit", (code) => {
        clearTimeout(timer)
        code === 0 ? resolve() : reject(new Error(`electron exited ${code}`))
      })
    })
    const parsed = JSON.parse(readFileSync(marksFile, "utf8")) as { marks: MarkRecord }
    return parsed.marks
  } finally {
    rmSync(userData, { recursive: true, force: true })
    rmSync(path.dirname(marksFile), { recursive: true, force: true })
  }
}

const median = (values: readonly number[]): number => {
  const sorted = [...values].sort((a, b) => a - b)
  const mid = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 0 ? (sorted[mid - 1]! + sorted[mid]!) / 2 : sorted[mid]!
}

const percentile = (values: readonly number[], p: number): number => {
  const sorted = [...values].sort((a, b) => a - b)
  if (sorted.length === 1) return sorted[0]!
  const rank = p * (sorted.length - 1)
  const low = Math.floor(rank)
  const high = Math.ceil(rank)
  return sorted[low]! + (sorted[high]! - sorted[low]!) * (rank - low)
}

const round2 = (value: number): number => Math.round(value * 100) / 100

const main = async (): Promise<void> => {
  const args = parseArgs(process.argv.slice(2))
  if (args.build) {
    console.log("[startup-bench] building dist/ ...")
    await buildDesktop()
  }
  console.log(`[startup-bench] warmup=${args.warmup} measured=${args.runs}`)

  for (let i = 0; i < args.warmup; i++) {
    console.log(`[startup-bench] warmup run ${i + 1}/${args.warmup} (discarded)`)
    await runOnce()
  }

  const samples: MarkRecord[] = []
  for (let i = 0; i < args.runs; i++) {
    const marks = await runOnce()
    samples.push(marks)
    console.log(`[startup-bench] run ${i + 1}/${args.runs} shellMounted=${marks.shellMounted}ms firstPaint=${marks.firstPaint}ms`)
  }

  const aggregate: Record<string, { median: number; p95: number; min: number; max: number; n: number } | null> = {}
  for (const mark of MARK_ORDER) {
    const values = samples
      .map((sample) => sample[mark])
      .filter((value): value is number => typeof value === "number")
    aggregate[mark] = values.length === 0
      ? null
      : {
          median: round2(median(values)),
          p95: round2(percentile(values, 0.95)),
          min: round2(Math.min(...values)),
          max: round2(Math.max(...values)),
          n: values.length,
        }
  }

  const receipt = {
    schema: "openagents-desktop-startup-bench/v1",
    capturedAtIso: new Date().toISOString(),
    label: args.label,
    unit: "ms-from-process-start",
    runs: args.runs,
    warmupDiscarded: args.warmup,
    mode: "startup-marks (deterministic fixtures)",
    platform: `${process.platform}-${process.arch}`,
    marks: aggregate,
    rawSamples: samples,
  }

  mkdirSync(path.dirname(args.out), { recursive: true })
  writeFileSync(args.out, JSON.stringify(receipt, null, 2))

  console.log("")
  console.log(`[startup-bench] === milestone chain (median / p95, ms from process start) ===`)
  for (const mark of MARK_ORDER) {
    const agg = aggregate[mark]
    if (agg === null) { console.log(`  ${mark.padEnd(20)} —`); continue }
    console.log(`  ${mark.padEnd(20)} ${String(agg.median).padStart(8)}  / ${String(agg.p95).padStart(8)}  (n=${agg.n})`)
  }
  console.log("")
  console.log(`[startup-bench] receipt written to ${args.out}`)

  // Budget enforcement (2026-07-13 startup incident): an ordering regression
  // that puts hydration or network back on the pre-paint path blows these by
  // seconds; honest machine noise does not.
  const breaches: Array<string> = []
  for (const [mark, budget] of Object.entries(BUDGETS_MS) as ReadonlyArray<[MarkName, number]>) {
    const agg = aggregate[mark]
    if (agg === null || agg === undefined) {
      breaches.push(`${mark}: no samples (budget ${budget}ms)`)
      continue
    }
    if (agg.median > budget) breaches.push(`${mark}: median ${agg.median}ms > budget ${budget}ms`)
  }
  if (breaches.length > 0) {
    console.error(`[startup-bench] BUDGET BREACH — ${breaches.join("; ")}`)
    process.exit(1)
  }
  console.log(`[startup-bench] budgets OK (windowReadyToShow<${BUDGETS_MS.windowReadyToShow}ms, shellMounted<${BUDGETS_MS.shellMounted}ms medians)`)
}

if (import.meta.main) {
  main().catch((error) => {
    console.error("[startup-bench] failed:", error)
    process.exit(1)
  })
}
