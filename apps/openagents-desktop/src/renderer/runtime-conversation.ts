import type { DesktopThread } from "../chat-contract.ts"
import {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
} from "../agent-graph-presentation.ts"
import type {
  ConfirmedRuntimeInteraction,
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import type { ChatHost } from "./shell.ts"
import {
  openDesktopRuntimeLiveThread,
  type DesktopRuntimeLiveHandle,
  type DesktopRuntimeLiveUpdate,
} from "./runtime-live-client.ts"

export type RuntimeConversationRequest = (
  value: unknown,
) => Promise<DesktopRuntimeGatewayResponse>

export type RuntimeConversationOptions = Readonly<{
  request: RuntimeConversationRequest
  subscribe?: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  randomId?: () => string
  liveTimeoutMs?: number
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

export const runtimeInteractionNotes = (
  interactions: ReadonlyArray<ConfirmedRuntimeInteraction>,
): DesktopThread["notes"] => interactions.map(interaction => {
  const questions = interaction.kind === "provider_question"
    ? interaction.questions.map(question => ({
        questionRef: question.questionRef,
        question: question.displayText,
        header: interaction.displayTitle,
        multiSelect: question.multiSelect,
        options: question.options.map(option => ({
          optionRef: option.optionRef,
          label: option.label,
          ...(option.description === undefined ? {} : { description: option.description }),
        })),
      }))
    : [{
        question: interaction.displayText,
        header: interaction.displayTitle,
        multiSelect: false,
        options: interaction.kind === "tool_approval"
          ? [
              { optionRef: "approve", label: "Approve" },
              { optionRef: "deny", label: "Deny" },
            ]
          : [
              { optionRef: "accept", label: "Accept" },
              { optionRef: "request_changes", label: "Request changes" },
              { optionRef: "replan", label: "Replan" },
            ],
      }]
  return {
    key: `runtime-interaction-${interaction.interactionRef}`,
    role: "system" as const,
    text: interaction.displayText,
    timestamp: timestamp(interaction.requestedAt),
    question: {
      turnRef: interaction.turnId,
      threadRef: interaction.threadId,
      questionRef: interaction.interactionRef,
      status: interaction.status,
      source: "runtime" as const,
      kind: interaction.kind,
      ...(interaction.decisionRef === undefined ? {} : { decisionRef: interaction.decisionRef }),
      questions,
    },
  }
})

type LiveTimeline = NonNullable<DesktopRuntimeLiveUpdate["snapshot"]>["timeline"]
type LiveGraphs = NonNullable<DesktopRuntimeLiveUpdate["snapshot"]>["graphs"]

const projectedThread = (input: Readonly<{
  summary: Readonly<{ threadRef: string; title: string; updatedAt: string }>
  messages: NonNullable<DesktopRuntimeLiveUpdate["snapshot"]>["messages"]
  timeline: LiveTimeline
  graphs?: LiveGraphs
  interactions?: ReadonlyArray<ConfirmedRuntimeInteraction>
}>): DesktopThread => ({
  ...threadSummary(input.summary),
  ...(() => {
    const graph = newestLiveAgentGraph(input.graphs ?? [])
    return graph === null
      ? {}
      : { agentGraph: projectLiveAgentGraphPresentation(graph, { maxRows: 200 }) }
  })(),
  notes: [
    ...input.messages.map(message => ({
      key: message.messageRef,
      role: "user" as const,
      text: message.body,
      timestamp: timestamp(message.createdAt),
    })),
    ...timelineNotes(input.timeline?.events ?? []),
    ...runtimeInteractionNotes(input.interactions ?? []),
  ],
})

type LiveThreadObserver = Readonly<{
  close: () => Promise<void>
  current: () => DesktopThread | null
  timeline: () => LiveTimeline
  waitFor: (predicate: () => boolean) => Promise<boolean>
}>

export const makeRuntimeConversationChatHost = (
  options: RuntimeConversationOptions,
): ChatHost => {
  const randomId = options.randomId ?? (() => globalThis.crypto.randomUUID())
  const liveTimeoutMs = options.liveTimeoutMs ?? 30_000

  const catalog = () => options.request({
    kind: "query",
    requestId: `renderer-conversation-catalog-${++requestSequence}`,
    query: { id: "conversation.catalog" },
  })

  const confirmedThread = async (
    threadRef: string,
    requiredMessageRef?: string,
  ): Promise<DesktopThread | null> => {
    const [catalogResult, threadResult, timelineResult, interactionResult] = await Promise.all([
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
      options.request({
        kind: "query",
        requestId: `renderer-conversation-interactions-${++requestSequence}`,
        query: { id: "runtime.interactions", threadRef },
      }).catch(() => null),
    ])
    if (
      catalogResult.kind !== "conversation_catalog" ||
      catalogResult.status.phase !== "live" ||
      threadResult.kind !== "conversation_thread" ||
      threadResult.status.phase !== "live"
    ) return null
    const summary = catalogResult.threads.find(thread => thread.threadRef === threadRef)
    if (summary === undefined) return null
    const projected = projectedThread({
      summary,
      messages: threadResult.messages,
      timeline: timelineResult.kind === "conversation_timeline"
        ? { status: timelineResult.status, run: timelineResult.run, events: timelineResult.events }
        : null,
      interactions: interactionResult?.kind === "runtime_interactions"
        ? interactionResult.interactions
        : [],
    })
    return requiredMessageRef === undefined || projected.notes.some(note => note.key === requiredMessageRef)
      ? projected
      : null
  }

  const openLiveObserver = async (
    threadRef: string,
    onThread?: (thread: DesktopThread) => void,
  ): Promise<LiveThreadObserver | null> => {
    if (options.subscribe === undefined) return null
    let current: DesktopThread | null = null
    let timeline: LiveTimeline = null
    let lastSignature = ""
    const waiters = new Set<() => void>()
    const signal = (): void => {
      for (const waiter of [...waiters]) waiter()
    }
    const subscriptionRef = `subscription.renderer.conversation.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
    let handle: DesktopRuntimeLiveHandle | null = null
    handle = await openDesktopRuntimeLiveThread({
      bridge: { request: options.request, subscribe: options.subscribe },
      subscriptionRef,
      generation: 1,
      threadRef,
      onUpdate: update => {
        if (update.snapshot === null || update.snapshot.status.phase !== "live") return
        timeline = update.snapshot.timeline
        if (update.snapshot.thread !== null) {
          const snapshot = update.snapshot
          void options.request({
            kind: "query",
            requestId: `renderer-conversation-live-interactions-${++requestSequence}`,
            query: { id: "runtime.interactions", threadRef },
          }).catch(() => null).then(interactionResult => {
            current = projectedThread({
              summary: snapshot.thread!,
              messages: snapshot.messages,
              timeline,
              graphs: snapshot.graphs,
              interactions: interactionResult?.kind === "runtime_interactions"
                ? interactionResult.interactions
                : [],
            })
            const signature = JSON.stringify({
              notes: current.notes.map(note => [note.key, note.role, note.text, note.question?.status, note.question?.decisionRef]),
              graph: current.agentGraph ?? null,
            })
            if (signature !== lastSignature) {
              lastSignature = signature
              onThread?.(current)
            }
            signal()
          })
        }
        if (update.snapshot.thread === null) signal()
      },
    })
    if (handle === null) return null
    return {
      close: handle.close,
      current: () => current,
      timeline: () => timeline,
      waitFor: predicate => {
        if (predicate()) return Promise.resolve(true)
        return new Promise(resolve => {
          const timeout = setTimeout(() => {
            waiters.delete(check)
            resolve(false)
          }, liveTimeoutMs)
          const check = (): void => {
            if (!predicate()) return
            clearTimeout(timeout)
            waiters.delete(check)
            resolve(true)
          }
          waiters.add(check)
        })
      },
    }
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
      const confirmed = await confirmedThread(threadRef)
      if (confirmed !== null || options.subscribe === undefined) return confirmed
      const observer = await openLiveObserver(threadRef)
      if (observer === null) return null
      try {
        return await observer.waitFor(() => observer.current() !== null)
          ? observer.current()
          : null
      } finally {
        await observer.close()
      }
    },
    openThread: confirmedThread,
    hydrateThread: async threadRef => {
      const confirmed = await confirmedThread(threadRef)
      if (options.subscribe === undefined) return confirmed
      const observer = await openLiveObserver(threadRef)
      if (observer === null) return confirmed
      try {
        return await observer.waitFor(() => observer.current() !== null)
          ? observer.current()
          : confirmed
      } finally {
        await observer.close()
      }
    },
    sendMessage: async input => {
      const messageRef = nextId("message", randomId)
      const runRef = `turn.desktop.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
      const observer = await openLiveObserver(input.id, input.onUpdate)
      if (options.subscribe !== undefined && observer === null) {
        return { ok: false, error: "Live conversation subscription is unavailable." }
      }
      try {
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

        let thread = observer === null
          ? await confirmedThread(input.id, messageRef)
          : await observer.waitFor(() => observer.current()?.notes.some(note => note.key === messageRef) === true)
            ? observer.current()
            : null
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

        const commandStatus = async () => options.request({
          kind: "query",
          requestId: `renderer-conversation-command-${++requestSequence}`,
          query: {
            id: "conversation.commandOutcome",
            intentId: `intent.start.${runRef}`,
            threadRef: input.id,
          },
        })
        const command = await commandStatus()
        if (command.kind === "runtime_command_status" && command.status === "expired") {
          return { ok: false, error: "Runtime command expired while this device was offline." }
        }
        if (observer !== null) {
          const terminal = await observer.waitFor(() => {
            const run = observer.timeline()?.run
            return run?.runRef === runRef && (
              run.status === "completed" || run.status === "failed" || run.status === "canceled"
            )
          })
          thread = observer.current() ?? thread
          if (terminal) return { ok: true, thread }
          const finalCommand = await commandStatus()
          if (finalCommand.kind === "runtime_command_status" && finalCommand.status === "expired") {
            return { ok: false, error: "Runtime command expired while this device was offline." }
          }
        } else {
          thread = await confirmedThread(input.id) ?? thread
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
      } finally {
        await observer?.close()
      }
    },
  }
}

export type DesktopChatHostSelection = Readonly<{
  host: ChatHost
  /**
   * "runtime" when the authoritative Khala Sync conversation catalog is live
   * (signed in); "local" otherwise. Callers gate harness-lane availability on
   * this evidence (#8712) — the runtime host serves both lanes, the local
   * host serves only the fable-local lane.
   */
  mode: "runtime" | "local"
}>

export const selectDesktopChatHostSelection = async (input: Readonly<{
  request: RuntimeConversationRequest | undefined
  subscribe?: RuntimeConversationOptions["subscribe"]
  local: ChatHost
  options?: Omit<RuntimeConversationOptions, "request" | "subscribe">
}>): Promise<DesktopChatHostSelection> => {
  if (input.request === undefined) return { host: input.local, mode: "local" }
  const result = await input.request({
    kind: "query",
    requestId: `renderer-conversation-mode-${++requestSequence}`,
    query: { id: "conversation.catalog" },
  })
  return result.kind === "conversation_catalog" && result.status.phase === "live"
    ? { host: makeRuntimeConversationChatHost({ request: input.request, subscribe: input.subscribe, ...input.options }), mode: "runtime" }
    : { host: input.local, mode: "local" }
}

export const selectDesktopChatHost = async (input: Readonly<{
  request: RuntimeConversationRequest | undefined
  subscribe?: RuntimeConversationOptions["subscribe"]
  local: ChatHost
  options?: Omit<RuntimeConversationOptions, "request" | "subscribe">
}>): Promise<ChatHost> => (await selectDesktopChatHostSelection(input)).host
