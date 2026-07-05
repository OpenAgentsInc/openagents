import { randomUUID } from "node:crypto"
import {
  decodeFleetAccountEntity,
  decodeKhalaRuntimeEvent,
  selectDispatchAccount,
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
  RUNTIME_START_TURN_MUTATOR_NAME,
  pushKhalaSyncMutation,
  runtimeSyncClientForTurn,
  type PushKhalaSyncMutationResult,
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
 * - Account selection now uses the real capacity/load-aware
 *   `selectDispatchAccount` (#8389, `@openagentsinc/khala-sync`), scoped to
 *   `provider: "codex"` and round-robin-tie-broken per thread — see
 *   `handleTurnStart`. The remaining honest limitation is one layer below
 *   the algorithm: `candidateAccountsFromRegistry` still projects a
 *   placeholder `capacityAvailable: 1` for every ready registry account (a
 *   real live-capacity signal is not wired yet), so with today's inputs the
 *   real ranking mostly reduces to readiness + the round-robin tie-break —
 *   the selection algorithm itself is no longer naive, only its inputs are.
 * - `message.append` for an in-flight turn is still honestly NOT literal
 *   mid-turn steering: the Codex SDK's `runStreamed(prompt)` call has no API
 *   to inject into an already-running turn's stream (verified against
 *   `@openai/codex-sdk`'s type surface — `Thread` only exposes
 *   `run`/`runStreamed`, no `send`/`interject`). Instead of a flat
 *   rejection, an append targeting a turn actively dispatching on THIS
 *   Pylon is durably queued and becomes a real follow-up `runtime.startTurn`
 *   once that turn settles (`dispatchQueuedFollowUps`), resuming the same
 *   Codex thread where possible (`Codex#resumeThread`) so context is not
 *   lost. See `handleMessageAppend`'s doc for the exact outcome in every
 *   case (attached / not-currently-running / no turn to attach to).
 * - `turn.continue` / `turn.retry` are still recorded `skipped_stale` with
 *   an explicit "not implemented in this pass" detail — no pylon-local
 *   action is taken for them (tracked as tracked follow-up work; the
 *   server-side turn status transition still happens at mutation time
 *   regardless). `turn.close` IS implemented (`handleTurnClose`): the
 *   server-side mutator already makes "closed" authoritative at
 *   mutation-apply time (mirrors `turn.interrupt`), so Pylon's job is only
 *   local bookkeeping — there is none beyond the `activeTurns` cleanup the
 *   dispatch loop already does, since the Codex workspace is per-THREAD
 *   (reused across turns), not per-turn.
 * - Dispatch only targets `codex`-provider accounts; `claude_agent`
 *   accounts are excluded entirely by `candidateAccountsFromRegistry` before
 *   `selectDispatchAccount` ever sees them (this module has no Claude thread
 *   runner yet).
 * - Cross-turn Codex context continuity (`resumeThreadId`) is best-effort:
 *   if the account that resumes a thread differs from the one that created
 *   it (isolated per-account homes), the resume fails cleanly into a normal
 *   `turn.finished(error)` — never a crash — but the user does lose context
 *   for that turn. This mostly matters once an owner has 2+ ready Codex
 *   accounts feeding the same round-robin tie-break.
 */

export type CandidateAccount = {
  readonly fleetAccount: FleetAccountEntity
  readonly registryEntry: PylonAccountRegistryEntry
}

/**
 * Projects this Pylon's OWN local Codex account registry
 * (`loadPylonAccountRegistry`) into the same `FleetAccountEntity` shape
 * `selectDispatchAccount` (#8389, `@openagentsinc/khala-sync`) consumes —
 * every registered Codex account reports `readiness: "ready"` and
 * `capacityAvailable: 1` (a real, one-slot-per-account placeholder; a real
 * live-capacity signal is not wired yet). Claude accounts are intentionally
 * excluded: there is no Claude thread runner wired into this dispatch
 * consumer yet.
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
  /**
   * When set, resume this existing Codex SDK thread (`Codex#resumeThread`)
   * instead of starting a fresh, contextless one — the cross-turn
   * continuity mechanism `handleTurnStart` wires from
   * `store.getRuntimeCodexThreadId`. Only meaningful when the account
   * resuming is the SAME one that created the thread (isolated per-account
   * `~/.codex`-equivalent homes); a mismatch fails cleanly, not silently.
   */
  readonly resumeThreadId?: string
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
 *
 * When `input.resumeThreadId` is set, resumes that Codex thread
 * (`Codex#resumeThread`) instead of starting a fresh one, so a follow-up
 * turn in the same Khala Sync chat thread keeps the model's prior context.
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
      resumeThread: (
        id: string,
        options: Record<string, unknown>,
      ) => {
        runStreamed: (
          prompt: string,
          turnOptions?: Record<string, unknown>,
        ) => Promise<{ events: AsyncIterable<unknown> }>
      }
    }
  }
  const codex = new sdk.Codex({ env: input.env })
  const threadOptions = {
    approvalPolicy: CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY,
    networkAccessEnabled: input.networkAccessEnabled,
    sandboxMode: CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE,
    skipGitRepoCheck: true,
    workingDirectory: input.cwd,
    ...(input.model === undefined ? {} : { model: input.model }),
  }
  const thread = input.resumeThreadId === undefined
    ? codex.startThread(threadOptions)
    : codex.resumeThread(input.resumeThreadId, threadOptions)
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
  /** The Khala Sync thread this turn belongs to (needed to correlate a
   * `message.append` intent's `turnId` back to a thread, and to seed any
   * queued follow-up turn in the same thread). */
  readonly threadId: string
  /** `chat_message.<id>` message ids appended via `message.append` while
   * this turn was actively dispatching. Drained into real follow-up
   * `runtime.startTurn` dispatches once this turn settles — see
   * `dispatchQueuedFollowUps`. */
  pendingAppendMessageIds: string[]
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
  /**
   * Per-thread last-dispatched `accountRefHash`, used ONLY to round-robin
   * a full capacity/load tie in `selectDispatchAccount` — never dispatch
   * correctness. Persist the same Map across ticks (like `activeTurns`) for
   * real fairness across turns; in-memory only (does not survive a
   * supervisor restart), which is fine since it only affects tie-breaking.
   */
  readonly lastDispatchedAccountByThread?: Map<string, string>
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
  /**
   * Test/override seam for pushing a Pylon-authored follow-up
   * `runtime.startTurn` control-intent mutation (queued `message.append`
   * follow-up dispatch — see `dispatchQueuedFollowUps`). Default
   * `pushKhalaSyncMutation`.
   */
  readonly pushControlIntentImpl?: typeof pushKhalaSyncMutation
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
  readonly store: PylonOrchestrationStore
  readonly intent: RuntimeControlIntentRow
  readonly turnId: string
  readonly prompt: string
  readonly account: ResolvedPylonAccountSelection
  readonly turn: ActiveRuntimeTurn
  readonly source: KhalaRuntimeSource
  readonly resumeThreadId?: string
}): Promise<void> => {
  const { options, store, intent, turnId, prompt, account, turn, source, resumeThreadId } = input
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

  /** Best-effort: capture the SDK's own thread id (from its `thread.started`
   * event) so a LATER turn in this same Khala thread can resume it. Never
   * throws — a failure to persist this is a lost continuity opportunity,
   * not a dispatch failure. */
  const captureCodexThreadId = (raw: CodexRawEvent): void => {
    if (raw.type !== "thread.started") return
    const codexThreadId = raw.thread_id
    if (typeof codexThreadId !== "string" || codexThreadId.length === 0) return
    try {
      store.setRuntimeCodexThreadId(intent.threadId, codexThreadId)
    } catch (error) {
      options.log?.(
        `runtime-intent turn=${turnId} thread=${intent.threadId} failed to persist codex thread id: ${boundedDetail(
          error instanceof Error ? error.message : "unknown",
        )}`,
      )
    }
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
      ...(resumeThreadId === undefined ? {} : { resumeThreadId }),
    })
    for await (const raw of events) {
      captureCodexThreadId(raw)
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
  store: PylonOrchestrationStore,
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
  const lastUsedAccountRefHash = options.lastDispatchedAccountByThread?.get(row.threadId)
  const selected = selectDispatchAccount(candidates.map((c) => c.fleetAccount), {
    provider: "codex",
    ...(lastUsedAccountRefHash === undefined ? {} : { lastUsedAccountRefHash }),
  })
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
  options.lastDispatchedAccountByThread?.set(row.threadId, selected.accountRefHash)

  const clientIdentity = runtimeSyncClientForTurn({ pylonRef: options.pylonRef, turnId })
  const turn: ActiveRuntimeTurn = {
    abortController: new AbortController(),
    clientGroupId: clientIdentity.clientGroupId,
    clientId: clientIdentity.clientId,
    interrupted: false,
    nextEventSequence: makeCounter(0),
    nextMutationId: makeCounter(0),
    pendingAppendMessageIds: [],
    threadId: row.threadId,
  }
  options.activeTurns.set(turnId, turn)
  const resumeThreadId = store.getRuntimeCodexThreadId(row.threadId)
  void dispatchTurnStart({
    account,
    intent: row,
    options,
    prompt: message.body,
    ...(resumeThreadId === null ? {} : { resumeThreadId }),
    source,
    store,
    turn,
    turnId,
  }).finally(() => {
    if (options.activeTurns.get(turnId) === turn) options.activeTurns.delete(turnId)
    if (turn.pendingAppendMessageIds.length > 0) {
      void dispatchQueuedFollowUps({
        messageIds: turn.pendingAppendMessageIds,
        options,
        threadId: row.threadId,
      })
    }
  })

  return {
    detail: `dispatch started against account ${selected.accountRefHash}`,
    outcome: "applied",
  }
}

