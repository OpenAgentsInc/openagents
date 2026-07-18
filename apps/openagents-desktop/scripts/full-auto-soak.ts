/**
 * FA-SOAK-01 (#8992): the one-command Full Auto soak entrypoint.
 *
 *   pnpm --dir apps/openagents-desktop run soak:full-auto -- --smoke
 *     Short REAL-TIME fixture soak: runs every fault-matrix scenario that
 *     does not require compressed FA-H5 backoff time (clean completion,
 *     owner stop, cap exhaustion, account-exhaustion rotation, app restart,
 *     cache pressure, workspace drift) against stub lanes under the real
 *     wall clock, then prints the machine-readable SM-10 summary. Exits
 *     nonzero if any run terminates untyped or off its expected class.
 *
 *   ... -- --compressed
 *     The full 10-scenario compressed-clock matrix (exactly what
 *     tests/full-auto-soak.e2e.test.ts runs in CI), for local reproduction.
 *
 *   ... -- --collect [--user-data <path>]
 *     Read-only SM-10 measurement over the LIVE run-report store
 *     (full-auto/run-reports.json under the Desktop userData directory):
 *     decodes each stored report, classifies its terminal state + stop
 *     attribution, and prints the summary. This is the post-dogfood
 *     evidence-collection step named by --afk-prep. Strictly read-only: it
 *     never syncs, persists, or quarantines the owner's store.
 *
 *   ... -- --afk-prep
 *     Prints the exact owner recipe for a real 24-48h AFK dogfood window
 *     (which build to run, how to enable Full Auto, where run reports land,
 *     how to collect SM-10/SM-11 evidence afterward). The harness PREPARES
 *     the window; executing it stays owner ceremony.
 *
 * Options: --out <path> writes the JSON summary to a file as well.
 * This script performs no Keychain access and no live provider calls.
 */
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Exit, Schema } from "effect"

import { resolveUserDataDir } from "./full-auto-control-client.ts"
import { FullAutoRunReportSchema } from "../src/full-auto-run-report.ts"
import { isFullAutoRunTerminal } from "../src/full-auto-run-registry.ts"
import {
  buildFullAutoSoakSm10Summary,
  classifySm10Termination,
  FULL_AUTO_SOAK_SCENARIOS,
  makeCompressedSoakClock,
  makeRealtimeSoakClock,
  runFullAutoSoakMatrix,
  type FullAutoSoakSm10Summary,
  type Sm10PopulationEntry,
} from "../tests/full-auto-soak-harness.ts"

const USAGE = `usage: full-auto-soak (--smoke | --compressed | --collect [--user-data <path>] | --afk-prep) [--out <path>]

  --smoke       short real-time fixture soak (no compressed-clock scenarios)
  --compressed  the full CI fault matrix under the compressed clock
  --collect     read-only SM-10 over the live run-report store
  --afk-prep    print the owner recipe for a real 24-48h AFK dogfood window
  --out <path>  additionally write the JSON summary to <path>`

const AFK_PREP_RECIPE = `FA-SOAK-01 owner recipe: 24-48h Full Auto AFK dogfood window (SM-10/SM-11 evidence)
=====================================================================================

This prepares a REAL long window on real lanes and a real workspace. The
window itself is owner ceremony: nothing here runs providers for you.

1) Which build to run
   - Packaged (preferred for a long window):
       pnpm --dir apps/openagents-desktop run package:mac
     then open apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app.
   - If you want the control CLI available during the window, launch with the
     control API enabled:
       OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 open apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app
     (dev alternative: OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 pnpm --dir apps/openagents-desktop run dev)

2) Enable Full Auto (UI-first)
   - In the app: open the workspace you want worked on, start a new thread,
     and switch ON the "Full Auto" toggle in the composer. That binds the
     granted workspace and execution profile durably (FA-H2/FA-H6).
   - Or via the control CLI (the app must be running with the control API on):
       pnpm --dir apps/openagents-desktop run full-auto run-start \\
         --workspace <absolute path> --title "AFK dogfood" \\
         --objective "<what to work on>" --done "<done condition>" [--turn-cap <n>]

3) During the window (24-48h)
   - Leave the app running and the machine awake (System Settings > Displays/
     Battery: prevent sleep while the window runs).
   - Do NOT sign accounts out; do not switch the granted workspace (FA-H2
     blocks typed on drift -- that is correct behavior, not a bug).
   - Optional check-ins:  pnpm --dir apps/openagents-desktop run full-auto runs
                          pnpm --dir apps/openagents-desktop run full-auto run-status <runRef>

4) Where the evidence lands
   - Private run reports (durable, bounded):
       <userData>/full-auto/run-reports.json
     (default macOS userData: ~/Library/Application Support/OpenAgents)
   - Via the control API while the app runs:
       pnpm --dir apps/openagents-desktop run full-auto report <runRef>    (private report)
       pnpm --dir apps/openagents-desktop run full-auto receipt <runRef>   (public-safe receipt)

5) Collect SM-10/SM-11 evidence afterward
   - SM-10 (typed-termination rate over the window's run population):
       node --import tsx apps/openagents-desktop/scripts/full-auto-soak.ts --collect
     (add --user-data <path> if the app ran against a non-default userData
     directory; add --out <path> to save the JSON summary as an artifact)
   - SM-11 (owner-AFK receipt): save the "receipt <runRef>" JSON for each
     terminal run in the window alongside the SM-10 summary. The receipt is
     public-safe by construction (digests/counts/enums only).

Nothing in this recipe touches the Keychain, and this script never calls a
live provider itself.`

