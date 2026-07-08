import {
  decodeKhalaRuntimeControlIntent,
  decodeKhalaRuntimeEvent,
  decodeRuntimeControlIntentEntity,
  decodeRuntimeEventEntity,
  decodeRuntimeTurnEntity,
  EntityId,
  EntityType,
  MutationResult,
  MutatorName,
  personalScope,
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
  threadScope,
  type KhalaRuntimeControlIntent,
  type KhalaRuntimeControlIntentKind,
  type KhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type RuntimeControlIntentEntity,
  type RuntimeControlIntentStatus,
  type RuntimeEventEntity,
  type RuntimeTurnEntity,
  type RuntimeTurnStatus,
} from "@openagentsinc/khala-sync"
import { ensureScopeOwner } from "./fleet-projection.js"
import type { MutatorContext, MutatorDefinition } from "./push-engine.js"
import { defineMutator } from "./push-engine.js"

/**
 * Khala Code runtime mutators (#8370).
 *
 * These mutators give mobile/desktop/server surfaces an AI SDK-shaped,
 * server-authoritative control lane without exposing private runtime
 * material outside the owner/thread scopes:
 *
 * - `runtime.startTurn` records a body-free `turn.start` intent and creates
 *   a queued turn.
 * - `runtime.appendUserMessage` records a body-free `message.append`
 *   control intent, optionally tied to an existing turn.
 * - `runtime.interruptTurn`, `runtime.continueTurn`, `runtime.retryTurn`,
 *   and `runtime.closeTurn` advance existing owner-private turns.
 * - `runtime.recordEvent` records full runtime stream events only in
 *   `scope.thread.<threadId>` and updates the public-safe turn summary.
 *
 * Every mutator is in-band-rejecting and ledger-idempotent via
 * `executePush`. Runtime events can carry raw text/tool deltas, so the only
 * replicated event scope is the exact private thread scope.
 */

export const RUNTIME_START_TURN_MUTATOR_NAME = "runtime.startTurn"
export const RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME =
  "runtime.appendUserMessage"
export const RUNTIME_INTERRUPT_TURN_MUTATOR_NAME = "runtime.interruptTurn"
export const RUNTIME_CONTINUE_TURN_MUTATOR_NAME = "runtime.continueTurn"
export const RUNTIME_RETRY_TURN_MUTATOR_NAME = "runtime.retryTurn"
export const RUNTIME_CLOSE_TURN_MUTATOR_NAME = "runtime.closeTurn"
export const RUNTIME_RECORD_EVENT_MUTATOR_NAME = "runtime.recordEvent"

export const RUNTIME_SCOPE_REJECTION = "unauthorized_scope"
export const RUNTIME_INTENT_KIND_REJECTION = "runtime_intent_kind_mismatch"
export const RUNTIME_TURN_REQUIRED_REJECTION = "runtime_turn_required"
export const RUNTIME_TURN_EXISTS_REJECTION = "runtime_turn_exists"
export const RUNTIME_TURN_NOT_FOUND_REJECTION = "runtime_turn_not_found"
export const RUNTIME_INTENT_EXISTS_REJECTION = "runtime_intent_exists"
export const RUNTIME_MESSAGE_REQUIRED_REJECTION = "runtime_message_required"
export const RUNTIME_EVENT_EXISTS_REJECTION = "runtime_event_exists"
export const RUNTIME_EVENT_SEQUENCE_REJECTION = "runtime_event_sequence_invalid"
export const RUNTIME_RAW_BODY_REJECTION = "runtime_raw_body_not_allowed"

const RuntimeTurnEntityType = EntityType.make(RUNTIME_TURN_ENTITY_TYPE)
const RuntimeControlIntentEntityType = EntityType.make(
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
)
const RuntimeEventEntityType = EntityType.make(RUNTIME_EVENT_ENTITY_TYPE)

export const decodeRuntimeControlIntentArgs = (
  argsJson: string,
): KhalaRuntimeControlIntent =>
  decodeKhalaRuntimeControlIntent(JSON.parse(argsJson) as unknown)

