/**
 * Codex preflight prober tests (EP250 anti-speedbump core). Enforces the
 * probe-verified evidence rule: the receipted minimal read-only probe recipe,
 * verified/reconnect/quota/rate-limit/missing/failed classification through the
 * REAL parser, the credentials_missing fast path (NO spawn — the live
 * missing-auth probe burns ~50s of 401 retries), the host-side timeout
 * bound, health + onResult feeds, concurrent probing, and the session cache
 * semantics of ensureProbed vs probeAll.
 */
import { describe, expect, test } from "vite-plus/test"
import { EventEmitter } from "node:events"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { PassThrough } from "node:stream"

import {
  fixtureCodexRateLimitStdout,
  fixtureCodexRevokedStderr,
  fixtureCodexRevokedStdout,
  fixtureCodexSuccessStdout,
  makeCodexAccountHealth,
  makeFixtureCodexChildSpawn,
  type CodexChildAccount,
} from "./codex-child-runtime.ts"
import {
  CODEX_PREFLIGHT_PROMPT,
  CODEX_PREFLIGHT_SANDBOX,
  makeCodexPreflight,
  type CodexProbeResult,
} from "./codex-preflight.ts"
import type { CodexAppServerSpawn } from "./codex-app-server-client.ts"

const scratch = (): string => mkdtempSync(join(tmpdir(), "codex-preflight-"))

const account = (ref: string): CodexChildAccount => ({
  ref,
  home: `/isolated/accounts/codex/${ref}`,
})

