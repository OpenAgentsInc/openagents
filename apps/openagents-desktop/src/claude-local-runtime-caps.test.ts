/**
 * Claude local runtime — EP250 wave-1 capability substrate (programmatic
 * oracles, no Electron window). Covers the four daily-coding capability gaps
 * this lane builds the typed runtime for:
 *
 * - J2/J4 plan + todo events: TodoWrite -> plan_updated; opt-in plan mode.
 * - G4 child steer/interrupt: steerChild reaches a running delegate child.
 * - A3 queue follow-up: enqueue while a turn streams, promote on idle.
 * - I2 user MCP servers: enabled stdio/http passthrough, failed-start and
 *   invalid-config -> mcp_server_unavailable with the turn still completing.
 *
 * These enforce openagents_desktop.chat.claude_local_runtime_capabilities.v1.
 */
import { describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  CLAUDE_LOCAL_MCP_SERVER_LIMIT,
  decodeClaudeLocalMcpServerConfigs,
  normalizeClaudeLocalMcpServers,
  type ClaudeLocalEvent,
  type ClaudeLocalMcpServerConfig,
} from "./claude-local-contract.ts"
import {
  CLAUDE_DELEGATE_TOOL_NAME,
  CLAUDE_LOCAL_MODEL,
  makeClaudeLocalRuntime,
  makeFixtureClaudeMcpFactory,
  type ClaudeDelegateRuntime,
  type ClaudeLocalQuery,
  type ClaudeLocalRuntime,
  type FixtureClaudeMcpTool,
} from "./claude-local-runtime.ts"

// ---------------------------------------------------------------------------
// Shared harness
// ---------------------------------------------------------------------------

const makeReadyRoot = (): string => {
  const root = mkdtempSync(join(tmpdir(), "claude-caps-homes-"))
  mkdirSync(join(root, ".claude-pylon-b"))
  writeFileSync(join(root, ".claude-pylon-b", "claude-oauth-token"), "sk-ant-oat01-b\n")
  return root
}

type CapturedQuery = { prompt: string | AsyncIterable<unknown>; options: Record<string, unknown> }

type Harness = {
  runtime: ClaudeLocalRuntime
  captured: CapturedQuery[]
}

const makeHarness = (input: {
  script: (captured: CapturedQuery) => AsyncIterable<unknown>
  delegate?: ClaudeDelegateRuntime
  userMcpServers?: () => ReadonlyArray<ClaudeLocalMcpServerConfig>
}): Harness => {
  const root = makeReadyRoot()
  const captured: CapturedQuery[] = []
  const query: ClaudeLocalQuery = call => {
    captured.push(call)
    return input.script(call)
  }
  const scratch = mkdtempSync(join(tmpdir(), "claude-caps-scratch-"))
  const runtime = makeClaudeLocalRuntime({
    scratchRoot: () => scratch,
    env: { PYLON_ACCOUNT_HOME_ROOT: root },
    queryImpl: async () => query,
    mcpImpl: async () => makeFixtureClaudeMcpFactory(),
    ...(input.delegate === undefined ? {} : { delegate: input.delegate }),
    ...(input.userMcpServers === undefined ? {} : { userMcpServers: input.userMcpServers }),
  })
  return { runtime, captured }
}

const collect = () => {
  const events: ClaudeLocalEvent[] = []
  return { events, emit: (event: ClaudeLocalEvent) => events.push(event) }
}

/** A deferred that resolves the first time `emit` sees a matching event. */
const waitFor = <K extends ClaudeLocalEvent["kind"]>(kind: K) => {
  let resolve!: (event: Extract<ClaudeLocalEvent, { kind: K }>) => void
  const promise = new Promise<Extract<ClaudeLocalEvent, { kind: K }>>(r => {
    resolve = r
  })
  const onEmit = (event: ClaudeLocalEvent): void => {
    if (event.kind === kind) resolve(event as Extract<ClaudeLocalEvent, { kind: K }>)
  }
  return { promise, onEmit }
}

const fixtureDelegateTool = (options: Record<string, unknown>): FixtureClaudeMcpTool => {
  const servers = options.mcpServers as Record<string, { tools: Array<FixtureClaudeMcpTool> }>
  const tool = servers.codex!.tools.find(candidate => candidate.name === "delegate")
  return tool!
}

const toolResultText = (raw: unknown): string => {
  const record = raw as { content?: Array<{ text?: unknown }>; isError?: unknown }
  return Array.isArray(record.content)
    ? record.content.map(part => (typeof part.text === "string" ? part.text : "")).join(" ")
    : ""
}

