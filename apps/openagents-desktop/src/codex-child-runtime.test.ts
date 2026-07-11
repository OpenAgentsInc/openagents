/**
 * Codex child runtime tests (#8712 Lane C): the receipted spawn recipe,
 * exact usage accounting from turn.completed, typed rotation on the exact
 * revoked-refresh-token failure shape, the typed all-accounts-unavailable
 * result, the host-side timeout bound, and concurrent children with
 * isolated per-child scratch dirs — all through the REAL JSONL parser.
 */
import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  CODEX_CHILD_MODEL,
  CODEX_CHILD_REASONING_EFFORT,
  isCodexReconnectRequiredText,
  type CodexChildStreamEvent,
} from "./codex-child-contract.ts"
import {
  FIXTURE_CODEX_CHILD_TEXT,
  discoverRegisteredCodexAccounts,
  fixtureCodexRevokedStderr,
  fixtureCodexRevokedStdout,
  fixtureCodexSuccessStdout,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
  type CodexChildAccount,
} from "./codex-child-runtime.ts"

const accounts: ReadonlyArray<CodexChildAccount> = [
  { ref: "codex", home: "/isolated/accounts/codex/codex" },
  { ref: "codex-2", home: "/isolated/accounts/codex/codex-2" },
]

type SpawnCapture = { args: ReadonlyArray<string>; env: Record<string, string | undefined>; cwd: string }

const collect = () => {
  const events: CodexChildStreamEvent[] = []
  return { events, onEvent: (event: CodexChildStreamEvent) => events.push(event) }
}

describe("isCodexReconnectRequiredText", () => {
  test("matches the exact receipted revoked-token strings and nothing generic", () => {
    expect(isCodexReconnectRequiredText(
      "Your access token could not be refreshed because your refresh token was revoked",
    )).toBe(true)
    expect(isCodexReconnectRequiredText("ERROR refresh_token_invalidated")).toBe(true)
    expect(isCodexReconnectRequiredText("token_invalidated")).toBe(true)
    expect(isCodexReconnectRequiredText("network unreachable")).toBe(false)
    expect(isCodexReconnectRequiredText("rate limit exceeded")).toBe(false)
  })
})

