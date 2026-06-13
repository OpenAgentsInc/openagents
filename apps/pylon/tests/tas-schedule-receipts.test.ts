import { describe, expect, test } from "bun:test"
import {
  buildScheduleReceipt,
  createScheduleFiringState,
  recordFiring,
  type ScheduleReceipt,
} from "../src/tas/schedule-receipts"

const receiptKeys: Array<keyof ScheduleReceipt> = [
  "blockerRef",
  "continuationRef",
  "duplicate",
  "fireKeyRef",
  "kind",
  "nextRunRef",
  "receiptRef",
  "runRef",
  "scheduleRef",
]

describe("schedule receipts", () => {
  test("records the first firing for a schedule fireKey", () => {
    const result = recordFiring(createScheduleFiringState(), {
      scheduleId: "schedule.fixture.nightly",
      fireKey: "fire.fixture.2026-06-13T03:00:00Z",
    })

    expect(result.recorded).toBe(true)
    expect(result.duplicate).toBe(false)
    expect(
      result.state.fireKeysByScheduleId
        .get("schedule.fixture.nightly")
        ?.has("fire.fixture.2026-06-13T03:00:00Z"),
    ).toBe(true)
  })

  test("treats a duplicate fireKey as an idempotent no-op and flags it", () => {
    const first = recordFiring(createScheduleFiringState(), {
      scheduleId: "schedule.fixture.nightly",
      fireKey: "fire.fixture.2026-06-13T03:00:00Z",
    })
    const duplicate = recordFiring(first.state, {
      scheduleId: "schedule.fixture.nightly",
      fireKey: "fire.fixture.2026-06-13T03:00:00Z",
    })

    expect(duplicate.recorded).toBe(false)
    expect(duplicate.duplicate).toBe(true)
    expect(duplicate.state).toBe(first.state)
    expect(duplicate.state.fireKeysByScheduleId.get("schedule.fixture.nightly")?.size).toBe(1)
  })

  test("builds skip and continue receipts", () => {
    const skipped = buildScheduleReceipt({
      kind: "skipped",
      scheduleId: "schedule.fixture.overnight",
      fireKey: "fire.fixture.2026-06-13T04:00:00Z",
      blockerRef: "blocker.fixture.budget_expired",
      receiptRef: "receipt.fixture.skip",
    })
    const continued = buildScheduleReceipt({
      kind: "continued",
      scheduleId: "schedule.fixture.overnight",
      fireKey: "fire.fixture.2026-06-13T05:00:00Z",
      continuationRef: "continuation.fixture.resume01",
      runRef: "run.fixture.continued01",
      receiptRef: "receipt.fixture.continue",
    })

    expect(skipped).toEqual({
      kind: "skipped",
      scheduleRef: "schedule.fixture.overnight",
      receiptRef: "receipt.fixture.skip",
      fireKeyRef: "fire.fixture.2026-06-13T04:00:00Z",
      nextRunRef: null,
      runRef: null,
      blockerRef: "blocker.fixture.budget_expired",
      continuationRef: null,
      duplicate: false,
    })
    expect(continued).toEqual({
      kind: "continued",
      scheduleRef: "schedule.fixture.overnight",
      receiptRef: "receipt.fixture.continue",
      fireKeyRef: "fire.fixture.2026-06-13T05:00:00Z",
      nextRunRef: null,
      runRef: "run.fixture.continued01",
      blockerRef: null,
      continuationRef: "continuation.fixture.resume01",
      duplicate: false,
    })
  })

  test("builds refs-only receipts", () => {
    const rawGoal = "open /Users/example/private/repo and deploy the branch"
    const receipt = buildScheduleReceipt({
      kind: "fired",
      scheduleId: "schedule.fixture.refs_only",
      fireKey: "fire.fixture.refs_only",
      runRef: "run.fixture.refs_only",
      receiptRef: "receipt.fixture.refs_only",
      duplicate: true,
    })

    expect(Object.keys(receipt).sort()).toEqual([...receiptKeys].sort())
    expect(JSON.stringify(receipt)).not.toContain(rawGoal)
  })
})