// ---------------------------------------------------------------------------
// J2/J4 — plan + todo events
// ---------------------------------------------------------------------------

describe("plan_updated (J2/J4): TodoWrite surfaces structured plan entries", () => {
  test("a TodoWrite tool call emits plan_updated with mapped entries AND the raw tool_use", async () => {
    const harness = makeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL }
        yield {
          type: "assistant",
          message: {
            content: [{
              type: "tool_use",
              id: "todo-1",
              name: "TodoWrite",
              input: {
                todos: [
                  { content: "Read the audit", status: "completed", activeForm: "Reading the audit" },
                  { content: "Build the substrate", status: "in_progress", activeForm: "Building" },
                  { content: "Write the tests", status: "pending", activeForm: "Writing tests" },
                ],
              },
            }],
          },
        }
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "t-plan", threadRef: "th-plan", history: [], message: "plan it", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    // The raw tool_use trace is still emitted (additive, not replaced).
    expect(sink.events.some(e => e.kind === "tool_use")).toBe(true)
    const plan = sink.events.find(e => e.kind === "plan_updated") as
      Extract<ClaudeLocalEvent, { kind: "plan_updated" }> | undefined
    expect(plan).toBeDefined()
    expect(plan!.entries).toEqual([
      { step: "Read the audit", status: "completed" },
      { step: "Build the substrate", status: "in_progress" },
      { step: "Write the tests", status: "pending" },
    ])
  })

  test("a non-TodoWrite tool never emits plan_updated; unknown todo status coerces to pending", async () => {
    const harness = makeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL }
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "r1", name: "Read", input: { file_path: "x" } }] },
        }
        yield {
          type: "assistant",
          message: {
            content: [{
              type: "tool_use", id: "todo-2", name: "TodoWrite",
              input: { todos: [{ content: "step", status: "bogus", activeForm: "Stepping" }] },
            }],
          },
        }
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
    })
    const sink = collect()
    await harness.runtime.runTurn({
      turnRef: "t-plan2", threadRef: "th-plan2", history: [], message: "go", emit: sink.emit,
    })
    const plans = sink.events.filter(e => e.kind === "plan_updated") as
      Array<Extract<ClaudeLocalEvent, { kind: "plan_updated" }>>
    expect(plans.length).toBe(1)
    expect(plans[0]!.entries).toEqual([{ step: "step", status: "pending" }])
  })

  test("default turn uses permissionMode default with ExitPlanMode disallowed", async () => {
    const harness = makeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL }
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    await harness.runtime.runTurn({
      turnRef: "t-d", threadRef: "th-d", history: [], message: "hi", emit: collect().emit,
    })
    const options = harness.captured[0]!.options
    expect(options.permissionMode).toBe("default")
    expect(options.disallowedTools).toContain("ExitPlanMode")
  })

  test("opt-in plan mode switches to permissionMode plan and allows ExitPlanMode", async () => {
    const harness = makeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL }
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    await harness.runtime.runTurn({
      turnRef: "t-p", threadRef: "th-p", history: [], message: "hi", emit: collect().emit, planMode: true,
    })
    const options = harness.captured[0]!.options
    expect(options.permissionMode).toBe("plan")
    expect(options.disallowedTools).not.toContain("ExitPlanMode")
    // Skill + EnterPlanMode stay disallowed even in plan mode.
    expect(options.disallowedTools).toContain("Skill")
    expect(options.disallowedTools).toContain("EnterPlanMode")
  })
})

// ---------------------------------------------------------------------------
// G4 — steer / interrupt a running delegate child
// ---------------------------------------------------------------------------

