import { spawn } from "node:child_process"
import { accessSync, constants, existsSync, readFileSync, realpathSync, statSync } from "node:fs"
import { createRequire } from "node:module"
import { homedir } from "node:os"
import path from "node:path"

import {
  classifyProviderRuntimeCompatibility,
  supportedProviderRuntimeVersions,
  type ProviderRuntimeCompatibility,
} from "./provider-runtime-compatibility.ts"

const require = createRequire(import.meta.url)

export type CodexRuntimeTarget = Readonly<{
  platform: NodeJS.Platform
  arch: string
  executable: string
}>

export const codexRuntimeTarget = (
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): CodexRuntimeTarget | null => {
  const key = `${platform}:${arch}`
  const targets: Record<string, Readonly<{ executable: string }>> = {
    "darwin:arm64": { executable: "codex" },
    "darwin:x64": { executable: "codex" },
    "linux:arm64": { executable: "codex" },
    "linux:x64": { executable: "codex" },
    "win32:arm64": { executable: "codex.exe" },
    "win32:x64": { executable: "codex.exe" },
  }
  const target = targets[key]
  return target === undefined ? null : { platform, arch, ...target }
}

export type InstalledCodexResolutionOptions = Readonly<{
  resourcesPath?: string | null
  env?: NodeJS.ProcessEnv
  homeDir?: string
  candidatePaths?: ReadonlyArray<string>
  exists?: (value: string) => boolean
  isFile?: (value: string) => boolean
  isExecutable?: (value: string) => boolean
  hasExpectedArchitecture?: (value: string, target: CodexRuntimeTarget) => boolean
  canonicalize?: (value: string) => string
  platform?: NodeJS.Platform
  arch?: string
}>

export const codexRuntimeStates = [
  "ready",
  "unsupported_target",
  "missing_install",
  "wrong_architecture",
  "not_file",
  "not_executable",
  "spawn_failed",
  "timeout",
  "malformed_version",
  "incompatible_version",
] as const
export type CodexRuntimeState = (typeof codexRuntimeStates)[number]
export type CodexRuntimeSource = "standalone_install" | "chatgpt_app" | "path"

export type CodexRuntimeResolution = Readonly<{
  state: CodexRuntimeState
  source: CodexRuntimeSource | null
  target: CodexRuntimeTarget | null
  /** Main-process private; renderer projections must omit it. */
  executablePath: string | null
  expectedVersion: string
  observedVersion: string | null
  capabilities: ReadonlyArray<"exec_json" | "app_server" | "device_auth">
  recovery: "none" | "install_or_update_codex"
}>

export type CodexRuntimeCandidate = Readonly<{
  state: "candidate" | "unsupported_target" | "missing_install" | "wrong_architecture" | "not_file" | "not_executable"
  source: CodexRuntimeSource | null
  target: CodexRuntimeTarget | null
  executablePath: string | null
}>

const regularFileByDefault = (value: string): boolean => {
  try {
    return statSync(value).isFile()
  } catch {
    return false
  }
}

const executableByDefault = (value: string): boolean => {
  if (process.platform === "win32") return true
  try { accessSync(value, constants.X_OK); return true } catch { return false }
}

const expectedMacCpuType = (arch: string): number | null =>
  arch === "arm64" ? 0x0100000c : arch === "x64" ? 0x01000007 : null

/** Header-only Mach-O validation: no shell, PATH, `file`, or version manager. */
const expectedArchitectureByDefault = (value: string, target: CodexRuntimeTarget): boolean => {
  if (target.platform !== "darwin") return true
  const expected = expectedMacCpuType(target.arch)
  if (expected === null) return false
  try {
    const header = readFileSync(value).subarray(0, 4096)
    if (header.length < 8) return false
    const magic = header.readUInt32BE(0)
    if (magic === 0xfeedface || magic === 0xfeedfacf) return header.readUInt32BE(4) === expected
    if (magic === 0xcefaedfe || magic === 0xcffaedfe) return header.readUInt32LE(4) === expected
    const fat64 = magic === 0xcafebabf
    if (magic === 0xcafebabe || fat64) {
      const count = Math.min(header.readUInt32BE(4), 32)
      const stride = fat64 ? 32 : 20
      for (let index = 0; index < count; index += 1) {
        const offset = 8 + index * stride
        if (offset + 4 <= header.length && header.readUInt32BE(offset) === expected) return true
      }
    }
    return false
  } catch {
    return false
  }
}

