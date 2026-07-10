/**
 * Axis B — Grok worker executor behind a pylon-core-shaped port.
 * Does not live in apps/pylon; importable by pylon-core when PY-1 lands.
 */

import { spawn } from "node:child_process"
import { open } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

import type {
  AuthPlane,
  GrokFailureClass,
  GrokUsageSnapshot,
  MarginalCostClass,
  WorkerClaimPin,
  WorkerCloseout,
} from "./types.js"

export type GrokClaimedWorkInput = {
  readonly pin: WorkerClaimPin
  readonly prompt: string
  readonly model?: string
  readonly timeoutMs?: number
  readonly plane?: AuthPlane
  readonly marginalCostClass?: MarginalCostClass
  readonly sessionId?: string
}

export type GrokFollowUpInput = Omit<GrokClaimedWorkInput, "sessionId"> & {
  readonly sessionId: string
}

export type GrokWorkerExecutorPort = {
  readonly kind: "grok_cli"
  readonly runClaimedWork: (input: GrokClaimedWorkInput) => Promise<WorkerCloseout>
  readonly runFollowUp: (input: GrokFollowUpInput) => Promise<WorkerCloseout>
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

export type GrokReadinessProbeCommand = (input: {
  readonly argv: readonly string[]
  readonly env: NodeJS.ProcessEnv
  readonly timeoutMs: number
}) => Promise<{
  readonly code: number
  readonly stdout: string
  readonly stderr: string
  readonly timedOut: boolean
}>

export const DEFAULT_GROK_READINESS_TIMEOUT_MS = 10_000
export const MAX_GROK_READINESS_TIMEOUT_MS = 60_000
export const MAX_GROK_READINESS_OUTPUT_BYTES = 65_536
export const MAX_GROK_READINESS_MODELS = 256
export const DEFAULT_GROK_WORKER_TIMEOUT_MS = 30 * 60 * 1_000
export const MAX_GROK_WORKER_TIMEOUT_MS = 4 * 60 * 60 * 1_000
export const MAX_GROK_WORKER_OUTPUT_BYTES = 1_048_576
const GROK_READINESS_FORCE_KILL_GRACE_MS = 250
const GROK_CONFIG_SCAN_MAX_BYTES = 1_048_576
const GROK_CONFIG_SCAN_CHUNK_BYTES = 65_536
const CANONICAL_UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u

const isCanonicalUuid = (value: string): boolean =>
  CANONICAL_UUID_PATTERN.test(value)

const sharedGrokCredentialKey = (key: string): boolean =>
  key === "XAI_API_KEY" ||
  key === "GROK_CODE_XAI_API_KEY" ||
  key === "GROK_AUTH" ||
  key === "GROK_AUTH_PATH" ||
  key === "GROK_LOCAL_AUTH" ||
  key === "GROK_ALPHA_TEST_KEY" ||
  key === "GROK_DEPLOYMENT_KEY" ||
  key.startsWith("GROK_AUTH_") ||
  key.startsWith("GROK_OIDC_") ||
  key.startsWith("GROK_OAUTH2_")

export function isolateGrokCliEnvironment(
  baseEnv: NodeJS.ProcessEnv,
  home: string,
): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = { ...baseEnv, GROK_HOME: home }
  for (const key of Object.keys(env)) {
    if (sharedGrokCredentialKey(key)) delete env[key]
  }
  return env
}

const sharedGrokCredentialPresent = (env: NodeJS.ProcessEnv): boolean =>
  Object.entries(env).some(([key, value]) =>
    sharedGrokCredentialKey(key) && typeof value === "string" && value.trim() !== ""
  )

const readBoundedStream = async (
  stream: ReadableStream<Uint8Array>,
): Promise<string> => {
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  let bytes = 0
  let output = ""
  try {
    while (true) {
      const next = await reader.read()
      if (next.done) break
      bytes += next.value.byteLength
      if (bytes > MAX_GROK_READINESS_OUTPUT_BYTES) {
        await reader.cancel().catch(() => undefined)
        throw new Error("Grok readiness output exceeded its bound.")
      }
      output += decoder.decode(next.value, { stream: true })
    }
    return output + decoder.decode()
  } finally {
    reader.releaseLock()
  }
}

