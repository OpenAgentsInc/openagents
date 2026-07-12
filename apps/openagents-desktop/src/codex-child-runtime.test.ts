/**
 * Codex child runtime tests (#8712 Lane C + EP250 rotation fix). Enforces
 * openagents_desktop.seam.codex_delegation_no_substitution.v2 and the child
 * half of openagents_desktop.chat.fable_local_owner_full_access.v1: the
 * receipted spawn recipe (danger-full-access owner-local profile), exact usage
 * accounting from turn.completed, BROADENED auth-class classification
 * (including the live SHORT variant that the original marker set missed),
 * typed pre-content rotation for non-auth failures, post-content failures
 * staying terminal, in-process account health ordering (last-good first,
 * auth-failed last, success clears), the typed all-accounts-unavailable
 * result, the host-side timeout bound, and concurrent children with isolated
 * per-child scratch dirs — all through the REAL JSONL parser.
 */
import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  CODEX_CHILD_MODEL,
  CODEX_CHILD_REASONING_EFFORT,
  CODEX_CHILD_SANDBOX,
  isCodexReconnectRequiredText,
  type CodexChildStreamEvent,
} from "./codex-child-contract.ts"
import {
  FIXTURE_CODEX_CHILD_TEXT,
  FIXTURE_CODEX_SHORT_AUTH_MESSAGE,
  discoverRegisteredCodexAccounts,
  defaultSpawnCodex,
  fixtureCodexRevokedStderr,
  fixtureCodexRevokedStdout,
  fixtureCodexShortAuthStdout,
  fixtureCodexSuccessStdout,
  makeCodexAccountHealth,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
  sharedCodexAccountHealth,
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

test("default Codex launch uses the packaged native binary with ambient PATH disabled", async () => {
  const cwd = mkdtempSync(join(tmpdir(), "openagents-codex-bundled-"))
  const child = defaultSpawnCodex({ args: ["--version"], cwd, env: { PATH: "", CODEX_HOME: cwd } })
  expect(child).not.toBeNull()
  if (child === null) return
  const code = await new Promise<number | null>(resolve => {
    child.on("error", () => resolve(null))
    child.on("close", (...args) => resolve(typeof args[0] === "number" ? args[0] : null))
  })
  expect(code).toBe(0)
})

describe("isCodexReconnectRequiredText (BROADENED, EP250)", () => {
  test("matches the LONG receipted revoked-token strings", () => {
    expect(isCodexReconnectRequiredText(
      "Your access token could not be refreshed because your refresh token was revoked",
    )).toBe(true)
    expect(isCodexReconnectRequiredText("ERROR refresh_token_invalidated")).toBe(true)
    expect(isCodexReconnectRequiredText("token_invalidated")).toBe(true)
  })

  test("matches the LIVE SHORT variant VERBATIM (the exact message the original set missed)", () => {
    // Owner evidence 2026-07-11: one delegated child failed final with
    // exactly this text — no "revoked", no "*_invalidated" substrings — and
    // no rotation happened while a known-good codex-5 home sat idle.
    expect(isCodexReconnectRequiredText(
      "Your access token could not be refreshed. Please log out and sign in again.",
    )).toBe(true)
    expect(isCodexReconnectRequiredText(FIXTURE_CODEX_SHORT_AUTH_MESSAGE)).toBe(true)
  })

  test("matches the broadened auth-class markers case-insensitively", () => {
    expect(isCodexReconnectRequiredText("please Sign In Again to continue")).toBe(true)
    expect(isCodexReconnectRequiredText("server returned 401")).toBe(true)
    expect(isCodexReconnectRequiredText("Unauthorized")).toBe(true)
    expect(isCodexReconnectRequiredText("credential REVOKED by provider")).toBe(true)
    expect(isCodexReconnectRequiredText("could not refresh the Refresh Token")).toBe(true)
  })

  test("does not match generic non-auth failures", () => {
    expect(isCodexReconnectRequiredText("network unreachable")).toBe(false)
    expect(isCodexReconnectRequiredText("rate limit exceeded")).toBe(false)
    expect(isCodexReconnectRequiredText("stream disconnected")).toBe(false)
    expect(isCodexReconnectRequiredText("codex exec exited 1")).toBe(false)
  })
})

describe("makeCodexChildRuntime.runChild", () => {
  test("spawns the receipted recipe: exec --json, pinned model/effort, danger-full-access sandbox (owner-local profile), ephemeral, isolated CODEX_HOME", async () => {
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
      health: makeCodexAccountHealth(),
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
      // Owner-local danger profile (owner statement 2026-07-11): full
      // access, mirroring the Khala->Pylon owner-local executor invariant.
      "danger-full-access",
      "--skip-git-repo-check",
      "-C",
      join(scratch, "codex-children", "child-1"),
      "--ephemeral",
      "Summarize this\n\nContext:\nextra context",
    ])
    expect(CODEX_CHILD_SANDBOX).toBe("danger-full-access")
    // Pylon fallback: CODEX_HOME only, never another provider-home mutation.
    expect(spawn.env.CODEX_HOME).toBe("/isolated/accounts/codex/codex")
    expect(spawn.env.PATH).toBe("/usr/bin")
    expect(spawn.cwd).toBe(join(scratch, "codex-children", "child-1"))
  })

  test("prefers the ordinary authenticated Codex session and clears a stale inherited CODEX_HOME", async () => {
    const captured: SpawnCapture[] = []
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-current-")),
      env: { HOME: "/owner", CODEX_HOME: "/stale/pylon-home", PATH: "/usr/bin" },
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexSuccessStdout(), exitCode: 0 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => [
        { ref: "codex-current", home: "/owner/.codex", source: "current_session" },
        accounts[0]!,
      ],
      health: makeCodexAccountHealth(),
    })

    const result = await runtime.runChild({ childRef: "child-current", task: "go", onEvent: () => {} })
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.accountRef).toBe("codex-current")
    expect(captured[0]!.env.HOME).toBe("/owner")
    expect(captured[0]!.env.CODEX_HOME).toBeUndefined()
  })

  test("single child success carries exact usage totals from turn.completed (total = input+output+reasoning)", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: fixtureCodexSuccessStdout("thread-42"), exitCode: 0 }]),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
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

  test("rotates on the LONG revoked-token failure with a TYPED visible event, then succeeds on the next account", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
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

  test("EP250 LIVE MISS REGRESSION: the exact SHORT auth variant classifies auth-class and rotates typed (never a terminal child_failed)", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        // The live shape: turn.failed with ONLY the short message, exit 1,
        // no stderr markers at all.
        { stdout: fixtureCodexShortAuthStdout, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-short-auth", task: "go", onEvent: sink.onEvent })
    if (!result.ok) throw new Error(`expected rotation to succeed, got ${result.reason}: ${result.detail}`)
    expect(result.accountRef).toBe("codex-2")
    const reconnect = sink.events.find(event => event.kind === "account_reconnect_required") as
      Extract<CodexChildStreamEvent, { kind: "account_reconnect_required" }>
    expect(reconnect).toBeDefined()
    expect(reconnect.accountRef).toBe("codex")
    expect(sink.events.some(event => event.kind === "pre_content_failure_rotated")).toBe(false)
  })

  test("all accounts auth-failed yields the typed account_reconnect_required failure naming the reconnect need", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
      ]),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-all-revoked", task: "go", onEvent: sink.onEvent })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("account_reconnect_required")
    expect(result.detail).toContain("all 2 available Codex session(s) need reconnect")
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
      health: makeCodexAccountHealth(),
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
      health: makeCodexAccountHealth(),
    })
    const result = await runtime.runChild({ childRef: "child-hang", task: "go" })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("child_timeout")
    expect(result.accountRef).toBe("codex")
    // A timeout consumed real budget: it is terminal, never a rotation.
    expect(spawned).toBe(1)
  })

  test("EP250 BROADENING: a generic PRE-content failure rotates with the typed pre_content_failure_rotated reason, then succeeds", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        // Non-auth failure with no agent_message and zero usage: rotation-
        // eligible (children are ephemeral; pre-content loses nothing).
        { stdout: JSON.stringify({ type: "error", message: "stream disconnected" }), exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-pre-content", task: "go", onEvent: sink.onEvent })
    if (!result.ok) throw new Error(`expected success, got ${result.reason}: ${result.detail}`)
    expect(result.accountRef).toBe("codex-2")
    const rotated = sink.events.find(event => event.kind === "pre_content_failure_rotated") as
      Extract<CodexChildStreamEvent, { kind: "pre_content_failure_rotated" }>
    expect(rotated).toBeDefined()
    expect(rotated.accountRef).toBe("codex")
    expect(rotated.detail).toContain("stream disconnected")
    // NOT auth-class: no reconnect event, and the account is not demoted.
    expect(sink.events.some(event => event.kind === "account_reconnect_required")).toBe(false)
  })

  test("all accounts failing PRE-content (mixed auth + generic) yields a typed child_failed naming the mix", async () => {
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: JSON.stringify({ type: "error", message: "stream disconnected" }), exitCode: 1 },
      ]),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-mixed", task: "go", onEvent: sink.onEvent })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("child_failed")
    expect(result.detail).toContain("all 2 available Codex session(s) failed before producing content")
    expect(result.detail).toContain("1 need reconnect")
    expect(result.detail).toContain("1 other pre-content failure(s)")
    expect(result.detail).toContain("stream disconnected")
    expect(sink.events.filter(event => event.kind === "account_reconnect_required")).toHaveLength(1)
    expect(sink.events.filter(event => event.kind === "pre_content_failure_rotated")).toHaveLength(1)
  })

  test("a POST-content failure is terminal child_failed and does NOT rotate (a partial child never double-runs)", async () => {
    let spawned = 0
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn(
        [{
          stdout: [
            JSON.stringify({ type: "thread.started", thread_id: "t-post" }),
            JSON.stringify({
              type: "item.completed",
              item: { id: "item_0", type: "agent_message", text: "partial answer before the failure" },
            }),
            JSON.stringify({ type: "error", message: "stream disconnected" }),
          ].join("\n"),
          exitCode: 1,
        }],
        () => {
          spawned += 1
        },
      ),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runChild({ childRef: "child-post", task: "go", onEvent: sink.onEvent })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("child_failed")
    expect(result.detail).toContain("stream disconnected")
    expect(result.accountRef).toBe("codex")
    expect(spawned).toBe(1)
    expect(sink.events.some(event => event.kind === "pre_content_failure_rotated")).toBe(false)
  })

  test("a clean exit with usage but no agent_message text is typed child_failed, never an empty success (post-content: usage was consumed)", async () => {
    let spawned = 0
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: JSON.stringify({ type: "turn.completed", usage: { input_tokens: 5 } }), exitCode: 0 }],
        () => {
          spawned += 1
        },
      ),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const result = await runtime.runChild({ childRef: "child-empty", task: "go" })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toBe("child_failed")
    expect(result.detail).toContain("no agent_message")
    // Usage was consumed (5 input tokens), so this is NOT pre-content.
    expect(spawned).toBe(1)
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
      health: makeCodexAccountHealth(),
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

  test("FRESHNESS + HEALTH (EP250): a mid-session reconnect ref is picked up AND ordered ahead of the auth-failed ref on the next call", async () => {
    // Owner receipt: codex-5 was registered at 15:30:40 but the 15:32
    // children never tried it. The registry read must happen per runChild —
    // never captured at construction — and the health memory must put the
    // freshly registered (untried) ref AHEAD of the auth-failed `codex` ref.
    const registered: CodexChildAccount[] = [
      { ref: "codex", home: "/isolated/accounts/codex/codex" },
    ]
    let discoverCalls = 0
    const spawnedHomes: string[] = []
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn(
        [
          // call 1: the only account is revoked -> typed all-revoked failure
          { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
          // call 2: codex-5 (untried, ordered FIRST) succeeds immediately —
          // the auth-failed `codex` ref is never re-burned.
          { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
        ],
        input => spawnedHomes.push(String(input.env.CODEX_HOME)),
      ),
      discoverImpl: async () => {
        discoverCalls += 1
        return [...registered]
      },
      health: makeCodexAccountHealth(),
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
    // Health ordering: the second call's FIRST attempt is codex-5 (untried
    // beats auth-failed); the broken codex home is not touched again.
    const attempts = sink.events.filter(event => event.kind === "attempt_started") as
      Array<Extract<CodexChildStreamEvent, { kind: "attempt_started" }>>
    expect(attempts[0]!.accountRef).toBe("codex-5")
    expect(spawnedHomes).toEqual([
      "/isolated/accounts/codex/codex",
      "/isolated/accounts/codex/codex-5",
    ])
  })
})

