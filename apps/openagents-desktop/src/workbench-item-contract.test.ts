import { describe, expect, test } from "vite-plus/test"
import { decodeCurrentServerNotification } from "@openagentsinc/codex-app-server-protocol/decode"

import {
  WORKBENCH_ARG_LIMIT,
  WORKBENCH_COMMAND_LIMIT,
  WORKBENCH_DIFF_LIMIT,
  WORKBENCH_OUTPUT_TAIL_LIMIT,
  WORKBENCH_PLAN_ENTRY_LIMIT,
  WORKBENCH_PLAN_PROSE_LIMIT,
  WORKBENCH_PLAN_STEP_LIMIT,
  decodeWorkbenchItem,
  workbenchArgEntries,
  workbenchFileChangeItemFromDiff,
  workbenchItemFromThreadItem,
  workbenchItemSignature,
  workbenchPlanItemFromEntries,
  workbenchToolCallFromSdkUse,
  type WorkbenchAgentChild,
  type WorkbenchCommandItem,
  type WorkbenchFileChangeItem,
  type WorkbenchPlanItem,
  type WorkbenchToolCallItem,
} from "./workbench-item-contract.ts"

/** Structurally narrows a decoded WorkbenchItem to the "agent" variant for assertions below. */
type WorkbenchAgentItemLike = Readonly<{
  kind: "agent"
  source: string
  tool?: string
  prompt?: string
  status: string
  children?: ReadonlyArray<WorkbenchAgentChild>
  activityKind?: string
  agentPath?: string
  agentThreadId?: string
}>

/**
 * camelCase fixtures asserted VALID against the current-source app-server
 * wire schema (the generated `item/completed` notification documents, via
 * the protocol package's Ajv decoder), so the tolerant reader provably
 * tracks the real wire contract rather than a hand-invented shape.
 */
const wireCommandExecution = {
  id: "item-1",
  type: "commandExecution",
  command: "pnpm test --filter desktop",
  commandActions: [],
  cwd: "/safe/repo",
  durationMs: 950,
  exitCode: 0,
  aggregatedOutput: "42 tests passed\nDone in 0.95s",
  status: "completed",
  source: "agent",
}
const wireFileChange = {
  id: "item-2",
  type: "fileChange",
  status: "completed",
  changes: [
    {
      path: "src/a.ts",
      kind: { type: "update" },
      diff: "--- a/src/a.ts\n+++ b/src/a.ts\n+added line\n+another\n-removed line\n",
    },
    { path: "src/new.ts", kind: { type: "add" }, diff: "+++ b/src/new.ts\n+fresh\n" },
  ],
}
const wireMcpToolCall = {
  id: "item-3",
  type: "mcpToolCall",
  server: "stripe",
  tool: "createCharge",
  arguments: { amount: 42, memo: "invoice", dryRun: true },
  durationMs: 88,
  error: null,
  result: { content: [{ type: "text", text: "charge created" }] },
  status: "completed",
}
const wireDynamicToolCall = {
  id: "item-4",
  type: "dynamicToolCall",
  tool: "productspec_update",
  namespace: "productspec",
  arguments: { section: "risks" },
  durationMs: 12,
  success: true,
  status: "completed",
}
const wireWebSearch = {
  id: "item-5",
  type: "webSearch",
  query: "effect schema union bounds",
  results: [{}, {}, {}],
  action: null,
}
/** The `plan` ThreadItem variant (T8 #8865): a collaboration-mode plan
 * write-up. Wire shape is exactly `{id, text, type: "plan"}` — no structured
 * entries, unlike `turn/plan/updated`. */
