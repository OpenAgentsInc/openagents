import { randomUUID } from "node:crypto"
import {
  decodeFleetAccountEntity,
  decodeKhalaRuntimeEvent,
  type FleetAccountEntity,
  type KhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
} from "@openagentsinc/khala-sync"
import type {
  KhalaRuntimeSource,
  KhalaRuntimeToolAuthority,
} from "@openagentsinc/agent-runtime-schema"
import {
  CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY,
  CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE,
} from "../codex-agent-executor.js"
import { CODEX_AGENT_SDK_PACKAGE } from "../codex-agent.js"
import {
  hashPylonAccountRef,
  pylonAccountEnvironment,
  type PylonAccountRegistryEntry,
  type ResolvedPylonAccountSelection,
} from "../account-registry.js"
import {
  RUNTIME_RECORD_EVENT_MUTATOR_NAME,
  pushKhalaSyncMutation,
  runtimeSyncClientForTurn,
} from "./runtime-sync-push.js"
import {
  readPendingRuntimeIntents,
  fetchChatMessage as fetchChatMessageFromWorker,
  type ChatMessageBody,
  type FetchChatMessageResult,
  type ReadPendingRuntimeIntentsResult,
  type RuntimeControlIntentRow,
} from "./runtime-intents.js"
import type {
  PylonOrchestrationStore,
  RuntimeIntentOutcomeStatus,
} from "./store.js"

/**
 * Runtime control-intent dispatch consumer (#8388) — the Pylon-side
 * enforcement loop that turns durable `runtime.*` control intents
 * (`khala_sync_runtime_control_intents`, written by
 * `packages/khala-sync-server/src/runtime-mutators.ts`) into REAL local
 * Codex turns, with real streamed `runtime.recordEvent` progress. This is
 * the entire gap described in
 * docs/khala-code/2026-07-04-mobile-tailnet-handshake.md: before this file,
 * `runtime.startTurn` durably recorded an intent and nothing consumed it.
 *
 * Mirrors `./fleet-intent-enforcement.ts`'s watermarked, exactly-once,
 * failure-isolated shape — but a running `turn.start` dispatch can take
 * minutes (a real agent turn), so unlike the fleet enforcement tick (which
 * applies every intent synchronously and returns), this tick launches
 * `turn.start` dispatches IN THE BACKGROUND (fire-and-forget, tracked in
 * `activeTurns`) and returns quickly. That is what lets the SAME process's
 * next tick observe and act on a `turn.interrupt` for an already-running
 * turn instead of only ever seeing it after the turn finishes.
 *
 * EXACTLY-ONCE: consumption is watermarked
 * (`store.getRuntimeIntentWatermark`/`setRuntimeIntentWatermark`, keyed on
 * the migration-0032 `seq` column) AND deduped per intent id — every
 * intent this tick decides to act on records a
 * `RuntimeIntentOutcomeRecord` keyed by `intentId`; a redelivered intent
 * that already has an outcome is skipped without re-dispatching. NOTE the
 * outcome recorded for `turn.start` means "dispatch was launched", not
 * "the turn finished successfully" — actual turn success/failure is a
 * separate concern reported through the `runtime_event` stream
 * (`turn.finished`), exactly like the mutator's own `queued` -> `running`
 * -> `completed`/`failed` turn-status lifecycle.
 *
 * FAILURE ISOLATION: one bad/malformed intent NEVER wedges the loop.
 * Synchronous validation errors (unresolvable prompt, no dispatch-ready
 * account) are caught per intent and recorded as a `failed` outcome; the
 * watermark still advances. A background dispatch's OWN errors are caught
 * inside the dispatch itself and reported as a `turn.finished` event with
 * `finishReason: "error"` — they can never throw back into the tick.
 *
 * KNOWN GAPS (documented honestly, not silently papered over):
 * - Account selection (`selectDispatchAccountNaive`) is a naive
 *   placeholder for #8389 (capacity-aware selection); see its doc comment.
 * - `message.append` for an in-flight turn is EXPLICITLY REJECTED, not
 *   silently dropped: the Codex SDK's `runStreamed(prompt)` call has no
 *   mid-turn steering API, so there is no real way to inject the message
 *   into the already-running turn. The rejection detail says so.
 * - `turn.continue` / `turn.retry` / `turn.close` are recorded
 *   `skipped_stale` with an explicit "not implemented in this pass"
 *   detail — no pylon-local action is taken for them yet.
 * - Dispatch only targets `codex`-provider accounts; `claude_agent`
 *   accounts are visible to `selectDispatchAccountNaive` but this module
 *   has no Claude thread runner, so a naive selection landing on one would
 *   fail fast with an honest `provider_not_supported` finish reason.
 */

