import {
  decodeFleetSteeringOutcomeAck,
  decodeFleetSteeringOutcomeBatch,
  decodeFleetSteeringPage,
  decodeKhalaFleetIntent,
  decodeKhalaFleetIntentJson,
  FLEET_STEERING_PAGE_MAX_INTENTS,
  type FleetSteeringClaimRef,
  type FleetSteeringDeliveryIntent,
  type FleetSteeringOutcome,
  type FleetSteeringOutcomeAck,
  type FleetSteeringOutcomeBatch,
  fleetSteeringOutcomeRefContent,
  type FleetSteeringPage,
  type FleetSteeringRunRef,
  type KhalaFleetIntent,
} from "@openagentsinc/khala-fleet-intents"
import {
  canonicalJson,
  decodeFleetApprovalEntity,
  decodeFleetRunEntity,
  FLEET_APPROVAL_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FleetApprovalEntity,
  FleetRunEntity,
  type FleetRunStatus,
  fleetRunScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Clock, Effect, Schema as S } from "effect"

import { FleetRunAuthorityError } from "./fleet-run-authority.js"
import {
  appendFleetEntityChange,
  ensureScopeOwner,
} from "./fleet-projection.js"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SyncTransactionWriter } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

export {
  FleetSteeringOutcomeBatch,
  type FleetSteeringOutcomeAck,
  type FleetSteeringPage,
} from "@openagentsinc/khala-fleet-intents"

export const FLEET_STEERING_OUTCOME_MUTATION_REF =
  "system:sarah_fleet_run_steering_outcome.v1" as const
export const FLEET_STEERING_PAGE_MAX_BYTES = 256 * 1_024
export const FLEET_STEERING_INLINE_BODY_MAX_BYTES = 16 * 1_024

const PublicOwnerRef = S.Trim.check(
  S.isMinLength(3),
  S.isMaxLength(160),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
)
const PublicPylonRef = S.Trim.check(
  S.isMinLength(3),
  S.isMaxLength(120),
  S.isPattern(/^[a-z0-9][a-z0-9._:-]*$/u),
)
const FleetSteeringReadInput = S.Struct({
  ownerUserId: PublicOwnerRef,
  pylonRef: PublicPylonRef,
  runRef: S.String.check(
    S.isPattern(/^fleet_run\.sarah\.[0-9a-f]{20}$/u),
  ),
  claimRef: S.String.check(
    S.isPattern(/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u),
  ),
  after: S.Int.check(
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
  ),
  limit: S.Int.check(
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(FLEET_STEERING_PAGE_MAX_INTENTS),
  ),
})
const FleetSteeringAppendOutcomesInput = S.Struct({
  ownerUserId: PublicOwnerRef,
  pylonRef: PublicPylonRef,
  runRef: FleetSteeringReadInput.fields.runRef,
  batch: S.Unknown,
})

export type FleetSteeringReadInput = typeof FleetSteeringReadInput.Type
export type FleetSteeringAppendOutcomesInput = Readonly<{
  ownerUserId: string
  pylonRef: string
  runRef: string
  batch: FleetSteeringOutcomeBatch
}>

export type FleetSteeringExchangeRepositoryShape = Readonly<{
  readPage: (
    input: unknown,
  ) => Effect.Effect<FleetSteeringPage, FleetRunAuthorityError>
  appendOutcomes: (
    input: unknown,
  ) => Effect.Effect<FleetSteeringOutcomeAck, FleetRunAuthorityError>
}>

type AcceptedLeaseRow = Readonly<{
  run_ref: string
  claim_ref: string
  owner_user_id: string
  pylon_ref: string
  state: string
}>

type RawIntentRow = Readonly<{
  seq: string | number | bigint
  intent_id: string
  run_ref: string
  requested_by_user_id: string
  intent_json: unknown
  created_at: string
}>

type StoredOutcomeRow = Readonly<{
  run_ref: string
  seq: string | number | bigint
  intent_id: string
  owner_user_id: string
  pylon_ref: string
  intake_claim_ref: string
  outcome: FleetSteeringOutcome["outcome"]
  outcome_ref: string
  observed_at: string
  recorded_at: string
}>

