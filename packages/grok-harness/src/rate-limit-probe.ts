/**
 * RL-1 / RL-2 — measure free CLI plane concurrency ceilings.
 *
 * Spawns N concurrent `grok -p` workers with a tiny prompt and records
 * success / 429 / other failures. Writes a JSON receipt for docs.
 */

import { mkdir, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { spawn } from "node:child_process"

import type { AuthPlane, GrokFailureClass, MarginalCostClass } from "./types.ts"

export type RlProbeWorkerResult = {
  readonly index: number
  readonly ok: boolean
  readonly wallClockMs: number
  readonly exitCode: number
  readonly stdoutBytes: number
  readonly stderrPreview: string
  readonly failureClass?: GrokFailureClass
  readonly textPreview: string
}

export type RlProbeConcurrencyResult = {
  readonly concurrency: number
  readonly startedAt: string
  readonly finishedAt: string
  readonly wallClockMs: number
  readonly successCount: number
  readonly failureCount: number
  readonly rateLimitedCount: number
  readonly workers: readonly RlProbeWorkerResult[]
}

export type RlProbeReceipt = {
  readonly schema: "openagents.grok_harness.rl_probe.v1"
  readonly plane: AuthPlane
  readonly marginalCostClass: MarginalCostClass
  readonly binary: string
  readonly model?: string
  readonly prompt: string
  readonly host: string
  readonly measuredAt: string
  readonly concurrencies: readonly RlProbeConcurrencyResult[]
  /** Highest concurrency with 100% success in this run. */
  readonly maxFullSuccessConcurrency: number
  /** Highest concurrency with ≥ successRatio threshold. */
  readonly maxPartialSuccessConcurrency: number
  readonly notes: readonly string[]
}

function classify(stderr: string, code: number): GrokFailureClass | undefined {
  if (code === 0) return undefined
  const lower = stderr.toLowerCase()
  if (lower.includes("429") || lower.includes("rate limit") || lower.includes("too many")) {
    return "account_rate_limited"
  }
  if (lower.includes("quota") || lower.includes("usage limit")) {
    return "account_quota_exhausted"
  }
  if (lower.includes("login") || lower.includes("auth") || lower.includes("unauthorized")) {
    return "auth_required"
  }
  if (code === 127) return "binary_missing"
  return "unknown"
}

function runOne(input: {
  readonly index: number
  readonly binary: string
  readonly prompt: string
  readonly model?: string
  readonly cwd: string
  readonly env: NodeJS.ProcessEnv
}): Promise<RlProbeWorkerResult> {
  const started = Date.now()
  const argv = [
    input.binary,
    "--no-auto-update",
    "--no-alt-screen",
    "-p",
    `${input.prompt} #${input.index}`,
    "--cwd",
    input.cwd,
    "--output-format",
    "plain",
    ...(input.model ? ["-m", input.model] : []),
  ]

  return new Promise((resolve) => {
    const proc = spawn(argv[0]!, argv.slice(1), {
      cwd: input.cwd,
      env: input.env,
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
      const failureClass = classify(stderr + stdout, exitCode)
      resolve({
        index: input.index,
        ok: exitCode === 0 && stdout.trim().length > 0,
        wallClockMs: Date.now() - started,
        exitCode,
        stdoutBytes: Buffer.byteLength(stdout),
        stderrPreview: stderr.slice(0, 400),
        textPreview: stdout.trim().slice(0, 200),
        ...(failureClass ? { failureClass } : {}),
      })
    })
  })
}

export async function runRlProbe(options: {
  readonly concurrencies: readonly number[]
  readonly prompt?: string
  readonly binary?: string
  readonly model?: string
  readonly cwd?: string
  readonly env?: NodeJS.ProcessEnv
  readonly successRatio?: number
}): Promise<RlProbeReceipt> {
  const binary = options.binary ?? "grok"
  const prompt = options.prompt ?? "Reply with only the single word: ok"
  const cwd = options.cwd ?? process.cwd()
  const env = options.env ?? process.env
  const successRatio = options.successRatio ?? 1
  const plane: AuthPlane = env.XAI_API_KEY?.trim() ? "api_key" : "cli_session"
  const marginalCostClass: MarginalCostClass =
    plane === "cli_session" ? "free" : "api_metered"

  const concurrencies: RlProbeConcurrencyResult[] = []
  let maxFull = 0
  let maxPartial = 0

  for (const concurrency of options.concurrencies) {
    const startedAt = new Date().toISOString()
    const t0 = Date.now()
    const workers = await Promise.all(
      Array.from({ length: concurrency }, (_, index) =>
        runOne({
          index,
          binary,
          prompt,
          ...(options.model === undefined ? {} : { model: options.model }),
          cwd,
          env,
        }),
      ),
    )
    const wallClockMs = Date.now() - t0
    const successCount = workers.filter((w) => w.ok).length
    const failureCount = concurrency - successCount
    const rateLimitedCount = workers.filter(
      (w) => w.failureClass === "account_rate_limited",
    ).length
    const finishedAt = new Date().toISOString()

    if (successCount === concurrency) maxFull = concurrency
    if (successCount / concurrency >= successRatio) maxPartial = concurrency

    concurrencies.push({
      concurrency,
      startedAt,
      finishedAt,
      wallClockMs,
      successCount,
      failureCount,
      rateLimitedCount,
      workers,
    })
  }

  return {
    schema: "openagents.grok_harness.rl_probe.v1",
    plane,
    marginalCostClass,
    binary,
    ...(options.model === undefined ? {} : { model: options.model }),
    prompt,
    host: env.HOSTNAME ?? env.HOST ?? "local",
    measuredAt: new Date().toISOString(),
    concurrencies,
    maxFullSuccessConcurrency: maxFull,
    maxPartialSuccessConcurrency: maxPartial,
    notes: [
      "RL-1: concurrent grok -p sessions on one host (cli_session or api_key plane).",
      "RL-2: token usage not exposed reliably on plain output — metering labeled not_measured.",
      "Conservative auto defaults should not exceed maxFullSuccessConcurrency without a new receipt.",
      "Free-window economics: marginal_cost_class=free only while plane=cli_session remains unmetered for us.",
    ],
  }
}

export async function writeRlProbeReceipt(
  receipt: RlProbeReceipt,
  path: string,
): Promise<string> {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify(receipt, null, 2)}\n`, "utf8")
  return path
}

export function defaultRlReceiptPath(now = new Date()): string {
  const stamp = now.toISOString().replace(/[:.]/g, "-")
  return join(
    process.cwd(),
    "docs/grok/receipts",
    `rl-probe-${stamp}.json`,
  )
}
