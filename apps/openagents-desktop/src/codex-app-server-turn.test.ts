import { describe, expect, test } from "vite-plus/test"
import { EventEmitter } from "node:events"
import { PassThrough } from "node:stream"

import type { CodexAppServerSpawn } from "./codex-app-server-client.ts"
import { runCodexAppServerTurn } from "./codex-app-server-turn.ts"
import type { ClaudeLocalEvent } from "./claude-local-contract.ts"
import type {
  WorkbenchCommandItem,
  WorkbenchCompactionItem,
  WorkbenchFileChangeItem,
  WorkbenchHookItem,
  WorkbenchNoticeItem,
  WorkbenchReasoningItem,
  WorkbenchReviewItem,
  WorkbenchSleepItem,
  WorkbenchToolCallItem,
} from "./workbench-item-contract.ts"

const THREAD_ID = "thread-typed-payloads"
const TURN_ID = "turn-typed-payloads"

/**
 * Protocol-speaking fixture app-server (#8859): after turn/start it streams
 * item/started + item/completed notifications carrying the FULL wire fields
 * toolFacts() used to flatten, then completes the turn. Asserts the typed
 * WorkbenchItem payloads ride the tool_use/tool_result events additively.
 */
const makeTypedPayloadSpawn = (): CodexAppServerSpawn => () => {
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
  const notify = (method: string, params: unknown): void => write({ method, params })
  const itemPair = (item: Record<string, unknown>, startedOverrides: Record<string, unknown>): void => {
    notify("item/started", { threadId: THREAD_ID, turnId: TURN_ID, item: { ...item, ...startedOverrides } })
    notify("item/completed", { threadId: THREAD_ID, turnId: TURN_ID, item })
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
      if (message.method === "initialize" && typeof message.id === "number") {
        write({ id: message.id, result: {} })
      } else if (message.method === "thread/start" && typeof message.id === "number") {
        write({ id: message.id, result: { thread: { id: THREAD_ID } } })
      } else if (message.method === "turn/start" && typeof message.id === "number") {
        write({ id: message.id, result: { turn: { id: TURN_ID } } })
        const command = {
          id: "item-command",
          type: "commandExecution",
          command: "pnpm test --filter desktop",
          commandActions: [],
          cwd: "/safe/repo",
          durationMs: 950,
          exitCode: 0,
          aggregatedOutput: "42 tests passed",
          status: "completed",
          source: "agent",
        }
        notify("item/started", {
          threadId: THREAD_ID,
          turnId: TURN_ID,
          item: { ...command, status: "inProgress", exitCode: null, durationMs: null, aggregatedOutput: null },
        })
        notify("item/commandExecution/outputDelta", {
          threadId: THREAD_ID, turnId: TURN_ID, itemId: "item-command", delta: "running tests\n",
        })
        notify("item/commandExecution/outputDelta", {
          threadId: THREAD_ID, turnId: TURN_ID, itemId: "item-command", delta: "42 tests passed",
        })
        notify("item/completed", { threadId: THREAD_ID, turnId: TURN_ID, item: command })
        const fileChange = {
          id: "item-files",
          type: "fileChange",
          status: "completed",
          changes: [{
            path: "src/a.ts",
            kind: { type: "update" },
            diff: "--- a/src/a.ts\n+++ b/src/a.ts\n+added\n-removed\n",
          }],
        }
        notify("item/started", { threadId: THREAD_ID, turnId: TURN_ID, item: { ...fileChange, status: "inProgress", changes: [] } })
        notify("item/fileChange/patchUpdated", { threadId: THREAD_ID, turnId: TURN_ID, itemId: "item-files", changes: fileChange.changes })
        notify("item/completed", { threadId: THREAD_ID, turnId: TURN_ID, item: fileChange })
        notify("turn/diff/updated", {
          threadId: THREAD_ID,
          turnId: TURN_ID,
          diff: "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-removed\n+added\n",
        })
        itemPair({
          id: "item-mcp",
          type: "mcpToolCall",
          server: "stripe",
          tool: "createCharge",
          arguments: { amount: 42 },
          durationMs: 88,
          error: null,
          result: { content: [{ type: "text", text: "charge created" }] },
          status: "completed",
        }, { status: "inProgress", result: null, durationMs: null })
        itemPair({
          id: "item-web",
          type: "webSearch",
          query: "effect schema bounds",
          results: [{}, {}],
        }, {})
        notify("item/agentMessage/delta", { threadId: THREAD_ID, turnId: TURN_ID, delta: "done" })
        notify("turn/completed", { threadId: THREAD_ID, turn: { id: TURN_ID, status: "completed", error: null } })
      }
    }
  })
  return child as never
}