export const decodeRuntimeEventArgs = (argsJson: string): KhalaRuntimeEvent =>
  decodeKhalaRuntimeEvent(JSON.parse(argsJson) as unknown)

type RuntimeTurnRow = Readonly<{
  turn_id: string
  thread_id: string
  owner_user_id: string
  lane: string
  status: string
  event_count: string | number
  latest_intent_id: string | null
  started_at: string | null
  settled_at: string | null
  created_at: string
  updated_at: string
}>

type RuntimeControlIntentConflictRow = Readonly<{
  intent_id: string
  owner_user_id: string
  idempotency_key: string
}>

type RuntimeEventConflictRow = Readonly<{
  event_id: string
}>

const transactionNowIso = async (ctx: MutatorContext): Promise<string> => {
  const rows: Array<{ now: Date | string }> = await ctx.writer.sql`
    SELECT now() AS now
  `
  const raw = rows[0]?.now
  if (raw === undefined) throw new Error("SELECT now() returned no row")
  return raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()
}

const reject = (
  ctx: MutatorContext,
  errorCode: string,
  errorMessageSafe: string,
): MutationResult =>
  new MutationResult({
    errorCode,
    errorMessageSafe,
    mutationId: ctx.mutationId,
    status: "rejected",
  })

const rejectForeignScope = (ctx: MutatorContext): MutationResult =>
  reject(
    ctx,
    RUNTIME_SCOPE_REJECTION,
    "this runtime thread scope belongs to a different user",
  )

const applied = (ctx: MutatorContext): MutationResult =>
  new MutationResult({ mutationId: ctx.mutationId, status: "applied" })

const ensureRuntimeThreadOwner = async (
  ctx: MutatorContext,
  threadId: string,
): Promise<MutationResult | null> => {
  const owner = await ensureScopeOwner(ctx.writer.sql, threadScope(threadId), ctx.userId)
  return owner === ctx.userId ? null : rejectForeignScope(ctx)
}