describe("steerChild (G4): reach a running delegate child", () => {
  /** A delegate whose child hangs until its abort signal fires. */
  const hangingDelegate = (): ClaudeDelegateRuntime => ({
    runChild: input =>
      new Promise(resolve => {
        input.signal?.addEventListener("abort", () =>
          resolve({ ok: false, reason: "child_failed", detail: "aborted", accountRef: null, durationMs: 1 }), { once: true })
      }),
  })

  const runWithHangingChild = (input: {
    delegate: ClaudeDelegateRuntime
    turnRef: string
    onChildStarted: (childRef: string, runtime: ClaudeLocalRuntime) => void
  }) => {
    const started = waitFor("child_started")
    const events: ClaudeLocalEvent[] = []
    const harness = makeHarness({
      delegate: input.delegate,
      script: async function* (call) {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL }
        const tool = fixtureDelegateTool(call.options)
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "d1", name: CLAUDE_DELEGATE_TOOL_NAME, input: { task: "long job" } }] },
        }
        const raw = await tool.handler({ task: "long job" }, {})
        yield {
          type: "user",
          message: { content: [{ type: "tool_result", tool_use_id: "d1", is_error: true, content: toolResultText(raw) }] },
        }
        yield { type: "result", subtype: "success", is_error: false, result: "turn done" }
      },
    })
    const donePromise = harness.runtime.runTurn({
      turnRef: input.turnRef, threadRef: `th-${input.turnRef}`, history: [], message: "delegate", emit: event => {
        events.push(event)
        started.onEmit(event)
      },
    })
    return { harness, events, started: started.promise, donePromise }
  }

  test("interrupt a running child emits child_steered(interrupted) and the turn completes", async () => {
    const delegate = hangingDelegate()
    const flow = runWithHangingChild({
      delegate,
      turnRef: "t-int",
      onChildStarted: () => {},
    })
    const child = await flow.started
    const outcome = flow.harness.runtime.steerChild({
      turnRef: "t-int", childRef: child.childRef, action: "interrupt",
    })
    expect(outcome).toEqual({ ok: true, outcome: "interrupted" })
    const result = await flow.donePromise
    expect(result.ok).toBe(true)
    const steered = flow.events.find(e => e.kind === "child_steered") as
      Extract<ClaudeLocalEvent, { kind: "child_steered" }> | undefined
    expect(steered).toBeDefined()
    expect(steered!.action).toBe("interrupt")
    expect(steered!.outcome).toBe("interrupted")
    expect(steered!.childRef).toBe(child.childRef)
  })

  test("message to a running child is honestly unsupported; a later interrupt still ends it", async () => {
    const delegate = hangingDelegate()
    const flow = runWithHangingChild({ delegate, turnRef: "t-msg", onChildStarted: () => {} })
    const child = await flow.started
    const messageOutcome = flow.harness.runtime.steerChild({
      turnRef: "t-msg", childRef: child.childRef, action: "message", body: "hurry up",
    })
    expect(messageOutcome).toEqual({ ok: true, outcome: "unsupported" })
    // The child is still running; interrupt to release the turn.
    flow.harness.runtime.steerChild({ turnRef: "t-msg", childRef: child.childRef, action: "interrupt" })
    await flow.donePromise
    const steered = flow.events.filter(e => e.kind === "child_steered") as
      Array<Extract<ClaudeLocalEvent, { kind: "child_steered" }>>
    expect(steered.map(e => e.outcome)).toEqual(["unsupported", "interrupted"])
    expect(steered[0]!.action).toBe("message")
  })

  test("an unknown child (or turn mismatch) returns not_found with no event", async () => {
    const delegate = hangingDelegate()
    const flow = runWithHangingChild({ delegate, turnRef: "t-nf", onChildStarted: () => {} })
    const child = await flow.started
    expect(flow.harness.runtime.steerChild({ turnRef: "t-nf", childRef: "child.codex.nope.9", action: "interrupt" }))
      .toEqual({ ok: false, outcome: "not_found" })
    // Right child, wrong turn -> not_found.
    expect(flow.harness.runtime.steerChild({ turnRef: "other-turn", childRef: child.childRef, action: "interrupt" }))
      .toEqual({ ok: false, outcome: "not_found" })
    // Clean up.
    flow.harness.runtime.steerChild({ turnRef: "t-nf", childRef: child.childRef, action: "interrupt" })
    await flow.donePromise
  })

  test("a whole-turn interrupt also aborts the running child", async () => {
    const delegate = hangingDelegate()
    const flow = runWithHangingChild({ delegate, turnRef: "t-whole", onChildStarted: () => {} })
    await flow.started
    expect(flow.harness.runtime.interrupt("t-whole")).toBe(true)
    const result = await flow.donePromise
    // The turn was interrupted; the hung child was released by the abort.
    expect(result.ok).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// A3 — queue follow-up while a turn streams
// ---------------------------------------------------------------------------

describe("queueFollowup (A3): enqueue during a turn, promote on idle", () => {
  const gatedHarness = () => {
    let releaseGate!: () => void
    const gate = new Promise<void>(r => {
      releaseGate = r
    })
    const started = waitFor("turn_started")
    const events: ClaudeLocalEvent[] = []
    const harness = makeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL }
        yield { type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: "working" } } }
        await gate
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
    })
    return { harness, events, gate: releaseGate, turnStarted: started, emit: (e: ClaudeLocalEvent) => { events.push(e); started.onEmit(e) } }
  }

  test("enqueue -> followup_queued; on turn end -> followup_promoted (ordered)", async () => {
    const h = gatedHarness()
    const done = h.harness.runtime.runTurn({
      turnRef: "t-q", threadRef: "th-q", history: [], message: "first", emit: h.emit,
    })
    await h.turnStarted.promise
    const queued = h.harness.runtime.queueFollowup({ threadRef: "th-q", message: "second message" })
    expect(queued).toEqual({ ok: true, queued: true, queueRef: queued.ok ? queued.queueRef : "", position: 1 })
    h.gate()
    await done
    const kinds = h.events.map(e => e.kind)
    // followup_queued lands while streaming; followup_promoted after completion.
    expect(kinds.indexOf("followup_queued")).toBeGreaterThanOrEqual(0)
    expect(kinds.indexOf("followup_promoted")).toBeGreaterThan(kinds.indexOf("turn_completed"))
    const promoted = h.events.find(e => e.kind === "followup_promoted") as
      Extract<ClaudeLocalEvent, { kind: "followup_promoted" }>
    expect(promoted.message).toBe("second message")
    expect(promoted.queueRef).toBe(queued.ok ? queued.queueRef : "")
  })

  test("two queued follow-ups get positions 1 and 2; only the first promotes on this turn end (FIFO)", async () => {
    const h = gatedHarness()
    const done = h.harness.runtime.runTurn({
      turnRef: "t-q2", threadRef: "th-q2", history: [], message: "first", emit: h.emit,
    })
    await h.turnStarted.promise
    const a = h.harness.runtime.queueFollowup({ threadRef: "th-q2", message: "msg A" })
    const b = h.harness.runtime.queueFollowup({ threadRef: "th-q2", message: "msg B" })
    expect(a.ok && a.position).toBe(1)
    expect(b.ok && b.position).toBe(2)
    h.gate()
    await done
    const promoted = h.events.filter(e => e.kind === "followup_promoted") as
      Array<Extract<ClaudeLocalEvent, { kind: "followup_promoted" }>>
    expect(promoted.length).toBe(1)
    expect(promoted[0]!.message).toBe("msg A")
  })

  test("queueFollowup with no live turn for the thread returns no_active_turn and emits nothing", async () => {
    const h = gatedHarness()
    const outcome = h.harness.runtime.queueFollowup({ threadRef: "th-idle", message: "nobody home" })
    expect(outcome).toEqual({ ok: false, queued: false, reason: "no_active_turn" })
    expect(h.events.length).toBe(0)
    h.gate()
  })
})

