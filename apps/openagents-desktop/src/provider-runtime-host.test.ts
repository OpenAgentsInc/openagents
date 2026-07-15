import { describe, expect, test } from "vite-plus/test"
import { PassThrough } from "node:stream"
import { readFileSync } from "node:fs"
import path from "node:path"

import {
  executableOutsideAsar,
  discoverBundledCodexRuntime,
  inspectProviderRuntimeCompatibility,
  makeCodexRuntimeAuthority,
  publicCodexRuntimeProjection,
  readInstalledClaudeAgentSdkVersion,
  resolveBundledClaudeExecutable,
  resolveBundledCodexExecutable,
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

const bundledCodex = "/checkout/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex"
const bundledOptions = {
  platform: "darwin" as const,
  arch: "arm64",
  resolveFromDependencyGraph: () => bundledCodex,
  exists: (value: string) => value === bundledCodex,
  isFile: () => true,
  isExecutable: () => true,
  hasExpectedArchitecture: () => true,
  sha256: () => "29915529b97697def1a957b0505e770aa6a45744435d62fc263e98d7619e167a",
}

describe("provider runtime host", () => {
  test("the clean checkout resolves the pinned native Codex and Claude SDK packages", () => {
    expect(resolveBundledCodexExecutable()).toContain("@openai+codex")
    expect(resolveBundledClaudeExecutable()).toContain("@anthropic-ai+claude-agent-sdk-darwin-arm64")
    expect(readInstalledClaudeAgentSdkVersion()).toBe("0.3.172")
  })

  test("the installed app resolves the exact package-owned unpacked Claude binary", () => {
    const resolved = resolveBundledClaudeExecutable({
      resourcesPath: "/Applications/OpenAgents.app/Contents/Resources",
      exists: value => value.endsWith("/app.asar.unpacked/node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"),
    })
    expect(resolved).toBe(
      "/Applications/OpenAgents.app/Contents/Resources/app.asar.unpacked/node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude",
    )
    expect(resolved).not.toContain("/app.asar/")
  })

  test("the installed app resolves the exact package-owned unpacked Codex binary", () => {
    const resolved = resolveBundledCodexExecutable({
      platform: "darwin",
      arch: "arm64",
      resolveFromDependencyGraph: () => null,
      resourcesPath: "/Applications/OpenAgents.app/Contents/Resources",
      exists: value => value.includes("/app.asar.unpacked/node_modules/@openai/codex-darwin-arm64/vendor/"),
      isFile: () => true,
      isExecutable: () => true,
      hasExpectedArchitecture: () => true,
      sha256: () => "29915529b97697def1a957b0505e770aa6a45744435d62fc263e98d7619e167a",
    })
    expect(resolved).toBe(
      "/Applications/OpenAgents.app/Contents/Resources/app.asar.unpacked/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex",
    )
    expect(resolved).not.toContain("/.codex")
  })

  test("an Electron virtual ASAR executable is translated before spawn", () => {
    const virtual = "/Applications/OpenAgents.app/Contents/Resources/app.asar/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex"
    const unpacked = virtual.replace("/app.asar/", "/app.asar.unpacked/")
    expect(executableOutsideAsar(virtual, value => value === unpacked)).toBe(unpacked)
    expect(executableOutsideAsar(virtual, () => false)).toBeNull()
    expect(executableOutsideAsar("/checkout/node_modules/codex", value => value.startsWith("/checkout")))
      .toBe("/checkout/node_modules/codex")
  })

  test("concurrent observations return redacted compatible facts", async () => {
    const result = await inspectProviderRuntimeCompatibility({
      ...bundledOptions,
      spawnVersion: () => child("codex-cli 0.144.1\n"),
      readClaudeVersion: () => "0.3.172",
    })
    expect(result.map(item => item.state)).toEqual(["compatible", "compatible"])
    expect(JSON.stringify(result)).not.toContain("/private")
  })

  test("missing, malformed, incompatible, crash, and update states fail closed", async () => {
    expect((await inspectProviderRuntimeCompatibility({ platform: "darwin", arch: "arm64", resolveFromDependencyGraph: () => null, resourcesPath: null, readClaudeVersion: () => null }))[0]?.state).toBe("missing")
    expect((await inspectProviderRuntimeCompatibility({ ...bundledOptions, spawnVersion: () => child("garbage"), readClaudeVersion: () => "0.4.0" })).map(item => item.state)).toEqual(["malformed", "incompatible"])
    expect((await inspectProviderRuntimeCompatibility({ ...bundledOptions, spawnVersion: () => child("", 1), readClaudeVersion: () => "0.3.172" }))[0]?.state).toBe("malformed")
  })

  test("a hung version process is killed and settles malformed", async () => {
    const hung = child("codex-cli 0.144.1", 0, 100)
    const result = await inspectProviderRuntimeCompatibility({
      ...bundledOptions,
      spawnVersion: () => hung,
      readClaudeVersion: () => "0.3.172",
      timeoutMs: 5,
    })
    expect(result[0]?.state).toBe("malformed")
    expect(hung.kills()).toBe(1)
  })

  test("classifies package, target, file, executable, and architecture failures", () => {
    const base = { ...bundledOptions }
    expect(discoverBundledCodexRuntime({ ...base, resolveFromDependencyGraph: () => null, resourcesPath: null }).state).toBe("missing_package")
    expect(discoverBundledCodexRuntime({ ...base, resolveFromDependencyGraph: () => "/tmp/codex", exists: () => true }).state).toBe("wrong_target")
    expect(discoverBundledCodexRuntime({ ...base, isFile: () => false }).state).toBe("not_file")
    expect(discoverBundledCodexRuntime({ ...base, isExecutable: () => false }).state).toBe("not_executable")
    expect(discoverBundledCodexRuntime({ ...base, hasExpectedArchitecture: () => false }).state).toBe("wrong_architecture")
    expect(discoverBundledCodexRuntime({ ...base, sha256: () => "unreviewed" }).state).toBe("unverified_binary")
    expect(discoverBundledCodexRuntime({ ...base, platform: "aix" }).state).toBe("unsupported_target")
  })

  test("pins one absolute bundled identity across PATH and NVM drift", async () => {
    let resolutions = 0
    const authority = makeCodexRuntimeAuthority({
      ...bundledOptions,
      resolveFromDependencyGraph: () => {
        resolutions++
        return bundledCodex
      },
      spawnVersion: executable => {
        expect(executable).toBe(bundledCodex)
        return child("codex-cli 0.144.1\n")
      },
    })
    const before = process.env.PATH
    try {
      process.env.PATH = "/Users/example/.nvm/versions/node/v1/bin:/tmp/global-codex"
      expect(authority.executable()).toBe(bundledCodex)
      process.env.PATH = "/Users/example/.nvm/versions/node/v99/bin"
      expect(authority.executable()).toBe(bundledCodex)
      expect((await authority.inspect()).state).toBe("ready")
      expect((await authority.inspect()).state).toBe("ready")
      expect(resolutions).toBe(1)
    } finally {
      process.env.PATH = before
    }
  })

  test("public projection is bounded and excludes private paths and raw output", async () => {
    const resolution = await makeCodexRuntimeAuthority({
      ...bundledOptions,
      spawnVersion: () => child("corrupt output from /Users/private/.nvm/bin/codex"),
    }).inspect()
    const serialized = JSON.stringify(publicCodexRuntimeProjection(resolution))
    expect(serialized).not.toContain("/Users/private")
    expect(serialized).not.toContain("executablePath")
    expect(serialized.length).toBeLessThan(400)
  })

  test("every production Codex consumer uses the single authority and never a bare binary", () => {
    const root = import.meta.dirname
    for (const file of ["codex-child-runtime.ts", "codex-connect.ts", "main.ts"]) {
      const source = readFileSync(path.join(root, file), "utf8")
      expect(source).not.toContain("resolveBundledCodexExecutable")
      expect(source).not.toMatch(/spawn\(\s*["']codex["']/u)
      expect(source).toContain("codexRuntimeAuthority")
    }
    const main = readFileSync(path.join(root, "main.ts"), "utf8")
    expect(main).toContain("never probes the ambient Claude/OpenCode maintenance catalog")
    expect(main).toContain("const codexResolution = await codexRuntimeAuthority.inspect()")
    expect(main).toContain('harness: "codex" as const')
    expect(main).not.toContain("collectHarnessMaintenanceStatus({}, ambientHarnesses)")
    expect(main).not.toContain("runHarnessMaintenanceUpdate")
    expect(main).toContain("await desktopUpdateHost.check()")
  })
})
