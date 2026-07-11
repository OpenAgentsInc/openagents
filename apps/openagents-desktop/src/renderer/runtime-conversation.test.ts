import { describe, expect, test } from "bun:test"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"
import type { DesktopRuntimeGatewayResponse } from "../runtime-gateway-contract.ts"
import { openAgentsDesktopUxContractRegistry } from "../contracts/ux-contracts.ts"
import type { ChatHost } from "./shell.ts"
import {
  makeRuntimeConversationChatHost,
  selectDesktopChatHost,
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

  test("maps confirmed threads/messages and waits for exact mutation refs", async () => {
    const threads = new Map<string, { title: string; messages: Array<{ ref: string; body: string }> }>([
      ["thread.synced.1", { title: "Synced", messages: [{ ref: "message.synced.1", body: "Confirmed" }] }],
    ])
    const commands: Array<Record<string, unknown>> = []
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as { requestId?: string; commandId?: string; query?: { id: string; threadRef?: string }; command?: Record<string, string> }
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
      const command = value.command!
      commands.push(command)
      if (command.id === "conversation.create") {
        threads.set(command.threadRef!, { title: command.title!, messages: [] })
      } else if (command.id === "conversation.append") {
        threads.get(command.threadRef!)!.messages.push({
          ref: command.messageRef!,
          body: command.body!,
        })
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
        const ids = ["new-thread", "new-message"]
        return () => ids.shift()!
      })(),
      sleep: async () => undefined,
      pollAttempts: 2,
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
    expect(result.thread?.notes.at(-1)).toMatchObject({
      key: "message.desktop.new-message",
      text: "Follow-up",
      role: "user",
    })
    expect(commands.map(command => command.id)).toEqual([
      "conversation.create",
      "conversation.append",
    ])
  })

  test("never reports an unconfirmed append completed", async () => {
    let sleeps = 0
    const chat = makeRuntimeConversationChatHost({
      randomId: () => "pending",
      pollAttempts: 2,
      sleep: async () => { sleeps += 1 },
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
    expect(sleeps).toBe(2)
  })
})
