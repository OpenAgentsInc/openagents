import { describe, expect, test } from "bun:test"
import {
  appendStructuredEvent,
  createStructuredEventLog,
  emptyStructuredEventLog,
  projectForAudience,
  type ProjectionAudience,
  type StructuredEvent,
} from "../src/tas/event-log-projection"

const event = (
  sequence: number,
  projectionLevel: ProjectionAudience,
  overrides: Partial<StructuredEvent> = {},
): StructuredEvent => ({
  eventId: `event.fixture.${sequence}`,
  sequence,
  kind: "status.changed",
  subjectRef: "run.fixture.structured_event_log",
  at: `2026-06-13T00:00:0${sequence}.000Z`,
  projectionLevel,
  detailRef: `detail.fixture.${sequence}`,
  ...overrides,
})

describe("structured event log", () => {
  test("appends monotonic events and deduplicates by event id", () => {
    const first = appendStructuredEvent(emptyStructuredEventLog(), event(1, "public"))
    const duplicate = appendStructuredEvent(
      first.log,
      event(1, "public", { eventId: "event.fixture.1" }),
    )
    const gap = appendStructuredEvent(first.log, event(3, "public"))
    const second = appendStructuredEvent(first.log, event(2, "companion"))

    expect(first).toMatchObject({ appended: true, reason: "appended" })
    expect(duplicate).toMatchObject({ appended: false, reason: "duplicate" })
    expect(gap).toMatchObject({ appended: false, reason: "sequence_gap" })
    expect(second).toMatchObject({ appended: true, reason: "appended" })
    expect(second.log.events.map(({ sequence }) => sequence)).toEqual([1, 2])
    expect(second.log.lastSequence).toBe(2)
  })

  test("projects redacted event streams per audience", () => {
    const log = createStructuredEventLog([
      event(1, "public", { kind: "receipt.created" }),
      event(2, "companion", { kind: "approval.requested" }),
      event(3, "api", { kind: "tool.result" }),
      event(4, "pylon", { kind: "file.edited" }),
    ])

    const publicProjection = projectForAudience(log.events, "public")
    const companionProjection = projectForAudience(log.events, "companion")
    const apiProjection = projectForAudience(log.events, "api")
    const pylonProjection = projectForAudience(log.events, "pylon")

    expect(publicProjection).toEqual([
      {
        eventId: "event.fixture.1",
        sequence: 1,
        kind: "receipt.created",
        subjectRef: "run.fixture.structured_event_log",
        at: "2026-06-13T00:00:01.000Z",
        projectionLevel: "public",
      },
    ])
    expect(JSON.stringify(publicProjection)).not.toContain("detail.fixture")
    expect(companionProjection.map(({ sequence }) => sequence)).toEqual([1, 2])
    expect(apiProjection.map(({ sequence }) => sequence)).toEqual([1, 2, 3])
    expect(pylonProjection.map(({ sequence }) => sequence)).toEqual([1, 2, 3, 4])
    expect(pylonProjection[3]?.detailRef).toBe("detail.fixture.4")
  })

  test("replay projections are deterministic for a fixed event stream", () => {
    const unorderedEvents = [
      event(3, "api", { eventId: "event.fixture.c", kind: "tool.result" }),
      event(1, "public", { eventId: "event.fixture.a", kind: "status.changed" }),
      event(2, "companion", { eventId: "event.fixture.b", kind: "approval.requested" }),
    ]

    const firstReplay = projectForAudience(unorderedEvents, "api")
    const secondReplay = projectForAudience(unorderedEvents, "api")

    expect(firstReplay).toEqual(secondReplay)
    expect(firstReplay.map(({ eventId }) => eventId)).toEqual([
      "event.fixture.a",
      "event.fixture.b",
      "event.fixture.c",
    ])
    expect(unorderedEvents.map(({ eventId }) => eventId)).toEqual([
      "event.fixture.c",
      "event.fixture.a",
      "event.fixture.b",
    ])
  })
})
