import { createHash } from "node:crypto"
import { isAbsolute, resolve } from "node:path"
import type {
  ForgeDispatchVerificationCommand,
  ForgeDispatchWorkItem,
} from "@openagentsinc/forge-protocol"

export const FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF =
  "forge.verification.runner.docker_bun.v0.1"

export type ForgeDockerVerificationLimits = Readonly<{
  cpus?: number
  memory?: string
  pidsLimit?: number
  timeoutSeconds?: number
  tmpfsSize?: string
  homeTmpfsSize?: string
}>

export type ResolvedForgeDockerVerificationLimits = Readonly<{
  cpus: number
  memory: string
  pidsLimit: number
  timeoutSeconds: number
  tmpfsSize: string
  homeTmpfsSize: string
}>

export type ForgeDockerVerificationCommandPlan = Readonly<{
  runnerRef: typeof FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF
  commandRef: string
  verificationRef: string
  workspaceRef: string
  imageRef: string
  argsRef: string
  dockerArgs: string[]
  timeoutMs: number
  workingDirectory: string
  limits: ResolvedForgeDockerVerificationLimits
}>

export type ForgeDockerCommandRunner = (input: {
  args: string[]
  cwd: string
  timeoutMs: number
}) => Promise<{
  exitCode: number | null
  stdout: string
  stderr: string
  timedOut: boolean
}>

export type ForgeDockerVerificationResult = Readonly<{
  schema: "openagents.forge.verification.docker_bun.result.v0.1"
  runnerRef: typeof FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF
  commandRef: string
  verificationRef: string
  workspaceRef: string
  imageRef: string
  argsRef: string
  status: "passed" | "failed" | "timed_out" | "error"
  exitCode: number | null
  stdoutBytes: number
  stderrBytes: number
  stdoutDigestRef: string | null
  stderrDigestRef: string | null
  network: "none"
  readOnlyRootFilesystem: true
  workspaceMountReadOnly: true
  cpus: number
  memory: string
  pidsLimit: number
  timeoutSeconds: number
  redacted: true
  observedAt: string
  completedAt: string
}>

export type PlanForgeDockerVerificationInput = Readonly<{
  workspacePath: string
  command: ForgeDispatchVerificationCommand
  image?: string
  dockerBinary?: string
  limits?: ForgeDockerVerificationLimits
}>

export type RunForgeDockerVerificationInput = PlanForgeDockerVerificationInput &
  Readonly<{
    runner?: ForgeDockerCommandRunner
    now?: () => Date
  }>

const DEFAULT_IMAGE = "oven/bun:1.3.11"
const DEFAULT_LIMITS: ResolvedForgeDockerVerificationLimits = {
  cpus: 1,
  memory: "2g",
  pidsLimit: 256,
  timeoutSeconds: 30 * 60,
  tmpfsSize: "256m",
  homeTmpfsSize: "64m",
}

const allowedBunSubcommands = new Set(["test", "run", "--version"])
const memoryLimitPattern = /^[1-9][0-9]*(m|g)$/i
const dockerImagePattern =
  /^[A-Za-z0-9][A-Za-z0-9_.:/@-]{0,191}[A-Za-z0-9]$/

const stableRef = (prefix: string, input: string): string =>
  `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`

const digestRef = (prefix: string, input: string): string | null =>
  input.length === 0 ? null : stableRef(prefix, input)

const byteLength = (input: string): number =>
  new TextEncoder().encode(input).byteLength

const cleanDockerBinary = (value: string | undefined): string => {
  const binary = value?.trim() || "docker"
  if (
    binary.length === 0 ||
    binary.includes("\0") ||
    binary.includes("/") ||
    binary.includes("\\") ||
    /\s/.test(binary)
  ) {
    throw new Error("Forge Docker verification docker binary must be a command name")
  }
  return binary
}

const cleanImage = (value: string | undefined): string => {
  const image = value?.trim() || DEFAULT_IMAGE
  if (!dockerImagePattern.test(image)) {
    throw new Error("Forge Docker verification image must be a bounded image ref")
  }
  return image
}

const cleanWorkspacePath = (workspacePath: string): string => {
  if (!isAbsolute(workspacePath)) {
    throw new Error("Forge Docker verification workspace path must be absolute")
  }
  if (workspacePath.includes("\0") || workspacePath.includes(":")) {
    throw new Error("Forge Docker verification workspace path contains unsafe characters")
  }
  return resolve(workspacePath)
}

