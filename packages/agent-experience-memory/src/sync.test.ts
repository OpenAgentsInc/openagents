import { describe, expect, test } from "vite-plus/test";

import {
  buildEngramBody,
  buildEngramEvent,
  engramContentDigest,
  type EngramEvent,
} from "./engram.js";
import {
  EngramSyncQueue,
  MemoryTransport,
  type EngramTransport,
  type PublishResult,
} from "./sync.js";

const PUBKEY = "a".repeat(64);
const sign = (eventId: string): string => `sig-${eventId.slice(0, 16)}`;

const engram = (slug: string, value: string, createdAt: number): EngramEvent => {
  const body = buildEngramBody(slug, value, {
    admission: "admitted",
    entityId: "entity-1",
    contentDigest: engramContentDigest(value),
    sourceEventRefs: [],
    relations: [],
    derivedFromSlugs: [],
  });
  return buildEngramEvent(PUBKEY, createdAt, slug, JSON.stringify(body), sign);
};

describe("the memory transport", () => {
  test("stores what it accepts and finds it by filter", async () => {
    const transport = new MemoryTransport();
    const first = engram("note/one", "first", 1_000);
    const second = engram("note/two", "second", 2_000);
    await transport.publish(first);
    await transport.publish(second);

    expect(await transport.fetch({ slugs: ["note/one"] })).toEqual([first]);
    expect(await transport.fetch({ since: 1_500 })).toEqual([second]);
    expect(await transport.fetch({ authors: [PUBKEY] })).toHaveLength(2);
    expect(await transport.fetch({ limit: 1 })).toEqual([first]);
  });

  test("refuses an engram whose id does not verify", async () => {
    const transport = new MemoryTransport();
    const authentic = engram("note/one", "authentic", 1_000);
    const tampered: EngramEvent = {
      ...authentic,
      content: authentic.content.replace("authentic", "forged"),
    };

    const result = await transport.publish(tampered);
    expect(result).toEqual({ ok: false, reason: "refused", detail: "event id does not verify" });
    expect(transport.stored()).toHaveLength(0);
  });
});

describe("the sync queue", () => {
  test("publishing does not wait on the transport", () => {
    const queue = new EngramSyncQueue(new MemoryTransport());
    // No await: publish returns nothing to wait on, and the engram is queued.
    queue.publish(engram("note/one", "first", 1_000));
    expect(queue.status().pending).toBe(1);
    expect(queue.status().delivered).toBe(0);
  });

  test("a drain delivers what is queued", async () => {
    const transport = new MemoryTransport();
    const queue = new EngramSyncQueue(transport);
    queue.publish(engram("note/one", "first", 1_000));
    queue.publish(engram("note/two", "second", 2_000));

    expect(await queue.drain()).toBe(2);
    expect(queue.status()).toMatchObject({ pending: 0, delivered: 2, refused: 0 });
    expect(transport.stored()).toHaveLength(2);
  });

  test("an unreachable transport loses nothing and says it is behind", async () => {
    const transport = new MemoryTransport();
    transport.reachable = false;
    const queue = new EngramSyncQueue(transport);
    queue.publish(engram("note/one", "first", 1_000));

    expect(await queue.drain()).toBe(0);
    expect(queue.behind()).toBe(true);
    expect(queue.status()).toMatchObject({ pending: 1, delivered: 0 });
    expect(queue.status().lastFailure?.reason).toBe("unreachable");

    // The engram is still there when the transport comes back.
    transport.reachable = true;
    expect(await queue.drain()).toBe(1);
    expect(queue.behind()).toBe(false);
    expect(transport.stored()).toHaveLength(1);
  });

  test("a refusal is terminal and does not retry forever", async () => {
    const transport = new MemoryTransport();
    const queue = new EngramSyncQueue(transport);
    const authentic = engram("note/one", "authentic", 1_000);
    queue.publish({ ...authentic, content: authentic.content.replace("authentic", "forged") });

    expect(await queue.drain()).toBe(0);
    expect(queue.status()).toMatchObject({ pending: 0, refused: 1 });
    // Nothing left to retry: the transport judged it, and repeating the call
    // only repeats the judgement.
    expect(await queue.drain()).toBe(0);
    expect(queue.behind()).toBe(false);
  });

  test("a transport that throws is treated as failing, not as a crash", async () => {
    const throwing: EngramTransport = {
      publish(): Promise<PublishResult> {
        throw new Error("socket exploded");
      },
      fetch(): Promise<ReadonlyArray<EngramEvent>> {
        throw new Error("socket exploded");
      },
    };
    const queue = new EngramSyncQueue(throwing);
    queue.publish(engram("note/one", "first", 1_000));

    await expect(queue.drain()).resolves.toBe(0);
    expect(queue.status().lastFailure).toMatchObject({
      reason: "failed",
      detail: "socket exploded",
    });
    expect(queue.status().pending).toBe(1);
    await expect(queue.fetch({})).resolves.toEqual([]);
  });

  test("an engram is never delivered twice", async () => {
    const transport = new MemoryTransport();
    const queue = new EngramSyncQueue(transport);
    const event = engram("note/one", "first", 1_000);

    queue.publish(event);
    expect(await queue.drain()).toBe(1);

    // Republishing the whole ledger costs one pass, not one delivery per pass.
    queue.publish(event);
    expect(queue.status().pending).toBe(0);
    expect(await queue.drain()).toBe(0);
    expect(queue.status().delivered).toBe(1);
  });

  test("the same engram queued twice before a drain delivers once", async () => {
    const transport = new MemoryTransport();
    const queue = new EngramSyncQueue(transport);
    const event = engram("note/one", "first", 1_000);
    queue.publish(event);
    queue.publish(event);

    expect(queue.status().pending).toBe(1);
    expect(await queue.drain()).toBe(1);
  });

  test("fetching reads back what was delivered", async () => {
    const transport = new MemoryTransport();
    const queue = new EngramSyncQueue(transport);
    const event = engram("note/one", "first", 1_000);
    queue.publish(event);
    await queue.drain();

    expect(await queue.fetch({ slugs: ["note/one"] })).toEqual([event]);
  });
});
