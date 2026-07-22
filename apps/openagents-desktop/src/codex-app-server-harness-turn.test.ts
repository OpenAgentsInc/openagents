/**
 * HARN-09 (#9167) renderer-parity proof for the codex app-server lane: the
 * SAME scripted app-server wire is driven through (a) the legacy hand-written
 * path (`runCodexAppServerTurn`) and (b) the flag-on adapter path
 * (`runCodexAppServerHarnessTurn`), asserting the `ClaudeLocalEvent`
 * sequences are EQUAL — order, kinds, and payload fields — and the settled
 * outcomes are equal. Parity is by construction (the adapter route keeps the
 * legacy turn as display authority and suppresses every lowered duplicate),
 * and these tests pin that construction.
 */
import { describe, expect, test } from "vite-plus/test"
import { EventEmitter } from "node:events"
import { PassThrough } from "node:stream"

import type { CodexAppServerRequest, CodexAppServerSpawn } from "./codex-app-server-client.ts"
import { runCodexAppServerTurn, type RunCodexAppServerTurnInput } from "./codex-app-server-turn.ts"
import { runCodexAppServerHarnessTurn } from "./codex-app-server-harness-turn.ts"
import type { ClaudeLocalEvent } from "./claude-local-contract.ts"

const THREAD_ID = "thread-parity"
const TURN_ID = "turn-parity"
const CHILD_ID = "thread-parity-child"

type FixtureApi = Readonly<{
  notify: (method: string, params: unknown) => void
  /** Write a server->client request; `onResponse` fires when the client answers. */
  request: (method: string, params: unknown, onResponse: (result: unknown) => void) => void
}>

/**
 * Protocol-speaking scripted app-server (same shape as the #8859 fixture):
 * answers initialize/thread/start/turn/start, then hands control to the
 * scenario script. Each call returns a FRESH child, so the same script can
 * drive the legacy and adapter paths independently.
 */
const makeScriptedSpawn = (script: (api: FixtureApi) => void): CodexAppServerSpawn => () => {
  const stdin = new PassThrough()
  const stdout = new PassThrough()
  const child = new EventEmitter() as EventEmitter & {
    stdin: PassThrough
    stdout: PassThrough
    stderr: PassThrough
    kill: () => boolean
  }
  child.stdin = stdin
  child.stdout = stdout
  child.stderr = new PassThrough()
  child.kill = () => {
    child.emit("close", 0)
    return true
  }
  const write = (message: unknown): void => {
    stdout.write(`${JSON.stringify(message)}\n`)
  }
  let serverRequestId = 9_000
  const pendingServerRequests = new Map<number, (result: unknown) => void>()
  const api: FixtureApi = {
    notify: (method, params) => write({ method, params }),
    request: (method, params, onResponse) => {
      serverRequestId += 1
      pendingServerRequests.set(serverRequestId, onResponse)
      write({ id: serverRequestId, method, params })
    },
  }
  let buffered = ""
  stdin.on("data", chunk => {
    buffered += chunk.toString("utf8")
    while (buffered.includes("\n")) {
      const newline = buffered.indexOf("\n")
      const line = buffered.slice(0, newline)
      buffered = buffered.slice(newline + 1)
      if (line === "") continue
      const message = JSON.parse(line) as Record<string, unknown>
      if (typeof message.id === "number" && message.method === undefined) {
        // Response to a fixture-written server request.
        const pending = pendingServerRequests.get(message.id)
        pendingServerRequests.delete(message.id)
        pending?.(message.result)
        continue
      }
      if (message.method === "initialize" && typeof message.id === "number") {
        write({ id: message.id, result: {} })
      } else if (message.method === "thread/start" && typeof message.id === "number") {
        write({ id: message.id, result: { thread: { id: THREAD_ID } } })
      } else if (message.method === "turn/start" && typeof message.id === "number") {
        write({ id: message.id, result: { turn: { id: TURN_ID } } })
        script(api)
      } else if (typeof message.id === "number") {
        // Any other client request (e.g. turn/interrupt): acknowledge.
        write({ id: message.id, result: {} })
      }
    }
  })
  return child as never
}

type PathResult = Readonly<{ events: ReadonlyArray<ClaudeLocalEvent>; outcome: unknown }>

/**
 * DOCUMENTED BENIGN DIFFERENCE (the only one): `child_completed.durationMs`
 * is wall-clock time measured with `Date.now()` inside the shared display
 * code, so two separate runs of the same wire legitimately differ by a few
 * milliseconds. It is normalized to 0 on both sides before comparison; every
 * other payload field must match exactly.
 */
const normalizeWallClock = (events: ReadonlyArray<ClaudeLocalEvent>): ReadonlyArray<ClaudeLocalEvent> =>
  events.map(event =>
    event.kind === "child_completed" && typeof event.durationMs === "number"
      ? { ...event, durationMs: 0 }
      : event)

