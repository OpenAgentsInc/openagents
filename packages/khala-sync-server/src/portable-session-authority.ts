import {
  EntityId,
  EntityType,
  MutationResult,
  MutatorName,
  personalScope,
  canonicalJson,
} from "@openagentsinc/khala-sync"
import {
  PORTABLE_COMMAND_SCHEMA_VERSION,
  PORTABLE_SESSION_SCHEMA_VERSION,
  PortableAttachmentSchema,
  PortableCheckpointSchema,
  PortableCodingSessionSchema,
  PortableSessionCommandOutcomeSchema,
  PortableSessionCommandSchema,
  PortableTargetDescriptorSchema,
  auditPortableSessionSnapshot,
  type PortableAttachment,
  type PortableCheckpoint,
  type PortableSessionCommand,
  type PortableSessionCommandOutcome,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract"
import { Schema as S } from "effect"

import { defineMutator, type MutatorDefinition } from "./push-engine.js"
import type { SyncTransactionWriter } from "./outbox-writer.js"
import type { SqlTag } from "./sql.js"

export const PORTABLE_REGISTER_SESSION_MUTATOR_NAME =
  "portable.registerSession"
export const PORTABLE_REQUEST_COMMAND_MUTATOR_NAME =
  "portable.requestCommand"

export const PORTABLE_SESSION_ENTITY_TYPE = "portable_session"
export const PORTABLE_AGENT_GRAPH_ENTITY_TYPE = "portable_agent_graph"
export const PORTABLE_ATTACHMENT_ENTITY_TYPE = "portable_attachment"
export const PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE = "portable_target_directory"
export const PORTABLE_THREAD_CURRENT_ENTITY_TYPE = "portable_thread_current"
export const PORTABLE_COMMAND_ENTITY_TYPE = "portable_command"

const forbiddenPrivateMaterial =
  /"(?:token|apiKey|authorization|sessionToken|refreshToken|mnemonic|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)/i

const decodeAttachment = S.decodeUnknownSync(PortableAttachmentSchema)
const decodeCommand = S.decodeUnknownSync(PortableSessionCommandSchema)
const decodeCheckpoint = S.decodeUnknownSync(PortableCheckpointSchema)
const decodeOutcome = S.decodeUnknownSync(PortableSessionCommandOutcomeSchema)

const boundedTargets = S.Array(PortableTargetDescriptorSchema).check(S.isMaxLength(64))
const RegisterPortableSessionArgsSchema = S.Struct({
  session: PortableCodingSessionSchema,
  targets: boundedTargets,
  attachment: PortableAttachmentSchema,
})
type RegisterPortableSessionArgs = typeof RegisterPortableSessionArgsSchema.Type

const PortableEventKind = S.Literals([
  "agent_lifecycle",
  "activity_cursor",
  "command_outcome",
  "checkpoint_sealed",
  "attachment_transition",
  "projection_repaired",
])

const PortableEventArgsSchema = S.Struct({
  eventRef: S.String.check(S.isMinLength(3), S.isMaxLength(256)),
  sessionRef: S.String.check(S.isMinLength(3), S.isMaxLength(256)),
  threadRef: S.String.check(S.isMinLength(3), S.isMaxLength(256)),
  threadCursor: S.Int.check(S.isGreaterThan(0)),
  attachmentRef: S.String.check(S.isMinLength(3), S.isMaxLength(256)),
  attachmentGeneration: S.Int.check(S.isGreaterThan(0)),
  eventKind: PortableEventKind,
  current: S.Record(S.String, S.Union([S.String, S.Number, S.Boolean, S.Null])),
})
export type PortableSessionEventInput = typeof PortableEventArgsSchema.Type

export class PortableSessionAuthorityError extends Error {
  readonly _tag = "PortableSessionAuthorityError"
  override readonly name = "PortableSessionAuthorityError"
  constructor(
    readonly code:
      | "invalid"
      | "unauthorized"
      | "conflict"
      | "stale_generation"
      | "cursor_gap"
      | "not_found"
      | "target_unavailable"
      | "expired",
    message: string,
  ) {
    super(message)
  }
}

const assertPublicSafe = (value: unknown): void => {
  if (forbiddenPrivateMaterial.test(canonicalJson(value))) {
    throw new PortableSessionAuthorityError(
      "invalid",
      "portable session payload contains forbidden private material",
    )
  }
}

const parseJson = (raw: unknown): unknown =>
  typeof raw === "string" ? JSON.parse(raw) : raw

const appendProjection = (
  writer: SyncTransactionWriter,
  ownerUserId: string,
  entityType: string,
  entityId: string,
  postImage: unknown,
  mutationRef: string,
) => writer.appendChange({
  scope: personalScope(ownerUserId),
  entityType: EntityType.make(entityType),
  entityId: EntityId.make(entityId),
  op: "upsert",
  postImage,
  mutationRef,
})

export const decodePortableRegisterSessionArgs = (argsJson: string): RegisterPortableSessionArgs => {
  const raw = JSON.parse(argsJson) as unknown
  assertPublicSafe(raw)
  const value = S.decodeUnknownSync(RegisterPortableSessionArgsSchema)(raw)
  const violations = auditPortableSessionSnapshot({
    session: value.session,
    targets: value.targets,
    attachments: [value.attachment],
    checkpoints: [],
    leases: [],
    pendingCommands: [],
    topLevelCatalogSessionRefs: [value.session.graph.nodes.find(
      node => node.agentRef === value.session.graph.rootAgentRef,
    )?.threadRef ?? "missing.root"],
  })
  if (violations.length > 0) {
    throw new PortableSessionAuthorityError(
      "invalid",
      `portable session invariant failed: ${violations[0]!.code}`,
    )
  }
  if (value.session.schema !== PORTABLE_SESSION_SCHEMA_VERSION ||
      value.attachment.sessionRef !== value.session.sessionRef ||
      value.attachment.generation <= 0 || value.attachment.state !== "active") {
    throw new PortableSessionAuthorityError("invalid", "initial attachment is invalid")
  }
  return value
}

type ExistingTargetRow = {
  owner_user_id: string
  target_class: string
  adapter_ref: string
  compatibility_ref: string
  isolation: string
  data_posture: string
}

const upsertTarget = async (
  sql: SqlTag,
  target: PortableTargetDescriptor,
  ownerUserId: string,
): Promise<void> => {
  if (target.ownerRef !== ownerUserId) {
    throw new PortableSessionAuthorityError("unauthorized", "target owner mismatch")
  }
  const existing: ExistingTargetRow[] = await sql`
    SELECT owner_user_id, target_class, adapter_ref, compatibility_ref,
           isolation, data_posture
    FROM khala_sync_portable_targets
    WHERE target_ref = ${target.targetRef}
    FOR UPDATE
  `
  const row = existing[0]
  if (row !== undefined && (
    row.owner_user_id !== ownerUserId ||
    row.target_class !== target.targetClass ||
    row.adapter_ref !== target.adapterRef ||
    row.compatibility_ref !== target.compatibilityRef ||
    row.isolation !== target.isolation ||
    row.data_posture !== target.dataPosture
  )) {
    throw new PortableSessionAuthorityError(
      "conflict",
      "target identity or policy cannot be silently replaced",
    )
  }
  await sql`
    INSERT INTO khala_sync_portable_targets
      (target_ref, owner_user_id, target_class, adapter_ref,
       compatibility_ref, isolation, data_posture, health, updated_at)
    VALUES
      (${target.targetRef}, ${ownerUserId}, ${target.targetClass},
       ${target.adapterRef}, ${target.compatibilityRef}, ${target.isolation},
       ${target.dataPosture}, ${target.health}, now())
    ON CONFLICT (target_ref) DO UPDATE SET
      health = EXCLUDED.health,
      updated_at = now()
  `
}

export const registerPortableSession = async (
  writer: SyncTransactionWriter,
  args: RegisterPortableSessionArgs,
  ownerUserId: string,
  mutationRef: string,
): Promise<void> => {
  assertPublicSafe(args)
  if (args.session.ownerRef !== ownerUserId) {
    throw new PortableSessionAuthorityError("unauthorized", "session owner mismatch")
  }
  for (const target of args.targets) {
    await upsertTarget(writer.sql, target, ownerUserId)
  }
  if (!args.targets.some(target => target.targetRef === args.attachment.targetRef)) {
    throw new PortableSessionAuthorityError("invalid", "initial target is not authorized")
  }
  const existing: Array<{ owner_user_id: string }> = await writer.sql`
    SELECT owner_user_id
    FROM khala_sync_portable_sessions
    WHERE session_ref = ${args.session.sessionRef}
    FOR UPDATE
  `
  if (existing[0] !== undefined) {
    throw new PortableSessionAuthorityError(
      existing[0].owner_user_id === ownerUserId ? "conflict" : "unauthorized",
      "portable session already exists",
    )
  }
  await writer.sql`
    INSERT INTO khala_sync_portable_sessions
      (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
       event_log_ref, current_projection_ref, command_scope_ref,
       root_agent_ref, adopted_from_local_history, adoption_receipt_ref,
       state, latest_event_cursor, current_attachment_ref,
       current_attachment_generation)
    VALUES
      (${args.session.sessionRef}, ${ownerUserId}, ${String(personalScope(ownerUserId))},
       ${args.session.workContextRef}, ${args.session.eventLogRef},
       ${args.session.currentProjectionRef}, ${args.session.commandScopeRef},
       ${args.session.graph.rootAgentRef}, ${args.session.adoptedFromLocalHistory},
       ${args.session.adoptionReceiptRef ?? null}, 'active', 0,
       ${args.attachment.attachmentRef}, ${args.attachment.generation})
  `
  for (const target of args.targets) {
    await writer.sql`
      INSERT INTO khala_sync_portable_session_targets (session_ref, target_ref)
      VALUES (${args.session.sessionRef}, ${target.targetRef})
    `
  }
  for (const node of args.session.graph.nodes) {
    await writer.sql`
      INSERT INTO khala_sync_portable_agent_nodes
        (session_ref, agent_ref, parent_agent_ref, thread_ref, transcript_ref,
         activity_cursor, lifecycle, attachment_generation)
      VALUES
        (${args.session.sessionRef}, ${node.agentRef},
         ${node.parentAgentRef ?? null}, ${node.threadRef}, ${node.transcriptRef},
         ${node.activityCursor}, ${node.lifecycle}, ${node.attachmentGeneration})
    `
  }
  await writer.sql`
    INSERT INTO khala_sync_portable_attachments
      (attachment_ref, session_ref, target_ref, generation, state,
       descendant_agent_refs_json, capability_lease_refs_json, checkpoint_ref,
       evidence_refs_json)
    VALUES
      (${args.attachment.attachmentRef}, ${args.attachment.sessionRef},
       ${args.attachment.targetRef}, ${args.attachment.generation},
       ${args.attachment.state}, ${canonicalJson(args.attachment.descendantAgentRefs)}::jsonb,
       ${canonicalJson(args.attachment.capabilityLeaseRefs)}::jsonb,
       ${args.attachment.checkpointRef ?? null},
       ${canonicalJson(args.attachment.evidenceRefs)}::jsonb)
  `
  await appendProjection(writer, ownerUserId, PORTABLE_SESSION_ENTITY_TYPE,
    args.session.sessionRef, args.session, mutationRef)
  await appendProjection(writer, ownerUserId, PORTABLE_AGENT_GRAPH_ENTITY_TYPE,
    args.session.sessionRef, args.session.graph, mutationRef)
  await appendProjection(writer, ownerUserId, PORTABLE_ATTACHMENT_ENTITY_TYPE,
    args.attachment.attachmentRef, args.attachment, mutationRef)
  await appendProjection(writer, ownerUserId, PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE,
    args.session.sessionRef, { sessionRef: args.session.sessionRef, targets: args.targets },
    mutationRef)
}

export const portableRegisterSessionMutator: MutatorDefinition =
  defineMutator<RegisterPortableSessionArgs>({
    name: MutatorName.make(PORTABLE_REGISTER_SESSION_MUTATOR_NAME),
    decodeArgs: decodePortableRegisterSessionArgs,
    execute: async (args, ctx) => {
      if (String(personalScope(ctx.userId)) !== `scope.user.${args.session.ownerRef}`) {
        return new MutationResult({
          mutationId: ctx.mutationId,
          status: "rejected",
          errorCode: "unauthorized_scope",
          errorMessageSafe: "portable session scope does not belong to the authenticated owner",
        })
      }
      try {
        await registerPortableSession(ctx.writer, args, ctx.userId, ctx.mutationRef)
      } catch (error) {
        if (error instanceof PortableSessionAuthorityError) {
          return new MutationResult({
            mutationId: ctx.mutationId,
            status: "rejected",
            errorCode: `portable_${error.code}`,
            errorMessageSafe: error.message,
          })
        }
        throw error
      }
      return new MutationResult({ mutationId: ctx.mutationId, status: "applied" })
    },
  })

export const decodePortableSessionCommandArgs = (argsJson: string): PortableSessionCommand => {
  const raw = JSON.parse(argsJson) as unknown
  assertPublicSafe(raw)
  return decodeCommand(raw)
}

type SessionAuthorityRow = {
  owner_user_id: string
  current_attachment_ref: string | null
  current_attachment_generation: number | string | bigint
}

export const requestPortableSessionCommand = async (
  writer: SyncTransactionWriter,
  command: PortableSessionCommand,
  ownerUserId: string,
  mutationRef: string,
): Promise<"accepted" | "duplicate"> => {
  assertPublicSafe(command)
  if (command.schema !== PORTABLE_COMMAND_SCHEMA_VERSION || command.ownerRef !== ownerUserId) {
    throw new PortableSessionAuthorityError("unauthorized", "command owner mismatch")
  }
  const sessions: SessionAuthorityRow[] = await writer.sql`
    SELECT owner_user_id, current_attachment_ref, current_attachment_generation
    FROM khala_sync_portable_sessions
    WHERE session_ref = ${command.sessionRef}
    FOR UPDATE
  `
  const session = sessions[0]
  if (session === undefined) throw new PortableSessionAuthorityError("not_found", "session not found")
  if (session.owner_user_id !== ownerUserId) {
    throw new PortableSessionAuthorityError("unauthorized", "session owner mismatch")
  }
  const existing: Array<{ command_json: unknown }> = await writer.sql`
    SELECT command_json
    FROM khala_sync_portable_commands
    WHERE command_ref = ${command.commandRef} OR idempotency_key = ${command.idempotencyKey}
    FOR UPDATE
  `
  if (existing.length > 0) {
    if (existing.length !== 1 ||
        canonicalJson(parseJson(existing[0]!.command_json)) !== canonicalJson(command)) {
      throw new PortableSessionAuthorityError("conflict", "command identity was reused with different bytes")
    }
    return "duplicate"
  }
  if (session.current_attachment_ref !== command.expectedAttachmentRef ||
      Number(session.current_attachment_generation) !== command.expectedGeneration) {
    throw new PortableSessionAuthorityError("stale_generation", "command source generation is stale")
  }
  if (Date.parse(command.expiresAt) <= Date.now()) {
    throw new PortableSessionAuthorityError("expired", "command expired before acceptance")
  }
  if (["move", "attach", "failback"].includes(command.kind)) {
    if (!command.destinationTargetRef) {
      throw new PortableSessionAuthorityError("invalid", "movement command requires destination target")
    }
    const targets: Array<{ health: string; owner_user_id: string }> = await writer.sql`
      SELECT health, owner_user_id
      FROM khala_sync_portable_targets
      WHERE target_ref = ${command.destinationTargetRef}
    `
    if (targets[0]?.owner_user_id !== ownerUserId || targets[0]?.health !== "ready") {
      throw new PortableSessionAuthorityError("target_unavailable", "destination target is not ready")
    }
  }
  await writer.sql`
    INSERT INTO khala_sync_portable_commands
      (command_ref, idempotency_key, owner_user_id, session_ref, kind,
       expected_attachment_ref, expected_generation, destination_target_ref,
       checkpoint_ref, expires_at, command_json, status)
    VALUES
      (${command.commandRef}, ${command.idempotencyKey}, ${ownerUserId},
       ${command.sessionRef}, ${command.kind}, ${command.expectedAttachmentRef},
       ${command.expectedGeneration}, ${command.destinationTargetRef ?? null},
       ${command.checkpointRef ?? null}, ${command.expiresAt},
       ${canonicalJson(command)}::jsonb, 'accepted')
  `
  await appendProjection(writer, ownerUserId, PORTABLE_COMMAND_ENTITY_TYPE,
    command.commandRef, { command, status: "accepted" }, mutationRef)
  return "accepted"
}

export const portableRequestCommandMutator: MutatorDefinition =
  defineMutator<PortableSessionCommand>({
    name: MutatorName.make(PORTABLE_REQUEST_COMMAND_MUTATOR_NAME),
    decodeArgs: decodePortableSessionCommandArgs,
    execute: async (command, ctx) => {
      try {
        await requestPortableSessionCommand(ctx.writer, command, ctx.userId, ctx.mutationRef)
      } catch (error) {
        if (error instanceof PortableSessionAuthorityError) {
          return new MutationResult({
            mutationId: ctx.mutationId,
            status: "rejected",
            errorCode: `portable_${error.code}`,
            errorMessageSafe: error.message,
          })
        }
        throw error
      }
      return new MutationResult({ mutationId: ctx.mutationId, status: "applied" })
    },
  })

export const portableSessionMutators: ReadonlyArray<MutatorDefinition> = [
  portableRegisterSessionMutator,
  portableRequestCommandMutator,
]

type EventAuthorityRow = SessionAuthorityRow & { latest_event_cursor: number | string | bigint }

export const appendPortableSessionEvent = async (
  writer: SyncTransactionWriter,
  raw: unknown,
  mutationRef: string,
): Promise<number> => {
  assertPublicSafe(raw)
  const event = S.decodeUnknownSync(PortableEventArgsSchema)(raw)
  const sessions: EventAuthorityRow[] = await writer.sql`
    SELECT owner_user_id, current_attachment_ref, current_attachment_generation,
           latest_event_cursor
    FROM khala_sync_portable_sessions
    WHERE session_ref = ${event.sessionRef}
    FOR UPDATE
  `
  const session = sessions[0]
  if (session === undefined) throw new PortableSessionAuthorityError("not_found", "session not found")
  if (session.current_attachment_ref !== event.attachmentRef ||
      Number(session.current_attachment_generation) !== event.attachmentGeneration) {
    throw new PortableSessionAuthorityError("stale_generation", "event source generation is stale")
  }
  const cursors: Array<{ latest_cursor: number | string | bigint | null }> = await writer.sql`
    SELECT max(thread_cursor) AS latest_cursor
    FROM khala_sync_portable_events
    WHERE session_ref = ${event.sessionRef} AND thread_ref = ${event.threadRef}
  `
  const latest = Number(cursors[0]?.latest_cursor ?? 0)
  if (event.threadCursor !== latest + 1) {
    throw new PortableSessionAuthorityError("cursor_gap", "event must be the durable next thread cursor")
  }
  const inserted: Array<{ event_seq: number | string | bigint }> = await writer.sql`
    INSERT INTO khala_sync_portable_events
      (session_ref, event_ref, thread_ref, thread_cursor, attachment_ref,
       attachment_generation, event_kind, event_json)
    VALUES
      (${event.sessionRef}, ${event.eventRef}, ${event.threadRef},
       ${event.threadCursor}, ${event.attachmentRef}, ${event.attachmentGeneration},
       ${event.eventKind}, ${canonicalJson(event.current)}::jsonb)
    RETURNING event_seq
  `
  const eventSeq = Number(inserted[0]!.event_seq)
  await writer.sql`
    INSERT INTO khala_sync_portable_thread_current
      (session_ref, thread_ref, latest_cursor, current_json,
       repaired_from_event_seq, updated_at)
    VALUES
      (${event.sessionRef}, ${event.threadRef}, ${event.threadCursor},
       ${canonicalJson(event.current)}::jsonb, ${eventSeq}, now())
    ON CONFLICT (session_ref, thread_ref) DO UPDATE SET
      latest_cursor = EXCLUDED.latest_cursor,
      current_json = EXCLUDED.current_json,
      repaired_from_event_seq = EXCLUDED.repaired_from_event_seq,
      updated_at = now()
  `
  await writer.sql`
    UPDATE khala_sync_portable_sessions
    SET latest_event_cursor = ${eventSeq}, updated_at = now()
    WHERE session_ref = ${event.sessionRef}
  `
  await appendProjection(writer, session.owner_user_id,
    PORTABLE_THREAD_CURRENT_ENTITY_TYPE, `${event.sessionRef}:${event.threadRef}`,
    { sessionRef: event.sessionRef, threadRef: event.threadRef,
      latestCursor: event.threadCursor, current: event.current,
      repairedFromEventSeq: eventSeq }, mutationRef)
  return eventSeq
}

type RawEventRow = {
  event_seq: number | string | bigint
  thread_ref: string
  thread_cursor: number | string | bigint
  event_json: unknown
}

export const repairPortableSessionCurrentProjection = async (
  writer: SyncTransactionWriter,
  sessionRef: string,
  ownerUserId: string,
  mutationRef: string,
): Promise<ReadonlyArray<{ threadRef: string; latestCursor: number; repairedFromEventSeq: number }>> => {
  const sessions: Array<{ owner_user_id: string }> = await writer.sql`
    SELECT owner_user_id FROM khala_sync_portable_sessions
    WHERE session_ref = ${sessionRef} FOR UPDATE
  `
  if (sessions[0]?.owner_user_id !== ownerUserId) {
    throw new PortableSessionAuthorityError(
      sessions[0] === undefined ? "not_found" : "unauthorized",
      "portable session is unavailable",
    )
  }
  const events: RawEventRow[] = await writer.sql`
    SELECT event_seq, thread_ref, thread_cursor, event_json
    FROM khala_sync_portable_events
    WHERE session_ref = ${sessionRef}
    ORDER BY event_seq ASC
  `
  const latest = new Map<string, RawEventRow>()
  for (const event of events) latest.set(event.thread_ref, event)
  await writer.sql`DELETE FROM khala_sync_portable_thread_current WHERE session_ref = ${sessionRef}`
  const repaired: Array<{ threadRef: string; latestCursor: number; repairedFromEventSeq: number }> = []
  for (const event of latest.values()) {
    await writer.sql`
      INSERT INTO khala_sync_portable_thread_current
        (session_ref, thread_ref, latest_cursor, current_json,
         repaired_from_event_seq, updated_at)
      VALUES
        (${sessionRef}, ${event.thread_ref}, ${Number(event.thread_cursor)},
         ${canonicalJson(parseJson(event.event_json))}::jsonb,
         ${Number(event.event_seq)}, now())
    `
    const row = {
      threadRef: event.thread_ref,
      latestCursor: Number(event.thread_cursor),
      repairedFromEventSeq: Number(event.event_seq),
    }
    repaired.push(row)
    await appendProjection(writer, ownerUserId,
      PORTABLE_THREAD_CURRENT_ENTITY_TYPE, `${sessionRef}:${event.thread_ref}`,
      { sessionRef, ...row, current: parseJson(event.event_json) }, mutationRef)
  }
  return repaired
}

type MoveCommandRow = {
  command_json: unknown
  outcome_json: unknown | null
  owner_user_id: string
  status: string
}

export const completePortableSessionMove = async (
  writer: SyncTransactionWriter,
  input: {
    commandRef: string
    checkpoint: PortableCheckpoint
    destinationAttachment: PortableAttachment
    outcome: PortableSessionCommandOutcome
  },
  mutationRef: string,
): Promise<"completed" | "duplicate"> => {
  assertPublicSafe(input)
  const checkpoint = decodeCheckpoint(input.checkpoint)
  const destination = decodeAttachment(input.destinationAttachment)
  const outcome = decodeOutcome(input.outcome)
  const commands: MoveCommandRow[] = await writer.sql`
    SELECT command_json, outcome_json, owner_user_id, status
    FROM khala_sync_portable_commands
    WHERE command_ref = ${input.commandRef}
    FOR UPDATE
  `
  const row = commands[0]
  if (row === undefined) throw new PortableSessionAuthorityError("not_found", "move command not found")
  const command = decodeCommand(parseJson(row.command_json))
  if (!["move", "attach", "failback"].includes(command.kind)) {
    throw new PortableSessionAuthorityError("conflict", "command cannot complete a move")
  }
  if (checkpoint.sessionRef !== command.sessionRef ||
      checkpoint.checkpointRef !== command.checkpointRef ||
      checkpoint.sourceAttachmentRef !== command.expectedAttachmentRef ||
      checkpoint.sourceGeneration !== command.expectedGeneration ||
      destination.sessionRef !== command.sessionRef ||
      destination.targetRef !== command.destinationTargetRef ||
      destination.generation !== command.expectedGeneration + 1 ||
      destination.state !== "active" ||
      destination.checkpointRef !== checkpoint.checkpointRef ||
      outcome.commandRef !== command.commandRef || outcome.status !== "completed") {
    throw new PortableSessionAuthorityError("invalid", "move completion refs do not match accepted command")
  }
  if (row.status === "completed") {
    const checkpointRows: Array<{ digest: string }> = await writer.sql`
      SELECT digest FROM khala_sync_portable_checkpoints
      WHERE checkpoint_ref = ${checkpoint.checkpointRef}
        AND session_ref = ${command.sessionRef}
    `
    const attachmentRows: Array<{ target_ref: string; generation: number | string | bigint }> = await writer.sql`
      SELECT target_ref, generation FROM khala_sync_portable_attachments
      WHERE attachment_ref = ${destination.attachmentRef}
        AND session_ref = ${command.sessionRef}
    `
    if (canonicalJson(parseJson(row.outcome_json)) === canonicalJson(outcome) &&
        checkpointRows[0]?.digest === checkpoint.digest &&
        attachmentRows[0]?.target_ref === destination.targetRef &&
        Number(attachmentRows[0]?.generation) === destination.generation) {
      return "duplicate"
    }
    throw new PortableSessionAuthorityError("conflict", "completed move replay differs from durable outcome")
  }
  if (row.status !== "accepted") {
    throw new PortableSessionAuthorityError("conflict", "command cannot complete a move")
  }
  const sessions: EventAuthorityRow[] = await writer.sql`
    SELECT owner_user_id, current_attachment_ref, current_attachment_generation,
           latest_event_cursor
    FROM khala_sync_portable_sessions
    WHERE session_ref = ${command.sessionRef}
    FOR UPDATE
  `
  const session = sessions[0]
  if (session?.current_attachment_ref !== command.expectedAttachmentRef ||
      Number(session.current_attachment_generation) !== command.expectedGeneration) {
    throw new PortableSessionAuthorityError("stale_generation", "source attachment is no longer authoritative")
  }
  if (checkpoint.eventLogCursor > Number(session.latest_event_cursor)) {
    throw new PortableSessionAuthorityError("cursor_gap", "checkpoint is ahead of durable event log")
  }
  const targets: Array<{ health: string; owner_user_id: string }> = await writer.sql`
    SELECT health, owner_user_id FROM khala_sync_portable_targets
    WHERE target_ref = ${destination.targetRef}
  `
  if (targets[0]?.owner_user_id !== row.owner_user_id || targets[0]?.health !== "ready") {
    throw new PortableSessionAuthorityError("target_unavailable", "destination target is not ready")
  }
  const nodes: Array<{ agent_ref: string }> = await writer.sql`
    SELECT agent_ref FROM khala_sync_portable_agent_nodes
    WHERE session_ref = ${command.sessionRef}
  `
  const expectedDescendants = nodes.map(value => value.agent_ref).sort()
  const suppliedDescendants = [...destination.descendantAgentRefs].sort()
  if (canonicalJson(expectedDescendants) !== canonicalJson(suppliedDescendants)) {
    throw new PortableSessionAuthorityError("invalid", "destination fence does not cover complete graph")
  }
  await writer.sql`
    INSERT INTO khala_sync_portable_checkpoints
      (checkpoint_ref, session_ref, source_attachment_ref, source_generation,
       digest, parent_checkpoint_ref, repository_ref, repository_revision_ref,
       repository_post_image_digest, diff_digest, event_log_cursor,
       catalog_generation_ref, graph_digest, approval_refs_json,
       artifact_refs_json, receipt_refs_json)
    VALUES
      (${checkpoint.checkpointRef}, ${checkpoint.sessionRef},
       ${checkpoint.sourceAttachmentRef}, ${checkpoint.sourceGeneration},
       ${checkpoint.digest}, ${checkpoint.parentCheckpointRef ?? null},
       ${checkpoint.repositoryRef}, ${checkpoint.repositoryRevisionRef},
       ${checkpoint.repositoryPostImageDigest}, ${checkpoint.diffDigest},
       ${checkpoint.eventLogCursor}, ${checkpoint.catalogGenerationRef},
       ${checkpoint.graphDigest}, ${canonicalJson(checkpoint.approvalRefs)}::jsonb,
       ${canonicalJson(checkpoint.artifactRefs)}::jsonb,
       ${canonicalJson(checkpoint.receiptRefs)}::jsonb)
  `
  await writer.sql`
    UPDATE khala_sync_portable_attachments
    SET state = 'detached', checkpoint_ref = ${checkpoint.checkpointRef}, updated_at = now()
    WHERE attachment_ref = ${command.expectedAttachmentRef}
      AND generation = ${command.expectedGeneration}
  `
  await writer.sql`
    INSERT INTO khala_sync_portable_attachments
      (attachment_ref, session_ref, target_ref, generation, state,
       descendant_agent_refs_json, capability_lease_refs_json, checkpoint_ref,
       evidence_refs_json)
    VALUES
      (${destination.attachmentRef}, ${destination.sessionRef}, ${destination.targetRef},
       ${destination.generation}, ${destination.state},
       ${canonicalJson(destination.descendantAgentRefs)}::jsonb,
       ${canonicalJson(destination.capabilityLeaseRefs)}::jsonb,
       ${checkpoint.checkpointRef}, ${canonicalJson(destination.evidenceRefs)}::jsonb)
  `
  await writer.sql`
    UPDATE khala_sync_portable_agent_nodes
    SET attachment_generation = ${destination.generation},
        lifecycle = CASE WHEN lifecycle IN ('completed', 'failed', 'canceled')
                         THEN lifecycle ELSE 'waiting' END
    WHERE session_ref = ${command.sessionRef}
  `
  await writer.sql`
    UPDATE khala_sync_portable_sessions
    SET current_attachment_ref = ${destination.attachmentRef},
        current_attachment_generation = ${destination.generation},
        state = 'active', updated_at = now()
    WHERE session_ref = ${command.sessionRef}
  `
  await writer.sql`
    UPDATE khala_sync_portable_commands
    SET status = 'completed', outcome_json = ${canonicalJson(outcome)}::jsonb,
        updated_at = now()
    WHERE command_ref = ${command.commandRef}
  `
  await appendProjection(writer, row.owner_user_id, PORTABLE_ATTACHMENT_ENTITY_TYPE,
    destination.attachmentRef, destination, mutationRef)
  await appendProjection(writer, row.owner_user_id, PORTABLE_COMMAND_ENTITY_TYPE,
    command.commandRef, { command, outcome }, mutationRef)
  return "completed"
}

export const recordPortableSessionCommandOutcome = async (
  writer: SyncTransactionWriter,
  rawOutcome: PortableSessionCommandOutcome,
  mutationRef: string,
): Promise<"recorded" | "duplicate"> => {
  assertPublicSafe(rawOutcome)
  const outcome = decodeOutcome(rawOutcome)
  const rows: MoveCommandRow[] = await writer.sql`
    SELECT command_json, outcome_json, owner_user_id, status
    FROM khala_sync_portable_commands
    WHERE command_ref = ${outcome.commandRef}
    FOR UPDATE
  `
  const row = rows[0]
  if (row === undefined) throw new PortableSessionAuthorityError("not_found", "command not found")
  const command = decodeCommand(parseJson(row.command_json))
  if (outcome.sessionRef !== command.sessionRef ||
      outcome.sourceAttachmentRef !== command.expectedAttachmentRef ||
      outcome.sourceGeneration !== command.expectedGeneration) {
    throw new PortableSessionAuthorityError("invalid", "command outcome refs do not match command")
  }
  if (row.outcome_json !== null) {
    if (canonicalJson(parseJson(row.outcome_json)) === canonicalJson(outcome)) return "duplicate"
    throw new PortableSessionAuthorityError("conflict", "command already has a different durable outcome")
  }
  await writer.sql`
    UPDATE khala_sync_portable_commands
    SET status = ${outcome.status}, outcome_json = ${canonicalJson(outcome)}::jsonb,
        updated_at = now()
    WHERE command_ref = ${command.commandRef}
  `
  await appendProjection(writer, row.owner_user_id, PORTABLE_COMMAND_ENTITY_TYPE,
    command.commandRef, { command, outcome }, mutationRef)
  return "recorded"
}

/** Owner-authorized retention purge; dependent graph/log/commands cascade atomically. */
export const purgePortableSessionAuthority = async (
  writer: SyncTransactionWriter,
  input: { sessionRef: string; ownerUserId: string },
  mutationRef: string,
): Promise<boolean> => {
  const attachments: Array<{ attachment_ref: string }> = await writer.sql`
    SELECT attachment_ref FROM khala_sync_portable_attachments
    WHERE session_ref = ${input.sessionRef}
  `
  const commands: Array<{ command_ref: string }> = await writer.sql`
    SELECT command_ref FROM khala_sync_portable_commands
    WHERE session_ref = ${input.sessionRef}
  `
  const current: Array<{ thread_ref: string }> = await writer.sql`
    SELECT thread_ref FROM khala_sync_portable_thread_current
    WHERE session_ref = ${input.sessionRef}
  `
  const rows: Array<{ session_ref: string }> = await writer.sql`
    DELETE FROM khala_sync_portable_sessions
    WHERE session_ref = ${input.sessionRef} AND owner_user_id = ${input.ownerUserId}
    RETURNING session_ref
  `
  if (rows.length !== 1) return false
  const tombstone = (entityType: string, entityId: string) => writer.appendChange({
    scope: personalScope(input.ownerUserId),
    entityType: EntityType.make(entityType),
    entityId: EntityId.make(entityId),
    op: "delete",
    mutationRef,
  })
  await tombstone(PORTABLE_SESSION_ENTITY_TYPE, input.sessionRef)
  await tombstone(PORTABLE_AGENT_GRAPH_ENTITY_TYPE, input.sessionRef)
  await tombstone(PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE, input.sessionRef)
  for (const row of attachments) await tombstone(PORTABLE_ATTACHMENT_ENTITY_TYPE, row.attachment_ref)
  for (const row of commands) await tombstone(PORTABLE_COMMAND_ENTITY_TYPE, row.command_ref)
  for (const row of current) await tombstone(
    PORTABLE_THREAD_CURRENT_ENTITY_TYPE,
    `${input.sessionRef}:${row.thread_ref}`,
  )
  return true
}

export type PortableSessionAuthoritySnapshot = {
  session: Record<string, unknown>
  targets: ReadonlyArray<Record<string, unknown>>
  agents: ReadonlyArray<Record<string, unknown>>
  attachments: ReadonlyArray<Record<string, unknown>>
  checkpoints: ReadonlyArray<Record<string, unknown>>
  commands: ReadonlyArray<Record<string, unknown>>
  current: ReadonlyArray<Record<string, unknown>>
}

/** Restart-safe bounded read: no volatile socket/process state participates. */
export const readPortableSessionAuthoritySnapshot = async (
  sql: SqlTag,
  input: { sessionRef: string; ownerUserId: string },
): Promise<PortableSessionAuthoritySnapshot | null> => {
  const sessions: Array<Record<string, unknown>> = await sql`
    SELECT session_ref, owner_user_id, owner_scope_ref, work_context_ref,
           event_log_ref, current_projection_ref, command_scope_ref,
           root_agent_ref, state, latest_event_cursor, current_attachment_ref,
           current_attachment_generation, adopted_from_local_history,
           adoption_receipt_ref, created_at, updated_at
    FROM khala_sync_portable_sessions
    WHERE session_ref = ${input.sessionRef} AND owner_user_id = ${input.ownerUserId}
  `
  if (sessions[0] === undefined) return null
  const targets: Array<Record<string, unknown>> = await sql`
    SELECT target.target_ref, target.target_class, target.adapter_ref,
           target.compatibility_ref, target.isolation, target.data_posture,
           target.health, target.updated_at
    FROM khala_sync_portable_session_targets AS membership
    JOIN khala_sync_portable_targets AS target
      ON target.target_ref = membership.target_ref
    WHERE membership.session_ref = ${input.sessionRef}
      AND target.owner_user_id = ${input.ownerUserId}
    ORDER BY target.target_ref ASC
  `
  const agents: Array<Record<string, unknown>> = await sql`
    SELECT agent_ref, parent_agent_ref, thread_ref, transcript_ref,
           activity_cursor, lifecycle, attachment_generation
    FROM khala_sync_portable_agent_nodes
    WHERE session_ref = ${input.sessionRef}
    ORDER BY agent_ref ASC
  `
  const attachments: Array<Record<string, unknown>> = await sql`
    SELECT attachment_ref, target_ref, generation, state,
           descendant_agent_refs_json, capability_lease_refs_json,
           checkpoint_ref, evidence_refs_json
    FROM khala_sync_portable_attachments
    WHERE session_ref = ${input.sessionRef}
    ORDER BY generation ASC
  `
  const checkpoints: Array<Record<string, unknown>> = await sql`
    SELECT checkpoint_ref, source_attachment_ref, source_generation, digest,
           repository_ref, repository_revision_ref,
           repository_post_image_digest, diff_digest, event_log_cursor,
           catalog_generation_ref, graph_digest, approval_refs_json,
           artifact_refs_json, receipt_refs_json
    FROM khala_sync_portable_checkpoints
    WHERE session_ref = ${input.sessionRef}
    ORDER BY created_at ASC
  `
  const commands: Array<Record<string, unknown>> = await sql`
    SELECT command_ref, idempotency_key, kind, expected_attachment_ref,
           expected_generation, destination_target_ref, checkpoint_ref,
           expires_at, status, outcome_json, created_at, updated_at
    FROM khala_sync_portable_commands
    WHERE session_ref = ${input.sessionRef}
    ORDER BY created_at ASC
  `
  const current: Array<Record<string, unknown>> = await sql`
    SELECT thread_ref, latest_cursor, current_json, repaired_from_event_seq,
           updated_at
    FROM khala_sync_portable_thread_current
    WHERE session_ref = ${input.sessionRef}
    ORDER BY thread_ref ASC
  `
  return { session: sessions[0], targets, agents, attachments, checkpoints, commands, current }
}