describe("codex-app-server-turn typed item payloads (#8859)", () => {
  test("item/started + item/completed emit typed WorkbenchItem payloads alongside the string summaries", async () => {
    const events: Array<ClaudeLocalEvent> = []
    const outcome = await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref",
      turnRef: "turn-ref",
      accountRef: "codex",
      prompt: "run the suite",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: makeTypedPayloadSpawn(),
    })
    expect(outcome.outcome).toBe("success")
    expect(outcome.text).toBe("done")

    const toolUses = events.filter(event => event.kind === "tool_use") as
      Array<Extract<ClaudeLocalEvent, { kind: "tool_use" }>>
    const toolResults = events.filter(event => event.kind === "tool_result") as
      Array<Extract<ClaudeLocalEvent, { kind: "tool_result" }>>
    const toolProgress = events.filter(event => event.kind === "tool_progress") as
      Array<Extract<ClaudeLocalEvent, { kind: "tool_progress" }>>
    expect(toolUses).toHaveLength(4)
    expect(toolResults).toHaveLength(5)
    expect(toolProgress).toHaveLength(4)

    // Backward compatibility: the string contract is unchanged.
    expect(toolUses[0]!.toolName).toBe("Bash")
    expect(toolUses[0]!.summary).toBe('{"command":"pnpm test --filter desktop"}')
    expect(toolUses[0]!.itemRef).toBe("item-command")
    expect(toolResults[0]!.summary).toBe("42 tests passed")
    expect(toolResults[0]!.itemRef).toBe("item-command")
    const commandProgress = toolProgress.filter(event => event.item.kind === "command")
    expect(commandProgress.map(event => (event.item as WorkbenchCommandItem).outputTail)).toEqual([
      "running tests\n",
      "running tests\n42 tests passed",
    ])

    // Command: started payload is running with no exit; completion carries
    // cwd/exitCode/durationMs/output tail/source — the fields toolFacts lost.
    const commandStarted = toolUses[0]!.item as WorkbenchCommandItem
    expect(commandStarted).toMatchObject({
      kind: "command", source: "codex", command: "pnpm test --filter desktop",
      cwd: "/safe/repo", status: "in_progress", exitCode: null,
    })
    const commandDone = toolResults[0]!.item as WorkbenchCommandItem
    expect(commandDone).toEqual({
      kind: "command",
      source: "codex",
      command: "pnpm test --filter desktop",
      cwd: "/safe/repo",
      status: "completed",
      exitCode: 0,
      durationMs: 950,
      outputTail: "42 tests passed",
      commandSource: "agent",
    })

    // File change: per-file path/kind/diff with derived counts — no more
    // "1 file change(s)" as the only surviving fact.
    expect(toolUses[1]!.toolName).toBe("FileChange")
    const files = toolResults[1]!.item as WorkbenchFileChangeItem
    expect(files.kind).toBe("fileChange")
    expect(files.changes).toHaveLength(1)
    expect(files.changes[0]).toMatchObject({ path: "src/a.ts", kind: "update", adds: 1, dels: 1 })
    expect(files.changes[0]!.diff).toContain("+added")
    const fileProgress = toolProgress.find(event => event.itemRef === "item-files")!.item as WorkbenchFileChangeItem
    expect(fileProgress.changes[0]).toMatchObject({ path: "src/a.ts", adds: 1, dels: 1 })
    const turnDiff = toolResults.find(event => event.itemRef === `turn-diff:${TURN_ID}`)!.item as WorkbenchFileChangeItem
    expect(turnDiff).toMatchObject({ kind: "fileChange", scope: "turn", status: "completed" })
    expect(turnDiff.changes[0]).toMatchObject({ path: "src/a.ts", adds: 1, dels: 1 })

    // MCP call: args/result/duration survive.
    const mcp = toolResults[2]!.item as WorkbenchToolCallItem
    expect(mcp).toMatchObject({
      kind: "toolCall", callKind: "mcp", server: "stripe", tool: "createCharge",
      durationMs: 88, resultSnippet: "charge created", status: "completed",
    })
    expect(mcp.args).toEqual([{ key: "amount", value: "42" }])

    // Web search: the query survives (it used to be dropped entirely).
    const web = toolResults[3]!.item as WorkbenchToolCallItem
    expect(web).toMatchObject({
      kind: "toolCall", callKind: "web", query: "effect schema bounds", resultCount: 2,
    })
  })
})

/**
 * The `plan` ThreadItem (T8 #8865): `{id, text, type: "plan"}`,
 * collaboration-mode plan write-ups. Previously `toolFacts()` had no case for
 * it, so the item was silently dropped between `item/started` and
 * `item/completed`. It now rides the SAME per-turn stable-key plan note as
 * `turn/plan/updated` (`plan_updated`), so both plan representations render
 * through one `DesktopPlanCard`.
 */