const installedCodexCandidates = (
  target: CodexRuntimeTarget,
  options: InstalledCodexResolutionOptions,
): ReadonlyArray<Readonly<{ path: string; source: CodexRuntimeSource }>> => {
  if (options.candidatePaths !== undefined) {
    return options.candidatePaths.map(value => ({ path: value, source: "path" as const }))
  }
  const env = options.env ?? process.env
  const home = options.homeDir ?? homedir()
  const paths = target.platform === "win32" ? path.win32 : path.posix
  const candidates: Array<Readonly<{ path: string; source: CodexRuntimeSource }>> = []
  if (target.platform === "win32") {
    const localAppData = env.LOCALAPPDATA
    if (localAppData !== undefined && localAppData !== "") {
      candidates.push({
        path: paths.join(localAppData, "Programs", "OpenAI", "Codex", "bin", target.executable),
        source: "standalone_install",
      })
    }
  } else {
    candidates.push({ path: paths.join(home, ".local", "bin", target.executable), source: "standalone_install" })
    if (target.platform === "darwin") {
      candidates.push({ path: "/Applications/ChatGPT.app/Contents/Resources/codex", source: "chatgpt_app" })
      candidates.push({ path: paths.join(home, "Applications", "ChatGPT.app", "Contents", "Resources", "codex"), source: "chatgpt_app" })
      candidates.push({ path: "/opt/homebrew/bin/codex", source: "path" })
    }
    candidates.push({ path: "/usr/local/bin/codex", source: "path" })
    candidates.push({ path: "/usr/bin/codex", source: "path" })
  }
  const delimiter = target.platform === "win32" ? ";" : ":"
  for (const directory of (env.PATH ?? "").split(delimiter)) {
    if (directory !== "" && paths.isAbsolute(directory)) {
      candidates.push({ path: paths.join(directory, target.executable), source: "path" })
    }
  }
  const seen = new Set<string>()
  return candidates.filter(candidate => !seen.has(candidate.path) && seen.add(candidate.path))
}

export const discoverInstalledCodexRuntime = (
  options: InstalledCodexResolutionOptions = {},
): CodexRuntimeCandidate => {
  const target = codexRuntimeTarget(options.platform, options.arch)
  if (target === null) return { state: "unsupported_target", source: null, target: null, executablePath: null }
  const exists = options.exists ?? existsSync
  const isFile = options.isFile ?? regularFileByDefault
  const isExecutable = options.isExecutable ?? executableByDefault
  const hasExpectedArchitecture = options.hasExpectedArchitecture ?? expectedArchitectureByDefault
  const canonicalize = options.canonicalize ?? (value => realpathSync(value))
  const targetPaths = target.platform === "win32" ? path.win32 : path.posix
  let firstFailure: CodexRuntimeCandidate | null = null
  for (const candidate of installedCodexCandidates(target, options)) {
    if (!targetPaths.isAbsolute(candidate.path) || !exists(candidate.path)) continue
    const executablePath = canonicalize(candidate.path)
    const failure = !isFile(executablePath)
      ? "not_file" as const
      : !isExecutable(executablePath)
        ? "not_executable" as const
        : !hasExpectedArchitecture(executablePath, target)
          ? "wrong_architecture" as const
          : null
    if (failure !== null) {
      firstFailure ??= { state: failure, source: candidate.source, target, executablePath }
      continue
    }
    return { state: "candidate", source: candidate.source, target, executablePath }
  }
  return firstFailure ?? { state: "missing_install", source: null, target, executablePath: null }
}

export const executableOutsideAsar = (
  candidate: string,
  exists: (value: string) => boolean = existsSync,
): string | null => {
  const asarSegment = `${path.sep}app.asar${path.sep}`
  if (candidate.includes(asarSegment)) {
    const unpacked = candidate.replace(
      asarSegment,
      `${path.sep}app.asar.unpacked${path.sep}`,
    )
    return exists(unpacked) ? unpacked : null
  }
  return exists(candidate) ? candidate : null
}

/**
 * Resolves the user's existing Codex installation. OpenAgents never packages,
 * copies, re-signs, or substitutes this executable. Discovery is bounded to
 * the documented standalone location, the official ChatGPT app resource, and
 * absolute directories already present in the launch environment's PATH.
 */
