import { describe, expect, test } from "bun:test"
import { PassThrough } from "node:stream"

import {
  executableOutsideAsar,
  inspectProviderRuntimeCompatibility,
  readInstalledClaudeAgentSdkVersion,
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

describe("provider runtime host", () => {
  test("the clean checkout resolves the pinned native Codex and Claude SDK packages", () => {
    expect(resolveBundledCodexExecutable()).toContain("@openai+codex")
    expect(readInstalledClaudeAgentSdkVersion()).toBe("0.3.172")
  })

  test("the installed app resolves the exact package-owned unpacked Codex binary", () => {
    const resolved = resolveBundledCodexExecutable({
      resourcesPath: "/Applications/OpenAgents.app/Contents/Resources",
      exists: value => value.includes("/app.asar.unpacked/node_modules/@openai/codex-darwin-arm64/vendor/"),
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
      resolveCodex: () => "/private/bundled/codex",
      spawnVersion: () => child("codex-cli 0.144.1\n"),
      readClaudeVersion: () => "0.3.172",
    })
    expect(result.map(item => item.state)).toEqual(["compatible", "compatible"])
    expect(JSON.stringify(result)).not.toContain("/private")
  })

  test("missing, malformed, incompatible, crash, and update states fail closed", async () => {
    expect((await inspectProviderRuntimeCompatibility({ resolveCodex: () => null, readClaudeVersion: () => null }))[0]?.state).toBe("missing")
    expect((await inspectProviderRuntimeCompatibility({ resolveCodex: () => "codex", spawnVersion: () => child("garbage"), readClaudeVersion: () => "0.4.0" })).map(item => item.state)).toEqual(["malformed", "incompatible"])
    expect((await inspectProviderRuntimeCompatibility({ resolveCodex: () => "codex", spawnVersion: () => child("", 1), readClaudeVersion: () => "0.3.172" }))[0]?.state).toBe("malformed")
  })

  test("a hung version process is killed and settles malformed", async () => {
    const hung = child("codex-cli 0.144.1", 0, 100)
    const result = await inspectProviderRuntimeCompatibility({
      resolveCodex: () => "codex",
      spawnVersion: () => hung,
      readClaudeVersion: () => "0.3.172",
      timeoutMs: 5,
    })
    expect(result[0]?.state).toBe("malformed")
    expect(hung.kills()).toBe(1)
  })
})
