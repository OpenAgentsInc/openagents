import {
  PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY,
  type PylonFlexibleLoadEventRecord,
  type PylonFlexibleLoadEventState,
} from './pylon-flexible-load-events'

/**
 * Curtailment-drill plan helpers.
 *
 * Advances `blocker.product_promises.flexible_load_event_history_missing` for
 * `energy.flexible_load_proof.v1` by binding the planned training-marathon
 * curtailment drill (`training.marathon_operations.v1`) to the existing
 * flexible-load event-telemetry schema (`pylon-flexible-load-events.ts`).
 *
 * This module is INTENTIONALLY honest: before the drill actually runs there is
 * no measured telemetry, so the only event record it will build is one in the
 * `requested` state. The per-state evidence contract below records what each
 * later transition MUST emit for the record to advance to `settled` — it does
 * not, and cannot, fabricate measured watts, evidence, or settlement.
 *
 * Authority: read-only event telemetry. Building a plan record neither
 * dispatches capacity, sheds load, spends a wallet, nor upgrades any
 * grid-service claim.
 */

/**
 * Ordered happy-path lifecycle a real drill event walks through. `blocked` and
 * `failed` are off-path terminal states handled separately and are not part of
 * this advancement sequence.
 */
export const CURTAILMENT_DRILL_EVENT_STATE_SEQUENCE: ReadonlyArray<
  PylonFlexibleLoadEventState
> = [
  'requested',
  'acknowledged',
  'executed',
  'measured',
  'verified',
  'compensated',
  'settled',
]

/**
 * The evidence a drill operator must capture to advance the event into each
 * state. These mirror the staging rules enforced by `assertRecordSafe` in
 * `pylon-flexible-load-events.ts`; this is the human-/operator-facing contract
 * that the schema enforces mechanically.
 */
export interface CurtailmentDrillStateRequirement {
  readonly state: PylonFlexibleLoadEventState
  readonly summary: string
  /** Ref groups (record fields) that MUST be non-empty to reach this state. */
  readonly requiredRefFields: ReadonlyArray<keyof PylonFlexibleLoadEventRecord>
  /** Non-ref fields that MUST be populated to reach this state. */
  readonly requiredValueFields: ReadonlyArray<keyof PylonFlexibleLoadEventRecord>
}

export const CURTAILMENT_DRILL_EVENT_STATE_REQUIREMENTS:
  ReadonlyArray<CurtailmentDrillStateRequirement> = [
    {
      state: 'requested',
      summary:
        'Drill scheduled: target shed watts and the curtailment request are recorded before any load is shed.',
      requiredRefFields: ['requestRefs'],
      requiredValueFields: ['requestedResponseWatts'],
    },
    {
      state: 'acknowledged',
      summary:
        'Selected pylons acknowledge the curtailment request before the shed window opens.',
      requiredRefFields: ['acknowledgementRefs'],
      requiredValueFields: [],
    },
    {
      state: 'executed',
      summary:
        'Fleet sheds the requested portion on schedule and resumes from sealed checkpoints; execution proof captured.',
      requiredRefFields: ['executionRefs'],
      requiredValueFields: [],
    },
    {
      state: 'measured',
      summary:
        'Actual shed power is measured (not modeled) and bound to measurement evidence.',
      requiredRefFields: ['measurementRefs'],
      requiredValueFields: ['actualResponseWatts'],
    },
    {
      state: 'verified',
      summary:
        'Independent verification confirms the shed-and-resume happened as recorded.',
      requiredRefFields: ['evidenceRefs'],
      requiredValueFields: [],
    },
    {
      state: 'compensated',
      summary:
        'Any contributor compensation for interrupted accepted work is recorded.',
      requiredRefFields: ['compensationRefs'],
      requiredValueFields: [],
    },
    {
      state: 'settled',
      summary:
        'Drill settlement is recorded. NOTE: a drill receipt is NOT a grid-services revenue claim.',
      requiredRefFields: ['settlementRefs'],
      requiredValueFields: [],
    },
  ]

export interface CurtailmentDrillPlanInput {
  readonly caveatRefs?: ReadonlyArray<string>
  readonly checkpointRefs?: ReadonlyArray<string>
  readonly createdAtIso: string
  readonly drillRef: string
  readonly id: string
  readonly profileRefs: ReadonlyArray<string>
  readonly providerRef: string
  readonly requestRefs: ReadonlyArray<string>
  readonly requestedResponseWatts: number
  readonly sourceRefs?: ReadonlyArray<string>
  readonly updatedAtIso: string
  readonly workClassRefs: ReadonlyArray<string>
}

/**
 * Build the FIRST, pre-execution drill event in the honest `requested` state.
 *
 * All measured/verified/settled fields are deliberately empty or null because
 * the drill has not run yet. The record validates against the event schema and
 * is the seed that the drill execution advances through
 * {@link CURTAILMENT_DRILL_EVENT_STATE_SEQUENCE}.
 */
export const buildCurtailmentDrillRequestedEvent = (
  input: CurtailmentDrillPlanInput,
): PylonFlexibleLoadEventRecord => ({
  acceptedWorkImpactRefs: [],
  acknowledgementRefs: [],
  actualResponseWatts: null,
  authority: PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY,
  blockerRefs: [],
  caveatRefs: input.caveatRefs ?? [
    'caveat.public.curtailment_drill_not_grid_settlement',
  ],
  checkpointRefs: input.checkpointRefs ?? [],
  compensationRefs: [],
  createdAtIso: input.createdAtIso,
  eventRef: input.drillRef,
  evidenceRefs: [],
  executionRefs: [],
  id: input.id,
  interruptedWorkRefs: [],
  lostWorkCostCents: 0,
  measurementRefs: [],
  profileRefs: input.profileRefs,
  providerRef: input.providerRef,
  requestedResponseWatts: input.requestedResponseWatts,
  requestRefs: input.requestRefs,
  resumeRefs: [],
  settlementRefs: [],
  sourceRefs: input.sourceRefs ?? [],
  state: 'requested',
  updatedAtIso: input.updatedAtIso,
  workClassRefs: input.workClassRefs,
})

/**
 * The planned, public-safe seed event for the training-marathon curtailment
 * drill. Concrete telemetry/evidence/settlement refs are filled in only when
 * the drill actually runs.
 */
export const exampleCurtailmentDrillRequestedEvent =
  (): PylonFlexibleLoadEventRecord =>
    buildCurtailmentDrillRequestedEvent({
      checkpointRefs: ['checkpoint.policy.marathon_window_seal_v1'],
      createdAtIso: '2026-06-20T00:00:00.000Z',
      drillRef: 'event.flex.training_marathon_curtailment_drill_1',
      id: 'flex_event.training_marathon_curtailment_drill_1',
      profileRefs: ['profile.flex.psion_pretraining_window'],
      providerRef: 'provider.marathon_fleet_cohort_1',
      requestRefs: ['request.public.training_marathon_curtailment_drill_1'],
      requestedResponseWatts: 250000,
      sourceRefs: ['source.public.training_marathon_operations'],
      updatedAtIso: '2026-06-20T00:00:00.000Z',
      workClassRefs: ['work_class.psion_pretraining_window'],
    })
