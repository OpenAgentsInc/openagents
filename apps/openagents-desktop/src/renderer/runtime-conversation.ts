import { compareDesktopThreadsByCreatedAt, type DesktopThread } from "../chat-contract.ts"
import {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
} from "../agent-graph-presentation.ts"
import type {
  ConfirmedRuntimeInteraction,
  DesktopRuntimeControlLane,
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import {
  makeComposerInterruptIntent,
  makeComposerInterruptOutcome,
  makeComposerSubmitOutcome,
} from "../composer-admission.ts"
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
  now?: () => Date
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
  createdAt?: string
  updatedAt: string
}>): DesktopThread => ({
  id: thread.threadRef,
  title: thread.title,
  ...(thread.createdAt === undefined ? {} : { createdAt: thread.createdAt }),
  updatedAt: thread.updatedAt,
  notes: [],
})

const nextId = (kind: "thread" | "message", randomId: () => string): string =>
  `${kind}.desktop.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`

/**
 * Exact lane for a confirmed durable run (CUT-16). The durable lane fence
 * rejects control intents whose target lane mismatches the stored turn lane,
 * so controls derive the lane from the confirmed run projection first and
 * only fall back to the lane this renderer requested at dispatch. An unknown
 * lane stays null — the caller omits the field rather than guessing.
 */
export const laneForConfirmedRun = (
  runtime: string | undefined,
  harness?: "fable" | "codex",
): DesktopRuntimeControlLane | null =>
  runtime === "claude_code"
    ? "claude_pylon"
    : runtime === "openagents_native"
      ? "hosted_khala"
      : runtime === "codex" || runtime === "opencode_codex"
        ? "codex_app_server"
        : harness === "fable"
          ? "claude_pylon"
          : harness === "codex"
            ? "codex_app_server"
            : null

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

type ActiveDurableSend = Readonly<{
  threadRef: string
  runRef: string
  harness?: "fable" | "codex"
  observer: LiveThreadObserver | null
}>