// ---------------------------------------------------------------------------
// Naive account selection (placeholder for #8389)
// ---------------------------------------------------------------------------

/**
 * Pick a dispatch-ready account: the first with `capacityAvailable > 0`, or
 * (when no account reports a positive capacity, including when capacity is
 * entirely unreported) the first with `readiness: "ready"`.
 *
 * // naive placeholder for #8389 (capacity-aware selection) — a parallel
 * // session is building the real algorithm; swap this call for it once it
 * // lands on main.
 */
export const selectDispatchAccountNaive = (
  accounts: ReadonlyArray<FleetAccountEntity>,
): FleetAccountEntity | undefined => {
  const withPositiveCapacity = accounts.find(
    (account) => account.capacityAvailable !== undefined && account.capacityAvailable > 0,
  )
  if (withPositiveCapacity !== undefined) return withPositiveCapacity
  return accounts.find((account) => account.readiness === "ready")
}

export type CandidateAccount = {
  readonly fleetAccount: FleetAccountEntity
  readonly registryEntry: PylonAccountRegistryEntry
}

/**
 * Projects this Pylon's OWN local Codex account registry
 * (`loadPylonAccountRegistry`) into the same `FleetAccountEntity` shape
 * `selectDispatchAccountNaive` consumes — every registered Codex account
 * reports `readiness: "ready"` and `capacityAvailable: 1` (a real,
 * one-slot-per-account placeholder; #8389 will source real live capacity).
 * Claude accounts are intentionally excluded: there is no Claude thread
 * runner wired into this dispatch consumer yet.
 */
export const candidateAccountsFromRegistry = (
  registry: ReadonlyArray<PylonAccountRegistryEntry>,
  now: Date = new Date(),
): ReadonlyArray<CandidateAccount> =>
  registry
    .filter((entry) => entry.provider === "codex")
    .map((entry) => ({
      fleetAccount: decodeFleetAccountEntity({
        accountRefHash: hashPylonAccountRef(entry.provider, entry.ref),
        capacityAvailable: 1,
        capacityBusy: 0,
        capacityQueued: 0,
        provider: entry.provider,
        readiness: "ready" as const,
        updatedAt: now.toISOString(),
      }),
      registryEntry: entry,
    }))

// ---------------------------------------------------------------------------
// Prompt resolution (chat_message.<messageId> bodyRef convention)
// ---------------------------------------------------------------------------

const BODY_REF_MESSAGE_ID_PATTERN = /^chat_message\.(.+)$/

/** Extracts `<messageId>` from a `chat_message.<messageId>` bodyRef, or `null`. */
export const chatMessageIdFromBodyRef = (bodyRef: string | undefined): string | null => {
  if (bodyRef === undefined) return null
  const match = BODY_REF_MESSAGE_ID_PATTERN.exec(bodyRef)
  return match?.[1] ?? null
}

// ---------------------------------------------------------------------------
// Codex raw event -> KhalaRuntimeEvent translation
// ---------------------------------------------------------------------------

export type CodexRawEvent = Record<string, unknown> & { type?: unknown }

const stableId = (prefix: string, seed: string): string =>
  `${prefix}.${Buffer.from(seed).toString("hex").slice(0, 24) || randomUUID().replace(/-/g, "").slice(0, 24)}`

const runtimeOwnerLocalToolAuthority = (toolName: string): KhalaRuntimeToolAuthority => ({
  allowed: true,
  authorityRef: stableId("authority.pylon.runtime_dispatch", toolName),
  blockerRefs: [],
  decisionRef: stableId("decision.pylon.runtime_dispatch", toolName),
  policyRef: "policy.pylon.runtime_dispatch.owner_local_full_access.v1",
  status: "allowed",
  toolRef: toolName,
})

