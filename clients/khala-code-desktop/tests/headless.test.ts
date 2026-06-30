import { describe, expect, test } from "bun:test"

describe("Khala Code desktop headless JSONL mode", () => {
  test("writes progress ThreadEvents to stderr and one final message to stdout", async () => {
    const proc = Bun.spawn([
      process.execPath,
      "src/bun/index.ts",
      "--json",
      "hello",
    ], {
      cwd: new URL("..", import.meta.url).pathname,
      env: {
        ...process.env,
        KHALA_CODE_DESKTOP_BACKEND: "mock",
      },
      stderr: "pipe",
      stdout: "pipe",
    })
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ])

    const stdoutLines = stdout.trim().split(/\n/u).filter(Boolean)
    const progress = stderr.trim().split(/\n/u).filter(Boolean).map(line => JSON.parse(line) as { type: string })
    const final = JSON.parse(stdoutLines[0] ?? "{}") as {
      finalMessage?: string
      ok?: boolean
    }

    expect(exitCode).toBe(0)
    expect(stdoutLines).toHaveLength(1)
    expect(progress.map(event => event.type)).toEqual([
      "thread.started",
      "turn.started",
      "item.started",
      "item.completed",
      "turn.completed",
    ])
    expect(final).toMatchObject({
      finalMessage: "Mock Khala Code is ready with the full local tool catalog enabled.",
      ok: true,
    })
  })
})
