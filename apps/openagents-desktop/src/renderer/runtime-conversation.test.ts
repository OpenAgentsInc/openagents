import { describe, expect, test } from "bun:test"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"
import { decodeLiveAgentGraphEntity } from "@openagentsinc/khala-sync"
import { readFileSync } from "node:fs"
import type { DesktopThread } from "../chat-contract.ts"
import type {
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import { openAgentsDesktopUxContractRegistry } from "../contracts/ux-contracts.ts"
import type { ChatHost } from "./shell.ts"
import {
  makeConvergingDesktopChatHost,
  makeRuntimeConversationChatHost,
  runtimeInteractionNotes,
  selectDesktopChatHost,
  selectDesktopChatHostSelection,
} from "./runtime-conversation.ts"

test("projects canonical questions, approvals, plans, and terminal states as typed cards", () => {
  const base = {
    schema: "openagents.runtime_interaction_projection.v1" as const,
    threadId: "thread.runtime.cards",
    turnId: "turn.runtime.cards",
    displayText: "Choose",
    expiresAt: "2026-07-11T22:05:00.000Z",
    requestedSequence: 7,
    requestedAt: "2026-07-11T22:00:00.000Z",
    version: 1,
  }
  const notes = runtimeInteractionNotes([
    {
      ...base,
      interactionRef: "interaction.question.1",
      kind: "provider_question",
      status: "pending",
      displayTitle: "Verification",
      questions: [{
        questionRef: "question.runtime.1",
        displayText: "Which verification?",
        multiSelect: false,
        options: [{ optionRef: "option.tests", label: "Tests", description: "Run tests" }],
      }],
    },
    {
      ...base,
      interactionRef: "interaction.tool.1",
      kind: "tool_approval",
      status: "resolved",
      decisionRef: "decision.desktop.1",
      displayTitle: "Approve workspaceWrite",
      questions: [],
      version: 2,
    },
    {
      ...base,
      interactionRef: "interaction.plan.1",
      kind: "plan_review",
      status: "revoked",
      displayTitle: "Review plan",
      questions: [],
      version: 2,
    },
  ])
  expect(notes.map(note => note.question)).toMatchObject([
    {
      source: "runtime",
      threadRef: "thread.runtime.cards",
      questionRef: "interaction.question.1",
      status: "pending",
      questions: [{ questionRef: "question.runtime.1", options: [{ optionRef: "option.tests", label: "Tests" }] }],
    },
    {
      kind: "tool_approval",
      status: "resolved",
      decisionRef: "decision.desktop.1",
      questions: [{ options: [{ optionRef: "approve" }, { optionRef: "deny" }] }],
    },
    {
      kind: "plan_review",
      status: "revoked",
      questions: [{ options: [{ optionRef: "accept" }, { optionRef: "request_changes" }, { optionRef: "replan" }] }],
    },
  ])
})

const status = { phase: "live" as const, cursor: 5, pendingMutationCount: 0 }
const now = "2026-07-10T20:15:00.000Z"
const liveGraph = decodeLiveAgentGraphEntity({
  schema: "openagents.live_agent_graph.v1",
  graphRef: "graph.runtime.live.1",
  sessionRef: "session.runtime.live.1",
  threadRef: "thread.live.1",
  attachmentGeneration: 1,
  cursor: 2,
  lastDeltaRef: "delta.runtime.live.2",
  nodes: [
    {
      agentRef: "agent.runtime.root",
      sessionRef: "session.runtime.live.1",
      threadRef: "thread.runtime.root",
      transcriptRef: "transcript.runtime.root",
      runRef: "run.runtime.root",
      parent: { kind: "root" },
      provider: { state: "known", kind: "codex", providerRef: "provider.codex.named" },
      runtime: { state: "known", kind: "codex_app_server", runtimeRef: "runtime.codex.named" },
      worktree: { state: "unknown", reason: "provider_omitted" },
      status: "running",
      attention: { state: "none" },
      terminal: { state: "active" },
      currentTool: { state: "none" },
      attachmentGeneration: 1,
      activityCursor: 2,
      createdAt: now,
      updatedAt: now,
      startedAt: now,
      endedAt: null,
      version: 2,
    },
    {
      agentRef: "agent.runtime.child",
      sessionRef: "session.runtime.live.1",
      threadRef: "thread.runtime.child",
      transcriptRef: "transcript.runtime.child",
      runRef: "run.runtime.child",
      parent: { kind: "agent", agentRef: "agent.runtime.root" },
      provider: { state: "known", kind: "codex", providerRef: "provider.codex.named" },
      runtime: { state: "known", kind: "codex_app_server", runtimeRef: "runtime.codex.child" },
      worktree: { state: "unknown", reason: "provider_omitted" },
      status: "running",
      attention: { state: "question", attentionRef: "question.runtime.child", since: now },
      terminal: { state: "active" },
      currentTool: { state: "known", toolCallRef: "tool.runtime.child", toolName: "Search", status: "running" },
      attachmentGeneration: 1,
      activityCursor: 1,
      createdAt: now,
      updatedAt: now,
      startedAt: now,
      endedAt: null,
      version: 1,
    },
  ],
  edges: [{
    edgeRef: "edge.runtime.root.child",
    kind: "parent",
    fromAgentRef: "agent.runtime.root",
    toAgentRef: "agent.runtime.child",
    version: 1,
  }],
  updatedAt: now,
})

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

  test("converges after a raced boot probe without polling or losing local threads", async () => {
    const localThread: DesktopThread = {
      id: "thread.local.raced",
      title: "Local draft",
      updatedAt: now,
      notes: [],
    }
    const local: ChatHost = {
      listThreads: async () => [localThread],
      newThread: async () => localThread,
      openThread: async id => id === localThread.id ? localThread : null,
      sendMessage: async () => ({ ok: true, thread: localThread }),
    }
    let catalogCalls = 0
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as { requestId?: string; query?: { id?: string } }
      if (value.query?.id !== "conversation.catalog") {
        return { kind: "request_rejected", reason: "invalid_request" }
      }
      catalogCalls += 1
      if (catalogCalls === 1) {
        return {
          kind: "conversation_unavailable",
          requestId: value.requestId!,
          reason: "not_live",
        }
      }
      return {
        kind: "conversation_catalog",
        requestId: value.requestId!,
        status,
        threads: [{
          threadRef: "thread.hosted.after-bootstrap",
          title: "Hosted after bootstrap",
          messageCount: 0,
          lastMessageAt: now,
          updatedAt: now,
          version: 1,
        }],
      }
    }
    const host = makeConvergingDesktopChatHost({ local, request })

    expect((await host.listThreads()).map(thread => thread.id)).toEqual([
      "thread.local.raced",
    ])
    expect((await host.listThreads()).map(thread => thread.id)).toEqual([
      "thread.hosted.after-bootstrap",
      "thread.local.raced",
    ])
    expect(catalogCalls).toBe(3)
    expect(await host.openThread("thread.local.raced")).toBe(localThread)
    // Known local authority never incurs another hosted admission query.
    expect(catalogCalls).toBe(3)
  })

  test("New Chat falls back to the durable local store when live Sync cannot confirm creation", async () => {
    const localThread: DesktopThread = {
      id: "thread.local.new-chat-fallback",
      title: "New chat",
      updatedAt: now,
      notes: [],
    }
    let localCreates = 0
    let runtimeCreates = 0
    const local: ChatHost = {
      listThreads: async () => [],
      newThread: async () => { localCreates += 1; return localThread },
      openThread: async id => id === localThread.id ? localThread : null,
      sendMessage: async () => ({ ok: true, thread: localThread }),
    }
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as { requestId?: string; query?: { id?: string }; command?: { id?: string } }
      if (value.query?.id === "conversation.catalog") {
        return { kind: "conversation_catalog", requestId: value.requestId!, status, threads: [] }
      }
      if (value.command?.id === "conversation.create") runtimeCreates += 1
      return { kind: "request_rejected", reason: "invalid_request" }
    }
    const host = makeConvergingDesktopChatHost({ local, request })

    expect(await host.newThread()).toEqual(localThread)
    expect(runtimeCreates).toBe(1)
    expect(localCreates).toBe(1)
    // The fallback ref is pinned local: opening it never probes Sync again.
    expect(await host.openThread(localThread.id)).toEqual(localThread)
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
      if (value.query?.id === "runtime.interactions") {
        return {
          kind: "runtime_interactions",
          requestId: value.requestId!,
          threadRef: value.query.threadRef!,
          interactions: [],
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
        graphRefs: [liveGraph.graphRef],
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
        graphs: [liveGraph],
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
    expect(result.thread?.agentGraph).toMatchObject({
      graphRef: liveGraph.graphRef,
      totalCount: 2,
      attentionCount: 1,
    })
    expect(result.thread?.agentGraph?.rows.map(row => [row.agentRef, row.depth])).toEqual([
      ["agent.runtime.root", 0],
      ["agent.runtime.child", 1],
    ])
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

describe("durable runtime turn controls (CUT-16)", () => {
  test("registers the enforced durable-runtime-controls contract", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    expect(openAgentsDesktopUxContractRegistry.contracts.find(
      contract => contract.contractId === "openagents_desktop.chat.durable_runtime_turn_controls.v1",
    )?.state).toBe("enforced")
  })

  const until = async (predicate: () => boolean): Promise<void> => {
    for (let attempt = 0; attempt < 2000 && !predicate(); attempt++) {
      await new Promise(resolve => setTimeout(resolve, 1))
    }
    expect(predicate()).toBe(true)
  }

  /**
   * Scripted durable fixture: a live subscription whose confirmed run stays
   * `running` (with an exact provider runtime) until the test terminalizes it,
   * so Stop and queue-until-idle can be exercised mid-flight.
   */
  const makeDurableControlFixture = (runtime: "claude_code" | "codex" | undefined) => {
    const threadRef = "thread.control.1"
    const listeners = new Set<(event: DesktopRuntimeGatewayEvent) => void>()
    const commands: Array<Record<string, unknown>> = []
    const messages: Array<{ messageRef: string; body: string }> = []
    let currentRun: { runRef: string; status: "running" | "completed" | "failed" | "canceled"; version: number } | null = null
    let subscriptionRef: string | null = null
    let sequence = 0

    const runProjection = () => currentRun === null ? null : {
      runRef: currentRun.runRef,
      routeRef: threadRef,
      ...(runtime === undefined ? {} : { runtime }),
      status: currentRun.status,
      createdAt: now,
      updatedAt: now,
      startedAt: now,
      completedAt: currentRun.status === "completed" ? now : null,
      failedAt: currentRun.status === "failed" ? now : null,
      canceledAt: currentRun.status === "canceled" ? now : null,
      version: currentRun.version,
    }
    const emit = (): void => {
      if (subscriptionRef === null) return
      sequence += 1
      const event: DesktopRuntimeGatewayEvent = {
        kind: "conversation.live.update",
        envelope: {
          kind: "conversation.live",
          delivery: "confirmed",
          subscriptionRef,
          generation: 1,
          sequence,
          threadRef,
          cursor: 5 + sequence,
          recovery: sequence === 1 ? "initial" : "resumed",
          messageRefs: messages.map(message => message.messageRef),
          eventRefs: [],
          graphRefs: [],
        },
        snapshot: {
          status: { phase: "live", cursor: 5 + sequence, pendingMutationCount: 0 },
          thread: {
            threadRef,
            title: "Controls",
            messageCount: messages.length,
            lastMessageAt: messages.length === 0 ? null : now,
            updatedAt: now,
            version: sequence,
          },
          messages: messages.map((message, index) => ({
            messageRef: message.messageRef,
            threadRef,
            body: message.body,
            createdAt: now,
            updatedAt: now,
            version: index + 1,
          })),
          timeline: currentRun === null ? null : {
            status: { phase: "live", cursor: 5 + sequence, pendingMutationCount: 0 },
            run: runProjection()!,
            events: [],
          },
          graphs: [],
        },
      }
      for (const listener of [...listeners]) listener(event)
    }
    const request = async (raw: unknown): Promise<DesktopRuntimeGatewayResponse> => {
      const value = raw as {
        command?: Record<string, string | number>
        commandId?: string
        query?: { id: string; intentId?: string; threadRef?: string }
        requestId?: string
      }
      if (value.query?.id === "conversation.catalog") {
        return {
          kind: "conversation_catalog",
          requestId: value.requestId!,
          status,
          threads: [{ threadRef, title: "Controls", messageCount: messages.length, lastMessageAt: messages.length === 0 ? null : now, updatedAt: now, version: 1 }],
        }
      }
      if (value.query?.id === "conversation.thread") {
        return {
          kind: "conversation_thread",
          requestId: value.requestId!,
          threadRef,
          status,
          messages: messages.map((message, index) => ({ messageRef: message.messageRef, threadRef, body: message.body, createdAt: now, updatedAt: now, version: index + 1 })),
        }
      }
      if (value.query?.id === "conversation.timeline") {
        return { kind: "conversation_timeline", requestId: value.requestId!, threadRef, status, run: runProjection(), events: [] }
      }
      if (value.query?.id === "runtime.interactions") {
        return { kind: "runtime_interactions", requestId: value.requestId!, threadRef, interactions: [] }
      }
      if (value.query?.id === "conversation.commandOutcome") {
        return { kind: "runtime_command_status", requestId: value.requestId!, commandRef: value.query.intentId!, threadRef, runRef: currentRun?.runRef ?? null, status: "settled", mutationId: 1, version: 3, updatedAt: now }
      }
      const command = value.command!
      commands.push(command)
      if (command.id === "conversation.subscribe") {
        subscriptionRef = String(command.subscriptionRef)
        emit()
        return { kind: "conversation_subscription_outcome", commandId: value.commandId!, subscriptionRef: subscriptionRef!, generation: Number(command.generation), status: "subscribed" }
      }
      if (command.id === "conversation.unsubscribe") {
        return { kind: "conversation_subscription_outcome", commandId: value.commandId!, subscriptionRef: String(command.subscriptionRef), generation: Number(command.generation), status: "unsubscribed" }
      }
      if (command.id === "conversation.append") {
        messages.push({ messageRef: String(command.messageRef), body: String(command.body) })
        queueMicrotask(emit)
        return { kind: "conversation_mutation_outcome", commandId: value.commandId!, status: "pending_reconcile", mutationId: commands.length }
      }
      if (command.id === "conversation.start") {
        currentRun = { runRef: String(command.runRef), status: "running", version: 3 }
        queueMicrotask(emit)
        return { kind: "runtime_command_outcome", commandId: value.commandId!, threadRef, runRef: String(command.runRef), messageRef: String(command.messageRef), status: "accepted", mutationId: commands.length }
      }
      if (command.id === "conversation.interrupt") {
        return { kind: "runtime_command_outcome", commandId: value.commandId!, threadRef, runRef: String(command.runRef), status: "unknown_pending_reconcile", mutationId: commands.length }
      }
      throw new Error(`unexpected command ${String(command.id)}`)
    }
    let nextId = 0
    const chat = makeRuntimeConversationChatHost({
      request,
      subscribe: listener => {
        listeners.add(listener)
        return () => listeners.delete(listener)
      },
      randomId: () => `control-${++nextId}`,
      liveTimeoutMs: 4000,
    })
    return {
      chat,
      commands,
      threadRef,
      terminalize: (terminal: "completed" | "failed" | "canceled") => {
        if (currentRun !== null) {
          currentRun = { ...currentRun, status: terminal, version: currentRun.version + 1 }
          emit()
        }
      },
      startedRunRefs: () => commands.filter(command => command.id === "conversation.start").map(command => String(command.runRef)),
    }
  }

  test("Stop interrupts the in-flight durable turn with the exact confirmed refs and lane", async () => {
    const fixture = makeDurableControlFixture("claude_code")
    // No harness given: the lane must come from the CONFIRMED run runtime.
    const sendPromise = fixture.chat.sendMessage({ id: fixture.threadRef, message: "First" })
    await until(() => fixture.commands.some(command => command.id === "conversation.start"))
    // Wait until the observer has seen the confirmed running timeline.
    await until(() => fixture.commands.length >= 3)
    await new Promise(resolve => setTimeout(resolve, 5))

    expect(await fixture.chat.interruptActive!()).toBe(true)
    const interrupt = fixture.commands.find(command => command.id === "conversation.interrupt")
    const runRef = fixture.startedRunRefs()[0]!
    expect(interrupt).toEqual({
      id: "conversation.interrupt",
      commandRef: `desktop.interrupt.${runRef}`,
      threadRef: fixture.threadRef,
      runRef,
      lane: "claude_pylon",
      expectedVersion: 3,
    })

    // Admission truth only: the CONFIRMED canceled terminal finalizes the turn.
    fixture.terminalize("canceled")
    const result = await sendPromise
    expect(result.ok).toBe(true)
    // The send is settled, so Stop no longer has an in-flight durable target.
    expect(await fixture.chat.interruptActive!()).toBe(false)
    expect(fixture.commands.filter(command => command.id === "conversation.interrupt")).toHaveLength(1)
  })

  test("Stop without an in-flight durable send is a no-op that sends nothing", async () => {
    const fixture = makeDurableControlFixture("codex")
    expect(await fixture.chat.interruptActive!()).toBe(false)
    expect(fixture.commands).toEqual([])
  })

  test("queue-until-idle promotes the follow-up only at the confirmed terminal on the same lane", async () => {
    const fixture = makeDurableControlFixture("codex")
    const sendPromise = fixture.chat.sendMessage({ id: fixture.threadRef, message: "First", harness: "codex" })
    await until(() => fixture.commands.some(command => command.id === "conversation.start"))

    expect(await fixture.chat.queueFollowup!({ threadRef: fixture.threadRef, message: "Queued follow-up" }))
      .toEqual({ ok: true, queued: true })
    // Nothing is appended while the first turn still streams.
    expect(fixture.commands.filter(command => command.id === "conversation.append")).toHaveLength(1)

    fixture.terminalize("completed")
    await until(() => fixture.startedRunRefs().length === 2)
    fixture.terminalize("completed")

    const result = await sendPromise
    expect(result.ok).toBe(true)
    const appends = fixture.commands
      .filter(command => command.id === "conversation.append")
      .map(command => command.body)
    expect(appends).toEqual(["First", "Queued follow-up"])
    const starts = fixture.commands.filter(command => command.id === "conversation.start")
    expect(starts.map(command => command.lane)).toEqual(["codex_app_server", "codex_app_server"])
    expect(new Set(fixture.startedRunRefs()).size).toBe(2)
    expect(result.thread?.notes.filter(note => note.role === "user").map(note => note.text))
      .toEqual(["First", "Queued follow-up"])
  })

  test("queueFollowup without an in-flight durable send reports queued:false and sends nothing", async () => {
    const fixture = makeDurableControlFixture("codex")
    expect(await fixture.chat.queueFollowup!({ threadRef: fixture.threadRef, message: "Orphaned" }))
      .toEqual({ ok: false, queued: false })
    expect(fixture.commands).toEqual([])
  })
})
