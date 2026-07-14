/**
 * Post-update first-launch receipt oracles (DMG-1, #8786).
 *
 * From the 2026-07-13 ChatGPT updater incident
 * (docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md): applying
 * an update is NOT success — the first demonstrated launch of the new build
 * is. These oracles prove the three named cases (marker present, absent,
 * late) plus the fail-closed decode/version rules, entirely against the pure
 * contract and reducer — no live update feed, no clock, no Electron.
 */
import { describe, expect, test } from "vite-plus/test"
import {
  LAUNCH_RECEIPT_SCHEMA_ID,
  LAUNCH_RECEIPT_WINDOW_MS,
  type UpdateManifest,
  UPDATE_CONTRACT_SCHEMA_ID,
  createLaunchReceipt,
  evaluateLaunchReceipt,
} from "../src/update-contract.ts"
import {
  type UpdateEvent,
  type UpdateMachineState,
  initialUpdateState,
  migrationCategories,
  runUpdateEvents,
  updateReducer,
} from "../src/update-rollback.ts"

const manifest = (version: string): UpdateManifest => ({
  schema: UPDATE_CONTRACT_SCHEMA_ID,
  app: "openagents-desktop",
  channel: "rc",
  version,
  artifactName: `OpenAgents-${version}-arm64.dmg`,
  artifactSha256: "ab".repeat(32),
  artifactByteLength: 1024,
  releasedAt: "2026-07-13T00:00:00Z",
})

const appliedState = (): UpdateMachineState =>
  runUpdateEvents(initialUpdateState("0.1.0-rc.1", "rc"), [
    { type: "check_started" },
    { type: "manifest_verified", manifest: manifest("0.1.0-rc.2") },
    { type: "artifact_verified" },
    ...migrationCategories.map((category) => ({
      type: "migration_recorded" as const,
      category,
      disposition: { status: "preserved" as const },
    })),
    { type: "staged" },
    { type: "apply_requested" },
    { type: "apply_succeeded" },
  ]).state

const APPLIED_AT = 1_000_000

describe("evaluateLaunchReceipt (pure, clock-free)", () => {
  test("marker PRESENT and matching confirms the update", () => {
    const receipt = createLaunchReceipt("0.1.0-rc.2", "2026-07-13T12:00:00Z")
    expect(
      evaluateLaunchReceipt({
        receipt,
        expectedVersion: "0.1.0-rc.2",
        appliedAtMs: APPLIED_AT,
        nowMs: APPLIED_AT + 5_000,
      }),
    ).toEqual({ outcome: "confirmed" })
    // Confirmation is time-independent — a valid receipt read even after the
    // window still confirms (the machine was never told to roll back).
    expect(
      evaluateLaunchReceipt({
        receipt,
        expectedVersion: "0.1.0-rc.2",
        appliedAtMs: APPLIED_AT,
        nowMs: APPLIED_AT + LAUNCH_RECEIPT_WINDOW_MS * 2,
      }).outcome,
    ).toBe("confirmed")
  })

  test("marker ABSENT: awaiting inside the bounded window, rollback_required after it", () => {
    const open = evaluateLaunchReceipt({
      receipt: null,
      expectedVersion: "0.1.0-rc.2",
      appliedAtMs: APPLIED_AT,
      nowMs: APPLIED_AT + 1_000,
    })
    expect(open).toEqual({
      outcome: "awaiting",
      problem: "receipt_missing",
      remainingMs: LAUNCH_RECEIPT_WINDOW_MS - 1_000,
    })

    const elapsed = evaluateLaunchReceipt({
      receipt: null,
      expectedVersion: "0.1.0-rc.2",
      appliedAtMs: APPLIED_AT,
      nowMs: APPLIED_AT + LAUNCH_RECEIPT_WINDOW_MS,
    })
    expect(elapsed).toEqual({ outcome: "rollback_required", problem: "receipt_missing" })
  })

  test("an undecodable marker is never launch evidence (fail closed)", () => {
    const result = evaluateLaunchReceipt({
      receipt: { schema: "garbage", version: 42 },
      expectedVersion: "0.1.0-rc.2",
      appliedAtMs: APPLIED_AT,
      nowMs: APPLIED_AT + LAUNCH_RECEIPT_WINDOW_MS + 1,
    })
    expect(result).toEqual({ outcome: "rollback_required", problem: "receipt_invalid" })
  })

  test("a stale marker from the PREVIOUS build is a version mismatch, not confirmation", () => {
    const stale = createLaunchReceipt("0.1.0-rc.1", "2026-07-12T00:00:00Z")
    const inWindow = evaluateLaunchReceipt({
      receipt: stale,
      expectedVersion: "0.1.0-rc.2",
      appliedAtMs: APPLIED_AT,
      nowMs: APPLIED_AT + 1,
    })
    expect(inWindow.outcome).toBe("awaiting")
    expect(inWindow.outcome === "awaiting" && inWindow.problem).toBe("receipt_version_mismatch")

    const afterWindow = evaluateLaunchReceipt({
      receipt: stale,
      expectedVersion: "0.1.0-rc.2",
      appliedAtMs: APPLIED_AT,
      nowMs: APPLIED_AT + LAUNCH_RECEIPT_WINDOW_MS + 1,
    })
    expect(afterWindow).toEqual({ outcome: "rollback_required", problem: "receipt_version_mismatch" })
  })

  test("the receipt document schema is exact", () => {
    const receipt = createLaunchReceipt("0.1.0-rc.2", "2026-07-13T12:00:00Z")
    expect(receipt.schema).toBe(LAUNCH_RECEIPT_SCHEMA_ID)
    expect(receipt.app).toBe("openagents-desktop")
  })
})