const fixedError = (
  kind: FleetRunAuthorityError["kind"],
  reason: string,
  refs: Readonly<{ runRef?: string; pylonRef?: string }> = {},
): FleetRunAuthorityError =>
  new FleetRunAuthorityError({ kind, reason, ...refs })

const invalidRequest = (): FleetRunAuthorityError =>
  fixedError("invalid_request", "fleet steering exchange request failed validation")

const exchangeErrorFromUnknown = (error: unknown): FleetRunAuthorityError =>
  error instanceof FleetRunAuthorityError
    ? error
    : fixedError("storage_unavailable", "fleet steering exchange is unavailable")

const decodeUnknown = <A>(schema: S.Decoder<A>, input: unknown): A => {
  try {
    return S.decodeUnknownSync(schema)(input, { onExcessProperty: "error" })
  } catch {
    throw invalidRequest()
  }
}

const safeSequence = (value: string | number | bigint): number => {
  const sequence = typeof value === "number" ? value : Number(value)
  if (!Number.isSafeInteger(sequence) || sequence < 1) {
    throw fixedError(
      "storage_unavailable",
      "fleet steering sequence failed integrity validation",
    )
  }
  return sequence
}

const validIsoTimestamp = (value: string): boolean => {
  const parsed = new Date(value)
  return Number.isFinite(parsed.getTime()) && parsed.toISOString() === value
}

const decodeStoredIntent = (row: RawIntentRow): KhalaFleetIntent => {
  try {
    return typeof row.intent_json === "string"
      ? decodeKhalaFleetIntentJson(row.intent_json)
      : decodeKhalaFleetIntent(row.intent_json)
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet steering intent failed integrity validation",
      { runRef: row.run_ref },
    )
  }
}

const deliveryFromRow = (
  row: RawIntentRow,
  expectedRunRef: string,
): FleetSteeringDeliveryIntent => {
  const intent = decodeStoredIntent(row)
  const seq = safeSequence(row.seq)
  if (
    row.run_ref !== expectedRunRef ||
    row.intent_id !== intent.intentId ||
    intent.runRef !== expectedRunRef ||
    !validIsoTimestamp(row.created_at)
  ) {
    throw fixedError(
      "storage_unavailable",
      "fleet steering intent identity failed integrity validation",
      { runRef: expectedRunRef },
    )
  }
  if (
    intent.kind === "steer_message" &&
    ((intent.body !== undefined &&
      new TextEncoder().encode(intent.body).byteLength >
        FLEET_STEERING_INLINE_BODY_MAX_BYTES) ||
      (intent.bodyRef !== undefined &&
        (intent.bodyRef.length > 240 ||
          !/^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u.test(intent.bodyRef))))
  ) {
    throw fixedError(
      "storage_unavailable",
      "fleet steering body carrier failed integrity validation",
      { runRef: expectedRunRef },
    )
  }
  return { seq, intentId: row.intent_id, intent, createdAt: row.created_at }
}

const requireAcceptedLease = async (
  sql: SqlTag,
  input: Readonly<{
    ownerUserId: string
    pylonRef: string
    runRef: string
    claimRef: string
  }>,
): Promise<AcceptedLeaseRow> => {
  const rows: Array<AcceptedLeaseRow> = await sql`
    SELECT lease.run_ref, lease.claim_ref, lease.owner_user_id,
           lease.pylon_ref, lease.state
    FROM sarah_fleet_run_intake_leases AS lease
    INNER JOIN sarah_fleet_run_requests AS request
      ON request.run_ref = lease.run_ref
     AND request.owner_user_id = lease.owner_user_id
    WHERE lease.run_ref = ${input.runRef}
      AND lease.claim_ref = ${input.claimRef}
      AND lease.owner_user_id = ${input.ownerUserId}
      AND lease.pylon_ref = ${input.pylonRef}
      AND lease.state = 'accepted'
      AND request.status = 'claimed_by_pylon'
    LIMIT 1
  `
  const row = rows[0]
  if (row === undefined) {
    throw fixedError(
      "claim_conflict",
      "fleet steering requires the exact accepted run claim",
      { runRef: input.runRef, pylonRef: input.pylonRef },
    )
  }
  return row
}

