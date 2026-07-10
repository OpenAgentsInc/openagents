import {
  decodeKhalaFleetIntentJson,
  type KhalaFleetIntent,
  type KhalaFleetIntentKind,
} from "@openagentsinc/khala-fleet-intents"
import {
  canonicalJson,
  fleetRunScope,
  MutationResult,
  MutatorName,
} from "@openagentsinc/khala-sync"
import { ensureScopeOwner } from "./fleet-projection.js"
import type { MutatorContext, MutatorDefinition } from "./push-engine.js"
import { defineMutator } from "./push-engine.js"

/**
 * MH-6 (#8585): the three typed fleet steering mutators.
 *
 * These are the Khala Sync arm of the ONE cross-device steering vocabulary —
 * they consume `KhalaFleetIntent` from `@openagentsinc/khala-fleet-intents`
 * (MH-0 #8581) DIRECTLY; there is no second vocabulary. A phone approval card
 * and a desktop approval button dispatch the identical typed value; on the
 * phone it travels over Sync as one of these mutators, on the desktop the EN
 * runtime can dispatch the same intent locally.
 *
 * - `fleet.dispatchRunControl` — pause / resume / drain / stop a FleetRun
 *   (`fleet_run_control`).
 * - `fleet.dispatchApprovalDecision` — allow / deny a pending tool approval
 *   (`approval_decision`).
 * - `fleet.dispatchSteerMessage` — steer an in-flight worker/turn with an
 *   additional message (`steer_message`).
 *
 * AUTHORITY STAYS SERVER/DESKTOP-SIDE. An applied mutation does two atomic
 * things inside the push-engine transaction: it (1) records the durable typed
 * intent in `khala_sync_fleet_steering_intents` (the receipt + the resumable
 * `seq` watermark the Pylon authority reads through the accepted-claim
 * steering exchange). It deliberately does NOT project the requested action
 * as executor-effective. Only an `applied` outcome ACK may append an effective
 * run or approval post-image. Mobile/web is never a second supervisor
 * implementation; it appends typed requests and reads acknowledged state.
 *
 * OWNERSHIP: the target scope is `scope.fleet_run.<runRef>`; the mutator
 * consults `khala_sync_scope_owners` (first-writer-wins). A scope owned by a
 * different user is an in-band `unauthorized_scope` rejection with zero writes.
 */

export const FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME =
  "fleet.dispatchRunControl"
export const FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME =
  "fleet.dispatchApprovalDecision"
export const FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME =
  "fleet.dispatchSteerMessage"

/** In-band rejection codes (never throw for a client-caused condition). */
export const FLEET_STEERING_SCOPE_REJECTION = "unauthorized_scope"
export const FLEET_STEERING_KIND_REJECTION = "fleet_intent_kind_mismatch"
export const FLEET_STEERING_RUN_REQUIRED_REJECTION = "fleet_run_required"
export const FLEET_STEERING_INTENT_EXISTS_REJECTION = "fleet_intent_exists"
export const FLEET_STEERING_REF_SHAPE_REJECTION = "fleet_intent_ref_shape"
export const FLEET_STEERING_BODY_SHAPE_REJECTION = "fleet_intent_body_shape"

/** The public-safe ref shape the durable table's CHECK constraints enforce. */
const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/
const PUBLIC_BODY_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u
const FLEET_STEERING_INLINE_BODY_MAX_BYTES = 16 * 1_024
const FLEET_STEERING_INTENT_MAX_BYTES = 32 * 1_024

export const decodeFleetIntentArgs = (argsJson: string): KhalaFleetIntent =>
  decodeKhalaFleetIntentJson(argsJson)

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

const applied = (ctx: MutatorContext): MutationResult =>
  new MutationResult({ mutationId: ctx.mutationId, status: "applied" })

const transactionNowIso = async (ctx: MutatorContext): Promise<string> => {
  const rows: Array<{ now: Date | string }> = await ctx.writer.sql`
    SELECT now() AS now
  `
  const raw = rows[0]?.now
  if (raw === undefined) throw new Error("SELECT now() returned no row")
  return raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()
}

interface SteeringIntentColumns {
  readonly action: string | null
  readonly approvalRef: string | null
  readonly decision: string | null
}

const columnsFor = (intent: KhalaFleetIntent): SteeringIntentColumns => {
  switch (intent.kind) {
    case "fleet_run_control":
      return { action: intent.action, approvalRef: null, decision: null }
    case "approval_decision":
      return {
        action: null,
        approvalRef: intent.approvalRef,
        decision: intent.decision,
      }
    default:
      return { action: null, approvalRef: null, decision: null }
  }
}

/**
 * Guard the client-minted refs against the durable table's ref-shape CHECK so
 * a malformed key is an in-band rejection, not a raw transaction failure.
 */