describe("makeCodexAccountHealth (in-process account health memory)", () => {
  const pool: ReadonlyArray<CodexChildAccount> = [
    { ref: "codex", home: "/h/codex" },
    { ref: "codex-2", home: "/h/codex-2" },
    { ref: "codex-5", home: "/h/codex-5" },
  ]

  test("orders last-known-good first (most recent success first), untried next, auth-failed LAST", () => {
    const health = makeCodexAccountHealth()
    health.recordAuthFailure("codex")
    health.recordSuccess("codex-5")
    expect(health.order(pool).map(account => account.ref)).toEqual(["codex-5", "codex-2", "codex"])
    // A newer success outranks an older one.
    health.recordSuccess("codex-2")
    expect(health.order(pool).map(account => account.ref)).toEqual(["codex-2", "codex-5", "codex"])
  })

  test("auth-failed accounts are still tried when they are all that is left (a reconnect may have fixed them), and a success clears the mark", () => {
    const health = makeCodexAccountHealth()
    health.recordAuthFailure("codex")
    // Still a candidate — LAST, never dropped.
    expect(health.order([pool[0]!]).map(account => account.ref)).toEqual(["codex"])
    expect(health.stateOf("codex")).toBe("auth_failed")
    // Success clears: the ref is promoted to last-known-good.
    health.recordSuccess("codex")
    expect(health.stateOf("codex")).toBe("last_good")
    expect(health.order(pool).map(account => account.ref)).toEqual(["codex", "codex-2", "codex-5"])
  })

  test("the runtime demotes an auth-failed account for the NEXT call (integration through runChild)", async () => {
    const health = makeCodexAccountHealth()
    const attemptsPerCall: string[][] = []
    let currentAttempts: string[] = []
    const runtime = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        // call 1: codex auth-fails (SHORT live variant), codex-2 succeeds.
        { stdout: fixtureCodexShortAuthStdout, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
        // call 2: first attempt succeeds — and must be codex-2 (last-good),
        // with the auth-failed codex demoted, not tried at all.
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
      health,
    })
    const onEvent = (event: CodexChildStreamEvent): void => {
      if (event.kind === "attempt_started") currentAttempts.push(event.accountRef)
    }

    const first = await runtime.runChild({ childRef: "call-1", task: "go", onEvent })
    attemptsPerCall.push(currentAttempts)
    currentAttempts = []
    if (!first.ok) throw new Error(`expected call 1 success, got ${first.reason}`)
    expect(first.accountRef).toBe("codex-2")
    expect(attemptsPerCall[0]).toEqual(["codex", "codex-2"])

    const second = await runtime.runChild({ childRef: "call-2", task: "go", onEvent })
    attemptsPerCall.push(currentAttempts)
    if (!second.ok) throw new Error(`expected call 2 success, got ${second.reason}`)
    // Last-known-good first; the broken codex ref stops burning attempts.
    expect(second.accountRef).toBe("codex-2")
    expect(attemptsPerCall[1]).toEqual(["codex-2"])
    expect(health.stateOf("codex")).toBe("auth_failed")
    expect(health.stateOf("codex-2")).toBe("last_good")
  })

  test("the shared module-level memory exists (main-process lifetime default)", () => {
    // makeCodexChildRuntime defaults to this instance so concurrent siblings
    // and subsequent calls share ordering. Tests inject their own.
    expect(typeof sharedCodexAccountHealth.order).toBe("function")
    expect(typeof sharedCodexAccountHealth.recordSuccess).toBe("function")
    expect(typeof sharedCodexAccountHealth.recordAuthFailure).toBe("function")
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
    const ownerHome = join(pylonHome, "owner")
    mkdirSync(ownerHome, { recursive: true })
    const env = { HOME: ownerHome, PYLON_HOME: pylonHome }

    const before = await discoverRegisteredCodexAccounts(env)
    expect(before.map(account => account.ref)).toEqual(["codex"])

    // Mid-session UI reconnect registers a new ref (the codex-5 case).
    const homeB = join(pylonHome, "accounts", "codex", "codex-5")
    mkdirSync(homeB, { recursive: true })
    writeConfig([{ ref: "codex", home: homeA }, { ref: "codex-5", home: homeB }])

    const after = await discoverRegisteredCodexAccounts(env)
    expect(after.map(account => account.ref)).toEqual(["codex", "codex-5"])
  })

  test("puts an ordinary authenticated ~/.codex session before Pylon fallbacks", async () => {
    const pylonHome = mkdtempSync(join(tmpdir(), "codex-child-current-registry-"))
    const ownerHome = join(pylonHome, "owner")
    const currentHome = join(ownerHome, ".codex")
    const fallbackHome = join(pylonHome, "accounts", "codex", "codex")
    mkdirSync(currentHome, { recursive: true })
    mkdirSync(fallbackHome, { recursive: true })
    writeFileSync(join(currentHome, "auth.json"), "{}")
    writeFileSync(join(pylonHome, "config.json"), JSON.stringify({
      dev: { accounts: [{ provider: "codex", ref: "codex", home: fallbackHome }] },
    }))

    const found = await discoverRegisteredCodexAccounts({ HOME: ownerHome, PYLON_HOME: pylonHome })
    expect(found.map(account => [account.ref, account.source])).toEqual([
      ["codex-current", "current_session"],
      ["codex", "pylon"],
    ])
  })
})
