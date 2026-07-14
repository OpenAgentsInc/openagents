/**
 * Update/rollback state-machine oracles (CUT-26, #8706).
 *
 * The reducer is pure and total, so every law is proven as a deterministic
 * event-sequence walk — no clock, no I/O, no Electron.
 */
import { describe, expect, test } from "vite-plus/test"
import {
  type UpdateManifest,
  UPDATE_CONTRACT_SCHEMA_ID,
} from "../src/update-contract.ts"
import {
  type UpdateEvent,
  type UpdateMachineState,
  initialUpdateState,
  migrationCategories,
  runUpdateEvents,
  updatePhases,
  updateReducer,
} from "../src/update-rollback.ts"

const manifest = (version: string, channel: "stable" | "rc" = "rc"): UpdateManifest => ({
  schema: UPDATE_CONTRACT_SCHEMA_ID,
  app: "openagents-desktop",
  channel,
  version,
  artifactName: `OpenAgents-${version}-arm64.dmg`,
  artifactSha256: "ab".repeat(32),
  artifactByteLength: 1024,
  releasedAt: "2026-07-12T00:00:00Z",
})

const recordAllPreserved = (): ReadonlyArray<UpdateEvent> =>
  migrationCategories.map((category) => ({
    type: "migration_recorded" as const,
    category,
    disposition: { status: "preserved" as const },
  }))

const happyPathToStaged = (state: UpdateMachineState) =>
  runUpdateEvents(state, [
    { type: "check_started" },
    { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
    { type: "artifact_verified" },
    ...recordAllPreserved(),
    { type: "staged" },
  ])

describe("update happy path", () => {
  test("check -> download -> verify -> stage -> apply advances the installed version and awaits the first-launch receipt", () => {
    const staged = happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc"))
    expect(staged.refusals).toEqual([])
    expect(staged.state.phase).toBe("staged")
    expect(staged.state.previous).toEqual({ version: "0.1.0-rc.1" })

    const applied = runUpdateEvents(staged.state, [
      { type: "apply_requested" },
      { type: "apply_succeeded" },
    ])
    expect(applied.refusals).toEqual([])
    // Apply is NOT success (#8786): the machine holds with the previous
    // release staged until the new build demonstrates a first launch.
    expect(applied.state.phase).toBe("awaiting_launch_receipt")
    expect(applied.state.installed).toBe("0.1.0-rc.2")
    expect(applied.state.candidate).toBeNull()
    expect(applied.state.previous).toEqual({ version: "0.1.0-rc.1" })

    const confirmed = runUpdateEvents(applied.state, [
      { type: "launch_receipt_recorded", version: "0.1.0-rc.2" },
    ])
    expect(confirmed.refusals).toEqual([])
    expect(confirmed.state.phase).toBe("idle")
    // The rollback slot survives a confirmed launch for post-launch regression recovery.
    expect(confirmed.state.previous).toEqual({ version: "0.1.0-rc.1" })
  })
})

describe("version monotonicity in the machine (defense in depth)", () => {
  test("refuses a same-version candidate", () => {
    const result = runUpdateEvents(initialUpdateState("0.1.0-rc.2", "rc"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
    ])
    expect(result.refusals).toEqual(["candidate_not_monotonic"])
    expect(result.state.phase).toBe("checking")
    expect(result.state.candidate).toBeNull()
  })

  test("refuses a downgrade candidate outright — rollback is the only downgrade path", () => {
    const result = runUpdateEvents(initialUpdateState("0.2.0", "rc"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.9") },
    ])
    expect(result.refusals).toEqual(["candidate_not_monotonic"])
  })

  test("refuses an rc manifest on a stable-channel machine", () => {
    const result = runUpdateEvents(initialUpdateState("0.1.0", "stable"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.2.0-rc.1", "rc") },
    ])
    expect(result.refusals).toEqual(["candidate_channel_mismatch"])
  })
})

describe("migration ledger gates apply", () => {
  test("apply is refused while any category is unknown", () => {
    const staged = runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
      { type: "artifact_verified" },
      // Only 3 of 4 categories recorded — drafts stays unknown.
      { type: "migration_recorded", category: "sessions", disposition: { status: "preserved" } },
      { type: "migration_recorded", category: "vaultRefs", disposition: { status: "preserved" } },
      { type: "migration_recorded", category: "settings", disposition: { status: "preserved" } },
      { type: "staged" },
      { type: "apply_requested" },
    ])
    expect(staged.refusals).toEqual(["migration_ledger_incomplete"])
    expect(staged.state.phase).toBe("staged")
  })

  test("loss_accounted is admissible only with a bounded public-safe reason ref", () => {
    const base = runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
      { type: "artifact_verified" },
    ]).state

    const badRef = updateReducer(base, {
      type: "migration_recorded",
      category: "drafts",
      disposition: { status: "loss_accounted", reasonRef: "/etc/passwd leak\nnewline" },
    })
    expect(badRef.refusal).toBe("loss_reason_ref_invalid")
    expect(badRef.state.ledger.drafts).toEqual({ status: "unknown" })

    const goodRef = updateReducer(base, {
      type: "migration_recorded",
      category: "drafts",
      disposition: { status: "loss_accounted", reasonRef: "migration.drafts.schema_v2_reset" },
    })
    expect(goodRef.refusal).toBeNull()
    expect(goodRef.state.ledger.drafts).toEqual({
      status: "loss_accounted",
      reasonRef: "migration.drafts.schema_v2_reset",
    })
  })

  test("a fully loss-accounted ledger admits apply (loss is accounted, not hidden)", () => {
    const result = runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
      { type: "artifact_verified" },
      ...migrationCategories.map((category) => ({
        type: "migration_recorded" as const,
        category,
        disposition: { status: "loss_accounted" as const, reasonRef: `migration.${category}.reset` },
      })),
      { type: "staged" },
      { type: "apply_requested" },
    ])
    expect(result.refusals).toEqual([])
    expect(result.state.phase).toBe("applying")
  })
})