/** Run the same scripted wire through both paths with identical inputs. */
const runBothPaths = async (
  script: (api: FixtureApi) => void,
  overrides?: Partial<RunCodexAppServerTurnInput>,
): Promise<Readonly<{ legacy: PathResult; harness: PathResult }>> => {
  const runPath = async (
    impl: (input: RunCodexAppServerTurnInput) => Promise<unknown>,
  ): Promise<PathResult> => {
    const events: Array<ClaudeLocalEvent> = []
    const outcome = await impl({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref",
      turnRef: "turn-ref",
      accountRef: "codex",
      prompt: "run the parity scenario",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: makeScriptedSpawn(script),
      ...overrides,
    })
    return { events, outcome }
  }
  const legacy = await runPath(runCodexAppServerTurn)
  const harness = await runPath(
    runCodexAppServerHarnessTurn as (input: RunCodexAppServerTurnInput) => Promise<unknown>,
  )
  return { legacy, harness }
}

/** The full display surface in one turn: reasoning stream, command with
 * output deltas, file change with patch updates, turn diff, MCP call, web
 * search, plan, meter (usage + rate limits), guardian review notices, a
 * delegated child, streamed text, and completion. */
const richTurnScript = (api: FixtureApi): void => {
  const scoped = { threadId: THREAD_ID, turnId: TURN_ID }
  api.notify("item/reasoning/summaryTextDelta", { ...scoped, itemId: "item-r", delta: "thinking " })
  api.notify("item/reasoning/summaryPartAdded", { ...scoped, itemId: "item-r" })
  api.notify("item/reasoning/textDelta", { ...scoped, itemId: "item-r", delta: "harder" })
  api.notify("item/completed", {
    ...scoped,
    item: { id: "item-r", type: "reasoning", summary: ["thinking", "harder"] },
  })
  const command = {
    id: "item-command",
    type: "commandExecution",
    command: "pnpm test",
    cwd: "/safe/repo",
    durationMs: 500,
    exitCode: 0,
    aggregatedOutput: "3 tests passed",
    status: "completed",
    source: "agent",
  }
  api.notify("item/started", {
    ...scoped,
    item: { ...command, status: "inProgress", exitCode: null, durationMs: null, aggregatedOutput: null },
  })
  api.notify("item/commandExecution/outputDelta", { ...scoped, itemId: "item-command", delta: "3 tests passed" })
  api.notify("item/completed", { ...scoped, item: command })
  const fileChange = {
    id: "item-files",
    type: "fileChange",
    status: "completed",
    changes: [{ path: "src/a.ts", kind: { type: "update" }, diff: "+added\n-removed\n" }],
  }
  api.notify("item/started", { ...scoped, item: { ...fileChange, status: "inProgress", changes: [] } })
  api.notify("item/fileChange/patchUpdated", { ...scoped, itemId: "item-files", changes: fileChange.changes })
  api.notify("item/completed", { ...scoped, item: fileChange })
  api.notify("turn/diff/updated", {
    ...scoped,
    diff: "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-removed\n+added\n",
  })
  api.notify("item/started", {
    ...scoped,
    item: { id: "item-mcp", type: "mcpToolCall", server: "stripe", tool: "createCharge", status: "inProgress" },
  })
  api.notify("item/completed", {
    ...scoped,
    item: { id: "item-mcp", type: "mcpToolCall", server: "stripe", tool: "createCharge", status: "completed" },
  })
  api.notify("item/completed", {
    ...scoped,
    item: { id: "item-web", type: "webSearch", query: "effect schema bounds", results: [{}] },
  })
  api.notify("turn/plan/updated", {
    ...scoped,
    plan: [
      { step: "read the failing test", status: "completed" },
      { step: "fix the projector", status: "inProgress" },
    ],
  })
  api.notify("item/autoApprovalReview/started", {
    ...scoped,
    action: { type: "command", command: "pnpm test" },
    review: {},
  })
  api.notify("item/autoApprovalReview/completed", {
    ...scoped,
    action: { type: "command", command: "pnpm test" },
    review: { status: "approved", rationale: "test command" },
  })
  api.notify("thread/tokenUsage/updated", {
    ...scoped,
    tokenUsage: {
      last: { inputTokens: 900, cachedInputTokens: 600, outputTokens: 40, reasoningOutputTokens: 12, totalTokens: 952 },
    },
  })
  api.notify("account/rateLimits/updated", {
    rateLimits: { primary: { usedPercent: 41, resetsAt: 1_800_000_000, windowDurationMins: 300 } },
  })
  // Delegated child thread.
  api.notify("thread/started", {
    thread: { id: CHILD_ID, parentThreadId: THREAD_ID, preview: "delegate: audit the tests" },
  })
  api.notify("item/agentMessage/delta", { threadId: CHILD_ID, itemId: "child-m", delta: "child reply" })
  api.notify("thread/tokenUsage/updated", {
    threadId: CHILD_ID,
    tokenUsage: { last: { inputTokens: 10, cachedInputTokens: 0, outputTokens: 5, reasoningOutputTokens: 0, totalTokens: 15 } },
  })
  api.notify("turn/completed", { threadId: CHILD_ID, turn: { id: "child-turn", status: "completed", error: null } })
  // Main-thread text and completion.
  api.notify("item/agentMessage/delta", { ...scoped, itemId: "item-msg", delta: "parity " })
  api.notify("item/agentMessage/delta", { ...scoped, itemId: "item-msg", delta: "proven" })
  api.notify("turn/completed", { threadId: THREAD_ID, turn: { id: TURN_ID, status: "completed", error: null } })
}