const readTurnForUpdate = async (
  ctx: MutatorContext,
  turnId: string,
): Promise<RuntimeTurnRow | null> => {
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    SELECT turn_id, thread_id, owner_user_id, lane, status, event_count,
           latest_intent_id, started_at, settled_at, created_at, updated_at
    FROM khala_sync_runtime_turns
    WHERE turn_id = ${turnId}
    FOR UPDATE
  `
  return rows[0] ?? null
}

const readControlIntentConflict = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
): Promise<RuntimeControlIntentConflictRow | null> => {
  const rows: Array<RuntimeControlIntentConflictRow> = await ctx.writer.sql`
    SELECT intent_id, owner_user_id, idempotency_key
    FROM khala_sync_runtime_control_intents
    WHERE intent_id = ${intent.intentId}
       OR (owner_user_id = ${ctx.userId}
           AND idempotency_key = ${intent.idempotencyKey})
    LIMIT 1
  `
  return rows[0] ?? null
}

const readRuntimeEventConflict = async (
  ctx: MutatorContext,
  event: KhalaRuntimeEvent,
): Promise<RuntimeEventConflictRow | null> => {
  const rows: Array<RuntimeEventConflictRow> = await ctx.writer.sql`
    SELECT event_id
    FROM khala_sync_runtime_events
    WHERE event_id = ${event.eventId}
       OR (turn_id = ${event.turnId} AND sequence = ${event.sequence})
    LIMIT 1
  `
  return rows[0] ?? null
}

const turnEntityFromRow = (row: RuntimeTurnRow): RuntimeTurnEntity =>
  decodeRuntimeTurnEntity({
    createdAt: row.created_at,
    eventCount: Number(row.event_count),
    latestIntentId: row.latest_intent_id,
    lane: row.lane,
    ownerUserId: row.owner_user_id,
    settledAt: row.settled_at,
    startedAt: row.started_at,
    status: row.status,
    threadId: row.thread_id,
    turnId: row.turn_id,
    updatedAt: row.updated_at,
  })

const controlIntentEntityFromIntent = (
  intent: KhalaRuntimeControlIntent,
  input: {
    readonly ownerUserId: string
    readonly status: RuntimeControlIntentStatus
    readonly nowIso: string
  },
): RuntimeControlIntentEntity =>
  decodeRuntimeControlIntentEntity({
    createdAt: input.nowIso,
    intent,
    intentId: intent.intentId,
    kind: intent.kind,
    ownerUserId: input.ownerUserId,
    status: input.status,
    threadId: intent.threadId,
    turnId: intent.turnId ?? null,
    updatedAt: input.nowIso,
  })

const runtimeEventEntityFromEvent = (
  event: KhalaRuntimeEvent,
  input: {
    readonly ownerUserId: string
    readonly nowIso: string
  },
): RuntimeEventEntity =>
  decodeRuntimeEventEntity({
    createdAt: input.nowIso,
    event,
    eventId: event.eventId,
    kind: event.kind,
    observedAt: event.observedAt,
    ownerUserId: input.ownerUserId,
    sequence: event.sequence,
    threadId: event.threadId,
    turnId: event.turnId,
  })

const appendTurnEntityChanges = async (
  ctx: MutatorContext,
  entity: RuntimeTurnEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.turnId),
    entityType: RuntimeTurnEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: personalScope(entity.ownerUserId),
  })
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.turnId),
    entityType: RuntimeTurnEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const appendControlIntentEntityChanges = async (
  ctx: MutatorContext,
  entity: RuntimeControlIntentEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.intentId),
    entityType: RuntimeControlIntentEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: personalScope(entity.ownerUserId),
  })
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.intentId),
    entityType: RuntimeControlIntentEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const appendRuntimeEventEntityChange = async (
  ctx: MutatorContext,
  entity: RuntimeEventEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.eventId),
    entityType: RuntimeEventEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const validateControlIntentBasics = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  expectedKind: KhalaRuntimeControlIntentKind,
): Promise<MutationResult | null> => {
  if (intent.kind !== expectedKind) {
    return reject(
      ctx,
      RUNTIME_INTENT_KIND_REJECTION,
      "runtime control intent kind does not match the mutator",
    )
  }
  if (intent.body !== undefined) {
    return reject(
      ctx,
      RUNTIME_RAW_BODY_REJECTION,
      "runtime control intents must carry bodyRef or promptRef, not raw body",
    )
  }
  const conflict = await readControlIntentConflict(ctx, intent)
  if (conflict !== null) {
    return reject(
      ctx,
      RUNTIME_INTENT_EXISTS_REJECTION,
      "this runtime control intent was already recorded",
    )
  }
  return null
}

const requireTurnId = (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
): string | MutationResult => {
  if (intent.turnId === undefined) {
    return reject(
      ctx,
      RUNTIME_TURN_REQUIRED_REJECTION,
      "this runtime control intent requires a turn id",
    )
  }
  return intent.turnId
}

const requireMessageId = (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
): string | MutationResult => {
  if (intent.messageId === undefined) {
    return reject(
      ctx,
      RUNTIME_MESSAGE_REQUIRED_REJECTION,
      "append user message requires a message id",
    )
  }
  return intent.messageId
}

const isMutationResult = (value: string | MutationResult): value is MutationResult =>
  value instanceof MutationResult

const insertControlIntent = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  status: RuntimeControlIntentStatus,
  nowIso: string,
): Promise<RuntimeControlIntentEntity> => {
  const entity = controlIntentEntityFromIntent(intent, {
    nowIso,
    ownerUserId: ctx.userId,
    status,
  })
  // `intent_json` is jsonb: bind the OBJECT, never a pre-stringified string.
  // Both drivers (Bun's `SQL` in tests, postgres.js over Hyperdrive in the
  // Worker) serialize a JS object to jsonb exactly once. Passing an
  // already-serialized string (e.g. `canonicalJson(...)`) makes the driver
  // JSON-encode it AGAIN, storing a jsonb string SCALAR
  // (`"{\"bodyRef\":...}"`, `jsonb_typeof = 'string'`) instead of an object,
  // so `intent_json->>'bodyRef'` is NULL. That double-encoding broke hosted
  // chat turn resolution; the readers stay defensive to both encodings.
  await ctx.writer.sql`
    INSERT INTO khala_sync_runtime_control_intents
      (intent_id, thread_id, turn_id, owner_user_id, kind, status,
       idempotency_key, intent_json, created_at, updated_at)
    VALUES
      (${entity.intentId}, ${entity.threadId}, ${entity.turnId},
       ${entity.ownerUserId}, ${entity.kind}, ${entity.status},
       ${intent.idempotencyKey}, ${entity.intent}::jsonb,
       ${entity.createdAt}, ${entity.updatedAt})
  `
  await appendControlIntentEntityChanges(ctx, entity)
  return entity
}

const insertTurn = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  nowIso: string,
): Promise<RuntimeTurnEntity> => {
  const turnId = intent.turnId
  if (turnId === undefined) {
    throw new Error("insertTurn requires a turn id after validation")
  }
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    INSERT INTO khala_sync_runtime_turns
      (turn_id, thread_id, owner_user_id, lane, status, event_count,
       latest_intent_id, started_at, settled_at, created_at, updated_at)
    VALUES
      (${turnId}, ${intent.threadId}, ${ctx.userId}, ${intent.target.lane},
       'queued', 0, ${intent.intentId}, ${null}, ${null}, ${nowIso}, ${nowIso})
    RETURNING turn_id, thread_id, owner_user_id, lane, status, event_count,
              latest_intent_id, started_at, settled_at, created_at, updated_at
  `
  const row = rows[0]
  if (row === undefined) {
    throw new Error("runtime turn insert returned no row")
  }
  const entity = turnEntityFromRow(row)
  await appendTurnEntityChanges(ctx, entity)
  return entity
}