describe("makeCodexPreflight", () => {
  test("stops before provider work and preserves an exact Codex configuration diagnostic", async () => {
    let spawned = 0
    const issue = {
      path: "/Users/owner/.codex/config.toml",
      line: 408,
      column: 1,
      message: "invalid transport",
    }
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      discoverImpl: async () => [account("codex-current")],
      spawnImpl: input => { spawned += 1; return makeFixtureCodexChildSpawn([])(input) },
      configCheck: async () => ({ state: "invalid", issue }),
    })
    expect(await preflight.probeAll("test")).toEqual([expect.objectContaining({
      state: "config_invalid",
      detail: "/Users/owner/.codex/config.toml:408:1: invalid transport",
      configuration: { issue, repaired: false },
    })])
    expect(spawned).toBe(0)
  })

  test("production preflight uses an ephemeral read-only native app-server turn", async () => {
    const stdin = new PassThrough()
    const stdout = new PassThrough()
    const stderr = new PassThrough()
    const child = new EventEmitter() as EventEmitter & {
      stdin: PassThrough
      stdout: PassThrough
      stderr: PassThrough
      kill: () => boolean
    }
    child.stdin = stdin
    child.stdout = stdout
    child.stderr = stderr
    child.kill = () => true
    const messages: Array<Record<string, any>> = []
    let buffer = ""
    stdin.on("data", chunk => {
      buffer += chunk.toString("utf8")
      while (buffer.includes("\n")) {
        const index = buffer.indexOf("\n")
        const line = buffer.slice(0, index)
        buffer = buffer.slice(index + 1)
        if (line === "") continue
        const message = JSON.parse(line) as Record<string, any>
        messages.push(message)
        if (message.method === "initialize") stdout.write(`${JSON.stringify({ id: message.id, result: {} })}\n`)
        if (message.method === "thread/start") stdout.write(`${JSON.stringify({ id: message.id, result: { thread: { id: "preflight-thread" } } })}\n`)
        if (message.method === "turn/start") {
          stdout.write(`${JSON.stringify({ id: message.id, result: { turn: { id: "preflight-turn", status: "inProgress" } } })}\n`)
          queueMicrotask(() => {
            stdout.write(`${JSON.stringify({ method: "item/agentMessage/delta", params: {
              threadId: "preflight-thread", turnId: "preflight-turn", delta: "ok",
            } })}\n`)
            stdout.write(`${JSON.stringify({ method: "turn/completed", params: {
              threadId: "preflight-thread", turn: { id: "preflight-turn", status: "completed" },
            } })}\n`)
          })
        }
      }
    })
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: () => { throw new Error("legacy codex exec must not run") },
      discoverImpl: async () => [account("codex-native")],
      health: makeCodexAccountHealth(),
      appServer: {
        binary: () => "/packaged/codex",
        installProductSpecSkill: () => ({
          skillRoot: "/isolated/accounts/codex/codex-native/skills",
          skillPath: "/isolated/accounts/codex/codex-native/skills/productspec-work/SKILL.md",
        }),
        spawnImpl: (() => child) as unknown as CodexAppServerSpawn,
      },
    })
    const results = await preflight.probeAll("test")
    expect(results[0]).toMatchObject({ state: "verified", detail: "app-server probe turn completed" })
    expect(messages.find(message => message.method === "thread/start")?.params).toMatchObject({
      ephemeral: true,
      approvalPolicy: "never",
    })
    expect(messages.find(message => message.method === "turn/start")?.params).toMatchObject({
      sandboxPolicy: { type: "readOnly", networkAccess: true },
      input: [{ type: "text", text: CODEX_PREFLIGHT_PROMPT, text_elements: [] }],
    })
  })

  test("spawns the receipted MINIMAL probe recipe: read-only sandbox, low effort, --ephemeral, tiny prompt", async () => {
    const captured: Array<{ args: ReadonlyArray<string>; env: Record<string, string | undefined> }> = []
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexSuccessStdout(), exitCode: 0 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => [account("codex-5")],
      health: makeCodexAccountHealth(),
    })
    const results = await preflight.probeAll("test")
    expect(results[0]!.state).toBe("verified")
    const args = captured[0]!.args
    expect(args[0]).toBe("exec")
    expect(args).toContain("--json")
    expect(args).toContain("-s")
    expect(args).toContain(CODEX_PREFLIGHT_SANDBOX)
    expect(CODEX_PREFLIGHT_SANDBOX).toBe("read-only")
    expect(args).toContain("model_reasoning_effort=low")
    expect(args).toContain("--ephemeral")
    expect(args[args.length - 1]).toBe(CODEX_PREFLIGHT_PROMPT)
    expect(captured[0]!.env.CODEX_HOME).toBe("/isolated/accounts/codex/codex-5")
  })

  test("classifies verified / reconnect / policy / quota / rate-limit distinctly across a concurrent round", async () => {
    const health = makeCodexAccountHealth()
    const streamed: CodexProbeResult[] = []
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
        { stdout: fixtureCodexRateLimitStdout, exitCode: 1 },
        { stdout: `${JSON.stringify({ type: "turn.failed", error: { message: "provider rate limit exceeded" } })}\n`, exitCode: 1 },
        { stdout: `${JSON.stringify({ type: "turn.failed", error: { message: "command denied by policy" } })}\n`, exitCode: 1 },
      ]),
      discoverImpl: async () => [account("codex"), account("codex-5"), account("codex-7"), account("codex-8"), account("codex-9")],
      health,
      onResult: result => streamed.push(result),
    })
    const results = await preflight.probeAll("boot")
    const byRef = new Map(results.map(result => [result.ref, result]))
    expect(byRef.get("codex")!.state).toBe("reconnect_required")
    expect(byRef.get("codex-5")!.state).toBe("verified")
    // Exhausted quota and transient throttling are distinct; neither is auth-broken.
    expect(byRef.get("codex-7")!.state).toBe("quota_exhausted")
    expect(byRef.get("codex-8")!.state).toBe("rate_limited")
    expect(byRef.get("codex-9")!.state).toBe("policy_denied")
    expect(health.stateOf("codex")).toBe("auth_failed")
    expect(health.stateOf("codex-5")).toBe("last_good")
    expect(health.stateOf("codex-7")).toBe(null)
    expect(health.stateOf("codex-8")).toBe(null)
    expect(health.stateOf("codex-9")).toBe(null)
    expect(streamed).toHaveLength(5)
    expect(preflight.verifiedRefs()).toEqual(["codex-5"])
    for (const result of results) {
      expect(typeof result.observedAt).toBe("string")
      expect(result.observedAt.length).toBeGreaterThan(0)
      expect(result.durationMs).toBeGreaterThanOrEqual(0)
    }
  })

  test("credentials_missing fast path: no auth.json means NO spawn (the live missing-auth probe burns ~50s of 401 retries)", async () => {
    let spawned = 0
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => false,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 0 }], () => {
        spawned += 1
      }),
      discoverImpl: async () => [account("codex-empty")],
      health: makeCodexAccountHealth(),
    })
    const results = await preflight.probeAll("boot")
    expect(results[0]!.state).toBe("credentials_missing")
    expect(spawned).toBe(0)
  })

  test("a hanging probe is SIGTERMed into probe_failed by the host-side bound", async () => {
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: JSON.stringify({ type: "thread.started", thread_id: "t" }), exitCode: 0, hang: true },
      ]),
      discoverImpl: async () => [account("codex-hang")],
      health: makeCodexAccountHealth(),
      timeoutMs: 40,
    })
    const results = await preflight.probeAll("boot")
    expect(results[0]!.state).toBe("probe_failed")
    expect(results[0]!.detail).toContain("timed out")
  })

  test("ensureProbed probes ONCE per session and then reuses; probeAll re-probes; concurrent probeAll calls share one round", async () => {
    let rounds = 0
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: fixtureCodexSuccessStdout(), exitCode: 0 }]),
      discoverImpl: async () => {
        rounds += 1
        return [account("codex-5")]
      },
      health: makeCodexAccountHealth(),
    })
    const [a, b] = await Promise.all([preflight.probeAll("x"), preflight.probeAll("y")])
    expect(a).toBe(b)
    expect(rounds).toBe(1)
    await preflight.ensureProbed()
    expect(rounds).toBe(1)
    await preflight.probeAll("refresh")
    expect(rounds).toBe(2)
  })
})
