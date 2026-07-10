import { describe, expect, test } from "bun:test"
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  createGrokHeadlessWorkerExecutor,
  grokConfiguredApiCredentialPresent,
  probeGrokReadiness,
} from "./worker-executor.ts"

describe("GrokHeadlessWorkerExecutor", () => {
  test("readiness is bounded, preserves isolated GROK_HOME, and fails closed", async () => {
    const calls: Array<{ argv: readonly string[]; home: string | undefined; timeoutMs: number }> = []
    const ready = await probeGrokReadiness({
      env: { GROK_HOME: "/isolated/grok/account-a" },
      timeoutMs: 250,
      runCommand: async ({ argv, env, timeoutMs }) => {
        calls.push({ argv, home: env.GROK_HOME, timeoutMs })
        return argv[1] === "version"
          ? { code: 0, stdout: "grok 0.2.93", stderr: "", timedOut: false }
          : { code: 0, stdout: "grok-4\ngrok-code-fast-1", stderr: "", timedOut: false }
      },
    })
    expect(ready).toMatchObject({
      ready: true,
      version: "grok 0.2.93",
      models: ["grok-4", "grok-code-fast-1"],
    })
    expect(calls).toEqual([
      { argv: ["grok", "version"], home: "/isolated/grok/account-a", timeoutMs: 250 },
      { argv: ["grok", "models"], home: "/isolated/grok/account-a", timeoutMs: 250 },
    ])

    const timedOut = await probeGrokReadiness({
      runCommand: async () => ({
        code: 143,
        stdout: "",
        stderr: "private path must not become authority",
        timedOut: true,
      }),
    })
    expect(timedOut).toMatchObject({
      ready: false,
      failureClass: "timeout",
      detail: "Grok readiness probe timed out.",
    })

    const authFailed = await probeGrokReadiness({
      runCommand: async ({ argv }) =>
        argv[1] === "version"
          ? { code: 0, stdout: "grok 0.2.93", stderr: "", timedOut: false }
          : { code: 1, stdout: "", stderr: "login required", timedOut: false },
    })
    expect(authFailed).toMatchObject({
      ready: false,
      failureClass: "auth_required",
      detail: "Grok isolated CLI authentication is unavailable.",
    })
    expect(JSON.stringify(authFailed)).not.toContain("login required")

    const emptyModels = await probeGrokReadiness({
      runCommand: async ({ argv }) =>
        argv[1] === "version"
          ? { code: 0, stdout: "grok 0.2.93", stderr: "", timedOut: false }
          : { code: 0, stdout: "No models", stderr: "", timedOut: false },
    })
    expect(emptyModels).toMatchObject({
      ready: false,
      failureClass: "unknown",
      detail: "Grok readiness returned no runnable models.",
    })

    const oversized = await probeGrokReadiness({
      runCommand: async () => ({
        code: 0,
        stdout: "x".repeat(70_000),
        stderr: "",
        timedOut: false,
      }),
    })
    expect(oversized).toMatchObject({
      ready: false,
      failureClass: "unknown",
      detail: "Grok readiness output exceeded its bound.",
    })
  })

  test("isolated custody rejects configured API-key and env-key sources without exposing values", async () => {
    const home = await mkdtemp(join(tmpdir(), "grok-custody-"))
    try {
      await writeFile(
        join(home, "config.toml"),
        'model.private."env_key" = "OWNER_SHARED_GROK_KEY"\n',
      )
      expect(await grokConfiguredApiCredentialPresent({ GROK_HOME: home })).toBe(true)

      let calls = 0
      const readiness = await probeGrokReadiness({
        env: {
          GROK_HOME: home,
          OWNER_SHARED_GROK_KEY: "xai-private-value",
        },
        runCommand: async () => {
          calls += 1
          return { code: 0, stdout: "grok 0.2.93", stderr: "", timedOut: false }
        },
      })
      expect(calls).toBe(0)
      expect(readiness).toEqual({
        ready: false,
        binary: "grok",
        plane: "api_key",
        models: [],
        failureClass: "auth_required",
        detail: "Grok configured API credentials cannot satisfy isolated CLI custody.",
      })
      expect(JSON.stringify(readiness)).not.toContain("OWNER_SHARED_GROK_KEY")
      expect(JSON.stringify(readiness)).not.toContain("xai-private-value")

      await writeFile(join(home, "config.toml"), "# no model credential\n")
      expect(await grokConfiguredApiCredentialPresent({
        GROK_HOME: home,
        GROK_AUTH: "shared-session-token",
      })).toBe(true)
    } finally {
      await rm(home, { force: true, recursive: true })
    }
  })

  test("default readiness returns after its deadline even when the CLI ignores SIGTERM", async () => {
    const home = await mkdtemp(join(tmpdir(), "grok-readiness-timeout-"))
    try {
      const binary = join(home, "ignore-term")
      await writeFile(binary, "#!/bin/sh\ntrap '' TERM\nwhile :; do :; done\n")
      await chmod(binary, 0o700)
      const startedAt = Date.now()
      const readiness = await probeGrokReadiness({
        binary,
        env: { GROK_HOME: home },
        timeoutMs: 100,
      })
      expect(Date.now() - startedAt).toBeLessThan(750)
      expect(readiness).toMatchObject({
        ready: false,
        failureClass: "timeout",
        detail: "Grok readiness probe timed out.",
      })
    } finally {
      await rm(home, { force: true, recursive: true })
    }
  })

  test("runClaimedWork records claim pins and metering honesty", async () => {
    const executor = createGrokHeadlessWorkerExecutor({
      async runCommand(argv, cwd) {
        expect(argv).toContain("-p")
        expect(argv).toContain("--no-auto-update")
        expect(argv).toContain("--always-approve")
        expect(argv).not.toContain("--session-id")
        expect(argv).not.toContain("--resume")
        expect(cwd).toBe("/tmp/work")
        const promptIdx = argv.indexOf("-p")
        const prompt = argv[promptIdx + 1] ?? ""
        expect(prompt).toContain("claimRef=claim-1")
        expect(prompt).toContain("workUnitRef=issue-9")
        return {
          code: 0,
          stdout: "done",
          stderr: "",
          wallClockMs: 12,
        }
      },
    })

    const closeout = await executor.runClaimedWork({
      pin: {
        claimRef: "claim-1",
        workUnitRef: "issue-9",
        runRef: "run-1",
        cwd: "/tmp/work",
        verifyCommand: "bun test",
      },
      prompt: "Fix the test",
      plane: "cli_session",
      marginalCostClass: "free",
    })

    expect(closeout.ok).toBe(true)
    expect(closeout.claimRef).toBe("claim-1")
    expect(closeout.text).toBe("done")
    expect(closeout.usage.metering).toBe("not_measured")
    expect(closeout.usage.marginalCostClass).toBe("free")
    expect(closeout.usage.plane).toBe("cli_session")
  })

  test("creates and resumes the same canonical session with pinned headless argv", async () => {
    const sessionId = "d5d2c7cc-31f3-5b3f-8d11-41a68b1f88ef"
    const calls: Array<{
      argv: string[]
      cwd: string
      timeoutMs: number
    }> = []
    const executor = createGrokHeadlessWorkerExecutor({
      binary: "grok-test",
      async runCommand(argv, cwd, timeoutMs) {
        calls.push({ argv, cwd, timeoutMs })
        return {
          code: 0,
          stdout: argv.includes("--resume") ? "follow-up complete" : "initial complete",
          stderr: "",
          wallClockMs: 12,
        }
      },
    })
    const pin = {
      claimRef: "claim-session",
      workUnitRef: "issue-8640",
      runRef: "run-session",
      cwd: "/tmp/session-work",
      repo: "OpenAgentsInc/openagents",
      commit: "abc123",
      branch: "main",
      verifyCommand: "bun test",
    }

    const initial = await executor.runClaimedWork({
      pin,
      prompt: "Initial private task body",
      sessionId,
      model: "grok-code-fast-1",
      timeoutMs: 9_999,
      plane: "cli_session",
      marginalCostClass: "subscription",
    })
    const followUp = await executor.runFollowUp({
      pin,
      prompt: "Private steering body",
      sessionId,
      model: "grok-code-fast-1",
      timeoutMs: 9_999,
      plane: "cli_session",
      marginalCostClass: "subscription",
    })

    expect(calls).toHaveLength(2)
    const initialArgv = calls[0]?.argv ?? []
    const followUpArgv = calls[1]?.argv ?? []
    expect(initialArgv.slice(0, 6)).toEqual([
      "grok-test",
      "--no-auto-update",
      "--no-alt-screen",
      "--always-approve",
      "--session-id",
      sessionId,
    ])
    expect(initialArgv).not.toContain("--resume")
    expect(followUpArgv.slice(0, 6)).toEqual([
      "grok-test",
      "--no-auto-update",
      "--no-alt-screen",
      "--always-approve",
      "--resume",
      sessionId,
    ])
    expect(followUpArgv).not.toContain("--session-id")
    for (const call of calls) {
      expect(call.cwd).toBe(pin.cwd)
      expect(call.timeoutMs).toBe(9_999)
      expect(call.argv).toContain("--always-approve")
      expect(call.argv).toContain("-p")
      expect(call.argv).toContain("--cwd")
      expect(call.argv).toContain(pin.cwd)
      expect(call.argv).toContain("--output-format")
      expect(call.argv).toContain("plain")
      expect(call.argv).toContain("-m")
      expect(call.argv).toContain("grok-code-fast-1")
    }
    const initialPrompt = initialArgv[initialArgv.indexOf("-p") + 1] ?? ""
    const followUpPrompt = followUpArgv[followUpArgv.indexOf("-p") + 1] ?? ""
    expect(initialPrompt).toContain("claimRef=claim-session")
    expect(initialPrompt).toContain("Initial private task body")
    expect(initialPrompt).not.toContain(sessionId)
    expect(followUpPrompt).toContain("claimRef=claim-session")
    expect(followUpPrompt).toContain("Private steering body")
    expect(followUpPrompt).not.toContain(sessionId)
    expect(initial).toMatchObject({
      ok: true,
      claimRef: "claim-session",
      stopReason: "end_turn",
      text: "initial complete",
      usage: {
        metering: "not_measured",
        model: "grok-code-fast-1",
        plane: "cli_session",
        marginalCostClass: "subscription",
      },
    })
    expect(followUp).toMatchObject({
      ok: true,
      claimRef: "claim-session",
      stopReason: "end_turn",
      text: "follow-up complete",
      usage: {
        metering: "not_measured",
        model: "grok-code-fast-1",
        plane: "cli_session",
        marginalCostClass: "subscription",
      },
    })
    expect(JSON.stringify(initial)).not.toContain(sessionId)
    expect(JSON.stringify(followUp)).not.toContain(sessionId)
    expect(JSON.stringify(initial)).not.toContain("Initial private task body")
    expect(JSON.stringify(followUp)).not.toContain("Private steering body")
  })

  test("rejects non-canonical session ids without invoking Grok", async () => {
    let calls = 0
    const executor = createGrokHeadlessWorkerExecutor({
      async runCommand() {
        calls += 1
        return {
          code: 0,
          stdout: "must not run",
          stderr: "",
          wallClockMs: 1,
        }
      },
    })
    const pin = {
      claimRef: "claim-invalid-session",
      workUnitRef: "issue-8640",
      runRef: "run-invalid-session",
      cwd: "/tmp/invalid-session-work",
    }

    const initial = await executor.runClaimedWork({
      pin,
      prompt: "Private initial body",
      sessionId: "D5D2C7CC-31F3-5B3F-8D11-41A68B1F88EF",
    })
    const followUp = await executor.runFollowUp({
      pin,
      prompt: "Private follow-up body",
      sessionId: "not-a-uuid",
    })

    expect(calls).toBe(0)
    expect(initial).toMatchObject({
      ok: false,
      claimRef: "claim-invalid-session",
      stopReason: "invalid_session_id",
      text: "",
      usage: { metering: "not_measured", wallClockMs: 0 },
      failureClass: "unknown",
    })
    expect(followUp).toMatchObject({
      ok: false,
      claimRef: "claim-invalid-session",
      stopReason: "invalid_session_id",
      text: "",
      usage: { metering: "not_measured", wallClockMs: 0 },
      failureClass: "unknown",
    })
    const publicCloseouts = JSON.stringify([initial, followUp])
    expect(publicCloseouts).not.toContain("D5D2C7CC")
    expect(publicCloseouts).not.toContain("not-a-uuid")
    expect(publicCloseouts).not.toContain("Private initial body")
    expect(publicCloseouts).not.toContain("Private follow-up body")
  })

  test("classifies rate limit failures", async () => {
    const executor = createGrokHeadlessWorkerExecutor({
      async runCommand() {
        return {
          code: 1,
          stdout: "",
          stderr: "Error: 429 rate limit exceeded",
          wallClockMs: 5,
        }
      },
    })

    const closeout = await executor.runClaimedWork({
      pin: {
        claimRef: "c",
        workUnitRef: "w",
        runRef: "r",
        cwd: "/tmp",
      },
      prompt: "x",
    })

    expect(closeout.ok).toBe(false)
    expect(closeout.failureClass).toBe("account_rate_limited")
  })

  test("classifies provider 402 usage exhaustion before the CLI auth wrapper", async () => {
    const executor = createGrokHeadlessWorkerExecutor({
      async runCommand() {
        return {
          code: 1,
          stdout: "",
          stderr: "AuthenticationError: HTTP 402 Payment Required for usage",
          wallClockMs: 5,
        }
      },
    })

    const closeout = await executor.runClaimedWork({
      pin: {
        claimRef: "claim-402",
        workUnitRef: "work-402",
        runRef: "run-402",
        cwd: "/tmp",
      },
      prompt: "x",
    })

    expect(closeout.ok).toBe(false)
    expect(closeout.failureClass).toBe("account_quota_exhausted")
  })

  test("kills a worker that exceeds its bounded execution deadline", async () => {
    const home = await mkdtemp(join(tmpdir(), "grok-worker-timeout-"))
    try {
      const binary = join(home, "ignore-term")
      await writeFile(binary, "#!/bin/sh\ntrap '' TERM\nwhile :; do :; done\n")
      await chmod(binary, 0o700)
      const executor = createGrokHeadlessWorkerExecutor({
        binary,
        env: { GROK_HOME: home },
      })
      const startedAt = Date.now()
      const closeout = await executor.runClaimedWork({
        pin: {
          claimRef: "claim-timeout",
          workUnitRef: "work-timeout",
          runRef: "run-timeout",
          cwd: home,
        },
        prompt: "Stay bounded.",
        timeoutMs: 100,
        plane: "cli_session",
        marginalCostClass: "subscription",
      })

      expect(Date.now() - startedAt).toBeLessThan(900)
      expect(closeout).toMatchObject({
        ok: false,
        claimRef: "claim-timeout",
        stopReason: "timeout",
        failureClass: "timeout",
        usage: { metering: "not_measured" },
      })
      expect(closeout.text).toBe("")
    } finally {
      await rm(home, { force: true, recursive: true })
    }
  })
})
