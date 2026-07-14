import { Runtime } from "@openagentsinc/runtime-platform"
import { describe, expect, test } from "vite-plus/test"

describe("effect authority-boundary scan", () => {
  test("runs report-only and emits line-addressed findings", async () => {
    const proc = Runtime.spawn([process.execPath, "scripts/effect-authority-boundary-scan.ts"], {
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
    expect(stdout).toMatch(/MIGRATE .+:\d+ /)
  })
})
