import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { describe, expect, test } from "bun:test"

import {
  khalaMobileVisualTierStarterCaptures,
  runKhalaMobileVisualTier,
} from "./mobile-visual-tier.js"

describe("Khala mobile visual tier", () => {
  test("blesses a starter baseline set with public-safe visual-baseline records", async () => {
    await withTempDir(async dir => {
      const report = await runKhalaMobileVisualTier({
        baselineDir: join(dir, "baselines"),
        bless: true,
        captures: khalaMobileVisualTierStarterCaptures(),
        candidateDir: join(dir, "candidates"),
        now: () => "2026-07-07T00:00:00.000Z",
      })

      expect(report.ok).toBe(true)
      expect(report.results.map(result => result.status)).toEqual(["blessed", "blessed"])
      expect(report.simulatorTruth).toBe("not_claimed")
      expect(await readFile(join(dir, "baselines", "manifest.json"), "utf8")).toContain(
        "openagents.khala_visual_baselines.v1",
      )
    })
  })

  test("reports an unexplained changed screenshot as blocking", async () => {
    await withTempDir(async dir => {
      await runKhalaMobileVisualTier({
        baselineDir: join(dir, "baselines"),
        bless: true,
        captures: khalaMobileVisualTierStarterCaptures(),
        candidateDir: join(dir, "candidates-a"),
        now: () => "2026-07-07T00:00:00.000Z",
      })
      const changedCapture = {
        ...khalaMobileVisualTierStarterCaptures()[0]!,
        png: khalaMobileVisualTierStarterCaptures()[1]!.png,
      }

      const report = await runKhalaMobileVisualTier({
        baselineDir: join(dir, "baselines"),
        captures: [changedCapture],
        candidateDir: join(dir, "candidates-b"),
        requireBaseline: true,
      })

      expect(report.ok).toBe(false)
      expect(report.changed).toHaveLength(1)
      expect(report.changed[0]?.status).toBe("changed")
      expect(report.changed[0]?.delta).toMatch(/^deltas\//)
    })
  })

  test("records the blessing reason for an intentional changed screenshot", async () => {
    await withTempDir(async dir => {
      await runKhalaMobileVisualTier({
        baselineDir: join(dir, "baselines"),
        bless: true,
        captures: khalaMobileVisualTierStarterCaptures(),
        candidateDir: join(dir, "candidates-a"),
        now: () => "2026-07-07T00:00:00.000Z",
      })
      const changedCapture = {
        ...khalaMobileVisualTierStarterCaptures()[0]!,
        png: khalaMobileVisualTierStarterCaptures()[1]!.png,
      }

      const report = await runKhalaMobileVisualTier({
        baselineDir: join(dir, "baselines"),
        blessings: [{
          captureId: changedCapture.id,
          reason: "Intentional starter baseline refresh proves QAM-4 blessing receipts.",
          reviewedBy: "codex",
        }],
        captures: [changedCapture],
        candidateDir: join(dir, "candidates-b"),
        requireBaseline: true,
      })

      expect(report.ok).toBe(true)
      expect(report.blessings[0]?.reason).toContain("Intentional")
      expect(report.results[0]?.status).toBe("blessed")
    })
  })
})

const withTempDir = async (
  run: (dir: string) => Promise<void>,
): Promise<void> => {
  const dir = await mkdtemp(join(tmpdir(), "khala-mobile-visual-tier-"))
  try {
    await run(dir)
  } finally {
    await rm(dir, { force: true, recursive: true })
  }
}
