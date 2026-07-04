import {
  decodeFleetInboxFlagEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  type FleetInboxFlagEntity,
  type FleetRunEntity,
  fleetRunScope,
  type FleetWorkerEntity,
  MutationResult,
  MutatorName,
} from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"
import {
  appendFleetEntityChange,
  ensureScopeOwner,
  type FleetEntityChange,
} from "./fleet-projection.js"
import type { MutatorContext, MutatorDefinition } from "./push-engine.js"
import { defineMutator } from "./push-engine.js"

/**
 * Fleet cockpit operator mutators (KS-6.1 #8302 + KS-3.2 #8292; SPEC §2.4,
 * MUTATORS.md):
 *
 * - `fleet.setDesiredSlots` — set the run's desired concurrent slots
 * - `fleet.pauseRun` — pause the run (desired behavior: stop dispatching)
 * - `fleet.resumeRun` — resume a paused run
 * - `fleet.pauseWorker` — pause ONE worker slot (`fleet_worker` post-image)
 * - `fleet.resumeWorker` — resume a paused worker slot (phase → `idle`)
 * - `fleet.acknowledgeInboxFlag` — durably ack an inbox/attention flag
 *   (`fleet_inbox_flag` post-image)
 * - `fleet.stopRun` — TERMINAL stop; guarded by an explicit `confirm: true`
 *   arg (anything else is an in-band `confirmation_required` rejection with
 *   zero writes — it does not even claim scope ownership)
 *
 * Each executes inside the push engine's single transaction: scope-owner
 * gate → intent row (`khala_sync_fleet_intents`) → updated `fleet_run`
 * post-image append, all atomic and attributable to `ctx.mutationRef`.
 *
 * OWNERSHIP: the target scope is `scope.fleet_run.<runId>`; the mutator
 * consults `khala_sync_scope_owners`. An UNOWNED scope is claimed by the
 * caller (first-writer-wins, same rule as the projection side); a scope
 * owned by a DIFFERENT user is an in-band `unauthorized_scope` rejection
 * with zero writes (the ownership probe is ON CONFLICT DO NOTHING, so the
 * rejection path preserves reject-before-write).
 *
 * HONEST V1 CONTRACT: an applied mutation means the operator intent is
 * durably recorded and the updated `fleet_run` post-image is projected —
 * cockpit UIs converge on the new desired state. Supervisor ENFORCEMENT
 * (the Pylon-side supervisor polling/consuming `khala_sync_fleet_intents`
 * and actually changing dispatch behavior) is a follow-up lane; until it
 * lands, an intent is a recorded request, not proof of changed behavior.
 */

export const FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME = "fleet.setDesiredSlots"
export const FLEET_PAUSE_RUN_MUTATOR_NAME = "fleet.pauseRun"
export const FLEET_RESUME_RUN_MUTATOR_NAME = "fleet.resumeRun"
export const FLEET_PAUSE_WORKER_MUTATOR_NAME = "fleet.pauseWorker"
export const FLEET_RESUME_WORKER_MUTATOR_NAME = "fleet.resumeWorker"
export const FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME =
  "fleet.acknowledgeInboxFlag"
export const FLEET_STOP_RUN_MUTATOR_NAME = "fleet.stopRun"

/** In-band rejection code when the fleet scope belongs to another user. */
export const FLEET_SCOPE_REJECTION = "unauthorized_scope"

/** In-band rejection code when `fleet.stopRun` lacks `confirm: true`. */
export const FLEET_STOP_CONFIRMATION_REJECTION = "confirmation_required"

/** Public-safe structured ref (same shape as the contract `FleetPublicRef`). */
const PublicRefField = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

const RunIdField = PublicRefField

const SetDesiredSlotsArgs = S.Struct({
  runId: RunIdField,
  desiredSlots: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(1024),
  ),
})
type SetDesiredSlotsArgs = typeof SetDesiredSlotsArgs.Type

const RunOnlyArgs = S.Struct({ runId: RunIdField })
type RunOnlyArgs = typeof RunOnlyArgs.Type

const WorkerArgs = S.Struct({ runId: RunIdField, workerId: PublicRefField })
type WorkerArgs = typeof WorkerArgs.Type

const AcknowledgeInboxFlagArgs = S.Struct({
  runId: RunIdField,
  flagRef: PublicRefField,
})
type AcknowledgeInboxFlagArgs = typeof AcknowledgeInboxFlagArgs.Type

const StopRunArgs = S.Struct({ runId: RunIdField, confirm: S.Boolean })
type StopRunArgs = typeof StopRunArgs.Type