const makePlanItemSpawn = (): CodexAppServerSpawn => () => {
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
  const notify = (method: string, params: unknown): void => write({ method, params })
  let buffered = ""
  stdin.on("data", chunk => {
    buffered += chunk.toString("utf8")
    while (buffered.includes("\n")) {
      const newline = buffered.indexOf("\n")
      const line = buffered.slice(0, newline)
      buffered = buffered.slice(newline + 1)
      if (line === "") continue
      const message = JSON.parse(line) as Record<string, unknown>
      if (message.method === "initialize" && typeof message.id === "number") {
        write({ id: message.id, result: {} })
      } else if (message.method === "thread/start" && typeof message.id === "number") {
        write({ id: message.id, result: { thread: { id: THREAD_ID } } })
      } else if (message.method === "turn/start" && typeof message.id === "number") {
        write({ id: message.id, result: { turn: { id: TURN_ID } } })
        const plan = { id: "item-plan", type: "plan", text: "Investigate, then fix behind a flag." }
        notify("item/started", { threadId: THREAD_ID, turnId: TURN_ID, item: { ...plan, text: "" } })
        // A structured checklist arrives on the SAME turn via turn/plan/updated.
        notify("turn/plan/updated", {
          threadId: THREAD_ID, turnId: TURN_ID,
          plan: [{ step: "Reproduce the bug", status: "completed" }],
        })
        notify("item/completed", { threadId: THREAD_ID, turnId: TURN_ID, item: plan })
        notify("item/agentMessage/delta", { threadId: THREAD_ID, turnId: TURN_ID, delta: "done" })
        notify("turn/completed", { threadId: THREAD_ID, turn: { id: TURN_ID, status: "completed", error: null } })
      }
    }
  })
  return child as never
}

describe("codex-app-server-turn `plan` ThreadItem projection (T8 #8865)", () => {
  test("the dropped `plan` ThreadItem now emits plan_updated, merging onto the SAME plan note as the structured checklist", async () => {
    const events: Array<ClaudeLocalEvent> = []
    const outcome = await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref",
      turnRef: "turn-ref",
      accountRef: "codex",
      prompt: "investigate the bug",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: makePlanItemSpawn(),
    })
    expect(outcome.outcome).toBe("success")

    const planEvents = events.filter(event => event.kind === "plan_updated") as
      Array<Extract<ClaudeLocalEvent, { kind: "plan_updated" }>>
    expect(planEvents).toHaveLength(2)
    // turn/plan/updated: the structured checklist, no prose.
    expect(planEvents[0]).toMatchObject({ entries: [{ step: "Reproduce the bug", status: "completed" }] })
    expect(planEvents[0]!.prose).toBeUndefined()
    // The `plan` ThreadItem: prose only, no structured entries of its own —
    // `local-runtime-event-persistence.ts` merges it onto the SAME note.
    expect(planEvents[1]).toMatchObject({ entries: [], prose: "Investigate, then fix behind a flag." })
  })
})

/**
 * Protocol-speaking fixture app-server (T11 #8868): after turn/start it
 * streams `thread/tokenUsage/updated` (partial then fuller) and
 * `account/rateLimits/updated` (primary only, then secondary only — the
 * sparse-rolling-update shape the wire actually documents), then completes.
 */
const makeMeterSpawn = (): CodexAppServerSpawn => () => {
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
  const write = (message: unknown): void => { stdout.write(`${JSON.stringify(message)}\n`) }
  const notify = (method: string, params: unknown): void => write({ method, params })
  let buffered = ""
  stdin.on("data", chunk => {
    buffered += chunk.toString("utf8")
    while (buffered.includes("\n")) {
      const newline = buffered.indexOf("\n")
      const line = buffered.slice(0, newline)
      buffered = buffered.slice(newline + 1)
      if (line === "") continue
      const message = JSON.parse(line) as Record<string, unknown>
      if (message.method === "initialize" && typeof message.id === "number") {
        write({ id: message.id, result: {} })
      } else if (message.method === "thread/start" && typeof message.id === "number") {
        write({ id: message.id, result: { thread: { id: THREAD_ID } } })
      } else if (message.method === "turn/start" && typeof message.id === "number") {
        write({ id: message.id, result: { turn: { id: TURN_ID } } })
        // First rolling token-usage update omits reasoning/cached — an
        // honest partial snapshot, never a fabricated zero for them.
        notify("thread/tokenUsage/updated", {
          threadId: THREAD_ID, turnId: TURN_ID,
          tokenUsage: { last: { inputTokens: 100, outputTokens: 20, totalTokens: 120 } },
        })
        // Sparse rate-limit update: primary only this round.
        notify("account/rateLimits/updated", {
          rateLimits: { primary: { usedPercent: 12, windowDurationMins: 300 } },
        })
        // Second rolling token-usage update adds cached/reasoning + a new total.
        notify("thread/tokenUsage/updated", {
          threadId: THREAD_ID, turnId: TURN_ID,
          tokenUsage: {
            last: { inputTokens: 150, cachedInputTokens: 30, outputTokens: 25, reasoningOutputTokens: 5, totalTokens: 180 },
          },
        })
        // Second rate-limit update: secondary only — must not erase primary.
        notify("account/rateLimits/updated", {
          rateLimits: { secondary: { usedPercent: 4, resetsAt: 1_800_000_000 } },
        })
        notify("item/agentMessage/delta", { threadId: THREAD_ID, turnId: TURN_ID, delta: "done" })
        notify("turn/completed", { threadId: THREAD_ID, turn: { id: TURN_ID, status: "completed", error: null } })
      }
    }
  })
  return child as never
}

