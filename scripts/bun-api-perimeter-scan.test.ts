import { Runtime } from "@openagentsinc/runtime-platform"
import { describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const runScan = async (
  args: readonly string[],
): Promise<{ readonly exitCode: number; readonly stdout: string }> => {
  const proc = Runtime.spawn(
    [process.execPath, "--import", "tsx", "scripts/bun-api-perimeter-scan.ts", ...args],
    {
      cwd: process.cwd(),
      stderr: "pipe",
      stdout: "pipe",
    },
  )

  const [stdout, _stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])

  return { exitCode, stdout }
}

describe("bun-api perimeter scan", () => {
  test("passes on the repo with a zero Bun perimeter", async () => {
    const { exitCode, stdout } = await runScan([])

    expect(exitCode).toBe(0)
    expect(stdout).toContain("Bun-API perimeter scan")
    expect(stdout).toContain("mode: enforce")
    expect(stdout).toContain("new violations: 0")
    expect(stdout).toContain("perimeter (named seam) findings: 0")
    expect(stdout).toContain("grandfathered files: 0 of 0 allowlisted")
  }, 60_000)

  test("fails on a fixture violation outside the allowlist", async () => {
    const fixtureRoot = mkdtempSync(join(tmpdir(), "bun-perimeter-fixture-"))
    try {
      mkdirSync(join(fixtureRoot, "src"), { recursive: true })
      writeFileSync(
        join(fixtureRoot, "src", "fresh-bun-usage.ts"),
        [
          'import { Database } from "bun:sqlite"',
          'export const db = new Database(":memory:")',
          'export const text = await Bun.file("x").text()',
          "",
        ].join("\n"),
      )

      const { exitCode, stdout } = await runScan([fixtureRoot])

      expect(exitCode).toBe(1)
      expect(stdout).toContain("new violations: 2")
      expect(stdout).toContain("NEW ")
      expect(stdout).toContain("fresh-bun-usage.ts:1")
      expect(stdout).toContain("fresh-bun-usage.ts:3")
      expect(stdout).toContain("@openagentsinc/sqlite-runtime")
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true })
    }
  }, 60_000)
})