export const decodeFleetSetDesiredSlotsArgs = (
  argsJson: string,
): SetDesiredSlotsArgs =>
  S.decodeUnknownSync(SetDesiredSlotsArgs)(JSON.parse(argsJson) as unknown)

export const decodeFleetRunOnlyArgs = (argsJson: string): RunOnlyArgs =>
  S.decodeUnknownSync(RunOnlyArgs)(JSON.parse(argsJson) as unknown)

export const decodeFleetWorkerArgs = (argsJson: string): WorkerArgs =>
  S.decodeUnknownSync(WorkerArgs)(JSON.parse(argsJson) as unknown)

export const decodeFleetAcknowledgeInboxFlagArgs = (
  argsJson: string,
): AcknowledgeInboxFlagArgs =>
  S.decodeUnknownSync(AcknowledgeInboxFlagArgs)(
    JSON.parse(argsJson) as unknown,
  )

export const decodeFleetStopRunArgs = (argsJson: string): StopRunArgs =>
  S.decodeUnknownSync(StopRunArgs)(JSON.parse(argsJson) as unknown)

/**
 * Durable intent vocabulary (khala_sync_fleet_intents.intent — kept in
 * lockstep with migration 0005's CHECK constraint).
 */
type FleetIntent =
  | "set_desired_slots"
  | "pause"
  | "resume"
  | "pause_worker"
  | "resume_worker"
  | "acknowledge_inbox_flag"
  | "stop"

/**
 * Read an entity's current post-image (latest committed upsert in the
 * scope's changelog). Absent or undecodable (pre-contract) images yield
 * `null` and the mutator synthesizes a baseline — the projection is a
 * post-image log, so the next system projection self-heals any drift.
 */
const readCurrentEntity = async <A>(
  ctx: MutatorContext,
  runId: string,
  entityType: string,
  entityId: string,
  decode: (value: unknown) => A,
): Promise<A | null> => {
  const scope = fleetRunScope(runId)
  const rows: Array<{ post_image_json: string | object }> = await ctx.writer
    .sql`
    SELECT post_image_json FROM khala_sync_changelog
    WHERE scope = ${scope} AND entity_type = ${entityType}
      AND entity_id = ${entityId} AND op = 'upsert'
    ORDER BY version DESC
    LIMIT 1
  `
  const row = rows[0]
  if (row === undefined) return null
  try {
    const value =
      typeof row.post_image_json === "string"
        ? (JSON.parse(row.post_image_json) as unknown)
        : row.post_image_json
    return decode(value)
  } catch {
    return null
  }
}

const readCurrentFleetRun = (
  ctx: MutatorContext,
  runId: string,
): Promise<FleetRunEntity | null> =>
  readCurrentEntity(ctx, runId, "fleet_run", runId, decodeFleetRunEntity)

const readCurrentFleetWorker = (
  ctx: MutatorContext,
  runId: string,
  workerId: string,
): Promise<FleetWorkerEntity | null> =>
  readCurrentEntity(
    ctx,
    runId,
    "fleet_worker",
    workerId,
    decodeFleetWorkerEntity,
  )

const readCurrentFleetInboxFlag = (
  ctx: MutatorContext,
  runId: string,
  flagRef: string,
): Promise<FleetInboxFlagEntity | null> =>
  readCurrentEntity(
    ctx,
    runId,
    "fleet_inbox_flag",
    flagRef,
    decodeFleetInboxFlagEntity,
  )

/** Transaction-clock timestamp (derived inside the transaction, SPEC §2.4). */
const transactionNowIso = async (ctx: MutatorContext): Promise<string> => {
  const rows: Array<{ now: Date | string }> = await ctx.writer.sql`
    SELECT now() AS now
  `
  const raw = rows[0]?.now
  if (raw === undefined) throw new Error("SELECT now() returned no row")
  return raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()
}

const baselineRun = (runId: string, updatedAt: string): FleetRunEntity =>
  decodeFleetRunEntity({
    counters: {
      activeAssignments: 0,
      blockedAssignments: 0,
      completedAssignments: 0,
      failedAssignments: 0,
      workUnitsTotal: 0,
    },
    desiredSlots: 0,
    runId,
    startedAt: null,
    status: "draft",
    updatedAt,
    workerKind: "auto",
  })

/** One fleet entity change built inside the mutator's transaction. */
type BuildFleetChange = (
  nowIso: string,
) => Promise<Extract<FleetEntityChange, { readonly op: "upsert" }>>