const reserveDelivery = async (
  sql: SqlTag,
  input: Readonly<{
    ownerUserId: string
    pylonRef: string
    runRef: string
    claimRef: string
    delivery: FleetSteeringDeliveryIntent
    deliveredAt: string
  }>,
): Promise<void> => {
  await sql`
    INSERT INTO sarah_fleet_run_steering_deliveries
      (run_ref, seq, intent_id, owner_user_id, pylon_ref, intake_claim_ref,
       delivered_at)
    VALUES
      (${input.runRef}, ${input.delivery.seq}, ${input.delivery.intentId},
       ${input.ownerUserId}, ${input.pylonRef}, ${input.claimRef},
       ${input.deliveredAt})
    ON CONFLICT DO NOTHING
  `
  const rows: Array<{
    owner_user_id: string
    pylon_ref: string
    intake_claim_ref: string
  }> = await sql`
    SELECT owner_user_id, pylon_ref, intake_claim_ref
    FROM sarah_fleet_run_steering_deliveries
    WHERE run_ref = ${input.runRef}
      AND seq = ${input.delivery.seq}
      AND intent_id = ${input.delivery.intentId}
    LIMIT 1
  `
  const row = rows[0]
  if (
    row === undefined ||
    row.owner_user_id !== input.ownerUserId ||
    row.pylon_ref !== input.pylonRef ||
    row.intake_claim_ref !== input.claimRef
  ) {
    throw fixedError(
      "claim_conflict",
      "fleet steering intent is reserved to another accepted claim",
      { runRef: input.runRef, pylonRef: input.pylonRef },
    )
  }
}

const pageWithinByteLimit = (
  input: FleetSteeringReadInput,
  deliveries: ReadonlyArray<FleetSteeringDeliveryIntent>,
): ReadonlyArray<FleetSteeringDeliveryIntent> =>
  deliveries.reduce<ReadonlyArray<FleetSteeringDeliveryIntent>>(
    (selected, delivery) => {
      const candidate = [...selected, delivery]
      const candidatePage = {
        ok: true,
        runRef: input.runRef,
        claimRef: input.claimRef,
        intents: candidate,
        nextAfter: delivery.seq,
        upToDate: false,
      }
      return new TextEncoder().encode(canonicalJson(candidatePage)).byteLength <=
        FLEET_STEERING_PAGE_MAX_BYTES
        ? candidate
        : selected
    },
    [],
  )

const readSteeringPage = async (
  sql: SyncSql,
  input: FleetSteeringReadInput,
  deliveredAt: string,
): Promise<FleetSteeringPage> => {
  return withSyncTransaction(sql, async (writer) => {
    await requireAcceptedLease(writer.sql, input)
    const rows: Array<RawIntentRow> = await writer.sql`
      SELECT intent.seq, intent.intent_id, intent.run_ref,
             intent.requested_by_user_id, intent.intent_json,
             intent.created_at
      FROM khala_sync_fleet_steering_intents AS intent
      LEFT JOIN sarah_fleet_run_steering_outcomes AS outcome
        ON outcome.run_ref = intent.run_ref
       AND outcome.seq = intent.seq
       AND outcome.intent_id = intent.intent_id
      WHERE intent.run_ref = ${input.runRef}
        AND intent.requested_by_user_id = ${input.ownerUserId}
        AND intent.seq > ${input.after}
        AND outcome.seq IS NULL
      ORDER BY intent.seq ASC
      LIMIT ${input.limit + 1}
    `
    const candidates = rows
      .slice(0, input.limit)
      .map((row) => deliveryFromRow(row, input.runRef))
    const intents = pageWithinByteLimit(input, candidates)
    if (candidates.length > 0 && intents.length === 0) {
      throw fixedError(
        "storage_unavailable",
        "fleet steering intent exceeds the delivery page bound",
        { runRef: input.runRef },
      )
    }
    await intents.reduce<Promise<void>>(
      async (pending, delivery) => {
        await pending
        await reserveDelivery(writer.sql, {
          ownerUserId: input.ownerUserId,
          pylonRef: input.pylonRef,
          runRef: input.runRef,
          claimRef: input.claimRef,
          delivery,
          deliveredAt,
        })
      },
      Promise.resolve(),
    )
    const nextAfter = intents.at(-1)?.seq ?? input.after
    const page = decodeFleetSteeringPage({
      ok: true,
      runRef: input.runRef,
      claimRef: input.claimRef,
      intents,
      nextAfter,
      upToDate: intents.length === rows.length,
    })
    if (
      new TextEncoder().encode(canonicalJson(page)).byteLength >
      FLEET_STEERING_PAGE_MAX_BYTES
    ) {
      throw fixedError(
        "storage_unavailable",
        "fleet steering page exceeded its response bound",
        { runRef: input.runRef },
      )
    }
    return page
  })
}

