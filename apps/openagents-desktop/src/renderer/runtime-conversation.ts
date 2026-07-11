import type { DesktopThread } from "../chat-contract.ts"
import type { DesktopRuntimeGatewayResponse } from "../runtime-gateway-contract.ts"
import type { ChatHost } from "./shell.ts"

export type RuntimeConversationRequest = (
  value: unknown,
) => Promise<DesktopRuntimeGatewayResponse>

export type RuntimeConversationOptions = Readonly<{
  request: RuntimeConversationRequest
  randomId?: () => string
  sleep?: (ms: number) => Promise<void>
  pollAttempts?: number
}>

let requestSequence = 0

const timestamp = (value: string): string => {
  const date = new Date(value)
  return Number.isFinite(date.getTime())
    ? date.toISOString().slice(11, 16)
    : "--:--"
}

const threadSummary = (thread: Readonly<{
  threadRef: string
  title: string
  updatedAt: string
}>): DesktopThread => ({
  id: thread.threadRef,
  title: thread.title,
  updatedAt: thread.updatedAt,
  notes: [],
})

const nextId = (kind: "thread" | "message", randomId: () => string): string =>
  `${kind}.desktop.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`

const timelineNotes = (
  events: Extract<DesktopRuntimeGatewayResponse, { kind: "conversation_timeline" }>["events"],
): DesktopThread["notes"] => {
  const notes: Array<DesktopThread["notes"][number]> = []
  const textByMessage = new Map<string, number>()
  for (const event of events) {
    const item = event.item
    if (item == null) continue
    if (item.kind === "text") {
      const index = textByMessage.get(item.messageRef)
      if (index === undefined) {
        textByMessage.set(item.messageRef, notes.length)
        notes.push({ key: event.eventRef, role: "assistant", text: item.text, timestamp: timestamp(event.createdAt) })
      } else {
        const previous = notes[index]!
        notes[index] = { ...previous, text: previous.text + item.text }
      }
      continue
    }
    const text = item.kind === "reasoning"
      ? `Reasoning · ${item.text}`
      : item.kind === "connected"
        ? `Connected · ${item.lane}`
        : item.kind === "tool"
          ? `${item.toolName} · ${item.status}`
          : item.kind === "plan"
            ? `Plan · ${item.status}`
            : item.kind === "usage"
              ? `Usage · ${item.totalTokens ?? 0} tokens`
              : item.kind === "terminal"
                ? `Turn ${item.status}`
                : item.kind === "interrupted"
                  ? "Turn interrupted"
                  : item.kind === "approval"
                    ? `Approval · ${item.status}`
                    : item.kind === "question"
                      ? item.prompt
                      : item.kind === "error"
                        ? item.messageSafe
                        : item.detail
    notes.push({ key: event.eventRef, role: "system", text, timestamp: timestamp(event.createdAt) })
  }
  return notes
}