const executeFleetIntent = async (
  ctx: MutatorContext,
  input: {
    readonly runId: string
    readonly intent: FleetIntent
    readonly desiredSlots: number | null
    readonly workerId: string | null
    readonly flagRef: string | null
    readonly buildChange: BuildFleetChange
  },
): Promise<MutationResult> => {
  const scope = fleetRunScope(input.runId)

  // AUTHORIZE FIRST: claim-or-read the scope owner. Foreign owner ⇒
  // in-band rejection with zero writes (the probe is conflict-do-nothing).
  const owner = await ensureScopeOwner(ctx.writer.sql, scope, ctx.userId)
  if (owner !== ctx.userId) {
    return new MutationResult({
      errorCode: FLEET_SCOPE_REJECTION,
      errorMessageSafe:
        "this fleet run scope belongs to a different user",
      mutationId: ctx.mutationId,
      status: "rejected",
    })
  }

  const nowIso = await transactionNowIso(ctx)
  const change = await input.buildChange(nowIso)

  // Durable operator intent (business write) + projected post-image, one
  // transaction, both attributable to this mutation.
  await ctx.writer.sql`
    INSERT INTO khala_sync_fleet_intents
      (scope, run_id, intent, desired_slots, worker_id, flag_ref,
       requested_by_user_id, mutation_ref)
    VALUES
      (${scope}, ${input.runId}, ${input.intent}, ${input.desiredSlots},
       ${input.workerId}, ${input.flagRef}, ${ctx.userId}, ${ctx.mutationRef})
  `
  await appendFleetEntityChange(ctx.writer, input.runId, change, ctx.mutationRef)

  return new MutationResult({ mutationId: ctx.mutationId, status: "applied" })
}

/** Build the patched `fleet_run` post-image for a run-level intent. */
const buildRunChange =
  (
    ctx: MutatorContext,
    runId: string,
    apply: (current: FleetRunEntity) => FleetRunEntity,
  ): BuildFleetChange =>
  async (nowIso) => {
    const current =
      (await readCurrentFleetRun(ctx, runId)) ?? baselineRun(runId, nowIso)
    const updated = apply(
      decodeFleetRunEntity({
        ...current,
        counters: { ...current.counters },
        updatedAt: nowIso,
      }),
    )
    return { entity: updated, kind: "fleet_run", op: "upsert" }
  }

/**
 * Build the patched `fleet_worker` post-image for a per-worker intent. A
 * worker the scope has never projected gets a minimal baseline (post-image
 * log semantics: the next system projection self-heals the rest).
 */
const buildWorkerChange =
  (
    ctx: MutatorContext,
    runId: string,
    workerId: string,
    phase: "paused" | "idle",
  ): BuildFleetChange =>
  async (nowIso) => {
    const current = await readCurrentFleetWorker(ctx, runId, workerId)
    const updated = decodeFleetWorkerEntity({
      ...(current ?? {}),
      phase,
      updatedAt: nowIso,
      workerId,
    })
    return { entity: updated, kind: "fleet_worker", op: "upsert" }
  }

/**
 * Build the acknowledged `fleet_inbox_flag` post-image. When the flag was
 * projected before, its `kind`/`openedAt` are preserved; a flag the server
 * has not seen still records the ack durably with kind `unclassified`
 * (HONEST V1: flag producers are a follow-up projection lane — an ack is a
 * durable operator statement either way, and merges once producers land).
 */
const buildAcknowledgeFlagChange =
  (ctx: MutatorContext, runId: string, flagRef: string): BuildFleetChange =>
  async (nowIso) => {
    const current = await readCurrentFleetInboxFlag(ctx, runId, flagRef)
    const updated = decodeFleetInboxFlagEntity({
      kind: "unclassified",
      ...(current ?? {}),
      acknowledgedAt: nowIso,
      flagRef,
      status: "acknowledged",
      updatedAt: nowIso,
    })
    return { entity: updated, kind: "fleet_inbox_flag", op: "upsert" }
  }

const runStatusPatch = (
  status: "paused" | "running",
): ((current: FleetRunEntity) => FleetRunEntity) =>
  (current) =>
    decodeFleetRunEntity({
      ...current,
      counters: { ...current.counters },
      status,
    })

export const fleetSetDesiredSlotsMutator: MutatorDefinition =
  defineMutator<SetDesiredSlotsArgs>({
    decodeArgs: decodeFleetSetDesiredSlotsArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        buildChange: buildRunChange(ctx, args.runId, (current) =>
          decodeFleetRunEntity({
            ...current,
            counters: { ...current.counters },
            desiredSlots: args.desiredSlots,
          }),
        ),
        desiredSlots: args.desiredSlots,
        flagRef: null,
        intent: "set_desired_slots",
        runId: args.runId,
        workerId: null,
      }),
    name: MutatorName.make(FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME),
  })

