/**
 * Axis B — Grok worker executor behind a pylon-core-shaped port.
 * Does not live in apps/pylon; importable by pylon-core when PY-1 lands.
 */

import { spawn } from "node:child_process"

import type {
  AuthPlane,
  GrokFailureClass,
  GrokUsageSnapshot,
  MarginalCostClass,
  WorkerClaimPin,
  WorkerCloseout,
} from "./types.ts"

export type GrokWorkerExecutorPort = {
  readonly kind: "grok_cli"
  readonly runClaimedWork: (input: {
    readonly pin: WorkerClaimPin
    readonly prompt: string
    readonly model?: string
    readonly timeoutMs?: number
    readonly plane?: AuthPlane
    readonly marginalCostClass?: MarginalCostClass
  }) => Promise<WorkerCloseout>
  readonly readiness: () => Promise<GrokReadiness>
}

export type GrokReadiness = {
  readonly ready: boolean
  readonly binary: string
  readonly version?: string
  readonly plane: AuthPlane
  readonly models: readonly string[]
  readonly failureClass?: GrokFailureClass
  readonly detail?: string
}

function classifyError(text: string, code: number | null): GrokFailureClass {
  const lower = text.toLowerCase()
  if (lower.includes("429") || lower.includes("rate limit")) {
    return "account_rate_limited"
  }
  if (lower.includes("quota") || lower.includes("usage limit")) {
    return "account_quota_exhausted"
  }
  if (
    lower.includes("login") ||
    lower.includes("auth") ||
    lower.includes("unauthorized") ||
    lower.includes("api key")
  ) {
    return "auth_required"
  }
  if (code === 127 || lower.includes("not found")) return "binary_missing"
  if (lower.includes("timeout")) return "timeout"
  return "unknown"
}

export async function probeGrokReadiness(options: {
  readonly binary?: string
  readonly env?: NodeJS.ProcessEnv
} = {}): Promise<GrokReadiness> {
  const binary = options.binary ?? "grok"
  const env = options.env ?? process.env
  const plane: AuthPlane = env.XAI_API_KEY?.trim()
    ? "api_key"
    : "cli_session"

  try {
    const versionProc = Bun.spawn([binary, "version"], {
      stdout: "pipe",
      stderr: "pipe",
      env,
    })
    const versionOut = await new Response(versionProc.stdout).text()
    const versionErr = await new Response(versionProc.stderr).text()
    const versionCode = await versionProc.exited
    if (versionCode !== 0) {
      return {
        ready: false,
        binary,
        plane,
        models: [],
        failureClass: classifyError(versionErr || versionOut, versionCode),
        detail: (versionErr || versionOut).trim(),
      }
    }

    const modelsProc = Bun.spawn([binary, "models"], {
      stdout: "pipe",
      stderr: "pipe",
      env,
    })
    const modelsOut = await new Response(modelsProc.stdout).text()
    await modelsProc.exited
    const models = modelsOut
      .split("\n")
      .map((l) => l.replace(/^[*\-\s]+/, "").trim())
      .filter((l) => l.startsWith("grok-"))

    return {
      ready: true,
      binary,
      version: versionOut.trim().split("\n")[0],
      plane,
      models,
    }
  } catch (error) {
    return {
      ready: false,
      binary,
      plane,
      models: [],
      failureClass: "binary_missing",
      detail: error instanceof Error ? error.message : String(error),
    }
  }
}

/**
 * Headless single-turn worker via `grok -p` (secondary path).
 * ACP path can wrap the same port later.
 */
export function createGrokHeadlessWorkerExecutor(options: {
  readonly binary?: string
  readonly env?: NodeJS.ProcessEnv
  /** Inject for tests */
  readonly runCommand?: (argv: string[], cwd: string) => Promise<{
    code: number
    stdout: string
    stderr: string
    wallClockMs: number
  }>
} = {}): GrokWorkerExecutorPort {
  const binary = options.binary ?? "grok"

  const runCommand =
    options.runCommand ??
    (async (argv, cwd) => {
      const started = Date.now()
      const proc = spawn(argv[0]!, argv.slice(1), {
        cwd,
        env: options.env ?? process.env,
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
      const code: number = await new Promise((resolve) => {
        proc.on("close", (c) => resolve(c ?? 1))
      })
      return { code, stdout, stderr, wallClockMs: Date.now() - started }
    })

  return {
    kind: "grok_cli",
    async readiness() {
      return probeGrokReadiness({ binary, env: options.env })
    },
    async runClaimedWork(input) {
      const plane = input.plane ?? (options.env?.XAI_API_KEY ? "api_key" : "cli_session")
      const marginalCostClass: MarginalCostClass =
        input.marginalCostClass ?? (plane === "cli_session" ? "free" : "api_metered")

      const systemPin = [
        `claimRef=${input.pin.claimRef}`,
        `workUnitRef=${input.pin.workUnitRef}`,
        `runRef=${input.pin.runRef}`,
        input.pin.repo ? `repo=${input.pin.repo}` : null,
        input.pin.commit ? `commit=${input.pin.commit}` : null,
        input.pin.branch ? `branch=${input.pin.branch}` : null,
        input.pin.verifyCommand ? `verify=${input.pin.verifyCommand}` : null,
        "Stay in scope. One focused change. Do not expand.",
      ]
        .filter(Boolean)
        .join("\n")

      const prompt = `${systemPin}\n\n${input.prompt}`

      const argv = [
        binary,
        "--no-auto-update",
        "--no-alt-screen",
        "-p",
        prompt,
        "--cwd",
        input.pin.cwd,
        "--output-format",
        "plain",
        ...(input.model ? ["-m", input.model] : []),
      ]

      const result = await runCommand(argv, input.pin.cwd)
      const combined = `${result.stdout}\n${result.stderr}`
      const failureClass =
        result.code === 0 ? undefined : classifyError(combined, result.code)

      const usage: GrokUsageSnapshot = {
        metering: "not_measured",
        wallClockMs: result.wallClockMs,
        model: input.model,
        plane,
        marginalCostClass,
      }

      return {
        ok: result.code === 0,
        claimRef: input.pin.claimRef,
        stopReason: result.code === 0 ? "end_turn" : `exit_${result.code}`,
        text: result.stdout.trim(),
        usage,
        ...(failureClass ? { failureClass } : {}),
      }
    },
  }
}
