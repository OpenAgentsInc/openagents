import type {
  DesktopRuntimeCapabilityId,
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayRequest,
  DesktopRuntimeGatewayResponse,
} from "./runtime-gateway-contract.ts"
import { DesktopRuntimeGatewayProtocolVersion } from "./runtime-gateway-contract.ts"
import type { CodexHistoryCatalog, CodexHistoryPage, CodexHistorySearchResponse } from "./codex-history-contract.ts"
import type {
  ConfirmedAgentRun,
  ConfirmedAgentTimelineEvent,
  ConfirmedRuntimeInteraction,
  RuntimeInteractionDecisionCommand,
  RuntimeCommandOutcome,
} from "@openagentsinc/khala-sync-client"
import type {
  DesktopCorrelationStage,
  DesktopOperationContext,
} from "./desktop-operation-context.ts"
import type { DesktopRuntimeLiveSubscriptions } from "./runtime-live-subscriptions.ts"

type CapabilityState = Readonly<{
  id: DesktopRuntimeCapabilityId
  state: "available" | "unavailable"
  reason?: string
}>

type ConversationStatus = Readonly<{
  phase: "idle" | "bootstrapping" | "catching_up" | "live" | "must_refetch" | "denied"
  cursor: number | null
  pendingMutationCount: number
}>

export type DesktopRuntimeConversation = Readonly<{
  catalog: () => Readonly<{
    status: ConversationStatus
    threads: ReadonlyArray<Readonly<{
      threadRef: string
      title: string
      messageCount: number
      lastMessageAt: string | null
      updatedAt: string
      version: number
    }>>
  }>
  thread: (threadRef: string) => Readonly<{
    status: ConversationStatus
    messages: ReadonlyArray<Readonly<{
      messageRef: string
      threadRef: string
      body: string
      createdAt: string
      updatedAt: string
      version: number
    }>>
  }>
  create: (threadRef: string, title: string) => number
  append: (threadRef: string, messageRef: string, body: string) => number
}>

export type DesktopRuntimeAgentTimeline = Readonly<{
  snapshot: (runRef: string) => Readonly<{
    status: ConversationStatus
    run: ConfirmedAgentRun | null
    events: ReadonlyArray<ConfirmedAgentTimelineEvent>
  }>
  snapshotForThread?: (threadRef: string) => Readonly<{
    status: ConversationStatus
    run: ConfirmedAgentRun | null
    events: ReadonlyArray<ConfirmedAgentTimelineEvent>
  }>
}>

export type DesktopRuntimeCommands = Readonly<{
  start: (input: Readonly<{ threadRef: string; messageRef: string; runRef: string; lane?: "codex_app_server" | "claude_pylon" }>, context?: DesktopOperationContext) => number
  interrupt: (input: Readonly<{ commandRef: string; threadRef: string; runRef: string }>, context?: DesktopOperationContext) => number
  outcome: (input: Readonly<{ intentId: string; threadRef: string }>) => RuntimeCommandOutcome | null
}>

export type DesktopRuntimeCodexHistory = Readonly<{
  catalog: () => CodexHistoryCatalog | Promise<CodexHistoryCatalog>
  page: (threadRef: string, offset: number, limit: number) => CodexHistoryPage | null | Promise<CodexHistoryPage | null>
  search: (query: string, limit: number) => CodexHistorySearchResponse | Promise<CodexHistorySearchResponse>
}>

export type DesktopRuntimeInteractions = Readonly<{
  list: (
    threadRef: string,
  ) => ReadonlyArray<ConfirmedRuntimeInteraction> | Promise<ReadonlyArray<ConfirmedRuntimeInteraction>>
  decide: (
    command: RuntimeInteractionDecisionCommand,
  ) => number | Promise<number>
}>

export type DesktopRuntimeGateway = Readonly<{
  start: () => void
  request: (request: DesktopRuntimeGatewayRequest, context?: DesktopOperationContext) => DesktopRuntimeGatewayResponse | Promise<DesktopRuntimeGatewayResponse>
  subscribe: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  dispose: () => void
}>