const probeOutputIsBounded = (value: string): boolean =>
  new TextEncoder().encode(value).byteLength <= MAX_GROK_READINESS_OUTPUT_BYTES

const workerOutputIsBounded = (value: string): boolean =>
  new TextEncoder().encode(value).byteLength <= MAX_GROK_WORKER_OUTPUT_BYTES

const boundedWorkerTimeout = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) return DEFAULT_GROK_WORKER_TIMEOUT_MS
  return Math.max(100, Math.min(MAX_GROK_WORKER_TIMEOUT_MS, Math.trunc(value)))
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

const fixedFailureDetail = (
  failureClass: GrokFailureClass,
): string => {
  switch (failureClass) {
    case "account_rate_limited":
      return "Grok readiness is rate limited."
    case "account_quota_exhausted":
      return "Grok readiness quota is exhausted."
    case "auth_required":
      return "Grok isolated CLI authentication is unavailable."
    case "binary_missing":
      return "Grok CLI is unavailable."
    case "timeout":
      return "Grok readiness probe timed out."
    default:
      return "Grok readiness probe failed."
  }
}

const safeVersion = (value: string): string | undefined => {
  const firstLine = value.trim().split("\n")[0]?.trim() ?? ""
  return /^grok(?: build)?(?: version)?\s+v?\d+(?:\.\d+){1,3}(?:[-+][A-Za-z0-9._-]+)?$/iu.test(firstLine)
    ? firstLine
    : undefined
}