const sha256Hex = async (input: unknown): Promise<string> => {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(canonicalJson(input)),
  )
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")
}

export const fleetSteeringOutcomeRef = async (input: Readonly<{
  runRef: FleetSteeringRunRef
  claimRef: FleetSteeringClaimRef
  pylonRef: string
  outcome: FleetSteeringOutcome
}>): Promise<string> =>
  `outcome.pylon.fleet_steering.${(
    await sha256Hex(
      fleetSteeringOutcomeRefContent({
        runRef: input.runRef,
        claimRef: input.claimRef,
        pylonRef: input.pylonRef,
        seq: input.outcome.seq,
        intentId: input.outcome.intentId,
        outcome: input.outcome.outcome,
        observedAt: input.outcome.observedAt,
      }),
    )
  ).slice(0, 24)}`

const assertOutcomeContentBinding = async (
  input: Readonly<{
    runRef: FleetSteeringRunRef
    claimRef: FleetSteeringClaimRef
    pylonRef: string
    outcome: FleetSteeringOutcome
  }>,
): Promise<void> => {
  if (input.outcome.outcomeRef !== (await fleetSteeringOutcomeRef(input))) {
    throw invalidRequest()
  }
}

const storedOutcomeMatches = (
  row: StoredOutcomeRow,
  input: Readonly<{
    ownerUserId: string
    pylonRef: string
    runRef: string
    claimRef: string
    outcome: FleetSteeringOutcome
  }>,
): boolean =>
  row.run_ref === input.runRef &&
  safeSequence(row.seq) === input.outcome.seq &&
  row.intent_id === input.outcome.intentId &&
  row.owner_user_id === input.ownerUserId &&
  row.pylon_ref === input.pylonRef &&
  row.intake_claim_ref === input.claimRef &&
  row.outcome === input.outcome.outcome &&
  row.outcome_ref === input.outcome.outcomeRef &&
  row.observed_at === input.outcome.observedAt

const readStoredOutcome = async (
  sql: SqlTag,
  runRef: string,
  outcome: FleetSteeringOutcome,
): Promise<StoredOutcomeRow | undefined> => {
  const rows: Array<StoredOutcomeRow> = await sql`
    SELECT * FROM sarah_fleet_run_steering_outcomes
    WHERE (run_ref = ${runRef} AND seq = ${outcome.seq})
       OR intent_id = ${outcome.intentId}
       OR outcome_ref = ${outcome.outcomeRef}
    LIMIT 1
  `
  return rows[0]
}

const readIntentForOutcome = async (
  sql: SqlTag,
  input: Readonly<{
    ownerUserId: string
    pylonRef: string
    runRef: string
    claimRef: string
    outcome: FleetSteeringOutcome
  }>,
): Promise<KhalaFleetIntent> => {
  const rows: Array<RawIntentRow> = await sql`
    SELECT intent.seq, intent.intent_id, intent.run_ref,
           intent.requested_by_user_id, intent.intent_json,
           intent.created_at
    FROM khala_sync_fleet_steering_intents AS intent
    INNER JOIN sarah_fleet_run_steering_deliveries AS delivery
      ON delivery.run_ref = intent.run_ref
     AND delivery.seq = intent.seq
     AND delivery.intent_id = intent.intent_id
    WHERE intent.run_ref = ${input.runRef}
      AND intent.seq = ${input.outcome.seq}
      AND intent.intent_id = ${input.outcome.intentId}
      AND intent.requested_by_user_id = ${input.ownerUserId}
      AND delivery.owner_user_id = ${input.ownerUserId}
      AND delivery.pylon_ref = ${input.pylonRef}
      AND delivery.intake_claim_ref = ${input.claimRef}
    LIMIT 1
  `
  const row = rows[0]
  if (row === undefined) {
    throw fixedError(
      "claim_conflict",
      "fleet steering outcome does not match a delivered intent",
      { runRef: input.runRef },
    )
  }
  return deliveryFromRow(row, input.runRef).intent
}