export const desktopRuntimeCapabilities = (input: Readonly<{
  sessionLocalState: "signed_out" | "credential_present_unverified" | "session_ready" | "denied" | "unavailable"
  syncLocalState: "ready" | "unavailable"
  syncNetworkPhase: "idle" | "bootstrapping" | "catching_up" | "live" | "must_refetch" | "denied" | "closed"
}>): ReadonlyArray<CapabilityState> => [
  {
    id: "agent-graph",
    state: input.syncNetworkPhase === "live" ? "available" : "unavailable",
    reason: input.syncNetworkPhase === "live"
      ? undefined
      : "Confirmed live-agent graph Sync is not live.",
  },
  {
    id: "agent-timeline",
    state: input.syncNetworkPhase === "live" ? "available" : "unavailable",
    reason: input.syncNetworkPhase === "live"
      ? undefined
      : "Confirmed agent timeline Sync is not live.",
  },
  { id: "codex-history", state: "available" },
  { id: "workspace", state: "available" },
  { id: "git-review", state: "available" },
  { id: "provider-accounts", state: "available" },
  {id:"local-identity",state:input.syncLocalState==="ready"?"available":"unavailable",...(input.syncLocalState==="ready"?{}:{reason:"Device-local identity persistence is unavailable."})},
  {
    id: "openagents-session",
    state: input.sessionLocalState === "session_ready" ? "available" : "unavailable",
    reason: input.sessionLocalState === "session_ready"
      ? undefined
      : input.sessionLocalState === "signed_out"
      ? "OpenAgents sign-in is required."
      : input.sessionLocalState === "credential_present_unverified"
        ? "Stored OpenAgents credentials require server verification."
        : input.sessionLocalState === "denied"
          ? "OpenAgents session access was denied."
          : "OS-encrypted OpenAgents session custody is unavailable.",
  },
  {
    id: "conversation-sync",
    state: input.syncNetworkPhase === "live" ? "available" : "unavailable",
    reason: input.syncNetworkPhase === "live"
      ? undefined
      : "Confirmed conversation Sync is not live.",
  },
  {
    id: "khala-sync",
    state: input.syncNetworkPhase === "live" ? "available" : "unavailable",
    reason: input.syncNetworkPhase === "live"
      ? undefined
      : input.syncLocalState === "unavailable"
        ? "Local Sync persistence is unavailable."
        : input.syncNetworkPhase === "idle"
          ? "Authenticated Sync is idle."
          : input.syncNetworkPhase === "closed"
            ? "Authenticated Sync is closed."
            : `Authenticated Sync is ${input.syncNetworkPhase}.`,
  },
  {
    id: "conversation-stream",
    state: input.syncNetworkPhase === "live" ? "available" : "unavailable",
    reason: input.syncNetworkPhase === "live"
      ? undefined
      : "The durable conversation runtime is not connected yet.",
  },
]