/**
 * T9 #8866: `item/autoApprovalReview/started|completed` (the Guardian
 * background auto-reviewer) were previously fully unconsumed — a real event
 * the app-server can send that the renderer never saw at all. This proves
 * both notifications now surface as bounded, read-only `lane_notice` events
 * (never a fabricated interactive "approval" the user cannot act on).
 */
const makeGuardianReviewSpawn = (): CodexAppServerSpawn => () => {
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
  const notify = (method: string, params: unknown): void => write({ method, params })
  let buffered = ""
  stdin.on("data", chunk => {
    buffered += chunk.toString("utf8")
    while (buffered.includes("\n")) {
      const newline = buffered.indexOf("\n")
      const line = buffered.slice(0, newline)
      buffered = buffered.slice(newline + 1)
      if (line === "") continue
      const message = JSON.parse(line) as Record<string, unknown>
      if (message.method === "initialize" && typeof message.id === "number") {
        write({ id: message.id, result: {} })
      } else if (message.method === "thread/start" && typeof message.id === "number") {
        write({ id: message.id, result: { thread: { id: THREAD_ID } } })
      } else if (message.method === "turn/start" && typeof message.id === "number") {
        write({ id: message.id, result: { turn: { id: TURN_ID } } })
        notify("item/autoApprovalReview/started", {
          threadId: THREAD_ID,
          turnId: TURN_ID,
          reviewId: "review-1",
          startedAtMs: 1,
          action: { type: "command", command: "rm -rf /tmp/scratch", cwd: "/safe/repo", source: "shell" },
          review: { status: "inProgress" },
        })
        notify("item/autoApprovalReview/completed", {
          threadId: THREAD_ID,
          turnId: TURN_ID,
          reviewId: "review-1",
          startedAtMs: 1,
          completedAtMs: 2,
          decisionSource: "agent",
          action: { type: "command", command: "rm -rf /tmp/scratch", cwd: "/safe/repo", source: "shell" },
          review: { status: "denied", rationale: "destructive path outside the workspace" },
        })
        notify("item/agentMessage/delta", { threadId: THREAD_ID, turnId: TURN_ID, delta: "done" })
        notify("turn/completed", { threadId: THREAD_ID, turn: { id: TURN_ID, status: "completed", error: null } })
      }
    }
  })
  return child as never
}

describe("codex-app-server-turn context/usage meter (T11 #8868)", () => {
  test("emits meter_updated events with exact wire values (no fabricated zeros) alongside existing accounting", async () => {
    const events: Array<ClaudeLocalEvent> = []
    const outcome = await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref",
      turnRef: "turn-ref",
      accountRef: "codex",
      prompt: "run the suite",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: makeMeterSpawn(),
    })
    expect(outcome.outcome).toBe("success")
    // The pre-existing internal-accounting field is unaffected by the new
    // additive event (still tracks the LATEST tokenUsage snapshot).
    expect(outcome.usage).toEqual({
      inputTokens: 150,
      cachedInputTokens: 30,
      outputTokens: 25,
      reasoningOutputTokens: 5,
      totalTokens: 180,
    })

    const meterEvents = events.filter(event => event.kind === "meter_updated") as
      Array<Extract<ClaudeLocalEvent, { kind: "meter_updated" }>>
    expect(meterEvents).toHaveLength(4)

    // First tokenUsage update: honest partial snapshot — no cachedInputTokens
    // or reasoningTokens key at all (never a fake 0).
    expect(meterEvents[0]).toEqual({
      kind: "meter_updated",
      inputTokens: 100,
      outputTokens: 20,
      totalTokens: 120,
    })
    expect(meterEvents[0]).not.toHaveProperty("cachedInputTokens")
    expect(meterEvents[0]).not.toHaveProperty("reasoningTokens")

    // First rate-limit update: primary only.
    expect(meterEvents[1]).toEqual({
      kind: "meter_updated",
      rateLimits: [{ label: "primary", usedPercent: 12, windowDurationMins: 300 }],
    })

    // Second tokenUsage update: the fuller snapshot.
    expect(meterEvents[2]).toEqual({
      kind: "meter_updated",
      inputTokens: 150,
      cachedInputTokens: 30,
      outputTokens: 25,
      reasoningTokens: 5,
      totalTokens: 180,
    })

    // Second rate-limit update: secondary only, per-event — the renderer
    // (not this emitter) is responsible for merging it with the earlier
    // primary window (see local-harness.test.ts mergeMeterSnapshot coverage).
    expect(meterEvents[3]).toEqual({
      kind: "meter_updated",
      rateLimits: [{ label: "secondary", usedPercent: 4, resetsAt: 1_800_000_000 }],
    })
  })

  test("emits no meter_updated event when a tokenUsage notification carries no usable fields", async () => {
    const events: Array<ClaudeLocalEvent> = []
    await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref-empty",
      turnRef: "turn-ref-empty",
      accountRef: "codex",
      prompt: "run the suite",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: (() => {
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
        child.kill = () => { child.emit("close", 0); return true }
        const write = (message: unknown): void => { stdout.write(`${JSON.stringify(message)}\n`) }
        const notify = (method: string, params: unknown): void => write({ method, params })
        let buffered = ""
        stdin.on("data", chunk => {
          buffered += chunk.toString("utf8")
          while (buffered.includes("\n")) {
            const newline = buffered.indexOf("\n")
            const line = buffered.slice(0, newline)
            buffered = buffered.slice(newline + 1)
            if (line === "") continue
            const message = JSON.parse(line) as Record<string, unknown>
            if (message.method === "initialize" && typeof message.id === "number") {
              write({ id: message.id, result: {} })
            } else if (message.method === "thread/start" && typeof message.id === "number") {
              write({ id: message.id, result: { thread: { id: "thread-ref-empty" } } })
            } else if (message.method === "turn/start" && typeof message.id === "number") {
              write({ id: message.id, result: { turn: { id: "turn-ref-empty" } } })
              notify("thread/tokenUsage/updated", { threadId: "thread-ref-empty", turnId: "turn-ref-empty", tokenUsage: {} })
              notify("item/agentMessage/delta", { threadId: "thread-ref-empty", turnId: "turn-ref-empty", delta: "done" })
              notify("turn/completed", { threadId: "thread-ref-empty", turn: { id: "turn-ref-empty", status: "completed", error: null } })
            }
          }
        })
        return child as never
      })(),
    })
    expect(events.filter(event => event.kind === "meter_updated")).toHaveLength(0)
  })
})