const emitSummary = (summary: FullAutoSoakSm10Summary, outPath: string | undefined): void => {
  console.log(`[full-auto-soak] sm10-summary ${JSON.stringify(summary)}`)
  if (outPath !== undefined) {
    writeFileSync(path.resolve(outPath), `${JSON.stringify(summary, null, 2)}\n`, "utf8")
    console.log(`[full-auto-soak] summary written to ${path.resolve(outPath)}`)
  }
}

const runFixtureSoak = async (
  mode: "smoke" | "compressed",
  outPath: string | undefined,
): Promise<number> => {
  const scenarios = mode === "smoke"
    ? FULL_AUTO_SOAK_SCENARIOS.filter(scenario => !scenario.requiresCompressedClock)
    : FULL_AUTO_SOAK_SCENARIOS
  console.log(
    `[full-auto-soak] running ${scenarios.length} fixture scenario(s) in ${mode} mode` +
      (mode === "smoke"
        ? " (compressed-clock-only FA-H5 backoff scenarios are excluded by design; run --compressed for the full matrix)"
        : ""),
  )
  const roots: Array<string> = []
  try {
    const { results, summary } = await runFullAutoSoakMatrix({
      scenarios,
      makeRoot: scenarioId => {
        const root = mkdtempSync(path.join(tmpdir(), `oa-fa-soak-${scenarioId}-`))
        roots.push(root)
        return root
      },
      makeClock: () => (mode === "smoke" ? makeRealtimeSoakClock() : makeCompressedSoakClock()),
    })
    for (const result of results) {
      console.log(
        `[full-auto-soak] ${result.scenario}: state=${result.state} stop=${result.stopAttribution ?? "(none)"} ` +
          `class=${result.classification} continuations=${result.continuations} ` +
          `failures=${result.dispatchFailures} rotations=${result.rotations} restarts=${result.restarts}`,
      )
    }
    emitSummary(summary, outPath)
    const offExpected = results.filter(result => result.classification !== result.expected)
    if (offExpected.length > 0) {
      console.error(
        `[full-auto-soak] FAIL: ${offExpected.length} run(s) terminated off their expected class: ` +
          offExpected.map(result => `${result.scenario} -> ${result.classification}`).join(", "),
      )
      return 1
    }
    console.log("[full-auto-soak] OK: every fixture run terminated typed in its expected class")
    return 0
  } finally {
    for (const root of roots) rmSync(root, { recursive: true, force: true })
  }
}

/** Per-entry decode so one malformed row never blocks the measurement, and
 * strictly read-only (never the store's own open/quarantine path). */
const decodeReportEntry = Schema.decodeUnknownExit(FullAutoRunReportSchema)

const collectLiveSm10 = (userData: string | undefined, outPath: string | undefined): number => {
  const userDataDir = resolveUserDataDir(userData)
  const reportsPath = path.join(userDataDir, "full-auto", "run-reports.json")
  let raw: string
  try {
    raw = readFileSync(reportsPath, "utf8")
  } catch {
    console.error(
      `[full-auto-soak] no run-report store found at ${reportsPath} -- has Desktop run any Full Auto runs against this userData directory?`,
    )
    return 1
  }
  const parsed: unknown = JSON.parse(raw)
  const rows = typeof parsed === "object" && parsed !== null && Array.isArray((parsed as { reports?: unknown }).reports)
    ? ((parsed as { reports: ReadonlyArray<unknown> }).reports)
    : []
  const entries: Array<Sm10PopulationEntry> = []
  let skippedInvalid = 0
  let skippedActive = 0
  for (const row of rows) {
    const decoded = decodeReportEntry(row)
    if (!Exit.isSuccess(decoded)) {
      skippedInvalid += 1
      continue
    }
    const report = decoded.value
    if (!isFullAutoRunTerminal(report.state)) {
      skippedActive += 1
      continue
    }
    const stopAttribution = report.stopAttribution ?? null
    entries.push({
      scenario: "live",
      runRef: report.runRef,
      state: report.state,
      stopAttribution,
      classification: classifySm10Termination({ state: report.state, stopAttribution }),
    })
  }
  const summary = buildFullAutoSoakSm10Summary(entries, {
    clockMode: "live",
    generatedAt: new Date().toISOString(),
  })
  console.log(
    `[full-auto-soak] live report store: ${rows.length} report(s); ${entries.length} terminal measured, ` +
      `${skippedActive} still active (excluded), ${skippedInvalid} undecodable (excluded, reported honestly)`,
  )
  emitSummary(summary, outPath)
  return 0
}

const main = async (): Promise<void> => {
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
  const outPath = takeOption("--out")
  const userData = takeOption("--user-data")
  const smoke = takeFlag("--smoke")
  const compressed = takeFlag("--compressed")
  const collect = takeFlag("--collect")
  const afkPrep = takeFlag("--afk-prep")

  const modes = [smoke, compressed, collect, afkPrep].filter(Boolean).length
  if (modes !== 1 || argv.length > 0) {
    console.error(USAGE)
    process.exitCode = 2
    return
  }
  if (afkPrep) {
    console.log(AFK_PREP_RECIPE)
    return
  }
  if (collect) {
    process.exitCode = collectLiveSm10(userData, outPath)
    return
  }
  process.exitCode = await runFixtureSoak(smoke ? "smoke" : "compressed", outPath)
}

main().catch(error => {
  console.error("[full-auto-soak] failed:", error)
  process.exitCode = 1
})