const updateTurnForIntent = async (
  ctx: MutatorContext,
  input: {
    readonly turnId: string
    readonly status: RuntimeTurnStatus
    readonly latestIntentId: string
    readonly settledAt: string | null
    readonly nowIso: string
  },
): Promise<RuntimeTurnEntity> => {
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    UPDATE khala_sync_runtime_turns
    SET status = ${input.status},
        latest_intent_id = ${input.latestIntentId},
        settled_at = ${input.settledAt},
        updated_at = ${input.nowIso}
    WHERE turn_id = ${input.turnId}
    RETURNING turn_id, thread_id, owner_user_id, lane, status, event_count,
              latest_intent_id, started_at, settled_at, created_at, updated_at
  `
  const row = rows[0]
  if (row === undefined) {
    throw new Error("runtime turn disappeared during control update")
  }
  const entity = turnEntityFromRow(row)
  await appendTurnEntityChanges(ctx, entity)
  return entity
}

const validateExistingTurnIntent = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  expectedKind: KhalaRuntimeControlIntentKind,
): Promise<
  | { readonly kind: "ok"; readonly turn: RuntimeTurnRow; readonly turnId: string }
  | { readonly kind: "rejected"; readonly result: MutationResult }
> => {
  const basics = await validateControlIntentBasics(ctx, intent, expectedKind)
  if (basics !== null) return { kind: "rejected", result: basics }

  const turnId = requireTurnId(ctx, intent)
  if (isMutationResult(turnId)) return { kind: "rejected", result: turnId }

  const turn = await readTurnForUpdate(ctx, turnId)
  if (turn === null) {
    return {
      kind: "rejected",
      result: reject(
        ctx,
        RUNTIME_TURN_NOT_FOUND_REJECTION,
        "this runtime turn does not exist",
      ),
    }
  }
  if (turn.owner_user_id !== ctx.userId || turn.thread_id !== intent.threadId) {
    return { kind: "rejected", result: rejectForeignScope(ctx) }
  }

  const ownerRejection = await ensureRuntimeThreadOwner(ctx, intent.threadId)
  if (ownerRejection !== null) {
    return { kind: "rejected", result: ownerRejection }
  }

  return { kind: "ok", turn, turnId }
}

const executeExistingTurnIntent = async (
  intent: KhalaRuntimeControlIntent,
  ctx: MutatorContext,
  input: {
    readonly expectedKind: KhalaRuntimeControlIntentKind
    readonly status: RuntimeTurnStatus
    readonly controlStatus?: RuntimeControlIntentStatus | undefined
    readonly settled: boolean
  },
): Promise<MutationResult> => {
  const validated = await validateExistingTurnIntent(
    ctx,
    intent,
    input.expectedKind,
  )
  if (validated.kind === "rejected") return validated.result

  const nowIso = await transactionNowIso(ctx)
  await insertControlIntent(
    ctx,
    intent,
    input.controlStatus ?? "accepted",
    nowIso,
  )
  await updateTurnForIntent(ctx, {
    latestIntentId: intent.intentId,
    nowIso,
    settledAt: input.settled ? nowIso : null,
    status: input.status,
    turnId: validated.turnId,
  })
  return applied(ctx)
}

export const runtimeStartTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: async (intent, ctx) => {
      const basics = await validateControlIntentBasics(ctx, intent, "turn.start")
      if (basics !== null) return basics

      const turnId = requireTurnId(ctx, intent)
      if (isMutationResult(turnId)) return turnId

      const existing = await readTurnForUpdate(ctx, turnId)
      if (existing !== null) {
        return existing.owner_user_id === ctx.userId
          ? reject(
              ctx,
              RUNTIME_TURN_EXISTS_REJECTION,
              "this runtime turn already exists",
            )
          : rejectForeignScope(ctx)
      }

      const ownerRejection = await ensureRuntimeThreadOwner(ctx, intent.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      await insertControlIntent(ctx, intent, "accepted", nowIso)
      await insertTurn(ctx, intent, nowIso)
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_START_TURN_MUTATOR_NAME),
  })

export const runtimeAppendUserMessageMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: async (intent, ctx) => {
      const basics = await validateControlIntentBasics(
        ctx,
        intent,
        "message.append",
      )
      if (basics !== null) return basics

      const messageId = requireMessageId(ctx, intent)
      if (isMutationResult(messageId)) return messageId

      const turnId = intent.turnId
      let turn: RuntimeTurnRow | null = null
      if (turnId !== undefined) {
        turn = await readTurnForUpdate(ctx, turnId)
        if (turn === null) {
          return reject(
            ctx,
            RUNTIME_TURN_NOT_FOUND_REJECTION,
            "this runtime turn does not exist",
          )
        }
        if (turn.owner_user_id !== ctx.userId || turn.thread_id !== intent.threadId) {
          return rejectForeignScope(ctx)
        }
      }

      const ownerRejection = await ensureRuntimeThreadOwner(ctx, intent.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      await insertControlIntent(ctx, intent, "accepted", nowIso)
      if (turn !== null && turnId !== undefined) {
        await updateTurnForIntent(ctx, {
          latestIntentId: intent.intentId,
          nowIso,
          settledAt: turn.settled_at,
          status: turn.status as RuntimeTurnStatus,
          turnId,
        })
      }
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME),
  })

export const runtimeInterruptTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        expectedKind: "turn.interrupt",
        settled: true,
        status: "interrupted",
      }),
    name: MutatorName.make(RUNTIME_INTERRUPT_TURN_MUTATOR_NAME),
  })

export const runtimeContinueTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        expectedKind: "turn.continue",
        settled: false,
        status: "queued",
      }),
    name: MutatorName.make(RUNTIME_CONTINUE_TURN_MUTATOR_NAME),
  })

export const runtimeRetryTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        expectedKind: "turn.retry",
        settled: false,
        status: "queued",
      }),
    name: MutatorName.make(RUNTIME_RETRY_TURN_MUTATOR_NAME),
  })

export const runtimeCloseTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        controlStatus: "settled",
        expectedKind: "turn.close",
        settled: true,
        status: "closed",
      }),
    name: MutatorName.make(RUNTIME_CLOSE_TURN_MUTATOR_NAME),
  })

const statusForRuntimeEvent = (
  event: KhalaRuntimeEvent,
  current: RuntimeTurnStatus,
): RuntimeTurnStatus => {
  switch (event.kind) {
    case "turn.started":
      return "running"
    case "turn.interrupted":
      return "interrupted"
    case "turn.finished":
      return turnFinishedStatus(event.finishReason)
    default:
      return current
  }
}

const turnFinishedStatus = (
  finishReason: KhalaRuntimeFinishReason,
): RuntimeTurnStatus => {
  switch (finishReason) {
    case "error":
      return "failed"
    case "cancelled":
    case "interrupted":
      return "interrupted"
    default:
      return "completed"
  }
}

const updateTurnForRuntimeEvent = async (
  ctx: MutatorContext,
  turn: RuntimeTurnRow,
  event: KhalaRuntimeEvent,
  nowIso: string,
): Promise<RuntimeTurnEntity> => {
  const status = statusForRuntimeEvent(event, turn.status as RuntimeTurnStatus)
  const startedAt =
    event.kind === "turn.started" ? (turn.started_at ?? nowIso) : turn.started_at
  const settledAt =
    event.kind === "turn.finished" || event.kind === "turn.interrupted"
      ? nowIso
      : turn.settled_at
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    UPDATE khala_sync_runtime_turns
    SET status = ${status},
        event_count = event_count + 1,
        started_at = ${startedAt},
        settled_at = ${settledAt},
        updated_at = ${nowIso}
    WHERE turn_id = ${turn.turn_id}
    RETURNING turn_id, thread_id, owner_user_id, lane, status, event_count,
              latest_intent_id, started_at, settled_at, created_at, updated_at
  `
  const row = rows[0]
  if (row === undefined) {
    throw new Error("runtime turn disappeared during event update")
  }
  const entity = turnEntityFromRow(row)
  await appendTurnEntityChanges(ctx, entity)
  return entity
}