export const fleetPauseRunMutator: MutatorDefinition =
  defineMutator<RunOnlyArgs>({
    decodeArgs: decodeFleetRunOnlyArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        buildChange: buildRunChange(ctx, args.runId, runStatusPatch("paused")),
        desiredSlots: null,
        flagRef: null,
        intent: "pause",
        runId: args.runId,
        workerId: null,
      }),
    name: MutatorName.make(FLEET_PAUSE_RUN_MUTATOR_NAME),
  })

export const fleetResumeRunMutator: MutatorDefinition =
  defineMutator<RunOnlyArgs>({
    decodeArgs: decodeFleetRunOnlyArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        buildChange: buildRunChange(ctx, args.runId, runStatusPatch("running")),
        desiredSlots: null,
        flagRef: null,
        intent: "resume",
        runId: args.runId,
        workerId: null,
      }),
    name: MutatorName.make(FLEET_RESUME_RUN_MUTATOR_NAME),
  })

export const fleetPauseWorkerMutator: MutatorDefinition =
  defineMutator<WorkerArgs>({
    decodeArgs: decodeFleetWorkerArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        buildChange: buildWorkerChange(ctx, args.runId, args.workerId, "paused"),
        desiredSlots: null,
        flagRef: null,
        intent: "pause_worker",
        runId: args.runId,
        workerId: args.workerId,
      }),
    name: MutatorName.make(FLEET_PAUSE_WORKER_MUTATOR_NAME),
  })

export const fleetResumeWorkerMutator: MutatorDefinition =
  defineMutator<WorkerArgs>({
    decodeArgs: decodeFleetWorkerArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        buildChange: buildWorkerChange(ctx, args.runId, args.workerId, "idle"),
        desiredSlots: null,
        flagRef: null,
        intent: "resume_worker",
        runId: args.runId,
        workerId: args.workerId,
      }),
    name: MutatorName.make(FLEET_RESUME_WORKER_MUTATOR_NAME),
  })

export const fleetAcknowledgeInboxFlagMutator: MutatorDefinition =
  defineMutator<AcknowledgeInboxFlagArgs>({
    decodeArgs: decodeFleetAcknowledgeInboxFlagArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        buildChange: buildAcknowledgeFlagChange(ctx, args.runId, args.flagRef),
        desiredSlots: null,
        flagRef: args.flagRef,
        intent: "acknowledge_inbox_flag",
        runId: args.runId,
        workerId: null,
      }),
    name: MutatorName.make(FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME),
  })

export const fleetStopRunMutator: MutatorDefinition =
  defineMutator<StopRunArgs>({
    decodeArgs: decodeFleetStopRunArgs,
    execute: (args, ctx) => {
      // TERMINAL GUARD, before ANY write (including the scope-owner claim):
      // stopping a run is not undoable by `fleet.resumeRun` semantics alone,
      // so the caller must state `confirm: true` explicitly. Anything else
      // is an in-band ack'd rejection that leaves the world untouched.
      if (!args.confirm) {
        return Promise.resolve(
          new MutationResult({
            errorCode: FLEET_STOP_CONFIRMATION_REJECTION,
            errorMessageSafe:
              "fleet.stopRun is terminal and requires confirm: true",
            mutationId: ctx.mutationId,
            status: "rejected",
          }),
        )
      }
      return executeFleetIntent(ctx, {
        buildChange: buildRunChange(ctx, args.runId, (current) =>
          decodeFleetRunEntity({
            ...current,
            counters: { ...current.counters },
            desiredSlots: 0,
            status: "stopped",
          }),
        ),
        desiredSlots: null,
        flagRef: null,
        intent: "stop",
        runId: args.runId,
        workerId: null,
      })
    },
    name: MutatorName.make(FLEET_STOP_RUN_MUTATOR_NAME),
  })

/** All fleet operator mutators, ready for a `makeMutatorRegistry` array. */
export const fleetOperatorMutators: ReadonlyArray<MutatorDefinition> = [
  fleetSetDesiredSlotsMutator,
  fleetPauseRunMutator,
  fleetResumeRunMutator,
  fleetPauseWorkerMutator,
  fleetResumeWorkerMutator,
  fleetAcknowledgeInboxFlagMutator,
  fleetStopRunMutator,
]
