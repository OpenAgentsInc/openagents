import { describe, expect, test } from "bun:test"
import {
  PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION,
  publicActivityTimelineCursorForEvent,
  publicActivityTimelineLiveAtReadStaleness,
  type PublicActivityTimelineEnvelope,
  type PublicActivityTimelineEvent,
} from "@openagentsinc/public-activity-timeline"

import { fetchPublicActivityTimeline } from "../src/bun/public-activity-timeline"

const event = (
  input: Omit<PublicActivityTimelineEvent, "cursor">,
): PublicActivityTimelineEvent => ({
  ...input,
  cursor: publicActivityTimelineCursorForEvent(input),
})

const realBitcoinTimelineFixture: PublicActivityTimelineEnvelope = {
  schemaVersion: PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION,
  generatedAt: "2026-06-18T00:00:00.000Z",
  staleness: publicActivityTimelineLiveAtReadStaleness([
    "pylon.presence",
    "training.receipts",
    "forum.activity",
  ]),
  nextCursor: null,
  sourceLag: [
    {
      sourceKind: "forum",
      status: "stale",
      latestSourceEventAt: null,
      observedAt: "2026-06-18T00:00:00.000Z",
      lagSeconds: null,
      maxStalenessSeconds: 30,
      sourceRefs: ["forum.activity.public.1"],
      blockerRefs: ["blocker.public.activity_timeline.source_lag.forum"],
      caveatRefs: ["caveat.public.activity_timeline.source_lag"],
    },
  ],
  events: [
    event({
      eventRef: "activity.training.settlement.1",
      ts: "2026-06-18T00:00:01.000Z",
      kind: "real_bitcoin_moved",
      sourceKind: "settlement_receipt",
      runRef: "run.cs336.a1.demo",
      refs: ["receipt.public.real.1"],
      sourceRefs: ["receipt.public.real.1"],
      blockerRefs: [],
      caveatRefs: [],
      amountSats: 2100,
      realBitcoinMoved: true,
      state: "settled",
      text: "Receipt-backed real Bitcoin movement confirmed.",
    }),
    event({
      eventRef: "activity.forum.topic.1",
      ts: "2026-06-18T00:00:02.000Z",
      kind: "forum_topic_created",
      sourceKind: "forum",
      refs: ["forum.topic.public.1"],
      sourceRefs: ["forum.topic.public.1"],
      blockerRefs: [],
      caveatRefs: ["caveat.public.activity_timeline.source_lag"],
      state: "posted",
      text: "Public Forum topic created.",
    }),
  ],
}

describe("fetchPublicActivityTimeline", () => {
  test("loads and validates the public activity timeline contract", async () => {
    const seen: string[] = []
    const result = await fetchPublicActivityTimeline({
      baseUrl: "https://openagents.test/",
      limit: 7,
      nowIso: () => "2026-06-18T00:00:00.000Z",
      fetchFn: async (url) => {
        seen.push(String(url))
        return new Response(JSON.stringify(realBitcoinTimelineFixture))
      },
    })

    expect(result.ok).toBe(true)
    expect(seen).toEqual([
      "https://openagents.test/api/public/activity-timeline?limit=7",
    ])
    expect(result.sourceUrl).toBe(
      "https://openagents.test/api/public/activity-timeline?limit=7",
    )
    expect(result.fetchedAt).toBe("2026-06-18T00:00:00.000Z")
    expect(result.envelope?.schemaVersion).toBe(
      "openagents.public_activity_timeline.v1",
    )
    expect(result.envelope?.events.some(event => event.kind === "real_bitcoin_moved")).toBe(
      true,
    )
  })

  test("returns an unavailable projection when the envelope contains private material", async () => {
    const unsafe = {
      ...realBitcoinTimelineFixture,
      events: realBitcoinTimelineFixture.events.map((event, index) =>
        index === 0
          ? {
              ...event,
              text: "raw local path /Users/example/.secrets/token.json",
            }
          : event,
      ),
    }

    const result = await fetchPublicActivityTimeline({
      baseUrl: "https://openagents.test",
      fetchFn: async () => new Response(JSON.stringify(unsafe)),
    })

    expect(result.ok).toBe(false)
    expect(result.envelope).toBeNull()
    expect(result.error).toContain("raw/private material")
  })

  test("returns a typed unavailable projection on HTTP failure", async () => {
    const result = await fetchPublicActivityTimeline({
      baseUrl: "https://openagents.test",
      fetchFn: async () => new Response("nope", { status: 503 }),
    })

    expect(result.ok).toBe(false)
    expect(result.envelope).toBeNull()
    expect(result.error).toBe("public activity timeline 503")
  })
})