export const runtimeRecordEventMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeEvent>({
    decodeArgs: decodeRuntimeEventArgs,
    execute: async (event, ctx) => {
      if (!Number.isSafeInteger(event.sequence) || event.sequence < 0) {
        return reject(
          ctx,
          RUNTIME_EVENT_SEQUENCE_REJECTION,
          "runtime event sequence must be a non-negative safe integer",
        )
      }

      const conflict = await readRuntimeEventConflict(ctx, event)
      if (conflict !== null) {
        return reject(
          ctx,
          RUNTIME_EVENT_EXISTS_REJECTION,
          "this runtime event was already recorded",
        )
      }

      const turn = await readTurnForUpdate(ctx, event.turnId)
      if (turn === null) {
        return reject(
          ctx,
          RUNTIME_TURN_NOT_FOUND_REJECTION,
          "this runtime turn does not exist",
        )
      }
      if (turn.owner_user_id !== ctx.userId || turn.thread_id !== event.threadId) {
        return rejectForeignScope(ctx)
      }

      const ownerRejection = await ensureRuntimeThreadOwner(ctx, event.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      const eventEntity = runtimeEventEntityFromEvent(event, {
        nowIso,
        ownerUserId: ctx.userId,
      })
      // `event_json` is jsonb: bind the OBJECT (see `insertControlIntent`);
      // a pre-stringified string would be double-encoded into a jsonb string
      // scalar.
      await ctx.writer.sql`
        INSERT INTO khala_sync_runtime_events
          (event_id, turn_id, thread_id, owner_user_id, kind, sequence,
           observed_at, event_json, created_at)
        VALUES
          (${eventEntity.eventId}, ${eventEntity.turnId}, ${eventEntity.threadId},
           ${eventEntity.ownerUserId}, ${eventEntity.kind}, ${eventEntity.sequence},
           ${eventEntity.observedAt}, ${eventEntity.event}::jsonb,
           ${eventEntity.createdAt})
      `
      await appendRuntimeEventEntityChange(ctx, eventEntity)
      await updateTurnForRuntimeEvent(ctx, turn, event, nowIso)
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_RECORD_EVENT_MUTATOR_NAME),
  })

export const runtimeMutators: ReadonlyArray<MutatorDefinition> = [
  runtimeStartTurnMutator,
  runtimeAppendUserMessageMutator,
  runtimeInterruptTurnMutator,
  runtimeContinueTurnMutator,
  runtimeRetryTurnMutator,
  runtimeCloseTurnMutator,
  runtimeRecordEventMutator,
]