export const createDesktopRuntimeGateway = (
  capabilities: ReadonlyArray<CapabilityState> | (() => ReadonlyArray<CapabilityState>) =
    () => desktopRuntimeCapabilities({ sessionLocalState: "unavailable", syncLocalState: "unavailable", syncNetworkPhase: "closed" }),
  sessionActions?: Readonly<{
    signIn: (signal?: AbortSignal) => Promise<Readonly<{ state: "verified" | "cancelled" | "unavailable" }>>
    signOut: (signal?: AbortSignal) => Promise<Readonly<{ state: "signed_out" | "unavailable" }>>
  }>,
  sessionPhase: () => "signed_out" | "unverified" | "session_ready" | "denied" | "unavailable" = () => "unavailable",
  conversation: () => DesktopRuntimeConversation | null = () => null,
  timeline: () => DesktopRuntimeAgentTimeline | null = () => null,
  codexHistory: () => DesktopRuntimeCodexHistory | null = () => null,
  identityTier:()=>"local_only"|"account_linked"|"local_unavailable"=()=>"local_unavailable",
  runtimeCommands: () => DesktopRuntimeCommands | null = () => null,
  observeOperation: (stage: DesktopCorrelationStage, context: DesktopOperationContext) => void = () => undefined,
  liveSubscriptions: () => DesktopRuntimeLiveSubscriptions | null = () => null,
  runtimeInteractions: () => DesktopRuntimeInteractions | null = () => null,
): DesktopRuntimeGateway => {
  let phase: "idle" | "ready" | "disposed" = "idle"
  let sequence = 0
  let sessionActionInFlight = false
  let sessionActionAbort: AbortController | null = null
  const listeners = new Set<(event: DesktopRuntimeGatewayEvent) => void>()

  const publish = (event: DesktopRuntimeGatewayEvent): void => {
    for (const listener of [...listeners]) listener(event)
  }

  const emit = (next: "ready" | "disposed"): void => {
    const event: DesktopRuntimeGatewayEvent = {
      kind: "runtime.lifecycle",
      phase: next,
      protocolVersion: DesktopRuntimeGatewayProtocolVersion,
      sequence: ++sequence,
    }
    publish(event)
  }

  return {
    start: () => {
      if (phase !== "idle") return
      phase = "ready"
      emit("ready")
    },
    request: (request, context) => {
      if (context !== undefined) observeOperation("gateway.received", context)
      const outcome = (() : DesktopRuntimeGatewayResponse | Promise<DesktopRuntimeGatewayResponse> => {
      if (phase === "disposed") return { kind: "request_rejected", reason: "gateway_disposed" }
      if (request.kind === "query") {
        if (request.query.id === "conversation.commandOutcome") {
          const service = runtimeCommands()
          if (service === null) {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "not_live" }
          }
          try {
            const result = service.outcome(request.query)
            return result === null
              ? { kind: "conversation_unavailable", requestId: request.requestId, reason: "not_found" }
              : { kind: "runtime_command_status", requestId: request.requestId, ...result }
          } catch {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "read_failed" }
          }
        }
        if (request.query.id === "codex.history.catalog") {
          const service = codexHistory()
          if (service === null) return { kind: "codex_history_unavailable", requestId: request.requestId, reason: "read_failed" }
          return Promise.resolve(service.catalog()).then(catalog => ({ kind: "codex_history_catalog" as const, requestId: request.requestId, catalog })).catch(() => ({ kind: "codex_history_unavailable" as const, requestId: request.requestId, reason: "read_failed" as const }))
        }
        if (request.query.id === "codex.history.page") {
          const service = codexHistory()
          if (service === null) return { kind: "codex_history_unavailable", requestId: request.requestId, reason: "read_failed" }
          return Promise.resolve(service.page(request.query.threadRef, request.query.offset, request.query.limit)).then(page => page === null ? ({ kind: "codex_history_unavailable" as const, requestId: request.requestId, reason: "not_found" as const }) : ({ kind: "codex_history_page" as const, requestId: request.requestId, page })).catch(() => ({ kind: "codex_history_unavailable" as const, requestId: request.requestId, reason: "read_failed" as const }))
        }
        if (request.query.id === "codex.history.search") {
          const service = codexHistory()
          if (service === null) return { kind: "codex_history_unavailable", requestId: request.requestId, reason: "read_failed" }
          return Promise.resolve(service.search(request.query.query, request.query.limit)).then(search => ({ kind: "codex_history_search" as const, requestId: request.requestId, search })).catch(() => ({ kind: "codex_history_unavailable" as const, requestId: request.requestId, reason: "read_failed" as const }))
        }
        if (request.query.id === "conversation.catalog") {
          const service = conversation()
          if (service === null) {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "not_live" }
          }
          try {
            const result = service.catalog()
            return {
              kind: "conversation_catalog",
              requestId: request.requestId,
              ...result,
            }
          } catch {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "read_failed" }
          }
        }
        if (request.query.id === "conversation.thread") {
          const service = conversation()
          if (service === null) {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "not_live" }
          }
          try {
            const result = service.thread(request.query.threadRef)
            return {
              kind: "conversation_thread",
              requestId: request.requestId,
              threadRef: request.query.threadRef,
              ...result,
            }
          } catch {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "read_failed" }
          }
        }
        if (request.query.id === "agent.timeline") {
          const runRef = request.query.runRef
          const service = timeline()
          if (service === null) {
            return { kind: "agent_timeline_unavailable", requestId: request.requestId, reason: "not_live" }
          }
          try {
            const result = service.snapshot(runRef)
            if (result.status.phase !== "live") {
              return { kind: "agent_timeline_unavailable", requestId: request.requestId, reason: "not_live" }
            }
            if (result.run === null) {
              return { kind: "agent_timeline_unavailable", requestId: request.requestId, reason: "not_found" }
            }
            if (
              result.run.runRef !== runRef ||
              result.events.length > 500 ||
              result.events.some(event =>
                event.runRef !== runRef || event.artifactRefs.length > 100)
            ) {
              return { kind: "agent_timeline_unavailable", requestId: request.requestId, reason: "read_failed" }
            }
            const confirmedRun = result.run
            return {
              kind: "agent_timeline",
              requestId: request.requestId,
              runRef,
              status: result.status,
              run: confirmedRun,
              events: result.events,
            }
          } catch {
            return { kind: "agent_timeline_unavailable", requestId: request.requestId, reason: "read_failed" }
          }
        }
        if (request.query.id === "conversation.timeline") {
          const service = timeline()
          if (service === null || service.snapshotForThread === undefined) {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "not_live" }
          }
          try {
            const result = service.snapshotForThread(request.query.threadRef)
            return {
              kind: "conversation_timeline",
              requestId: request.requestId,
              threadRef: request.query.threadRef,
              ...result,
            }
          } catch {
            return { kind: "conversation_unavailable", requestId: request.requestId, reason: "read_failed" }
          }
        }
        if (request.query.id === "runtime.interactions") {
          const query = request.query
          const service = runtimeInteractions()
          if (service === null) {
            return {
              kind: "runtime_interactions_unavailable",
              requestId: request.requestId,
              reason: "not_live",
            }
          }
          return Promise.resolve(service.list(query.threadRef))
            .then(interactions => interactions.length > 100 ||
                interactions.some(interaction => interaction.threadId !== query.threadRef)
              ? {
                  kind: "runtime_interactions_unavailable" as const,
                  requestId: request.requestId,
                  reason: "read_failed" as const,
                }
              : {
                  kind: "runtime_interactions" as const,
                  requestId: request.requestId,
                  threadRef: query.threadRef,
                  interactions,
                })
            .catch(() => ({
              kind: "runtime_interactions_unavailable" as const,
              requestId: request.requestId,
              reason: "read_failed" as const,
            }))
        }
        return {
          kind: "query_result",
          requestId: request.requestId,
          result: {
            capabilities: typeof capabilities === "function" ? capabilities() : capabilities,
            kind: "runtime.bootstrap",
            lifecycle: phase === "idle" ? "starting" : phase,
            protocolVersion: DesktopRuntimeGatewayProtocolVersion,
            sessionPhase: sessionPhase(),
            identityTier:identityTier(),
          },
        }
      }
      if (
        request.command.id === "conversation.subscribe" ||
        request.command.id === "conversation.unsubscribe"
      ) {
        const command = request.command
        const service = liveSubscriptions()
        if (service === null) {
          return Promise.resolve({
            kind: "conversation_subscription_outcome" as const,
            commandId: request.commandId,
            subscriptionRef: command.subscriptionRef,
            generation: command.generation,
            status: "unavailable" as const,
          })
        }
        if (command.id === "conversation.subscribe") {
          return service.subscribe(command, update => publish(update)).then(result => ({
            kind: "conversation_subscription_outcome" as const,
            commandId: request.commandId,
            subscriptionRef: command.subscriptionRef,
            generation: command.generation,
            status: result.status,
            ...(result.status === "stale_generation"
              ? { activeGeneration: result.activeGeneration }
              : {}),
          }))
        }
        return service.unsubscribe(
          command.subscriptionRef,
          command.generation,
        ).then(unsubscribed => ({
          kind: "conversation_subscription_outcome" as const,
          commandId: request.commandId,
          subscriptionRef: command.subscriptionRef,
          generation: command.generation,
          status: unsubscribed ? "unsubscribed" as const : "not_found" as const,
        }))
      }
      if (
        request.command.id === "conversation.create" ||
        request.command.id === "conversation.append"
      ) {
        const service = conversation()
        if (service === null) {
          return {
            kind: "conversation_mutation_outcome",
            commandId: request.commandId,
            status: "unavailable",
          }
        }
        try {
          const mutationId = request.command.id === "conversation.create"
            ? service.create(request.command.threadRef, request.command.title)
            : service.append(
                request.command.threadRef,
                request.command.messageRef,
                request.command.body,
              )
          return {
            kind: "conversation_mutation_outcome",
            commandId: request.commandId,
            status: "pending_reconcile",
            mutationId,
          }
        } catch {
          return {
            kind: "conversation_mutation_outcome",
            commandId: request.commandId,
            status: "unavailable",
          }
        }
      }
      if (request.command.id === "runtime.decideInteraction") {
        const command = request.command
        const service = runtimeInteractions()
        if (service === null) {
          return {
            kind: "runtime_interaction_decision_outcome",
            commandId: request.commandId,
            interactionRef: command.interactionRef,
            threadRef: command.threadRef,
            turnRef: command.turnRef,
            status: "unavailable",
          }
        }
        return Promise.resolve(service.decide({
          interactionRef: command.interactionRef,
          threadId: command.threadRef,
          turnId: command.turnRef,
          envelope: command.envelope,
        }))
          .then(mutationId => ({
            kind: "runtime_interaction_decision_outcome" as const,
            commandId: request.commandId,
            interactionRef: command.interactionRef,
            threadRef: command.threadRef,
            turnRef: command.turnRef,
            status: "pending_reconcile" as const,
            mutationId,
          }))
          .catch(() => ({
            kind: "runtime_interaction_decision_outcome" as const,
            commandId: request.commandId,
            interactionRef: command.interactionRef,
            threadRef: command.threadRef,
            turnRef: command.turnRef,
            status: "unavailable" as const,
          }))
      }
      if (
        request.command.id === "conversation.start" ||
        request.command.id === "conversation.interrupt"
      ) {
        const service = runtimeCommands()
        if (service === null) {
          return {
            kind: "runtime_command_outcome",
            commandId: request.commandId,
            threadRef: request.command.threadRef,
            runRef: request.command.runRef,
            ...(request.command.id === "conversation.start"
              ? { messageRef: request.command.messageRef }
              : {}),
            status: "unavailable",
            reason: "Authenticated runtime Sync is unavailable.",
          }
        }
        try {
          const mutationId = request.command.id === "conversation.start"
            ? service.start(request.command, context)
            : service.interrupt(request.command, context)
          return {
            kind: "runtime_command_outcome",
            commandId: request.commandId,
            threadRef: request.command.threadRef,
            runRef: request.command.runRef,
            ...(request.command.id === "conversation.start"
              ? { messageRef: request.command.messageRef }
              : {}),
            status: "unknown_pending_reconcile",
            mutationId,
          }
        } catch {
          return {
            kind: "runtime_command_outcome",
            commandId: request.commandId,
            threadRef: request.command.threadRef,
            runRef: request.command.runRef,
            ...(request.command.id === "conversation.start"
              ? { messageRef: request.command.messageRef }
              : {}),
            status: "rejected",
            reason: "Runtime command was rejected before admission.",
          }
        }
      }
      if (request.command.id === "session.sign_in") {
        if (sessionActions === undefined || sessionActionInFlight) {
          return Promise.resolve({
            kind: "session_outcome",
            commandId: request.commandId,
            status: "unavailable",
            phase: "unavailable",
          })
        }
        sessionActionInFlight = true
        const abort = new AbortController()
        sessionActionAbort = abort
        return sessionActions.signIn(abort.signal)
          .then(result => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: result.state === "verified" ? "completed" as const : result.state,
            phase: result.state === "verified" ? "session_ready" as const : result.state === "cancelled" ? "signed_out" as const : "unavailable" as const,
          }))
          .catch(() => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: "unavailable" as const,
            phase: "unavailable" as const,
          }))
          .finally(() => {
            if (sessionActionAbort === abort) sessionActionAbort = null
            sessionActionInFlight = false
          })
      }
      if (request.command.id === "session.sign_out") {
        if (sessionActions === undefined || sessionActionInFlight) {
          return Promise.resolve({
            kind: "session_outcome",
            commandId: request.commandId,
            status: "unavailable",
            phase: "unavailable",
          })
        }
        sessionActionInFlight = true
        const abort = new AbortController()
        sessionActionAbort = abort
        return sessionActions.signOut(abort.signal)
          .then(result => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: result.state === "signed_out" ? "completed" as const : "unavailable" as const,
            phase: result.state,
          }))
          .catch(() => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: "unavailable" as const,
            phase: "unavailable" as const,
          }))
          .finally(() => {
            if (sessionActionAbort === abort) sessionActionAbort = null
            sessionActionInFlight = false
          })
      }
      return {
        kind: "command_outcome",
        commandId: request.commandId,
        status: "unavailable",
        reason: "Conversation interrupt is unavailable until the durable runtime is connected.",
      }
      })()
      const attach = (response: DesktopRuntimeGatewayResponse): DesktopRuntimeGatewayResponse =>
        context === undefined ? response : { ...response, context }
      return outcome instanceof Promise ? outcome.then(attach) : attach(outcome)
    },
    subscribe: listener => {
      if (phase === "disposed") {
        listener({
          kind: "runtime.lifecycle",
          phase: "disposed",
          protocolVersion: DesktopRuntimeGatewayProtocolVersion,
          sequence,
        })
        return () => undefined
      }
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    dispose: () => {
      if (phase === "disposed") return
      phase = "disposed"
      sessionActionAbort?.abort()
      sessionActionAbort = null
      emit("disposed")
      listeners.clear()
      const service = liveSubscriptions()
      if (service !== null) void service.dispose()
    },
  }
}
