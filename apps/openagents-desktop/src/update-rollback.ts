/**
 * Desktop update/rollback state machine (CUT-26, #8706).
 *
 * A deterministic, total, PURE reducer over the update lifecycle:
 *
 *   idle → checking → downloading → verifying → staged → applying
 *                                                          ├─ success → idle (installed = candidate, previous retained)
 *                                                          └─ failure → rolling_back → idle (installed = previous)
 *
 * Laws enforced structurally (each has a deterministic oracle in
 * `tests/update-rollback.test.ts`):
 *
 * 1. Version monotonicity — a candidate is admitted only when it is a strict
 *    upgrade for the machine's channel (`isMonotonicUpgrade`). The reducer
 *    re-checks even if the verification seam already did: defense in depth.
 * 2. Rollback is the ONLY sanctioned downgrade, and only to the retained
 *    `previous` slot. No previous slot → rollback refused.
 * 3. Apply is refused until every migration-ledger category (sessions,
 *    vault refs, settings, drafts) is `preserved` or `loss_accounted` with a
 *    bounded public-safe reason ref. `unknown` never applies.
 * 4. Interruption is loss-free: interrupt during checking/downloading/
 *    verifying discards the in-flight candidate and returns to idle;
 *    interrupt during `staged` keeps the durable staged slot; interrupt
 *    during `applying` is treated as an apply failure and rolls back.
 * 5. Illegal events never throw and never mutate — the reducer returns the
 *    unchanged state plus a typed refusal.
 *
 * No I/O, no clock, no Electron — the host wires this machine to the
 * verification seam (`update-contract.ts`) and the restart plumbing in a
 * later CUT-26 exit; the state logic itself is fully provable here.
 */
import {
  type UpdateChannel,
  type UpdateManifest,
  type UpdateVerificationFailure,
  isMonotonicUpgrade,
} from "./update-contract.ts"

export const UPDATE_ROLLBACK_SCHEMA_ID = "openagents.desktop.update_rollback.v1" as const

// ---------------------------------------------------------------------------
// Migration ledger — sessions, vault refs, settings, drafts
// ---------------------------------------------------------------------------

export const migrationCategories = ["sessions", "vaultRefs", "settings", "drafts"] as const
export type MigrationCategory = (typeof migrationCategories)[number]

const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,119}$/

export type MigrationDisposition =
  | { readonly status: "unknown" }
  | { readonly status: "preserved" }
  | { readonly status: "loss_accounted"; readonly reasonRef: string }

export type MigrationLedger = Readonly<Record<MigrationCategory, MigrationDisposition>>

export const emptyMigrationLedger = (): MigrationLedger => ({
  sessions: { status: "unknown" },
  vaultRefs: { status: "unknown" },
  settings: { status: "unknown" },
  drafts: { status: "unknown" },
})

const ledgerReadyToApply = (ledger: MigrationLedger): boolean =>
  migrationCategories.every((category) => ledger[category].status !== "unknown")

// ---------------------------------------------------------------------------
// Machine state
// ---------------------------------------------------------------------------

export const updatePhases = [
  "idle",
  "checking",
  "downloading",
  "verifying",
  "staged",
  "applying",
  "rolling_back",
  "rollback_failed",
] as const
export type UpdatePhase = (typeof updatePhases)[number]

export interface RetainedRelease {
  readonly version: string
}

export type UpdateFailureRecord =
  | { readonly kind: "manifest_rejected"; readonly reason: UpdateVerificationFailure | "not_monotonic" }
  | { readonly kind: "artifact_rejected" }
  | { readonly kind: "apply_failed" }
  | { readonly kind: "interrupted"; readonly during: UpdatePhase }
  | { readonly kind: "rollback_failed" }

export interface UpdateMachineState {
  readonly schema: typeof UPDATE_ROLLBACK_SCHEMA_ID
  readonly phase: UpdatePhase
  readonly channel: UpdateChannel
  /** Currently installed release version. Never regresses except via rollback. */
  readonly installed: string
  /** Retained rollback slot — the only sanctioned downgrade target. */
  readonly previous: RetainedRelease | null
  /** In-flight candidate manifest (admitted, not yet applied). */
  readonly candidate: UpdateManifest | null
  readonly ledger: MigrationLedger
  /** Honest record of the most recent failure; cleared on the next admit. */
  readonly lastFailure: UpdateFailureRecord | null
}

