import {
  agentRunScope,
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  type AgentRunEntity,
  type AgentRunEventEntity,
  canonicalJson,
  type ChangelogEntry,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
  encodeAgentRunEntity,
  encodeAgentRunEventEntity,
  EntityId,
  EntityType,
} from "@openagentsinc/khala-sync"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"

/**
 * Agent run + goal scope projection (KS-6.6, #8416; SPEC §2.1
 * `scope.agent_run.<runId>`, §7 invariant 8/9).
 *
 * `scope.agent_run.<runId>` has been part of the read-auth taxonomy since
 * KS-7.1 (#8305), but had NO producer until this module: KS-8.13/#8324's
 * product-state projection routes team/thread scopes only
 * (`khala-code-product-state-projection.ts`'s `scopesForRow` has no
 * `agent_run` case, and `agent_runs`/`agent_goals` are not in
 * `KHALA_CODE_PRODUCT_STATE_TABLES`). This is the extension KS-6.6 asked
 * for: whenever an agent run is queued or relaunched (with a continuation),
 * `omni-handlers.ts` calls this projector ALONGSIDE its legacy
 * `notifySyncScopes(env, syncScopeForAgentRun(run))` poke, so the run's own
 * scope gets a real, replayable post-image instead of a bare "something
 * changed, go refetch" signal.
 *
 * ONE ENTITY PER SCOPE for the run itself: unlike `scope.public.gym-run-
 * progress` (one shared scope, many runRef-keyed entities) or
 * `scope.fleet_run.<id>` (owner-gated via `khala_sync_scope_owners`), the
 * `agent_run` entity is single-instance per scope — the run only ever
 * projects itself, keyed by its own id. No scope-owner bookkeeping table is
 * needed: read-side ownership already comes straight from D1 `agent_runs`
 * (`scope-auth.ts`'s `canReadResolvedRun`), so this projector needs no
 * bespoke migration — it rides the generic `khala_sync_changelog` +
 * `khala_sync_scopes` tables `withSyncTransaction` already provides.
 *
 * MANY-ENTITIES-PER-SCOPE for the event feed (KS-6.6 follow-up, closing the
 * "schema gap" from RUNBOOK.md's 2026-07-05 client-repoint research): the
 * SAME `scope.agent_run.<runId>` scope also carries `agent_run_event`
 * entities (`projectAgentRunEventsBestEffort` below), one per event, keyed
 * by the event's own id — the multi-entity-per-scope convention (borrowed
 * from `scope.public.gym-run-progress`'s runRef-keyed rows, adapted so every
 * event entity shares its parent run's scope rather than a single shared
 * public scope). This mirrors the legacy DO room, which already multiplexes
 * both `agent_runs` and `agent_run_events` collections onto one room/scope.
 *
 * REDACTION BOUNDARY (invariant 9): the caller passes an ALREADY public-safe
 * raw shape (mirroring `agentRunProjection`/`agentRunMissionProjection`/
 * `publicGoalContext`/`agentRunEventProjection` in the Worker's
 * `omni-runs.ts` — the exact fields already shipped to authenticated clients
 * over the legacy sync-worker outbox and the mission-launch HTTP response).
 * This function still allowlist-maps field by field through
 * `decodeAgentRunEntity`/`decodeAgentRunEventEntity` (never spreads) and
 * re-checks the serialized post-image against
 * {@link AGENT_RUN_POST_IMAGE_FORBIDDEN_PATTERN} as defense in depth before
 * it can ever reach storage.
 */

// ---------------------------------------------------------------------------
// Named system writer (SPEC §7 invariant 3)
// ---------------------------------------------------------------------------

export const AGENT_RUN_PROJECTION_SYSTEM_REF =
  "system:agent_run_projection.omni_handlers.v1"

// ---------------------------------------------------------------------------
// Redaction guard (defense in depth behind the contract patterns)
// ---------------------------------------------------------------------------

