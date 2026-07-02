import { describe, expect, test } from "bun:test"

import {
  createCodexThreadItemEventProjector,
} from "../src/bun/codex-thread-item-projector"
import {
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES,
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE,
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_VARIANTS,
} from "../src/bun/codex-thread-item-fixtures"
import {
  KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
  KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES,
} from "../src/bun/codex-parity-contract"
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
  test("pins shared ThreadItem fixtures to the parity contract", () => {
    expect(KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE.referenceCommit)
      .toBe(KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT)
    expect([...KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_VARIANTS])
      .toEqual([...KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES])
  })

  test("projects every supported ThreadItem variant into stable transcript cards", () => {
    const projector = createCodexThreadItemEventProjector({
      desktopTurnId: "desktop-turn-fixture",
      renderUserMessages: true,
    })
    const variants = KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES.map((fixture) => fixture.item)

    const events = variants.flatMap(variant => [...projector.accept(item(variant))])
    const visibleVariants = KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES.filter((fixture) => fixture.rendersVisible)
    const projectedMessages = messageEvents(events).map(event => event.message)

    expect(projectedMessages).toHaveLength(visibleVariants.length)
    expect(projectedMessages.map(message => message.codexItem?.itemType ?? null)).toEqual(
      visibleVariants.map(fixture =>
        fixture.variant === "agentMessage" || fixture.variant === "userMessage" ? null : fixture.variant
      ),
    )
    expect(projector.messages().map(message => message.id)).toEqual(visibleVariants.map(fixture => String(fixture.item.id)))
    expect(projector.messages().find(message => message.id === "item-agent")?.codexItem).toBeUndefined()
    expect(projector.messages().find(message => message.id === "item-user")?.codexItem).toBeUndefined()
    expect(projector.messages().find(message => message.id === "item-reasoning")).toBeUndefined()
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
    expect(projector.messages().find(message => message.id === "item-agent")?.codexItem).toBeUndefined()
    expect(projector.messages().find(message => message.id === "item-command")?.body).toContain("ok")
    expect(projector.messages().find(message => message.id === "item-file")?.body).toContain("+new")
  })

  test("labels tool cards with relative paths instead of absolute worktree paths", () => {
    const displayRoot = "/Users/christopherdavid/work/openagents"
    const sourcePath = `${displayRoot}/clients/khala-code-desktop/src/ui/transcript-render.ts`
    const projector = createCodexThreadItemEventProjector({
      desktopTurnId: "desktop-turn-fixture",
      displayRoot,
    })

    projector.accept(item({
      type: "commandExecution",
      id: "item-read",
      command: `sed -n '1,20p' ${sourcePath}`,
      cwd: displayRoot,
      source: "agent",
      status: "completed",
      commandActions: [{
        type: "read",
        command: "sed",
        name: "sed",
        path: sourcePath,
      }],
      aggregatedOutput: `${sourcePath}: ok`,
      exitCode: 0,
      durationMs: 11,
    }))
    projector.accept(item({
      type: "fileChange",
      id: "item-edit",
      status: "completed",
      changes: [{
        path: sourcePath,
        kind: "update",
        diff: "--- a/clients/khala-code-desktop/src/ui/transcript-render.ts\n+++ b/clients/khala-code-desktop/src/ui/transcript-render.ts\n@@ -1 +1 @@\n-old\n+new\n",
      }],
    }))
    projector.accept(item({
      type: "dynamicToolCall",
      id: "item-dynamic-read",
      namespace: "filesystem",
      tool: "read_file",
      arguments: { path: sourcePath },
      status: "completed",
      contentItems: [{ type: "inputText", text: sourcePath }],
      success: true,
      durationMs: 4,
    }))

    const messages = projector.messages()
    expect(messages.find(message => message.id === "item-read")?.codexItem).toMatchObject({
      subtitle: ".",
      title: "Read clients/khala-code-desktop/src/ui/transcript-render.ts",
    })
    expect(messages.find(message => message.id === "item-edit")?.codexItem?.title).toBe(
      "Edited clients/khala-code-desktop/src/ui/transcript-render.ts",
    )
    expect(messages.find(message => message.id === "item-dynamic-read")?.codexItem?.title).toBe(
      "Read clients/khala-code-desktop/src/ui/transcript-render.ts",
    )
    expect(messages.map(message => `${message.codexItem?.title ?? ""}\n${message.body}`).join("\n")).not.toContain(
      "/Users/christopherdavid/work/openagents",
    )
  })

  test("suppresses Codex reasoning items so only the transient Thinking shimmer represents waiting", () => {
    const projector = createCodexThreadItemEventProjector({ desktopTurnId: "desktop-turn-fixture" })

    const events = [
      ...projector.accept(note("item/reasoning/summaryTextDelta", {
        itemId: "item-reasoning",
        delta: "summary",
      })),
      ...projector.accept(note("item/reasoning/textDelta", {
        itemId: "item-reasoning",
        delta: "hidden thought",
      })),
      ...projector.accept(item({
        type: "reasoning",
        id: "item-reasoning",
        summary: ["summary"],
        content: ["hidden thought"],
      })),
    ]

    expect(events).toEqual([])
    expect(projector.messages()).toEqual([])
  })

  test("projects raw rollout response function calls and output into one visible tool card", () => {
    const projector = createCodexThreadItemEventProjector({
      desktopTurnId: "desktop-turn-fixture",
      displayRoot: "/workspace",
    })

    const callEvents = projector.accept(item({
      type: "response_item",
      payload: {
        type: "function_call",
        id: "fc-fixture",
        call_id: "call-fixture",
        name: "exec_command",
        arguments: "{\"cmd\":\"git status --short\"}",
        status: "completed",
      },
    }))
    const outputEvents = projector.accept(item({
      type: "response_item",
      payload: {
        type: "function_call_output",
        call_id: "call-fixture",
        output: "Exit code: 0\nOutput:\n M src/app.ts",
      },
    }))

    expect(messageEvents(callEvents)[0]?.message).toMatchObject({
      id: "call-fixture",
      codexItem: {
        itemId: "call-fixture",
        itemType: "function_call",
        title: "Exec command",
      },
    })
    expect(messageEvents(outputEvents)[0]?.message).toMatchObject({
      id: "call-fixture",
      codexItem: {
        itemId: "call-fixture",
        itemType: "function_call",
        status: "completed",
      },
    })
    expect(projector.messages()).toHaveLength(1)
    expect(projector.messages()[0]?.body).toContain("git status --short")
    expect(projector.messages()[0]?.body).toContain("Exit code: 0")
    expect(projector.messages()[0]?.body).toContain("M src/app.ts")
  })

  test("renders approvals and resolves them by server request id", () => {
    const projector = createCodexThreadItemEventProjector({ desktopTurnId: "desktop-turn-fixture" })

    const pending = projector.accept(note("item/commandExecution/requestApproval", {
      itemId: "item-command",
      command: "git status",
      cwd: "/workspace",
      reason: "command wants workspace access",
      availableDecisions: ["accept", "decline"],
      proposedExecpolicyAmendment: ["git status"],
      proposedNetworkPolicyAmendments: [{
        action: "allow",
        host: "api.example.com",
      }],
    }, 7))
    const resolved = projector.accept(note("serverRequest/resolved", {
      requestId: 7,
    }))

    expect(messageEvents(pending)[0]?.message.codexItem).toMatchObject({
      itemId: "item-command",
      itemType: "approval",
      requestId: "7",
      status: "pending",
      approval: {
        method: "item/commandExecution/requestApproval",
        requestId: 7,
        availableDecisions: ["accept", "decline"],
        proposedExecpolicyAmendment: ["git status"],
        proposedNetworkPolicyAmendments: [{
          action: "allow",
          host: "api.example.com",
        }],
      },
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
