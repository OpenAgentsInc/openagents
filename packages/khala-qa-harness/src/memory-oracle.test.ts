import { mkdtemp, readFile, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_QA_MEMORY_ORACLE_SCHEMA,
  buildKhalaCodeQaShutdownOracle,
  evaluateKhalaCodeQaMemoryOracle,
  summarizeKhalaCodeQaShutdownOracles,
  type KhalaCodeQaMemoryPhase,
  type KhalaCodeQaMemorySample,
} from "./index.js"
import { runKhalaCodeQaMonkeyNight } from "./monkey-night.js"

const memorySample = (input: {
  readonly heapUsedBytes: number
  readonly phase: KhalaCodeQaMemoryPhase
  readonly rssBytes: number
  readonly runIndex?: number | undefined
  readonly seed?: string | undefined
}): KhalaCodeQaMemorySample => ({
  heapTotalBytes: input.heapUsedBytes + 1024,
  heapUsedBytes: input.heapUsedBytes,
  observedAt: `2026-07-02T00:00:${String(input.runIndex ?? 0).padStart(2, "0")}.000Z`,
  phase: input.phase,
  rssBytes: input.rssBytes,
  ...(input.runIndex === undefined ? {} : { runIndex: input.runIndex }),
  ...(input.seed === undefined ? {} : { seed: input.seed }),
})

describe("Khala Code QA memory and shutdown oracles", () => {
  test("evaluates RSS, JS heap, and monotonic growth budgets", () => {
    const report = evaluateKhalaCodeQaMemoryOracle({
      generatedAt: "2026-07-02T00:00:00.000Z",
      samples: [
        memorySample({ heapUsedBytes: 100, phase: "after_monkey_run", rssBytes: 1_000_000_000, runIndex: 0 }),
        memorySample({ heapUsedBytes: 120, phase: "after_monkey_run", rssBytes: 1_100_000_000, runIndex: 1 }),
        memorySample({ heapUsedBytes: 140, phase: "after_monkey_run", rssBytes: 1_200_000_000, runIndex: 2 }),
        memorySample({ heapUsedBytes: 140, phase: "after_monkey_night", rssBytes: 1_600_000_000 }),
      ],
    })

    expect(report.schema).toBe(KHALA_CODE_QA_MEMORY_ORACLE_SCHEMA)
    expect(report.status).toBe("fail")
    expect(report.budgetEvaluations.find((budget) =>
      budget.budgetId === "memory.rss_after_monkey_night.v1"
    )).toMatchObject({
      actualBytes: 1_600_000_000,
      status: "fail",
      thresholdBytes: 1_500_000_000,
    })
    expect(report.trendEvaluation).toMatchObject({
      budgetId: "memory.rss_monotonic_growth_after_monkey_night.v1",
      status: "fail",
    })
  })

  test("summarizes shutdown orphan process failures", () => {
    const pass = buildKhalaCodeQaShutdownOracle({ observedAt: "2026-07-02T00:00:00.000Z" })
    const fail = buildKhalaCodeQaShutdownOracle({
      observedAt: "2026-07-02T00:00:01.000Z",
      orphanProcesses: [{ command: "fixture-worker", pid: 1234, reason: "still alive after shutdown" }],
    })
    const summary = summarizeKhalaCodeQaShutdownOracles([pass, fail])

    expect(pass.status).toBe("pass")
    expect(fail.status).toBe("fail")
    expect(summary).toMatchObject({
      actualOrphans: 1,
      checkedShutdowns: 2,
      failedShutdowns: 1,
      status: "fail",
    })
  })

  test("monkey night records memory samples and fails on a memory ceiling breach", async () => {
    const root = await mkdtemp(join(tmpdir(), "khala-monkey-memory-"))
    const sampler = (input: {
      readonly phase: KhalaCodeQaMemoryPhase
      readonly runIndex?: number | undefined
      readonly seed?: string | undefined
    }): KhalaCodeQaMemorySample => {
      const after = input.phase !== "before_monkey_run"
      const index = input.runIndex ?? 2
      return memorySample({
        heapUsedBytes: after ? 100_000_000 : 90_000_000,
        phase: input.phase,
        rssBytes: after ? 1_600_000_000 + index : 900_000_000,
        runIndex: input.runIndex,
        seed: input.seed,
      })
    }

    try {
      const summary = await runKhalaCodeQaMonkeyNight({
        artifactDir: root,
        memorySampler: sampler,
        runs: 2,
        seedPrefix: "memory-breach",
        steps: 8,
      })
      const memoryReport = JSON.parse(await readFile(join(root, "monkey-night-memory-oracle.json"), "utf8"))

      expect(summary.status).toBe("fail")
      expect(summary.memoryOracle.status).toBe("fail")
      expect(summary.shutdownOracle.status).toBe("pass")
      expect(memoryReport.schema).toBe(KHALA_CODE_QA_MEMORY_ORACLE_SCHEMA)
      expect(memoryReport.samples).toHaveLength(5)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