/**
 * Material that must NEVER appear in an agent_run post-image, checked
 * against the canonical serialization as a last line of defense (the
 * contract's ref/timestamp/literal patterns are the structural first line).
 * `goal` (the user's free-text objective) and `repository.owner`/`repo`/
 * `ref` are deliberately exempt content fields — same "content field"
 * discipline as chat `body` in `khala-code-product-state-projection.ts` —
 * so this scan runs against the STRUCTURAL view only (every field except
 * those content fields), mirroring that module's `structuralView` helper.
 */
export const AGENT_RUN_POST_IMAGE_FORBIDDEN_PATTERN =
  /token_hash|api[_-]?key|authorization:|bearer[:\s]|mnemonic|secret|provideraccountref|authgrantref|githubwritegrantref|callback|\/users\/|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/i

/**
 * Content fields exempt from the forbidden-material scan (free user text).
 * `summary`/`payloadJson` cover the agent_run_event companion entity
 * (KS-6.6 event-feed follow-up) — `payloadJson` is already scrubbed of
 * credential-shaped material at D1 WRITE time
 * (`omni-runs.ts`'s `jsonOrNull`), so this exemption is consistent with the
 * "content field, not a ref" discipline used for `goal` above.
 */
const AGENT_RUN_POST_IMAGE_CONTENT_FIELDS: ReadonlySet<string> = new Set([
  "goal",
  "objective",
  "owner",
  "repo",
  "ref",
  "summary",
  "payloadJson",
])

export class AgentRunPostImageRedactionError extends Error {
  readonly _tag = "AgentRunPostImageRedactionError"
  override readonly name = "AgentRunPostImageRedactionError"
  constructor() {
    super(
      "refusing to project agent_run: serialized post-image matches the " +
        "forbidden-material pattern (SPEC §7 invariant 9)",
    )
  }
}

const structuralView = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(structuralView)
  if (typeof value !== "object" || value === null) return value
  const structural: Record<string, unknown> = {}
  for (const [key, entryValue] of Object.entries(value)) {
    structural[key] = AGENT_RUN_POST_IMAGE_CONTENT_FIELDS.has(key)
      ? undefined
      : structuralView(entryValue)
  }
  return structural
}

const assertAgentRunPostImageRedacted = (postImage: unknown): void => {
  if (
    AGENT_RUN_POST_IMAGE_FORBIDDEN_PATTERN.test(
      canonicalJson(structuralView(postImage)),
    )
  ) {
    throw new AgentRunPostImageRedactionError()
  }
}

// ---------------------------------------------------------------------------
// Append (single entity upsert — one scope per run, no aggregate)
// ---------------------------------------------------------------------------

export interface AgentRunProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason: "storage_failed" | "redaction_refused" | "projection_failed"
  readonly messageSafe: string
}

export type AgentRunProjectionOutcome =
  | { readonly ok: true; readonly entry: ChangelogEntry }
  | { readonly ok: false; readonly diagnostic: AgentRunProjectionDiagnostic }

const diagnosticFromUnknown = (
  error: unknown,
): AgentRunProjectionDiagnostic => {
  if (error instanceof AgentRunPostImageRedactionError) {
    return { messageSafe: error.message, reason: "redaction_refused" }
  }
  const tag = (error as { _tag?: unknown })?._tag
  if (tag === "KhalaSyncStorageError") {
    const messageSafe = (error as { messageSafe?: unknown }).messageSafe
    return {
      messageSafe:
        typeof messageSafe === "string" ? messageSafe : "storage failure",
      reason: "storage_failed",
    }
  }
  // Anything else (driver errors, schema decode failures) can embed raw row
  // values or connection strings — never echo them.
  return {
    messageSafe: "agent run projection failed",
    reason: "projection_failed",
  }
}

/**
 * Project ONE agent run (with its optionally-attached goal) into
 * `scope.agent_run.<runId>` in ONE Postgres transaction (version allocation
 * + append), FAIL-SOFT: this function NEVER throws — any failure
 * (connection, constraint, schema decode, redaction refusal) rolls back the
 * projection transaction and comes back as a typed diagnostic for the
 * caller to log.
 *
 * Dual-write contract (KS-6.6): the caller invokes this ALONGSIDE its
 * existing legacy `notifySyncScopes` poke; a projection failure must never
 * fail that call site's queued-run response.
 */