describe("codex app-server turn ownership", () => {
  test("quarantines pre-bind events and rejects stale or unaffiliated text from another chat", async () => {
    const currentThread = "thread-current"
    const currentTurn = "turn-current"
    const spawn: CodexAppServerSpawn = () => {
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
      child.kill = () => { child.emit("close", 0); return true }
      const write = (message: unknown): void => { stdout.write(`${JSON.stringify(message)}\n`) }
      const notify = (method: string, params: unknown): void => write({ method, params })
      let buffered = ""
      stdin.on("data", chunk => {
        buffered += chunk.toString("utf8")
        while (buffered.includes("\n")) {
          const newline = buffered.indexOf("\n")
          const line = buffered.slice(0, newline)
          buffered = buffered.slice(newline + 1)
          if (line === "") continue
          const request = JSON.parse(line) as Record<string, unknown>
          if (request.method === "initialize" && typeof request.id === "number") {
            write({ id: request.id, result: {} })
          } else if (request.method === "thread/start" && typeof request.id === "number") {
            // This is the dangerous reuse window: the new listener exists,
            // but its provider thread/turn identities do not yet exist.
            notify("item/agentMessage/delta", {
              threadId: "thread-previous",
              turnId: "turn-previous",
              delta: "PREVIOUS CHAT TEXT",
            })
            write({ id: request.id, result: { thread: { id: currentThread } } })
          } else if (request.method === "turn/start" && typeof request.id === "number") {
            write({ id: request.id, result: { turn: { id: currentTurn } } })
            // All three notifications can be parsed before the promise
            // continuation binds currentTurn, so replay must re-check them.
            notify("item/agentMessage/delta", { delta: "UNAFFILIATED TEXT" })
            notify("item/agentMessage/delta", {
              threadId: "thread-previous",
              turnId: "turn-previous",
              delta: "LATE PREVIOUS CHAT TEXT",
            })
            notify("item/agentMessage/delta", {
              threadId: currentThread,
              turnId: currentTurn,
              delta: "Current answer",
            })
            notify("turn/completed", {
              threadId: currentThread,
              turn: { id: currentTurn, status: "completed", error: null },
            })
          }
        }
      })
      return child as never
    }

    const events: Array<ClaudeLocalEvent> = []
    const outcome = await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "desktop-thread-current",
      turnRef: "desktop-turn-current",
      accountRef: "codex",
      prompt: "current prompt",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: spawn,
    })

    expect(outcome).toMatchObject({ outcome: "success", text: "Current answer" })
    expect(events.filter(event => event.kind === "text_delta")).toEqual([
      { kind: "text_delta", text: "Current answer" },
    ])
    expect(JSON.stringify(events)).not.toContain("PREVIOUS CHAT TEXT")
    expect(JSON.stringify(events)).not.toContain("UNAFFILIATED TEXT")
  })
})

// ---------------------------------------------------------------------------
// Streaming reasoning disclosure (#8863, epic #8857 T6). Reasoning rides the
// SAME started/progress/completed tool-card pairing infrastructure every
// other typed item uses (toolName "Reasoning", keyed by itemRef/itemId) so
// `tool-cards.ts` and `dispatch.tsx` need no reasoning-specific plumbing.
// ---------------------------------------------------------------------------
const THREAD_ID_2 = "thread-reasoning-stream"
const TURN_ID_2 = "turn-reasoning-stream"

