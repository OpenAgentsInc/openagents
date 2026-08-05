/**
 * Session store behaviour: durability across reopen, crash-mid-update
 * safety, the per-session lock (a concurrent status fold and background
 * task cannot lose an update), signed-record replay idempotency, the
 * effect-ledger binding rules, and the custody tripwire.
 */
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  EffectBindingConflictError,
  SecretMaterialError,
  SessionAlreadyExistsError,
  SignedRecordConflictError,
} from "./errors.js";
import { memoryStringKv } from "./kv.js";
import type { SignedNostrRecord } from "./model.js";
import { openSessionStore } from "./store.js";
import { crashingStringKv, sampleSession, TEST_PROVIDER_PUBKEY } from "./testkit.js";

const signedRecord = (id: string, createdAt: number): SignedNostrRecord => ({
  id,
  pubkey: TEST_PROVIDER_PUBKEY,
  created_at: createdAt,
  kind: 39_607,
  tags: [],
  content: `{"seq":${createdAt}}`,
  sig: "ee".repeat(64),
});

describe("session store", () => {
  test("sessions survive a store reopen on the same backing storage", async () => {
    const kv = memoryStringKv();
    const session = sampleSession("s1");
    await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(session);
      }),
    );
    const reloaded = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        return yield* store.get("s1");
      }),
    );
    expect(reloaded.sessionId).toBe("s1");
    expect(reloaded.signedRecords).toEqual(session.signedRecords);
    expect(reloaded.exitPackages).toEqual(session.exitPackages);
  });

  test("creating a duplicate session id is refused", async () => {
    const kv = memoryStringKv();
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("dup"));
        return yield* Effect.flip(store.create(sampleSession("dup")));
      }),
    );
    expect(outcome).toBeInstanceOf(SessionAlreadyExistsError);
  });

  test("a crash mid-update never surfaces a partial record: old state survives", async () => {
    const seed = memoryStringKv();
    await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv: seed });
        yield* store.create(sampleSession("crashy"));
      }),
    );
    // Tear the first write of the update (the staging write).
    const crashKv = crashingStringKv({ tearOnSet: 1 }, seed.snapshot());
    await expect(
      Effect.runPromise(
        Effect.gen(function* () {
          const store = yield* openSessionStore({ kv: crashKv });
          yield* store.update("crashy", (session) =>
            Effect.succeed({
              ...session,
              projection: { ...session.projection, state: "funding_broadcast" },
            }),
          );
        }),
      ),
    ).rejects.toThrow();
    const recovered = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv: crashKv.survivingKv() });
        return yield* store.get("crashy");
      }),
    );
    expect(recovered.projection.state).toBe("ordered");
  });

  test("a crash tearing the base write recovers the complete NEW record", async () => {
    const seed = memoryStringKv();
    await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv: seed });
        yield* store.create(sampleSession("rollforward"));
      }),
    );
    const crashKv = crashingStringKv({ tearOnSet: 2 }, seed.snapshot());
    await expect(
      Effect.runPromise(
        Effect.gen(function* () {
          const store = yield* openSessionStore({ kv: crashKv });
          yield* store.update("rollforward", (session) =>
            Effect.succeed({
              ...session,
              projection: { ...session.projection, state: "funding_broadcast" },
            }),
          );
        }),
      ),
    ).rejects.toThrow();
    const recovered = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv: crashKv.survivingKv() });
        return yield* store.get("rollforward");
      }),
    );
    expect(recovered.projection.state).toBe("funding_broadcast");
  });

  test("concurrent status fold and background task cannot lose an update (lock)", async () => {
    const kv = memoryStringKv();
    const rounds = 25;
    const finalSession = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("locked", { signedRecords: [] }));
        const worker = (label: string) =>
          Effect.gen(function* () {
            for (let round = 0; round < rounds; round += 1) {
              yield* store.update("locked", (session) =>
                Effect.gen(function* () {
                  // Force interleaving between read and write: without the
                  // per-session lock this yields lost updates.
                  yield* Effect.yieldNow;
                  return {
                    ...session,
                    signedRecords: [
                      ...session.signedRecords,
                      signedRecord(
                        `${label}${String(round).padStart(2, "0")}`.padEnd(64, "0"),
                        round,
                      ),
                    ],
                  };
                }),
              );
            }
          });
        yield* Effect.all([worker("aa"), worker("bb")], { concurrency: "unbounded" });
        return yield* store.get("locked");
      }),
    );
    expect(finalSession.signedRecords.length).toBe(rounds * 2);
  });

  test("signed-record exact replay is idempotent; changed bytes fail closed", async () => {
    const kv = memoryStringKv();
    const record = signedRecord("ff".repeat(32), 100);
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("replay"));
        yield* store.appendSignedRecord("replay", record);
        yield* store.appendSignedRecord("replay", record); // exact replay
        const afterReplay = yield* store.get("replay");
        const conflict = yield* Effect.flip(
          store.appendSignedRecord("replay", { ...record, content: "{\"forged\":true}" }),
        );
        return { afterReplay, conflict };
      }),
    );
    expect(
      outcome.afterReplay.signedRecords.filter((candidate) => candidate.id === record.id).length,
    ).toBe(1);
    expect(outcome.conflict).toBeInstanceOf(SignedRecordConflictError);
  });

  test("effect ledger binds one effect id to one request and one result", async () => {
    const kv = memoryStringKv();
    const request = { operation: "funding_broadcast", txTemplateDigest: "ab".repeat(32) };
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("effects"));
        yield* store.recordEffectRequest("effects", "effect-1", request);
        yield* store.recordEffectRequest("effects", "effect-1", request); // idempotent
        const requestConflict = yield* Effect.flip(
          store.recordEffectRequest("effects", "effect-1", { operation: "other" }),
        );
        const orphanResult = yield* Effect.flip(
          store.recordEffectResult("effects", "effect-9", { ok: true }, null),
        );
        yield* store.recordEffectResult("effects", "effect-1", { txid: "cd".repeat(32) }, "cd".repeat(32));
        const resultConflict = yield* Effect.flip(
          store.recordEffectResult("effects", "effect-1", { txid: "ee".repeat(32) }, null),
        );
        return { requestConflict, orphanResult, resultConflict };
      }),
    );
    expect(outcome.requestConflict).toBeInstanceOf(EffectBindingConflictError);
    expect((outcome.requestConflict as EffectBindingConflictError).reason).toBe(
      "request_digest_mismatch",
    );
    expect((outcome.orphanResult as EffectBindingConflictError).reason).toBe(
      "result_without_request",
    );
    expect((outcome.resultConflict as EffectBindingConflictError).reason).toBe(
      "result_digest_mismatch",
    );
  });

  test("the custody tripwire refuses secret-looking payloads and never echoes the value", async () => {
    const kv = memoryStringKv();
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        return yield* Effect.flip(
          store.create(
            sampleSession("leaky", {
              engineSnapshot: { restore: { mnemonic: "abandon abandon about" } },
            }),
          ),
        );
      }),
    );
    expect(outcome).toBeInstanceOf(SecretMaterialError);
    const error = outcome as SecretMaterialError;
    expect(error.identifier).toBe("swp_secret_material_forbidden");
    expect(error.path).toContain("mnemonic");
    expect(JSON.stringify(error)).not.toContain("abandon");
    // And nothing reached storage.
    expect(Object.keys(kv.snapshot()).length).toBe(0);
  });
});
