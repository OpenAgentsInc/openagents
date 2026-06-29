import { describe, expect, test } from "bun:test"

describe("effect authority-boundary scan", () => {
  test("runs report-only and emits line-addressed findings", async () => {
    const proc = Bun.spawn([process.execPath, "run", "scan:effect-authority-boundaries"], {
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