export const makeRuntimeConversationChatHost = (
  options: RuntimeConversationOptions,
): ChatHost => {
  const randomId = options.randomId ?? (() => globalThis.crypto.randomUUID())
  const now = options.now ?? (() => new Date())
  const liveTimeoutMs = options.liveTimeoutMs ?? 30_000

  /**
   * The durable send this renderer currently has in flight (CUT-16 runtime
   * controls). Stop/queue affordances act only on this exact confirmed
   * thread/run — the host never invents authority over turns started
   * elsewhere.
   */
  let activeSend: ActiveDurableSend | null = null
  const followupQueue: Array<{ threadRef: string; message: string }> = []

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
              title: current.title,
              updatedAt: current.updatedAt,
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

  /**
   * One durable turn: append the message, start the run, and wait for the
   * exact confirmed terminal. Extracted so queue-until-idle can promote a
   * queued follow-up through the identical confirmed-only path. Registers the
   * in-flight run as the active send so Stop targets the exact thread/run.
   */
  const runDurableTurn = async (input: Readonly<{
    threadRef: string
    message: string
    harness?: "fable" | "codex"
    observer: LiveThreadObserver | null
    messageRef?: string
    runRef?: string
  }>): Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>> => {
    const messageRef = input.messageRef ?? nextId("message", randomId)
    const runRef = input.runRef ?? `turn.desktop.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
    const observer = input.observer
    activeSend = {
      threadRef: input.threadRef,
      runRef,
      ...(input.harness === undefined ? {} : { harness: input.harness }),
      observer,
    }
    const outcome = await options.request({
      kind: "command",
      commandId: `renderer-conversation-append-${++requestSequence}`,
      command: {
        id: "conversation.append",
        threadRef: input.threadRef,
        messageRef,
        body: input.message,
      },
    })
    if (
      outcome.kind !== "conversation_mutation_outcome" ||
      outcome.status !== "pending_reconcile"
    ) return { ok: false, error: "Authoritative conversation Sync is unavailable." }

    let thread = observer === null
      ? await confirmedThread(input.threadRef, messageRef)
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
        threadRef: input.threadRef,
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
        threadRef: input.threadRef,
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
      thread = await confirmedThread(input.threadRef) ?? thread
      const timeline = await options.request({
        kind: "query",
        requestId: `renderer-conversation-stream-${++requestSequence}`,
        query: { id: "conversation.timeline", threadRef: input.threadRef },
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
  }

  return {
    listThreads: async () => {
      const result = await catalog()
      return result.kind === "conversation_catalog" && result.status.phase === "live"
        ? result.threads.map(threadSummary).sort(compareDesktopThreadsByCreatedAt)
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
        return { ok: false, error: "Live conversation subscription is unavailable.", failureKind: "offline" as const }
      }
      try {
        let result = await runDurableTurn({
          threadRef: input.id,
          message: input.message,
          ...(input.harness === undefined ? {} : { harness: input.harness }),
          observer,
          messageRef,
          runRef,
        })
        // Queue-until-idle (CUT-16): follow-ups queued while this durable
        // send streamed are delivered now — at the previous turn's CONFIRMED
        // terminal — as real appended messages plus started turns on the same
        // lane. A failed promotion stops the drain and reports honestly
        // instead of silently dropping the queued text.
        while (result.ok) {
          const index = followupQueue.findIndex(entry => entry.threadRef === input.id)
          if (index === -1) break
          const entry = followupQueue.splice(index, 1)[0]!
          const followup = await runDurableTurn({
            threadRef: input.id,
            message: entry.message,
            ...(input.harness === undefined ? {} : { harness: input.harness }),
            observer,
          })
          result = followup.ok ? followup : {
            ok: false,
            error: "A queued follow-up could not be delivered.",
            thread: followup.thread ?? result.thread,
          }
        }
        return result
      } finally {
        activeSend = null
        for (let index = followupQueue.length - 1; index >= 0; index--) {
          if (followupQueue[index]!.threadRef === input.id) followupQueue.splice(index, 1)
        }
        await observer?.close()
      }
    },
    interruptActiveControlIdentity: async threadRef => {
      const send = activeSend
      if (send === null || (threadRef !== undefined && send.threadRef !== threadRef)) return null
      const intentRef = `desktop.interrupt.${send.runRef}`
      return { threadRef: send.threadRef, intentRef, idempotencyKey: intentRef }
    },
    interruptActiveControl: async threadRef => {
      // Stop acts only on the exact durable send this renderer has in flight.
      // Admission truth only: the confirmed canceled terminal (not this
      // acknowledgement) is what finalizes the turn and reverts the composer.
      const send = activeSend
      if (send === null || (threadRef !== undefined && send.threadRef !== threadRef)) return null
      const run = send.observer !== null
        ? send.observer.timeline()?.run ?? null
        : await (async () => {
            const timeline = await options.request({
              kind: "query",
              requestId: `renderer-conversation-interrupt-run-${++requestSequence}`,
              query: { id: "conversation.timeline", threadRef: send.threadRef },
            })
            return timeline.kind === "conversation_timeline" ? timeline.run : null
          })()
      if (run === null || run.runRef !== send.runRef) return null
      if (run.status === "completed" || run.status === "failed" || run.status === "canceled") return null
      const createdAt = now().toISOString()
      const control = makeComposerInterruptIntent({
        threadRef: send.threadRef,
        turnRef: send.runRef,
        intentRef: `desktop.interrupt.${send.runRef}`,
        createdAt,
        targetGeneration: { state: "known", value: run.version },
      })
      const lane = laneForConfirmedRun(run.runtime, send.harness)
      const outcome = await options.request({
        kind: "command",
        commandId: `renderer-conversation-interrupt-${++requestSequence}`,
        command: {
          id: "conversation.interrupt",
          commandRef: control.intentRef,
          threadRef: send.threadRef,
          runRef: send.runRef,
          ...(lane === null ? {} : { lane }),
          expectedVersion: run.version,
        },
      })
      const observedAt = now().toISOString()
      if (outcome.kind !== "runtime_command_outcome") {
        return makeComposerInterruptOutcome({
          control,
          observedAt,
          admission: { status: "rejected", reasonRef: "reason.invalid_outcome" },
          delivery: { status: "failed", reasonRef: "reason.invalid_outcome" },
        })
      }
      if (outcome.status === "accepted") {
        return makeComposerInterruptOutcome({
          control,
          observedAt,
          admission: { status: "accepted", acceptedAt: observedAt },
          delivery: { status: "pending" },
        })
      }
      if (outcome.status === "unknown_pending_reconcile") {
        return makeComposerInterruptOutcome({
          control,
          observedAt,
          admission: { status: "pending" },
          delivery: { status: "pending" },
        })
      }
      const reasonRef = outcome.status === "unavailable"
        ? "reason.adapter_unavailable"
        : "reason.adapter_rejected"
      return makeComposerInterruptOutcome({
        control,
        observedAt,
        admission: { status: "rejected", reasonRef },
        delivery: outcome.status === "unavailable"
          ? { status: "unsupported", reasonRef }
          : { status: "failed", reasonRef },
      })
    },
    queueFollowup: async input => {
      const send = activeSend
      if (send === null || send.threadRef !== input.threadRef || input.message.trim() === "") {
        return { ok: false, queued: false }
      }
      followupQueue.push({ threadRef: input.threadRef, message: input.message })
      return { ok: true, queued: true }
    },
    queueFollowupControl: async input => {
      const send = activeSend
      const observedAt = now().toISOString()
      if (
        send === null || send.threadRef !== input.threadRef || input.message.trim() === "" ||
        input.control.kind !== "turn.queue" ||
        input.control.threadRef !== input.threadRef ||
        input.control.messageRef !== input.clientUserMessageId
      ) {
        return makeComposerSubmitOutcome({
          control: input.control,
          observedAt,
          admission: { status: "rejected", reasonRef: "reason.target_mismatch" },
          delivery: { status: "failed", reasonRef: "reason.target_mismatch" },
        })
      }
      followupQueue.push({ threadRef: input.threadRef, message: input.message })
      return makeComposerSubmitOutcome({
        control: input.control,
        observedAt,
        admission: { status: "accepted", acceptedAt: observedAt },
        delivery: { status: "queued", queueRef: `queue.${input.intentRef}` },
      })
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

/**
 * A renderer can mount while an already-verified Sync session is still
 * bootstrapping. Pinning the one-shot boot result made that transient state a
 * lifetime decision. This facade re-admits each user operation with one
 * authoritative catalog query: no interval, no timeline poller, and no delay
 * to first paint. Threads retain the host that created/opened them so a local
 * draft is never silently reinterpreted as a hosted ref when Sync comes live.
 */
export const makeConvergingDesktopChatHost = (input: Readonly<{
  request: RuntimeConversationRequest | undefined
  subscribe?: RuntimeConversationOptions["subscribe"]
  local: ChatHost
  options?: Omit<RuntimeConversationOptions, "request" | "subscribe">
}>): ChatHost => {
  if (input.request === undefined) return input.local
  const runtime = makeRuntimeConversationChatHost({
    request: input.request,
    subscribe: input.subscribe,
    ...input.options,
  })
  const threadModes = new Map<string, "local" | "runtime">()
  let active: ChatHost | null = null

  const current = async (): Promise<Readonly<{ host: ChatHost; mode: "local" | "runtime" }>> => {
    const result = await input.request!({
      kind: "query",
      requestId: `renderer-conversation-converge-${++requestSequence}`,
      query: { id: "conversation.catalog" },
    })
    return result.kind === "conversation_catalog" && result.status.phase === "live"
      ? { host: runtime, mode: "runtime" }
      : { host: input.local, mode: "local" }
  }
  const remember = (thread: DesktopThread | null, mode: "local" | "runtime"): DesktopThread | null => {
    if (thread !== null) threadModes.set(thread.id, mode)
    return thread
  }
  const hostForThread = async (threadRef: string): Promise<Readonly<{
    host: ChatHost
    mode: "local" | "runtime"
  }>> => {
    const known = threadModes.get(threadRef)
    if (known === "local") return { host: input.local, mode: "local" }
    if (known === "runtime") return { host: runtime, mode: "runtime" }
    return current()
  }

  return {
    listThreads: async () => {
      const selected = await current()
      if (selected.mode === "local") {
        const local = await input.local.listThreads()
        for (const thread of local) threadModes.set(thread.id, "local")
        return local
      }
      const [hosted, local] = await Promise.all([
        runtime.listThreads(),
        input.local.listThreads(),
      ])
      for (const thread of hosted) threadModes.set(thread.id, "runtime")
      // Sync can mirror an app-local draft under the exact same stable ref.
      // The local store remains the creation/open authority for that ref;
      // allowing the hosted catalog to win makes the row inert while the
      // hosted detail projection is still catching up. Re-apply local modes
      // after hosted discovery and render the local post-image for duplicates.
      for (const thread of local) threadModes.set(thread.id, "local")
      const localRefs = new Set(local.map(thread => thread.id))
      return [...local, ...hosted.filter(thread => !localRefs.has(thread.id))]
        .sort(compareDesktopThreadsByCreatedAt)
    },
    newThread: async laneRef => {
      // New Chat is local-first navigation. It must never wait behind live
      // Sync's unbounded pending-reconciliation path: create durably in the
      // app-owned store and pin the ref local before consulting the network.
      // Preserve the requested provider lane: dropping it binds the durable
      // thread to Codex while the composer can already be showing Claude.
      const local = await input.local.newThread(laneRef)
      if (local !== null) return remember(local, "local")
      // A missing/broken local bridge is the only reason to attempt the typed
      // runtime host. This is degradation, not the normal critical path.
      return remember(await runtime.newThread(laneRef), "runtime")
    },
    openThread: async threadRef => {
      const selected = await hostForThread(threadRef)
      const opened = await selected.host.openThread(threadRef)
      if (opened !== null) return remember(opened, selected.mode)
      if (threadModes.has(threadRef)) return null
      const fallback = selected.mode === "runtime"
        ? { host: input.local, mode: "local" as const }
        : { host: runtime, mode: "runtime" as const }
      return remember(await fallback.host.openThread(threadRef), fallback.mode)
    },
    hydrateThread: async threadRef => {
      const selected = await hostForThread(threadRef)
      const hydrated = await (selected.host.hydrateThread?.(threadRef) ?? selected.host.openThread(threadRef))
      return remember(hydrated, selected.mode)
    },
    sendMessage: async value => {
      const selected = await hostForThread(value.id)
      active = selected.host
      try {
        const result = await selected.host.sendMessage(value)
        if (result.thread !== undefined && result.thread !== null) {
          threadModes.set(result.thread.id, selected.mode)
        }
        return result
      } finally {
        active = null
      }
    },
    ...(input.local.selectLane === undefined ? {} : {
      selectLane: async (threadRef: string, laneRef: string) => {
        const selected = await hostForThread(threadRef)
        return selected.host.selectLane?.(threadRef, laneRef) ?? {
          ok: false,
          reason: "unknown_lane",
          message: "Provider lane selection is unavailable for this thread.",
        }
      },
    }),
    ...(input.local.laneForThread === undefined ? {} : {
      laneForThread: async (threadRef: string) => {
        const selected = await hostForThread(threadRef)
        return selected.host.laneForThread?.(threadRef) ?? null
      },
    }),
    interruptActive: async () => active?.interruptActive?.() ?? false,
    interruptActiveControlIdentity: async threadRef =>
      active?.interruptActiveControlIdentity?.(threadRef) ?? null,
    interruptActiveControl: async () => active?.interruptActiveControl?.() ?? null,
    steerChild: async value => active?.steerChild?.(value) ?? { ok: false, outcome: "not_found" },
    queueFollowup: async value => active?.queueFollowup?.(value) ?? { ok: false, queued: false },
    queueFollowupControl: async value => active?.queueFollowupControl?.(value) ?? makeComposerSubmitOutcome({
      control: value.control,
      observedAt: new Date().toISOString(),
      admission: { status: "rejected", reasonRef: "reason.adapter_unavailable" },
      delivery: { status: "unsupported", reasonRef: "reason.adapter_unavailable" },
    }),
    steerCurrent: async value => active?.steerCurrent?.(value) ?? { ok: false, outcome: "not_found" },
    steerCurrentControl: async value => active?.steerCurrentControl?.(value) ?? makeComposerSubmitOutcome({
      control: value.control,
      observedAt: new Date().toISOString(),
      admission: { status: "rejected", reasonRef: "reason.adapter_unavailable" },
      delivery: { status: "unsupported", reasonRef: "reason.adapter_unavailable" },
    }),
  }
}