const makeReasoningStreamSpawn = (): CodexAppServerSpawn => () => {
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
  const write = (message: unknown): void => { stdout.write(`${JSON.stringify(message)}\n`) }
  const notify = (method: string, params: unknown): void => write({ method, params })
  let buffered = ""
  stdin.on("data", chunk => {
    buffered += chunk.toString("utf8")
    while (buffered.includes("\n")) {
      const newline = buffered.indexOf("\n")
      const line = buffered.slice(0, newline)
      buffered = buffered.slice(newline + 1)
      if (line === "") continue
      const message = JSON.parse(line) as Record<string, unknown>
      if (message.method === "initialize" && typeof message.id === "number") {
        write({ id: message.id, result: {} })
      } else if (message.method === "thread/start" && typeof message.id === "number") {
        write({ id: message.id, result: { thread: { id: THREAD_ID_2 } } })
      } else if (message.method === "turn/start" && typeof message.id === "number") {
        write({ id: message.id, result: { turn: { id: TURN_ID_2 } } })
        // Streaming item: raw content deltas, a summary delta, a new summary
        // part boundary, then one more summary delta before it completes.
        notify("item/reasoning/textDelta", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2, itemId: "item-reasoning", contentIndex: 0, delta: "Checking the cache",
        })
        notify("item/reasoning/summaryTextDelta", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2, itemId: "item-reasoning", summaryIndex: 0, delta: " first.",
        })
        notify("item/reasoning/summaryPartAdded", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2, itemId: "item-reasoning", summaryIndex: 1,
        })
        notify("item/reasoning/summaryTextDelta", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2, itemId: "item-reasoning", summaryIndex: 1, delta: "Then verified the token.",
        })
        notify("item/completed", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2,
          item: {
            id: "item-reasoning", type: "reasoning",
            summary: ["Checked the cache first.", "Then verified the token was valid."],
          },
        })
        // A second reasoning item completes with NO prior streaming at all
        // (a fast turn where deltas never arrived) — the started+completed
        // pair should still emit together so the FIFO card pairing balances.
        notify("item/completed", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2,
          item: { id: "item-reasoning-instant", type: "reasoning", summary: ["Instant reasoning, no deltas."] },
        })
        // A third reasoning item is fully redacted — no deltas, no summary
        // on completion. Honest absence: no card at all, live or historical.
        notify("item/completed", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2,
          item: { id: "item-reasoning-redacted", type: "reasoning" },
        })
        // A fourth reasoning item streams ghost text but the completed
        // payload comes back with an empty summary array — fall back to the
        // already-streamed text rather than dropping it silently.
        notify("item/reasoning/textDelta", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2, itemId: "item-reasoning-fallback", contentIndex: 0, delta: "Streamed but never finalized.",
        })
        notify("item/completed", {
          threadId: THREAD_ID_2, turnId: TURN_ID_2,
          item: { id: "item-reasoning-fallback", type: "reasoning", summary: [] },
        })
        notify("item/agentMessage/delta", { threadId: THREAD_ID_2, turnId: TURN_ID_2, delta: "done" })
        notify("turn/completed", { threadId: THREAD_ID_2, turn: { id: TURN_ID_2, status: "completed", error: null } })
      }
    }
  })
  return child as never
}