describe("verification failure discards the download", () => {
  test("artifact rejection returns to idle with nothing staged or resumable", () => {
    const result = runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
      { type: "artifact_rejected" },
    ])
    expect(result.refusals).toEqual([])
    expect(result.state.phase).toBe("idle")
    expect(result.state.candidate).toBeNull()
    expect(result.state.previous).toBeNull()
    expect(result.state.lastFailure).toEqual({ kind: "artifact_rejected" })
  })

  test("manifest rejection records the typed reason and returns to idle", () => {
    const result = runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
      { type: "check_started" },
      { type: "manifest_rejected", reason: "signature_invalid" },
    ])
    expect(result.state.phase).toBe("idle")
    expect(result.state.lastFailure).toEqual({
      kind: "manifest_rejected",
      reason: "signature_invalid",
    })
  })
})

describe("interruption", () => {
  test("interrupt during download is a loss-free discard", () => {
    const result = runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
      { type: "interrupted" },
    ])
    expect(result.state.phase).toBe("idle")
    expect(result.state.candidate).toBeNull()
    expect(result.state.installed).toBe("0.1.0-rc.1")
    expect(result.state.lastFailure).toEqual({ kind: "interrupted", during: "downloading" })
  })

  test("interrupt while staged keeps the durable staged slot and its ledger", () => {
    const staged = happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc"))
    const result = updateReducer(staged.state, { type: "interrupted" })
    expect(result.refusal).toBeNull()
    expect(result.state.phase).toBe("staged")
    expect(result.state.candidate?.version).toBe("0.1.0-rc.2")
    expect(result.state.ledger.sessions).toEqual({ status: "preserved" })
    // Resume: apply still works after the interruption.
    const applied = runUpdateEvents(result.state, [
      { type: "apply_requested" },
      { type: "apply_succeeded" },
      { type: "launch_receipt_recorded", version: "0.1.0-rc.2" },
    ])
    expect(applied.refusals).toEqual([])
    expect(applied.state.installed).toBe("0.1.0-rc.2")
    expect(applied.state.phase).toBe("idle")
  })

  test("interrupt during apply rolls back to the retained previous version", () => {
    const staged = happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc"))
    const result = runUpdateEvents(staged.state, [
      { type: "apply_requested" },
      { type: "interrupted" },
      { type: "rollback_completed" },
    ])
    expect(result.refusals).toEqual([])
    expect(result.state.phase).toBe("idle")
    expect(result.state.installed).toBe("0.1.0-rc.1")
    expect(result.state.previous).toBeNull() // slot consumed by the rollback
    expect(result.state.candidate).toBeNull()
  })
})

