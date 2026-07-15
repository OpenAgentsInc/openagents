import { spawn } from "node:child_process"
import { accessSync, constants, existsSync, readFileSync, statSync } from "node:fs"
import { createRequire } from "node:module"
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
  packageName: string
  triple: string
  executable: string
}>

export const codexRuntimeTarget = (
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): CodexRuntimeTarget | null => {
  const key = `${platform}:${arch}`
  const targets: Record<string, Readonly<{ packageName: string; triple: string; executable: string }>> = {
    "darwin:arm64": { packageName: "@openai/codex-darwin-arm64", triple: "aarch64-apple-darwin", executable: "codex" },
    "darwin:x64": { packageName: "@openai/codex-darwin-x64", triple: "x86_64-apple-darwin", executable: "codex" },
    "linux:arm64": { packageName: "@openai/codex-linux-arm64", triple: "aarch64-unknown-linux-musl", executable: "codex" },
    "linux:x64": { packageName: "@openai/codex-linux-x64", triple: "x86_64-unknown-linux-musl", executable: "codex" },
    "win32:arm64": { packageName: "@openai/codex-win32-arm64", triple: "aarch64-pc-windows-msvc", executable: "codex.exe" },
    "win32:x64": { packageName: "@openai/codex-win32-x64", triple: "x86_64-pc-windows-msvc", executable: "codex.exe" },
  }
  const target = targets[key]
  return target === undefined ? null : { platform, arch, ...target }
}

export type BundledCodexResolutionOptions = Readonly<{
  resourcesPath?: string | null
  exists?: (value: string) => boolean
  isFile?: (value: string) => boolean
  isExecutable?: (value: string) => boolean
  hasExpectedArchitecture?: (value: string, target: CodexRuntimeTarget) => boolean
  platform?: NodeJS.Platform
  arch?: string
  /** Test seam for dependency-graph resolution; never an ambient PATH lookup. */
  resolveFromDependencyGraph?: (target: CodexRuntimeTarget) => string | null
}>

export const codexRuntimeStates = [
  "ready",
  "unsupported_target",
  "missing_package",
  "wrong_target",
  "wrong_architecture",
  "not_file",
  "not_executable",
  "spawn_failed",
  "timeout",
  "malformed_version",
  "incompatible_version",
] as const
export type CodexRuntimeState = (typeof codexRuntimeStates)[number]
export type CodexRuntimeSource = "dependency_graph" | "packaged_unpacked"

export type CodexRuntimeResolution = Readonly<{
  state: CodexRuntimeState
  source: CodexRuntimeSource | null
  target: CodexRuntimeTarget | null
  /** Main-process private; renderer projections must omit it. */
  executablePath: string | null
  expectedVersion: string
  observedVersion: string | null
  capabilities: ReadonlyArray<"exec_json" | "app_server" | "device_auth">
  recovery: "none" | "repair_openagents"
}>

