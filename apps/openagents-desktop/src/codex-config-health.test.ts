import { EventEmitter } from "node:events"
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { PassThrough } from "node:stream"
import { describe, expect, test } from "vite-plus/test"

import { checkCodexConfiguration, parseCodexConfigurationIssue } from "./codex-config-health.ts"
import type { CodexChildSpawn } from "./codex-child-runtime.ts"

const fixtureSpawn = (responses: ReadonlyArray<Readonly<{ exitCode: number; stderr?: string }>>): CodexChildSpawn => {
  let index = 0
  return () => {
    const response = responses[index++] ?? responses.at(-1)!
    const child = new EventEmitter() as EventEmitter & { stdout: PassThrough; stderr: PassThrough; killed: boolean; kill: () => boolean }
    child.stdout = new PassThrough()
    child.stderr = new PassThrough()
    child.killed = false
    child.kill = () => { child.killed = true; return true }
    queueMicrotask(() => {
      if (response.stderr !== undefined) child.stderr.write(response.stderr)
      child.emit("close", response.exitCode)
    })
    return child
  }
}

describe("Codex configuration health", () => {
  test("parses the exact file location and parser message", () => {
    expect(parseCodexConfigurationIssue("failed to load configuration: /Users/me/.codex/config.toml:408:1: invalid transport")).toEqual({
      path: "/Users/me/.codex/config.toml", line: 408, column: 1, message: "invalid transport",
    })
  })

  test("backs up and removes only an inert disabled MCP stanza, then verifies the repair", async () => {
    const root = mkdtempSync(join(tmpdir(), "codex-config-health-"))
    const file = join(root, "config.toml")
    writeFileSync(file, "model = \"gpt\"\n\n[mcp_servers.broken]\nenabled = false\n\n[mcp_servers.good]\ncommand = \"node\"\n")
    const health = await checkCodexConfiguration({
      spawn: fixtureSpawn([
        { exitCode: 1, stderr: `failed to load configuration: ${file}:3:1: invalid transport\n` },
        { exitCode: 0 },
      ]),
      env: {}, cwd: root, now: () => new Date("2026-07-15T10:00:00.000Z"),
    })
    expect(health.state).toBe("repaired")
    expect(readFileSync(file, "utf8")).not.toContain("mcp_servers.broken")
    expect(readFileSync(file, "utf8")).toContain("mcp_servers.good")
    if (health.state === "repaired") expect(readFileSync(health.backupPath, "utf8")).toContain("mcp_servers.broken")
  })

  test("never rewrites an ambiguous broken stanza", async () => {
    const root = mkdtempSync(join(tmpdir(), "codex-config-health-"))
    const file = join(root, "config.toml")
    const source = "[mcp_servers.broken]\nenabled = false\nurl = \"not-valid\"\n"
    writeFileSync(file, source)
    const health = await checkCodexConfiguration({
      spawn: fixtureSpawn([{ exitCode: 1, stderr: `failed to load configuration: ${file}:1:1: invalid transport\n` }]),
      env: {}, cwd: root,
    })
    expect(health.state).toBe("invalid")
    expect(readFileSync(file, "utf8")).toBe(source)
  })
})