const wirePlan = {
  id: "item-6",
  type: "plan",
  text: "Investigate the flaky auth test, then land the fix behind a flag.",
}
const wireCollabAgentToolCall = {
  id: "item-7",
  type: "collabAgentToolCall",
  agentsStates: {
    "thread-child-1": { status: "running", message: "Reading tests" },
    "thread-child-2": { status: "completed", message: null },
  },
  model: "gpt-5.6-sol",
  prompt: "Wire the file-change card to per-file diffs",
  receiverThreadIds: ["thread-child-1", "thread-child-2"],
  senderThreadId: "thread-parent",
  status: "inProgress",
  tool: "spawnAgent",
}
const wireSubAgentActivity = {
  id: "item-8",
  type: "subAgentActivity",
  agentPath: "timeline-builder/a11y-oracle",
  agentThreadId: "thread-child-3",
  kind: "interacted",
}

describe("WorkbenchItem projection from app-server (camelCase) wire items", () => {
  test("every fixture is valid against the generated current-source item/completed wire schema", () => {
    for (const fixture of [
      wireCommandExecution, wireFileChange, wireMcpToolCall, wireDynamicToolCall, wireWebSearch, wirePlan,
      wireCollabAgentToolCall, wireSubAgentActivity,
    ]) {
      const result = decodeCurrentServerNotification("item/completed", {
        threadId: "thread-1",
        turnId: "turn-1",
        completedAtMs: 1_752_000_000_000,
        item: fixture,
      })
      expect(result._tag, `wire-valid: ${fixture.type}`).toBe("Decoded")
    }
  })

  test("commandExecution keeps command, cwd, exitCode, durationMs, output tail, and source", () => {
    const item = workbenchItemFromThreadItem(wireCommandExecution, "codex") as WorkbenchCommandItem
    expect(item).toEqual({
      kind: "command",
      source: "codex",
      command: "pnpm test --filter desktop",
      cwd: "/safe/repo",
      status: "completed",
      exitCode: 0,
      durationMs: 950,
      outputTail: "42 tests passed\nDone in 0.95s",
      commandSource: "agent",
    })
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("commandExecution bounds long output to its tail and reports the cap", () => {
    const output = `${"discarded".repeat(600)}VERDICT`
    const item = workbenchItemFromThreadItem({ ...wireCommandExecution, aggregatedOutput: output }, "codex") as WorkbenchCommandItem
    expect(item.outputTail).toHaveLength(WORKBENCH_OUTPUT_TAIL_LIMIT)
    expect(item.outputTail?.endsWith("VERDICT")).toBe(true)
    expect(item.outputCapReached).toBe(true)
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("fileChange keeps per-file path, kind, diff, and derived add/del counts", () => {
    const item = workbenchItemFromThreadItem(wireFileChange, "codex") as WorkbenchFileChangeItem
    expect(item.kind).toBe("fileChange")
    expect(item.status).toBe("completed")
    expect(item.scope).toBe("item")
    expect(item.changes).toHaveLength(2)
    expect(item.changes[0]).toMatchObject({ path: "src/a.ts", kind: "update", adds: 2, dels: 1 })
    expect(item.changes[0]!.diff).toContain("+added line")
    expect(item.changes[1]).toMatchObject({ path: "src/new.ts", kind: "add", adds: 1, dels: 0 })
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("turn diffs and retained apply_patch text share bounded per-file projection", () => {
    const git = workbenchFileChangeItemFromDiff(
      "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-old\n+new\n" +
      "diff --git a/src/new.ts b/src/new.ts\nnew file mode 100644\n--- /dev/null\n+++ b/src/new.ts\n+fresh\n",
      "codex",
    )
    expect(git).toMatchObject({ kind: "fileChange", scope: "turn", status: "in_progress" })
    expect(git.changes).toMatchObject([
      { path: "src/a.ts", kind: "update", adds: 1, dels: 1 },
      { path: "src/new.ts", kind: "add", adds: 1, dels: 0 },
    ])

    const retained = workbenchItemFromThreadItem({
      type: "apply_patch",
      status: "completed",
      input: JSON.stringify({ patch: "*** Begin Patch\n*** Delete File: src/old.ts\n-old\n*** Update File: src/a.ts\n@@\n-before\n+after\n*** End Patch" }),
    }, "codex") as WorkbenchFileChangeItem
    expect(retained.scope).toBe("item")
    expect(retained.changes.map(change => [change.path, change.kind])).toEqual([
      ["src/old.ts", "delete"],
      ["src/a.ts", "update"],
    ])
  })

  test("mcpToolCall keeps server, tool, k/v args, result snippet, and duration", () => {
    const item = workbenchItemFromThreadItem(wireMcpToolCall, "codex") as WorkbenchToolCallItem
    expect(item).toMatchObject({
      kind: "toolCall",
      callKind: "mcp",
      server: "stripe",
      tool: "createCharge",
      durationMs: 88,
      status: "completed",
      resultSnippet: "charge created",
    })
    expect(item.args).toEqual([
      { key: "amount", value: "42" },
      { key: "memo", value: "invoice" },
      { key: "dryRun", value: "true" },
    ])
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("mcpToolCall failure keeps the typed error message", () => {
    const item = workbenchItemFromThreadItem({
      ...wireMcpToolCall,
      status: "failed",
      result: null,
      error: { message: "server unreachable" },
    }, "codex") as WorkbenchToolCallItem
    expect(item.status).toBe("failed")
    expect(item.errorMessage).toBe("server unreachable")
    expect(item.resultSnippet).toBeUndefined()
  })

  test("dynamicToolCall keeps tool, namespace, args, success, and duration", () => {
    const item = workbenchItemFromThreadItem(wireDynamicToolCall, "codex") as WorkbenchToolCallItem
    expect(item).toMatchObject({
      kind: "toolCall",
      callKind: "dynamic",
      tool: "productspec_update",
      namespace: "productspec",
      durationMs: 12,
      status: "completed",
    })
    expect(item.args).toEqual([{ key: "section", value: "risks" }])
  })

  test("webSearch keeps the query and result count", () => {
    const item = workbenchItemFromThreadItem(wireWebSearch, "codex") as WorkbenchToolCallItem
    expect(item).toMatchObject({
      kind: "toolCall",
      callKind: "web",
      query: "effect schema union bounds",
      resultCount: 3,
      status: "completed",
    })
  })

  test("imageGeneration and imageView project as image tool calls", () => {
    const generated = workbenchItemFromThreadItem({
      id: "i", type: "imageGeneration", result: "ok", revisedPrompt: "a calmer sky",
      savedPath: "/tmp/out.png", status: "completed",
    }, "codex") as WorkbenchToolCallItem
    expect(generated).toMatchObject({
      callKind: "image", tool: "imageGeneration", path: "/tmp/out.png", resultSnippet: "a calmer sky",
    })
    const viewed = workbenchItemFromThreadItem(
      { id: "i", type: "imageView", path: "/tmp/in.png" },
      "codex",
    ) as WorkbenchToolCallItem
    expect(viewed).toMatchObject({ callKind: "image", tool: "imageView", path: "/tmp/in.png" })
  })

  test("collabAgentToolCall keeps tool, prompt, and per-agent children from agentsStates{}", () => {
    const item = workbenchItemFromThreadItem(wireCollabAgentToolCall, "codex") as WorkbenchAgentItemLike
    expect(item).toMatchObject({
      kind: "agent",
      source: "codex",
      tool: "spawnAgent",
      prompt: "Wire the file-change card to per-file diffs",
      status: "in_progress",
    })
    expect(item.children).toEqual([
      { threadRef: "thread-child-1", status: "running" },
      { threadRef: "thread-child-2", status: "completed" },
    ])
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("collabAgentToolCall status inProgress/completed/failed normalize to the coarse bucket", () => {
    for (const [wire, expected] of [["inProgress", "in_progress"], ["completed", "completed"], ["failed", "failed"]] as const) {
      const item = workbenchItemFromThreadItem({ ...wireCollabAgentToolCall, status: wire }, "codex") as WorkbenchAgentItemLike
      expect(item.status).toBe(expected)
    }
  })

  test("subAgentActivity keeps agentPath, activityKind, and the single agentThreadId as a child row", () => {
    const item = workbenchItemFromThreadItem(wireSubAgentActivity, "codex") as WorkbenchAgentItemLike
    expect(item).toMatchObject({
      kind: "agent",
      source: "codex",
      status: "in_progress",
      agentPath: "timeline-builder/a11y-oracle",
      agentThreadId: "thread-child-3",
      activityKind: "interacted",
    })
    expect(item.children).toEqual([{ threadRef: "thread-child-3", status: "running" }])
    expect(item.tool).toBeUndefined()
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("subAgentActivity kind=interrupted maps to a failed item status and an interrupted child row", () => {
    const item = workbenchItemFromThreadItem(
      { ...wireSubAgentActivity, kind: "interrupted" },
      "codex",
    ) as WorkbenchAgentItemLike
    expect(item.status).toBe("failed")
    expect(item.activityKind).toBe("interrupted")
    expect(item.children).toEqual([{ threadRef: "thread-child-3", status: "interrupted" }])
  })

  test("non-tool items project as null (this wave emits tool-class and agent-class payloads only)", () => {
    expect(workbenchItemFromThreadItem({ id: "x", type: "agentMessage", text: "hi" }, "codex")).toBeNull()
    expect(workbenchItemFromThreadItem({ id: "x", type: "reasoning" }, "codex")).toBeNull()
    expect(workbenchItemFromThreadItem({ id: "x", type: "contextCompaction" }, "codex")).toBeNull()
  })

  test("the `plan` ThreadItem projects as a prose-only plan item (T8 #8865) instead of being dropped", () => {
    const item = workbenchItemFromThreadItem(wirePlan, "codex") as WorkbenchPlanItem
    expect(item).toEqual({
      kind: "plan",
      source: "codex",
      entries: [],
      prose: "Investigate the flaky auth test, then land the fix behind a flag.",
    })
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("a `plan` ThreadItem with empty/absent text is honestly dropped (nothing to show)", () => {
    expect(workbenchItemFromThreadItem({ id: "x", type: "plan", text: "" }, "codex")).toBeNull()
    expect(workbenchItemFromThreadItem({ id: "x", type: "plan" }, "codex")).toBeNull()
    expect(workbenchItemFromThreadItem({ id: "x", type: "plan", text: "   " }, "codex")).toBeNull()
  })
})

describe("workbenchPlanItemFromEntries (T8 #8865, the one plan-item constructor)", () => {
  test("carries structured entries, prose, or both", () => {
    expect(workbenchPlanItemFromEntries({
      source: "codex",
      entries: [{ step: "Read the audit", status: "completed" }, { step: "Ship it", status: "pending" }],
    })).toEqual({
      kind: "plan",
      source: "codex",
      entries: [{ step: "Read the audit", status: "completed" }, { step: "Ship it", status: "pending" }],
    })
    expect(workbenchPlanItemFromEntries({ source: "local", entries: [], prose: "Plan mode write-up." })).toEqual({
      kind: "plan",
      source: "local",
      entries: [],
      prose: "Plan mode write-up.",
    })
    expect(workbenchPlanItemFromEntries({
      source: "claude",
      entries: [{ step: "a", status: "in_progress" }],
      prose: "Narrative alongside the checklist.",
    })).toEqual({
      kind: "plan",
      source: "claude",
      entries: [{ step: "a", status: "in_progress" }],
      prose: "Narrative alongside the checklist.",
    })
  })

  test("bounds entry count, step length, and prose length; drops blank prose", () => {
    const entries = Array.from({ length: WORKBENCH_PLAN_ENTRY_LIMIT + 20 }, (_, index) => ({
      step: `step ${index}`,
      status: "pending" as const,
    }))
    const bounded = workbenchPlanItemFromEntries({
      source: "codex",
      entries: [{ step: "x".repeat(WORKBENCH_PLAN_STEP_LIMIT + 100), status: "completed" }, ...entries],
      prose: "y".repeat(WORKBENCH_PLAN_PROSE_LIMIT + 100),
    })
    expect(bounded.entries).toHaveLength(WORKBENCH_PLAN_ENTRY_LIMIT)
    expect(bounded.entries[0]!.step).toHaveLength(WORKBENCH_PLAN_STEP_LIMIT)
    expect(bounded.prose).toHaveLength(WORKBENCH_PLAN_PROSE_LIMIT)
    expect(decodeWorkbenchItem(bounded)).not.toBeNull()

    expect(workbenchPlanItemFromEntries({ source: "codex", entries: [], prose: "   " }).prose).toBeUndefined()
    expect(workbenchPlanItemFromEntries({ source: "codex", entries: [] }).prose).toBeUndefined()
  })

  test("applies the redactor to both entry steps and prose", () => {
    const redact = (value: string): string => value.replaceAll("secret", "[REDACTED]")
    const item = workbenchPlanItemFromEntries({
      source: "codex",
      entries: [{ step: "read the secret file", status: "pending" }],
      prose: "the plan touches a secret",
    }, redact)
    expect(item.entries[0]!.step).toBe("read the [REDACTED] file")
    expect(item.prose).toBe("the plan touches a [REDACTED]")
  })
})

describe("WorkbenchItem projection from rollout/exec (snake_case) records", () => {
  test("command_execution reads exit_code, duration_ms, and aggregated_output", () => {
    const item = workbenchItemFromThreadItem({
      type: "command_execution",
      command: "echo fixture",
      cwd: "/safe/repo",
      exit_code: 1,
      duration_ms: 40,
      aggregated_output: "boom",
      status: "failed",
    }, "codex") as WorkbenchCommandItem
    expect(item).toMatchObject({
      kind: "command", command: "echo fixture", cwd: "/safe/repo",
      exitCode: 1, durationMs: 40, outputTail: "boom", status: "failed",
    })
  })

  test("mcp_tool_call and custom_tool_call read snake_case tool identity", () => {
    const mcp = workbenchItemFromThreadItem({
      type: "mcp_tool_call", server: "linear", tool_name: "createIssue",
      arguments: { title: "bug" }, status: "completed",
    }, "codex") as WorkbenchToolCallItem
    expect(mcp).toMatchObject({ callKind: "mcp", server: "linear", tool: "createIssue" })
    const dynamic = workbenchItemFromThreadItem({
      type: "custom_tool_call", name: "lint", input: { fix: true }, success: false,
    }, "codex") as WorkbenchToolCallItem
    expect(dynamic).toMatchObject({ callKind: "dynamic", tool: "lint", status: "failed" })
    expect(dynamic.args).toEqual([{ key: "fix", value: "true" }])
  })

  test("in_progress and inProgress statuses normalize identically", () => {
    for (const status of ["in_progress", "inProgress"]) {
      const item = workbenchItemFromThreadItem(
        { type: "command_execution", command: "sleep 1", status },
        "codex",
      ) as WorkbenchCommandItem
      expect(item.status).toBe("in_progress")
    }
  })

  test("collab_agent_tool_call and sub_agent_activity read snake_case identity fields", () => {
    const collab = workbenchItemFromThreadItem({
      type: "collab_agent_tool_call",
      tool: "closeAgent",
      prompt: "Wrap up",
      status: "completed",
      agents_states: { "child-1": { status: "shutdown" } },
    }, "codex") as WorkbenchAgentItemLike
    expect(collab).toMatchObject({ kind: "agent", tool: "closeAgent", status: "completed" })
    expect(collab.children).toEqual([{ threadRef: "child-1", status: "shutdown" }])
    const activity = workbenchItemFromThreadItem({
      type: "sub_agent_activity",
      agent_path: "root/child",
      agent_thread_id: "child-2",
      kind: "started",
    }, "codex") as WorkbenchAgentItemLike
    expect(activity).toMatchObject({ kind: "agent", agentPath: "root/child", agentThreadId: "child-2", activityKind: "started" })
  })
})

describe("bounds and redaction discipline", () => {
  test("oversize fields are bounded at emission: command head, output TAIL, diff head", () => {
    const item = workbenchItemFromThreadItem({
      type: "commandExecution",
      command: "x".repeat(WORKBENCH_COMMAND_LIMIT + 500),
      aggregatedOutput: `${"y".repeat(WORKBENCH_OUTPUT_TAIL_LIMIT)}TAIL-MARKER`,
      status: "completed",
    }, "codex") as WorkbenchCommandItem
    expect(item.command).toHaveLength(WORKBENCH_COMMAND_LIMIT)
    expect(item.outputTail).toHaveLength(WORKBENCH_OUTPUT_TAIL_LIMIT)
    expect(item.outputTail!.endsWith("TAIL-MARKER")).toBe(true)
    expect(decodeWorkbenchItem(item)).not.toBeNull()
    const fileChange = workbenchItemFromThreadItem({
      type: "fileChange",
      status: "completed",
      changes: [{ path: "big.ts", kind: { type: "update" }, diff: "+".repeat(WORKBENCH_DIFF_LIMIT + 9) }],
    }, "codex") as WorkbenchFileChangeItem
    expect(fileChange.changes[0]!.diff).toHaveLength(WORKBENCH_DIFF_LIMIT)
    expect(fileChange.changes[0]!.diffCapReached).toBe(true)
  })

  test("arg projection caps entry count and value length", () => {
    const wide: Record<string, unknown> = {}
    for (let index = 0; index < WORKBENCH_ARG_LIMIT + 10; index++) wide[`k${index}`] = "v".repeat(999)
    const entries = workbenchArgEntries(wide)
    expect(entries).toHaveLength(WORKBENCH_ARG_LIMIT)
    expect(entries[0]!.value).toHaveLength(400)
  })

  test("the schema rejects out-of-bounds payloads (emitters must bound first)", () => {
    expect(decodeWorkbenchItem({
      kind: "command", source: "codex", status: "completed",
      command: "x".repeat(WORKBENCH_COMMAND_LIMIT + 1),
    })).toBeNull()
    expect(decodeWorkbenchItem({ kind: "mystery", source: "codex" })).toBeNull()
  })

  test("the lane redactor is applied to commands, paths, args, and outputs", () => {
    const redact = (value: string): string => value.replaceAll("secret", "[REDACTED]")
    const command = workbenchItemFromThreadItem({
      type: "command_execution", command: "echo secret", cwd: "/home/secret",
      aggregated_output: "the secret leaked", status: "completed",
    }, "codex", redact) as WorkbenchCommandItem
    expect(command.command).toBe("echo [REDACTED]")
    expect(command.cwd).toBe("/home/[REDACTED]")
    expect(command.outputTail).toBe("the [REDACTED] leaked")
    const mcp = workbenchItemFromThreadItem({
      type: "mcpToolCall", server: "s", tool: "t", arguments: { token: "secret" }, status: "completed",
    }, "codex", redact) as WorkbenchToolCallItem
    expect(mcp.args).toEqual([{ key: "token", value: "[REDACTED]" }])
  })

  test("collabAgentToolCall caps agentsStates{} children at 16 and drops unrecognized statuses", () => {
    const agentsStates: Record<string, unknown> = {}
    for (let index = 0; index < 24; index++) agentsStates[`child-${index}`] = { status: "running" }
    agentsStates["child-bogus"] = { status: "not-a-real-status" }
    const item = workbenchItemFromThreadItem(
      { type: "collabAgentToolCall", tool: "wait", status: "inProgress", agentsStates },
      "codex",
    ) as WorkbenchAgentItemLike
    expect(item.children).toHaveLength(16)
    expect(decodeWorkbenchItem(item)).not.toBeNull()
  })

  test("the redactor applies to collabAgentToolCall.prompt and subAgentActivity.agentPath", () => {
    const redact = (value: string): string => value.replaceAll("secret", "[REDACTED]")
    const collab = workbenchItemFromThreadItem({
      type: "collabAgentToolCall", tool: "spawnAgent", prompt: "hold the secret", status: "inProgress",
    }, "codex", redact) as WorkbenchAgentItemLike
    expect(collab.prompt).toBe("hold the [REDACTED]")
    const activity = workbenchItemFromThreadItem({
      type: "subAgentActivity", agentPath: "secret/child", agentThreadId: "c", kind: "started",
    }, "codex", redact) as WorkbenchAgentItemLike
    expect(activity.agentPath).toBe("[REDACTED]/child")
  })
})

describe("Claude SDK lane projection (source-tagged, no Codex assumptions)", () => {
  test("mcp__server__tool names project as MCP calls with the server segment", () => {
    const item = workbenchToolCallFromSdkUse({
      toolName: "mcp__linear__createIssue",
      input: { title: "bug" },
      status: "in_progress",
    })
    expect(item).toMatchObject({
      kind: "toolCall", source: "claude", callKind: "mcp",
      server: "linear", tool: "createIssue", status: "in_progress",
    })
    expect(item.args).toEqual([{ key: "title", value: "bug" }])
  })

  test("plain SDK tools project as dynamic calls with result/error on completion", () => {
    const ok = workbenchToolCallFromSdkUse({
      toolName: "Bash", input: { command: "ls" }, status: "completed", resultSnippet: "README.md",
    })
    expect(ok).toMatchObject({
      callKind: "dynamic", tool: "Bash", source: "claude",
      status: "completed", resultSnippet: "README.md",
    })
    const failed = workbenchToolCallFromSdkUse({
      toolName: "Bash", input: {}, status: "failed", errorMessage: "exit 127",
    })
    expect(failed).toMatchObject({ status: "failed", errorMessage: "exit 127" })
    expect(decodeWorkbenchItem(ok)).not.toBeNull()
  })
})

describe("workbenchItemSignature (cheap memo equality)", () => {
  test("flips on status and content-length changes, stable otherwise", () => {
    const running = workbenchItemFromThreadItem(
      { type: "commandExecution", command: "pnpm test", status: "inProgress" },
      "codex",
    )!
    const done = workbenchItemFromThreadItem(
      { type: "commandExecution", command: "pnpm test", status: "completed", exitCode: 0 },
      "codex",
    )!
    expect(workbenchItemSignature(running)).not.toBe(workbenchItemSignature(done))
    expect(workbenchItemSignature(running)).toBe(workbenchItemSignature({ ...running }))
    expect(workbenchItemSignature(undefined)).toBe("")
  })

  test("plan signature flips on prose changes even with identical entries (T8 #8865)", () => {
    const withoutProse = workbenchPlanItemFromEntries({ source: "codex", entries: [{ step: "a", status: "pending" }] })
    const withProse = workbenchPlanItemFromEntries({ source: "codex", entries: [{ step: "a", status: "pending" }], prose: "narrative" })
    expect(workbenchItemSignature(withoutProse)).not.toBe(workbenchItemSignature(withProse))
    expect(workbenchItemSignature(withProse)).toBe(workbenchItemSignature({ ...withProse }))
  })

  test("agent items flip when a child's status changes, stable otherwise", () => {
    const running = workbenchItemFromThreadItem(wireCollabAgentToolCall, "codex")!
    const settled = workbenchItemFromThreadItem({
      ...wireCollabAgentToolCall,
      agentsStates: { "thread-child-1": { status: "completed" }, "thread-child-2": { status: "completed" } },
    }, "codex")!
    expect(workbenchItemSignature(running)).not.toBe(workbenchItemSignature(settled))
    expect(workbenchItemSignature(running)).toBe(workbenchItemSignature({ ...running }))
  })
})
