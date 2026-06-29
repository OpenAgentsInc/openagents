import { describe, expect, test } from "bun:test"

import {
  activeTimelineFixture,
  emptyTimelineFixture,
  publicActivityTimelineFixtures,
  realBitcoinTimelineFixture,
  replayRangeTimelineFixture,
  simulationOnlyTimelineFixture,
  staleTimelineFixture,
} from "./fixtures.js"
import {
  PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION,
  PUBLIC_ACTIVITY_TIMELINE_STALENESS_CONTRACT_VERSION,
  assertPublicActivityTimelineEnvelopeSafe,
  assertPublicActivityTimelineEventSafe,
  decodePublicActivityTimelineEnvelope,
  orderPublicActivityTimelineEvents,
  publicActivityTimelineCursorForEvent,
  publicActivityTimelineEventKinds,
  publicActivityTimelineSourceKinds,
} from "./index.js"

describe("@openagentsinc/public-activity-timeline", () => {
  test("decodes every fixture envelope with the v1 schema", () => {
    for (const fixture of publicActivityTimelineFixtures) {
      const decoded = decodePublicActivityTimelineEnvelope(fixture)
      expect(decoded.schemaVersion).toBe(PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION)
      expect(decoded.staleness.contractVersion).toBe(
        PUBLIC_ACTIVITY_TIMELINE_STALENESS_CONTRACT_VERSION,
      )
      expect(assertPublicActivityTimelineEnvelopeSafe(decoded)).toEqual(decoded)
    }
  })

  test("covers every finite launch event kind and source kind", () => {
    expect(publicActivityTimelineEventKinds).toEqual([
      "pylon_registered",
      "pylon_heartbeat",
      "wallet_ready",
      "assignment_ready",
      "window_opened",
      "window_closed",
      "work_claimed",
      "trace_submitted",
      "verification_queued",
      "verification_verified",
      "verification_rejected",
      "khala_inference_served",
      "settlement_recorded",
      "real_bitcoin_moved",
      "forum_topic_created",
      "forum_posted",
      "artanis_tick",
      "capacity_snapshot",
      "projection_gap",
    ])
    expect(publicActivityTimelineSourceKinds).toEqual([
      "pylon_api",
      "pylon_presence",
      "training_window",
      "training_trace",
      "training_verification",
      "inference_receipt",
      "settlement_receipt",
      "forum",
      "artanis",
      "capacity_funnel",
      "projection_gap",
    ])
  })

  test("orders events by cursor tuple: ts, source kind, event ref", () => {
    const unordered = [
      activeTimelineFixture.events[3],
      activeTimelineFixture.events[0],
      activeTimelineFixture.events[2],
      activeTimelineFixture.events[1],
    ].filter(item => item !== undefined)

    const ordered = orderPublicActivityTimelineEvents(unordered)
    expect(ordered.map(event => event?.eventRef)).toEqual([
      "event.public.pylon.registered.7",
      "event.public.pylon.heartbeat.7",
      "event.public.pylon.wallet_ready.7",
      "event.public.pylon.assignment_ready.7",
    ])

    for (const event of ordered) {
      expect(event?.cursor).toBe(publicActivityTimelineCursorForEvent(event))
    }
  })

  test("fixtures cover empty, active, stale, replay-range, simulation, and real-Bitcoin cases", () => {
    expect(emptyTimelineFixture.events).toHaveLength(0)
    expect(activeTimelineFixture.events.map(event => event.kind)).toContain(
      "pylon_registered",
    )
    expect(staleTimelineFixture.sourceLag[0]?.status).toBe("stale")
    expect(replayRangeTimelineFixture.range?.filterKinds).toEqual([
      "window_opened",
      "work_claimed",
      "trace_submitted",
      "verification_verified",
    ])
    expect(simulationOnlyTimelineFixture.events[0]?.realBitcoinMoved).toBe(false)
    expect(realBitcoinTimelineFixture.events.at(-1)?.kind).toBe(
      "real_bitcoin_moved",
    )
  })

  test("requires source refs or blocker refs for every event", () => {
    const unsafe = {
      ...activeTimelineFixture.events[0],
      sourceRefs: [],
      blockerRefs: [],
    }

    expect(() => assertPublicActivityTimelineEventSafe(unsafe)).toThrow(
      "sourceRefs or blockerRefs",
    )
  })

  test("rejects raw prompts, private paths, payment secrets, and provider material", () => {
    const unsafeValues = [
      { text: "raw_prompt: ship this private flow" },
      { refs: ["/Users/example/private-source"] },
      { sourceRefs: ["payment_preimage=abc123"] },
      { caveatRefs: ["provider_payload.raw.1"] },
    ]

    for (const value of unsafeValues) {
      expect(() =>
        assertPublicActivityTimelineEventSafe({
          ...activeTimelineFixture.events[0],
          ...value,
        }),
      ).toThrow("raw/private material")
    }
  })

  test("keeps simulation rows visible without emitting real bitcoin movement", () => {
    const [simulation] = simulationOnlyTimelineFixture.events

    expect(simulation?.kind).toBe("settlement_recorded")
    expect(simulation?.realBitcoinMoved).toBe(false)
    expect(
      simulationOnlyTimelineFixture.events.some(
        event => event.kind === "real_bitcoin_moved",
      ),
    ).toBe(false)
    expect(
      assertPublicActivityTimelineEnvelopeSafe(simulationOnlyTimelineFixture),
    ).toEqual(simulationOnlyTimelineFixture)
  })

  test("allows realBitcoinMoved only with receipt-backed source refs", () => {
    const realMovement = realBitcoinTimelineFixture.events.at(-1)
    expect(realMovement?.realBitcoinMoved).toBe(true)
    if (realMovement === undefined) {
      throw new Error("missing real Bitcoin fixture event")
    }
    expect(assertPublicActivityTimelineEventSafe(realMovement)).toEqual(realMovement)

    expect(() =>
      assertPublicActivityTimelineEventSafe({
        ...realMovement,
        sourceRefs: ["training.verification.challenge.public.no_receipt"],
      }),
    ).toThrow("requires a public receipt source ref")
  })

  test("requires Khala inference events to be receipt-backed", () => {
    const event = activeTimelineFixture.events.find(
      candidate => candidate.kind === "khala_inference_served",
    )
    if (event === undefined) {
      throw new Error("missing Khala fixture event")
    }

    expect(assertPublicActivityTimelineEventSafe(event)).toEqual(event)
    expect(() =>
      assertPublicActivityTimelineEventSafe({
        ...event,
        sourceRefs: ["source.public.no_receipt"],
      }),
    ).toThrow("require an inference receipt source ref")
  })

  test("requires projection-gap events and stale source lag to expose blockers", () => {
    expect(staleTimelineFixture.events[0]?.kind).toBe("projection_gap")
    expect(staleTimelineFixture.events[0]?.blockerRefs).toContain(
      "blocker.public.forum_activity_projection_lag",
    )

    expect(() =>
      assertPublicActivityTimelineEnvelopeSafe({
        ...staleTimelineFixture,
        sourceLag: [
          {
            ...staleTimelineFixture.sourceLag[0],
            sourceRefs: [],
            blockerRefs: [],
          },
        ],
      }),
    ).toThrow("source lag must expose sourceRefs or blockerRefs")
  })
})
