import { describe, expect, test } from "vite-plus/test"
import { EventEmitter } from "node:events"
import { PassThrough } from "node:stream"

import type { CodexAppServerSpawn } from "./codex-app-server-client.ts"
import { runCodexAppServerTurn } from "./codex-app-server-turn.ts"
import type { FableLocalEvent } from "./fable-local-contract.ts"
import type {
  WorkbenchCommandItem,
  WorkbenchFileChangeItem,
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
    const events: Array<FableLocalEvent> = []
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
      Array<Extract<FableLocalEvent, { kind: "tool_use" }>>
    const toolResults = events.filter(event => event.kind === "tool_result") as
      Array<Extract<FableLocalEvent, { kind: "tool_result" }>>
    const toolProgress = events.filter(event => event.kind === "tool_progress") as
      Array<Extract<FableLocalEvent, { kind: "tool_progress" }>>
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