const codexToolName = (itemType: string, item: Record<string, unknown>): string => {
  if (itemType === "command_execution") return "commandExecution"
  if (itemType === "file_change") return "fileChange"
  if (itemType === "web_search") return "webSearch"
  if (itemType === "mcp_tool_call") {
    const toolName = item.tool_name ?? item.name
    return typeof toolName === "string" && toolName.length > 0 ? toolName : "mcpTool"
  }
  return itemType
}

const codexItemFailed = (item: Record<string, unknown>): boolean => {
  if (typeof item.exit_code === "number" && item.exit_code !== 0) return true
  return item.status === "failed"
}

export type RuntimeEventTranslationContext = {
  readonly threadId: string
  readonly turnId: string
  readonly source: KhalaRuntimeSource
  /** Mutable per-turn cursor: `true` once `turn.started` has been emitted. */
  readonly turnStarted: { value: boolean }
  readonly allocateSequence: () => number
  readonly nowIso: () => string
}

/**
 * Translates ONE raw Codex thread event into zero or more
 * `KhalaRuntimeEvent`s. Pure given its context's `allocateSequence`/
 * `nowIso`/`turnStarted` seams — real production calls thread a live
 * sequence counter and clock through them; tests inject deterministic
 * fakes.
 */
export const codexRawEventToRuntimeEvents = (
  raw: CodexRawEvent,
  ctx: RuntimeEventTranslationContext,
): ReadonlyArray<KhalaRuntimeEvent> => {
  const type = typeof raw.type === "string" ? raw.type : undefined
  const base = (kind: string, extra: Record<string, unknown>): KhalaRuntimeEvent =>
    decodeKhalaRuntimeEvent({
      causalityRefs: [],
      eventId: randomUUID(),
      kind,
      observedAt: ctx.nowIso(),
      redactionClass: "private_ref",
      schema: "openagents.khala_runtime_event.v1",
      sequence: ctx.allocateSequence(),
      source: ctx.source,
      threadId: ctx.threadId,
      turnId: ctx.turnId,
      visibility: "private",
      ...extra,
    })

  if (type === "turn.started") {
    if (ctx.turnStarted.value) return []
    ctx.turnStarted.value = true
    return [base("turn.started", {})]
  }

  if (type === "item.completed") {
    const item = raw.item as Record<string, unknown> | undefined
    const itemType = typeof item?.type === "string" ? item.type : undefined
    if (item === undefined || itemType === undefined) return []

    if (itemType === "agent_message") {
      const text = typeof item.text === "string" ? item.text : ""
      const messageId = randomUUID()
      return [
        base("text.delta", { chunkId: randomUUID(), messageId, text }),
        base("text.completed", { messageId }),
      ]
    }
    if (itemType === "reasoning") {
      const text = typeof item.text === "string" ? item.text : ""
      const messageId = randomUUID()
      return [
        base("reasoning.delta", { chunkId: randomUUID(), messageId, text }),
        base("reasoning.completed", { messageId }),
      ]
    }
    if (
      itemType === "command_execution" ||
      itemType === "file_change" ||
      itemType === "mcp_tool_call" ||
      itemType === "web_search"
    ) {
      const toolCallId = randomUUID()
      const toolName = codexToolName(itemType, item)
      const authority = runtimeOwnerLocalToolAuthority(toolName)
      const callEvent = base("tool.call", { authority, toolCallId, toolName })
      if (codexItemFailed(item)) {
        return [
          callEvent,
          base("tool.error", {
            authority,
            errorRef: randomUUID(),
            messageSafe: `${toolName} failed`,
            toolCallId,
            toolName,
          }),
        ]
      }
      return [
        callEvent,
        base("tool.result", { authority, resultRef: randomUUID(), toolCallId, toolName }),
      ]
    }
    return []
  }

  if (type === "turn.completed") {
    const usageRaw = raw.usage as Record<string, unknown> | undefined
    const inputTokens = typeof usageRaw?.input_tokens === "number" ? usageRaw.input_tokens : 0
    const outputTokens = typeof usageRaw?.output_tokens === "number" ? usageRaw.output_tokens : 0
    const reasoningTokens =
      typeof usageRaw?.reasoning_output_tokens === "number" ? usageRaw.reasoning_output_tokens : 0
    return [
      base("usage.recorded", {
        usage: {
          inputTokens,
          outputTokens,
          reasoningTokens,
          totalTokens: inputTokens + outputTokens + reasoningTokens,
          usageRef: randomUUID(),
        },
      }),
      base("turn.finished", { finishReason: "stop" satisfies KhalaRuntimeFinishReason }),
    ]
  }

  if (type === "turn.failed" || type === "error") {
    return [base("turn.finished", { finishReason: "error" satisfies KhalaRuntimeFinishReason })]
  }

  return []
}