const configuredApiCredentialPattern =
  /(?:^|[\s{,.])(?:(?:api_key|env_key|auth_provider_command)|["'](?:api_key|env_key|auth_provider_command)["'])\s*=/mu

async function configDeclaresApiCredential(path: string): Promise<boolean> {
  let handle: Awaited<ReturnType<typeof open>> | undefined
  try {
    handle = await open(path, "r")
    const chunks: Buffer[] = []
    let offset = 0
    while (offset <= GROK_CONFIG_SCAN_MAX_BYTES) {
      const remaining = GROK_CONFIG_SCAN_MAX_BYTES + 1 - offset
      const buffer = Buffer.alloc(Math.min(GROK_CONFIG_SCAN_CHUNK_BYTES, remaining))
      const { bytesRead } = await handle.read(buffer, 0, buffer.byteLength, offset)
      if (bytesRead === 0) break
      chunks.push(buffer.subarray(0, bytesRead))
      offset += bytesRead
    }
    if (offset > GROK_CONFIG_SCAN_MAX_BYTES) return true
    const value = Buffer.concat(chunks, offset).toString("utf8")
    return configuredApiCredentialPattern.test(value)
  } catch (error) {
    const code = error !== null && typeof error === "object" && "code" in error
      ? String(error.code)
      : ""
    return code !== "ENOENT" && code !== "ENOTDIR"
  } finally {
    await handle?.close().catch(() => undefined)
  }
}

/**
 * Named subscription custody is valid only when the CLI cannot resolve a
 * configured API credential ahead of the cached Grok login. We intentionally
 * fail closed on any api_key/env_key declaration without reading or projecting
 * its value.
 */
export async function grokConfiguredApiCredentialPresent(
  env: NodeJS.ProcessEnv,
): Promise<boolean> {
  if (sharedGrokCredentialPresent(env)) return true
  const grokHome = env.GROK_HOME?.trim() || join(homedir(), ".grok")
  const paths = [
    join(grokHome, "config.toml"),
    join(grokHome, "managed_config.toml"),
    join(grokHome, "requirements.toml"),
    "/etc/grok/managed_config.toml",
    "/etc/grok/requirements.toml",
  ]
  for (const path of paths) {
    if (await configDeclaresApiCredential(path)) return true
  }
  return false
}

export async function probeGrokReadiness(options: {
  readonly binary?: string
  readonly env?: NodeJS.ProcessEnv
  readonly timeoutMs?: number
  readonly runCommand?: GrokReadinessProbeCommand
  readonly configuredApiCredentialProbe?: ((env: NodeJS.ProcessEnv) => Promise<boolean>)
} = {}): Promise<GrokReadiness> {
  const binary = options.binary ?? "grok"
  const env = options.env ?? process.env
  const requestedTimeout = options.timeoutMs ?? DEFAULT_GROK_READINESS_TIMEOUT_MS
  const timeoutMs = Number.isFinite(requestedTimeout)
    ? Math.max(100, Math.min(MAX_GROK_READINESS_TIMEOUT_MS, Math.trunc(requestedTimeout)))
    : DEFAULT_GROK_READINESS_TIMEOUT_MS
  const configuredApiCredential = await (
    options.configuredApiCredentialProbe ?? grokConfiguredApiCredentialPresent
  )(env).catch(() => true)
  const plane: AuthPlane = configuredApiCredential ? "api_key" : "cli_session"

  if (configuredApiCredential) {
    return {
      ready: false,
      binary,
      plane,
      models: [],
      failureClass: "auth_required",
      detail: "Grok configured API credentials cannot satisfy isolated CLI custody.",
    }
  }

  const runCommand: GrokReadinessProbeCommand =
    options.runCommand ??
    (async ({ argv, env: commandEnv, timeoutMs: commandTimeoutMs }) => {
      const proc = Bun.spawn([...argv], {
        stdout: "pipe",
        stderr: "pipe",
        env: commandEnv,
      })
      let forceKillTimer: ReturnType<typeof setTimeout> | undefined
      const terminate = () => {
        if (forceKillTimer !== undefined) return
        try {
          proc.kill()
        } catch {
          // The process already exited.
        }
        forceKillTimer = setTimeout(() => {
          try {
            proc.kill("SIGKILL")
          } catch {
            // The process already exited.
          }
        }, GROK_READINESS_FORCE_KILL_GRACE_MS)
      }
      let outputExceeded = false
      type CommandResult = {
        code: number
        stdout: string
        stderr: string
        timedOut: boolean
      }
      let resolveEarly!: (result: CommandResult) => void
      const early = new Promise<CommandResult>((resolve) => {
        resolveEarly = resolve
      })
      const bounded = (stream: ReadableStream<Uint8Array>) =>
        readBoundedStream(stream).catch(() => {
          outputExceeded = true
          terminate()
          resolveEarly({
            code: 1,
            stdout: "",
            stderr: "Grok readiness output exceeded its bound.",
            timedOut: false,
          })
          return ""
        })
      const completed = Promise.all([
        bounded(proc.stdout),
        bounded(proc.stderr),
        proc.exited,
      ]).then(([stdout, stderr, code]) => ({
        code: outputExceeded ? 1 : code,
        stdout,
        stderr: outputExceeded
          ? "Grok readiness output exceeded its bound."
          : stderr,
        timedOut: false,
      }))
      const timer = setTimeout(() => {
        terminate()
        resolveEarly({
          code: 1,
          stdout: "",
          stderr: "Grok readiness probe timed out.",
          timedOut: true,
        })
      }, commandTimeoutMs)
      const outcome = await Promise.race([
        completed.then((result) => ({ source: "completed" as const, result })),
        early.then((result) => ({ source: "early" as const, result })),
      ])
      clearTimeout(timer)
      if (forceKillTimer !== undefined && outcome.source === "completed") {
        clearTimeout(forceKillTimer)
      }
      return outcome.result
    })

  try {
    const version = await runCommand({
      argv: [binary, "version"],
      env,
      timeoutMs,
    })
    if (
      !probeOutputIsBounded(version.stdout) ||
      !probeOutputIsBounded(version.stderr)
    ) {
      return {
        ready: false,
        binary,
        plane,
        models: [],
        failureClass: "unknown",
        detail: "Grok readiness output exceeded its bound.",
      }
    }
    if (version.timedOut || version.code !== 0) {
      const failureClass = version.timedOut
        ? "timeout"
        : classifyError(version.stderr || version.stdout, version.code)
      return {
        ready: false,
        binary,
        plane,
        models: [],
        failureClass,
        detail: fixedFailureDetail(failureClass),
      }
    }

    const versionText = safeVersion(version.stdout)

    const modelsResult = await runCommand({
      argv: [binary, "models"],
      env,
      timeoutMs,
    })
    if (
      !probeOutputIsBounded(modelsResult.stdout) ||
      !probeOutputIsBounded(modelsResult.stderr)
    ) {
      return {
        ready: false,
        binary,
        ...(versionText === undefined ? {} : { version: versionText }),
        plane,
        models: [],
        failureClass: "unknown",
        detail: "Grok readiness output exceeded its bound.",
      }
    }
    if (modelsResult.timedOut || modelsResult.code !== 0) {
      const failureClass = modelsResult.timedOut
        ? "timeout"
        : classifyError(
            modelsResult.stderr || modelsResult.stdout,
            modelsResult.code,
          )
      return {
        ready: false,
        binary,
        ...(versionText === undefined ? {} : { version: versionText }),
        plane,
        models: [],
        failureClass,
        detail: fixedFailureDetail(failureClass),
      }
    }
    const models = modelsResult.stdout
      .split("\n")
      .map((l) => l.slice(0, 256).replace(/^[*\-\s]+/, "").trim())
      .filter((l) => l.startsWith("grok-"))
      .slice(0, MAX_GROK_READINESS_MODELS)

    if (models.length === 0) {
      return {
        ready: false,
        binary,
        ...(versionText === undefined ? {} : { version: versionText }),
        plane,
        models: [],
        failureClass: "unknown",
        detail: "Grok readiness returned no runnable models.",
      }
    }

    return {
      ready: true,
      binary,
      ...(versionText === undefined ? {} : { version: versionText }),
      plane,
      models,
    }
  } catch {
    return {
      ready: false,
      binary,
      plane,
      models: [],
      failureClass: "binary_missing",
      detail: fixedFailureDetail("binary_missing"),
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
  readonly runCommand?: (argv: string[], cwd: string, timeoutMs: number) => Promise<{
    code: number
    stdout: string
    stderr: string
    wallClockMs: number
    timedOut?: boolean
  }>
} = {}): GrokWorkerExecutorPort {
  const binary = options.binary ?? "grok"

  const runCommand =
    options.runCommand ??
    (async (argv, cwd, requestedTimeoutMs) => {
      const started = Date.now()
      const timeoutMs = boundedWorkerTimeout(requestedTimeoutMs)
      const proc = spawn(argv[0]!, argv.slice(1), {
        cwd,
        env: options.env ?? process.env,
        stdio: ["ignore", "pipe", "pipe"],
      })
      let stdout = ""
      let stderr = ""
      let outputBytes = 0
      let outputExceeded = false
      let timedOut = false
      let forceKillTimer: ReturnType<typeof setTimeout> | undefined
      const terminate = () => {
        try {
          proc.kill("SIGTERM")
        } catch {
          // The process already exited.
        }
        forceKillTimer ??= setTimeout(() => {
          try {
            proc.kill("SIGKILL")
          } catch {
            // The process already exited.
          }
        }, GROK_READINESS_FORCE_KILL_GRACE_MS)
      }
      const append = (current: string, chunk: unknown): string => {
        const bytes = Buffer.from(String(chunk))
        outputBytes += bytes.byteLength
        if (outputBytes > MAX_GROK_WORKER_OUTPUT_BYTES) {
          outputExceeded = true
          terminate()
          return current
        }
        return current + bytes.toString("utf8")
      }
      proc.stdout.on("data", (c) => {
        stdout = append(stdout, c)
      })
      proc.stderr.on("data", (c) => {
        stderr = append(stderr, c)
      })
      const timeout = setTimeout(() => {
        timedOut = true
        terminate()
      }, timeoutMs)
      const code: number = await new Promise((resolve) => {
        proc.on("close", (c) => resolve(c ?? 1))
        proc.on("error", () => resolve(127))
      })
      clearTimeout(timeout)
      if (forceKillTimer !== undefined) clearTimeout(forceKillTimer)
      return {
        code: timedOut || outputExceeded ? 1 : code,
        stdout: outputExceeded ? "" : stdout,
        stderr: outputExceeded ? "Grok worker output exceeded its bound." : stderr,
        wallClockMs: Date.now() - started,
        timedOut,
      }
    })

  const usageFor = (
    input: Omit<GrokClaimedWorkInput, "sessionId">,
    wallClockMs: number,
  ): GrokUsageSnapshot => {
    const plane = input.plane ?? (options.env?.XAI_API_KEY ? "api_key" : "cli_session")
    const marginalCostClass: MarginalCostClass =
      input.marginalCostClass ?? (plane === "cli_session" ? "free" : "api_metered")
    return {
      metering: "not_measured",
      wallClockMs,
      ...(input.model === undefined ? {} : { model: input.model }),
      plane,
      marginalCostClass,
    }
  }

  const invalidSessionCloseout = (
    input: Omit<GrokClaimedWorkInput, "sessionId">,
  ): WorkerCloseout => ({
    ok: false,
    claimRef: input.pin.claimRef,
    stopReason: "invalid_session_id",
    text: "",
    usage: usageFor(input, 0),
    failureClass: "unknown",
  })

  const runTurn = async (
    input: Omit<GrokClaimedWorkInput, "sessionId">,
    sessionArgs: readonly string[],
  ): Promise<WorkerCloseout> => {
    const systemPin = [
      `claimRef=${input.pin.claimRef}`,
      `workUnitRef=${input.pin.workUnitRef}`,
      `runRef=${input.pin.runRef}`,
      input.pin.accountRefHash ? `accountRefHash=${input.pin.accountRefHash}` : null,
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
      // This is an owner-local, isolated claimed-work executor with no stdin.
      // Without explicit unattended approval Grok can answer successfully
      // while declining every edit/tool prompt, producing a false clean run.
      "--always-approve",
      ...sessionArgs,
      "-p",
      prompt,
      "--cwd",
      input.pin.cwd,
      "--output-format",
      "plain",
      ...(input.model ? ["-m", input.model] : []),
    ]

    let result: Awaited<ReturnType<typeof runCommand>>
    try {
      result = await runCommand(argv, input.pin.cwd, boundedWorkerTimeout(input.timeoutMs))
    } catch {
      result = {
        code: 1,
        stdout: "",
        stderr: "",
        wallClockMs: 0,
      }
    }
    const combined = `${result.stdout}\n${result.stderr}`
    const failureClass =
      result.code === 0
        ? undefined
        : result.timedOut === true
          ? "timeout"
          : classifyError(combined, result.code)

    return {
      ok: result.code === 0 && workerOutputIsBounded(result.stdout) && workerOutputIsBounded(result.stderr),
      claimRef: input.pin.claimRef,
      stopReason: result.code === 0 ? "end_turn" : result.timedOut === true ? "timeout" : `exit_${result.code}`,
      text: workerOutputIsBounded(result.stdout) ? result.stdout.trim() : "",
      usage: usageFor(input, result.wallClockMs),
      ...(failureClass ? { failureClass } : {}),
    }
  }

  return {
    kind: "grok_cli",
    async readiness() {
      return probeGrokReadiness({
        binary,
        ...(options.env === undefined ? {} : { env: options.env }),
      })
    },
    async runClaimedWork(input) {
      if (input.sessionId !== undefined && !isCanonicalUuid(input.sessionId)) {
        return invalidSessionCloseout(input)
      }
      return await runTurn(
        input,
        input.sessionId === undefined ? [] : ["--session-id", input.sessionId],
      )
    },
    async runFollowUp(input) {
      if (!isCanonicalUuid(input.sessionId)) return invalidSessionCloseout(input)
      return await runTurn(input, ["--resume", input.sessionId])
    },
  }
}