const assertNextUnacknowledgedIntent = async (
  sql: SqlTag,
  runRef: string,
  ownerUserId: string,
  sequence: number,
): Promise<void> => {
  const rows: Array<{ seq: string | number | bigint }> = await sql`
    SELECT intent.seq
    FROM khala_sync_fleet_steering_intents AS intent
    LEFT JOIN sarah_fleet_run_steering_outcomes AS outcome
      ON outcome.run_ref = intent.run_ref
     AND outcome.seq = intent.seq
     AND outcome.intent_id = intent.intent_id
    WHERE intent.run_ref = ${runRef}
      AND intent.requested_by_user_id = ${ownerUserId}
      AND outcome.seq IS NULL
    ORDER BY intent.seq ASC
    LIMIT 1
  `
  const nextSequence = rows[0] === undefined ? undefined : safeSequence(rows[0].seq)
  if (nextSequence !== sequence) {
    throw fixedError(
      "claim_conflict",
      "fleet steering outcomes must acknowledge same-run intents in order",
      { runRef },
    )
  }
}

const decodeFleetRunPostImage = (raw: string | object): FleetRunEntity => {
  try {
    return typeof raw === "string"
      ? S.decodeUnknownSync(S.fromJsonString(FleetRunEntity))(raw)
      : decodeFleetRunEntity(raw)
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run projection failed integrity validation",
    )
  }
}

const decodeFleetApprovalPostImage = (
  raw: string | object,
): FleetApprovalEntity => {
  try {
    return typeof raw === "string"
      ? S.decodeUnknownSync(S.fromJsonString(FleetApprovalEntity))(raw)
      : decodeFleetApprovalEntity(raw)
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet approval projection failed integrity validation",
    )
  }
}

const readCurrentPostImage = async (
  sql: SqlTag,
  scope: SyncScope,
  entityType: string,
  entityId: string,
): Promise<string | object | undefined> => {
  const rows: Array<{ post_image_json: string | object }> = await sql`
    SELECT post_image_json
    FROM khala_sync_changelog
    WHERE scope = ${scope}
      AND entity_type = ${entityType}
      AND entity_id = ${entityId}
      AND op = 'upsert'
    ORDER BY version DESC
    LIMIT 1
  `
  return rows[0]?.post_image_json
}

const statusForAction = (
  action: "pause" | "resume" | "drain" | "stop",
): FleetRunStatus => {
  switch (action) {
    case "pause":
      return "paused"
    case "resume":
      return "running"
    case "drain":
      return "draining"
    case "stop":
      return "stopped"
  }
}

const runTransitionAllowed = (
  current: FleetRunStatus,
  action: "pause" | "resume" | "drain" | "stop",
): boolean => {
  const desired = statusForAction(action)
  if (current === desired) {
    return true
  }
  if (action === "pause") {
    return current === "running"
  }
  if (action === "resume") {
    return current === "paused"
  }
  if (action === "drain") {
    return current === "running" || current === "paused"
  }
  return (
    current === "draft" ||
    current === "running" ||
    current === "paused" ||
    current === "draining"
  )
}