// ---------------------------------------------------------------------------
// Codex thread execution seam
// ---------------------------------------------------------------------------

export type RuntimeCodexThreadRunner = (input: {
  readonly instructions: string
  readonly cwd: string
  readonly env: Record<string, string | undefined>
  readonly networkAccessEnabled: boolean
  readonly signal: AbortSignal
  readonly model?: string
}) => Promise<{ readonly events: AsyncIterable<CodexRawEvent> }>

/**
 * The real runner: one Codex SDK thread against the given working
 * directory, owner-local full access (mirrors
 * `codex-agent-executor.ts`'s `runWithCodexSdk` sandbox/approval
 * invariant — the SDK equivalent of
 * `--dangerously-bypass-approvals-and-sandbox`), network enabled, aborted
 * via the given `AbortSignal`. NOT a refactor of `runWithCodexSdk`: that
 * function's event reporters are hardwired to the ATIF/fleet-assignment
 * turn-ingest pipeline (`assignmentRef`/`leaseRef`/`pylonRef`), not to
 * Khala Sync runtime events, and a plain chat turn has none of those. This
 * is a smaller, dedicated runner for this consumer, reusing the exact same
 * SDK invocation shape and sandbox/approval constants for parity with the
 * proven fleet path.
 */
export const runWithRealCodexSdk: RuntimeCodexThreadRunner = async (input) => {
  const sdk = (await import(CODEX_AGENT_SDK_PACKAGE)) as {
    Codex: new (options?: { env?: Record<string, string | undefined> }) => {
      startThread: (options: Record<string, unknown>) => {
        runStreamed: (
          prompt: string,
          turnOptions?: Record<string, unknown>,
        ) => Promise<{ events: AsyncIterable<unknown> }>
      }
    }
  }
  const codex = new sdk.Codex({ env: input.env })
  const thread = codex.startThread({
    approvalPolicy: CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY,
    networkAccessEnabled: input.networkAccessEnabled,
    sandboxMode: CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE,
    skipGitRepoCheck: true,
    workingDirectory: input.cwd,
    ...(input.model === undefined ? {} : { model: input.model }),
  })
  const result = await thread.runStreamed(input.instructions, { signal: input.signal })
  return result as { events: AsyncIterable<CodexRawEvent> }
}

// ---------------------------------------------------------------------------
// Enforcement loop
// ---------------------------------------------------------------------------

type ActiveRuntimeTurn = {
  readonly abortController: AbortController
  interrupted: boolean
  readonly clientGroupId: string
  readonly clientId: string
  readonly nextEventSequence: () => number
  readonly nextMutationId: () => number
}

export type ActiveRuntimeTurns = Map<string, ActiveRuntimeTurn>

export type PushRuntimeEventFn = (input: {
  readonly clientGroupId: string
  readonly clientId: string
  readonly mutationId: number
  readonly event: KhalaRuntimeEvent
}) => Promise<void>

