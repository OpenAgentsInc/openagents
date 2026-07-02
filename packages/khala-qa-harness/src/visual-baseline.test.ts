import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { describe, expect, test } from "bun:test"

import {
  assertKhalaVisualBaseline,
  evaluateKhalaVisualBaseline,
  KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA,
  readKhalaVisualBaselineManifest,
} from "./visual-baseline.js"

const rgbaWhitePng = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
  "base64",
)
const grayscaleBlackPng = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=",
  "base64",
)

describe("Khala visual baseline oracle", () => {
  test("blesses a public-safe screenshot and records a relative manifest entry", async () => {
    await withTempDir(async dir => {
      const screenshotPath = join(dir, "candidate.png")
      await writeFile(screenshotPath, rgbaWhitePng)

      const result = await evaluateKhalaVisualBaseline({
        baselineDir: join(dir, "baselines"),
        bless: true,
        capture: capture(screenshotPath),
        now: () => "2026-07-02T00:00:00.000Z",
      })
      const manifest = await readKhalaVisualBaselineManifest(join(dir, "baselines"))

      expect(result.status).toBe("blessed")
      expect(result.ok).toBe(true)
      expect(manifest).toMatchObject({
        schema: KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA,
        entries: [{
          height: 1,
          id: "cockpit.desktop",
          redactionCheckedAt: "2026-07-02T00:00:00.000Z",
          screenshot: "screenshots/cockpit.desktop.png",
          viewport: "desktop",
          width: 1,
        }],
      })
      expect(manifest.entries[0]?.screenshot).not.toContain(dir)
    })
  })

  test("matches identical pixels even when PNG encoding metadata differs", async () => {
    await withTempDir(async dir => {
      const baselinePath = join(dir, "baseline.png")
      const candidatePath = join(dir, "candidate.png")
      await writeFile(baselinePath, rgbaWhitePng)
      await writeFile(candidatePath, rgbaWhitePng)

      await assertKhalaVisualBaseline({
        baselineDir: join(dir, "baselines"),
        bless: true,
        capture: capture(baselinePath),
      })
      const result = await assertKhalaVisualBaseline({
        baselineDir: join(dir, "baselines"),
        capture: capture(candidatePath),
        requireBaseline: true,
      })

      expect(result.status).toBe("matched")
      expect(result.ok).toBe(true)
    })
  })

  test("writes a delta image and fails when pixels differ", async () => {
    await withTempDir(async dir => {
      const baselinePath = join(dir, "baseline.png")
      const candidatePath = join(dir, "candidate.png")
      await writeFile(baselinePath, rgbaWhitePng)
      await writeFile(candidatePath, grayscaleBlackPng)

      await assertKhalaVisualBaseline({
        baselineDir: join(dir, "baselines"),
        bless: true,
        capture: capture(baselinePath),
      })
      const result = await evaluateKhalaVisualBaseline({
        baselineDir: join(dir, "baselines"),
        capture: capture(candidatePath),
        requireBaseline: true,
      })

      expect(result.ok).toBe(false)
      expect(result.status).toBe("changed")
      expect(result.delta).toBe("deltas/cockpit.desktop.delta.png")
      expect(result.diffPixels).toBe(1)
      const deltaBytes = await readFile(join(dir, "baselines", result.delta!))
      expect(deltaBytes.subarray(0, 8)).toEqual(Buffer.from([
        0x89,
        0x50,
        0x4e,
        0x47,
        0x0d,
        0x0a,
        0x1a,
        0x0a,
      ]))
    })
  })

  test("can soft-report or require missing baselines", async () => {
    await withTempDir(async dir => {
      const screenshotPath = join(dir, "candidate.png")
      await writeFile(screenshotPath, rgbaWhitePng)

      const soft = await evaluateKhalaVisualBaseline({
        baselineDir: join(dir, "baselines"),
        capture: capture(screenshotPath),
      })
      const strict = await evaluateKhalaVisualBaseline({
        baselineDir: join(dir, "baselines"),
        capture: capture(screenshotPath),
        requireBaseline: true,
      })

      expect(soft).toMatchObject({ ok: true, status: "missing" })
      expect(strict).toMatchObject({ ok: false, status: "missing" })
    })
  })

  test("rejects unsafe manifest metadata and unsafe capture ids", async () => {
    await withTempDir(async dir => {
      const baselineDir = join(dir, "baselines")
      await writeFile(join(dir, "candidate.png"), rgbaWhitePng)
      await mkdir(baselineDir, { recursive: true })
      await writeFile(
        join(baselineDir, "manifest.json"),
        `${JSON.stringify({
          entries: [{
            colorScheme: "dark",
            harness: "khala_code",
            height: 1,
            id: "cockpit.desktop",
            redactionCheckedAt: "2026-07-02T00:00:00.000Z",
            reducedMotion: "no-preference",
            screenshot: "/Users/alice/private.png",
            sha256: "a".repeat(64),
            viewport: "desktop",
            width: 1,
          }],
          schema: KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA,
        }, null, 2)}\n`,
      )

      await expect(readKhalaVisualBaselineManifest(baselineDir)).rejects.toThrow(
        "screenshot path",
      )
      await expect(
        evaluateKhalaVisualBaseline({
          baselineDir,
          capture: capture(join(dir, "candidate.png"), "unsafe/secret"),
        }),
      ).rejects.toThrow("public-safe")
    })
  })
})

const capture = (screenshotPath: string, id = "cockpit.desktop") => ({
  colorScheme: "dark" as const,
  harness: "khala_code_visual_test",
  id,
  reducedMotion: "no-preference" as const,
  screenshotPath,
  viewport: "desktop",
})

const withTempDir = async (
  run: (dir: string) => Promise<void>,
): Promise<void> => {
  const dir = await mkdtemp(join(tmpdir(), "khala-visual-baseline-"))
  try {
    await run(dir)
  } finally {
    await rm(dir, { force: true, recursive: true })
  }
}