const cleanWorkingDirectory = (value: string): string => {
  const raw = value.trim()
  if (raw.length === 0 || raw === ".") return "."
  const normalized = raw.replaceAll("\\", "/")
  if (normalized.startsWith("/") || normalized.includes("\0")) {
    throw new Error("Forge Docker verification working directory must be relative")
  }
  const segments = normalized.split("/").filter(Boolean)
  if (
    segments.length === 0 ||
    segments.some((segment) => segment === "." || segment === ".." || segment.length > 120)
  ) {
    throw new Error("Forge Docker verification working directory escapes the workspace")
  }
  return segments.join("/")
}

const cleanCommandArgs = (args: readonly string[]): string[] => {
  if (args.length < 2 || args.length > 32) {
    throw new Error("Forge Docker verification requires a bounded bun argv")
  }
  const cleaned = args.map((arg) => {
    if (
      typeof arg !== "string" ||
      arg.length === 0 ||
      arg.length > 240 ||
      arg.includes("\0") ||
      arg.startsWith("/") ||
      /(^|[/\\])\.\.($|[/\\])/.test(arg)
    ) {
      throw new Error("Forge Docker verification argv contains an unsafe token")
    }
    return arg
  })
  if (cleaned[0] !== "bun" || !allowedBunSubcommands.has(cleaned[1])) {
    throw new Error("Forge Docker verification only accepts bun test/run argv")
  }
  return cleaned
}

const positiveIntegerLimit = (
  value: number | undefined,
  fallback: number,
  field: string,
  max: number,
): number => {
  const candidate = value ?? fallback
  if (!Number.isInteger(candidate) || candidate < 1 || candidate > max) {
    throw new Error(`Forge Docker verification ${field} limit is out of range`)
  }
  return candidate
}

const positiveCpuLimit = (value: number | undefined): number => {
  const candidate = value ?? DEFAULT_LIMITS.cpus
  if (!Number.isFinite(candidate) || candidate <= 0 || candidate > 8) {
    throw new Error("Forge Docker verification cpu limit is out of range")
  }
  return candidate
}

const memoryLimit = (value: string | undefined, fallback: string, field: string): string => {
  const candidate = value ?? fallback
  if (!memoryLimitPattern.test(candidate)) {
    throw new Error(`Forge Docker verification ${field} must be an m/g size`)
  }
  return candidate.toLowerCase()
}

const resolveLimits = (
  command: ForgeDispatchVerificationCommand,
  limits: ForgeDockerVerificationLimits | undefined,
): ResolvedForgeDockerVerificationLimits => ({
  cpus: positiveCpuLimit(limits?.cpus),
  memory: memoryLimit(limits?.memory, DEFAULT_LIMITS.memory, "memory"),
  pidsLimit: positiveIntegerLimit(
    limits?.pidsLimit,
    DEFAULT_LIMITS.pidsLimit,
    "pids",
    2048,
  ),
  timeoutSeconds: positiveIntegerLimit(
    limits?.timeoutSeconds ?? command.timeout_seconds,
    DEFAULT_LIMITS.timeoutSeconds,
    "timeout",
    7200,
  ),
  tmpfsSize: memoryLimit(limits?.tmpfsSize, DEFAULT_LIMITS.tmpfsSize, "tmpfs size"),
  homeTmpfsSize: memoryLimit(
    limits?.homeTmpfsSize,
    DEFAULT_LIMITS.homeTmpfsSize,
    "home tmpfs size",
  ),
})