export interface EnforceRuntimeIntentsOptions {
  /** e.g. `https://openagents.com` (`OPENAGENTS_BASE_URL`). */
  readonly baseUrl: string
  /** Admin bearer (`OPENAGENTS_ADMIN_API_TOKEN`); never echoed. */
  readonly adminToken: string
  /** Agent bearer used to push `runtime.recordEvent` (`OPENAGENTS_AGENT_TOKEN`). */
  readonly agentToken: string
  /** This Pylon's public ref, used to namespace the synthetic push clientGroupId. */
  readonly pylonRef: string
  /** Restrict the poll to one owner (recommended: this Pylon's linked user). */
  readonly ownerUserId?: string
  readonly limit?: number
  readonly now?: Date
  /** Live registry of turns this PROCESS is currently running. Persist the
   * same Map across ticks so `turn.interrupt` can reach an active turn. */
  readonly activeTurns: ActiveRuntimeTurns
  /** Working-directory root for per-thread Codex scratch spaces. */
  readonly workspaceRoot: string
  readonly listCandidateAccounts: () => Promise<ReadonlyArray<CandidateAccount>>
  readonly resolveAccountSelection: (
    entry: PylonAccountRegistryEntry,
  ) => Promise<ResolvedPylonAccountSelection | null>
  readonly ensureWorkspace: (threadId: string) => Promise<string>
  /** Test/override seam for the whole poller. Default `readPendingRuntimeIntents`. */
  readonly readImpl?: (options: {
    baseUrl: string
    adminToken: string
    after?: number
    ownerUserId?: string
    limit?: number
  }) => Promise<ReadPendingRuntimeIntentsResult>
  /** Test/override seam for chat-message resolution. */
  readonly fetchChatMessageImpl?: (options: {
    baseUrl: string
    adminToken: string
    threadId: string
    messageId: string
  }) => Promise<FetchChatMessageResult>
  /** Test/override seam for pushing runtime events. */
  readonly pushEventImpl?: PushRuntimeEventFn
  /** Test/override seam for the Codex thread runner. */
  readonly codexThreadRunner?: RuntimeCodexThreadRunner
  readonly log?: (line: string) => void
}

export type EnforcedRuntimeIntentOutcome = {
  readonly intentId: string
  readonly threadId: string
  readonly kind: string
  readonly outcome: RuntimeIntentOutcomeStatus
  readonly detail: string | null
  readonly deduped: boolean
}

export type EnforceRuntimeIntentsResult =
  | Readonly<{
      ok: true
      outcomes: ReadonlyArray<EnforcedRuntimeIntentOutcome>
      nextAfter: number
      upToDate: boolean
    }>
  | Readonly<{
      ok: false
      error: string
      status: number | null
      reason: string | null
      watermark: number
    }>

const boundedDetail = (value: string): string => value.slice(0, 300)

const defaultPushEvent =
  (baseUrl: string, agentToken: string): PushRuntimeEventFn =>
  async (input) => {
    const result = await pushKhalaSyncMutation({
      agentToken,
      args: input.event,
      baseUrl,
      clientGroupId: input.clientGroupId,
      clientId: input.clientId,
      mutationId: input.mutationId,
      name: RUNTIME_RECORD_EVENT_MUTATOR_NAME,
    })
    if (!result.ok || result.result.status === "rejected") {
      const detail = !result.ok ? result.reason : result.result.errorMessageSafe
      throw new Error(`runtime.recordEvent push failed: ${detail ?? "unknown"}`)
    }
  }

const makeCounter = (start = 0): (() => number) => {
  let value = start
  return () => {
    value += 1
    return value
  }
}

const pushFinishedEvent = async (input: {
  readonly pushEvent: PushRuntimeEventFn
  readonly turn: ActiveRuntimeTurn
  readonly threadId: string
  readonly turnId: string
  readonly source: KhalaRuntimeSource
  readonly finishReason: KhalaRuntimeFinishReason
}): Promise<void> => {
  const event = decodeKhalaRuntimeEvent({
    causalityRefs: [],
    eventId: randomUUID(),
    finishReason: input.finishReason,
    kind: "turn.finished",
    observedAt: new Date().toISOString(),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_event.v1",
    sequence: input.turn.nextEventSequence(),
    source: input.source,
    threadId: input.threadId,
    turnId: input.turnId,
    visibility: "private",
  })
  await input.pushEvent({
    clientGroupId: input.turn.clientGroupId,
    clientId: input.turn.clientId,
    event,
    mutationId: input.turn.nextMutationId(),
  })
}

