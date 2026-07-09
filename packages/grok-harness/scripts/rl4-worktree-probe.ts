#!/usr/bin/env bun
/**
 * RL-4: concurrent Grok workers with worktree-ish cwd isolation and a
 * tool-using (always-approve) prompt — harder than RL-1 tiny plain chat.
 *
 *   bun packages/grok-harness/scripts/rl4-worktree-probe.ts --concurrency 1,2,4,8
 */

import { mkdir, mkdtemp, writeFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join, dirname, resolve } from "node:path"
import { spawn } from "node:child_process"

type WorkerResult = {
  index: number
  ok: boolean
  wallClockMs: number
  exitCode: number
  textPreview: string
  stderrPreview: string
  failureClass?: string
}

function classify(stderr: string, code: number): string | undefined {
  if (code === 0) return undefined
  const lower = stderr.toLowerCase()
  if (lower.includes("429") || lower.includes("rate limit")) return "account_rate_limited"
  if (lower.includes("quota")) return "account_quota_exhausted"
  if (lower.includes("auth") || lower.includes("login")) return "auth_required"
  return "unknown"
}

async function runWorker(input: {
  index: number
  binary: string
  workDir: string
  prompt: string
}): Promise<WorkerResult> {
  const started = Date.now()
  const argv = [
    input.binary,
    "--no-auto-update",
    "--no-alt-screen",
    "--always-approve",
    "-p",
    input.prompt,
    "--cwd",
    input.workDir,
    "--output-format",
    "plain",
  ]
  return await new Promise((resolvePromise) => {
    const proc = spawn(argv[0]!, argv.slice(1), {
      cwd: input.workDir,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    })
    let stdout = ""
    let stderr = ""
    proc.stdout.on("data", (c) => {
      stdout += String(c)
    })
    proc.stderr.on("data", (c) => {
      stderr += String(c)
    })
    proc.on("close", (code) => {
      const exitCode = code ?? 1
      resolvePromise({
        index: input.index,
        ok: exitCode === 0 && stdout.trim().length > 0,
        wallClockMs: Date.now() - started,
        exitCode,
        textPreview: stdout.trim().slice(0, 200),
        stderrPreview: stderr.slice(0, 300),
        ...(classify(stderr + stdout, exitCode)
          ? { failureClass: classify(stderr + stdout, exitCode) }
          : {}),
      })
    })
  })
}

function parseArgs(argv: string[]) {
  let concurrency = [1, 2, 4, 8]
  let out: string | undefined
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--concurrency" && argv[i + 1]) {
      concurrency = argv[++i]!.split(",").map((s) => Number(s.trim())).filter((n) => n > 0)
    } else if (argv[i] === "--out" && argv[i + 1]) {
      out = argv[++i]
    }
  }
  return { concurrency, out }
}

const args = parseArgs(process.argv.slice(2))
const binary = "grok"
const root = await mkdtemp(join(tmpdir(), "grok-rl4-"))
const bands = []
let maxFull = 0

try {
  for (const n of args.concurrency) {
    // Isolated dirs (worktree-like) with a tiny fixture file each
    const dirs: string[] = []
    for (let i = 0; i < n; i++) {
      const d = join(root, `wt-${n}-${i}`)
      await mkdir(d, { recursive: true })
      await writeFile(
        join(d, "README.md"),
        `# fixture ${i}\nmarker=rl4-${n}-${i}\n`,
        "utf8",
      )
      dirs.push(d)
    }

    const prompt =
      "Read README.md in the current working directory. Reply with only the " +
      "marker value from that file (the rl4-... string). Do not invent."

    const t0 = Date.now()
    const workers = await Promise.all(
      dirs.map((workDir, index) =>
        runWorker({ index, binary, workDir, prompt }),
      ),
    )
    const success = workers.filter((w) => w.ok).length
    if (success === n) maxFull = n
    bands.push({
      concurrency: n,
      wallClockMs: Date.now() - t0,
      successCount: success,
      failureCount: n - success,
      rateLimitedCount: workers.filter((w) => w.failureClass === "account_rate_limited").length,
      workers,
    })
  }
} finally {
  // keep root for debugging on failure? always clean
  await rm(root, { recursive: true, force: true }).catch(() => {})
}

const receipt = {
  schema: "openagents.grok_harness.rl4_worktree_probe.v1",
  plane: process.env.XAI_API_KEY ? "api_key" : "cli_session",
  marginalCostClass: process.env.XAI_API_KEY ? "api_metered" : "free",
  binary,
  measuredAt: new Date().toISOString(),
  promptClass: "read_file_in_cwd_always_approve",
  bands,
  maxFullSuccessConcurrency: maxFull,
  notes: [
    "RL-4: concurrent workers with isolated cwd + always-approve tool path.",
    "Harder than RL-1 plain chat; use this floor for tool-using fleet soft caps.",
  ],
}

const outPath = resolve(
  args.out ??
    join(
      process.cwd(),
      "docs/grok/receipts",
      `rl4-worktree-${new Date().toISOString().replace(/[:.]/g, "-")}.json`,
    ),
)
await mkdir(dirname(outPath), { recursive: true })
await writeFile(outPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8")

console.log(
  JSON.stringify(
    {
      outPath,
      maxFullSuccessConcurrency: maxFull,
      summary: bands.map((b) => ({
        concurrency: b.concurrency,
        success: b.successCount,
        fail: b.failureCount,
        rateLimited: b.rateLimitedCount,
        wallClockMs: b.wallClockMs,
      })),
    },
    null,
    2,
  ),
)