describe("makeCodexChildRuntime.runChild", () => {
  test("spawns the receipted recipe: exec --json, pinned model/effort, read-only sandbox, ephemeral, isolated CODEX_HOME", async () => {
    const captured: SpawnCapture[] = []
    const scratch = mkdtempSync(join(tmpdir(), "codex-child-scratch-"))
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => scratch,
      env: { PATH: "/usr/bin" },
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexSuccessStdout(), exitCode: 0 }],
        (input) => captured.push(input),
      ),
      discoverImpl: async () => [accounts[0]!],
    })
    const sink = collect()
    const result = await runtime.runChild({
      childRef: "child-1",
      task: "Summarize this",
      context: "extra context",
      onEvent: sink.onEvent,
    })
    expect(result.ok).toBe(true)
    const spawn = captured[0]!
    expect(spawn.args).toEqual([
      "exec",
      "--json",
      "-m",
      "gpt-5.6-sol",
      "-c",
      "model_reasoning_effort=medium",
      "-s",
      "read-only",
      "--skip-git-repo-check",
      "-C",
      join(scratch, "codex-children", "child-1"),
      "--ephemeral",
      "Summarize this\n\nContext:\nextra context",
    ])
    // pylonAccountEnvironment (provider codex): CODEX_HOME only — never the
    // default ~/.codex, never any other provider-home mutation.
    expect(spawn.env.CODEX_HOME).toBe("/isolated/accounts/codex/codex")
    expect(spawn.env.PATH).toBe("/usr/bin")
    expect(spawn.cwd).toBe(join(scratch, "codex-children", "child-1"))
  })

  test("single child success carries exact usage totals from turn.completed (total = input+output+reasoning)", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: fixtureCodexSuccessStdout("thread-42"), exitCode: 0 }]),
      discoverImpl: async () => [accounts[0]!],
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-usage", task: "go", onEvent: sink.onEvent })
    if (!result.ok) throw new Error(`expected success, got ${result.reason}`)
    expect(result.text).toBe(FIXTURE_CODEX_CHILD_TEXT)
    expect(result.threadId).toBe("thread-42")
    expect(result.accountRef).toBe("codex")
    // Spawn-config truth (the exec stream does not echo model/effort).
    expect(result.requestedModel).toBe(CODEX_CHILD_MODEL)
    expect(result.requestedEffort).toBe(CODEX_CHILD_REASONING_EFFORT)
    expect(result.usage).toEqual({
      inputTokens: 1200,
      cachedInputTokens: 900,
      outputTokens: 180,
      reasoningOutputTokens: 60,
      // Mirrors codex-agent-executor accounting: cached is NOT in the total.
      totalTokens: 1440,
    })
    expect(result.durationMs).toBeGreaterThanOrEqual(0)
    // Item stream flowed: the completed reasoning and agent_message items.
    const items = sink.events.filter(event => event.kind === "item")
    expect(items.map(event => (event as Extract<CodexChildStreamEvent, { kind: "item" }>).itemType))
      .toEqual(["reasoning", "agent_message"])
  })

  test("rotates on the exact revoked-token failure with a TYPED visible event, then succeeds on the next account", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-rotate", task: "go", onEvent: sink.onEvent })
    if (!result.ok) throw new Error(`expected success, got ${result.reason}`)
    expect(result.accountRef).toBe("codex-2")
    // Never a silent skip: the revoked account emitted a typed event first.
    expect(sink.events.map(event => event.kind)).toEqual([
      "attempt_started",
      "account_reconnect_required",
      "attempt_started",
      "item",
      "item",
    ])
    const reconnect = sink.events[1] as Extract<CodexChildStreamEvent, { kind: "account_reconnect_required" }>
    expect(reconnect.accountRef).toBe("codex")
    expect(reconnect.detail).toContain("reconnect")
  })

  test("all accounts revoked yields the typed account_reconnect_required failure naming the reconnect need", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
      ]),
      discoverImpl: async () => accounts,
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-all-revoked", task: "go", onEvent: sink.onEvent })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("account_reconnect_required")
    expect(result.detail).toContain("all 2 registered Codex account(s) need reconnect")
    expect(result.detail).toContain("khala fleet connect")
    expect(sink.events.filter(event => event.kind === "account_reconnect_required")).toHaveLength(2)
  })

  test("no registered account yields the typed no_codex_account failure without spawning", async () => {
    let spawned = 0
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 0 }], () => {
        spawned += 1
      }),
      discoverImpl: async () => [],
    })
    const result = await runtime.runChild({ childRef: "child-none", task: "go" })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("no_codex_account")
    expect(spawned).toBe(0)
  })

  test("host-side wall clock bound: a hanging child is SIGTERMed into a typed child_timeout (no rotation)", async () => {
    let spawned = 0
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: JSON.stringify({ type: "thread.started", thread_id: "t" }), exitCode: 0, hang: true }],
        () => {
          spawned += 1
        },
      ),
      discoverImpl: async () => accounts,
      timeoutMs: 40,
    })
    const result = await runtime.runChild({ childRef: "child-hang", task: "go" })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("child_timeout")
    expect(result.accountRef).toBe("codex")
    // A timeout consumed real budget: it is terminal, never a rotation.
    expect(spawned).toBe(1)
  })

  test("a non-revoked failure is typed child_failed and does not rotate", async () => {
    let spawned = 0
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: JSON.stringify({ type: "error", message: "stream disconnected" }), exitCode: 1 }],
        () => {
          spawned += 1
        },
      ),
      discoverImpl: async () => accounts,
    })
    const result = await runtime.runChild({ childRef: "child-fail", task: "go" })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("child_failed")
    expect(result.detail).toContain("stream disconnected")
    expect(spawned).toBe(1)
  })

  test("a clean exit without agent_message text is typed child_failed, never an empty success", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: JSON.stringify({ type: "turn.completed", usage: { input_tokens: 5 } }), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
    })
    const result = await runtime.runChild({ childRef: "child-empty", task: "go" })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("child_failed")
    expect(result.detail).toContain("no agent_message")
  })

  test("3 concurrent children run in isolated per-child scratch dirs and all complete", async () => {
    const cwds: string[] = []
    const scratch = mkdtempSync(join(tmpdir(), "codex-child-scratch-"))
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => scratch,
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexSuccessStdout(), exitCode: 0, delayMs: 10 }],
        (input) => cwds.push(input.cwd),
      ),
      discoverImpl: async () => [accounts[0]!],
    })
    const results = await Promise.all([
      runtime.runChild({ childRef: "child-a", task: "a" }),
      runtime.runChild({ childRef: "child-b", task: "b" }),
      runtime.runChild({ childRef: "child-c", task: "c" }),
    ])
    expect(results.every(result => result.ok)).toBe(true)
    expect(new Set(cwds).size).toBe(3)
    expect(cwds.sort()).toEqual([
      join(scratch, "codex-children", "child-a"),
      join(scratch, "codex-children", "child-b"),
      join(scratch, "codex-children", "child-c"),
    ])
  })

  test("FRESHNESS (EP250): the account candidate list is re-read at EVERY delegate call — a mid-session reconnect is picked up without restart", async () => {
    // Owner receipt: codex-5 was registered at 15:30:40 but the 15:32
    // children never tried it. The registry read must happen per runChild —
    // never captured at construction or turn start. This oracle mutates the
    // discover source BETWEEN calls and asserts the second call rotates into
    // the newly registered ref.
    const registered: CodexChildAccount[] = [
      { ref: "codex", home: "/isolated/accounts/codex/codex" },
    ]
    let discoverCalls = 0
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        // call 1: the only account is revoked -> typed all-revoked failure
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        // call 2: codex still revoked -> rotate -> codex-5 succeeds
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => {
        discoverCalls += 1
        return [...registered]
      },
    })

    const first = await runtime.runChild({ childRef: "child-before", task: "go" })
    expect(first.ok).toBe(false)
    if (!first.ok) expect(first.reason).toBe("account_reconnect_required")
    expect(discoverCalls).toBe(1)

    // Mid-session reconnect: a new ready ref lands in the registry.
    registered.push({ ref: "codex-5", home: "/isolated/accounts/codex/codex-5" })

    const sink = collect()
    const second = await runtime.runChild({ childRef: "child-after", task: "go", onEvent: sink.onEvent })
    expect(discoverCalls).toBe(2)
    if (!second.ok) throw new Error(`expected the fresh ref to be used, got ${second.reason}`)
    expect(second.accountRef).toBe("codex-5")
    expect(sink.events.some(event =>
      event.kind === "attempt_started" && event.accountRef === "codex-5")).toBe(true)
  })
})