/**
 * Runs one `turn.start` dispatch to completion. NEVER thrown out to the
 * caller — every failure path (unreachable message, no account, Codex SDK
 * error) is reported as a `turn.finished` event with `finishReason:
 * "error"` (or absorbed silently on a genuine `turn.interrupt` abort, since
 * the interrupt handler already recorded the terminal event for that
 * case). Callers launch this WITHOUT awaiting it (fire-and-forget) so the
 * enforcement tick stays fast.
 */
const dispatchTurnStart = async (input: {
  readonly options: EnforceRuntimeIntentsOptions
  readonly intent: RuntimeControlIntentRow
  readonly turnId: string
  readonly prompt: string
  readonly account: ResolvedPylonAccountSelection
  readonly turn: ActiveRuntimeTurn
  readonly source: KhalaRuntimeSource
}): Promise<void> => {
  const { options, intent, turnId, prompt, account, turn, source } = input
  const pushEvent = options.pushEventImpl ?? defaultPushEvent(options.baseUrl, options.agentToken)
  const runCodexThread = options.codexThreadRunner ?? runWithRealCodexSdk
  const turnStarted = { value: false }

  const pushOne = async (event: KhalaRuntimeEvent): Promise<void> => {
    await pushEvent({
      clientGroupId: turn.clientGroupId,
      clientId: turn.clientId,
      event,
      mutationId: turn.nextMutationId(),
    })
  }

  let finishedPushed = false
  try {
    const cwd = await options.ensureWorkspace(intent.threadId)
    const env = pylonAccountEnvironment(process.env as Record<string, string | undefined>, account)
    const { events } = await runCodexThread({
      cwd,
      env,
      instructions: prompt,
      networkAccessEnabled: true,
      signal: turn.abortController.signal,
    })
    for await (const raw of events) {
      const translated = codexRawEventToRuntimeEvents(raw, {
        allocateSequence: turn.nextEventSequence,
        nowIso: () => new Date().toISOString(),
        source,
        threadId: intent.threadId,
        turnId,
        turnStarted,
      })
      for (const event of translated) {
        if (event.kind === "turn.finished") finishedPushed = true
        await pushOne(event)
      }
    }
  } catch (error) {
    if (turn.interrupted || turn.abortController.signal.aborted) {
      // The `turn.interrupt` control-intent handler already recorded the
      // terminal `turn.interrupted` event for this turn — nothing more to
      // report here.
      return
    }
    options.log?.(
      `runtime-intent turn=${turnId} thread=${intent.threadId} dispatch error: ${boundedDetail(
        error instanceof Error ? error.message : "unknown",
      )}`,
    )
    if (!finishedPushed) {
      await pushFinishedEvent({ finishReason: "error", pushEvent, source, threadId: intent.threadId, turn, turnId })
    }
    return
  }
  if (!finishedPushed) {
    // Defensive: the stream ended without an explicit turn.completed/failed.
    await pushFinishedEvent({ finishReason: "unknown", pushEvent, source, threadId: intent.threadId, turn, turnId })
  }
}