export const resolveInstalledCodexExecutable = (
  options: InstalledCodexResolutionOptions = {},
): string | null => {
  const candidate = discoverInstalledCodexRuntime(options)
  return candidate.state === "candidate" ? candidate.executablePath : null
}

/**
 * Resolves the native Claude executable owned by the pinned Agent SDK. The
 * SDK's own resolver can see a virtual file inside Electron's ASAR and then
 * pass that virtual path to spawn(), which fails with ENOTDIR. Resolve from
 * the SDK's dependency graph and translate the result to app.asar.unpacked
 * before a local Claude turn starts.
 */
export const resolveBundledClaudeExecutable = (
  options: InstalledCodexResolutionOptions = {},
): string | null => {
  const exists = options.exists ?? existsSync
  const packageName = `@anthropic-ai/claude-agent-sdk-${process.platform}-${process.arch}`
  const executableName = process.platform === "win32" ? "claude.exe" : "claude"
  try {
    const sdkEntry = require.resolve("@anthropic-ai/claude-agent-sdk")
    const sdkRequire = createRequire(sdkEntry)
    const executable = sdkRequire.resolve(`${packageName}/${executableName}`)
    const executablePath = executableOutsideAsar(executable, exists)
    if (executablePath !== null) return executablePath
  } catch { /* packaged fallback below */ }
  const resourcesPath = options.resourcesPath ?? (
    process as NodeJS.Process & { resourcesPath?: string }
  ).resourcesPath
  if (typeof resourcesPath !== "string" || resourcesPath.length === 0) return null
  const unpacked = path.join(
    resourcesPath,
    "app.asar.unpacked",
    "node_modules",
    packageName,
    executableName,
  )
  return exists(unpacked) ? unpacked : null
}

export const readInstalledClaudeAgentSdkVersion = (): string | null => {
  try {
    const entry = require.resolve("@anthropic-ai/claude-agent-sdk")
    const manifest = JSON.parse(readFileSync(path.join(path.dirname(entry), "package.json"), "utf8")) as { version?: unknown }
    return typeof manifest.version === "string" ? manifest.version : null
  } catch {
    return null
  }
}

type VersionChild = Readonly<{
  stdout: NodeJS.ReadableStream | null
  on: (event: "close" | "error", listener: (...args: unknown[]) => void) => unknown
  kill: (signal?: NodeJS.Signals) => boolean
}>

export type CodexRuntimeAuthorityOptions = InstalledCodexResolutionOptions & Readonly<{
  readClaudeVersion?: () => string | null
  spawnVersion?: (executable: string) => VersionChild | null
  timeoutMs?: number
}>

export type ProviderRuntimeHostOptions = CodexRuntimeAuthorityOptions

const defaultSpawnVersion = (executable: string): VersionChild | null => {
  try {
    const env: NodeJS.ProcessEnv = {
      PATH: process.platform === "darwin" ? "/usr/bin:/bin:/usr/sbin:/sbin" : process.env.PATH,
      SYSTEMROOT: process.env.SYSTEMROOT,
    }
    return spawn(executable, ["--version"], {
      stdio: ["ignore", "pipe", "ignore"],
      env,
    }) as unknown as VersionChild
  } catch {
    return null
  }
}

const resolutionFromCandidate = (candidate: CodexRuntimeCandidate): CodexRuntimeResolution => ({
  state: candidate.state === "candidate" ? "spawn_failed" : candidate.state,
  source: candidate.source,
  target: candidate.target,
  executablePath: candidate.executablePath,
  expectedVersion: supportedProviderRuntimeVersions.codex_cli,
  observedVersion: null,
  capabilities: [],
  recovery: "install_or_update_codex",
})

