import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadEventSearchChannel,
  decodeDesktopThreadEventSearchRequest,
  decodeDesktopThreadEventSearchResult,
  invokeDesktopThreadEventSearch,
} from "./thread-event-search-bridge-contract.ts";

const projection = {
  schema: "openagents.thread_event_search_projection.v1" as const,
  query: "accepted event",
  results: [
    {
      threadRef: "thread.search.bridge.1",
      eventRef: "event.search.bridge.1",
      sequence: 4,
      authority: {
        state: "accepted" as const,
        relationRefs: ["relation.search.bridge.1"],
      },
      snippet: "an accepted event",
      score: 2 as const,
    },
  ],
  indexedEvents: 2,
  totalMatches: 1,
  indexTruncated: false,
  resultsTruncated: false,
};

describe("Desktop canonical accepted-event search bridge", () => {
  test("admits only an exact bounded normalized request", () => {
    expect(decodeDesktopThreadEventSearchRequest({ query: "  accepted\n event  " })).toEqual({
      query: "accepted event",
    });
    expect(decodeDesktopThreadEventSearchRequest({ query: "event", limit: 25 })).toEqual({
      query: "event",
      limit: 25,
    });
    expect(decodeDesktopThreadEventSearchRequest({ query: "event", limit: 0 })).toBeNull();
    expect(decodeDesktopThreadEventSearchRequest({ query: "x".repeat(201) })).toBeNull();
    expect(
      decodeDesktopThreadEventSearchRequest({ query: "event", receiptRef: "receipt.private.1" }),
    ).toBeNull();
  });

  test("invokes one fixed channel with only the normalized query", async () => {
    const calls: unknown[] = [];
    const result = await invokeDesktopThreadEventSearch(
      async (channel, request) => {
        calls.push({ channel, request });
        return { status: "available", projection };
      },
      { query: " accepted\t event " },
    );

    expect(result).toEqual({ status: "available", projection });
    expect(calls).toEqual([
      {
        channel: DesktopThreadEventSearchChannel,
        request: { query: "accepted event" },
      },
    ]);
    expect(JSON.stringify(calls)).not.toContain("receipt");
    expect(JSON.stringify(calls)).not.toContain("artifact");
  });

  test("rejects invalid input before invocation", async () => {
    let invoked = false;
    const result = await invokeDesktopThreadEventSearch(
      async () => {
        invoked = true;
        return { status: "available", projection };
      },
      { query: "event", filePath: "/private/owner/export.json" },
    );

    expect(result).toEqual({ status: "unavailable", reason: "invalid_request" });
    expect(invoked).toBe(false);
  });

  test("decodes only exact bounded projection and unavailable outcomes", () => {
    expect(decodeDesktopThreadEventSearchResult({ status: "available", projection })).toEqual({
      status: "available",
      projection,
    });
    expect(
      decodeDesktopThreadEventSearchResult({
        status: "unavailable",
        reason: "artifact_unavailable",
      }),
    ).toEqual({ status: "unavailable", reason: "artifact_unavailable" });
    expect(
      decodeDesktopThreadEventSearchResult({
        status: "available",
        projection: { ...projection, artifactBytes: "private transcript" },
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadEventSearchResult({
        status: "available",
        projection: {
          ...projection,
          results: [{ ...projection.results[0], body: "private transcript" }],
        },
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadEventSearchResult({
        status: "unavailable",
        reason: "native_error",
      }),
    ).toBeNull();
  });

  test("rejects inconsistent or invalid authority projections", () => {
    expect(
      decodeDesktopThreadEventSearchResult({
        status: "available",
        projection: { ...projection, totalMatches: 0 },
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadEventSearchResult({
        status: "available",
        projection: {
          ...projection,
          results: [
            {
              ...projection.results[0],
              authority: {
                state: "superseded",
                relationRefs: ["relation.search.bridge.1"],
                supersededByEventRef: projection.results[0].eventRef,
              },
            },
          ],
        },
      }),
    ).toBeNull();
  });

  test("collapses thrown transports and malformed replies without details", async () => {
    await expect(
      invokeDesktopThreadEventSearch(
        async () => {
          throw new Error("/private/owner/thread-exports/catalog.json");
        },
        { query: "event" },
      ),
    ).resolves.toEqual({ status: "unavailable", reason: "transport_unavailable" });

    await expect(
      invokeDesktopThreadEventSearch(
        async () => ({
          status: "available",
          projection,
          receiptRef: "receipt.private.1",
        }),
        { query: "event" },
      ),
    ).resolves.toEqual({ status: "unavailable", reason: "transport_unavailable" });
  });
});
