import {
  KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
  KHALA_CODE_CODEX_PARITY_REFERENCE_LABEL,
  KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES,
} from "./codex-parity-contract.js"

export type KhalaCodeCodexThreadItemVariant =
  typeof KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES[number]

export type KhalaCodeCodexThreadItemFixture = Readonly<{
  item: Readonly<Record<string, unknown>>
  rendersVisible: boolean
  variant: KhalaCodeCodexThreadItemVariant
}>

export const KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE = {
  referenceCommit: KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
  referenceLabel: KHALA_CODE_CODEX_PARITY_REFERENCE_LABEL,
  schema: "khala-code-desktop.codex-thread-item-fixtures.v1",
} as const

export const KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES: readonly KhalaCodeCodexThreadItemFixture[] = [
  {
    item: {
      clientId: "client-user",
      content: [{ text: "hello", textElements: [], type: "text" }],
      id: "item-user",
      type: "userMessage",
    },
    rendersVisible: true,
    variant: "userMessage",
  },
  {
    item: {
      fragments: [{ hookRunId: "hook-1", text: "hook says hi" }],
      id: "item-hook",
      type: "hookPrompt",
    },
    rendersVisible: true,
    variant: "hookPrompt",
  },
  {
    item: { id: "item-agent", text: "assistant text", type: "agentMessage" },
    rendersVisible: true,
    variant: "agentMessage",
  },
  {
    item: { id: "item-plan", text: "- step one", type: "plan" },
    rendersVisible: true,
    variant: "plan",
  },
  {
    item: { content: ["hidden-ish thought"], id: "item-reasoning", summary: ["summary"], type: "reasoning" },
    rendersVisible: false,
    variant: "reasoning",
  },
  {
    item: {
      aggregatedOutput: "pass",
      command: "bun test",
      commandActions: [],
      cwd: "/workspace",
      durationMs: 42,
      exitCode: 0,
      id: "item-command",
      source: "agent",
      status: "completed",
      type: "commandExecution",
    },
    rendersVisible: true,
    variant: "commandExecution",
  },
  {
    item: {
      changes: [{
        diff: "--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old\n+new\n",
        kind: "update",
        path: "src/app.ts",
      }],
      id: "item-file",
      status: "completed",
      type: "fileChange",
    },
    rendersVisible: true,
    variant: "fileChange",
  },
  {
    item: {
      appContext: null,
      arguments: { number: 7784 },
      durationMs: 10,
      error: null,
      id: "item-mcp",
      pluginId: null,
      result: { content: [{ text: "ok", type: "text" }] },
      server: "github",
      status: "completed",
      tool: "issue_comment",
      type: "mcpToolCall",
    },
    rendersVisible: true,
    variant: "mcpToolCall",
  },
  {
    item: {
      arguments: { q: "button" },
      contentItems: [{ text: "found", type: "inputText" }],
      durationMs: 9,
      id: "item-dynamic",
      namespace: "figma",
      status: "completed",
      success: true,
      tool: "search",
      type: "dynamicToolCall",
    },
    rendersVisible: true,
    variant: "dynamicToolCall",
  },
  {
    item: {
      agentsStates: { "thread-child": { message: "done", status: "completed" } },
      id: "item-collab",
      model: "gpt-5.1-codex",
      prompt: "review this",
      reasoningEffort: "medium",
      receiverThreadIds: ["thread-child"],
      senderThreadId: "thread-fixture",
      status: "completed",
      tool: "spawnAgent",
      type: "collabAgentToolCall",
    },
    rendersVisible: true,
    variant: "collabAgentToolCall",
  },
  {
    item: {
      agentPath: "/tmp/agent",
      agentThreadId: "thread-child",
      id: "item-subactivity",
      kind: "started",
      type: "subAgentActivity",
    },
    rendersVisible: true,
    variant: "subAgentActivity",
  },
  {
    item: { action: null, id: "item-web", query: "codex app-server", type: "webSearch" },
    rendersVisible: true,
    variant: "webSearch",
  },
  {
    item: { id: "item-image", path: "/tmp/screenshot.png", type: "imageView" },
    rendersVisible: true,
    variant: "imageView",
  },
  {
    item: { durationMs: 1000, id: "item-sleep", type: "sleep" },
    rendersVisible: true,
    variant: "sleep",
  },
  {
    item: {
      id: "item-image-generation",
      result: "generated",
      revisedPrompt: "a precise UI card",
      savedPath: "/tmp/image.png",
      status: "completed",
      type: "imageGeneration",
    },
    rendersVisible: true,
    variant: "imageGeneration",
  },
  {
    item: { id: "item-review-enter", review: "reviewing", type: "enteredReviewMode" },
    rendersVisible: true,
    variant: "enteredReviewMode",
  },
  {
    item: { id: "item-review-exit", review: "done", type: "exitedReviewMode" },
    rendersVisible: true,
    variant: "exitedReviewMode",
  },
  {
    item: { id: "item-compact", type: "contextCompaction" },
    rendersVisible: true,
    variant: "contextCompaction",
  },
] as const

export const KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_VARIANTS =
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES.map((fixture) => fixture.variant)
