import { describe, expect, test } from "bun:test"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"
import { readFileSync } from "node:fs"
import type { DesktopThread } from "../chat-contract.ts"
import type {
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import { openAgentsDesktopUxContractRegistry } from "../contracts/ux-contracts.ts"
import type { ChatHost } from "./shell.ts"
import {
  makeRuntimeConversationChatHost,
  selectDesktopChatHost,
  selectDesktopChatHostSelection,
} from "./runtime-conversation.ts"

const status = { phase: "live" as const, cursor: 5, pendingMutationCount: 0 }
const now = "2026-07-10T20:15:00.000Z"

describe("authoritative Runtime Gateway chat adapter", () => {
  test("registers the visible authoritative Sync-mode contract", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    expect(openAgentsDesktopUxContractRegistry.contracts.find(
      contract => contract.contractId === "openagents_desktop.chat.authoritative_sync_mode.v1",
    )?.state).toBe("enforced")
  })

  test("selects one mode at boot and retains local chat when Sync is unavailable", async () => {
    const local: ChatHost = {
      listThreads: async () => [],
      newThread: async () => null,
      openThread: async () => null,
      sendMessage: async () => ({ ok: false }),
    }
    const selected = await selectDesktopChatHost({
      local,
      request: async () => ({
        kind: "conversation_unavailable",
        requestId: "mode",
        reason: "not_live",
      }),
    })
    expect(selected).toBe(local)

    const catchingUp = await selectDesktopChatHost({
      local,
      request: async () => ({
        kind: "conversation_catalog",
        requestId: "mode-catching-up",
        status: { phase: "catching_up", cursor: 4, pendingMutationCount: 1 },
        threads: [],
      }),
    })
    expect(catchingUp).toBe(local)
  })

  test("selection reports the mode so lane availability can be evidence-gated (#8712)", async () => {
    const local: ChatHost = {
      listThreads: async () => [],
      newThread: async () => null,
      openThread: async () => null,
      sendMessage: async () => ({ ok: false }),
    }
    const localSelection = await selectDesktopChatHostSelection({
      local,
      request: async () => ({
        kind: "conversation_unavailable",
        requestId: "mode",
        reason: "not_live",
      }),
    })
    expect(localSelection.mode).toBe("local")
    expect(localSelection.host).toBe(local)

    const runtimeSelection = await selectDesktopChatHostSelection({
      local,
      request: async () => ({
        kind: "conversation_catalog",
        requestId: "mode-live",
        status,
        threads: [],
      }),
    })
    expect(runtimeSelection.mode).toBe("runtime")
    expect(runtimeSelection.host).not.toBe(local)
  })

  test("maps confirmed threads/messages and waits for exact mutation refs", async () => {
    const threads = new Map<string, { title: string; messages: Array<{ ref: string; body: string }> }>([
      ["thread.synced.1", { title: "Synced", messages: [{ ref: "message.synced.1", body: "Confirmed" }] }],
    ])
    const commands: Array<Record<string, unknown>> = []
    let startedRunRef: string | null = null
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as { requestId?: string; commandId?: string; query?: { id: string; intentId?: string; threadRef?: string }; command?: Record<string, string> }
      if (value.query?.id === "conversation.catalog") {
        return {
          kind: "conversation_catalog",
          requestId: value.requestId!,
          status,
          threads: [...threads].map(([threadRef, thread], index) => ({
            threadRef,
            title: thread.title,
            messageCount: thread.messages.length,
            lastMessageAt: now,
            updatedAt: now,
            version: index + 1,
          })),
        }
      }
      if (value.query?.id === "conversation.thread") {
        const threadRef = value.query.threadRef!
        const thread = threads.get(threadRef)
        return {
          kind: "conversation_thread",
          requestId: value.requestId!,
          threadRef,
          status,
          messages: (thread?.messages ?? []).map((message, index) => ({
            messageRef: message.ref,
            threadRef,
            body: message.body,
            createdAt: now,
            updatedAt: now,
            version: index + 3,
          })),
        }
      }
      if (value.query?.id === "conversation.timeline") {
        return {
          kind: "conversation_timeline",
          requestId: value.requestId!,
          threadRef: value.query.threadRef!,
          status,
          run: startedRunRef === null ? null : {
            runRef: startedRunRef,
            routeRef: value.query.threadRef!,
            status: "completed",
            createdAt: now,
            updatedAt: now,
            startedAt: now,
            completedAt: now,
            failedAt: null,
            canceledAt: null,
            version: 1,
          },
          events: startedRunRef === null ? [] : [
            { eventRef: "event.text.1", runRef: startedRunRef, sequence: 1, eventType: "text.delta", summary: "Hello", status: null, artifactRefs: [], item: { kind: "text", messageRef: "assistant.1", text: "Hello" }, createdAt: now, version: 2 },
            { eventRef: "event.tool.1", runRef: startedRunRef, sequence: 2, eventType: "tool.call", summary: "Called shell", status: "completed", artifactRefs: [], item: { kind: "tool", toolCallRef: "tool.1", toolName: "shell", status: "completed" }, createdAt: now, version: 3 },
            { eventRef: "event.terminal.1", runRef: startedRunRef, sequence: 3, eventType: "turn.finished", summary: "Turn finished", status: "completed", artifactRefs: [], item: { kind: "terminal", status: "completed" }, createdAt: now, version: 4 },
          ],
        }
      }
      if (value.query?.id === "conversation.commandOutcome") {
        return {
          kind: "runtime_command_status",
          requestId: value.requestId!,
          commandRef: value.query.intentId!,
          threadRef: value.query.threadRef!,
          runRef: startedRunRef,
          status: "settled",
          mutationId: null,
          version: 4,
          updatedAt: now,
        }
      }
      const command = value.command!
      commands.push(command)
      if (command.id === "conversation.create") {
        threads.set(command.threadRef!, { title: command.title!, messages: [] })
      } else if (command.id === "conversation.append") {
        threads.get(command.threadRef!)!.messages.push({
          ref: command.messageRef!,
          body: command.body!,
        })
      } else if (command.id === "conversation.start") {
        startedRunRef = command.runRef!
        return {
          kind: "runtime_command_outcome",
          commandId: value.commandId!,
          threadRef: command.threadRef!,
          runRef: command.runRef!,
          messageRef: command.messageRef!,
          status: "unknown_pending_reconcile",
          mutationId: commands.length,
        }
      }
      return {
        kind: "conversation_mutation_outcome",
        commandId: value.commandId!,
        status: "pending_reconcile",
        mutationId: commands.length,
      }
    }
    const chat = makeRuntimeConversationChatHost({
      request,
      randomId: (() => {
        const ids = ["new-thread", "new-message", "new-run"]
        return () => ids.shift()!
      })(),
    })

    expect((await chat.listThreads())[0]?.id).toBe("thread.synced.1")
    expect((await chat.openThread("thread.synced.1"))?.notes).toEqual([{
      key: "message.synced.1",
      role: "user",
      text: "Confirmed",
      timestamp: "20:15",
    }])
    expect((await chat.newThread())?.id).toBe("thread.desktop.new-thread")
    const result = await chat.sendMessage({
      id: "thread.synced.1",
      message: "Follow-up",
    })
    expect(result).toMatchObject({ ok: true })
    expect(result.thread?.notes.find(note => note.key === "message.desktop.new-message")).toMatchObject({
      key: "message.desktop.new-message",
      text: "Follow-up",
      role: "user",
    })
    expect(result.thread?.notes).toEqual(expect.arrayContaining([
      expect.objectContaining({ role: "assistant", text: "Hello" }),
      expect.objectContaining({ role: "system", text: "shell · completed" }),
      expect.objectContaining({ role: "system", text: "Turn completed" }),
    ]))
    expect(commands.map(command => command.id)).toEqual([
      "conversation.create",
      "conversation.append",
      "conversation.start",
    ])
    expect(commands.find(command => command.id === "conversation.start")).not.toHaveProperty("lane")
  })

  test("streams append and terminal confirmation from one fenced subscription without timeline polling", async () => {
    const listeners = new Set<(event: DesktopRuntimeGatewayEvent) => void>()
    const requests: Array<{
      command?: Record<string, string | number>
      commandId?: string
      query?: { id: string; intentId?: string; threadRef?: string }
      requestId?: string
    }> = []
    const emit = (event: DesktopRuntimeGatewayEvent): void => {
      for (const listener of [...listeners]) listener(event)
    }
    const liveEvent = (
      sequence: number,
      messages: Array<{ messageRef: string; body: string }>,
      terminal: boolean,
    ): DesktopRuntimeGatewayEvent => ({
      kind: "conversation.live.update",
      envelope: {
        kind: "conversation.live",
        delivery: "confirmed",
        subscriptionRef: "subscription.renderer.conversation.live-subscription",
        generation: 1,
        sequence,
        threadRef: "thread.live.1",
        cursor: 5 + sequence,
        recovery: sequence === 1 ? "initial" : "resumed",
        ...(terminal ? { runRef: "turn.desktop.live-run" } : {}),
        messageRefs: messages.map(message => message.messageRef),
        eventRefs: terminal ? ["event.live.text", "event.live.terminal"] : [],
        graphRefs: [],
      },
      snapshot: {
        status: { phase: "live", cursor: 5 + sequence, pendingMutationCount: 0 },
        thread: {
          threadRef: "thread.live.1",
          title: "Live",
          messageCount: messages.length,
          lastMessageAt: messages.length === 0 ? null : now,
          updatedAt: now,
          version: sequence,
        },
        messages: messages.map((message, index) => ({
          messageRef: message.messageRef,
          threadRef: "thread.live.1",
          body: message.body,
          createdAt: now,
          updatedAt: now,
          version: index + 1,
        })),
        timeline: terminal ? {
          status: { phase: "live", cursor: 5 + sequence, pendingMutationCount: 0 },
          run: {
            runRef: "turn.desktop.live-run",
            routeRef: "thread.live.1",
            status: "completed",
            createdAt: now,
            updatedAt: now,
            startedAt: now,
            completedAt: now,
            failedAt: null,
            canceledAt: null,
            version: 2,
          },
          events: [
            { eventRef: "event.live.text", runRef: "turn.desktop.live-run", sequence: 1, eventType: "text.delta", summary: "Live answer", status: null, artifactRefs: [], item: { kind: "text", messageRef: "assistant.live.1", text: "Live answer" }, createdAt: now, version: 2 },
            { eventRef: "event.live.terminal", runRef: "turn.desktop.live-run", sequence: 2, eventType: "turn.finished", summary: "Done", status: "completed", artifactRefs: [], item: { kind: "terminal", status: "completed" }, createdAt: now, version: 3 },
          ],
        } : null,
        graphs: [],
      },
    })
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as { command?: Record<string, string | number>; commandId?: string; query?: { id: string; intentId?: string; threadRef?: string }; requestId?: string }
      requests.push(value)
      if (value.command?.id === "conversation.subscribe") {
        emit(liveEvent(1, [], false))
        return {
          kind: "conversation_subscription_outcome",
          commandId: value.commandId!,
          subscriptionRef: String(value.command.subscriptionRef),
          generation: Number(value.command.generation),
          status: "subscribed",
        }
      }
      if (value.command?.id === "conversation.append") {
        queueMicrotask(() => emit(liveEvent(2, [{ messageRef: "message.desktop.live-message", body: "Stream it" }], false)))
        return { kind: "conversation_mutation_outcome", commandId: value.commandId!, status: "pending_reconcile", mutationId: 1 }
      }
      if (value.command?.id === "conversation.start") {
        queueMicrotask(() => emit(liveEvent(3, [{ messageRef: "message.desktop.live-message", body: "Stream it" }], true)))
        return { kind: "runtime_command_outcome", commandId: value.commandId!, threadRef: "thread.live.1", messageRef: "message.desktop.live-message", runRef: "turn.desktop.live-run", status: "accepted", mutationId: 2 }
      }
      if (value.query?.id === "conversation.commandOutcome") {
        return { kind: "runtime_command_status", requestId: value.requestId!, commandRef: value.query.intentId!, threadRef: "thread.live.1", runRef: "turn.desktop.live-run", status: "settled", mutationId: 2, version: 3, updatedAt: now }
      }
      if (value.command?.id === "conversation.unsubscribe") {
        return { kind: "conversation_subscription_outcome", commandId: value.commandId!, subscriptionRef: String(value.command.subscriptionRef), generation: Number(value.command.generation), status: "unsubscribed" }
      }
      throw new Error(`unexpected request ${value.command?.id ?? value.query?.id}`)
    }
    const ids = ["live-message", "live-run", "live-subscription"]
    const updates: Array<DesktopThread> = []
    const chat = makeRuntimeConversationChatHost({
      request,
      subscribe: listener => {
        listeners.add(listener)
        return () => listeners.delete(listener)
      },
      randomId: () => ids.shift()!,
      liveTimeoutMs: 100,
    })
    const result = await chat.sendMessage({
      id: "thread.live.1",
      message: "Stream it",
      onUpdate: thread => { updates.push(thread) },
    })

    expect(result.ok).toBe(true)
    expect(result.thread?.notes).toEqual(expect.arrayContaining([
      expect.objectContaining({ key: "message.desktop.live-message", text: "Stream it" }),
      expect.objectContaining({ role: "assistant", text: "Live answer" }),
      expect.objectContaining({ role: "system", text: "Turn completed" }),
    ]))
    expect(updates.at(-1)?.notes).toEqual(result.thread?.notes)
    expect(requests.filter(value => value.query?.id === "conversation.timeline")).toEqual([])
    expect(requests.map(value => value.command?.id).filter(Boolean)).toEqual([
      "conversation.subscribe",
      "conversation.append",
      "conversation.start",
      "conversation.unsubscribe",
    ])
    expect(listeners.size).toBe(0)
  })

  test("source oracle forbids recurring renderer timeline polling", () => {
    const source = readFileSync(new URL("./runtime-conversation.ts", import.meta.url), "utf8")
    expect(source).not.toContain("pollAttempts")
    expect(source).not.toContain("sleep(100)")
    expect(source).not.toContain("setInterval(")
  })

  const makeHarnessFixture = () => {
    const startCommands: Array<Record<string, unknown>> = []
    let startedRunRef: string | null = null
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as { requestId?: string; commandId?: string; query?: { id: string; intentId?: string; threadRef?: string }; command?: Record<string, string> }
      if (value.query?.id === "conversation.catalog") {
        return {
          kind: "conversation_catalog",
          requestId: value.requestId!,
          status,
          threads: [{ threadRef: "thread.harness.1", title: "Harness", messageCount: 1, lastMessageAt: now, updatedAt: now, version: 1 }],
        }
      }
      if (value.query?.id === "conversation.thread") {
        return {
          kind: "conversation_thread",
          requestId: value.requestId!,
          threadRef: value.query.threadRef!,
          status,
          messages: [{ messageRef: "message.desktop.harness-message", threadRef: value.query.threadRef!, body: "Pick a lane", createdAt: now, updatedAt: now, version: 2 }],
        }
      }
      if (value.query?.id === "conversation.timeline") {
        return {
          kind: "conversation_timeline",
          requestId: value.requestId!,
          threadRef: value.query.threadRef!,
          status,
          run: startedRunRef === null ? null : {
            runRef: startedRunRef,
            routeRef: value.query.threadRef!,
            status: "completed",
            createdAt: now,
            updatedAt: now,
            startedAt: now,
            completedAt: now,
            failedAt: null,
            canceledAt: null,
            version: 1,
          },
          events: [],
        }
      }
      if (value.query?.id === "conversation.commandOutcome") {
        return {
          kind: "runtime_command_status",
          requestId: value.requestId!,
          commandRef: value.query.intentId!,
          threadRef: value.query.threadRef!,
          runRef: startedRunRef,
          status: "settled",
          mutationId: null,
          version: 3,
          updatedAt: now,
        }
      }
      if (value.command?.id === "conversation.start") {
        startCommands.push(value.command)
        startedRunRef = value.command.runRef!
        return {
          kind: "runtime_command_outcome",
          commandId: value.commandId!,
          threadRef: value.command.threadRef!,
          runRef: value.command.runRef!,
          messageRef: value.command.messageRef!,
          status: "unknown_pending_reconcile",
          mutationId: 5,
        }
      }
      return {
        kind: "conversation_mutation_outcome",
        commandId: value.commandId!,
        status: "pending_reconcile",
        mutationId: 4,
      }
    }
    const ids = ["harness-message", "harness-run"]
    const chat = makeRuntimeConversationChatHost({
      request,
      randomId: () => ids.shift() ?? "harness-extra",
    })
    return { chat, startCommands }
  }

  test("harness fable targets the claude_pylon lane on conversation.start", async () => {
    const { chat, startCommands } = makeHarnessFixture()
    const result = await chat.sendMessage({ id: "thread.harness.1", message: "Pick a lane", harness: "fable" })
    expect(result.ok).toBe(true)
    expect(startCommands).toEqual([{
      id: "conversation.start",
      messageRef: "message.desktop.harness-message",
      runRef: "turn.desktop.harness-run",
      threadRef: "thread.harness.1",
      lane: "claude_pylon",
    }])
  })

  test("harness codex targets the codex_app_server lane on conversation.start", async () => {
    const { chat, startCommands } = makeHarnessFixture()
    const result = await chat.sendMessage({ id: "thread.harness.1", message: "Pick a lane", harness: "codex" })
    expect(result.ok).toBe(true)
    expect(startCommands).toEqual([{
      id: "conversation.start",
      messageRef: "message.desktop.harness-message",
      runRef: "turn.desktop.harness-run",
      threadRef: "thread.harness.1",
      lane: "codex_app_server",
    }])
  })

  test("rejected dispatch reports provider-neutral copy", async () => {
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as { requestId?: string; commandId?: string; query?: { id: string; threadRef?: string }; command?: Record<string, string> }
      if (value.query?.id === "conversation.catalog") {
        return {
          kind: "conversation_catalog",
          requestId: value.requestId!,
          status,
          threads: [{ threadRef: "thread.harness.1", title: "Harness", messageCount: 1, lastMessageAt: now, updatedAt: now, version: 1 }],
        }
      }
      if (value.query?.id === "conversation.thread") {
        return {
          kind: "conversation_thread",
          requestId: value.requestId!,
          threadRef: value.query.threadRef!,
          status,
          messages: [{ messageRef: "message.desktop.rejected-message", threadRef: value.query.threadRef!, body: "Rejected", createdAt: now, updatedAt: now, version: 2 }],
        }
      }
      if (value.query?.id === "conversation.timeline") {
        return { kind: "conversation_timeline", requestId: value.requestId!, threadRef: value.query.threadRef!, status, run: null, events: [] }
      }
      if (value.command?.id === "conversation.start") {
        return {
          kind: "runtime_command_outcome",
          commandId: value.commandId!,
          threadRef: value.command.threadRef!,
          runRef: value.command.runRef!,
          messageRef: value.command.messageRef!,
          status: "rejected",
          reason: "Runtime command was rejected before admission.",
        }
      }
      return { kind: "conversation_mutation_outcome", commandId: value.commandId!, status: "pending_reconcile", mutationId: 1 }
    }
    const ids = ["rejected-message", "rejected-run"]
    const chat = makeRuntimeConversationChatHost({
      request,
      randomId: () => ids.shift() ?? "rejected-extra",
    })
    const result = await chat.sendMessage({ id: "thread.harness.1", message: "Rejected", harness: "fable" })
    expect(result).toEqual({ ok: false, error: "Message was admitted, but agent dispatch was rejected." })
    expect(result.error).not.toContain("Codex")
  })

  test("never reports an unconfirmed append completed", async () => {
    const chat = makeRuntimeConversationChatHost({
      randomId: () => "pending",
      request: async raw => {
        const value = raw as { requestId?: string; commandId?: string; query?: { id: string; threadRef?: string } }
        if (value.query?.id === "conversation.catalog") {
          return {
            kind: "conversation_catalog",
            requestId: value.requestId!,
            status,
            threads: [{
              threadRef: "thread.pending",
              title: "Pending",
              messageCount: 0,
              lastMessageAt: null,
              updatedAt: now,
              version: 1,
            }],
          }
        }
        if (value.query?.id === "conversation.thread") {
          return {
            kind: "conversation_thread",
            requestId: value.requestId!,
            threadRef: value.query.threadRef!,
            status,
            messages: [],
          }
        }
        return {
          kind: "conversation_mutation_outcome",
          commandId: value.commandId!,
          status: "pending_reconcile",
          mutationId: 1,
        }
      },
    })
    const result = await chat.sendMessage({ id: "thread.pending", message: "Still pending" })
    expect(result.ok).toBe(false)
    expect(result.error).toContain("pending reconciliation")
    expect(result.error).not.toContain("completed")
  })

  test("surfaces the exact durable expired command after reconnect", async () => {
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as {
        command?: Record<string, string>
        commandId?: string
        query?: { id: string; intentId?: string; threadRef?: string }
        requestId?: string
      }
      if (value.command?.id === "conversation.append") {
        return {
          kind: "conversation_mutation_outcome",
          commandId: value.commandId!,
          mutationId: 1,
          status: "pending_reconcile",
        }
      }
      if (value.command?.id === "conversation.start") {
        return {
          kind: "runtime_command_outcome",
          commandId: value.commandId!,
          messageRef: value.command.messageRef!,
          mutationId: 2,
          runRef: value.command.runRef!,
          status: "unknown_pending_reconcile",
          threadRef: value.command.threadRef!,
        }
      }
      if (value.query?.id === "conversation.thread") {
        return {
          kind: "conversation_thread",
          messages: [{
            body: "Queued offline",
            createdAt: now,
            messageRef: "message.desktop.expired-message",
            threadRef: "thread.desktop.expired",
            updatedAt: now,
            version: 2,
          }],
          requestId: value.requestId!,
          status,
          threadRef: "thread.desktop.expired",
        }
      }
      if (value.query?.id === "conversation.catalog") {
        return {
          kind: "conversation_catalog",
          requestId: value.requestId!,
          status,
          threads: [{
            lastMessageAt: now,
            messageCount: 1,
            threadRef: "thread.desktop.expired",
            title: "Expired",
            updatedAt: now,
            version: 2,
          }],
        }
      }
      if (value.query?.id === "conversation.timeline") {
        return {
          events: [],
          kind: "conversation_timeline",
          requestId: value.requestId!,
          run: null,
          status,
          threadRef: "thread.desktop.expired",
        }
      }
      if (value.query?.id === "conversation.commandOutcome") {
        return {
          commandRef: value.query.intentId!,
          kind: "runtime_command_status",
          mutationId: null,
          requestId: value.requestId!,
          runRef: "turn.desktop.expired-run",
          status: "expired",
          threadRef: value.query.threadRef!,
          updatedAt: "2026-07-11T12:06:00.000Z",
          version: 7,
        }
      }
      throw new Error("unexpected request")
    }
    const ids = ["expired-message", "expired-run"]
    const chat = makeRuntimeConversationChatHost({
      request,
      randomId: () => ids.shift()!,
    })

    expect(await chat.sendMessage({
      id: "thread.desktop.expired",
      message: "Queued offline",
    })).toEqual({
      ok: false,
      error: "Runtime command expired while this device was offline.",
    })
  })
})