const handleTurnStart = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  const source: KhalaRuntimeSource = { adapterKind: "codex", lane: "codex_app_server", surface: "server" }
  const turnId = row.intent.turnId
  if (turnId === undefined) {
    return { detail: "turn.start intent carried no turnId", outcome: "failed" }
  }

  const messageId = chatMessageIdFromBodyRef(
    row.intent.kind === "turn.start" ? row.intent.bodyRef : undefined,
  )
  if (messageId === null) {
    return {
      detail: "turn.start intent carried no resolvable chat_message.<messageId> bodyRef",
      outcome: "failed",
    }
  }

  const fetchChatMessageImpl = options.fetchChatMessageImpl ?? fetchChatMessageFromWorker
  const messageResult = await fetchChatMessageImpl({
    adminToken: options.adminToken,
    baseUrl: options.baseUrl,
    messageId,
    threadId: row.threadId,
  })
  if (!messageResult.ok) {
    return {
      detail: boundedDetail(`chat_message lookup transport failed: ${messageResult.error}`),
      outcome: "failed",
    }
  }
  const message: ChatMessageBody | null = messageResult.message
  if (message === null || message.deletedAt !== null) {
    return {
      detail: `referenced chat_message.${messageId} does not exist (or was deleted) in thread ${row.threadId}`,
      outcome: "failed",
    }
  }

  const candidates = await options.listCandidateAccounts()
  const selected = selectDispatchAccountNaive(candidates.map((c) => c.fleetAccount))
  if (selected === undefined) {
    return { detail: "no dispatch-ready local Codex account available", outcome: "failed" }
  }
  const candidate = candidates.find((c) => c.fleetAccount.accountRefHash === selected.accountRefHash)
  if (candidate === undefined) {
    return { detail: "invariant violated: selected account has no matching registry entry", outcome: "failed" }
  }
  const account = await options.resolveAccountSelection(candidate.registryEntry)
  if (account === null) {
    return { detail: `local Codex account home for ${candidate.registryEntry.ref} could not be resolved`, outcome: "failed" }
  }

  const clientIdentity = runtimeSyncClientForTurn({ pylonRef: options.pylonRef, turnId })
  const turn: ActiveRuntimeTurn = {
    abortController: new AbortController(),
    clientGroupId: clientIdentity.clientGroupId,
    clientId: clientIdentity.clientId,
    interrupted: false,
    nextEventSequence: makeCounter(0),
    nextMutationId: makeCounter(0),
  }
  options.activeTurns.set(turnId, turn)
  void dispatchTurnStart({
    account,
    intent: row,
    options,
    prompt: message.body,
    source,
    turn,
    turnId,
  }).finally(() => {
    if (options.activeTurns.get(turnId) === turn) options.activeTurns.delete(turnId)
  })

  return {
    detail: `dispatch started against account ${selected.accountRefHash}`,
    outcome: "applied",
  }
}

const handleTurnInterrupt = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  const turnId = row.intent.turnId
  if (turnId === undefined) {
    return { detail: "turn.interrupt intent carried no turnId", outcome: "failed" }
  }
  const turn = options.activeTurns.get(turnId)
  if (turn === undefined) {
    return {
      detail: "no locally running turn to interrupt (different process, already finished, or never started)",
      outcome: "skipped_stale",
    }
  }
  turn.interrupted = true
  turn.abortController.abort()
  const pushEvent = options.pushEventImpl ?? defaultPushEvent(options.baseUrl, options.agentToken)
  await pushFinishedInterruptedEvent({ pushEvent, row, turn })
  return { detail: `turn ${turnId} aborted locally`, outcome: "applied" }
}

const pushFinishedInterruptedEvent = async (input: {
  readonly pushEvent: PushRuntimeEventFn
  readonly row: RuntimeControlIntentRow
  readonly turn: ActiveRuntimeTurn
}): Promise<void> => {
  const turnId = input.row.intent.turnId
  if (turnId === undefined) return
  const event = decodeKhalaRuntimeEvent({
    causalityRefs: [],
    eventId: randomUUID(),
    kind: "turn.interrupted",
    observedAt: new Date().toISOString(),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_event.v1",
    sequence: input.turn.nextEventSequence(),
    source: { adapterKind: "codex", lane: "codex_app_server", surface: "server" },
    threadId: input.row.threadId,
    turnId,
    visibility: "private",
  })
  await input.pushEvent({
    clientGroupId: input.turn.clientGroupId,
    clientId: input.turn.clientId,
    event,
    mutationId: input.turn.nextMutationId(),
  })
}

const NOT_IMPLEMENTED_KINDS = new Set(["turn.continue", "turn.retry", "turn.close"])

/**
 * Apply ONE decoded control-intent row. Never throws — synchronous
 * validation failures come back as `failed`; a `turn.start` that passes
 * validation launches its dispatch in the background and returns
 * `applied` immediately (see the module doc for what that outcome means).
 */
