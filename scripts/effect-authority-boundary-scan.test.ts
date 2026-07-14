import { Runtime } from "@openagentsinc/runtime-platform"
import { describe, expect, test } from "vite-plus/test"

describe("effect authority-boundary scan", () => {
  test("runs report-only and reports the current migration inventory", async () => {
    const proc = Runtime.spawn([process.execPath, "--import", "tsx", "scripts/effect-authority-boundary-scan.ts"], {
      cwd: process.cwd(),
      stderr: "pipe",
      stdout: "pipe",
    })

    const [stdout, _stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ])

    expect(exitCode).toBe(0)
    expect(stdout).toContain("Effect authority-boundary scan")
    expect(stdout).toContain("mode: report-only")
    expect(stdout).toMatch(/findings: [1-9][0-9]*/)
    expect(stdout).toContain("MIGRATE ")
  })
})