export const makeRuntimeConversationChatHost = (
  options: RuntimeConversationOptions,
): ChatHost => {
  const randomId = options.randomId ?? (() => globalThis.crypto.randomUUID())
  const sleep = options.sleep ?? (ms => new Promise(resolve => setTimeout(resolve, ms)))
  const pollAttempts = options.pollAttempts ?? 30

  const catalog = () => options.request({
    kind: "query",
    requestId: `renderer-conversation-catalog-${++requestSequence}`,
    query: { id: "conversation.catalog" },
  })

  const confirmedThread = async (
    threadRef: string,
    requiredMessageRef?: string,
  ): Promise<DesktopThread | null> => {
    for (let attempt = 0; attempt < pollAttempts; attempt += 1) {
      const [catalogResult, threadResult, timelineResult] = await Promise.all([
        catalog(),
        options.request({
          kind: "query",
          requestId: `renderer-conversation-thread-${++requestSequence}`,
          query: { id: "conversation.thread", threadRef },
        }),
        options.request({
          kind: "query",
          requestId: `renderer-conversation-timeline-${++requestSequence}`,
          query: { id: "conversation.timeline", threadRef },
        }),
      ])
      if (
        catalogResult.kind === "conversation_catalog" &&
        catalogResult.status.phase === "live" &&
        threadResult.kind === "conversation_thread" &&
        threadResult.status.phase === "live"
      ) {
        const summary = catalogResult.threads.find(thread => thread.threadRef === threadRef)
        if (summary !== undefined) {
          const projected: DesktopThread = {
            ...threadSummary(summary),
            notes: [
              ...threadResult.messages.map(message => ({
                key: message.messageRef,
                role: "user" as const,
                text: message.body,
                timestamp: timestamp(message.createdAt),
              })),
              ...(timelineResult.kind === "conversation_timeline"
                ? timelineNotes(timelineResult.events)
                : []),
            ],
          }
          if (
            requiredMessageRef === undefined ||
            projected.notes.some(note => note.key === requiredMessageRef)
          ) return projected
        }
      }
      await sleep(100)
    }
    return null
  }

  return {
    listThreads: async () => {
      const result = await catalog()
      return result.kind === "conversation_catalog" && result.status.phase === "live"
        ? result.threads.map(threadSummary)
        : []
    },
    newThread: async () => {
      const threadRef = nextId("thread", randomId)
      const outcome = await options.request({
        kind: "command",
        commandId: `renderer-conversation-create-${++requestSequence}`,
        command: { id: "conversation.create", threadRef, title: "New chat" },
      })
      if (
        outcome.kind !== "conversation_mutation_outcome" ||
        outcome.status !== "pending_reconcile"
      ) return null
      return confirmedThread(threadRef)
    },
    openThread: confirmedThread,
    hydrateThread: confirmedThread,
    sendMessage: async input => {
      const messageRef = nextId("message", randomId)
      const runRef = `turn.desktop.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
      const outcome = await options.request({
        kind: "command",
        commandId: `renderer-conversation-append-${++requestSequence}`,
        command: {
          id: "conversation.append",
          threadRef: input.id,
          messageRef,
          body: input.message,
        },
      })
      if (
        outcome.kind !== "conversation_mutation_outcome" ||
        outcome.status !== "pending_reconcile"
      ) return { ok: false, error: "Authoritative conversation Sync is unavailable." }

      let thread = await confirmedThread(input.id, messageRef)
      if (thread === null) {
        return {
          ok: false,
          error: "Message is still pending reconciliation.",
        }
      }
      const started = await options.request({
        kind: "command",
        commandId: `renderer-conversation-start-${++requestSequence}`,
        command: {
          id: "conversation.start",
          messageRef,
          runRef,
          threadRef: input.id,
          ...(input.harness === undefined ? {} : {
            lane: input.harness === "fable" ? "claude_pylon" as const : "codex_app_server" as const,
          }),
        },
      })
      if (
        started.kind !== "runtime_command_outcome" ||
        (started.status !== "accepted" && started.status !== "unknown_pending_reconcile")
      ) {
        return { ok: false, error: "Message was admitted, but agent dispatch was rejected." }
      }

      let lastSignature = ""
      for (let attempt = 0; attempt < Math.max(pollAttempts, 300); attempt += 1) {
        const command = await options.request({
          kind: "query",
          requestId: `renderer-conversation-command-${++requestSequence}`,
          query: {
            id: "conversation.commandOutcome",
            intentId: `intent.start.${runRef}`,
            threadRef: input.id,
          },
        })
        if (command.kind === "runtime_command_status" && command.status === "expired") {
          return { ok: false, error: "Runtime command expired while this device was offline." }
        }
        const next = await confirmedThread(input.id)
        if (next !== null) {
          thread = next
          const signature = next.notes.map(note => `${note.key}:${note.text.length}`).join("|")
          if (signature !== lastSignature) {
            lastSignature = signature
            input.onUpdate?.(next)
          }
        }
        const timeline = await options.request({
          kind: "query",
          requestId: `renderer-conversation-stream-${++requestSequence}`,
          query: { id: "conversation.timeline", threadRef: input.id },
        })
        if (
          timeline.kind === "conversation_timeline" &&
          timeline.run?.runRef === runRef &&
          (timeline.run.status === "completed" ||
            timeline.run.status === "failed" ||
            timeline.run.status === "canceled")
        ) return { ok: true, thread }
        await sleep(100)
      }
      return {
        ok: false,
        error: "Run outcome is still pending reconciliation.",
        thread: {
          ...thread,
          notes: [...thread.notes, {
            key: `pending-reconcile-${runRef}`,
            role: "system",
            text: "Run outcome is still pending reconciliation.",
            timestamp: "--:--",
          }],
        },
      }
    },
  }
}

export const selectDesktopChatHost = async (input: Readonly<{
  request: RuntimeConversationRequest | undefined
  local: ChatHost
  options?: Omit<RuntimeConversationOptions, "request">
}>): Promise<ChatHost> => {
  if (input.request === undefined) return input.local
  const result = await input.request({
    kind: "query",
    requestId: `renderer-conversation-mode-${++requestSequence}`,
    query: { id: "conversation.catalog" },
  })
  return result.kind === "conversation_catalog" && result.status.phase === "live"
    ? makeRuntimeConversationChatHost({ request: input.request, ...input.options })
    : input.local
}
