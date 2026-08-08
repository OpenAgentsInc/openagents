/**
 * Session store behaviour: durability across reopen, crash-mid-update
 * safety, the per-session lock (a concurrent status fold and background
 * task cannot lose an update), signed-record replay idempotency, the
 * effect-ledger binding rules, and the custody tripwire.
 */
import { Deferred, Effect, Fiber } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  EffectBindingConflictError,
  SecretMaterialError,
  SessionAlreadyExistsError,
  SessionNotFoundError,
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

  test("a delete concurrent with a mid-flight update cannot resurrect the session", async () => {
    // The audit's reachable trace: a background fold enters update, reads,
    // and awaits its effectful modify; the user confirms Delete; the fold's
    // persist then re-commits the record the delete just removed. The locks
    // must compose so the delete WAITS for the in-flight update and the
    // session stays gone.
    const kv = memoryStringKv();
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("victim"));
        const updateEntered = yield* Deferred.make<void>();
        const releaseUpdate = yield* Deferred.make<void>();
        const updater = yield* Effect.forkChild(
          store.update("victim", (session) =>
            Effect.gen(function* () {
              yield* Deferred.succeed(updateEntered, undefined);
              yield* Deferred.await(releaseUpdate);
              return {
                ...session,
                projection: { ...session.projection, state: "funding_broadcast" },
              };
            }),
          ),
        );
        // The update is provably inside its modify when delete is issued.
        yield* Deferred.await(updateEntered);
        const deleter = yield* Effect.forkChild(store.delete("victim"));
        yield* Effect.yieldNow;
        yield* Deferred.succeed(releaseUpdate, undefined);
        yield* Fiber.join(updater);
        yield* Fiber.join(deleter);
        const lookup = yield* Effect.flip(store.get("victim"));
        // And a later update finds nothing to resurrect either.
        const lateUpdate = yield* Effect.flip(
          store.update("victim", (session) => Effect.succeed(session)),
        );
        return { lookup, lateUpdate, snapshot: kv.snapshot() };
      }),
    );
    expect(outcome.lookup).toBeInstanceOf(SessionNotFoundError);
    expect(outcome.lateUpdate).toBeInstanceOf(SessionNotFoundError);
    // Storage agrees: no record and no staging remnant survived the delete.
    expect(Object.keys(outcome.snapshot)).toEqual([]);
  });

  test("effect failure binding: definitive failure needs a request, cannot follow a result, and a retry success clears it", async () => {
    const kv = memoryStringKv();
    const request = { operation: "reverse_invoice_payment", invoiceDigest: "ef".repeat(32) };
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("flaky"));
        const orphanFailure = yield* Effect.flip(
          store.recordEffectFailure("flaky", "pay-9", "cancelled", "no request"),
        );
        yield* store.recordEffectRequest("flaky", "pay-1", request);
        yield* store.recordEffectFailure("flaky", "pay-1", "cancelled", "user dismissed the prompt");
        const afterFailure = yield* store.get("flaky");
        // The failed attempt does NOT suppress a legitimate retry.
        const priorAfterFailure = yield* store.priorEffectResult("flaky", "pay-1", request);
        // The retry succeeds: the failure is cleared, the result binds.
        yield* store.recordEffectResult("flaky", "pay-1", { paymentId: "12".repeat(32) }, null);
        const afterRetry = yield* store.get("flaky");
        const failureAfterResult = yield* Effect.flip(
          store.recordEffectFailure("flaky", "pay-1", "failed", "too late"),
        );
        return { orphanFailure, afterFailure, priorAfterFailure, afterRetry, failureAfterResult };
      }),
    );
    expect((outcome.orphanFailure as EffectBindingConflictError).reason).toBe(
      "failure_without_request",
    );
    expect(outcome.afterFailure.effectLedger[0]?.failure?.reason).toBe("cancelled");
    expect(outcome.priorAfterFailure).toBeNull();
    expect(outcome.afterRetry.effectLedger[0]?.failure).toBeNull();
    expect(outcome.afterRetry.effectLedger[0]?.result).not.toBeNull();
    expect((outcome.failureAfterResult as EffectBindingConflictError).reason).toBe(
      "failure_after_result",
    );
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

  test("the tripwire catches secret member names by stem, not only canonical spellings", async () => {
    // The audit's exact escapes: refundKey, preimageHex, claimPrivateKeyWif.
    const kv = memoryStringKv();
    for (const [member, value] of [
      ["refundKey", "03".repeat(33)],
      ["preimageHex", "ab".repeat(32)],
      ["claimPrivateKeyWif", "not-echoed"],
    ] as const) {
      const outcome = await Effect.runPromise(
        Effect.gen(function* () {
          const store = yield* openSessionStore({ kv });
          return yield* Effect.flip(
            store.create(sampleSession(`leaky-${member}`, { engineSnapshot: { [member]: value } })),
          );
        }),
      );
      expect(outcome).toBeInstanceOf(SecretMaterialError);
      expect((outcome as SecretMaterialError).path).toContain(member);
    }
    expect(Object.keys(kv.snapshot()).length).toBe(0);
  });
});