export const initialUpdateState = (
  installed: string,
  channel: UpdateChannel,
): UpdateMachineState => ({
  schema: UPDATE_ROLLBACK_SCHEMA_ID,
  phase: "idle",
  channel,
  installed,
  previous: null,
  candidate: null,
  ledger: emptyMigrationLedger(),
  lastFailure: null,
})

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

export type UpdateEvent =
  | { readonly type: "check_started" }
  | { readonly type: "manifest_verified"; readonly manifest: UpdateManifest }
  | { readonly type: "manifest_rejected"; readonly reason: UpdateVerificationFailure }
  | { readonly type: "artifact_verified" }
  | { readonly type: "artifact_rejected" }
  | {
    readonly type: "migration_recorded"
    readonly category: MigrationCategory
    readonly disposition: MigrationDisposition
  }
  | { readonly type: "staged" }
  | { readonly type: "apply_requested" }
  | { readonly type: "apply_succeeded" }
  | { readonly type: "apply_failed" }
  | { readonly type: "rollback_requested" }
  | { readonly type: "rollback_completed" }
  | { readonly type: "rollback_failed" }
  | { readonly type: "interrupted" }

export const updateRefusals = [
  "event_not_admissible_in_phase",
  "candidate_not_monotonic",
  "candidate_channel_mismatch",
  "migration_ledger_incomplete",
  "loss_reason_ref_invalid",
  "no_previous_release_retained",
] as const
export type UpdateRefusal = (typeof updateRefusals)[number]

export interface UpdateTransition {
  readonly state: UpdateMachineState
  /** `null` means the event was admitted; otherwise the state is UNCHANGED. */
  readonly refusal: UpdateRefusal | null
}

const admit = (state: UpdateMachineState): UpdateTransition => ({ state, refusal: null })
const refuse = (state: UpdateMachineState, refusal: UpdateRefusal): UpdateTransition => ({
  state,
  refusal,
})

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

