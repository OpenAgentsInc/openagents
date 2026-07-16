import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { afterEach, describe, expect, test } from "vite-plus/test"

import {
  DESKTOP_CODEX_USAGE_OUTBOX_SCHEMA,
  openDesktopCodexUsageOutbox,
} from "./desktop-codex-usage-outbox.ts"

const directories: Array<string> = []
const fixture = () => {
  const directory = mkdtempSync(path.join(tmpdir(), "oa-usage-outbox-contract-"))
  directories.push(directory)
  return { directory, file: path.join(directory, "usage", "codex-outbox.json") }
}

afterEach(() => {
  for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true })
})

describe("openDesktopCodexUsageOutbox", () => {
  test("persists a credential-free completed report privately and reopens it", () => {
    const { file } = fixture()
    const now = () => new Date("2026-07-16T14:00:00.000Z")
    const outbox = openDesktopCodexUsageOutbox(file, now)

    outbox.recordAdmission({
      admissionRef: "admission-1",
      turnRef: "turn-1",
      model: "gpt-5.6-sol",
      admittedAt: now().toISOString(),
      expiresAt: "2026-07-17T14:00:00.000Z",
    })
    expect(outbox.complete("turn-1", {
      observedAt: "2026-07-16T14:01:00.000Z",
      usage: {
        inputTokens: 10,
        cachedInputTokens: 2,
        outputTokens: 5,
        reasoningTokens: 1,
        totalTokens: 15,
      },
    })).toBe(true)

    const disk = readFileSync(file, "utf8")
    expect(JSON.parse(disk)).toMatchObject({ schema: DESKTOP_CODEX_USAGE_OUTBOX_SCHEMA })
    expect(disk).not.toMatch(/credential|access.?token|refresh.?token|owner|prompt|response|workspace|path/i)
    if (process.platform !== "win32") {
      expect(statSync(file).mode & 0o777).toBe(0o600)
      expect(statSync(path.dirname(file)).mode & 0o777).toBe(0o700)
    }
    expect(openDesktopCodexUsageOutbox(file, now).due()).toHaveLength(1)
  })

  test("backs off retries and drops expired admissions on persistence", () => {
    const { file } = fixture()
    let current = new Date("2026-07-16T14:00:00.000Z")
    const outbox = openDesktopCodexUsageOutbox(file, () => current)
    outbox.recordAdmission({
      admissionRef: "admission-1",
      turnRef: "turn-1",
      model: "gpt-5.6-sol",
      admittedAt: current.toISOString(),
      expiresAt: "2026-07-16T14:02:00.000Z",
    })
    outbox.complete("turn-1", {
      observedAt: current.toISOString(),
      usage: { inputTokens: 1, cachedInputTokens: 0, outputTokens: 1, reasoningTokens: 0, totalTokens: 2 },
    })
    outbox.retry("admission-1")
    expect(outbox.due()).toEqual([])

    current = new Date("2026-07-16T14:00:30.000Z")
    expect(outbox.due()).toHaveLength(1)
    current = new Date("2026-07-16T14:03:00.000Z")
    outbox.retry("admission-1")
    expect(outbox.snapshot()).toEqual([])
  })

  test("quarantines malformed state and never blocks startup", () => {
    const { file } = fixture()
    const usageDirectory = path.dirname(file)
    mkdirSync(usageDirectory, { recursive: true })
    // Exercise an existing overly-permissive mode too; the next valid write repairs it.
    writeFileSync(file, "not-json")
    chmodSync(file, 0o644)

    const outbox = openDesktopCodexUsageOutbox(file)
    expect(outbox.snapshot()).toEqual([])
    expect(readdirSync(usageDirectory).some(name => name.startsWith("codex-outbox.json.quarantined-"))).toBe(true)
    outbox.clear()
    if (process.platform !== "win32") expect(statSync(file).mode & 0o777).toBe(0o600)
  })
})