export const planForgeDockerVerificationCommand = (
  input: PlanForgeDockerVerificationInput,
): ForgeDockerVerificationCommandPlan => {
  if (input.command.runner_ref !== FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF) {
    throw new Error("Forge Docker verification command targets a different runner")
  }

  const dockerBinary = cleanDockerBinary(input.dockerBinary)
  const image = cleanImage(input.image)
  const workspacePath = cleanWorkspacePath(input.workspacePath)
  const workingDirectory = cleanWorkingDirectory(input.command.working_directory)
  const args = cleanCommandArgs(input.command.args)
  const limits = resolveLimits(input.command, input.limits)
  const workdir =
    workingDirectory === "." ? "/workspace" : `/workspace/${workingDirectory}`

  const dockerArgs = [
    dockerBinary,
    "run",
    "--rm",
    "--pull=never",
    "--network",
    "none",
    "--cpus",
    String(limits.cpus),
    "--memory",
    limits.memory,
    "--memory-swap",
    limits.memory,
    "--pids-limit",
    String(limits.pidsLimit),
    "--read-only",
    "--cap-drop",
    "ALL",
    "--security-opt",
    "no-new-privileges",
    "--tmpfs",
    `/tmp:rw,noexec,nosuid,nodev,size=${limits.tmpfsSize}`,
    "--tmpfs",
    `/home/bun:rw,noexec,nosuid,nodev,size=${limits.homeTmpfsSize}`,
    "--env",
    "HOME=/home/bun",
    "--env",
    "BUN_INSTALL_CACHE_DIR=/tmp/bun-cache",
    "--mount",
    `type=bind,src=${workspacePath},dst=/workspace,readonly`,
    "--workdir",
    workdir,
    "--user",
    "1000:1000",
    image,
    ...args,
  ]

  return {
    runnerRef: FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF,
    commandRef: input.command.command_ref,
    verificationRef: stableRef(
      "verification.forge.docker_bun",
      `${workspacePath}\0${workingDirectory}\0${args.join("\0")}`,
    ),
    workspaceRef: stableRef("workspace.forge.verify", workspacePath),
    imageRef: stableRef("image.forge.verify", image),
    argsRef: stableRef("command.argv", args.join("\0")),
    dockerArgs,
    timeoutMs: limits.timeoutSeconds * 1000,
    workingDirectory,
    limits,
  }
}

export const planForgeDockerVerificationForWorkItem = (
  input: Omit<PlanForgeDockerVerificationInput, "command"> &
    Readonly<{ item: ForgeDispatchWorkItem }>,
): ForgeDockerVerificationCommandPlan => {
  if (input.item.verification_command === null) {
    throw new Error("Forge dispatch work item has no verification command")
  }
  return planForgeDockerVerificationCommand({
    workspacePath: input.workspacePath,
    command: input.item.verification_command,
    image: input.image,
    dockerBinary: input.dockerBinary,
    limits: input.limits,
  })
}

const defaultDockerCommandRunner: ForgeDockerCommandRunner = async (input) => {
  let timedOut = false
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    const proc = Bun.spawn(input.args, {
      cwd: input.cwd,
      stderr: "pipe",
      stdout: "pipe",
    })
    timer = setTimeout(() => {
      timedOut = true
      try {
        proc.kill()
      } catch {
        // The process may already have exited between the timeout and kill.
      }
    }, input.timeoutMs)
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ])
    return { exitCode, stdout, stderr, timedOut }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    return { exitCode: null, stdout: "", stderr: message, timedOut }
  } finally {
    if (timer !== undefined) clearTimeout(timer)
  }
}

export const runForgeDockerVerification = async (
  input: RunForgeDockerVerificationInput,
): Promise<ForgeDockerVerificationResult> => {
  const plan = planForgeDockerVerificationCommand(input)
  const now = input.now ?? (() => new Date())
  const observedAt = now().toISOString()
  const runner = input.runner ?? defaultDockerCommandRunner
  const commandResult = await runner({
    args: plan.dockerArgs,
    cwd: cleanWorkspacePath(input.workspacePath),
    timeoutMs: plan.timeoutMs,
  }).catch((error: unknown) => ({
    exitCode: null,
    stdout: "",
    stderr: error instanceof Error ? error.message : String(error),
    timedOut: false,
  }))
  const completedAt = now().toISOString()
  const status =
    commandResult.timedOut
      ? "timed_out"
      : commandResult.exitCode === null
        ? "error"
        : commandResult.exitCode === 0
          ? "passed"
          : "failed"

  return {
    schema: "openagents.forge.verification.docker_bun.result.v0.1",
    runnerRef: plan.runnerRef,
    commandRef: plan.commandRef,
    verificationRef: plan.verificationRef,
    workspaceRef: plan.workspaceRef,
    imageRef: plan.imageRef,
    argsRef: plan.argsRef,
    status,
    exitCode: commandResult.exitCode,
    stdoutBytes: byteLength(commandResult.stdout),
    stderrBytes: byteLength(commandResult.stderr),
    stdoutDigestRef: digestRef("verification.stdout", commandResult.stdout),
    stderrDigestRef: digestRef("verification.stderr", commandResult.stderr),
    network: "none",
    readOnlyRootFilesystem: true,
    workspaceMountReadOnly: true,
    cpus: plan.limits.cpus,
    memory: plan.limits.memory,
    pidsLimit: plan.limits.pidsLimit,
    timeoutSeconds: plan.limits.timeoutSeconds,
    redacted: true,
    observedAt,
    completedAt,
  }
}