const appendEffectiveProjection = async (
  writer: SyncTransactionWriter,
  runRef: string,
  ownerUserId: string,
  intent: KhalaFleetIntent,
  observedAt: string,
): Promise<void> => {
  const scope = fleetRunScope(runRef)
  const owner = await ensureScopeOwner(writer.sql, scope, ownerUserId)
  if (owner !== ownerUserId) {
    throw fixedError(
      "pylon_not_authorized",
      "fleet run scope is owned by another user",
      { runRef },
    )
  }
  if (intent.kind === "fleet_run_control") {
    const raw = await readCurrentPostImage(
      writer.sql,
      scope,
      FLEET_RUN_ENTITY_TYPE,
      runRef,
    )
    if (raw === undefined) {
      throw fixedError(
        "storage_unavailable",
        "fleet run projection is unavailable",
        { runRef },
      )
    }
    const current = decodeFleetRunPostImage(raw)
    if (!runTransitionAllowed(current.status, intent.action)) {
      throw fixedError(
        "claim_conflict",
        "fleet run control outcome is invalid for the effective run state",
        { runRef },
      )
    }
    await appendFleetEntityChange(
      writer,
      runRef,
      {
        kind: "fleet_run",
        op: "upsert",
        entity: decodeFleetRunEntity({
          ...current,
          counters: { ...current.counters },
          desiredSlots:
            intent.action === "drain" || intent.action === "stop"
              ? 0
              : current.desiredSlots,
          status: statusForAction(intent.action),
          updatedAt: observedAt,
        }),
      },
      FLEET_STEERING_OUTCOME_MUTATION_REF,
    )
    return
  }
  if (intent.kind === "approval_decision") {
    const raw = await readCurrentPostImage(
      writer.sql,
      scope,
      FLEET_APPROVAL_ENTITY_TYPE,
      intent.approvalRef,
    )
    if (raw === undefined) {
      throw fixedError(
        "claim_conflict",
        "fleet approval outcome has no pending effective approval",
        { runRef },
      )
    }
    const current = decodeFleetApprovalPostImage(raw)
    const desiredStatus = intent.decision === "allow" ? "allowed" : "denied"
    if (current.status !== "pending" && current.status !== desiredStatus) {
      throw fixedError(
        "claim_conflict",
        "fleet approval outcome conflicts with the effective approval state",
        { runRef },
      )
    }
    await appendFleetEntityChange(
      writer,
      runRef,
      {
        kind: "fleet_approval",
        op: "upsert",
        entity: decodeFleetApprovalEntity({
          ...current,
          approvalRef: intent.approvalRef,
          status: desiredStatus,
          decidedAt: observedAt,
          updatedAt: observedAt,
        }),
      },
      FLEET_STEERING_OUTCOME_MUTATION_REF,
    )
  }
}

const appendOneOutcome = async (
  writer: SyncTransactionWriter,
  input: Readonly<{
    ownerUserId: string
    pylonRef: string
    runRef: FleetSteeringRunRef
    claimRef: FleetSteeringClaimRef
    outcome: FleetSteeringOutcome
    recordedAt: string
  }>,
): Promise<"stored" | "duplicate"> => {
  const existing = await readStoredOutcome(writer.sql, input.runRef, input.outcome)
  if (existing !== undefined) {
    if (storedOutcomeMatches(existing, input)) {
      return "duplicate"
    }
    throw fixedError(
      "idempotency_conflict",
      "fleet steering outcome identity is already bound to different bytes",
      { runRef: input.runRef },
    )
  }
  await assertOutcomeContentBinding(input)
  const intent = await readIntentForOutcome(writer.sql, input)
  await assertNextUnacknowledgedIntent(
    writer.sql,
    input.runRef,
    input.ownerUserId,
    input.outcome.seq,
  )
  const inserted: Array<StoredOutcomeRow> = await writer.sql`
    INSERT INTO sarah_fleet_run_steering_outcomes
      (run_ref, seq, intent_id, owner_user_id, pylon_ref, intake_claim_ref,
       outcome, outcome_ref, observed_at, recorded_at)
    VALUES
      (${input.runRef}, ${input.outcome.seq}, ${input.outcome.intentId},
       ${input.ownerUserId}, ${input.pylonRef}, ${input.claimRef},
       ${input.outcome.outcome}, ${input.outcome.outcomeRef},
       ${input.outcome.observedAt}, ${input.recordedAt})
    ON CONFLICT DO NOTHING
    RETURNING *
  `
  if (inserted[0] === undefined) {
    const raced = await readStoredOutcome(writer.sql, input.runRef, input.outcome)
    if (raced !== undefined && storedOutcomeMatches(raced, input)) {
      return "duplicate"
    }
    throw fixedError(
      "idempotency_conflict",
      "fleet steering outcome identity is already bound to different bytes",
      { runRef: input.runRef },
    )
  }
  if (input.outcome.outcome === "applied") {
    await appendEffectiveProjection(
      writer,
      input.runRef,
      input.ownerUserId,
      intent,
      input.recordedAt,
    )
  }
  return "stored"
}

