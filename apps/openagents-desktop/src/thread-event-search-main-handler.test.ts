import { describe, expect, test } from "vite-plus/test";

import { DesktopThreadEventSearchChannel } from "./thread-event-search-bridge-contract.ts";
import {
  registerDesktopThreadEventSearchMainHandler,
  type DesktopThreadEventSearchMainHandler,
  type DesktopThreadEventSearchMainHandlerDependencies,
} from "./thread-event-search-main-handler.ts";

const projection = {
  schema: "openagents.thread_event_search_projection.v1" as const,
  query: "accepted event",
  results: [
    {
      threadRef: "thread.search.main.1",
      eventRef: "event.search.main.1",
      sequence: 3,
      authority: {
        state: "accepted" as const,
        relationRefs: ["relation.search.main.1"],
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

const openFixture = (overrides: Partial<DesktopThreadEventSearchMainHandlerDependencies> = {}) => {
  let handler: DesktopThreadEventSearchMainHandler | undefined;
  const channels: string[] = [];
  let unregisters = 0;
  const searches: unknown[] = [];
  const dependencies: DesktopThreadEventSearchMainHandlerDependencies = {
    register: (channel, value) => {
      channels.push(channel);
      handler = value;
      return () => {
        unregisters += 1;
      };
    },
    isTrustedSender: (event) => event === "trusted",
    search: async (request) => {
      searches.push(request);
      return { status: "available", projection };
    },
    ...overrides,
  };
  const registration = registerDesktopThreadEventSearchMainHandler(dependencies);
  if (handler === undefined) throw new Error("handler was not registered");
  return {
    channels,
    handler,
    registration,
    searches,
    get unregisters() {
      return unregisters;
    },
  };
};

describe("Desktop canonical accepted-event search main-process handler seam", () => {
  test("registers exactly the fixed channel and closes idempotently", async () => {
    const fixture = openFixture();
    expect(fixture.channels).toEqual([DesktopThreadEventSearchChannel]);

    fixture.registration.close();
    fixture.registration.close();
    expect(fixture.unregisters).toBe(1);
    await expect(fixture.handler("trusted", { query: "accepted event" })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
    expect(fixture.searches).toEqual([]);
  });

  test("rejects untrusted, throwing-trust, malformed, and broader input before search", async () => {
    const fixture = openFixture();
    await expect(fixture.handler("untrusted", { query: "accepted event" })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
    await expect(
      fixture.handler("trusted", { query: "accepted event", receiptRef: "receipt.private.1" }),
    ).resolves.toEqual({ status: "unavailable", reason: "invalid_request" });
    await expect(fixture.handler("trusted", { query: 4 })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
    const throwingTrust = openFixture({
      isTrustedSender: () => {
        throw new Error("native sender detail");
      },
    });
    await expect(throwingTrust.handler("trusted", { query: "accepted event" })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
    expect(fixture.searches).toEqual([]);
    expect(throwingTrust.searches).toEqual([]);
  });

  test("passes only the normalized bounded request and returns its query-bound projection", async () => {
    const fixture = openFixture();
    await expect(
      fixture.handler("trusted", { query: "  accepted\n event ", limit: 25 }),
    ).resolves.toEqual({ status: "available", projection });
    expect(fixture.searches).toEqual([{ query: "accepted event", limit: 25 }]);
    expect(JSON.stringify(fixture.searches)).not.toContain("receipt");
    expect(JSON.stringify(fixture.searches)).not.toContain("artifact");
  });

  test("preserves bounded acquisition rejection outcomes", async () => {
    for (const reason of [
      "artifact_unavailable",
      "artifact_corrupt",
      "identity_mismatch",
      "projection_rejected",
    ] as const) {
      const fixture = openFixture({
        search: async () => ({ status: "unavailable", reason }),
      });
      await expect(fixture.handler("trusted", { query: "accepted event" })).resolves.toEqual({
        status: "unavailable",
        reason,
      });
    }
  });

  test("collapses thrown, malformed, mismatched, and detail-leaking outcomes", async () => {
    const outputs: ReadonlyArray<unknown> = [
      { status: "available", projection: { ...projection, query: "other query" } },
      { status: "available", projection, receiptRef: "receipt.private.1" },
      { status: "available", projection: { ...projection, filePath: "/private/thread.json" } },
      { status: "unavailable", reason: "native_error" },
    ];
    for (const output of outputs) {
      const fixture = openFixture({ search: async () => output });
      await expect(fixture.handler("trusted", { query: "accepted event" })).resolves.toEqual({
        status: "unavailable",
        reason: "transport_unavailable",
      });
    }

    const throwing = openFixture({
      search: async () => {
        throw new Error("/private/thread-exports/catalog.json");
      },
    });
    await expect(throwing.handler("trusted", { query: "accepted event" })).resolves.toEqual({
      status: "unavailable",
      reason: "transport_unavailable",
    });
  });
});