/**
 * Drains queued `message.append` follow-ups once the turn they arrived
 * during has settled (see `ActiveRuntimeTurn.pendingAppendMessageIds` and
 * `handleMessageAppend`). Each queued `chat_message.<id>` becomes its OWN
 * genuine, client-visible `runtime.startTurn` mutation — the SAME mutator
 * (`RUNTIME_START_TURN_MUTATOR_NAME`) the mobile/desktop composer calls, so
 * the follow-up turn shows up in the thread like any other, and this
 * Pylon's OWN next enforcement tick dispatches it exactly like any other
 * `turn.start` (picking up `resumeThreadId` continuity automatically via
 * `store.getRuntimeCodexThreadId`). Never thrown to the caller: failures
 * here just mean the follow-up turn does not get created and are logged,
 * mirroring `dispatchTurnStart`'s own fire-and-forget error handling — the
 * ORIGINAL turn's outcome is never affected by a queued follow-up's fate.
 */
const dispatchQueuedFollowUps = async (input: {
  readonly options: EnforceRuntimeIntentsOptions
  readonly threadId: string
  readonly messageIds: ReadonlyArray<string>
}): Promise<void> => {
  const { options, threadId, messageIds } = input
  const pushMutation = options.pushControlIntentImpl ?? pushKhalaSyncMutation
  for (const messageId of messageIds) {
    const followUpTurnId = randomUUID()
    const clientIdentity = runtimeSyncClientForTurn({ pylonRef: options.pylonRef, turnId: followUpTurnId })
    const args = {
      bodyRef: `chat_message.${messageId}`,
      causalityRefs: [],
      createdAt: new Date().toISOString(),
      idempotencyKey: `idem.pylon_followup.${followUpTurnId}`,
      intentId: `intent.pylon_followup.${followUpTurnId}`,
      kind: "turn.start" as const,
      origin: { lane: "codex_app_server" as const, surface: "server" as const },
      redactionClass: "private_ref" as const,
      schema: "openagents.khala_runtime_control_intent.v1" as const,
      target: { lane: "codex_app_server" as const },
      threadId,
      turnId: followUpTurnId,
      visibility: "private" as const,
    }
    let result: PushKhalaSyncMutationResult
    try {
      result = await pushMutation({
        agentToken: options.agentToken,
        args,
        baseUrl: options.baseUrl,
        clientGroupId: clientIdentity.clientGroupId,
        clientId: clientIdentity.clientId,
        mutationId: 1,
        name: RUNTIME_START_TURN_MUTATOR_NAME,
      })
    } catch (error) {
      options.log?.(
        `runtime-intent follow-up turn.start push threw thread=${threadId} messageId=${messageId}: ${boundedDetail(
          error instanceof Error ? error.message : "unknown",
        )}`,
      )
      continue
    }
    if (!result.ok || result.result.status === "rejected") {
      const detail = !result.ok ? result.reason : result.result.errorMessageSafe
      options.log?.(
        `runtime-intent follow-up turn.start push failed thread=${threadId} messageId=${messageId}: ${detail ?? "unknown"}`,
      )
    }
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

const NOT_IMPLEMENTED_KINDS = new Set(["turn.continue", "turn.retry"])

/**
 * Handle a `message.append` control intent. NOT literal mid-turn steering
 * (see the module doc for why that is not possible with the Codex SDK) —
 * three honest outcomes depending on what this Pylon can observe locally:
 *
 * 1. The intent's `turnId` matches a turn actively dispatching on THIS
 *    process (`options.activeTurns`): the message is queued on it
 *    (`pendingAppendMessageIds`) and becomes a real follow-up
 *    `runtime.startTurn` once that turn settles (`dispatchQueuedFollowUps`,
 *    triggered from `handleTurnStart`'s dispatch `.finally()`) — `applied`.
 * 2. A `turnId` was given but does not match any locally active turn
 *    (different process, already settled, or never started here): the
 *    message was NOT attached to anything, but it is durably visible in the
 *    thread already (the mutator recorded both the `chat_message` and this
 *    control intent before Pylon ever saw it) — `skipped_stale`, mirroring
 *    `handleTurnInterrupt`'s precedent for the same "nothing local to act
 *    on" shape.
 * 3. No `turnId` was given at all (a bare append, not steering): there was
 *    never anything for Pylon to attach to by design — `applied`, since
 *    this intent was fully and correctly processed by doing nothing more.
 */
const handleMessageAppend = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  const messageId = chatMessageIdFromBodyRef(
    row.intent.kind === "message.append" ? row.intent.bodyRef : undefined,
  )
  if (messageId === null) {
    return {
      detail: "message.append intent carried no resolvable chat_message.<messageId> bodyRef",
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
  if (messageResult.message === null || messageResult.message.deletedAt !== null) {
    return {
      detail: `referenced chat_message.${messageId} does not exist (or was deleted) in thread ${row.threadId}`,
      outcome: "failed",
    }
  }

  const turnId = row.intent.turnId
  const activeTurn = turnId === undefined ? undefined : options.activeTurns.get(turnId)

  if (activeTurn !== undefined) {
    activeTurn.pendingAppendMessageIds.push(messageId)
    return {
      detail:
        `mid-turn steering is not supported by the local Codex SDK's single-prompt runStreamed call; ` +
        `chat_message.${messageId} was queued instead of applied — it will be dispatched as a real ` +
        `follow-up runtime.startTurn (resuming the same Codex conversation where possible) once turn ${turnId} settles`,
      outcome: "applied",
    }
  }

  if (turnId !== undefined) {
    return {
      detail:
        `turn ${turnId} is not currently dispatching on this Pylon (different process, already settled, ` +
        `or never started here) — chat_message.${messageId} was NOT attached to it, but remains durably ` +
        `visible in the thread; start a new turn to have it answered`,
      outcome: "skipped_stale",
    }
  }

  return {
    detail:
      `chat_message.${messageId} is durably recorded in the thread with no turn to attach to — ` +
      `it will be picked up by the next runtime.startTurn for this thread`,
    outcome: "applied",
  }
}

/**
 * Handle a `turn.close` control intent. The server-side
 * `runtime.closeTurn` mutator already made `closed` the authoritative turn
 * status at mutation-apply time (mirrors how `turn.interrupt`'s mutator
 * already sets `interrupted` before Pylon ever polls for it) — so Pylon's
 * only job is LOCAL bookkeeping. There is none beyond `activeTurns`
 * cleanup (already handled by `dispatchTurnStart`'s `.finally()`): the
 * Codex working directory is per-THREAD, reused across turns on purpose,
 * not per-turn, so there is no turn-scoped local resource to release.
 *
 * If the turn is STILL actively dispatching locally, this intentionally
 * does NOT abort it — that is `turn.interrupt`'s job, not `turn.close`'s;
 * closing an in-flight turn out from under its own dispatch would be a
 * silent behavior change beyond what was asked for this pass.
 */
const handleTurnClose = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  const turnId = row.intent.turnId
  if (turnId === undefined) {
    return { detail: "turn.close intent carried no turnId", outcome: "failed" }
  }
  if (options.activeTurns.has(turnId)) {
    return {
      detail:
        `turn ${turnId} is still actively dispatching locally; turn.close only cleans up an already-` +
        `settled turn — interrupt it first (runtime.interruptTurn) to stop it early`,
      outcome: "skipped_stale",
    }
  }
  return {
    detail: `turn ${turnId} closed — no local dispatch was active for it, nothing further to clean up`,
    outcome: "applied",
  }
}

/**
 * Apply ONE decoded control-intent row. Never throws — synchronous
 * validation failures come back as `failed`; a `turn.start` that passes
 * validation launches its dispatch in the background and returns
 * `applied` immediately (see the module doc for what that outcome means).
 */
const applyRuntimeIntent = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
  store: PylonOrchestrationStore,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  switch (row.intent.kind) {
    case "turn.start":
      return handleTurnStart(options, row, store)
    case "turn.interrupt":
      return handleTurnInterrupt(options, row)
    case "message.append":
      return handleMessageAppend(options, row)
    case "turn.close":
      return handleTurnClose(options, row)
    case "turn.continue":
    case "turn.retry":
      return {
        detail: `runtime.${row.intent.kind} dispatch is not implemented in this pass (turn.start/turn.interrupt/message.append/turn.close are)`,
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
      application = await applyRuntimeIntent(options, row, store)
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