export const projectAgentRunBestEffort = async (
  sql: SyncSql,
  raw: unknown,
): Promise<AgentRunProjectionOutcome> => {
  // Decode + mapping + redaction are PURE (no I/O) — do them BEFORE opening
  // a transaction so a decode/redaction refusal never touches storage.
  let entity: AgentRunEntity
  let postImage: unknown
  try {
    entity = decodeAgentRunEntity(raw)
    postImage = encodeAgentRunEntity(entity)
    assertAgentRunPostImageRedacted(postImage)
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }

  try {
    const entry = await withSyncTransaction(sql, async (writer) =>
      writer.appendChange({
        entityId: EntityId.make(entity.runId),
        entityType: EntityType.make(AGENT_RUN_ENTITY_TYPE),
        mutationRef: AGENT_RUN_PROJECTION_SYSTEM_REF,
        op: "upsert",
        postImage,
        scope: agentRunScope(entity.runId),
      }),
    )
    return { entry, ok: true }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}

// ---------------------------------------------------------------------------
// Append event batch (companion multi-entity — KS-6.6 event-feed follow-up)
// ---------------------------------------------------------------------------

export const AGENT_RUN_EVENT_PROJECTION_SYSTEM_REF =
  "system:agent_run_event_projection.omni_handlers.v1"

export type AgentRunEventsProjectionOutcome =
  | { readonly ok: true; readonly entries: ReadonlyArray<ChangelogEntry> }
  | { readonly ok: false; readonly diagnostic: AgentRunProjectionDiagnostic }

/**
 * Project a BATCH of agent-run events (the individual tool-call/message
 * transcript entries the legacy `agent-run:<runId>` DO room multiplexes onto
 * `agent_run_events`) into the SAME `scope.agent_run.<runId>` scope as their
 * parent run entity, one `agent_run_event` changelog row per event, keyed by
 * the event's own id — ONE Postgres transaction covering the whole batch
 * (so a multi-event append advances the scope version exactly once, same as
 * the legacy room's single outbox append), FAIL-SOFT: never throws.
 *
 * All-or-nothing per call: decode/encode/redact every event PURELY first
 * (before opening a transaction), so a single malformed event in the batch
 * fails the whole batch rather than silently dropping events from the
 * ordered transcript — a caller that retries the same batch (or the next
 * append) recovers cleanly rather than leaving an undetectable gap.
 *
 * Dual-write contract (KS-6.6 follow-up): the caller invokes this ALONGSIDE
 * the existing legacy `appendAgentRunSyncChanges` (`omni-runs.ts`) write; a
 * projection failure must never fail the runner-event ingest response.
 */
export const projectAgentRunEventsBestEffort = async (
  sql: SyncSql,
  runId: string,
  rawEvents: ReadonlyArray<unknown>,
): Promise<AgentRunEventsProjectionOutcome> => {
  if (rawEvents.length === 0) {
    return { entries: [], ok: true }
  }

  const entities: Array<AgentRunEventEntity> = []
  const postImages: Array<unknown> = []
  try {
    for (const raw of rawEvents) {
      const entity = decodeAgentRunEventEntity(raw)
      const postImage = encodeAgentRunEventEntity(entity)
      assertAgentRunPostImageRedacted(postImage)
      entities.push(entity)
      postImages.push(postImage)
    }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }

  try {
    const entries = await withSyncTransaction(sql, async (writer) => {
      const written: Array<ChangelogEntry> = []
      for (const [index, entity] of entities.entries()) {
        written.push(
          await writer.appendChange({
            entityId: EntityId.make(entity.id),
            entityType: EntityType.make(AGENT_RUN_EVENT_ENTITY_TYPE),
            mutationRef: AGENT_RUN_EVENT_PROJECTION_SYSTEM_REF,
            op: "upsert",
            postImage: postImages[index],
            scope: agentRunScope(runId),
          }),
        )
      }
      return written
    })
    return { entries, ok: true }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}