describe("rollback", () => {
  test("apply failure rolls back to exactly the retained previous version", () => {
    const staged = happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc"))
    const result = runUpdateEvents(staged.state, [
      { type: "apply_requested" },
      { type: "apply_failed" },
      { type: "rollback_completed" },
    ])
    expect(result.refusals).toEqual([])
    expect(result.state.installed).toBe("0.1.0-rc.1")
    expect(result.state.phase).toBe("idle")
  })

  test("manual post-apply rollback works once, then the slot is consumed", () => {
    const staged = happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc"))
    const applied = runUpdateEvents(staged.state, [
      { type: "apply_requested" },
      { type: "apply_succeeded" },
      { type: "launch_receipt_recorded", version: "0.1.0-rc.2" },
    ])
    const rolledBack = runUpdateEvents(applied.state, [
      { type: "rollback_requested" },
      { type: "rollback_completed" },
    ])
    expect(rolledBack.refusals).toEqual([])
    expect(rolledBack.state.installed).toBe("0.1.0-rc.1")
    // Second rollback: no slot left, refused.
    const second = updateReducer(rolledBack.state, { type: "rollback_requested" })
    expect(second.refusal).toBe("no_previous_release_retained")
    expect(second.state).toEqual(rolledBack.state)
  })

  test("rollback without a retained previous release is refused", () => {
    const fresh = initialUpdateState("0.1.0-rc.1", "rc")
    const result = updateReducer(fresh, { type: "rollback_requested" })
    expect(result.refusal).toBe("no_previous_release_retained")
    expect(result.state).toEqual(fresh)
  })

  test("a failed rollback lands in the honest rollback_failed phase, never a fake success", () => {
    const staged = happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc"))
    const result = runUpdateEvents(staged.state, [
      { type: "apply_requested" },
      { type: "apply_failed" },
      { type: "rollback_failed" },
    ])
    expect(result.refusals).toEqual([])
    expect(result.state.phase).toBe("rollback_failed")
    expect(result.state.lastFailure).toEqual({ kind: "rollback_failed" })
    // The machine does not silently claim any installed-version change.
    expect(result.state.installed).toBe("0.1.0-rc.1")
  })
})

describe("totality: illegal events never throw and never mutate", () => {
  test("every event against every phase-representative state is total", () => {
    const representatives: Array<UpdateMachineState> = [
      initialUpdateState("0.1.0-rc.1", "rc"),
      runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [{ type: "check_started" }]).state,
      runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
        { type: "check_started" },
        { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
      ]).state,
      happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc")).state,
      runUpdateEvents(happyPathToStaged(initialUpdateState("0.1.0-rc.1", "rc")).state, [
        { type: "apply_requested" },
        { type: "apply_succeeded" },
      ]).state,
    ]
    const events: Array<UpdateEvent> = [
      { type: "check_started" },
      { type: "manifest_verified", manifest: manifest("0.1.0-rc.9") },
      { type: "manifest_rejected", reason: "kid_not_pinned" },
      { type: "artifact_verified" },
      { type: "artifact_rejected" },
      { type: "migration_recorded", category: "sessions", disposition: { status: "preserved" } },
      { type: "staged" },
      { type: "apply_requested" },
      { type: "apply_succeeded" },
      { type: "apply_failed" },
      { type: "launch_receipt_recorded", version: "0.1.0-rc.2" },
      { type: "launch_receipt_recorded", version: "0.0.1" },
      { type: "launch_receipt_window_elapsed", problem: "receipt_missing" },
      { type: "rollback_requested" },
      { type: "rollback_completed" },
      { type: "rollback_failed" },
      { type: "interrupted" },
    ]
    for (const state of representatives) {
      for (const event of events) {
        const transition = updateReducer(state, event)
        expect(updatePhases).toContain(transition.state.phase)
        if (transition.refusal !== null) {
          // A refused event NEVER changes the state.
          expect(transition.state).toEqual(state)
        }
      }
    }
  })
})
