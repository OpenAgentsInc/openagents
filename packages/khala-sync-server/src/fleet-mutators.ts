import {
  decodeFleetRunEntity,
  type FleetRunEntity,
  fleetRunScope,
  MutationResult,
  MutatorName,
} from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"
import {
  appendFleetEntityChange,
  ensureScopeOwner,
} from "./fleet-projection.js"
import type { MutatorContext, MutatorDefinition } from "./push-engine.js"
import { defineMutator } from "./push-engine.js"

/**
 * Fleet cockpit operator mutators (KS-6.1, #8302; SPEC §2.4, MUTATORS.md):
 *
 * - `fleet.setDesiredSlots` — set the run's desired concurrent slots
 * - `fleet.pauseRun` — pause the run (desired behavior: stop dispatching)
 * - `fleet.resumeRun` — resume a paused run
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

/** In-band rejection code when the fleet scope belongs to another user. */
export const FLEET_SCOPE_REJECTION = "unauthorized_scope"

const RunIdField = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

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

export const decodeFleetSetDesiredSlotsArgs = (
  argsJson: string,
): SetDesiredSlotsArgs =>
  S.decodeUnknownSync(SetDesiredSlotsArgs)(JSON.parse(argsJson) as unknown)

export const decodeFleetRunOnlyArgs = (argsJson: string): RunOnlyArgs =>
  S.decodeUnknownSync(RunOnlyArgs)(JSON.parse(argsJson) as unknown)

type FleetIntent = "set_desired_slots" | "pause" | "resume"

/**
 * Read the run's current post-image (latest committed `fleet_run` upsert
 * in the scope's changelog). Absent or undecodable (pre-contract) images
 * yield `null` and the mutator synthesizes a baseline — the projection is
 * a post-image log, so the next system projection self-heals any drift.
 */
const readCurrentFleetRun = async (
  ctx: MutatorContext,
  runId: string,
): Promise<FleetRunEntity | null> => {
  const scope = fleetRunScope(runId)
  const rows: Array<{ post_image_json: string | object }> = await ctx.writer
    .sql`
    SELECT post_image_json FROM khala_sync_changelog
    WHERE scope = ${scope} AND entity_type = 'fleet_run'
      AND entity_id = ${runId} AND op = 'upsert'
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
    return decodeFleetRunEntity(value)
  } catch {
    return null
  }
}

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

const executeFleetIntent = async (
  ctx: MutatorContext,
  input: {
    readonly runId: string
    readonly intent: FleetIntent
    readonly desiredSlots: number | null
    readonly apply: (current: FleetRunEntity) => FleetRunEntity
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
  const current =
    (await readCurrentFleetRun(ctx, input.runId)) ??
    baselineRun(input.runId, nowIso)
  const updated = input.apply(
    decodeFleetRunEntity({
      ...current,
      counters: { ...current.counters },
      updatedAt: nowIso,
    }),
  )

  // Durable operator intent (business write) + projected post-image, one
  // transaction, both attributable to this mutation.
  await ctx.writer.sql`
    INSERT INTO khala_sync_fleet_intents
      (scope, run_id, intent, desired_slots, requested_by_user_id, mutation_ref)
    VALUES
      (${scope}, ${input.runId}, ${input.intent}, ${input.desiredSlots},
       ${ctx.userId}, ${ctx.mutationRef})
  `
  await appendFleetEntityChange(
    ctx.writer,
    input.runId,
    { entity: updated, kind: "fleet_run", op: "upsert" },
    ctx.mutationRef,
  )

  return new MutationResult({ mutationId: ctx.mutationId, status: "applied" })
}

export const fleetSetDesiredSlotsMutator: MutatorDefinition =
  defineMutator<SetDesiredSlotsArgs>({
    decodeArgs: decodeFleetSetDesiredSlotsArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        apply: (current) =>
          decodeFleetRunEntity({
            ...current,
            counters: { ...current.counters },
            desiredSlots: args.desiredSlots,
          }),
        desiredSlots: args.desiredSlots,
        intent: "set_desired_slots",
        runId: args.runId,
      }),
    name: MutatorName.make(FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME),
  })

export const fleetPauseRunMutator: MutatorDefinition =
  defineMutator<RunOnlyArgs>({
    decodeArgs: decodeFleetRunOnlyArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        apply: (current) =>
          decodeFleetRunEntity({
            ...current,
            counters: { ...current.counters },
            status: "paused",
          }),
        desiredSlots: null,
        intent: "pause",
        runId: args.runId,
      }),
    name: MutatorName.make(FLEET_PAUSE_RUN_MUTATOR_NAME),
  })

export const fleetResumeRunMutator: MutatorDefinition =
  defineMutator<RunOnlyArgs>({
    decodeArgs: decodeFleetRunOnlyArgs,
    execute: (args, ctx) =>
      executeFleetIntent(ctx, {
        apply: (current) =>
          decodeFleetRunEntity({
            ...current,
            counters: { ...current.counters },
            status: "running",
          }),
        desiredSlots: null,
        intent: "resume",
        runId: args.runId,
      }),
    name: MutatorName.make(FLEET_RESUME_RUN_MUTATOR_NAME),
  })

/** All fleet operator mutators, ready for a `makeMutatorRegistry` array. */
export const fleetOperatorMutators: ReadonlyArray<MutatorDefinition> = [
  fleetSetDesiredSlotsMutator,
  fleetPauseRunMutator,
  fleetResumeRunMutator,
]