const applyRuntimeIntent = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  switch (row.intent.kind) {
    case "turn.start":
      return handleTurnStart(options, row)
    case "turn.interrupt":
      return handleTurnInterrupt(options, row)
    case "message.append":
      // HONEST LIMITATION: the Codex SDK's runStreamed(prompt) call has no
      // mid-turn steering API. Rather than silently drop the message or
      // fake acceptance, this is an explicit, clearly-detailed rejection —
      // never a silent no-op.
      return {
        detail:
          "mid-turn steering is not supported by the local Codex SDK's single-prompt runStreamed call; " +
          "this message was NOT applied to any active turn and was not queued",
        outcome: "failed",
      }
    case "turn.continue":
    case "turn.retry":
    case "turn.close":
      return {
        detail: `runtime.${row.intent.kind} dispatch is not implemented in this pass (#8388 v1 handles turn.start/turn.interrupt/message.append only)`,
        outcome: "skipped_stale",
      }
    default:
      return NOT_IMPLEMENTED_KINDS.has(row.intent.kind)
        ? { detail: `unhandled kind ${row.intent.kind}`, outcome: "skipped_stale" }
        : { detail: `unrecognized control-intent kind ${row.intent.kind}`, outcome: "failed" }
  }
}

/**
 * One enforcement tick: poll the Worker's runtime-intents route from the
 * persisted watermark, dispatch every new intent, record per-intent
 * outcomes, and advance the watermark. Never throws.
 */
export const enforcePendingRuntimeIntents = async (
  store: PylonOrchestrationStore,
  options: EnforceRuntimeIntentsOptions,
): Promise<EnforceRuntimeIntentsResult> => {
  const log = options.log ?? ((line: string) => console.error(line))
  const read = options.readImpl ?? readPendingRuntimeIntents
  const watermark = store.getRuntimeIntentWatermark(options.ownerUserId)

  const page = await read({
    adminToken: options.adminToken,
    after: watermark,
    baseUrl: options.baseUrl,
    ...(options.limit === undefined ? {} : { limit: options.limit }),
    ...(options.ownerUserId === undefined ? {} : { ownerUserId: options.ownerUserId }),
  })
  if (!page.ok) {
    log(`runtime-intents poll failed error=${page.error} status=${page.status ?? "none"} watermark=${watermark}`)
    return { error: page.error, ok: false, reason: page.reason, status: page.status, watermark }
  }

  const outcomes: EnforcedRuntimeIntentOutcome[] = []
  for (const row of page.intents) {
    const existing = store.getRuntimeIntentOutcome(row.intentId)
    if (existing !== null) {
      outcomes.push({
        deduped: true,
        detail: existing.detail,
        intentId: row.intentId,
        kind: row.kind,
        outcome: existing.outcome,
        threadId: row.threadId,
      })
      continue
    }
    let application: { outcome: RuntimeIntentOutcomeStatus; detail: string }
    try {
      application = await applyRuntimeIntent(options, row)
    } catch (error) {
      application = {
        detail: boundedDetail(error instanceof Error ? error.message : "intent application threw"),
        outcome: "failed",
      }
    }
    let recorded = application
    try {
      const { outcome } = store.recordRuntimeIntentOutcome({
        detail: application.detail,
        intentId: row.intentId,
        kind: row.kind,
        outcome: application.outcome,
        threadId: row.threadId,
        turnId: row.turnId,
      })
      recorded = { detail: outcome.detail ?? "", outcome: outcome.outcome }
    } catch {
      // Outcome persistence failing must not wedge the loop either — the
      // watermark still advances (a lost outcome row is a bookkeeping gap,
      // never a correctness gap: turn dispatch is fire-and-forget and its
      // own errors are already self-contained).
    }
    outcomes.push({
      deduped: false,
      detail: recorded.detail,
      intentId: row.intentId,
      kind: row.kind,
      outcome: recorded.outcome,
      threadId: row.threadId,
    })
    log(
      `runtime-intent seq=${row.seq} intent=${row.intentId} kind=${row.kind} thread=${row.threadId} -> ${recorded.outcome}` +
        (recorded.detail.length === 0 ? "" : ` (${recorded.detail})`),
    )
  }

  if (page.nextAfter > watermark) {
    store.setRuntimeIntentWatermark(page.nextAfter, options.ownerUserId)
  }

  return { nextAfter: Math.max(page.nextAfter, watermark), ok: true, outcomes, upToDate: page.upToDate }
}