describe("discoverRegisteredCodexAccounts (real registry read)", () => {
  test("reads the pylon config fresh on every call — a ref registered after the first read appears in the second", async () => {
    const pylonHome = mkdtempSync(join(tmpdir(), "codex-child-registry-"))
    const homeA = join(pylonHome, "accounts", "codex", "codex")
    mkdirSync(homeA, { recursive: true })
    const writeConfig = (accounts: Array<{ ref: string; home: string }>): void => {
      writeFileSync(
        join(pylonHome, "config.json"),
        JSON.stringify({
          dev: { accounts: accounts.map(account => ({ provider: "codex", ...account })) },
        }),
      )
    }
    writeConfig([{ ref: "codex", home: homeA }])
    const env = { PYLON_HOME: pylonHome }

    const before = await discoverRegisteredCodexAccounts(env)
    expect(before.map(account => account.ref)).toEqual(["codex"])

    // Mid-session UI reconnect registers a new ref (the codex-5 case).
    const homeB = join(pylonHome, "accounts", "codex", "codex-5")
    mkdirSync(homeB, { recursive: true })
    writeConfig([{ ref: "codex", home: homeA }, { ref: "codex-5", home: homeB }])

    const after = await discoverRegisteredCodexAccounts(env)
    expect(after.map(account => account.ref)).toEqual(["codex", "codex-5"])
  })
})
