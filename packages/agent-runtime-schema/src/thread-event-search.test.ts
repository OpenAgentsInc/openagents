import { describe, expect, test } from "vite-plus/test";

import {
  searchCanonicalThreadEvents,
  ThreadEventSearchProjectionSchemaLiteral,
} from "./thread-event-search.js";

const artifact = (overrides: Record<string, unknown> = {}) => ({
  schema: "openagents.thread_export_artifact.v1",
  intentRef: "intent.search.1",
  threadRef: "thread.search.1",
  format: "canonical_event_bundle",
  artifactAudience: { kind: "owner_only" },
  events: [
    {
      eventRef: "event.accepted.1",
      sequence: 1,
      authority: { state: "accepted", relationRefs: ["relation.accepted.1"] },
      data: { item: { text: "First accepted answer about the kernel." } },
    },
    {
      eventRef: "event.superseded.1",
      sequence: 2,
      authority: {
        state: "superseded",
        relationRefs: ["relation.accepted.2", "relation.superseded.2"],
        supersededByEventRef: "event.replacement.1",
      },
      data: { item: { text: "Original accepted release guidance." } },
    },
    {
      eventRef: "event.reverted.1",
      sequence: 3,
      authority: {
        state: "reverted",
        relationRefs: ["relation.accepted.3", "relation.reverted.3"],
        revertedByEventRef: "event.revert.1",
        restoredEventRef: "event.accepted.1",
      },
      data: { item: { text: "A deployment instruction later reverted." } },
    },
  ],
  ...overrides,
});

describe("canonical accepted-event search projection", () => {
  test("lands on the original exact event and preserves supersession state", () => {
    const result = searchCanonicalThreadEvents({ artifacts: [artifact()], query: "release guidance" });
    expect(result).toEqual({
      schema: ThreadEventSearchProjectionSchemaLiteral,
      query: "release guidance",
      results: [
        {
          threadRef: "thread.search.1",
          eventRef: "event.superseded.1",
          sequence: 2,
          authority: {
            state: "superseded",
            relationRefs: ["relation.accepted.2", "relation.superseded.2"],
            supersededByEventRef: "event.replacement.1",
          },
          snippet: "Original accepted release guidance.",
          score: 1,
        },
      ],
      indexedEvents: 3,
      totalMatches: 1,
      indexTruncated: false,
      resultsTruncated: false,
    });
  });

  test("preserves reverted originals and returns no event body or synthesized replacement", () => {
    const result = searchCanonicalThreadEvents({ artifacts: [artifact()], query: "deployment" });
    expect(result.results[0]).toMatchObject({
      eventRef: "event.reverted.1",
      authority: {
        state: "reverted",
        revertedByEventRef: "event.revert.1",
        restoredEventRef: "event.accepted.1",
      },
    });
    expect(Object.keys(result.results[0]!)).toEqual([
      "threadRef",
      "eventRef",
      "sequence",
      "authority",
      "snippet",
      "score",
    ]);
  });

  test("ranks exact and prefix matches deterministically across canonical thread order", () => {
    const second = artifact({
      intentRef: "intent.search.2",
      threadRef: "thread.search.0",
      events: [
        {
          eventRef: "event.exact.1",
          sequence: 9,
          authority: { state: "accepted", relationRefs: ["relation.exact.1"] },
          data: { text: "kernel" },
        },
      ],
    });
    const first = artifact({
      threadRef: "thread.search.2",
      events: [
        {
          eventRef: "event.prefix.1",
          sequence: 1,
          authority: { state: "accepted", relationRefs: ["relation.prefix.1"] },
          data: { text: "Kernel scheduling" },
        },
      ],
    });
    const result = searchCanonicalThreadEvents({ artifacts: [first, second], query: " kernel " });
    expect(result.query).toBe("kernel");
    expect(result.results.map(({ eventRef, score }) => [eventRef, score])).toEqual([
      ["event.exact.1", 3],
      ["event.prefix.1", 2],
    ]);
  });

  test("fails closed on malformed, duplicate, or invalid authority input", () => {
    expect(() => searchCanonicalThreadEvents({ artifacts: [{ nope: true }], query: "x" })).toThrow();
    expect(() =>
      searchCanonicalThreadEvents({ artifacts: [artifact(), artifact()], query: "x" }),
    ).toThrow("duplicate thread artifacts");
    expect(() =>
      searchCanonicalThreadEvents({
        artifacts: [
          artifact({
            events: [
              {
                eventRef: "event.self.1",
                sequence: 1,
                authority: {
                  state: "superseded",
                  relationRefs: ["relation.self.1"],
                  supersededByEventRef: "event.self.1",
                },
                data: { text: "x" },
              },
            ],
          }),
        ],
        query: "x",
      }),
    ).toThrow("self-supersession");
  });

  test("reports result and per-event index truncation explicitly", () => {
    const leaves = Object.fromEntries(
      Array.from({ length: 257 }, (_, index) => [`field${String(index).padStart(3, "0")}`, "match"]),
    );
    const result = searchCanonicalThreadEvents({
      artifacts: [
        artifact({
          events: [
            {
              eventRef: "event.bounded.1",
              sequence: 1,
              authority: { state: "accepted", relationRefs: ["relation.bounded.1"] },
              data: leaves,
            },
            {
              eventRef: "event.bounded.2",
              sequence: 2,
              authority: { state: "accepted", relationRefs: ["relation.bounded.2"] },
              data: { text: "match" },
            },
          ],
        }),
      ],
      query: "match",
      limit: 1,
    });
    expect(result).toMatchObject({
      indexedEvents: 2,
      totalMatches: 2,
      indexTruncated: true,
      resultsTruncated: true,
    });
    expect(result.results).toHaveLength(1);
  });

  test("does not scan authority-bearing bytes for a blank query", () => {
    expect(searchCanonicalThreadEvents({ artifacts: [{ invalid: true }], query: "  " })).toEqual({
      schema: ThreadEventSearchProjectionSchemaLiteral,
      query: "",
      results: [],
      indexedEvents: 0,
      totalMatches: 0,
      indexTruncated: false,
      resultsTruncated: false,
    });
  });
});