describe("codex app-server harness turn (HARN-09 renderer parity)", () => {
  test("rich turn: adapter path emits the EXACT legacy event sequence and outcome", async () => {
    const { legacy, harness } = await runBothPaths(richTurnScript)
    expect(normalizeWallClock(harness.events)).toEqual(normalizeWallClock(legacy.events))
    expect(harness.outcome).toEqual(legacy.outcome)
    // The scenario really covered the display-only kinds the neutral core
    // stream cannot carry — otherwise this parity proof would be vacuous.
    const kinds = new Set(legacy.events.map(event => event.kind))
    for (const kind of [
      "text_delta",
      "tool_use",
      "tool_result",
      "tool_progress",
      "plan_updated",
      "meter_updated",
      "lane_notice",
      "child_started",
      "child_activity",
      "child_completed",
    ]) {
      expect(kinds).toContain(kind)
    }
    expect(legacy.outcome).toMatchObject({ outcome: "success", text: "parity proven" })
    // No doubled core events: exactly the legacy multiset, already asserted
    // by deep equality above; spot-check the terminal text count.
    const textEvents = harness.events.filter(event => event.kind === "text_delta")
    expect(textEvents).toHaveLength(2)
  })

  test("approval request routes through the SAME host reverse handler in both paths", async () => {
    const approvalCalls: Array<string> = []
    const script = (api: FixtureApi): void => {
      api.request(
        "item/commandExecution/requestApproval",
        { threadId: THREAD_ID, turnId: TURN_ID, callId: "call-1", command: "rm -rf ./scratch" },
        () => {
          api.notify("item/agentMessage/delta", { threadId: THREAD_ID, turnId: TURN_ID, delta: "approved and done" })
          api.notify("turn/completed", { threadId: THREAD_ID, turn: { id: TURN_ID, status: "completed", error: null } })
        },
      )
    }
    const onServerRequest = async (request: CodexAppServerRequest): Promise<unknown> => {
      approvalCalls.push(request.method)
      return { decision: "accept" }
    }
    const { legacy, harness } = await runBothPaths(script, { onServerRequest })
    expect(harness.events).toEqual(legacy.events)
    expect(harness.outcome).toEqual(legacy.outcome)
    expect(legacy.outcome).toMatchObject({ outcome: "success", text: "approved and done" })
    // Once per path — the adapter path never re-routes or drops the approval.
    expect(approvalCalls).toEqual([
      "item/commandExecution/requestApproval",
      "item/commandExecution/requestApproval",
    ])
  })

  test("failure turn classifies identically (rate limit)", async () => {
    const script = (api: FixtureApi): void => {
      api.notify("error", {
        threadId: THREAD_ID,
        turnId: TURN_ID,
        error: { message: "429 too many requests — rate limit reached" },
        willRetry: false,
      })
    }
    const { legacy, harness } = await runBothPaths(script)
    expect(harness.events).toEqual(legacy.events)
    expect(harness.outcome).toEqual(legacy.outcome)
    expect(legacy.outcome).toMatchObject({ outcome: "failed", rateLimited: true, preContent: true })
  })

  test("failed turn/completed classifies identically", async () => {
    const script = (api: FixtureApi): void => {
      api.notify("turn/completed", {
        threadId: THREAD_ID,
        turn: { id: TURN_ID, status: "failed", error: { message: "usage limit reached for this account" } },
      })
    }
    const { legacy, harness } = await runBothPaths(script)
    expect(harness.events).toEqual(legacy.events)
    expect(harness.outcome).toEqual(legacy.outcome)
    expect(legacy.outcome).toMatchObject({ outcome: "failed", quotaExhausted: true })
  })

  test("turn timeout settles identically through the bridge's no-terminal path", async () => {
    const script = (): void => {
      // Stream nothing: the host turn timer must settle the turn.
    }
    const { legacy, harness } = await runBothPaths(script, {
      turnTimeoutMs: 250,
      requestTimeoutMs: 2_000,
    })
    expect(harness.events).toEqual(legacy.events)
    expect(harness.outcome).toEqual(legacy.outcome)
    expect(legacy.outcome).toMatchObject({ outcome: "timeout" })
  })
})