describe("codex-app-server-turn streaming reasoning disclosure (#8863)", () => {
  test("reasoning text/summary deltas stream an in-progress card that collapses to the completed summary", async () => {
    const events: Array<ClaudeLocalEvent> = []
    const outcome = await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref",
      turnRef: "turn-ref",
      accountRef: "codex",
      prompt: "think it through",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: makeReasoningStreamSpawn(),
    })
    expect(outcome.outcome).toBe("success")

    type ReasoningTraceEvent = Extract<ClaudeLocalEvent, { kind: "tool_use" | "tool_progress" | "tool_result" }>
    const isReasoningTraceEvent = (event: ClaudeLocalEvent): event is ReasoningTraceEvent =>
      (event.kind === "tool_use" || event.kind === "tool_progress" || event.kind === "tool_result") &&
      event.toolName === "Reasoning"
    const reasoningEvents = events.filter(isReasoningTraceEvent)
    const byRef = (itemRef: string): Array<ReasoningTraceEvent> =>
      reasoningEvents.filter(event => event.itemRef === itemRef)

    // item-reasoning: opens on the first textDelta, updates in place through
    // the summary delta / part boundary / second summary delta, then
    // collapses to the authoritative completed summary.
    const streamed = byRef("item-reasoning")
    expect(streamed.map(event => event.kind)).toEqual([
      "tool_use", "tool_progress", "tool_progress", "tool_progress", "tool_result",
    ])
    expect(streamed.every(event => event.item !== undefined)).toBe(true)
    const streamedInProgress = streamed.slice(0, 4).map(event => event.item as WorkbenchReasoningItem)
    expect(streamedInProgress.every(item => item.status === "in_progress")).toBe(true)
    // Ghost text grows monotonically as deltas arrive.
    expect(streamedInProgress[0]!.summary).toBe("Checking the cache")
    expect(streamedInProgress[1]!.summary).toBe("Checking the cache first.")
    // summaryPartAdded inserts a paragraph break before the next delta.
    expect(streamedInProgress[2]!.summary).toBe("Checking the cache first.\n\n")
    expect(streamedInProgress[3]!.summary).toBe("Checking the cache first.\n\nThen verified the token.")
    const streamedDone = streamed.at(-1)!.item as WorkbenchReasoningItem
    expect(streamedDone).toEqual({
      kind: "reasoning",
      source: "codex",
      status: "completed",
      // The completed payload is authoritative over the streamed ghost text.
      summary: "Checked the cache first.\nThen verified the token was valid.",
    })

    // item-reasoning-instant: no deltas ever arrived (nothing was ever
    // mid-thought from this process's observation), so the started+
    // completed pair emits TOGETHER (FIFO card pairing balance) carrying the
    // SAME already-completed item — there is no honest "in_progress" moment
    // to represent, and the FIFO pairing in tool-cards.ts merges both notes
    // into one card keyed by itemRef regardless.
    const instant = byRef("item-reasoning-instant")
    expect(instant.map(event => event.kind)).toEqual(["tool_use", "tool_result"])
    for (const event of instant) {
      expect(event.item as WorkbenchReasoningItem).toMatchObject({
        status: "completed", summary: "Instant reasoning, no deltas.",
      })
    }

    // item-reasoning-redacted: no deltas, empty completed summary — honest
    // absence, not a single event.
    expect(byRef("item-reasoning-redacted")).toHaveLength(0)

    // item-reasoning-fallback: deltas streamed ghost text, but the completed
    // payload came back empty — falls back to the streamed text rather than
    // vanishing.
    const fallback = byRef("item-reasoning-fallback")
    expect(fallback.map(event => event.kind)).toEqual(["tool_use", "tool_result"])
    expect((fallback[1]!.item as WorkbenchReasoningItem)).toMatchObject({
      status: "completed", summary: "Streamed but never finalized.",
    })
  })
})

describe("codex-app-server-turn guardian approval-review notices (T9 #8866)", () => {
  test("item/autoApprovalReview/started|completed project as bounded lane_notice events, not a fabricated approval decision", async () => {
    const events: Array<ClaudeLocalEvent> = []
    const outcome = await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref",
      turnRef: "turn-ref",
      accountRef: "codex",
      prompt: "run a risky command",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: makeGuardianReviewSpawn(),
    })
    expect(outcome.outcome).toBe("success")

    // Guardian review is intentional product content. The fixture's
    // deliberately incomplete protocol responses also create private
    // compatibility receipts, but connection diagnostics must never become
    // transcript notices.
    const guardianNotices = (events.filter(event => event.kind === "lane_notice") as
      Array<Extract<ClaudeLocalEvent, { kind: "lane_notice" }>>)
      .filter(event => event.text.startsWith("Guardian review"))
    expect(guardianNotices).toHaveLength(2)
    expect(guardianNotices[0]!.text).toBe("Guardian review started: command: rm -rf /tmp/scratch")
    expect(guardianNotices[1]!.text).toBe(
      "Guardian review denied: command: rm -rf /tmp/scratch — destructive path outside the workspace",
    )
    expect(events.some(event =>
      event.kind === "lane_notice" && event.text.startsWith("Codex compatibility notice:"))).toBe(false)
    // No event pretends this is a user-actionable approval decision.
    expect(events.some(event => event.kind === "question_pending")).toBe(false)
  })
})

/**
 * Fixture app-server for the T12 long-tail + notice regression (#8869, epic
 * #8857 wave 2): streams `hookPrompt`/`sleep`/`enteredReviewMode`/
 * `exitedReviewMode`/`contextCompaction` item pairs — previously dropped
 * WHOLE by `toolFacts()` — plus the six previously-ignored notice-class
 * notifications (`thread/compacted`, `model/rerouted`, `warning`,
 * `configWarning`, `deprecationNotice`, `guardianWarning`).
 */
