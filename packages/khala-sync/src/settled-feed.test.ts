import { describe, expect, test } from "bun:test"
import {
  decodeSettledFeedEventEntity,
  decodeSettledFeedSummaryEntity,
  encodeSettledFeedEventEntity,
  encodeSettledFeedSummaryEntity,
  SETTLED_FEED_CHANNEL_ID,
  SETTLED_FEED_EVENT_ENTITY_TYPE,
  SETTLED_FEED_SUMMARY_ENTITY_ID,
  SETTLED_FEED_SUMMARY_ENTITY_TYPE,
} from "./settled-feed.js"

const nowIso = "2026-07-05T00:00:00.000Z"

const validEvent = {
  amountSats: 5,
  challengeRef: "challenge.tassadar.window.0001",
  contributorRef: "pylon.worker.orrery",
  eventRef: "settled.challenge_tassadar_window_0001.worker.0",
  party: "worker" as const,
  runRef: "run.tassadar.poc",
  settledAt: nowIso,
  totalSettledCount: 1,
  totalSettledSats: 5,
  windowRef: "window.tassadar.0001",
}

const validSummary = {
  latestEventRef: validEvent.eventRef,
  latestSettledAt: nowIso,
  totalSettledCount: 1,
  totalSettledSats: 5,
  updatedAt: nowIso,
}

describe("settled-feed event entity contract (KS-6.4)", () => {
  test("round-trips a valid event post-image", () => {
    const entity = decodeSettledFeedEventEntity(validEvent)
    expect(entity.eventRef).toBe(validEvent.eventRef)
    expect(entity.amountSats).toBe(5)
    expect(encodeSettledFeedEventEntity(entity)).toEqual(validEvent)
    expect(SETTLED_FEED_EVENT_ENTITY_TYPE).toBe("settled_feed_event")
    expect(SETTLED_FEED_CHANNEL_ID).toBe("settled-feed")
  })

  test("windowRef may be null", () => {
    const entity = decodeSettledFeedEventEntity({
      ...validEvent,
      windowRef: null,
    })
    expect(entity.windowRef).toBeNull()
  })

  test("party is bounded to worker | validator", () => {
    expect(() =>
      decodeSettledFeedEventEntity({ ...validEvent, party: "operator" }),
    ).toThrow()
  })

  test("structurally refuses filesystem paths, emails, whitespace, and empty refs", () => {
    // Bounded refs never contain a raw preimage or destination shape by
    // construction either, but this pattern's job (like the fleet
    // contracts' PUBLIC_REF_PATTERN) is only the structural slice — the
    // dedicated raw-payment-material scanner
    // (`assertSettledFeedPayloadPublicSafe`) is the belt-and-suspenders
    // second gate applied upstream before a post-image reaches this
    // contract; see its own tests in tassadar-settled-feed-sync.test.ts.
    for (const contributorRef of [
      "/Users/alice/secret",
      "alice@example.com",
      "with a space",
      "",
    ]) {
      expect(() =>
        decodeSettledFeedEventEntity({ ...validEvent, contributorRef }),
      ).toThrow()
    }
  })

  test("amounts/totals must be non-negative safe integers", () => {
    for (const amountSats of [-1, 1.5, Number.NaN]) {
      expect(() =>
        decodeSettledFeedEventEntity({ ...validEvent, amountSats }),
      ).toThrow()
    }
  })

  test("settledAt must be ISO-8601 UTC", () => {
    expect(() =>
      decodeSettledFeedEventEntity({ ...validEvent, settledAt: "not-a-date" }),
    ).toThrow()
  })
})

describe("settled-feed summary entity contract (KS-6.4)", () => {
  test("round-trips a valid summary post-image", () => {
    const entity = decodeSettledFeedSummaryEntity(validSummary)
    expect(entity.totalSettledCount).toBe(1)
    expect(encodeSettledFeedSummaryEntity(entity)).toEqual(validSummary)
    expect(SETTLED_FEED_SUMMARY_ENTITY_TYPE).toBe("settled_feed_summary")
    expect(SETTLED_FEED_SUMMARY_ENTITY_ID).toBe("summary")
  })

  test("latestEventRef/latestSettledAt may be null before any event", () => {
    const entity = decodeSettledFeedSummaryEntity({
      latestEventRef: null,
      latestSettledAt: null,
      totalSettledCount: 0,
      totalSettledSats: 0,
      updatedAt: nowIso,
    })
    expect(entity.latestEventRef).toBeNull()
    expect(entity.latestSettledAt).toBeNull()
  })
})
