import { readFileSync } from "node:fs";
import { Effect, Schema } from "effect";
import { NostrEventSchema } from "@openagentsinc/nip-mkt";
import { describe, expect, test } from "vite-plus/test";

import {
  MKT_DISCOVERY_MAX_FRAME_BYTES,
  MKT_DISCOVERY_LIMIT,
  MKT_DISCOVERY_SUBSCRIPTION_ID,
  MktDiscoveryBook,
  mktDiscoveryClose,
  mktDiscoveryRequest,
  projectDiscoveredOfferings,
} from "./live-discovery.js";

const DiscoveryFixtureSchema = Schema.Struct({
  schema: Schema.Literal("openagents.mkt-swp.discovery-fixture.v1"),
  observed_at: Schema.Number,
  provider_old: NostrEventSchema,
  provider_new: NostrEventSchema,
  offering_old: NostrEventSchema,
  offering_tie_a: NostrEventSchema,
  offering_tie_b: NostrEventSchema,
});

const fixture = Schema.decodeUnknownSync(DiscoveryFixtureSchema)(
  JSON.parse(readFileSync(new URL("../fixtures/live-discovery-v1.json", import.meta.url), "utf8")),
);

const eventFrame = (event: typeof NostrEventSchema.Type): string =>
  JSON.stringify(["EVENT", MKT_DISCOVERY_SUBSCRIPTION_ID, event]);

const eoseFrame = JSON.stringify(["EOSE", MKT_DISCOVERY_SUBSCRIPTION_ID]);

describe("transport-neutral MKT discovery", () => {
  test("emits the exact bounded public-head subscription and close frames", () => {
    expect(mktDiscoveryRequest()).toBe(
      JSON.stringify([
        "REQ",
        MKT_DISCOVERY_SUBSCRIPTION_ID,
        { kinds: [39600, 39601], limit: MKT_DISCOVERY_LIMIT },
      ]),
    );
    expect(mktDiscoveryClose()).toBe(JSON.stringify(["CLOSE", MKT_DISCOVERY_SUBSCRIPTION_ID]));
  });

  test("refuses an oversized frame before JSON parsing", async () => {
    const book = new MktDiscoveryBook();
    await expect(
      Effect.runPromise(
        book.ingestText(`"${"x".repeat(MKT_DISCOVERY_MAX_FRAME_BYTES)}"`, fixture.observed_at),
      ),
    ).rejects.toMatchObject({ code: "invalid_frame" });
  });

  test("publishes one atomic EOSE snapshot with deterministic replacement", async () => {
    const book = new MktDiscoveryBook();
    book.beginSnapshot();

    for (const event of [
      fixture.provider_old,
      fixture.offering_old,
      fixture.provider_new,
      fixture.offering_tie_b,
      fixture.offering_tie_a,
    ]) {
      await Effect.runPromise(book.ingestText(eventFrame(event), fixture.observed_at));
    }

    await expect(
      Effect.runPromise(book.snapshot("ws://localhost", fixture.observed_at)),
    ).rejects.toMatchObject({ code: "invalid_frame" });

    await expect(
      Effect.runPromise(book.ingestText(eoseFrame, fixture.observed_at)),
    ).resolves.toEqual({ type: "eose" });

    const snapshot = await Effect.runPromise(book.snapshot("ws://localhost", fixture.observed_at));
    expect(snapshot.providers).toHaveLength(1);
    expect(snapshot.providers[0]?.event.id).toBe(fixture.provider_new.id);
    expect(snapshot.offerings).toHaveLength(1);
    expect(snapshot.offerings[0]?.event.id).toBe(fixture.offering_tie_a.id);

    const offerings = await Effect.runPromise(projectDiscoveredOfferings(snapshot));
    expect(offerings).toEqual([
      expect.objectContaining({
        offeringStatus: "active",
        providerStatus: "paused",
        availability: "limited",
        sides: [expect.objectContaining({ feeBps: "30" })],
      }),
    ]);
  });

  test("does not erase a snapshot on a duplicate EOSE", async () => {
    const book = new MktDiscoveryBook();
    await Effect.runPromise(book.ingestText(eventFrame(fixture.provider_old), fixture.observed_at));
    await Effect.runPromise(book.ingestText(eoseFrame, fixture.observed_at));
    await expect(
      Effect.runPromise(book.ingestText(eoseFrame, fixture.observed_at)),
    ).resolves.toEqual({ type: "ignored" });
    const snapshot = await Effect.runPromise(book.snapshot("ws://localhost", fixture.observed_at));
    expect(snapshot.providers).toHaveLength(1);
  });

  test("keeps the prior atomic snapshot visible while a replacement snapshot loads", async () => {
    const book = new MktDiscoveryBook();
    await Effect.runPromise(book.ingestText(eventFrame(fixture.provider_old), fixture.observed_at));
    await Effect.runPromise(book.ingestText(eoseFrame, fixture.observed_at));

    book.beginSnapshot();
    await Effect.runPromise(book.ingestText(eventFrame(fixture.provider_new), fixture.observed_at));
    const prior = await Effect.runPromise(book.snapshot("ws://localhost", fixture.observed_at));
    expect(prior.providers[0]?.event.id).toBe(fixture.provider_old.id);

    await Effect.runPromise(book.ingestText(eoseFrame, fixture.observed_at));
    const replaced = await Effect.runPromise(book.snapshot("ws://localhost", fixture.observed_at));
    expect(replaced.providers[0]?.event.id).toBe(fixture.provider_new.id);
  });

  test("isolates an invalid signed event and accounts for the rejected frame", async () => {
    const book = new MktDiscoveryBook();
    await Effect.runPromise(book.ingestText(eventFrame(fixture.provider_old), fixture.observed_at));
    await expect(
      Effect.runPromise(
        book.ingestText(
          eventFrame({ ...fixture.provider_new, id: "00".repeat(32) }),
          fixture.observed_at,
        ),
      ),
    ).rejects.toMatchObject({ code: "invalid_event" });
    await Effect.runPromise(book.ingestText(eoseFrame, fixture.observed_at));
    const snapshot = await Effect.runPromise(book.snapshot("ws://localhost", fixture.observed_at));
    expect(snapshot.providers[0]?.event.id).toBe(fixture.provider_old.id);
    expect(snapshot.rejectedFrames).toBe(1);
  });

  test("surfaces AUTH and CLOSED as host-owned policy decisions", async () => {
    const book = new MktDiscoveryBook();
    await expect(
      Effect.runPromise(
        book.ingestText(JSON.stringify(["AUTH", { challenge: "fixture" }]), fixture.observed_at),
      ),
    ).rejects.toMatchObject({ code: "auth_required" });
    await expect(
      Effect.runPromise(
        book.ingestText(
          JSON.stringify(["CLOSED", MKT_DISCOVERY_SUBSCRIPTION_ID, "rate limited"]),
          fixture.observed_at,
        ),
      ),
    ).rejects.toMatchObject({ code: "subscription_closed" });
  });

  test("refuses projection when an Offering references an absent Provider", async () => {
    const book = new MktDiscoveryBook();
    await Effect.runPromise(book.ingestText(eventFrame(fixture.offering_old), fixture.observed_at));
    await Effect.runPromise(book.ingestText(eoseFrame, fixture.observed_at));
    const snapshot = await Effect.runPromise(book.snapshot("ws://localhost", fixture.observed_at));
    await expect(Effect.runPromise(projectDiscoveredOfferings(snapshot))).rejects.toMatchObject({
      code: "projection_failed",
    });
  });
});