// ---------------------------------------------------------------------------
// I2 — user-configured MCP servers
// ---------------------------------------------------------------------------

describe("user MCP servers (I2): passthrough, failed-start, invalid config", () => {
  test("an enabled stdio server is merged into mcpServers and its tools are allow-listed via canUseTool", async () => {
    const configs: ReadonlyArray<ClaudeLocalMcpServerConfig> = [
      { name: "docs", transport: "stdio", enabled: true, command: "docs-mcp", args: ["--stdio"], env: { TOKEN: "x" } },
      { name: "off", transport: "stdio", enabled: false, command: "nope" },
    ]
    const harness = makeHarness({
      userMcpServers: () => configs,
      script: async function* (call) {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL, mcp_servers: [{ name: "docs", status: "connected" }] }
        // A tool from the configured server surfaces as mcp__docs__<tool>.
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "m1", name: "mcp__docs__search", input: { q: "x" } }] },
        }
        // Prove canUseTool ALLOWS the user server tool.
        const canUse = call.options.canUseTool as (n: string, i: Record<string, unknown>, e: unknown) => Promise<Record<string, unknown>>
        const decision = await canUse("mcp__docs__search", { q: "x" }, undefined)
        yield { type: "result", subtype: "success", is_error: false, result: JSON.stringify(decision) }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "t-mcp", threadRef: "th-mcp", history: [], message: "search docs", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    const servers = harness.captured[0]!.options.mcpServers as Record<string, Record<string, unknown>>
    expect(servers.docs).toEqual({ type: "stdio", command: "docs-mcp", args: ["--stdio"], env: { TOKEN: "x" } })
    expect(servers.off).toBeUndefined()
    // No delegate -> no delegate auto-allow list; user tools flow via canUseTool.
    expect(harness.captured[0]!.options.allowedTools).toBeUndefined()
    expect(result.ok && JSON.parse(result.text).behavior).toBe("allow")
    // No unavailable event for a connected server.
    expect(sink.events.some(e => e.kind === "mcp_server_unavailable")).toBe(false)
  })

  test("an http server is normalized to a {type:http,url,headers} config", async () => {
    const harness = makeHarness({
      userMcpServers: () => [
        { name: "remote", transport: "http", enabled: true, url: "https://mcp.example.com", headers: { Authorization: "Bearer y" } },
      ],
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL, mcp_servers: [{ name: "remote", status: "connected" }] }
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    await harness.runtime.runTurn({ turnRef: "t-http", threadRef: "th-http", history: [], message: "hi", emit: collect().emit })
    const servers = harness.captured[0]!.options.mcpServers as Record<string, Record<string, unknown>>
    expect(servers.remote).toEqual({ type: "http", url: "https://mcp.example.com", headers: { Authorization: "Bearer y" } })
  })

  test("a server the SDK reports as failed at init emits mcp_server_unavailable and the turn still completes", async () => {
    const harness = makeHarness({
      userMcpServers: () => [{ name: "docs", transport: "stdio", enabled: true, command: "docs-mcp" }],
      script: async function* () {
        yield {
          type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL,
          mcp_servers: [{ name: "docs", status: "failed" }],
        }
        yield { type: "result", subtype: "success", is_error: false, result: "turn still ran" }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "t-fail", threadRef: "th-fail", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    expect(result.ok && result.text).toBe("turn still ran")
    const unavailable = sink.events.find(e => e.kind === "mcp_server_unavailable") as
      Extract<ClaudeLocalEvent, { kind: "mcp_server_unavailable" }> | undefined
    expect(unavailable).toBeDefined()
    expect(unavailable!.name).toBe("docs")
    expect(unavailable!.reason).toBe("failed")
  })

  test("an invalid config emits mcp_server_unavailable up-front and never blocks the turn", async () => {
    const harness = makeHarness({
      userMcpServers: () => [
        { name: "bad name!", transport: "stdio", enabled: true, command: "x" },
        { name: "nostdio", transport: "stdio", enabled: true },
        { name: "codex", transport: "stdio", enabled: true, command: "x" },
      ],
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "s", model: CLAUDE_LOCAL_MODEL }
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "t-inv", threadRef: "th-inv", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    const names = (sink.events.filter(e => e.kind === "mcp_server_unavailable") as
      Array<Extract<ClaudeLocalEvent, { kind: "mcp_server_unavailable" }>>).map(e => e.name)
    expect(names).toContain("bad name!")
    expect(names).toContain("nostdio")
    expect(names).toContain("codex")
    // None of the invalid servers reached the SDK options.
    expect(harness.captured[0]!.options.mcpServers).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// FROZEN config-schema validation bounds
// ---------------------------------------------------------------------------

describe("ClaudeLocalMcpServerConfig schema + normalization bounds", () => {
  test("decodes a well-formed config list", () => {
    const decoded = decodeClaudeLocalMcpServerConfigs([
      { name: "a", transport: "stdio", enabled: true, command: "a" },
      { name: "b", transport: "http", enabled: false, url: "https://b" },
    ])
    expect(decoded).not.toBeNull()
    expect(decoded!.length).toBe(2)
  })

  test("rejects a list longer than the server cap", () => {
    const many = Array.from({ length: CLAUDE_LOCAL_MCP_SERVER_LIMIT + 1 }, (_v, i) => ({
      name: `s${i}`, transport: "stdio" as const, enabled: true, command: "x",
    }))
    expect(decodeClaudeLocalMcpServerConfigs(many)).toBeNull()
  })

  test("rejects an unknown transport and a non-boolean enabled", () => {
    expect(decodeClaudeLocalMcpServerConfigs([{ name: "a", transport: "sse", enabled: true, url: "https://a" }])).toBeNull()
    expect(decodeClaudeLocalMcpServerConfigs([{ name: "a", transport: "stdio", enabled: "yes", command: "a" }])).toBeNull()
  })

  test("normalize: skips disabled, rejects bad name / reserved / duplicate / missing transport fields", () => {
    const result = normalizeClaudeLocalMcpServers([
      { name: "good", transport: "stdio", enabled: true, command: "g" },
      { name: "good", transport: "stdio", enabled: true, command: "dup" },
      { name: "disabled", transport: "stdio", enabled: false, command: "x" },
      { name: "codex", transport: "stdio", enabled: true, command: "x" },
      { name: "bad/name", transport: "stdio", enabled: true, command: "x" },
      { name: "needscmd", transport: "stdio", enabled: true },
      { name: "needsurl", transport: "http", enabled: true, url: "ftp://nope" },
      { name: "remote", transport: "http", enabled: true, url: "https://ok" },
    ])
    expect(result.valid.map(s => s.name)).toEqual(["good", "remote"])
    const invalidNames = result.invalid.map(i => i.name)
    expect(invalidNames).toEqual(["good", "codex", "bad/name", "needscmd", "needsurl"])
  })
})