const refShapeRejection = (
  ctx: MutatorContext,
  runRef: string,
  intent: KhalaFleetIntent,
): MutationResult | null => {
  if (
    !PUBLIC_REF_PATTERN.test(runRef) ||
    !PUBLIC_REF_PATTERN.test(intent.intentId) ||
    !PUBLIC_REF_PATTERN.test(intent.idempotencyKey) ||
    intent.intentId.length > 160 ||
    intent.idempotencyKey.length > 120
  ) {
    return reject(
      ctx,
      FLEET_STEERING_REF_SHAPE_REJECTION,
      "fleet steering intent refs must be public-safe (no @, /, or whitespace)",
    )
  }
  if (
    new TextEncoder().encode(canonicalJson(intent)).byteLength >
      FLEET_STEERING_INTENT_MAX_BYTES
  ) {
    return reject(
      ctx,
      FLEET_STEERING_BODY_SHAPE_REJECTION,
      "fleet steering intents must fit the bounded delivery contract",
    )
  }
  if (intent.kind === "steer_message") {
    const bodyCarriers =
      Number(intent.body !== undefined) + Number(intent.bodyRef !== undefined)
    if (
      bodyCarriers !== 1 ||
      (intent.body !== undefined &&
        new TextEncoder().encode(intent.body).byteLength >
          FLEET_STEERING_INLINE_BODY_MAX_BYTES) ||
      (intent.bodyRef !== undefined &&
        (intent.bodyRef.length > 240 ||
          !PUBLIC_BODY_REF_PATTERN.test(intent.bodyRef))) ||
      (intent.targetRef !== undefined &&
        (intent.targetRef.length > 180 ||
          !PUBLIC_BODY_REF_PATTERN.test(intent.targetRef)))
    ) {
      return reject(
        ctx,
        FLEET_STEERING_BODY_SHAPE_REJECTION,
        "fleet steering body carriers must be bounded and public-safe",
      )
    }
  }
  return null
}

const readSteeringConflict = async (
  ctx: MutatorContext,
  intent: KhalaFleetIntent,
): Promise<boolean> => {
  const rows: Array<{ intent_id: string }> = await ctx.writer.sql`
    SELECT intent_id
    FROM khala_sync_fleet_steering_intents
    WHERE intent_id = ${intent.intentId}
       OR (requested_by_user_id = ${ctx.userId}
           AND idempotency_key = ${intent.idempotencyKey})
    LIMIT 1
  `
  return rows[0] !== undefined
}

const insertSteeringIntent = async (
  ctx: MutatorContext,
  runRef: string,
  intent: KhalaFleetIntent,
  nowIso: string,
): Promise<void> => {
  const cols = columnsFor(intent)
  // `intent_json` is jsonb: bind the OBJECT, never a pre-stringified string
  // (double-encoding stores a jsonb string scalar; see runtime-mutators.ts).
  await ctx.writer.sql`
    INSERT INTO khala_sync_fleet_steering_intents
      (intent_id, scope, run_ref, kind, action, approval_ref, decision,
       surface, requested_by_user_id, idempotency_key, intent_json,
       mutation_ref, created_at)
    VALUES
      (${intent.intentId}, ${fleetRunScope(runRef)}, ${runRef}, ${intent.kind},
       ${cols.action}, ${cols.approvalRef}, ${cols.decision},
       ${intent.origin.surface}, ${ctx.userId}, ${intent.idempotencyKey},
       ${intent as unknown as object}::jsonb, ${ctx.mutationRef}, ${nowIso})
  `
}

// ---------------------------------------------------------------------------
// Shared dispatch flow
// ---------------------------------------------------------------------------

const dispatch = async (
  ctx: MutatorContext,
  intent: KhalaFleetIntent,
  expectedKind: KhalaFleetIntentKind,
): Promise<MutationResult> => {
  if (intent.kind !== expectedKind) {
    return reject(
      ctx,
      FLEET_STEERING_KIND_REJECTION,
      "fleet steering intent kind does not match the mutator",
    )
  }
  const runRef = intent.runRef
  if (runRef === undefined) {
    return reject(
      ctx,
      FLEET_STEERING_RUN_REQUIRED_REJECTION,
      "fleet steering intents must name the FleetRun they steer (runRef)",
    )
  }
  const shapeRejection = refShapeRejection(ctx, runRef, intent)
  if (shapeRejection !== null) return shapeRejection

  // AUTHORIZE FIRST: claim-or-read the scope owner (conflict-do-nothing, so a
  // foreign owner performs no write and preserves reject-before-write).
  const owner = await ensureScopeOwner(
    ctx.writer.sql,
    fleetRunScope(runRef),
    ctx.userId,
  )
  if (owner !== ctx.userId) {
    return reject(
      ctx,
      FLEET_STEERING_SCOPE_REJECTION,
      "this fleet run scope belongs to a different user",
    )
  }

  if (await readSteeringConflict(ctx, intent)) {
    return reject(
      ctx,
      FLEET_STEERING_INTENT_EXISTS_REJECTION,
      "this fleet steering intent was already recorded",
    )
  }

  const nowIso = await transactionNowIso(ctx)
  await insertSteeringIntent(ctx, runRef, intent, nowIso)
  return applied(ctx)
}

export const fleetDispatchRunControlMutator: MutatorDefinition =
  defineMutator<KhalaFleetIntent>({
    decodeArgs: decodeFleetIntentArgs,
    execute: (intent, ctx) => dispatch(ctx, intent, "fleet_run_control"),
    name: MutatorName.make(FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME),
  })

export const fleetDispatchApprovalDecisionMutator: MutatorDefinition =
  defineMutator<KhalaFleetIntent>({
    decodeArgs: decodeFleetIntentArgs,
    execute: (intent, ctx) => dispatch(ctx, intent, "approval_decision"),
    name: MutatorName.make(FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME),
  })

export const fleetDispatchSteerMessageMutator: MutatorDefinition =
  defineMutator<KhalaFleetIntent>({
    decodeArgs: decodeFleetIntentArgs,
    execute: (intent, ctx) => dispatch(ctx, intent, "steer_message"),
    name: MutatorName.make(FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME),
  })

/** All three fleet steering mutators, ready for a `makeMutatorRegistry` array. */
export const fleetSteeringMutators: ReadonlyArray<MutatorDefinition> = [
  fleetDispatchRunControlMutator,
  fleetDispatchApprovalDecisionMutator,
  fleetDispatchSteerMessageMutator,
]
