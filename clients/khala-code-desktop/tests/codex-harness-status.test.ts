import { EventEmitter } from "node:events"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "bun:test"

import { inspectCodexHarnessStatus } from "../src/bun/codex-harness-status"

type FakeChild = EventEmitter & {
  readonly stdout: EventEmitter
  readonly stderr: EventEmitter
  readonly kill: () => void
}

const tempDirs: string[] = []
const fixedNow = () => new Date("2026-07-01T15:00:00.000Z")

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempCodexHome(authJson?: string): Promise<string> {
  const home = await mkdtemp(join(tmpdir(), "khala-code-codex-home-"))
  tempDirs.push(home)
  if (authJson !== undefined) await writeFile(join(home, "auth.json"), authJson)
  return home
}

function versionChild(input: {
  readonly stdout?: string
  readonly stderr?: string
  readonly closeCode?: number
  readonly error?: NodeJS.ErrnoException
} = {}): FakeChild {
  const child = new EventEmitter() as FakeChild
  Object.assign(child, {
    stdout: new EventEmitter(),
    stderr: new EventEmitter(),
    kill: () => undefined,
  })
  queueMicrotask(() => {
    if (input.error !== undefined) {
      child.emit("error", input.error)
      return
    }
    if (input.stdout !== undefined) child.stdout.emit("data", Buffer.from(input.stdout))
    if (input.stderr !== undefined) child.stderr.emit("data", Buffer.from(input.stderr))
    child.emit("close", input.closeCode ?? 0, null)
  })
  return child
}

describe("Codex harness status", () => {
  test("reports a missing Codex binary before claiming the harness is ready", async () => {
    const missing = new Error("spawn codex ENOENT") as NodeJS.ErrnoException
    missing.code = "ENOENT"

    const status = await inspectCodexHarnessStatus({
      codexHomePath: await tempCodexHome(JSON.stringify({
        tokens: { access_token: "access-token" },
      })),
      now: fixedNow,
      spawnFn: () => versionChild({ error: missing }),
    })

    expect(status).toMatchObject({
      available: false,
      capability: "codex_harness",
      status: "unavailable",
      binary: {
        command: "codex",
        source: "PATH",
        available: false,
        error: "Codex CLI not found",
      },
      signIn: {
        command: "codex login",
      },
    })
    expect(status.reason).toContain("npm install -g @openai/codex")
    expect(status.reason).toContain("KHALA_CODE_CODEX_BINARY/KHALA_CODE_CODEX_COMMAND")
  })

  test("reports missing auth in the primary user Codex home", async () => {
    const home = await tempCodexHome()
    const status = await inspectCodexHarnessStatus({
      codexHomePath: home,
      now: fixedNow,
      spawnFn: () => versionChild({ stdout: "codex-cli 0.99.0\n" }),
    })

    expect(status).toMatchObject({
      available: false,
      status: "unavailable",
      binary: {
        available: true,
        version: "codex-cli 0.99.0",
      },
      home: {
        path: home,
        source: "input",
        role: "main_user_codex_home",
        authPath: join(home, "auth.json"),
        fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
      },
      auth: {
        state: "credentials_missing",
        blockerRefs: ["blocker.codex.credentials_missing"],
      },
      signIn: {
        required: true,
      },
    })
    expect(status.reason).toContain("primary user Codex home")
    expect(status.reason).toContain("before using Khala Code chat")
    expect(status.signIn.warning).toContain("isolated Pylon worker homes")
  })

  test("does not treat malformed auth.json as a usable login", async () => {
    const home = await tempCodexHome("{not-json")
    const status = await inspectCodexHarnessStatus({
      codexHomePath: home,
      now: fixedNow,
      spawnFn: () => versionChild({ stdout: "codex-cli 0.99.0\n" }),
    })

    expect(status).toMatchObject({
      available: false,
      status: "unavailable",
      auth: {
        state: "invalid",
        blockerRefs: ["blocker.codex.auth_json_invalid"],
        error: "Codex auth.json is not valid JSON.",
      },
    })
  })

  test("reports an explicit ready Codex binary and CODEX_HOME without exposing tokens", async () => {
    const home = await tempCodexHome(JSON.stringify({
      tokens: {
        access_token: "secret-access-token",
        refresh_token: "secret-refresh-token",
        account_id: "account-123",
      },
    }))
    const calls: [string, readonly string[]][] = []
    const status = await inspectCodexHarnessStatus({
      env: {
        CODEX_HOME: home,
        KHALA_CODE_CODEX_BINARY: "/opt/codex/bin/codex",
      } as NodeJS.ProcessEnv,
      now: fixedNow,
      spawnFn: (command, args) => {
        calls.push([command, args])
        return versionChild({ stdout: "codex-cli 1.2.3\n" })
      },
    })

    expect(calls).toEqual([["/opt/codex/bin/codex", ["--version"]]])
    expect(status).toMatchObject({
      available: true,
      observedAt: "2026-07-01T15:00:00.000Z",
      status: "ready",
      binary: {
        command: "/opt/codex/bin/codex",
        source: "env:KHALA_CODE_CODEX_BINARY",
        available: true,
        version: "codex-cli 1.2.3",
        error: null,
      },
      home: {
        path: home,
        source: "env:CODEX_HOME",
        role: "main_user_codex_home",
      },
      auth: {
        state: "ready",
        accessTokenPresent: true,
        refreshTokenPresent: true,
        accountIdPresent: true,
        blockerRefs: [],
      },
      signIn: {
        required: false,
      },
    })
    expect(JSON.stringify(status)).not.toContain("secret-access-token")
    expect(JSON.stringify(status)).not.toContain("secret-refresh-token")
  })
})