const makeLongTailAndNoticeSpawn = (): CodexAppServerSpawn => () => {
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
  const notify = (method: string, params: unknown): void => write({ method, params })
  const itemPair = (item: Record<string, unknown>): void => {
    notify("item/started", { threadId: THREAD_ID, turnId: TURN_ID, item })
    notify("item/completed", { threadId: THREAD_ID, turnId: TURN_ID, item })
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
      if (message.method === "initialize" && typeof message.id === "number") {
        write({ id: message.id, result: {} })
      } else if (message.method === "thread/start" && typeof message.id === "number") {
        write({ id: message.id, result: { thread: { id: THREAD_ID } } })
      } else if (message.method === "turn/start" && typeof message.id === "number") {
        write({ id: message.id, result: { turn: { id: TURN_ID } } })
        itemPair({
          id: "item-hook",
          type: "hookPrompt",
          fragments: [{ hookRunId: "run-1", text: "Guard fired before commit" }],
        })
        itemPair({ id: "item-sleep", type: "sleep", durationMs: 4_200 })
        itemPair({ id: "item-review-enter", type: "enteredReviewMode", review: "Review the diff before merge" })
        itemPair({ id: "item-review-exit", type: "exitedReviewMode", review: "Approved" })
        itemPair({ id: "item-compaction", type: "contextCompaction" })
        notify("thread/compacted", { threadId: THREAD_ID, turnId: TURN_ID })
        notify("model/rerouted", {
          threadId: THREAD_ID, turnId: TURN_ID,
          fromModel: "gpt-5.5", toModel: "gpt-5.5-safe", reason: "highRiskCyberActivity",
        })
        notify("warning", { message: "Sandbox is running with reduced isolation." })
        notify("configWarning", { summary: "Unknown config key", details: "profile.foo" })
        notify("deprecationNotice", { summary: "The --legacy-flag option is deprecated" })
        notify("guardianWarning", { threadId: THREAD_ID, message: "Guardian flagged a risky command." })
        notify("item/agentMessage/delta", { threadId: THREAD_ID, turnId: TURN_ID, delta: "done" })
        notify("turn/completed", { threadId: THREAD_ID, turn: { id: TURN_ID, status: "completed", error: null } })
      }
    }
  })
  return child as never
}

describe("codex-app-server-turn long-tail + notice honesty (#8869, T12 epic #8857 wave 2)", () => {
  test("hookPrompt, sleep, review-mode, contextCompaction, and every notice-class notification produce a rendering artifact — none are silently dropped", async () => {
    const events: Array<ClaudeLocalEvent> = []
    const outcome = await runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: {},
      workspace: "/safe/repo",
      threadRef: "thread-ref",
      turnRef: "turn-ref",
      accountRef: "codex",
      prompt: "run the suite",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.5",
      reasoningEffort: "medium",
      productSpecSkill: { skillRoot: "/skills", skillPath: "/skills/productspec-work" },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: makeLongTailAndNoticeSpawn(),
    })
    expect(outcome.outcome).toBe("success")

    const toolResults = events.filter(event => event.kind === "tool_result") as
      Array<Extract<ClaudeLocalEvent, { kind: "tool_result" }>>
    const itemOfKind = <K extends string>(kind: K): unknown =>
      toolResults.find(event => event.item?.kind === kind)?.item

    // The regression this test guards: every one of these ThreadItem variants
    // used to hit `toolFacts()`'s `default: return null` and vanish with zero
    // trace. Each must now produce SOME rendering artifact (a typed item).
    expect(itemOfKind("hook") as WorkbenchHookItem | undefined).toEqual({
      kind: "hook", source: "codex", text: "Guard fired before commit",
    })
    expect(itemOfKind("sleep") as WorkbenchSleepItem | undefined).toEqual({
      kind: "sleep", source: "codex", durationMs: 4_200,
    })
    const reviews = toolResults
      .map(event => event.item)
      .filter((item): item is WorkbenchReviewItem => item?.kind === "review")
    expect(reviews).toHaveLength(2)
    expect(reviews.find(item => item.phase === "entered")).toMatchObject({ review: "Review the diff before merge" })
    expect(reviews.find(item => item.phase === "exited")).toMatchObject({ review: "Approved" })

    // Two compaction rows: the item variant (item-compaction) and the
    // deprecated notification form (thread/compacted) — both honest.
    const compactions = toolResults
      .map(event => event.item)
      .filter((item): item is WorkbenchCompactionItem => item?.kind === "compaction")
    expect(compactions.length).toBeGreaterThanOrEqual(2)
    for (const compaction of compactions) expect(compaction).toEqual({ kind: "compaction", source: "codex" })

    // Every notice-class notification becomes a severity-carrying notice item
    // (model/rerouted, warning, configWarning, deprecationNotice,
    // guardianWarning — five distinct methods; thread/compacted is the
    // compaction-shaped exception counted above).
    const notices = toolResults
      .map(event => event.item)
      .filter((item): item is WorkbenchNoticeItem => item?.kind === "notice")
    expect(notices).toHaveLength(5)
    expect(notices.find(item => item.text.includes("MODEL REROUTED"))).toMatchObject({
      severity: "warning", text: "MODEL REROUTED · gpt-5.5 -> gpt-5.5-safe",
    })
    expect(notices.find(item => item.text.includes("reduced isolation"))).toMatchObject({ severity: "warning" })
    expect(notices.find(item => item.text.includes("Unknown config key"))).toMatchObject({
      severity: "warning", text: "Unknown config key: profile.foo",
    })
    expect(notices.find(item => item.text.includes("deprecated"))).toMatchObject({ severity: "info" })
    expect(notices.find(item => item.text.includes("Guardian flagged"))).toMatchObject({
      severity: "warning", text: "Guardian flagged a risky command.",
    })
  })
})
