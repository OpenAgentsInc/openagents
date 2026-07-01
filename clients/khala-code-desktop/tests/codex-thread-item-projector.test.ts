import { describe, expect, test } from "bun:test"

import {
  createCodexThreadItemEventProjector,
} from "../src/bun/codex-thread-item-projector"
import type { CodexAppServerNotification } from "../src/bun/codex-app-server-client"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc"

const note = (
  method: string,
  params: Record<string, unknown>,
  id?: number | string,
): CodexAppServerNotification => ({
  method,
  params: {
    threadId: "thread-fixture",
    turnId: "turn-fixture",
    ...params,
  },
  receivedAt: "2026-07-01T17:00:00.000Z",
  ...(id === undefined ? {} : { id }),
})

const item = (value: Record<string, unknown>): CodexAppServerNotification =>
  note("item/completed", { item: value })

const messageEvents = (
  events: readonly KhalaCodeDesktopChatTurnEvent[],
): readonly Extract<KhalaCodeDesktopChatTurnEvent, { readonly type: "message_start" | "message_replace" }>[] =>
  events.filter((event): event is Extract<KhalaCodeDesktopChatTurnEvent, { readonly type: "message_start" | "message_replace" }> =>
    event.type === "message_start" || event.type === "message_replace")

describe("Codex ThreadItem projector", () => {
  test("projects every supported ThreadItem variant into stable transcript cards", () => {
    const projector = createCodexThreadItemEventProjector({
      desktopTurnId: "desktop-turn-fixture",
      renderUserMessages: true,
    })
    const variants: readonly Record<string, unknown>[] = [
      {
        type: "userMessage",
        id: "item-user",
        clientId: "client-user",
        content: [{ type: "text", text: "hello", textElements: [] }],
      },
      {
        type: "hookPrompt",
        id: "item-hook",
        fragments: [{ text: "hook says hi", hookRunId: "hook-1" }],
      },
      { type: "agentMessage", id: "item-agent", text: "assistant text" },
      { type: "plan", id: "item-plan", text: "- step one" },
      { type: "reasoning", id: "item-reasoning", summary: ["summary"], content: ["hidden-ish thought"] },
      {
        type: "commandExecution",
        id: "item-command",
        command: "bun test",
        cwd: "/workspace",
        source: "agent",
        status: "completed",
        commandActions: [],
        aggregatedOutput: "pass",
        exitCode: 0,
        durationMs: 42,
      },
      {
        type: "fileChange",
        id: "item-file",
        status: "completed",
        changes: [{
          path: "src/app.ts",
          kind: "update",
          diff: "--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old\n+new\n",
        }],
      },
      {
        type: "mcpToolCall",
        id: "item-mcp",
        server: "github",
        tool: "issue_comment",
        status: "completed",
        arguments: { number: 7784 },
        appContext: null,
        pluginId: null,
        result: { content: [{ type: "text", text: "ok" }] },
        error: null,
        durationMs: 10,
      },
      {
        type: "dynamicToolCall",
        id: "item-dynamic",
        namespace: "figma",
        tool: "search",
        arguments: { q: "button" },
        status: "completed",
        contentItems: [{ type: "inputText", text: "found" }],
        success: true,
        durationMs: 9,
      },
      {
        type: "collabAgentToolCall",
        id: "item-collab",
        tool: "spawnAgent",
        status: "completed",
        senderThreadId: "thread-fixture",
        receiverThreadIds: ["thread-child"],
        prompt: "review this",
        model: "gpt-5.1-codex",
        reasoningEffort: "medium",
        agentsStates: { "thread-child": { status: "completed", message: "done" } },
      },
      {
        type: "subAgentActivity",
        id: "item-subactivity",
        kind: "started",
        agentThreadId: "thread-child",
        agentPath: "/tmp/agent",
      },
      { type: "webSearch", id: "item-web", query: "codex app-server", action: null },
      { type: "imageView", id: "item-image", path: "/tmp/screenshot.png" },
      { type: "sleep", id: "item-sleep", durationMs: 1000 },
      {
        type: "imageGeneration",
        id: "item-image-generation",
        status: "completed",
        revisedPrompt: "a precise UI card",
        result: "generated",
        savedPath: "/tmp/image.png",
      },
      { type: "enteredReviewMode", id: "item-review-enter", review: "reviewing" },
      { type: "exitedReviewMode", id: "item-review-exit", review: "done" },
      { type: "contextCompaction", id: "item-compact" },
    ]

    const events = variants.flatMap(variant => [...projector.accept(item(variant))])
    const cards = messageEvents(events).map(event => event.message)

    expect(cards).toHaveLength(variants.length)
    expect(cards.map(card => card.codexItem?.itemType)).toEqual(variants.map(variant => String(variant.type)))
    expect(projector.messages().map(message => message.id)).toEqual(variants.map(variant => String(variant.id)))
    expect(projector.messages().find(message => message.id === "item-file")?.body).toContain("```diff")
    expect(projector.messages().find(message => message.id === "item-command")?.codexItem).toMatchObject({
      itemId: "item-command",
      status: "completed",
      threadId: "thread-fixture",
      turnId: "turn-fixture",
    })
  })

  test("updates assistant text, command output, and patch cards in place", () => {
    const projector = createCodexThreadItemEventProjector({ desktopTurnId: "desktop-turn-fixture" })

    const events = [
      ...projector.accept(note("item/agentMessage/delta", {
        itemId: "item-agent",
        delta: "Hel",
      })),
      ...projector.accept(note("item/agentMessage/delta", {
        itemId: "item-agent",
        delta: "lo",
      })),
      ...projector.accept(note("item/commandExecution/outputDelta", {
        itemId: "item-command",
        delta: "running tests\n",
      })),
      ...projector.accept(note("item/commandExecution/outputDelta", {
        itemId: "item-command",
        delta: "ok\n",
      })),
      ...projector.accept(note("item/fileChange/patchUpdated", {
        itemId: "item-file",
        changes: [{
          path: "src/app.ts",
          kind: "update",
          diff: "--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old\n+new\n",
        }],
      })),
    ]

    expect(events.map(event => event.type)).toEqual([
      "message_start",
      "message_delta",
      "message_delta",
      "message_start",
      "message_delta",
      "message_replace",
      "message_start",
    ])
    expect(projector.messages()).toHaveLength(3)
    expect(projector.messages().find(message => message.id === "item-agent")?.body).toBe("Hello")
    expect(projector.messages().find(message => message.id === "item-command")?.body).toContain("ok")
    expect(projector.messages().find(message => message.id === "item-file")?.body).toContain("+new")
  })

  test("renders approvals and resolves them by server request id", () => {
    const projector = createCodexThreadItemEventProjector({ desktopTurnId: "desktop-turn-fixture" })

    const pending = projector.accept(note("item/commandExecution/requestApproval", {
      itemId: "item-command",
      command: "git status",
      cwd: "/workspace",
      reason: "command wants workspace access",
      availableDecisions: ["accept", "decline"],
    }, 7))
    const resolved = projector.accept(note("serverRequest/resolved", {
      requestId: 7,
    }))

    expect(messageEvents(pending)[0]?.message.codexItem).toMatchObject({
      itemId: "item-command",
      itemType: "approval",
      requestId: "7",
      status: "pending",
    })
    expect(messageEvents(resolved)[0]?.message.codexItem).toMatchObject({
      requestId: "7",
      status: "completed",
    })
    expect(projector.messages()[0]?.body).toContain("Approval resolved.")
  })

  test("keeps unknown Codex item variants visible", () => {
    const projector = createCodexThreadItemEventProjector({ desktopTurnId: "desktop-turn-fixture" })

    projector.accept(item({
      type: "futureThing",
      id: "item-future",
      surprising: true,
    }))

    expect(projector.messages()[0]).toMatchObject({
      id: "item-future",
      codexItem: {
        itemType: "futureThing",
        title: "Unknown Codex item: futureThing",
      },
    })
    expect(projector.messages()[0]?.body).toContain("surprising")
  })
})