const probeCodexRuntime = (
  candidate: CodexRuntimeCandidate,
  options: CodexRuntimeAuthorityOptions,
): Promise<CodexRuntimeResolution> => new Promise(resolve => {
    if (candidate.state !== "candidate" || candidate.executablePath === null) {
      resolve(resolutionFromCandidate(candidate))
      return
    }
    const child = (options.spawnVersion ?? defaultSpawnVersion)(candidate.executablePath)
    if (child === null) {
      resolve(resolutionFromCandidate(candidate))
      return
    }
    let output = ""
    let settled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    const finish = (state: CodexRuntimeState, observedVersion: string | null = null): void => {
      if (settled) return
      settled = true
      if (timer !== undefined) clearTimeout(timer)
      resolve({
        state,
        source: candidate.source,
        target: candidate.target,
        executablePath: candidate.executablePath,
        expectedVersion: supportedProviderRuntimeVersions.codex_cli,
        observedVersion,
        capabilities: state === "ready" ? ["exec_json", "app_server", "device_auth"] : [],
        recovery: state === "ready" ? "none" : "install_or_update_codex",
      })
    }
    child.stdout?.on("data", (chunk: Buffer | string) => {
      if (output.length < 200) output += (typeof chunk === "string" ? chunk : chunk.toString("utf8")).slice(0, 200 - output.length)
    })
    child.on("error", () => finish("spawn_failed"))
    child.on("close", (...args) => {
      if (args[0] !== 0) {
        finish("spawn_failed")
        return
      }
      const compatibility = classifyProviderRuntimeCompatibility("codex_cli", output)
      if (compatibility.state === "malformed") finish("malformed_version")
      else if (compatibility.state === "incompatible") finish("incompatible_version", compatibility.observedVersion)
      else if (compatibility.state === "compatible") finish("ready", compatibility.observedVersion)
      else finish("malformed_version")
    })
    timer = setTimeout(() => {
      child.kill("SIGTERM")
      finish("timeout")
    }, options.timeoutMs ?? 5_000)
  })

export type CodexRuntimeAuthority = Readonly<{
  executable: () => string | null
  inspect: () => Promise<CodexRuntimeResolution>
}>

/**
 * One process-lifetime user-installed Codex identity. Discovery runs exactly
 * once so a PATH or version-manager change cannot swap the executable beneath
 * an active Desktop process.
 */
export const makeCodexRuntimeAuthority = (
  options: CodexRuntimeAuthorityOptions = {},
): CodexRuntimeAuthority => {
  const candidate = discoverInstalledCodexRuntime(options)
  let inspection: Promise<CodexRuntimeResolution> | null = null
  return Object.freeze({
    executable: () => candidate.state === "candidate" ? candidate.executablePath : null,
    inspect: () => inspection ??= probeCodexRuntime(candidate, options),
  })
}

export const codexRuntimeAuthority = makeCodexRuntimeAuthority()

export type PublicCodexRuntimeProjection = Readonly<{
  state: CodexRuntimeState
  provenance: "standalone_install" | "chatgpt_app" | "path" | "unavailable"
  expectedVersion: string
  observedVersion: string | null
  compatible: boolean
  capabilities: ReadonlyArray<"exec_json" | "app_server" | "device_auth">
  recoveryMessage: string | null
}>

export const publicCodexRuntimeProjection = (
  resolution: CodexRuntimeResolution,
): PublicCodexRuntimeProjection => ({
  state: resolution.state,
  provenance: resolution.source ?? "unavailable",
  expectedVersion: resolution.expectedVersion,
  observedVersion: resolution.observedVersion,
  compatible: resolution.state === "ready",
  capabilities: resolution.capabilities,
  recoveryMessage: resolution.state === "ready"
    ? null
    : "Install or update Codex, then restart OpenAgents. Your existing Codex sign-in is reused.",
})

export const inspectProviderRuntimeCompatibility = async (
  options: ProviderRuntimeHostOptions = {},
): Promise<ReadonlyArray<ProviderRuntimeCompatibility>> => {
  const authority = Object.keys(options).length === 0
    ? codexRuntimeAuthority
    : makeCodexRuntimeAuthority(options)
  const [codexResolution, claudeVersion] = await Promise.all([
    authority.inspect(),
    Promise.resolve((options.readClaudeVersion ?? readInstalledClaudeAgentSdkVersion)()),
  ])
  const codexCompatibility = codexResolution.state === "ready"
    ? classifyProviderRuntimeCompatibility("codex_cli", `codex-cli ${codexResolution.observedVersion}`)
    : codexResolution.state === "incompatible_version"
      ? classifyProviderRuntimeCompatibility("codex_cli", `codex-cli ${codexResolution.observedVersion}`)
      : codexResolution.state === "unsupported_target" || codexResolution.state === "missing_install" || codexResolution.state === "wrong_architecture" || codexResolution.state === "not_file" || codexResolution.state === "not_executable"
        ? classifyProviderRuntimeCompatibility("codex_cli", null)
        : classifyProviderRuntimeCompatibility("codex_cli", "")
  return [
    codexCompatibility,
    classifyProviderRuntimeCompatibility("claude_agent_sdk", claudeVersion),
  ]
}