const appendSteeringOutcomes = async (
  sql: SyncSql,
  input: FleetSteeringAppendOutcomesInput,
  recordedAt: string,
): Promise<FleetSteeringOutcomeAck> =>
  withSyncTransaction(sql, async (writer) => {
    await requireAcceptedLease(writer.sql, {
      ownerUserId: input.ownerUserId,
      pylonRef: input.pylonRef,
      runRef: input.runRef,
      claimRef: input.batch.claimRef,
    })
    const outcomes = [...input.batch.outcomes]
    if (
      new Set(outcomes.map((outcome) => outcome.seq)).size !== outcomes.length ||
      new Set(outcomes.map((outcome) => outcome.intentId)).size !==
        outcomes.length ||
      new Set(outcomes.map((outcome) => outcome.outcomeRef)).size !==
        outcomes.length ||
      outcomes.some(
        (outcome, index) =>
          !validIsoTimestamp(outcome.observedAt) ||
          (index > 0 && outcome.seq <= outcomes[index - 1]!.seq),
      )
    ) {
      throw invalidRequest()
    }
    const dispositions = await outcomes.reduce<
      Promise<ReadonlyArray<"stored" | "duplicate">>
    >(
      async (pending, outcome) => [
        ...(await pending),
        await appendOneOutcome(writer, {
          ownerUserId: input.ownerUserId,
          pylonRef: input.pylonRef,
          runRef: input.runRef,
          claimRef: input.batch.claimRef,
          outcome,
          recordedAt,
        }),
      ],
      Promise.resolve([]),
    )
    return decodeFleetSteeringOutcomeAck({
      ok: true,
      runRef: input.runRef,
      claimRef: input.batch.claimRef,
      outcomes,
      storedOutcomeCount: dispositions.filter((value) => value === "stored")
        .length,
      duplicateOutcomeCount: dispositions.filter(
        (value) => value === "duplicate",
      ).length,
    })
  })

export type MakeFleetSteeringExchangeRepositoryOptions = Readonly<{
  sql: SyncSql
  now?: Effect.Effect<number>
}>

export const makeFleetSteeringExchangeRepository = (
  options: MakeFleetSteeringExchangeRepositoryOptions,
): FleetSteeringExchangeRepositoryShape => {
  const now = options.now ?? Clock.currentTimeMillis
  const readPage = Effect.fn("FleetSteeringExchangeRepository.readPage")(
    (rawInput: unknown) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () => decodeUnknown(FleetSteeringReadInput, rawInput),
          catch: exchangeErrorFromUnknown,
        })
        const nowMs = yield* now
        return yield* Effect.tryPromise({
          try: () =>
            readSteeringPage(
              options.sql,
              input,
              new Date(nowMs).toISOString(),
            ),
          catch: exchangeErrorFromUnknown,
        })
      }),
  )
  const appendOutcomes = Effect.fn(
    "FleetSteeringExchangeRepository.appendOutcomes",
  )((rawInput: unknown) =>
    Effect.gen(function* () {
      const input = yield* Effect.try({
        try: () => {
          const decoded = decodeUnknown(
            FleetSteeringAppendOutcomesInput,
            rawInput,
          )
          return {
            ownerUserId: decoded.ownerUserId,
            pylonRef: decoded.pylonRef,
            runRef: decoded.runRef,
            batch: decodeFleetSteeringOutcomeBatch(decoded.batch),
          }
        },
        catch: exchangeErrorFromUnknown,
      })
      const nowMs = yield* now
      return yield* Effect.tryPromise({
        try: () =>
          appendSteeringOutcomes(
            options.sql,
            input,
            new Date(nowMs).toISOString(),
          ),
        catch: exchangeErrorFromUnknown,
      })
    }),
  )
  return { readPage, appendOutcomes }
}
