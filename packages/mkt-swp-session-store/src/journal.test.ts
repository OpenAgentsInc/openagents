/**
 * Atomicity and crash-mid-write behaviour (issue #9320): a partially
 * written record must never be loadable as complete, and every simulated
 * crash point recovers to a complete committed record (old or new).
 */
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { TornSessionRecordError } from "./errors.js";
import {
  commitRecord,
  loadRecord,
  parseEnvelope,
  sealEnvelope,
  serializeEnvelope,
  stagingKeyOf,
} from "./journal.js";
import { memoryStringKv } from "./kv.js";
import { crashingStringKv } from "./testkit.js";

const KEY = "oa.swp.session/test";

describe("envelope integrity", () => {
  test("seal/serialize/parse round-trips", () => {
    const envelope = sealEnvelope(1, 7, { hello: "world", n: 42 });
    expect(parseEnvelope(serializeEnvelope(envelope))).toEqual(envelope);
  });

  test("every truncation of a serialized envelope is refused", () => {
    const serialized = serializeEnvelope(sealEnvelope(1, 1, { a: [1, 2, 3], b: "x" }));
    for (let length = 1; length < serialized.length; length += 7) {
      expect(parseEnvelope(serialized.slice(0, length))).toBeNull();
    }
    expect(parseEnvelope(serialized.slice(0, serialized.length - 1))).toBeNull();
  });

  test("a flipped payload byte breaks the digest", () => {
    const serialized = serializeEnvelope(sealEnvelope(1, 1, { amount: 1000 }));
    expect(parseEnvelope(serialized.replace("1000", "9000"))).toBeNull();
  });
});

describe("journaled commit and crash recovery", () => {
  const oldEnvelope = sealEnvelope(1, 1, { step: "old" });
  const newEnvelope = sealEnvelope(1, 2, { step: "new" });

  const seededSnapshot = () => {
    const kv = memoryStringKv();
    return Effect.runPromise(
      Effect.gen(function* () {
        yield* commitRecord(kv, KEY, oldEnvelope);
        return kv.snapshot();
      }),
    );
  };

  test("commit then load returns the record", async () => {
    const kv = memoryStringKv();
    const loaded = await Effect.runPromise(
      Effect.gen(function* () {
        yield* commitRecord(kv, KEY, oldEnvelope);
        return yield* loadRecord(kv, KEY);
      }),
    );
    expect(loaded).toEqual(oldEnvelope);
    expect(kv.snapshot()[stagingKeyOf(KEY)]).toBeUndefined();
  });

  test("crash tearing the staging write keeps the old committed record", async () => {
    const crashKv = crashingStringKv({ tearOnSet: 1 }, await seededSnapshot());
    await expect(Effect.runPromise(commitRecord(crashKv, KEY, newEnvelope))).rejects.toThrow();
    const recovered = await Effect.runPromise(loadRecord(crashKv.survivingKv(), KEY));
    expect(recovered).toEqual(oldEnvelope);
  });

  test("crash tearing the base write rolls forward to the staged new record", async () => {
    const crashKv = crashingStringKv({ tearOnSet: 2 }, await seededSnapshot());
    await expect(Effect.runPromise(commitRecord(crashKv, KEY, newEnvelope))).rejects.toThrow();
    const survivor = crashKv.survivingKv();
    const recovered = await Effect.runPromise(loadRecord(survivor, KEY));
    expect(recovered).toEqual(newEnvelope);
  });

  test("crash between staging and base write rolls forward", async () => {
    // Simulate by placing a complete newer staged copy beside the old base.
    const snapshot = await seededSnapshot();
    const kv = memoryStringKv({
      ...snapshot,
      [stagingKeyOf(KEY)]: serializeEnvelope(newEnvelope),
    });
    const recovered = await Effect.runPromise(loadRecord(kv, KEY));
    expect(recovered).toEqual(newEnvelope);
    // And the roll-forward is durable: the base key now holds the new record.
    expect(parseEnvelope(kv.snapshot()[KEY] ?? "")).toEqual(newEnvelope);
  });

  test("a crash during the very first write leaves no record, not a torn one", async () => {
    const crashKv = crashingStringKv({ tearOnSet: 1 });
    await expect(Effect.runPromise(commitRecord(crashKv, KEY, newEnvelope))).rejects.toThrow();
    const recovered = await Effect.runPromise(loadRecord(crashKv.survivingKv(), KEY));
    expect(recovered).toBeNull();
  });

  test("both copies torn refuses loudly instead of guessing", async () => {
    const kv = memoryStringKv({
      [KEY]: serializeEnvelope(oldEnvelope).slice(0, 30),
      [stagingKeyOf(KEY)]: serializeEnvelope(newEnvelope).slice(0, 30),
    });
    const outcome = await Effect.runPromise(Effect.flip(loadRecord(kv, KEY)));
    expect(outcome).toBeInstanceOf(TornSessionRecordError);
  });
});
