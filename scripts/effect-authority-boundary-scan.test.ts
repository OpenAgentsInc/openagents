import { Runtime } from "@openagentsinc/runtime-platform"
import { describe, expect, test } from "vite-plus/test"

describe("effect authority-boundary scan", () => {
  test("runs report-only and reports the clean authority boundary", async () => {
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
    expect(stdout).toContain("findings: 0")
    expect(stdout).toContain("No suspicious authority-boundary operations found.")
  })
})
