import { createHash, randomUUID } from "node:crypto"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import type { CanUseTool } from "@anthropic-ai/claude-agent-sdk"
import {
  decodeFleetAccountEntity,
  decodeKhalaRuntimeEvent,
  selectDispatchAccount,
  type FleetAccountEntity,
  type KhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
} from "@openagentsinc/khala-sync"
import type {
  KhalaRuntimeLane,
  KhalaRuntimeSource,
  KhalaRuntimeToolAuthority,
} from "@openagentsinc/agent-runtime-schema"
import {
  createClaudeCanUseToolInteractionController,
  type PylonRuntimeInteractionAuthority,
} from "@openagentsinc/pylon-core/executor/runtime-interaction-bridge"
import {
  CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY,
  CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE,
} from "../codex-agent-executor.js"
import { CODEX_AGENT_SDK_PACKAGE } from "../codex-agent.js"
import {
  isCodexAppServerPreFrameFailure,
  makeCodexAppServerRunner,
} from "./codex-app-server-source.js"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../claude-agent.js"
import {
  admitClaudePermission,
  type ClaudeOwnerLocalPermissionControl,
  type ClaudePermissionMode,
} from "../claude-agent-executor.js"
import {
  hashPylonAccountRef,
  pylonAccountEnvironment,
  type PylonAccountRegistryEntry,
  type ResolvedPylonAccountSelection,
} from "../account-registry.js"
import { readinessForTarget, type AccountDiscoveryTarget } from "../account-usage.js"
import type { BootstrapSummary } from "../bootstrap.js"
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
  fetchRuntimeTurn as fetchRuntimeTurnFromWorker,
  type ChatMessageBody,
  type FetchChatMessageResult,
  type FetchRuntimeTurnResult,
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
 *   `provider: "codex"` or `provider: "claude_agent"` depending on the
 *   intent's `target.lane` and round-robin-tie-broken per thread — see
 *   `handleTurnStart`. `candidateAccountsFromRegistry` (#8410 follow-up) now
 *   computes REAL per-account readiness via `readinessForTarget`
 *   (`../account-usage.js`, the same check `pylon accounts list` uses) when
 *   given a bootstrap `summary` — a registered account with revoked/missing
 *   credentials, or in a rate-limit/usage-quota cooldown, is excluded from
 *   dispatch instead of being treated as unconditionally `ready`.
 *   `capacityAvailable` remains a real, one-slot-per-account placeholder (a
 *   real live-capacity SIGNAL, as opposed to readiness, is not wired yet), so
 *   the ranking among READY accounts still mostly reduces to the
 *   round-robin tie-break.
 * - Cross-turn account continuity (`resumeThreadId`/`resumeSessionId`, see
 *   below) is reinforced by an explicit per-thread account PIN
 *   (`store.getRuntimeDispatchAccountRefHash`/
 *   `setRuntimeDispatchAccountRefHash`, #8410 follow-up): once a Khala
 *   thread's first `turn.start` dispatch picks an account, `handleTurnStart`
 *   keeps dispatching that SAME thread to that SAME account (bypassing the
 *   round-robin tie-break entirely for that thread) as long as it stays in
 *   the real dispatch-ready set above. Only when the pinned account goes
 *   unhealthy does the thread fall back to ordinary round-robin selection
 *   (and re-pin to whatever gets picked) — a deliberate trade-off of
 *   round-robin fairness FOR ONE THREAD in exchange for reliable Codex/Claude
 *   session-resume continuity, which is what actually matters once an owner
 *   has 2+ ready accounts for the same provider.
 * - `message.append` for an in-flight turn is still honestly NOT literal
 *   mid-turn steering, for EITHER provider today. For Codex: the SDK's
 *   `runStreamed(prompt)` call has no API to inject into an already-running
 *   turn's stream (verified against `@openai/codex-sdk`'s type surface —
 *   `Thread` only exposes `run`/`runStreamed`, no `send`/`interject`). For
 *   Claude (#8404): the Claude Agent SDK's `query()` function DOES support a
 *   real mid-turn injection path when invoked in "streaming input mode"
 *   (`prompt: AsyncIterable<SDKUserMessage>` instead of a plain string) —
 *   its returned `Query` exposes `streamInput()`/`interrupt()`/
 *   `setPermissionMode()` for a live session (verified against
 *   `@anthropic-ai/claude-agent-sdk`'s `sdk.d.ts` type surface). This is a
 *   REAL capability Codex does not have. `runWithRealClaudeAgentSdk` in this
 *   module does NOT use streaming input mode yet — it calls `query()` with a
 *   single string prompt, matching the proven invocation shape already used
 *   for real production Claude execution elsewhere in this codebase
 *   (`../claude-composer.ts`, `../claude-agent-executor.ts`), to keep this
 *   pass's risk bounded. So for BOTH providers today, an append targeting a
 *   turn actively dispatching on THIS Pylon is durably queued and becomes a
 *   real follow-up `runtime.startTurn` once that turn settles
 *   (`dispatchQueuedFollowUps`), resuming the same provider session where
 *   possible (`Codex#resumeThread` / Claude `options.resume`) so context is
 *   not lost. Wiring genuine live Claude steering via streaming input mode
 *   is a concrete, scoped follow-up, not implemented here. See
 *   `handleMessageAppend`'s doc for the exact outcome in every case
 *   (attached / not-currently-running / no turn to attach to).
 * - `turn.continue` / `turn.retry` are now REAL local redispatch (#8410
 *   follow-up, `handleTurnContinueOrRetry`) of the SAME turnId the mutator
 *   already re-queued (`executeExistingTurnIntent` flips that turn's status
 *   back to "queued" for its OWN id — this is not a fresh turn.start with a
 *   new id). Redispatch reuses `handleTurnStart`'s exact account
 *   selection/pin and `dispatchTurnStart` shell, resuming the same
 *   Codex/Claude session (`resumeThreadId`/`resumeSessionId`) so prior
 *   context is not lost. Two things are specific to resuming an
 *   ALREADY-SETTLED turn rather than starting a brand new one: (1) the
 *   prompt — a caller-supplied `bodyRef` wins (resolved exactly like
 *   `turn.start`); absent one (the common case — continue/retry are not new
 *   user messages), a short built-in continuation instruction is sent
 *   instead of the ORIGINAL triggering message, since this consumer does not
 *   look that up (a bounded, honestly-documented limitation, not a silent
 *   shortcut — the resumed session already has full prior context either
 *   way); (2) the event-sequence cursor — the turn may already have events
 *   recorded from its earlier attempt, so redispatch resumes numbering from
 *   the turn's current `event_count` (fetched via the new
 *   `GET .../runtime-turn` internal route / `fetchRuntimeTurn`) rather than
 *   restarting at 0, which would collide with an existing `(turn_id,
 *   sequence)` pair and be rejected as a duplicate. If the turnId is STILL
 *   actively dispatching on this exact process, there is nothing to
 *   continue/retry yet — `skipped_stale`, mirroring `turn.close`'s
 *   precedent for the same shape. `turn.close` IS implemented
 *   (`handleTurnClose`): the server-side mutator already makes "closed"
 *   authoritative at mutation-apply time (mirrors `turn.interrupt`), so
 *   Pylon's job is only local bookkeeping — there is none beyond the
 *   `activeTurns` cleanup the dispatch loop already does, since the
 *   Codex/Claude workspace is per-THREAD (reused across turns), not
 *   per-turn.
 * - Dispatch targets `codex`-provider accounts for `target.lane ===
 *   "codex_app_server"` and `claude_agent`-provider accounts for
 *   `target.lane === "claude_pylon"` (#8404). Any OTHER `target.lane` (e.g.
 *   `ai_sdk_core`) is recorded `failed` with an explicit "not wired" detail
 *   rather than silently falling back to one provider.
 * - Claude Agent SDK `task_started` / `task_progress` / `task_notification`
 *   records for real subagent tasks are normalized into body-free
 *   `agent.child.*` events and retained by the canonical live graph. Codex
 *   child production (CUT-11 #8691) converges on the typed app-server
 *   source: `subAgentActivity` and receiver-bearing `collabAgentToolCall`
 *   thread items are normalized into the SAME body-free `agent.child.*`
 *   events through this one conversation-service translation
 *   (`codexRawEventToRuntimeEvents`), fed by the app-server-first runner in
 *   `codex-app-server-source.ts` (pre-frame fallback to the exec SDK). The
 *   exec encoder still drops `subAgentActivity` and emits receiver-less
 *   `collab_tool_call`, which yields NOTHING here — no tool-name or
 *   transcript-text heuristic ever fabricates parentage.
 * - Cross-turn context continuity (`resumeThreadId` for Codex,
 *   `resumeSessionId` for Claude) is reinforced, but still not GUARANTEED,
 *   for both providers by the per-thread account pin described above: if the
 *   account that resumes a thread/session differs from the one that created
 *   it (isolated per-account homes) — which the pin only allows once the
 *   previously-pinned account has gone unhealthy — the resume fails cleanly
 *   into a normal `turn.finished(error)` — never a crash — but the user does
 *   lose context for that turn.
 */

export type CandidateAccount = {
  readonly fleetAccount: FleetAccountEntity
  readonly registryEntry: PylonAccountRegistryEntry
}

export type CandidateAccountsFromRegistryOptions = {
  readonly now?: Date
  /**
   * When given, computes REAL per-account dispatch readiness (#8410
   * follow-up) via `readinessForTarget` (`../account-usage.js`) — the same
   * check `pylon accounts list`/`pylon codex accounts list`/
   * `pylon accounts status` already use, which honors the codex-account
   * health ledger (`credentials_revoked`/`usage_limited`/`rate_limited`,
   * recorded by real dispatch failures elsewhere in Pylon, e.g.
   * `codex-agent-executor.ts`'s fleet-assignment path) and the quota ledger.
   * Omitted, every registered account is projected `readiness: "ready"` (the
   * historical naive behavior) — kept as the default ONLY so pure unit tests
   * of this function don't need real filesystem/SDK I/O; production wiring
   * (`runtime-intent-supervisor.ts`) always provides this.
   */
  readonly summary?: Pick<BootstrapSummary, "paths">
  readonly env?: Record<string, string | undefined>
}

const READINESS_COOLDOWN_STATES = new Set(["usage_limited", "rate_limited"])

/**
 * Maps a Codex/Claude readiness state (`CodexAgentReadinessState` /
 * `ClaudeAgentReadinessState`, both plain string unions) onto the bounded
 * `FleetAccountReadiness` `selectDispatchAccount` understands
 * (`"ready" | "cooldown" | "unavailable" | "unknown"`). `usage_limited` and
 * `rate_limited` are temporary/self-clearing (a cooldown window or quota
 * reset), everything else non-`"ready"` (missing credentials, revoked
 * credentials, SDK missing, network/timeout, disabled, unsupported platform)
 * is `"unavailable"` — none of those clear on their own without owner action.
 */
const fleetAccountReadinessFromState = (state: string): FleetAccountEntity["readiness"] =>
  state === "ready" ? "ready" : READINESS_COOLDOWN_STATES.has(state) ? "cooldown" : "unavailable"

const accountDiscoveryTargetForRegistryEntry = (
  entry: PylonAccountRegistryEntry,
  accountRefHash: string,
): AccountDiscoveryTarget => ({
  account: {
    accountRef: entry.ref,
    accountRefHash,
    home: entry.home,
    openAgentsProviderAccountRef: entry.openAgentsProviderAccountRef,
    provider: entry.provider,
    selector: "registry_ref",
  },
  accountRef: entry.ref,
  accountRefHash,
  home: entry.home,
  // Never surfaced by `readinessForTarget` (only used by the accounts-list/
  // status CLI projections this consumer does not build) — the hash is a
  // safe, stable placeholder.
  homeRef: accountRefHash,
  provider: entry.provider,
  selector: "registry_ref",
})

/**
 * Projects this Pylon's OWN local account registry (`loadPylonAccountRegistry`)
 * into the same `FleetAccountEntity` shape `selectDispatchAccount` (#8389,
 * `@openagentsinc/khala-sync`) consumes. Both `codex` and `claude_agent`
 * accounts are included (#8404): `handleTurnStart` is what actually restricts
 * eligibility to the provider matching the intent's `target.lane`, via
 * `selectDispatchAccount`'s `options.provider` filter — this function stays a
 * provider-agnostic projection so a single registry load serves both lanes.
 *
 * `capacityAvailable` stays a real, one-slot-per-account placeholder (a real
 * live-capacity signal is not wired yet) — `1` for a ready account, `0`
 * otherwise (an unready account claiming leftover capacity would be
 * misleading, even though `selectDispatchAccount` already excludes
 * non-`"ready"` accounts regardless of capacity).
 */
export const candidateAccountsFromRegistry = async (
  registry: ReadonlyArray<PylonAccountRegistryEntry>,
  options: CandidateAccountsFromRegistryOptions = {},
): Promise<ReadonlyArray<CandidateAccount>> => {
  const now = options.now ?? new Date()
  const entries = registry.filter((entry) => entry.provider === "codex" || entry.provider === "claude_agent")
  const out: CandidateAccount[] = []
  for (const entry of entries) {
    const accountRefHash = hashPylonAccountRef(entry.provider, entry.ref)
    const readiness =
      options.summary === undefined
        ? ("ready" as const)
        : fleetAccountReadinessFromState(
            (
              await readinessForTarget(
                options.summary,
                accountDiscoveryTargetForRegistryEntry(entry, accountRefHash),
                options.env ?? (Bun.env as Record<string, string | undefined>),
              )
            ).readiness.state,
          )
    out.push({
      fleetAccount: decodeFleetAccountEntity({
        accountRefHash,
        capacityAvailable: readiness === "ready" ? 1 : 0,
        capacityBusy: 0,
        capacityQueued: 0,
        provider: entry.provider,
        readiness,
        updatedAt: now.toISOString(),
      }),
      registryEntry: entry,
    })
  }
  return out
}

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

// Deterministic per-seed id. MUST be collision-resistant across distinct
// seeds: the previous `hex(seed).slice(0, 24)` form kept only the first 12
// seed characters, so every Desktop-origin turn (`turn.desktop.<random>`)
// produced the SAME `event.runtime_claim.*` id and every run after the first
// was rejected as "already recorded" and never dispatched (found live during
// the #8676/#8689 physical receipt journey, 2026-07-12). A truncated SHA-256
// keeps the id deterministic for exact-retry idempotency while making
// distinct seeds produce distinct ids.
const stableId = (prefix: string, seed: string): string =>
  `${prefix}.${createHash("sha256").update(seed).digest("hex").slice(0, 24)}`

const runtimeOwnerLocalToolAuthority = (toolName: string): KhalaRuntimeToolAuthority => ({
  allowed: true,
  authorityRef: stableId("authority.pylon.runtime_dispatch", toolName),
  blockerRefs: [],
  decisionRef: stableId("decision.pylon.runtime_dispatch", toolName),
  policyRef: "policy.pylon.runtime_dispatch.owner_local_full_access.v1",
  status: "allowed",
  toolRef: toolName,
})

/**
 * Canonical (exec-encoder snake_case) spelling for a Codex thread-item type.
 * The typed app-server source (CUT-11 #8691) delivers the SAME items with
 * camelCase `type` discriminators (`agentMessage`, `commandExecution`, …);
 * one conversation-service translation handles both spellings so the
 * app-server transport does not need a parallel item pipeline.
 */
const codexItemTypeAliases: Record<string, string> = {
  agentMessage: "agent_message",
  commandExecution: "command_execution",
  fileChange: "file_change",
  mcpToolCall: "mcp_tool_call",
  webSearch: "web_search",
  subAgentActivity: "sub_agent_activity",
  collabAgentToolCall: "collab_agent_tool_call",
}

const canonicalCodexItemType = (itemType: string): string =>
  codexItemTypeAliases[itemType] ?? itemType

const codexToolName = (itemType: string, item: Record<string, unknown>): string => {
  if (itemType === "command_execution") return "commandExecution"
  if (itemType === "file_change") return "fileChange"
  if (itemType === "web_search") return "webSearch"
  if (itemType === "mcp_tool_call") {
    // exec encoder: `tool_name`; app-server v2 item: `tool` (+ `server`).
    const toolName = item.tool_name ?? item.tool ?? item.name
    return typeof toolName === "string" && toolName.length > 0 ? toolName : "mcpTool"
  }
  return itemType
}

const codexItemFailed = (item: Record<string, unknown>): boolean => {
  if (typeof item.exit_code === "number" && item.exit_code !== 0) return true
  if (typeof item.exitCode === "number" && item.exitCode !== 0) return true
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
 * Stable correlation facts for one REAL Codex child/sub-agent thread
 * (CUT-11 #8691). Keyed by the provider's own child thread id — the ONLY
 * honest Codex parentage source (typed `subAgentActivity` /
 * `collabAgentToolCall` records from the app-server); tool names, history,
 * and transcript text are forbidden parentage sources.
 */
type CodexChildThreadFacts = Readonly<{
  childAgentId: string
  childRunId: string
  parentAgentId: string
  taskRef: string
}>

export type CodexRuntimeEventTranslationContext = RuntimeEventTranslationContext & {
  /**
   * Mutable per-turn map from a Codex child THREAD id to its stable child
   * facts (mirrors Claude's `childTasks`). Populated only from typed child
   * records carrying a real child thread id; the exec encoder's
   * receiver-less `collab_tool_call` records can never populate it.
   */
  readonly childThreads: Map<string, CodexChildThreadFacts>
}

const codexChildThreadFacts = (
  agentThreadId: string,
  parentAgentId: string,
): CodexChildThreadFacts => ({
  childAgentId: stableId("child.codex", agentThreadId),
  childRunId: stableId("run.child.codex", agentThreadId),
  parentAgentId,
  taskRef: stableId("task.codex", agentThreadId),
})

/** Terminal finish reason for a typed collab agent state, if terminal. */
const codexCollabTerminalFinishReason = (
  status: unknown,
): KhalaRuntimeFinishReason | null => {
  if (status === "completed" || status === "shutdown") return "stop"
  if (status === "errored") return "error"
  if (status === "interrupted") return "interrupted"
  return null
}

/**
 * Translates ONE raw Codex thread event into zero or more
 * `KhalaRuntimeEvent`s. Pure given its context's `allocateSequence`/
 * `nowIso`/`turnStarted` seams — real production calls thread a live
 * sequence counter and clock through them; tests inject deterministic
 * fakes.
 *
 * Accepts BOTH Codex event sources through one conversation-service seam
 * (CUT-11 #8691): the exec-encoder stream (snake_case item types) and the
 * typed app-server stream (camelCase item types). Child/sub-agent facts are
 * emitted ONLY from typed records naming a real child thread id
 * (`subAgentActivity`, `collabAgentToolCall` receivers) — the exec encoder
 * currently drops those records, which is exactly why the app-server source
 * exists; nothing here infers parentage from tool names or text.
 */
export const codexRawEventToRuntimeEvents = (
  raw: CodexRawEvent,
  ctx: CodexRuntimeEventTranslationContext,
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
    const rawItemType = typeof item?.type === "string" ? item.type : undefined
    if (item === undefined || rawItemType === undefined) return []
    const itemType = canonicalCodexItemType(rawItemType)

    if (itemType === "sub_agent_activity") {
      // Typed app-server child lifecycle record: { kind, agentThreadId }.
      const agentThreadId = typeof item.agentThreadId === "string" && item.agentThreadId.length > 0
        ? item.agentThreadId
        : typeof item.agent_thread_id === "string" && item.agent_thread_id.length > 0
          ? item.agent_thread_id
          : null
      if (agentThreadId === null) return []
      const kind = item.kind
      if (kind === "started") {
        if (ctx.childThreads.has(agentThreadId)) return []
        const facts = codexChildThreadFacts(agentThreadId, ctx.turnId)
        ctx.childThreads.set(agentThreadId, facts)
        return [base("agent.child.started", facts)]
      }
      if (kind === "interacted") {
        const events: KhalaRuntimeEvent[] = []
        let facts = ctx.childThreads.get(agentThreadId)
        if (facts === undefined) {
          // Loss-tolerant: the start record was not observed, but the typed
          // record still names the real child thread (mirrors Claude's
          // task_progress auto-start) — never fabricated from tool names.
          facts = codexChildThreadFacts(agentThreadId, ctx.turnId)
          ctx.childThreads.set(agentThreadId, facts)
          events.push(base("agent.child.started", facts))
        }
        events.push(base("agent.child.progress", facts))
        return events
      }
      if (kind === "interrupted") {
        const facts = ctx.childThreads.get(agentThreadId)
        if (facts === undefined) return []
        ctx.childThreads.delete(agentThreadId)
        return [base("agent.child.finished", {
          ...facts,
          finishReason: "interrupted" satisfies KhalaRuntimeFinishReason,
        })]
      }
      return []
    }

    if (itemType === "collab_agent_tool_call") {
      // Typed collab lifecycle: receivers are real child thread ids; the
      // per-agent states carry lifecycle truth. A record WITHOUT receiver
      // ids (the current exec encoder's shape) yields nothing — no receiver
      // id, no honest child identity.
      const receiversRaw = item.receiverThreadIds ?? item.receiver_thread_ids
      const receivers = Array.isArray(receiversRaw)
        ? receiversRaw.filter((entry): entry is string => typeof entry === "string" && entry.length > 0)
        : []
      if (receivers.length === 0) return []
      const senderThreadId = typeof item.senderThreadId === "string"
        ? item.senderThreadId
        : typeof item.sender_thread_id === "string"
          ? item.sender_thread_id
          : null
      // Nested topology: a receiver spawned by a TRACKED child parents to
      // that child; otherwise the root turn is the parent.
      const parentAgentId = senderThreadId === null
        ? ctx.turnId
        : ctx.childThreads.get(senderThreadId)?.childAgentId ?? ctx.turnId
      const states = (item.agentsStates ?? item.agents_states) as Record<string, unknown> | undefined
      const events: KhalaRuntimeEvent[] = []
      for (const receiverThreadId of receivers) {
        let facts = ctx.childThreads.get(receiverThreadId)
        if (facts === undefined) {
          facts = codexChildThreadFacts(receiverThreadId, parentAgentId)
          ctx.childThreads.set(receiverThreadId, facts)
          events.push(base("agent.child.started", facts))
        }
        const state = states?.[receiverThreadId] as Record<string, unknown> | undefined
        const finishReason = codexCollabTerminalFinishReason(state?.status)
        if (finishReason !== null) {
          ctx.childThreads.delete(receiverThreadId)
          events.push(base("agent.child.finished", { ...facts, finishReason }))
        } else {
          events.push(base("agent.child.progress", facts))
        }
      }
      return events
    }

    if (itemType === "agent_message") {
      const text = typeof item.text === "string" ? item.text : ""
      const messageId = randomUUID()
      return [
        base("text.delta", { chunkId: randomUUID(), messageId, text }),
        base("text.completed", { messageId }),
      ]
    }
    if (itemType === "reasoning") {
      // exec encoder: `text`; app-server v2 item: `summary` string sections.
      const text = typeof item.text === "string"
        ? item.text
        : Array.isArray(item.summary)
          ? item.summary.filter((entry): entry is string => typeof entry === "string").join("\n")
          : ""
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
// Claude Agent SDK raw message -> KhalaRuntimeEvent translation (#8404)
// ---------------------------------------------------------------------------

/**
 * One raw message from the Claude Agent SDK's `query()` async generator
 * (`@anthropic-ai/claude-agent-sdk`'s `SDKMessage` union — see `sdk.d.ts`).
 * Kept as a loosely-typed record (mirroring `CodexRawEvent`) rather than
 * importing the SDK's own types, since the SDK is a lazy optional dependency
 * everywhere else in this codebase (`claude-agent.ts`, `claude-composer.ts`,
 * `claude-agent-executor.ts`) and this module must not force it into the
 * always-loaded dependency graph.
 */
export type ClaudeRawMessage = Record<string, unknown> & { type?: unknown }

export type ClaudeRuntimeEventTranslationContext = RuntimeEventTranslationContext & {
  /**
   * Mutable per-turn map from a `tool_use` content block's id to its tool
   * name. The Claude Agent SDK delivers a tool call inside an `assistant`
   * message and its result LATER inside a separate `user` message (the CLI
   * executes the tool out-of-band and injects a synthetic tool-result turn),
   * so the result has to be correlated back to the call by `tool_use_id`
   * rather than arriving paired like Codex's single `item.completed`.
   */
  readonly pendingToolCalls: Map<string, string>
  /** Stable task-id correlation for real Claude subagent lifecycle messages. */
  readonly childTasks: Map<string, ClaudeChildTaskFacts>
}

type ClaudeChildTaskFacts = Readonly<{
  childAgentId: string
  childRunId: string
  parentAgentId: string
  taskRef: string
  childKindRef?: string
}>

const claudeContentBlocks = (message: unknown): ReadonlyArray<Record<string, unknown>> => {
  const content = (message as { content?: unknown } | undefined)?.content
  return Array.isArray(content)
    ? content.filter((block): block is Record<string, unknown> => block !== null && typeof block === "object")
    : []
}

/**
 * Maps a Claude Agent SDK `result` message's `subtype` to a
 * `KhalaRuntimeFinishReason`. `success` (and `is_error !== true`) is the only
 * clean-finish case; `error_max_turns`/`error_max_budget_usd` map to
 * `"length"` (a budget was exhausted, the closest existing semantic match —
 * `KhalaRuntimeFinishReason` has no dedicated "budget" reason); everything
 * else (`error_during_execution`, `error_max_structured_output_retries`, an
 * unrecognized subtype, or `is_error: true` on an otherwise-`success`
 * result) maps to `"error"`.
 */
const claudeFinishReasonFromResult = (raw: ClaudeRawMessage): KhalaRuntimeFinishReason => {
  const subtype = typeof raw.subtype === "string" ? raw.subtype : undefined
  if (subtype === "success" && raw.is_error !== true) return "stop"
  if (subtype === "error_max_turns" || subtype === "error_max_budget_usd") return "length"
  return "error"
}

const claudeChildTaskFacts = (
  raw: ClaudeRawMessage,
  ctx: ClaudeRuntimeEventTranslationContext,
): Readonly<{ taskId: string; facts: ClaudeChildTaskFacts }> | null => {
  const taskId = typeof raw.task_id === "string" && raw.task_id.length > 0
    ? raw.task_id
    : null
  if (taskId === null) return null
  const toolUseId = typeof raw.tool_use_id === "string" && raw.tool_use_id.length > 0
    ? raw.tool_use_id
    : taskId
  const subagentType = typeof raw.subagent_type === "string" && raw.subagent_type.length > 0
    ? raw.subagent_type
    : null
  return {
    taskId,
    facts: {
      childAgentId: stableId("child.claude", taskId),
      childRunId: stableId("run.child.claude", taskId),
      parentAgentId: ctx.turnId,
      taskRef: stableId("task.claude", toolUseId),
      ...(subagentType === null
        ? {}
        : { childKindRef: stableId("agent_kind.claude", subagentType) }),
    },
  }
}

/**
 * Translates ONE raw Claude Agent SDK `SDKMessage` into zero or more
 * `KhalaRuntimeEvent`s. Pure given its context's seams, mirroring
 * `codexRawEventToRuntimeEvents` — real production calls thread a live
 * sequence counter, clock, and per-turn tool/child correlation maps through
 * it; tests inject deterministic fakes.
 *
 * Only the full, non-streaming message shapes are handled (`assistant`
 * carries a COMPLETE `BetaMessage`, not per-token deltas) since
 * `runWithRealClaudeAgentSdk` does not enable `includePartialMessages` —
 * matching Codex's own per-item (not per-token) granularity today. A single
 * `text.delta` + `text.completed` pair is emitted per text content block,
 * exactly like Codex's `agent_message` item handling. Typed system task
 * messages are handled separately as body-free child lifecycle facts.
 */
export const claudeRawMessageToRuntimeEvents = (
  raw: ClaudeRawMessage,
  ctx: ClaudeRuntimeEventTranslationContext,
): ReadonlyArray<KhalaRuntimeEvent> => {
  const type = typeof raw.type === "string" ? raw.type : undefined
  const subtype = typeof raw.subtype === "string" ? raw.subtype : undefined
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

  if (type === "system" && subtype === "init") {
    if (ctx.turnStarted.value) return []
    ctx.turnStarted.value = true
    return [base("turn.started", {})]
  }

  if (type === "system" && subtype === "task_started") {
    const taskType = typeof raw.task_type === "string" ? raw.task_type : null
    const subagentType = typeof raw.subagent_type === "string" ? raw.subagent_type : null
    if (taskType !== "subagent" && (subagentType === null || subagentType.length === 0)) return []
    const child = claudeChildTaskFacts(raw, ctx)
    if (child === null) return []
    ctx.childTasks.set(child.taskId, child.facts)
    return [base("agent.child.started", child.facts)]
  }

  if (type === "system" && subtype === "task_progress") {
    const taskId = typeof raw.task_id === "string" ? raw.task_id : null
    if (taskId === null) return []
    let facts = ctx.childTasks.get(taskId)
    const events: KhalaRuntimeEvent[] = []
    if (facts === undefined && typeof raw.subagent_type === "string") {
      const child = claudeChildTaskFacts(raw, ctx)
      if (child === null) return []
      facts = child.facts
      ctx.childTasks.set(child.taskId, facts)
      events.push(base("agent.child.started", facts))
    }
    if (facts === undefined) return []
    events.push(base("agent.child.progress", {
      childAgentId: facts.childAgentId,
      childRunId: facts.childRunId,
      parentAgentId: facts.parentAgentId,
      taskRef: facts.taskRef,
    }))
    return events
  }

  if (type === "system" && subtype === "task_notification") {
    const taskId = typeof raw.task_id === "string" ? raw.task_id : null
    if (taskId === null) return []
    const facts = ctx.childTasks.get(taskId)
    if (facts === undefined) return []
    ctx.childTasks.delete(taskId)
    const status = raw.status
    const finishReason: KhalaRuntimeFinishReason = status === "completed"
      ? "stop"
      : status === "stopped"
        ? "interrupted"
        : "error"
    const usage = raw.usage as Record<string, unknown> | undefined
    const totalTokens = usage?.total_tokens
    return [base("agent.child.finished", {
      childAgentId: facts.childAgentId,
      childRunId: facts.childRunId,
      parentAgentId: facts.parentAgentId,
      taskRef: facts.taskRef,
      finishReason,
      ...(typeof totalTokens === "number" && Number.isFinite(totalTokens) && totalTokens >= 0
        ? { usage: { totalTokens, usageRef: stableId("usage.child.claude", taskId) } }
        : {}),
    })]
  }

  if (type === "assistant") {
    const events: KhalaRuntimeEvent[] = []
    for (const block of claudeContentBlocks(raw.message)) {
      const blockType = typeof block.type === "string" ? block.type : undefined
      if (blockType === "text") {
        const text = typeof block.text === "string" ? block.text : ""
        const messageId = randomUUID()
        events.push(base("text.delta", { chunkId: randomUUID(), messageId, text }))
        events.push(base("text.completed", { messageId }))
        continue
      }
      if (blockType === "thinking") {
        const text = typeof block.thinking === "string" ? block.thinking : ""
        const messageId = randomUUID()
        events.push(base("reasoning.delta", { chunkId: randomUUID(), messageId, text }))
        events.push(base("reasoning.completed", { messageId }))
        continue
      }
      if (blockType === "tool_use") {
        const toolCallId = typeof block.id === "string" ? block.id : randomUUID()
        const toolName = typeof block.name === "string" ? block.name : "tool"
        ctx.pendingToolCalls.set(toolCallId, toolName)
        const authority = runtimeOwnerLocalToolAuthority(toolName)
        events.push(base("tool.call", { authority, toolCallId, toolName }))
        continue
      }
      // redacted_thinking, server_tool_use, image, and other block kinds are
      // not surfaced as their own runtime event kind yet — the turn's text,
      // tool, and usage events still fully account for the turn.
    }
    return events
  }

  if (type === "user") {
    const events: KhalaRuntimeEvent[] = []
    for (const block of claudeContentBlocks(raw.message)) {
      if (block.type !== "tool_result") continue
      const toolCallId = typeof block.tool_use_id === "string" ? block.tool_use_id : randomUUID()
      const toolName = ctx.pendingToolCalls.get(toolCallId) ?? "tool"
      const authority = runtimeOwnerLocalToolAuthority(toolName)
      if (block.is_error === true) {
        events.push(
          base("tool.error", {
            authority,
            errorRef: randomUUID(),
            messageSafe: `${toolName} failed`,
            toolCallId,
            toolName,
          }),
        )
      } else {
        events.push(base("tool.result", { authority, resultRef: randomUUID(), toolCallId, toolName }))
      }
    }
    return events
  }

  if (type === "result") {
    const usageRaw = raw.usage as Record<string, unknown> | undefined
    const inputTokens = typeof usageRaw?.input_tokens === "number" ? usageRaw.input_tokens : 0
    const outputTokens = typeof usageRaw?.output_tokens === "number" ? usageRaw.output_tokens : 0
    const cacheReadInputTokens =
      typeof usageRaw?.cache_read_input_tokens === "number" ? usageRaw.cache_read_input_tokens : 0
    const cacheWriteInputTokens =
      typeof usageRaw?.cache_creation_input_tokens === "number" ? usageRaw.cache_creation_input_tokens : 0
    return [
      base("usage.recorded", {
        usage: {
          cacheReadInputTokens,
          cacheWriteInputTokens,
          inputTokens,
          outputTokens,
          // Claude's usage shape has no separate reasoning/thinking token
          // count (thinking tokens are already folded into output_tokens),
          // unlike Codex's `reasoning_output_tokens` — omitted rather than
          // fabricated as zero-and-meaningful.
          totalTokens: inputTokens + outputTokens,
          usageRef: randomUUID(),
        },
      }),
      base("turn.finished", { finishReason: claudeFinishReasonFromResult(raw) }),
    ]
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
  readonly imagePaths?: ReadonlyArray<string>
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

export const runtimeCodexPromptWithImages = (
  instructions: string,
  imagePaths: ReadonlyArray<string> | undefined,
): string | ReadonlyArray<Readonly<{ type: "text"; text: string }> | Readonly<{
  type: "local_image"
  path: string
}>> => imagePaths === undefined || imagePaths.length === 0
  ? instructions
  : [
      { type: "text", text: instructions },
      ...imagePaths.map(path => ({ type: "local_image" as const, path })),
    ]

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
/**
 * Default Codex event source (CUT-11 #8691): the typed app-server transport
 * first — the only source that carries `subAgentActivity` /
 * receiver-bearing `collabAgentToolCall` child records — with exactly one
 * fallback to the exec SDK runner when the app-server fails BEFORE any
 * event frame (spawn/handshake/typed pre-turn error; the probe-located
 * bundled-binary failure mode). Post-frame failures never re-execute.
 */
export const runWithCodexAppServerFirst: RuntimeCodexThreadRunner = async (input) => {
  const appServer = makeCodexAppServerRunner()
  try {
    return await appServer(input)
  } catch (error) {
    if (!isCodexAppServerPreFrameFailure(error)) throw error
    return runWithRealCodexSdk(input)
  }
}

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
  const prompt = runtimeCodexPromptWithImages(input.instructions, input.imagePaths)
  const result = await thread.runStreamed(prompt as string, { signal: input.signal })
  return result as { events: AsyncIterable<CodexRawEvent> }
}

// ---------------------------------------------------------------------------
// Claude Agent SDK thread execution seam (#8404)
// ---------------------------------------------------------------------------

export type RuntimeClaudeThreadRunner = (input: {
  readonly instructions: string
  readonly cwd: string
  readonly env: Record<string, string | undefined>
  readonly signal: AbortSignal
  readonly permissionMode: ClaudePermissionMode | "default"
  readonly permissionAuthorityRef?: string
  readonly model?: string
  readonly canUseTool?: CanUseTool
  readonly images?: NonNullable<ChatMessageBody["attachments"]>
  /**
   * When set, resume this existing Claude Agent SDK session
   * (`options.resume`) instead of starting a fresh, contextless one — the
   * cross-turn continuity mechanism `handleTurnStart` wires from
   * `store.getRuntimeClaudeSessionId`. Only meaningful when the account
   * resuming is the SAME one that created the session (isolated per-account
   * `CLAUDE_CONFIG_DIR` homes); a mismatch fails cleanly, not silently.
   */
  readonly resumeSessionId?: string
}) => Promise<{ readonly messages: AsyncIterable<ClaudeRawMessage> }>

export const runtimeClaudeUserMessageWithImages = (
  instructions: string,
  images: NonNullable<ChatMessageBody["attachments"]>,
) => ({
  type: "user" as const,
  message: {
    role: "user" as const,
    content: [
      { type: "text" as const, text: instructions },
      ...images.map(image => ({
        type: "image" as const,
        source: {
          type: "base64" as const,
          media_type: image.mediaType,
          data: image.dataBase64,
        },
      })),
    ],
  },
  parent_tool_use_id: null,
})

export const DEFAULT_HOSTED_KHALA_RUNTIME_MODEL = "gemini-3.5-flash"
export const HOSTED_KHALA_RUNTIME_PROVIDER = "vertex-gemini"

export type RuntimeHostedKhalaThreadRunner = (input: {
  readonly instructions: string
  readonly signal: AbortSignal
  readonly baseUrl: string
  readonly agentToken: string
  readonly model: string
  readonly source: KhalaRuntimeSource
  readonly threadId: string
  readonly turnId: string
  readonly allocateSequence: () => number
  readonly nowIso: () => string
  readonly images?: NonNullable<ChatMessageBody["attachments"]>
  readonly fetchImpl?: typeof globalThis.fetch
}) => Promise<{ readonly events: AsyncIterable<KhalaRuntimeEvent> }>

export type RuntimeUsageReceiptProvider = Readonly<{
  backendProfile: string
  model: string
  provider: string
}>

export type RuntimeUsageReceiptRecorder = (input: {
  readonly event: Extract<KhalaRuntimeEvent, { kind: "usage.recorded" }>
  readonly lane: KhalaRuntimeLane
  readonly ownerUserId: string
  readonly provider: RuntimeUsageReceiptProvider
  readonly pylonRef: string
  readonly threadId: string
  readonly turnId: string
}) => Promise<void>

const openAiChatCompletionText = (body: unknown): string => {
  const choices = (body as { choices?: unknown } | undefined)?.choices
  if (!Array.isArray(choices)) return ""
  const first = choices[0] as { message?: unknown; text?: unknown } | undefined
  const message = first?.message as { content?: unknown } | undefined
  const content = message?.content
  if (typeof content === "string") return content
  if (Array.isArray(content)) {
    return content
      .map((part) => ((part as { text?: unknown } | undefined)?.text))
      .filter((text): text is string => typeof text === "string")
      .join("")
  }
  return typeof first?.text === "string" ? first.text : ""
}

/**
 * OpenAI-shaped usage extraction for the hosted-Khala gateway lane. EXACT for
 * thinking + cached models (#8503): mirrors the agent-computer turn-runner's
 * `chatCompletionUsage`. The `/api/khala/cloud/runtime-turn-usage` route bills
 * `output + reasoning` (`khalaCloudRuntimeUsageTokenCounts`) and OpenAI's
 * contract folds `reasoning_tokens` INTO `completion_tokens`, so we return the
 * NON-reasoning remainder as `outputTokens` and carry `reasoning_tokens`
 * separately — the route re-sums them to `completion_tokens`, keeping the
 * billable total unchanged while replacing the prior hardcoded-zero false-exact.
 * A provider that reports reasoning OUTSIDE completion is carried additively
 * (never lost). `cached_tokens` (a subset of prompt) is carried, clamped to
 * input.
 */
const openAiChatCompletionUsage = (
  body: unknown,
): Readonly<{
  inputTokens: number
  outputTokens: number
  totalTokens: number
  reasoningTokens: number
  cacheReadTokens: number
}> => {
  const usage = (body as { usage?: unknown } | undefined)?.usage as
    | Record<string, unknown>
    | undefined
  const integer = (value: unknown): number =>
    typeof value === "number" && Number.isFinite(value)
      ? Math.max(0, Math.trunc(value))
      : 0
  const completionDetails = usage?.completion_tokens_details as
    | Record<string, unknown>
    | undefined
  const promptDetails = usage?.prompt_tokens_details as
    | Record<string, unknown>
    | undefined
  const inputTokens = integer(usage?.prompt_tokens ?? usage?.input_tokens)
  const completionTokens = integer(
    usage?.completion_tokens ?? usage?.output_tokens,
  )
  const reasoningTokens = integer(completionDetails?.reasoning_tokens)
  const cacheReadTokens = Math.min(
    inputTokens,
    integer(promptDetails?.cached_tokens),
  )
  const folded = reasoningTokens <= completionTokens
  const outputTokens = folded
    ? completionTokens - reasoningTokens
    : completionTokens
  const billableOutput = outputTokens + reasoningTokens
  return {
    inputTokens,
    outputTokens,
    reasoningTokens,
    cacheReadTokens,
    totalTokens: Math.max(
      inputTokens + billableOutput,
      integer(usage?.total_tokens),
    ),
  }
}

const hostedKhalaRuntimeEventBase = (
  input: Pick<
    Parameters<RuntimeHostedKhalaThreadRunner>[0],
    "allocateSequence" | "nowIso" | "source" | "threadId" | "turnId"
  >,
  kind: KhalaRuntimeEvent["kind"],
  extra: Record<string, unknown>,
): KhalaRuntimeEvent =>
  decodeKhalaRuntimeEvent({
    causalityRefs: [],
    eventId: randomUUID(),
    kind,
    observedAt: input.nowIso(),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_event.v1",
    sequence: input.allocateSequence(),
    source: input.source,
    threadId: input.threadId,
    turnId: input.turnId,
    visibility: "private",
    ...extra,
  })

export const runWithHostedKhalaGateway: RuntimeHostedKhalaThreadRunner = async (
  input,
) => {
  const fetchImpl = input.fetchImpl ?? globalThis.fetch
  const url = new URL("/v1/chat/completions", input.baseUrl)
  const response = await fetchImpl(url.toString(), {
    body: JSON.stringify({
      messages: [{
        content: input.images === undefined || input.images.length === 0
          ? input.instructions
          : [
              { type: "text", text: input.instructions },
              ...input.images.map(image => ({
                type: "image_url",
                image_url: { url: `data:${image.mediaType};base64,${image.dataBase64}` },
              })),
            ],
        role: "user",
      }],
      model: input.model,
      stream: false,
    }),
    headers: {
      Authorization: `Bearer ${input.agentToken}`,
      "content-type": "application/json",
      "x-openagents-client": "khala-code-mobile",
      "x-openagents-demand-kind": "external",
      "x-openagents-demand-source": "khala_mobile_org_cloud_runtime",
    },
    method: "POST",
    signal: input.signal,
  })
  if (!response.ok) {
    throw new Error(`hosted Khala gateway returned HTTP ${response.status}`)
  }
  const body = await response.json()
  const text = openAiChatCompletionText(body)
  const usage = openAiChatCompletionUsage(body)
  if (usage.inputTokens + usage.outputTokens <= 0) {
    throw new Error("hosted Khala gateway response did not include exact token usage")
  }
  return {
    events: (async function* () {
      yield hostedKhalaRuntimeEventBase(input, "turn.started", {})
      if (text.length > 0) {
        const messageId = randomUUID()
        yield hostedKhalaRuntimeEventBase(input, "text.delta", {
          chunkId: randomUUID(),
          messageId,
          text,
        })
        yield hostedKhalaRuntimeEventBase(input, "text.completed", {
          messageId,
        })
      }
      yield hostedKhalaRuntimeEventBase(input, "usage.recorded", {
        providerMetadata: {
          metadataRefs: [],
          modelRef: input.model,
          providerRef: HOSTED_KHALA_RUNTIME_PROVIDER,
        },
        usage: {
          inputTokens: usage.inputTokens,
          outputTokens: usage.outputTokens,
          // EXACT reasoning/cache (#8503): the receipt route bills
          // output+reasoning, so these carry the split extracted above (no more
          // hardcoded-zero false-exact for thinking/cached hosted turns).
          reasoningTokens: usage.reasoningTokens,
          cacheReadInputTokens: usage.cacheReadTokens,
          totalTokens: usage.totalTokens,
          usageRef: `usage.hosted_khala.${randomUUID()}`,
        },
      })
      yield hostedKhalaRuntimeEventBase(input, "turn.finished", {
        finishReason: "stop" satisfies KhalaRuntimeFinishReason,
      })
    })(),
  }
}

/**
 * The real runner: one Claude Agent SDK `query()` session against the given
 * working directory, using the permission mode already admitted from an
 * exact process-local authority (or the bounded default), and aborted via the
 * given `AbortSignal`. Bridges the caller's plain `AbortSignal` into the SDK's
 * own
 * `AbortController` option (the SDK wants a controller it can inspect, not
 * just a signal, unlike the Codex SDK's `runStreamed(prompt, {signal})`).
 *
 * Invokes `query()` with a single string `prompt`, NOT the SDK's streaming
 * input mode (`prompt: AsyncIterable<SDKUserMessage>`) — see this module's
 * doc for why that means genuine mid-turn steering is not wired in this
 * pass even though the SDK supports it.
 *
 * When `input.resumeSessionId` is set, resumes that Claude session
 * (`options.resume`) instead of starting a fresh one, so a follow-up turn in
 * the same Khala Sync chat thread keeps the model's prior context.
 */
export const runWithRealClaudeAgentSdk: RuntimeClaudeThreadRunner = async (input) => {
  const sdk = (await import(CLAUDE_AGENT_SDK_PACKAGE)) as {
    query: (args: { prompt: string | AsyncIterable<unknown>; options?: Record<string, unknown> }) => AsyncIterable<ClaudeRawMessage>
  }
  const abort = new AbortController()
  if (input.signal.aborted) abort.abort()
  else input.signal.addEventListener("abort", () => abort.abort(), { once: true })
  const prompt = input.images === undefined || input.images.length === 0
    ? input.instructions
    : (async function* () {
        yield runtimeClaudeUserMessageWithImages(input.instructions, input.images!)
      })()
  const messages = sdk.query({
    prompt,
    options: {
      abortController: abort,
      cwd: input.cwd,
      env: input.env,
      permissionMode: input.permissionMode,
      ...(input.canUseTool !== undefined
        ? { settingSources: [], canUseTool: input.canUseTool }
        : input.permissionMode === "bypassPermissions"
        ? { settingSources: ["project"] }
        : { settingSources: [], allowedTools: [] }),
      ...(input.model === undefined ? {} : { model: input.model }),
      ...(input.resumeSessionId === undefined ? {} : { resume: input.resumeSessionId }),
    },
  })
  return { messages }
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
  readonly currentEventSequence?: () => number
  readonly nextMutationId: () => number
  /** The Khala Sync thread this turn belongs to (needed to correlate a
   * `message.append` intent's `turnId` back to a thread, and to seed any
   * queued follow-up turn in the same thread). */
  readonly threadId: string
  /**
   * Which provider lane this turn is dispatching under (#8404). Threaded
   * through so `turn.interrupt`'s `turn.interrupted` event reports the right
   * `source.lane`/`adapterKind`, `message.append`'s queued-steering detail
   * names the right SDK, and a queued follow-up turn (`dispatchQueuedFollowUps`)
   * stays on the SAME provider as the turn it followed rather than defaulting
   * to Codex.
   */
  readonly lane: KhalaRuntimeLane
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
  /** Trusted owner-local composition only. Org-cloud/all-owner pollers omit it. */
  readonly claudeOwnerLocalPermissionIssuer?: (input: Readonly<{
    pylonRef: string
    runRef: string
    operationRef: string
    accountRefHash: string
    now: Date
  }>) => ClaudeOwnerLocalPermissionControl
  /** Explicit durable approval authority; omitted preserves prior defaults. */
  readonly runtimeInteractionAuthority?: PylonRuntimeInteractionAuthority
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
  /** Test/override seam for the `turn.continue`/`turn.retry` turn-state lookup. */
  readonly fetchRuntimeTurnImpl?: (options: {
    baseUrl: string
    adminToken: string
    turnId: string
  }) => Promise<FetchRuntimeTurnResult>
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
  /** Test/override seam for the Claude Agent SDK thread runner (#8404). */
  readonly claudeThreadRunner?: RuntimeClaudeThreadRunner
  /**
   * Test/override seam for the hosted Khala/Gemini lane (#8473). Production
   * org-cloud supervisors wire `runWithHostedKhalaGateway`; owner-local
   * Pylons leave it unset, so `hosted_khala` stays fail-closed there.
   */
  readonly hostedKhalaThreadRunner?: RuntimeHostedKhalaThreadRunner
  readonly hostedKhalaModel?: string
  readonly hostedKhalaFetchImpl?: typeof globalThis.fetch
  /**
   * Optional exact usage receipt sink (#8473). Org-cloud supervisors wire this
   * to `/api/khala/cloud/runtime-turn-usage`; legacy owner-local Pylons leave
   * it unset so their existing own-capacity ledger path is untouched.
   */
  readonly recordUsageReceiptImpl?: RuntimeUsageReceiptRecorder
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
      throw new Error(
        `runtime.recordEvent ${input.event.kind} sequence ${input.event.sequence} push failed: ${detail ?? "unknown"}`,
      )
    }
  }

const makeCounter = (start = 0): (() => number) => {
  let value = start
  return () => {
    value += 1
    return value
  }
}

const makeEventSequence = (start = 0): Readonly<{
  current: () => number
  next: () => number
}> => {
  let value = start
  return { current: () => value, next: () => { const allocated = value; value += 1; return allocated } }
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
 * caller — every failure path (unreachable message, no account, Codex/Claude
 * SDK error) is reported as a `turn.finished` event with `finishReason:
 * "error"` (or absorbed silently on a genuine `turn.interrupt` abort, since
 * the interrupt handler already recorded the terminal event for that
 * case). Callers launch this WITHOUT awaiting it (fire-and-forget) so the
 * enforcement tick stays fast.
 *
 * Branches on `input.lane` (#8404) for which provider actually runs the
 * turn and which raw-event translator normalizes its stream, but shares one
 * push/error/finish-defensive shell across both — a bad/missing terminal
 * event, an interrupt-during-dispatch, or a thrown SDK error are handled
 * identically regardless of provider.
 */
const dispatchTurnStart = async (input: {
  readonly options: EnforceRuntimeIntentsOptions
  readonly store: PylonOrchestrationStore
  readonly intent: RuntimeControlIntentRow
  readonly turnId: string
  readonly prompt: string
  readonly images?: NonNullable<ChatMessageBody["attachments"]>
  readonly account?: ResolvedPylonAccountSelection
  readonly turn: ActiveRuntimeTurn
  readonly source: KhalaRuntimeSource
  readonly lane: KhalaRuntimeLane
  readonly startedClaimed?: boolean
  /** Codex resume-thread id OR Claude resume-session id, depending on `lane`. */
  readonly resumeRef?: string
}): Promise<void> => {
  const { options, store, intent, turnId, prompt, images, account, turn, source, lane, resumeRef } = input
  const pushEvent = options.pushEventImpl ?? defaultPushEvent(options.baseUrl, options.agentToken)
  // `handleTurnStart` durably claims sequence 0 before provider execution.
  // Provider-native started frames are therefore acknowledgement noise, not
  // a second lifecycle event.
  const turnStarted = { value: input.startedClaimed ?? false }

  const pushOne = async (event: KhalaRuntimeEvent): Promise<void> => {
    await pushEvent({
      clientGroupId: turn.clientGroupId,
      clientId: turn.clientId,
      event,
      mutationId: turn.nextMutationId(),
    })
    if (event.kind === "usage.recorded" && options.recordUsageReceiptImpl !== undefined) {
      try {
        await options.recordUsageReceiptImpl({
          event,
          lane,
          ownerUserId: intent.ownerUserId,
          provider: usageReceiptProviderForLane(lane, options),
          pylonRef: options.pylonRef,
          threadId: intent.threadId,
          turnId,
        })
      } catch (error) {
        options.log?.(
          `runtime-intent turn=${turnId} thread=${intent.threadId} failed to record exact usage receipt: ${boundedDetail(
            error instanceof Error ? error.message : "unknown",
          )}`,
        )
      }
    }
  }

  /** Best-effort: capture the Codex SDK's own thread id (from its
   * `thread.started` event) so a LATER turn in this same Khala thread can
   * resume it. Never throws — a failure to persist this is a lost
   * continuity opportunity, not a dispatch failure. */
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

  /** Best-effort analogue of `captureCodexThreadId` for Claude: every SDK
   * message carries `session_id`, so this captures it from the first one
   * seen (mirroring `../claude-composer.ts`'s own capture loop). */
  const captureClaudeSessionId = (raw: ClaudeRawMessage): void => {
    const claudeSessionId = raw.session_id
    if (typeof claudeSessionId !== "string" || claudeSessionId.length === 0) return
    try {
      store.setRuntimeClaudeSessionId(intent.threadId, claudeSessionId)
    } catch (error) {
      options.log?.(
        `runtime-intent turn=${turnId} thread=${intent.threadId} failed to persist claude session id: ${boundedDetail(
          error instanceof Error ? error.message : "unknown",
        )}`,
      )
    }
  }

  let finishedPushed = false
  try {
    const cwd = await options.ensureWorkspace(intent.threadId)
    if (lane === "claude_pylon") {
      if (account === undefined) {
        throw new Error("Claude runtime dispatch requires a resolved account")
      }
      const env = pylonAccountEnvironment(process.env as Record<string, string | undefined>, account)
      const runClaudeThread = options.claudeThreadRunner ?? runWithRealClaudeAgentSdk
      const permissionNow = options.now ?? new Date()
      const permissionControl = options.claudeOwnerLocalPermissionIssuer?.({
        pylonRef: options.pylonRef,
        runRef: turnId,
        operationRef: intent.intentId,
        accountRefHash: account.accountRefHash,
        now: permissionNow,
      })
      const permission = admitClaudePermission({
        ...(permissionControl === undefined ? {} : { control: permissionControl }),
        expected: {
          pylonRef: options.pylonRef,
          runRef: turnId,
          operationRef: intent.intentId,
          accountRefHash: account.accountRefHash,
        },
        now: permissionNow,
      })
      if (permission.kind === "refused") {
        throw new Error(`Claude permission authority refused (${permission.blockerRef})`)
      }
      const canUseTool = options.runtimeInteractionAuthority === undefined ||
          turn.currentEventSequence === undefined
        ? undefined
        : createClaudeCanUseToolInteractionController({
            authority: options.runtimeInteractionAuthority,
            requestFor: tool => {
              const requestedAt = new Date()
              const toolName = tool.toolName.slice(0, 160) || "tool"
              return {
                interactionRef: stableId("interaction.runtime_tool", `${turnId}:${tool.toolUseId}`),
                threadRef: intent.threadId,
                turnRef: turnId,
                requestedSequence: turn.currentEventSequence!(),
                requestedAt: requestedAt.toISOString(),
                expiresAt: new Date(requestedAt.getTime() + 5 * 60_000).toISOString(),
                source,
                causalityRefs: [intent.intentId],
                payload: {
                  kind: "tool_approval",
                  displayText: `Allow ${toolName} for this coding turn?`,
                  toolCallId: stableId("tool_call.claude", tool.toolUseId),
                  toolName,
                  authority: {
                    allowed: false,
                    authorityRef: stableId("authority.runtime_tool", `${turnId}:${tool.toolUseId}`),
                    blockerRefs: ["blocker.owner_approval"],
                    decisionRef: stableId("decision.runtime_tool_pending", `${turnId}:${tool.toolUseId}`),
                    policyRef: "policy.openagents.runtime_tool_approval.v1",
                    status: "operator_escalation_required",
                    toolRef: stableId("tool.claude", toolName),
                  },
                },
              }
            },
          })
      const { messages } = await runClaudeThread({
        cwd,
        env,
        instructions: prompt,
        permissionMode: canUseTool === undefined ? permission.permissionMode : "default",
        ...(permission.authorityRef === null
          ? {}
          : { permissionAuthorityRef: permission.authorityRef }),
        signal: turn.abortController.signal,
        ...(images === undefined || images.length === 0 ? {} : { images }),
        ...(canUseTool === undefined ? {} : { canUseTool }),
        ...(resumeRef === undefined ? {} : { resumeSessionId: resumeRef }),
      })
      const pendingToolCalls = new Map<string, string>()
      const childTasks = new Map<string, ClaudeChildTaskFacts>()
      for await (const raw of messages) {
        captureClaudeSessionId(raw)
        const translated = claudeRawMessageToRuntimeEvents(raw, {
          allocateSequence: turn.nextEventSequence,
          nowIso: () => new Date().toISOString(),
          pendingToolCalls,
          childTasks,
          source,
          threadId: intent.threadId,
          turnId,
          turnStarted,
        })
        for (const event of translated) {
          if (input.startedClaimed === true && event.kind === "turn.started") continue
          if (event.kind === "turn.finished") finishedPushed = true
          await pushOne(event)
        }
      }
    } else if (lane === "codex_app_server") {
      if (account === undefined) {
        throw new Error("Codex runtime dispatch requires a resolved account")
      }
      const env = pylonAccountEnvironment(process.env as Record<string, string | undefined>, account)
      // CUT-11 (#8691): default to the typed app-server source (the only
      // honest Codex child-record transport), pre-frame fallback to the SDK.
      const runCodexThread = options.codexThreadRunner ?? runWithCodexAppServerFirst
      const imageDir = images === undefined || images.length === 0
        ? null
        : await mkdtemp(join(tmpdir(), "oa-runtime-images-"))
      const imagePaths: string[] = []
      if (imageDir !== null) {
        for (const [index, image] of images!.entries()) {
          const extension = image.mediaType === "image/png" ? "png"
            : image.mediaType === "image/jpeg" ? "jpg"
              : image.mediaType === "image/gif" ? "gif" : "webp"
          const path = join(imageDir, `${index + 1}.${extension}`)
          await writeFile(path, Buffer.from(image.dataBase64, "base64"), { mode: 0o600 })
          imagePaths.push(path)
        }
      }
      try {
        const { events } = await runCodexThread({
          cwd,
          env,
          instructions: prompt,
          networkAccessEnabled: true,
          signal: turn.abortController.signal,
          ...(imagePaths.length === 0 ? {} : { imagePaths }),
          ...(resumeRef === undefined ? {} : { resumeThreadId: resumeRef }),
        })
        const childThreads = new Map<string, CodexChildThreadFacts>()
        for await (const raw of events) {
          captureCodexThreadId(raw)
          const translated = codexRawEventToRuntimeEvents(raw, {
            allocateSequence: turn.nextEventSequence,
            childThreads,
            nowIso: () => new Date().toISOString(),
            source,
            threadId: intent.threadId,
            turnId,
            turnStarted,
          })
          for (const event of translated) {
            if (input.startedClaimed === true && event.kind === "turn.started") continue
            if (event.kind === "turn.finished") finishedPushed = true
            await pushOne(event)
          }
        }
      } finally {
        if (imageDir !== null) await rm(imageDir, { recursive: true, force: true })
      }
    } else if (lane === "hosted_khala") {
      const runHostedKhala = options.hostedKhalaThreadRunner
      if (runHostedKhala === undefined) {
        throw new Error("hosted_khala runtime dispatch is not configured")
      }
      const { events } = await runHostedKhala({
        agentToken: options.agentToken,
        allocateSequence: turn.nextEventSequence,
        baseUrl: options.baseUrl,
        fetchImpl: options.hostedKhalaFetchImpl,
        instructions: prompt,
        model: hostedKhalaModelForOptions(options),
        nowIso: () => new Date().toISOString(),
        signal: turn.abortController.signal,
        source,
        threadId: intent.threadId,
        turnId,
        ...(images === undefined || images.length === 0 ? {} : { images }),
      })
      for await (const event of events) {
        if (input.startedClaimed === true && event.kind === "turn.started") continue
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

/**
 * Which local provider account backs a `runtime.startTurn` intent's
 * `target.lane` (#8404). Any lane other than the two wired here is an
 * explicit `failed` outcome in `handleTurnStart`, never a silent fallback to
 * Codex.
 */
const SUPPORTED_DISPATCH_LANES: ReadonlyArray<KhalaRuntimeLane> = [
  "codex_app_server",
  "claude_pylon",
  "hosted_khala",
]
const SERVER_OWNED_MANAGED_CLOUD_LANE: KhalaRuntimeLane = "managed_cloud"

const providerForLane = (lane: KhalaRuntimeLane): "codex" | "claude_agent" =>
  lane === "claude_pylon" ? "claude_agent" : "codex"

const hostedKhalaModelForOptions = (
  options: Pick<EnforceRuntimeIntentsOptions, "hostedKhalaModel">,
): string => options.hostedKhalaModel ?? DEFAULT_HOSTED_KHALA_RUNTIME_MODEL

const sourceForLane = (
  lane: KhalaRuntimeLane,
  options?: Pick<EnforceRuntimeIntentsOptions, "hostedKhalaModel">,
): KhalaRuntimeSource => {
  if (lane === "claude_pylon") {
    return { adapterKind: "claude_code", lane: "claude_pylon", surface: "server" }
  }
  if (lane === "hosted_khala") {
    return {
      adapterKind: "openagents_native",
      lane: "hosted_khala",
      modelRef: hostedKhalaModelForOptions(options ?? {}),
      providerRef: HOSTED_KHALA_RUNTIME_PROVIDER,
      surface: "server",
    }
  }
  return { adapterKind: "codex", lane: "codex_app_server", surface: "server" }
}

const usageReceiptProviderForLane = (
  lane: KhalaRuntimeLane,
  options: Pick<EnforceRuntimeIntentsOptions, "hostedKhalaModel">,
): RuntimeUsageReceiptProvider => {
  if (lane === "claude_pylon") {
    return {
      backendProfile: "pylon-claude-org-capacity",
      model: "openagents/pylon-claude",
      provider: "pylon-claude-org-capacity",
    }
  }
  if (lane === "hosted_khala") {
    return {
      backendProfile: HOSTED_KHALA_RUNTIME_PROVIDER,
      model: hostedKhalaModelForOptions(options),
      provider: HOSTED_KHALA_RUNTIME_PROVIDER,
    }
  }
  return {
    backendProfile: "pylon-codex-org-capacity",
    model: "openagents/pylon-codex",
    provider: "pylon-codex-org-capacity",
  }
}

type SelectDispatchAccountResult =
  | {
      readonly ok: true
      readonly account: ResolvedPylonAccountSelection
      readonly accountRefHash: string
      readonly resumeRef?: string
    }
  | { readonly ok: false; readonly detail: string }

/**
 * Shared account selection/pin for both `handleTurnStart` and
 * `handleTurnContinueOrRetry` (#8410 follow-up — extracted so redispatching
 * an existing turn gets EXACTLY the same thread-resume account affinity,
 * round-robin tie-break, and pin-persistence behavior as starting one).
 * Never throws.
 */
const selectAndPinDispatchAccount = async (
  options: EnforceRuntimeIntentsOptions,
  store: PylonOrchestrationStore,
  threadId: string,
  lane: KhalaRuntimeLane,
  executionTargetId?: string,
): Promise<SelectDispatchAccountResult> => {
  const provider = providerForLane(lane)
  const candidates = await options.listCandidateAccounts()

  const expectedTargetPrefix = provider === "claude_agent" ? "claude:" : "codex:"
  const requestedAccountRefHash = executionTargetId === undefined
    ? undefined
    : executionTargetId.startsWith(expectedTargetPrefix)
      ? executionTargetId.slice(expectedTargetPrefix.length)
      : null
  if (requestedAccountRefHash === null || requestedAccountRefHash === "") {
    return { detail: `execution target does not match the ${provider} lane`, ok: false }
  }
  const requestedCandidate = requestedAccountRefHash === undefined
    ? undefined
    : candidates.find(candidate =>
        candidate.fleetAccount.accountRefHash === requestedAccountRefHash &&
        candidate.fleetAccount.provider === provider &&
        candidate.fleetAccount.readiness === "ready")
  if (requestedAccountRefHash !== undefined && requestedCandidate === undefined) {
    return { detail: `requested ${provider} execution target is not dispatch-ready`, ok: false }
  }

  // Thread-resume account affinity (#8410 follow-up): prefer the account
  // already PINNED to this Khala thread (see `store.getRuntimeDispatchAccountRefHash`'s
  // doc) over the ordinary round-robin pick, as long as it is STILL in the
  // real dispatch-ready set for this lane's provider — Codex/Claude sessions
  // are account-specific, so staying on the same account is what actually
  // lets `resumeThreadId`/`resumeSessionId` keep working across turns.
  const pinnedAccountRefHash = store.getRuntimeDispatchAccountRefHash(threadId)
  const pinnedCandidate = candidates.find(
    (c) =>
      c.fleetAccount.accountRefHash === pinnedAccountRefHash &&
      c.fleetAccount.provider === provider &&
      c.fleetAccount.readiness === "ready",
  )
  const lastUsedAccountRefHash = options.lastDispatchedAccountByThread?.get(threadId)
  const selected = requestedCandidate?.fleetAccount ??
    pinnedCandidate?.fleetAccount ??
    selectDispatchAccount(candidates.map((c) => c.fleetAccount), {
      provider,
      ...(lastUsedAccountRefHash === undefined ? {} : { lastUsedAccountRefHash }),
    })
  if (selected === undefined) {
    return {
      detail: `no dispatch-ready local ${provider === "claude_agent" ? "Claude" : "Codex"} account available`,
      ok: false,
    }
  }
  const candidate = candidates.find((c) => c.fleetAccount.accountRefHash === selected.accountRefHash)
  if (candidate === undefined) {
    return { detail: "invariant violated: selected account has no matching registry entry", ok: false }
  }
  const account = await options.resolveAccountSelection(candidate.registryEntry)
  if (account === null) {
    return {
      detail: `local ${provider === "claude_agent" ? "Claude" : "Codex"} account home for ${candidate.registryEntry.ref} could not be resolved`,
      ok: false,
    }
  }
  options.lastDispatchedAccountByThread?.set(threadId, selected.accountRefHash)
  try {
    // Pin (or re-pin) this thread to the account just selected, whether that
    // was the existing pin, a fresh round-robin pick, or a re-pin after the
    // previous pin went unhealthy — see `getRuntimeDispatchAccountRefHash`'s
    // doc. Best-effort: a failure to persist this is a lost continuity
    // opportunity for the NEXT dispatch, never a reason to fail this one.
    store.setRuntimeDispatchAccountRefHash(threadId, selected.accountRefHash)
  } catch (error) {
    options.log?.(
      `runtime-intent thread=${threadId} failed to persist dispatch account pin: ${boundedDetail(
        error instanceof Error ? error.message : "unknown",
      )}`,
    )
  }
  const resumeRef =
    lane === "claude_pylon"
      ? store.getRuntimeClaudeSessionId(threadId)
      : store.getRuntimeCodexThreadId(threadId)
  return {
    account,
    accountRefHash: selected.accountRefHash,
    ok: true,
    ...(resumeRef === null ? {} : { resumeRef }),
  }
}

const handleTurnStart = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
  store: PylonOrchestrationStore,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  const turnId = row.intent.turnId
  if (turnId === undefined) {
    return { detail: "turn.start intent carried no turnId", outcome: "failed" }
  }

  const targetLane = row.intent.target.lane
  if (targetLane === SERVER_OWNED_MANAGED_CLOUD_LANE) {
    return {
      detail: "managed_cloud is server-owned; local Pylon leaves it for Agent Computer dispatch",
      outcome: "skipped_stale",
    }
  }
  if (!SUPPORTED_DISPATCH_LANES.includes(targetLane)) {
    return {
      detail:
        `runtime.startTurn dispatch does not support target.lane "${targetLane}" — only ` +
        `${SUPPORTED_DISPATCH_LANES.join(" and ")} are wired in this consumer`,
      outcome: "failed",
    }
  }
  const lane = targetLane
  const source = sourceForLane(lane, options)

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

  let dispatchAccount: ResolvedPylonAccountSelection | undefined
  let dispatchResumeRef: string | undefined
  let dispatchDetail = `hosted Khala dispatch started on model ${hostedKhalaModelForOptions(options)}`
  if (lane !== "hosted_khala") {
    const selection = await selectAndPinDispatchAccount(
      options,
      store,
      row.threadId,
      lane,
      row.intent.target.executionTargetId,
    )
    if (!selection.ok) {
      return { detail: selection.detail, outcome: "failed" }
    }
    dispatchAccount = selection.account
    dispatchResumeRef = selection.resumeRef
    dispatchDetail = `dispatch started against account ${selection.accountRefHash}`
  }

  const clientIdentity = runtimeSyncClientForTurn({ pylonRef: options.pylonRef, turnId })
  const eventSequence = makeEventSequence(0)
  const turn: ActiveRuntimeTurn = {
    abortController: new AbortController(),
    clientGroupId: clientIdentity.clientGroupId,
    clientId: clientIdentity.clientId,
    interrupted: false,
    lane,
    currentEventSequence: eventSequence.current,
    nextEventSequence: eventSequence.next,
    nextMutationId: makeCounter(0),
    pendingAppendMessageIds: [],
    threadId: row.threadId,
  }
  const pushEvent = options.pushEventImpl ?? defaultPushEvent(options.baseUrl, options.agentToken)
  const claimEvent = decodeKhalaRuntimeEvent({
    causalityRefs: [row.intent.intentId],
    controlIntentId: row.intent.intentId,
    eventId: stableId("event.runtime_claim", turnId),
    kind: "turn.started",
    observedAt: new Date().toISOString(),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_event.v1",
    sequence: turn.nextEventSequence(),
    source,
    threadId: row.threadId,
    turnId,
    userMessageId: messageId,
    visibility: "private",
  })
  try {
    await pushEvent({
      clientGroupId: turn.clientGroupId,
      clientId: turn.clientId,
      event: claimEvent,
      mutationId: turn.nextMutationId(),
    })
  } catch (error) {
    // Sequence 0 is the durable single-winner claim: the server admits an
    // event only when its sequence equals the turn's current event_count.
    // A duplicate generation
    // or an indeterminate write never proceeds to provider execution. Carry
    // the bounded push error so a systematic rejection (auth, schema, or an
    // id-collision class of bug) is distinguishable from a lost race in the
    // supervisor log instead of being silently swallowed.
    return {
      detail: boundedDetail(
        `runtime turn claim was not newly admitted; provider dispatch skipped (${
          error instanceof Error ? error.message : String(error)
        })`,
      ),
      outcome: "skipped_stale",
    }
  }
  options.activeTurns.set(turnId, turn)
  void dispatchTurnStart({
    intent: row,
    lane,
    options,
    prompt: message.body,
    ...(message.attachments === undefined || message.attachments.length === 0
      ? {}
      : { images: message.attachments }),
    source,
    store,
    startedClaimed: true,
    turn,
    turnId,
    ...(dispatchAccount === undefined ? {} : { account: dispatchAccount }),
    ...(dispatchResumeRef === undefined ? {} : { resumeRef: dispatchResumeRef }),
  }).finally(() => {
    if (options.activeTurns.get(turnId) === turn) options.activeTurns.delete(turnId)
    if (turn.pendingAppendMessageIds.length > 0) {
      void dispatchQueuedFollowUps({
        lane,
        messageIds: turn.pendingAppendMessageIds,
        options,
        threadId: row.threadId,
      })
    }
  })

  return {
    detail: dispatchDetail,
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
 * `turn.start` (picking up `resumeThreadId`/`resumeSessionId` continuity
 * automatically via `store.getRuntimeCodexThreadId`/
 * `store.getRuntimeClaudeSessionId`). The follow-up's `target.lane` is the
 * SAME lane the original turn dispatched under (#8404) — a Claude-lane
 * turn's queued append becomes a Claude-lane follow-up, not a silent
 * fallback to Codex. Never thrown to the caller: failures here just mean the
 * follow-up turn does not get created and are logged, mirroring
 * `dispatchTurnStart`'s own fire-and-forget error handling — the ORIGINAL
 * turn's outcome is never affected by a queued follow-up's fate.
 */
const dispatchQueuedFollowUps = async (input: {
  readonly options: EnforceRuntimeIntentsOptions
  readonly threadId: string
  readonly lane: KhalaRuntimeLane
  readonly messageIds: ReadonlyArray<string>
}): Promise<void> => {
  const { options, threadId, lane, messageIds } = input
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
      origin: { lane, surface: "server" as const },
      redactionClass: "private_ref" as const,
      schema: "openagents.khala_runtime_control_intent.v1" as const,
      target: { lane },
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
    source: sourceForLane(input.turn.lane),
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

/**
 * Handle a `message.append` control intent. NOT literal mid-turn steering
 * for either provider today (see the module doc for why — Codex's SDK has no
 * mid-turn injection API at all, and this pass does not wire the Claude
 * Agent SDK's streaming-input mode even though the SDK itself supports it) —
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
    const sdkLabel =
      activeTurn.lane === "claude_pylon"
        ? "Claude Agent SDK's single-prompt query call"
        : activeTurn.lane === "hosted_khala"
          ? "hosted Khala gateway request"
          : "Codex SDK's single-prompt runStreamed call"
    return {
      detail:
        `mid-turn steering is not wired against the local ${sdkLabel}; ` +
        `chat_message.${messageId} was queued instead of applied — it will be dispatched as a real ` +
        `follow-up runtime.startTurn (resuming the same conversation where possible) once turn ${turnId} settles`,
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

const CONTINUE_INSTRUCTION = "Continue where you left off."
const RETRY_INSTRUCTION = "That didn't complete — please try again."

/**
 * Resolve the prompt for a `turn.continue`/`turn.retry` control intent
 * (#8410 follow-up). If the intent itself carries a resolvable
 * `chat_message.<id>` bodyRef (a caller explicitly supplying fresh
 * instructions), that message's body wins — identical resolution to
 * `turn.start`. Otherwise there is no new user input to resend: this is a
 * genuine "keep going"/"try again" request, not a disguised turn.start, so a
 * short built-in continuation instruction is used instead. Codex/Claude both
 * retain full prior conversation context via `resumeThreadId`/
 * `resumeSessionId` (the SAME cross-turn continuity `handleTurnStart`
 * already relies on), so this instruction is enough to make the resumed
 * session actually produce a new response — this consumer does not look up
 * the turn's ORIGINAL triggering message to resend verbatim, a bounded,
 * honestly-documented limitation rather than a silent shortcut.
 */
const resolvePromptForContinueOrRetry = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
): Promise<{
  ok: true
  prompt: string
  images?: NonNullable<ChatMessageBody["attachments"]>
} | { ok: false; detail: string }> => {
  const bodyRef =
    row.intent.kind === "turn.continue" || row.intent.kind === "turn.retry" ? row.intent.bodyRef : undefined
  const messageId = chatMessageIdFromBodyRef(bodyRef)
  if (messageId === null) {
    return { ok: true, prompt: row.intent.kind === "turn.retry" ? RETRY_INSTRUCTION : CONTINUE_INSTRUCTION }
  }

  const fetchChatMessageImpl = options.fetchChatMessageImpl ?? fetchChatMessageFromWorker
  const messageResult = await fetchChatMessageImpl({
    adminToken: options.adminToken,
    baseUrl: options.baseUrl,
    messageId,
    threadId: row.threadId,
  })
  if (!messageResult.ok) {
    return { detail: boundedDetail(`chat_message lookup transport failed: ${messageResult.error}`), ok: false }
  }
  if (messageResult.message === null || messageResult.message.deletedAt !== null) {
    return {
      detail: `referenced chat_message.${messageId} does not exist (or was deleted) in thread ${row.threadId}`,
      ok: false,
    }
  }
  return {
    ok: true,
    prompt: messageResult.message.body,
    ...(messageResult.message.attachments === undefined ||
        messageResult.message.attachments.length === 0
      ? {}
      : { images: messageResult.message.attachments }),
  }
}

/**
 * Handle a `turn.continue`/`turn.retry` control intent (#8410 follow-up):
 * genuine local redispatch of the SAME turnId the mutator already re-queued
 * (see the module doc). Mirrors `handleTurnStart`'s account selection/pin
 * (`selectAndPinDispatchAccount`) and `dispatchTurnStart` shell exactly,
 * with two differences specific to resuming an ALREADY-SETTLED turn: the
 * prompt (`resolvePromptForContinueOrRetry`) and the event-sequence cursor,
 * which resumes from the turn's CURRENT `event_count`
 * (`fetchRuntimeTurnImpl`) instead of restarting at 0 — redispatching a turn
 * that already recorded events and renumbering from 0 would collide with an
 * existing `(turn_id, sequence)` pair and be rejected as a duplicate by
 * `runtime.recordEvent`.
 *
 * If this turnId is STILL actively dispatching on this exact process
 * (`options.activeTurns`), there is nothing to continue/retry yet —
 * `skipped_stale`, mirroring `handleTurnClose`'s precedent for the same
 * "nothing local to act on yet" shape.
 */
const handleTurnContinueOrRetry = async (
  options: EnforceRuntimeIntentsOptions,
  row: RuntimeControlIntentRow,
  store: PylonOrchestrationStore,
): Promise<{ outcome: RuntimeIntentOutcomeStatus; detail: string }> => {
  const turnId = row.intent.turnId
  if (turnId === undefined) {
    return { detail: `${row.intent.kind} intent carried no turnId`, outcome: "failed" }
  }
  if (options.activeTurns.has(turnId)) {
    return {
      detail:
        `turn ${turnId} is still actively dispatching locally; ${row.intent.kind} only resumes an already-` +
        `settled turn`,
      outcome: "skipped_stale",
    }
  }

  const targetLane = row.intent.target.lane
  if (targetLane === SERVER_OWNED_MANAGED_CLOUD_LANE) {
    return {
      detail: "managed_cloud is server-owned; local Pylon leaves it for Agent Computer dispatch",
      outcome: "skipped_stale",
    }
  }
  if (!SUPPORTED_DISPATCH_LANES.includes(targetLane)) {
    return {
      detail:
        `runtime.${row.intent.kind} dispatch does not support target.lane "${targetLane}" — only ` +
        `${SUPPORTED_DISPATCH_LANES.join(" and ")} are wired in this consumer`,
      outcome: "failed",
    }
  }
  const lane = targetLane
  const source = sourceForLane(lane, options)

  const fetchRuntimeTurnImpl = options.fetchRuntimeTurnImpl ?? fetchRuntimeTurnFromWorker
  const turnResult = await fetchRuntimeTurnImpl({ adminToken: options.adminToken, baseUrl: options.baseUrl, turnId })
  if (!turnResult.ok) {
    return { detail: boundedDetail(`runtime-turn lookup transport failed: ${turnResult.error}`), outcome: "failed" }
  }
  const turnState = turnResult.turn
  if (turnState === null) {
    return { detail: `referenced turn ${turnId} does not exist`, outcome: "failed" }
  }
  if (turnState.threadId !== row.threadId) {
    return {
      detail: `turn ${turnId} belongs to thread ${turnState.threadId}, not the ${row.intent.kind} intent's thread ${row.threadId}`,
      outcome: "failed",
    }
  }

  const promptResult = await resolvePromptForContinueOrRetry(options, row)
  if (!promptResult.ok) {
    return { detail: promptResult.detail, outcome: "failed" }
  }

  let dispatchAccount: ResolvedPylonAccountSelection | undefined
  let dispatchResumeRef: string | undefined
  let dispatchDetail =
    `${row.intent.kind} redispatch started on hosted Khala model ${hostedKhalaModelForOptions(options)}, ` +
    `resuming event sequence after ${turnState.eventCount}`
  if (lane !== "hosted_khala") {
    const selection = await selectAndPinDispatchAccount(
      options,
      store,
      row.threadId,
      lane,
      row.intent.target.executionTargetId,
    )
    if (!selection.ok) {
      return { detail: selection.detail, outcome: "failed" }
    }
    dispatchAccount = selection.account
    dispatchResumeRef = selection.resumeRef
    dispatchDetail =
      `${row.intent.kind} redispatch started against account ${selection.accountRefHash}, ` +
      `resuming event sequence after ${turnState.eventCount}`
  }

  const clientIdentity = runtimeSyncClientForTurn({ pylonRef: options.pylonRef, turnId })
  const eventSequence = makeEventSequence(turnState.eventCount)
  const turn: ActiveRuntimeTurn = {
    abortController: new AbortController(),
    clientGroupId: clientIdentity.clientGroupId,
    clientId: clientIdentity.clientId,
    interrupted: false,
    lane,
    // Resume numbering AFTER whatever this turn's earlier attempt already
    // recorded — never restart at 0 (see this function's doc).
    currentEventSequence: eventSequence.current,
    nextEventSequence: eventSequence.next,
    nextMutationId: makeCounter(0),
    pendingAppendMessageIds: [],
    threadId: row.threadId,
  }
  options.activeTurns.set(turnId, turn)
  void dispatchTurnStart({
    intent: row,
    lane,
    options,
    prompt: promptResult.prompt,
    ...(promptResult.images === undefined ? {} : { images: promptResult.images }),
    source,
    store,
    turn,
    turnId,
    ...(dispatchAccount === undefined ? {} : { account: dispatchAccount }),
    ...(dispatchResumeRef === undefined ? {} : { resumeRef: dispatchResumeRef }),
  }).finally(() => {
    if (options.activeTurns.get(turnId) === turn) options.activeTurns.delete(turnId)
    if (turn.pendingAppendMessageIds.length > 0) {
      void dispatchQueuedFollowUps({
        lane,
        messageIds: turn.pendingAppendMessageIds,
        options,
        threadId: row.threadId,
      })
    }
  })

  return {
    detail: dispatchDetail,
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
      return handleTurnContinueOrRetry(options, row, store)
    default:
      return { detail: `unrecognized control-intent kind ${row.intent.kind}`, outcome: "failed" }
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
