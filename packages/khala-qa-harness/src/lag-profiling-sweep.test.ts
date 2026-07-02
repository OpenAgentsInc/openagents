import { mkdtemp, readFile, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  KHALA_CODE_LAG_PROFILING_SWEEP_SCHEMA,
  buildKhalaCodeLagProfilingFixtureSnapshots,
  buildKhalaCodeLagProfilingSweepReport,
  collectKhalaCodeLagProfilingSnapshot,
  createKhalaCodeLagProfilingIssueRequests,
  fileKhalaCodeLagProfilingOffenderIssues,
  loadKhalaCodeLagProfilingSnapshotInputs,
  writeKhalaCodeLagProfilingSweepReport,
} from "./index.js"
import { khalaCodeQaMetricBudgets } from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"

describe("Khala Code lag profiling sweep", () => {
  test("ranks p95 budget offenders while preserving every budget row", () => {
    const report = buildKhalaCodeLagProfilingSweepReport({
      generatedAt: "2026-07-02T00:00:00.000Z",
      runId: "fixture-lag-sweep",
      snapshots: buildKhalaCodeLagProfilingFixtureSnapshots(),
    })

    expect(report.schema).toBe(KHALA_CODE_LAG_PROFILING_SWEEP_SCHEMA)
    expect(report.budgets).toHaveLength(khalaCodeQaMetricBudgets.length)
    expect(report.sampledBudgetCount).toBe(khalaCodeQaMetricBudgets.length)
    expect(report.offenders.map((offender) => offender.budgetId)).toEqual([
      "budget.khala_code.composer.keystroke_echo.p95.v1",
      "budget.khala_code.transcript.scroll_dropped_frames.v1",
      "budget.khala_code.thread_switch.full.v1",
    ])
    expect(report.offenders.map((offender) => offender.rank)).toEqual([1, 2, 3])
    expect(report.offenders[0]).toMatchObject({
      metric: "composer.keystroke_echo_ms",
      p95: 30.4,
      ratio: 1.9,
      status: "offender",
      threshold: 16,
    })
  })

  test("writes a markdown report, JSON report, and one issue body per offender", async () => {
    const outDir = await mkdtemp(join(tmpdir(), "khala-lag-sweep-"))
    try {
      const report = buildKhalaCodeLagProfilingSweepReport({
        generatedAt: "2026-07-02T00:00:00.000Z",
        runId: "fixture-lag-sweep",
        snapshots: buildKhalaCodeLagProfilingFixtureSnapshots(),
      })
      const written = await writeKhalaCodeLagProfilingSweepReport({ outDir, report })

      const markdown = await readFile(written.markdownPath, "utf8")
      const json = JSON.parse(await readFile(written.jsonPath, "utf8")) as typeof report
      const issueBody = await readFile(
        written.issueBodyPaths["budget.khala_code.composer.keystroke_echo.p95.v1"] ?? "",
        "utf8",
      )

      expect(markdown).toContain("## Ranked Offenders")
      expect(markdown).toContain("budget.khala_code.composer.keystroke_echo.p95.v1")
      expect(json.schema).toBe(KHALA_CODE_LAG_PROFILING_SWEEP_SCHEMA)
      expect(Object.keys(written.issueBodyPaths)).toHaveLength(report.offenderCount)
      expect(issueBody).toContain("Parent: #8019")
      expect(issueBody).toContain("composer.keystroke_echo_ms")
      expect(issueBody).toContain("Sample Evidence")
    } finally {
      await rm(outDir, { force: true, recursive: true })
    }
  })

  test("creates one child issue request per offender with sample evidence", async () => {
    const report = buildKhalaCodeLagProfilingSweepReport({
      generatedAt: "2026-07-02T00:00:00.000Z",
      runId: "fixture-lag-sweep",
      snapshots: buildKhalaCodeLagProfilingFixtureSnapshots(),
    })
    const requests = createKhalaCodeLagProfilingIssueRequests({ parentIssueNumber: 8019, report })
    const filed: string[] = []
    const results = await fileKhalaCodeLagProfilingOffenderIssues({
      issueFiler: async (request) => {
        filed.push(request.budgetId)
        expect(request.labels).toEqual(["qa", "roadmap"])
        expect(request.body).toContain("## Sample Evidence")
        return { budgetId: request.budgetId, url: `https://example.test/${request.budgetId}` }
      },
      parentIssueNumber: 8019,
      report,
    })

    expect(requests).toHaveLength(report.offenderCount)
    expect(filed).toEqual(report.offenders.map((offender) => offender.budgetId))
    expect(results.map((result) => result.budgetId)).toEqual(filed)
  })

  test("collects a lag profiling snapshot through the driver metrics hook", async () => {
    const [fixtureInput] = buildKhalaCodeLagProfilingFixtureSnapshots()
    expect(fixtureInput).toBeDefined()
    const collected = await Effect.runPromise(
      collectKhalaCodeLagProfilingSnapshot({
        driver: {
          metrics: () => Effect.succeed(fixtureInput!.snapshot),
        },
        label: "mode-p-long-transcript",
        mode: "mode_p_preview_bridge",
        workload: ["long_transcript", "thread_switch"],
      }),
    )

    expect(collected.label).toBe("mode-p-long-transcript")
    expect(collected.mode).toBe("mode_p_preview_bridge")
    expect(collected.snapshot.samples.length).toBeGreaterThan(0)
  })

  test("loads wrapped and raw qaMetrics snapshot files", async () => {
    const outDir = await mkdtemp(join(tmpdir(), "khala-lag-sweep-load-"))
    try {
      const [fixtureInput] = buildKhalaCodeLagProfilingFixtureSnapshots()
      expect(fixtureInput).toBeDefined()
      const wrappedPath = join(outDir, "wrapped.json")
      const rawPath = join(outDir, "raw.json")
      await Bun.write(wrappedPath, `${JSON.stringify(fixtureInput, null, 2)}\n`)
      await Bun.write(rawPath, `${JSON.stringify(fixtureInput!.snapshot, null, 2)}\n`)

      const wrapped = await loadKhalaCodeLagProfilingSnapshotInputs(wrappedPath)
      const raw = await loadKhalaCodeLagProfilingSnapshotInputs(rawPath)

      expect(wrapped[0]?.mode).toBe("fixture")
      expect(raw[0]?.mode).toBe("mode_p_preview_bridge")
      expect(raw[0]?.label).toBe("raw.json")
    } finally {
      await rm(outDir, { force: true, recursive: true })
    }
  })
})
