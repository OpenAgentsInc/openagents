import { describe, expect, test } from "vite-plus/test"
import { PassThrough } from "node:stream"
import { readFileSync } from "node:fs"
import path from "node:path"

import {
  discoverInstalledCodexRuntime,
  inspectProviderRuntimeCompatibility,
  makeCodexRuntimeAuthority,
  publicCodexRuntimeProjection,
  readInstalledClaudeAgentSdkVersion,
  resolveBundledClaudeExecutable,
  resolveInstalledCodexExecutable,
} from "./provider-runtime-host.ts"

const child = (output: string, code = 0, delay = 0) => {
  const stdout = new PassThrough()
  const listeners = new Map<string, Array<(...args: unknown[]) => void>>()
  let kills = 0
  const value = {
    stdout,
    on: (event: "close" | "error", listener: (...args: unknown[]) => void) => {
      listeners.set(event, [...(listeners.get(event) ?? []), listener])
      return value
    },
    kill: () => { kills++; return true },
    kills: () => kills,
  }
  setTimeout(() => {
    stdout.write(output)
    stdout.end()
    for (const listener of listeners.get("close") ?? []) listener(code)
  }, delay)
  return value
}

const installedCodex = "/Users/example/.local/bin/codex"
const installedOptions = {
  platform: "darwin" as const,
  arch: "arm64",
  candidatePaths: [installedCodex],
  exists: (value: string) => value === installedCodex,
  isFile: () => true,
  isExecutable: () => true,
  hasExpectedArchitecture: () => true,
  canonicalize: (value: string) => value,
}

describe("provider runtime host", () => {
  test("resolves the user's documented standalone Codex install and bundled Claude SDK", () => {
    expect(resolveInstalledCodexExecutable(installedOptions)).toBe(installedCodex)
    expect(resolveBundledClaudeExecutable()).toContain("@anthropic-ai+claude-agent-sdk-darwin-arm64")
    expect(readInstalledClaudeAgentSdkVersion()).toBe("0.3.172")
  })

  test("finds Codex without a login-shell PATH on macOS", () => {
    const candidates: string[] = []
    const result = discoverInstalledCodexRuntime({
      platform: "darwin",
      arch: "arm64",
      homeDir: "/Users/example",
      env: { PATH: "/usr/bin:/bin" },
      exists: value => { candidates.push(value); return value === "/Applications/ChatGPT.app/Contents/Resources/codex" },
      isFile: () => true,
      isExecutable: () => true,
      hasExpectedArchitecture: () => true,
      canonicalize: value => value,
    })
    expect(result).toMatchObject({ state: "candidate", source: "chatgpt_app" })
    expect(result.executablePath).toBe("/Applications/ChatGPT.app/Contents/Resources/codex")
    expect(candidates[0]).toBe("/Users/example/.local/bin/codex")
  })

  test("uses the documented Windows standalone install location", () => {
    const expected = "C:\\Users\\Example\\AppData\\Local\\Programs\\OpenAI\\Codex\\bin\\codex.exe"
    const result = discoverInstalledCodexRuntime({
      platform: "win32",
      arch: "x64",
      env: { LOCALAPPDATA: "C:\\Users\\Example\\AppData\\Local", PATH: "" },
      exists: value => value === expected,
      isFile: () => true,
      isExecutable: () => true,
      canonicalize: value => value,
    })
    expect(result).toMatchObject({ state: "candidate", source: "standalone_install", executablePath: expected })
  })

  test("supports current installed Codex versions including prereleases", async () => {
    const result = await inspectProviderRuntimeCompatibility({
      ...installedOptions,
      spawnVersion: () => child("codex-cli 0.145.0-alpha.18\n"),
      readClaudeVersion: () => "0.3.172",
    })
    expect(result.map(item => item.state)).toEqual(["compatible", "compatible"])
    expect(result[0]).toMatchObject({ expectedVersion: ">=0.144.1", observedVersion: "0.145.0-alpha.18" })
  })

  test("missing, malformed, incompatible, crash, and timeout states fail closed", async () => {
    const missing = { ...installedOptions, exists: () => false }
    expect((await inspectProviderRuntimeCompatibility({ ...missing, readClaudeVersion: () => null }))[0]?.state).toBe("missing")
    expect((await inspectProviderRuntimeCompatibility({ ...installedOptions, spawnVersion: () => child("garbage"), readClaudeVersion: () => "0.4.0" })).map(item => item.state)).toEqual(["malformed", "incompatible"])
    expect((await inspectProviderRuntimeCompatibility({ ...installedOptions, spawnVersion: () => child("", 1), readClaudeVersion: () => "0.3.172" }))[0]?.state).toBe("malformed")
    const hung = child("codex-cli 0.145.0", 0, 100)
    expect((await inspectProviderRuntimeCompatibility({ ...installedOptions, spawnVersion: () => hung, readClaudeVersion: () => "0.3.172", timeoutMs: 5 }))[0]?.state).toBe("malformed")
    expect(hung.kills()).toBe(1)
  })

  test("rejects absent, non-file, non-executable, wrong-architecture, and unsupported candidates", () => {
    expect(discoverInstalledCodexRuntime({ ...installedOptions, exists: () => false }).state).toBe("missing_install")
    expect(discoverInstalledCodexRuntime({ ...installedOptions, isFile: () => false }).state).toBe("not_file")
    expect(discoverInstalledCodexRuntime({ ...installedOptions, isExecutable: () => false }).state).toBe("not_executable")
    expect(discoverInstalledCodexRuntime({ ...installedOptions, hasExpectedArchitecture: () => false }).state).toBe("wrong_architecture")
    expect(discoverInstalledCodexRuntime({ ...installedOptions, platform: "aix" }).state).toBe("unsupported_target")
  })

  test("pins one absolute installed identity across PATH drift", async () => {
    let canonicalizations = 0
    const authority = makeCodexRuntimeAuthority({
      ...installedOptions,
      canonicalize: value => { canonicalizations++; return value },
      spawnVersion: executable => {
        expect(executable).toBe(installedCodex)
        return child("codex-cli 0.145.0-alpha.18\n")
      },
    })
    const before = process.env.PATH
    try {
      process.env.PATH = "/tmp/global-codex"
      expect(authority.executable()).toBe(installedCodex)
      process.env.PATH = "/tmp/other-codex"
      expect((await authority.inspect()).state).toBe("ready")
      expect((await authority.inspect()).state).toBe("ready")
      expect(canonicalizations).toBe(1)
    } finally {
      process.env.PATH = before
    }
  })

  test("public projection is bounded and excludes private paths and raw output", async () => {
    const resolution = await makeCodexRuntimeAuthority({
      ...installedOptions,
      spawnVersion: () => child("corrupt output from /Users/private/bin/codex"),
    }).inspect()
    const serialized = JSON.stringify(publicCodexRuntimeProjection(resolution))
    expect(serialized).not.toContain("/Users/private")
    expect(serialized).not.toContain("executablePath")
    expect(serialized.length).toBeLessThan(450)
  })

  test("every production Codex consumer uses the single authority and never a bare binary", () => {
    const root = import.meta.dirname
    for (const file of ["codex-child-runtime.ts", "codex-connect.ts", "main.ts"]) {
      const source = readFileSync(path.join(root, file), "utf8")
      expect(source).not.toMatch(/spawn\(\s*["']codex["']/u)
      expect(source).toContain("codexRuntimeAuthority")
    }
  })
})