describe("launch-receipt state machine (marker present / absent / late)", () => {
  test("PRESENT: receipt for exactly the applied version confirms; previous stays retained", () => {
    const state = appliedState()
    expect(state.phase).toBe("awaiting_launch_receipt")
    expect(state.installed).toBe("0.1.0-rc.2")
    expect(state.previous).toEqual({ version: "0.1.0-rc.1" })

    const confirmed = updateReducer(state, { type: "launch_receipt_recorded", version: "0.1.0-rc.2" })
    expect(confirmed.refusal).toBeNull()
    expect(confirmed.state.phase).toBe("idle")
    expect(confirmed.state.installed).toBe("0.1.0-rc.2")
    expect(confirmed.state.previous).toEqual({ version: "0.1.0-rc.1" })
    expect(confirmed.state.lastFailure).toBeNull()
  })

  test("ABSENT: elapsed window rolls back AUTOMATICALLY to the retained previous version with a diagnostic", () => {
    const result = runUpdateEvents(appliedState(), [
      { type: "launch_receipt_window_elapsed", problem: "receipt_missing" },
      { type: "rollback_completed" },
    ])
    expect(result.refusals).toEqual([])
    expect(result.state.phase).toBe("idle")
    expect(result.state.installed).toBe("0.1.0-rc.1")
    expect(result.state.previous).toBeNull() // slot consumed by the rollback
    // The diagnostic names what happened — never a silent dead update.
    expect(result.state.lastFailure).toEqual({
      kind: "launch_receipt_missing",
      problem: "receipt_missing",
      appliedVersion: "0.1.0-rc.2",
    })
  })

  test("LATE: a receipt arriving after the window elapsed never resurrects the rolled-back update", () => {
    const rollingBack = runUpdateEvents(appliedState(), [
      { type: "launch_receipt_window_elapsed", problem: "receipt_missing" },
    ]).state
    expect(rollingBack.phase).toBe("rolling_back")

    const lateDuringRollback = updateReducer(rollingBack, {
      type: "launch_receipt_recorded",
      version: "0.1.0-rc.2",
    })
    expect(lateDuringRollback.refusal).toBe("event_not_admissible_in_phase")
    expect(lateDuringRollback.state).toEqual(rollingBack)

    const rolledBack = updateReducer(rollingBack, { type: "rollback_completed" }).state
    const lateAfterRollback = updateReducer(rolledBack, {
      type: "launch_receipt_recorded",
      version: "0.1.0-rc.2",
    })
    expect(lateAfterRollback.refusal).toBe("event_not_admissible_in_phase")
    expect(lateAfterRollback.state).toEqual(rolledBack)
    expect(lateAfterRollback.state.installed).toBe("0.1.0-rc.1")
  })

  test("a receipt for the WRONG version is refused and the machine keeps waiting", () => {
    const state = appliedState()
    const wrong = updateReducer(state, { type: "launch_receipt_recorded", version: "0.1.0-rc.1" })
    expect(wrong.refusal).toBe("launch_receipt_version_mismatch")
    expect(wrong.state).toEqual(state)
  })

  test("interruption while awaiting the receipt is durable — the window survives a crash/relaunch", () => {
    const state = appliedState()
    const interrupted = updateReducer(state, { type: "interrupted" })
    expect(interrupted.refusal).toBeNull()
    expect(interrupted.state.phase).toBe("awaiting_launch_receipt")
    expect(interrupted.state.previous).toEqual({ version: "0.1.0-rc.1" })
    expect(interrupted.state.lastFailure).toEqual({
      kind: "interrupted",
      during: "awaiting_launch_receipt",
    })
    // Resume: the receipt still confirms after the interruption.
    const confirmed = updateReducer(interrupted.state, {
      type: "launch_receipt_recorded",
      version: "0.1.0-rc.2",
    })
    expect(confirmed.refusal).toBeNull()
    expect(confirmed.state.phase).toBe("idle")
  })

  test("window-elapsed is only admissible while awaiting the receipt", () => {
    const idle = initialUpdateState("0.1.0-rc.1", "rc")
    const event: UpdateEvent = { type: "launch_receipt_window_elapsed", problem: "receipt_missing" }
    const result = updateReducer(idle, event)
    expect(result.refusal).toBe("event_not_admissible_in_phase")
    expect(result.state).toEqual(idle)
  })
})
