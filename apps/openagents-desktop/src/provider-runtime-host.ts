import { spawn } from "node:child_process"
import { existsSync, readFileSync } from "node:fs"
import { createRequire } from "node:module"
import path from "node:path"

import {
  classifyProviderRuntimeCompatibility,
  type ProviderRuntimeCompatibility,
} from "./provider-runtime-compatibility.ts"

const require = createRequire(import.meta.url)

const codexTarget = (): Readonly<{ packageName: string; triple: string; executable: string }> | null => {
  const key = `${process.platform}:${process.arch}`
  const targets: Record<string, Readonly<{ packageName: string; triple: string; executable: string }>> = {
    "darwin:arm64": { packageName: "@openai/codex-darwin-arm64", triple: "aarch64-apple-darwin", executable: "codex" },
    "darwin:x64": { packageName: "@openai/codex-darwin-x64", triple: "x86_64-apple-darwin", executable: "codex" },
    "linux:arm64": { packageName: "@openai/codex-linux-arm64", triple: "aarch64-unknown-linux-musl", executable: "codex" },
    "linux:x64": { packageName: "@openai/codex-linux-x64", triple: "x86_64-unknown-linux-musl", executable: "codex" },
    "win32:arm64": { packageName: "@openai/codex-win32-arm64", triple: "aarch64-pc-windows-msvc", executable: "codex.exe" },
    "win32:x64": { packageName: "@openai/codex-win32-x64", triple: "x86_64-pc-windows-msvc", executable: "codex.exe" },
  }
  return targets[key] ?? null
}

/** Resolves only the optional native package owned by the pinned Codex dependency. */
export const resolveBundledCodexExecutable = (): string | null => {
  const target = codexTarget()
  if (target === null) return null
  try {
    const codexEntrypoint = require.resolve("@openai/codex/bin/codex.js")
    const codexRequire = createRequire(codexEntrypoint)
    const packageJson = codexRequire.resolve(`${target.packageName}/package.json`)
    const executable = path.join(path.dirname(packageJson), "vendor", target.triple, "bin", target.executable)
    return existsSync(executable) ? executable : null
  } catch {
    return null
  }
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

export type ProviderRuntimeHostOptions = Readonly<{
  resolveCodex?: () => string | null
  readClaudeVersion?: () => string | null
  spawnVersion?: (executable: string) => VersionChild | null
  timeoutMs?: number
}>

const defaultSpawnVersion = (executable: string): VersionChild | null => {
  try {
    return spawn(executable, ["--version"], {
      stdio: ["ignore", "pipe", "ignore"],
      env: {
        PATH: process.env.PATH,
        SYSTEMROOT: process.env.SYSTEMROOT,
      },
    }) as unknown as VersionChild
  } catch {
    return null
  }
}

const observeCodexVersion = (options: ProviderRuntimeHostOptions): Promise<string | null> =>
  new Promise(resolve => {
    const executable = (options.resolveCodex ?? resolveBundledCodexExecutable)()
    if (executable === null) { resolve(null); return }
    const child = (options.spawnVersion ?? defaultSpawnVersion)(executable)
    if (child === null) { resolve(""); return }
    let output = ""
    let settled = false
    const finish = (value: string | null): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve(value)
    }
    child.stdout?.on("data", (chunk: Buffer | string) => {
      if (output.length < 200) output += (typeof chunk === "string" ? chunk : chunk.toString("utf8")).slice(0, 200 - output.length)
    })
    child.on("error", () => finish(""))
    child.on("close", (...args) => finish(args[0] === 0 ? output : ""))
    const timer = setTimeout(() => {
      child.kill("SIGTERM")
      finish("")
    }, options.timeoutMs ?? 5_000)
  })

export const inspectProviderRuntimeCompatibility = async (
  options: ProviderRuntimeHostOptions = {},
): Promise<ReadonlyArray<ProviderRuntimeCompatibility>> => {
  const [codexVersion, claudeVersion] = await Promise.all([
    observeCodexVersion(options),
    Promise.resolve((options.readClaudeVersion ?? readInstalledClaudeAgentSdkVersion)()),
  ])
  return [
    classifyProviderRuntimeCompatibility("codex_cli", codexVersion),
    classifyProviderRuntimeCompatibility("claude_agent_sdk", claudeVersion),
  ]
}