export const updateReducer = (
  state: UpdateMachineState,
  event: UpdateEvent,
): UpdateTransition => {
  switch (event.type) {
    case "check_started": {
      if (state.phase !== "idle") return refuse(state, "event_not_admissible_in_phase")
      return admit({ ...state, phase: "checking" })
    }

    case "manifest_verified": {
      if (state.phase !== "checking") return refuse(state, "event_not_admissible_in_phase")
      if (event.manifest.channel !== state.channel) {
        return refuse(state, "candidate_channel_mismatch")
      }
      const verdict = isMonotonicUpgrade(state.installed, event.manifest.version, state.channel)
      if (!verdict.admissible) return refuse(state, "candidate_not_monotonic")
      return admit({
        ...state,
        phase: "downloading",
        candidate: event.manifest,
        ledger: emptyMigrationLedger(),
        lastFailure: null,
      })
    }

    case "manifest_rejected": {
      if (state.phase !== "checking") return refuse(state, "event_not_admissible_in_phase")
      return admit({
        ...state,
        phase: "idle",
        candidate: null,
        lastFailure: { kind: "manifest_rejected", reason: event.reason },
      })
    }

    case "artifact_verified": {
      if (state.phase !== "downloading") return refuse(state, "event_not_admissible_in_phase")
      return admit({ ...state, phase: "verifying" })
    }

    case "artifact_rejected": {
      if (state.phase !== "downloading" && state.phase !== "verifying") {
        return refuse(state, "event_not_admissible_in_phase")
      }
      // A bad digest/signature discards the download entirely — nothing from
      // a failed verification may remain staged or resumable.
      return admit({
        ...state,
        phase: "idle",
        candidate: null,
        ledger: emptyMigrationLedger(),
        lastFailure: { kind: "artifact_rejected" },
      })
    }

    case "migration_recorded": {
      if (state.phase !== "verifying" && state.phase !== "staged") {
        return refuse(state, "event_not_admissible_in_phase")
      }
      if (
        event.disposition.status === "loss_accounted"
        && !PUBLIC_REF_PATTERN.test(event.disposition.reasonRef)
      ) {
        return refuse(state, "loss_reason_ref_invalid")
      }
      return admit({
        ...state,
        ledger: { ...state.ledger, [event.category]: event.disposition },
      })
    }

    case "staged": {
      if (state.phase !== "verifying") return refuse(state, "event_not_admissible_in_phase")
      // Staging retains the running release as the rollback slot.
      return admit({
        ...state,
        phase: "staged",
        previous: { version: state.installed },
      })
    }

    case "apply_requested": {
      if (state.phase !== "staged") return refuse(state, "event_not_admissible_in_phase")
      if (!ledgerReadyToApply(state.ledger)) return refuse(state, "migration_ledger_incomplete")
      return admit({ ...state, phase: "applying" })
    }

    case "apply_succeeded": {
      if (state.phase !== "applying" || state.candidate === null) {
        return refuse(state, "event_not_admissible_in_phase")
      }
      return admit({
        ...state,
        phase: "idle",
        installed: state.candidate.version,
        candidate: null,
        lastFailure: null,
        // `previous` stays retained so a post-apply regression can still roll back.
      })
    }

    case "apply_failed": {
      if (state.phase !== "applying") return refuse(state, "event_not_admissible_in_phase")
      return admit({ ...state, phase: "rolling_back", lastFailure: { kind: "apply_failed" } })
    }

    case "rollback_requested": {
      if (state.phase !== "idle") return refuse(state, "event_not_admissible_in_phase")
      if (state.previous === null) return refuse(state, "no_previous_release_retained")
      return admit({ ...state, phase: "rolling_back" })
    }

    case "rollback_completed": {
      if (state.phase !== "rolling_back" || state.previous === null) {
        return refuse(state, "event_not_admissible_in_phase")
      }
      // The retained slot is consumed: rollback restores EXACTLY that version
      // and there is no further downgrade target until the next stage.
      return admit({
        ...state,
        phase: "idle",
        installed: state.previous.version,
        previous: null,
        candidate: null,
        ledger: emptyMigrationLedger(),
      })
    }

    case "rollback_failed": {
      if (state.phase !== "rolling_back") return refuse(state, "event_not_admissible_in_phase")
      // Honest terminal-until-recovery state: the machine never pretends a
      // failed rollback succeeded. The host surfaces diagnostics/recovery.
      return admit({ ...state, phase: "rollback_failed", lastFailure: { kind: "rollback_failed" } })
    }

    case "interrupted": {
      switch (state.phase) {
        case "checking":
        case "downloading":
        case "verifying":
          // Loss-free discard: nothing was staged, nothing changed on disk.
          return admit({
            ...state,
            phase: "idle",
            candidate: null,
            ledger: emptyMigrationLedger(),
            lastFailure: { kind: "interrupted", during: state.phase },
          })
        case "staged":
          // A staged update is durable — restart resumes at `staged` with the
          // candidate and ledger intact.
          return admit({
            ...state,
            lastFailure: { kind: "interrupted", during: "staged" },
          })
        case "applying":
          // Interrupting an apply is an apply failure: roll back.
          return admit({
            ...state,
            phase: "rolling_back",
            lastFailure: { kind: "interrupted", during: "applying" },
          })
        default:
          return refuse(state, "event_not_admissible_in_phase")
      }
    }
  }
}

/** Convenience: run a deterministic event sequence from an initial state. */
export const runUpdateEvents = (
  state: UpdateMachineState,
  events: ReadonlyArray<UpdateEvent>,
): { readonly state: UpdateMachineState; readonly refusals: ReadonlyArray<UpdateRefusal> } => {
  let current = state
  const refusals: Array<UpdateRefusal> = []
  for (const event of events) {
    const transition = updateReducer(current, event)
    current = transition.state
    if (transition.refusal !== null) refusals.push(transition.refusal)
  }
  return { state: current, refusals }
}