export type CodexRuntimeCandidate = Readonly<{
  state: "candidate" | "unsupported_target" | "missing_package" | "wrong_target" | "wrong_architecture" | "not_file" | "not_executable"
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

const resolveCodexFromDependencyGraph = (target: CodexRuntimeTarget): string | null => {
  try {
    const codexEntrypoint = require.resolve("@openai/codex/bin/codex.js")
    const codexRequire = createRequire(codexEntrypoint)
    const packageJson = codexRequire.resolve(`${target.packageName}/package.json`)
    return path.join(path.dirname(packageJson), "vendor", target.triple, "bin", target.executable)
  } catch {
    return null
  }
}

const candidateMatchesTarget = (
  candidate: string,
  target: CodexRuntimeTarget,
  source: CodexRuntimeSource,
): boolean => {
  const normalized = candidate.split(path.sep).join("/")
  if (!path.isAbsolute(candidate)) return false
  const aliasLayout = normalized.includes(`/node_modules/${target.packageName}/vendor/${target.triple}/bin/${target.executable}`)
  if (source === "packaged_unpacked" || aliasLayout) return aliasLayout
  // pnpm materializes npm aliases under a target-qualified store directory,
  // while the package's own manifest name remains `@openai/codex`.
  return normalized.includes(`-${target.platform}-${target.arch}/node_modules/@openai/codex/vendor/${target.triple}/bin/${target.executable}`)
}

export const discoverBundledCodexRuntime = (
  options: BundledCodexResolutionOptions = {},
): CodexRuntimeCandidate => {
  const target = codexRuntimeTarget(options.platform, options.arch)
  if (target === null) return { state: "unsupported_target", source: null, target: null, executablePath: null }
  const exists = options.exists ?? existsSync
  const isFile = options.isFile ?? regularFileByDefault
  const isExecutable = options.isExecutable ?? executableByDefault
  const hasExpectedArchitecture = options.hasExpectedArchitecture ?? expectedArchitectureByDefault
  const dependencyCandidate = (options.resolveFromDependencyGraph ?? resolveCodexFromDependencyGraph)(target)
  if (dependencyCandidate !== null) {
    const executablePath = executableOutsideAsar(dependencyCandidate, exists)
    if (executablePath !== null) {
      const source = dependencyCandidate.includes(`${path.sep}app.asar${path.sep}`)
        ? "packaged_unpacked" as const
        : "dependency_graph" as const
      if (!candidateMatchesTarget(executablePath, target, source)) return { state: "wrong_target", source, target, executablePath }
      if (!isFile(executablePath)) return { state: "not_file", source, target, executablePath }
      if (!isExecutable(executablePath)) return { state: "not_executable", source, target, executablePath }
      if (!hasExpectedArchitecture(executablePath, target)) return { state: "wrong_architecture", source, target, executablePath }
      return { state: "candidate", source, target, executablePath }
    }
  }
  const resourcesPath = options.resourcesPath ?? (
    process as NodeJS.Process & { resourcesPath?: string }
  ).resourcesPath
  if (typeof resourcesPath !== "string" || resourcesPath.length === 0) {
    return { state: "missing_package", source: null, target, executablePath: null }
  }
  const executablePath = path.join(
    resourcesPath,
    "app.asar.unpacked",
    "node_modules",
    target.packageName,
    "vendor",
    target.triple,
    "bin",
    target.executable,
  )
  if (!exists(executablePath)) return { state: "missing_package", source: "packaged_unpacked", target, executablePath: null }
  if (!candidateMatchesTarget(executablePath, target, "packaged_unpacked")) return { state: "wrong_target", source: "packaged_unpacked", target, executablePath }
  if (!isFile(executablePath)) return { state: "not_file", source: "packaged_unpacked", target, executablePath }
  if (!isExecutable(executablePath)) return { state: "not_executable", source: "packaged_unpacked", target, executablePath }
  if (!hasExpectedArchitecture(executablePath, target)) return { state: "wrong_architecture", source: "packaged_unpacked", target, executablePath }
  return { state: "candidate", source: "packaged_unpacked", target, executablePath }
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
 * Resolves only the optional native package owned by the pinned Codex
 * dependency. Forge moves native packages out of `app.asar`; package
 * resolution is the development path and the exact `app.asar.unpacked`
 * package location is the installed path. Ambient PATH is never consulted.
 */
export const resolveBundledCodexExecutable = (
  options: BundledCodexResolutionOptions = {},
): string | null => {
  const candidate = discoverBundledCodexRuntime(options)
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
  options: BundledCodexResolutionOptions = {},
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

export type CodexRuntimeAuthorityOptions = BundledCodexResolutionOptions & Readonly<{
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
  recovery: "repair_openagents",
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
        recovery: state === "ready" ? "none" : "repair_openagents",
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
 * One process-lifetime package-owned Codex identity. Discovery runs exactly
 * once; ambient PATH/NVM changes cannot alter any subsequent consumer.
 */
export const makeCodexRuntimeAuthority = (
  options: CodexRuntimeAuthorityOptions = {},
): CodexRuntimeAuthority => {
  const candidate = discoverBundledCodexRuntime(options)
  let inspection: Promise<CodexRuntimeResolution> | null = null
  return Object.freeze({
    executable: () => candidate.state === "candidate" ? candidate.executablePath : null,
    inspect: () => inspection ??= probeCodexRuntime(candidate, options),
  })
}

export const codexRuntimeAuthority = makeCodexRuntimeAuthority()

export type PublicCodexRuntimeProjection = Readonly<{
  state: CodexRuntimeState
  provenance: "bundled_dependency" | "bundled_installed" | "unavailable"
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
  provenance: resolution.source === "dependency_graph"
    ? "bundled_dependency"
    : resolution.source === "packaged_unpacked" ? "bundled_installed" : "unavailable",
  expectedVersion: resolution.expectedVersion,
  observedVersion: resolution.observedVersion,
  compatible: resolution.state === "ready",
  capabilities: resolution.capabilities,
  recoveryMessage: resolution.state === "ready"
    ? null
    : "Repair or update OpenAgents to restore its bundled Codex runtime.",
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
      : codexResolution.state === "unsupported_target" || codexResolution.state === "missing_package" || codexResolution.state === "wrong_target" || codexResolution.state === "wrong_architecture" || codexResolution.state === "not_file" || codexResolution.state === "not_executable"
        ? classifyProviderRuntimeCompatibility("codex_cli", null)
        : classifyProviderRuntimeCompatibility("codex_cli", "")
  return [
    codexCompatibility,
    classifyProviderRuntimeCompatibility("claude_agent_sdk", claudeVersion),
  ]
}
