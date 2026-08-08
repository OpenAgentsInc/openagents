/**
 * Behaviour-contract oracle for the export/import round trip (issue #9320).
 *
 * Enforced contract (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-session-store):
 * - openagents_web.swap_history.export_import_round_trip.v1
 *
 * Export then import into a clean profile reproduces every session's
 * actionable state — the resume plan, the effect ledger, and the exit
 * packages needed for a unilateral exit — and import refuses foreign or
 * corrupt documents with a typed error, all-or-nothing.
 */
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { makeCatalog } from "@openagentsinc/swap-i18n";

import { contentDigestHex } from "./canonical.js";
import { HISTORY_IMPORT_REFUSAL_REASONS, HistoryImportError, SecretMaterialError, StorageDriverError } from "./errors.js";
import {
  EXPORT_SENSITIVITY_KEY,
  SWAP_HISTORY_EXPORT_FORMAT,
  exportPrivateHistory,
  importPrivateHistory,
  importRefusalKeyOf,
  type SwapHistoryExport,
} from "./export.js";
import { memoryStringKv, type StringKv } from "./kv.js";
import { planResume } from "./resume.js";
import { assertNoSecretMaterial } from "./secret-boundary.js";
import { openSessionStore, type SessionStore } from "./store.js";
import { sampleSession } from "./testkit.js";

const seedStore = () =>
  Effect.gen(function* () {
    const kv = memoryStringKv();
    const store = yield* openSessionStore({ kv });
    yield* store.create(sampleSession("swap-a"));
    yield* store.recordEffectRequest("swap-a", "fund-1", {
      operation: "funding_broadcast",
      templateDigest: "ab".repeat(32),
    });
    yield* store.recordEffectResult("swap-a", "fund-1", { txid: "cd".repeat(32) }, "cd".repeat(32));
    yield* store.create(
      sampleSession("swap-b", {
        createdAt: 1_754_100_000,
        projection: {
          state: "completed",
          terminal: true,
          outcome: "completed",
          rung: "measured",
          unclaimedFunds: true,
        },
      }),
    );
    return store;
  });

const cleanStore = (): Effect.Effect<SessionStore, never> =>
  openSessionStore({ kv: memoryStringKv() }).pipe(Effect.orDie);

describe("export/import round trip (openagents_web.swap_history.export_import_round_trip.v1)", () => {
  test("import into a clean profile reproduces every session's actionable state", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* seedStore();
        const document = yield* exportPrivateHistory(source);
        const target = yield* cleanStore();
        const result = yield* importPrivateHistory(target, document);
        const sourceSessions = yield* source.list();
        const targetSessions = yield* target.list();
        // The unilateral-exit dataset survives: the prior effect result is
        // queryable in the clean profile with the exact original request.
        const prior = yield* target.priorEffectResult("swap-a", "fund-1", {
          operation: "funding_broadcast",
          templateDigest: "ab".repeat(32),
        });
        return { document, result, sourceSessions, targetSessions, prior };
      }),
    );

    expect([...outcome.result.imported].sort()).toEqual(["swap-a", "swap-b"]);
    const byId = <T extends { readonly sessionId: string }>(
      sessions: ReadonlyArray<T>,
    ): ReadonlyArray<T> => [...sessions].sort((a, b) => a.sessionId.localeCompare(b.sessionId));
    expect(byId(outcome.targetSessions)).toEqual(byId(outcome.sourceSessions));
    // Resume planning — the actionable state — is identical.
    expect(planResume(byId(outcome.targetSessions))).toEqual(
      planResume(byId(outcome.sourceSessions)),
    );
    // The exit package needed for a unilateral exit travelled with it.
    const imported = outcome.targetSessions.find((session) => session.sessionId === "swap-a");
    expect(imported?.exitPackages.length).toBe(1);
    expect(outcome.prior?.result?.externalId).toBe("cd".repeat(32));
  });

  test("the export holds no secret material, and re-importing it is idempotent", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* seedStore();
        const document = yield* exportPrivateHistory(source);
        yield* assertNoSecretMaterial(document, "export");
        // Import back into the SAME store: everything identical, no writes.
        const result = yield* importPrivateHistory(source, document);
        return { document, result };
      }),
    );
    expect(outcome.document.format).toBe(SWAP_HISTORY_EXPORT_FORMAT);
    expect(outcome.result.imported).toEqual([]);
    expect([...outcome.result.identical].sort()).toEqual(["swap-a", "swap-b"]);
  });

  test("the tripwire runs on the export path itself, whatever wrote the store", async () => {
    // A store whose in-memory state carries secret-looking material (a bug
    // upstream of persist, by construction): exportPrivateHistory must
    // refuse rather than emit it.
    const smuggling = sampleSession("mule", {
      engineSnapshot: { restore: { preimageHex: "ab".repeat(32) } },
    });
    const fakeStore: SessionStore = {
      list: () => Effect.succeed([smuggling]),
      get: () => Effect.die("unused"),
      create: () => Effect.die("unused"),
      update: () => Effect.die("unused"),
      appendSignedRecord: () => Effect.die("unused"),
      recordEffectRequest: () => Effect.die("unused"),
      recordEffectResult: () => Effect.die("unused"),
      recordEffectFailure: () => Effect.die("unused"),
      priorEffectResult: () => Effect.die("unused"),
      delete: () => Effect.die("unused"),
      putValidated: () => Effect.die("unused"),
    };
    const refusal = await Effect.runPromise(Effect.flip(exportPrivateHistory(fakeStore)));
    expect(refusal).toBeInstanceOf(SecretMaterialError);
    expect(refusal.path).toContain("preimageHex");
    expect(JSON.stringify(refusal)).not.toContain("ab".repeat(32));
  });

  test("a v1 export imports with DEFAULT arguments: the shipped chain migrates it", async () => {
    // The promised case: a document produced by an older build. v1 ledger
    // entries had no `failure` member; the shipped v1→v2 step adds it.
    const request = { operation: "funding_broadcast", templateDigest: "ab".repeat(32) };
    const modern = sampleSession("legacy", {
      effectLedger: [
        {
          effectId: "fund-1",
          requestDigestHex: contentDigestHex(request),
          request,
          result: null,
          failure: null,
        },
      ],
    });
    const v1Session = {
      ...modern,
      effectLedger: modern.effectLedger.map(({ failure: _failure, ...v1Entry }) => v1Entry),
    };
    const v1Document = {
      format: SWAP_HISTORY_EXPORT_FORMAT,
      schemaVersion: 1,
      exportedAt: 0,
      sessions: [
        { schemaVersion: 1, contentDigestHex: contentDigestHex(v1Session), session: v1Session },
      ],
    };
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const target = yield* cleanStore();
        const result = yield* importPrivateHistory(target, v1Document);
        const imported = yield* target.get("legacy");
        return { result, imported };
      }),
    );
    expect(outcome.result.imported).toEqual(["legacy"]);
    expect(outcome.imported.effectLedger[0]?.failure).toBeNull();
  });

  test("every refusal reason maps to a catalog key that renders", () => {
    const catalog = makeCatalog();
    expect(typeof catalog[EXPORT_SENSITIVITY_KEY]).toBe("string");
    for (const reason of HISTORY_IMPORT_REFUSAL_REASONS) {
      const key = importRefusalKeyOf(new HistoryImportError({ reason, detail: "test" }));
      const message = catalog[key];
      expect(typeof message).toBe("string");
      expect((message as string).length).toBeGreaterThan(0);
    }
  });
});

describe("import refuses foreign and corrupt documents, all-or-nothing", () => {
  const refusalOf = (document: unknown) =>
    Effect.runPromise(
      Effect.gen(function* () {
        const target = yield* cleanStore();
        const refusal = yield* Effect.flip(importPrivateHistory(target, document));
        const after = yield* target.list();
        return { refusal, after };
      }),
    );

  test("a non-export document refuses with not_an_export", async () => {
    for (const document of [null, [], "history", { format: "boltz-history" }]) {
      const { refusal, after } = await refusalOf(document);
      expect(refusal).toBeInstanceOf(HistoryImportError);
      expect((refusal as HistoryImportError).reason).toBe("not_an_export");
      expect(after.length).toBe(0);
    }
  });

  test("a future-version export refuses with unsupported_version", async () => {
    const { refusal } = await refusalOf({
      format: SWAP_HISTORY_EXPORT_FORMAT,
      schemaVersion: 99,
      exportedAt: 0,
      sessions: [],
    });
    expect((refusal as HistoryImportError).reason).toBe("unsupported_version");
  });

  test("a tampered session refuses with digest_mismatch and applies NOTHING", async () => {
    const document = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* seedStore();
        return yield* exportPrivateHistory(source);
      }),
    );
    // First session intact, second tampered: all-or-nothing means even the
    // intact one is not applied.
    const tampered: SwapHistoryExport = {
      ...document,
      sessions: [
        document.sessions[0]!,
        {
          ...document.sessions[1]!,
          session: { ...document.sessions[1]!.session, relayUrl: "wss://evil.example" },
        },
      ],
    };
    const { refusal, after } = await refusalOf(tampered);
    expect((refusal as HistoryImportError).reason).toBe("digest_mismatch");
    expect(after.length).toBe(0);
  });

  test("a structurally invalid session refuses with session_invalid", async () => {
    // A well-digested document whose session fails the schema: only the
    // structural decode can refuse it.
    const malformed = { sessionId: "x", butNoOtherRequiredMembers: true };
    const { refusal } = await refusalOf({
      format: SWAP_HISTORY_EXPORT_FORMAT,
      schemaVersion: 1,
      exportedAt: 0,
      sessions: [
        { schemaVersion: 1, contentDigestHex: contentDigestHex(malformed), session: malformed },
      ],
    });
    expect((refusal as HistoryImportError).reason).toBe("session_invalid");
  });

  test("an export smuggling secret material refuses with secret_material", async () => {
    const document = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* cleanStore();
        yield* source
          .create(sampleSession("mule"))
          .pipe(Effect.orDie);
        return yield* exportPrivateHistory(source);
      }),
    );
    // Recompute the digest so ONLY the tripwire can catch it.
    const smuggled = {
      ...document.sessions[0]!.session,
      engineSnapshot: { claimPrivateKey: "ff".repeat(32) },
    };
    const { refusal, after } = await refusalOf({
      ...document,
      sessions: [
        {
          schemaVersion: 1,
          contentDigestHex: contentDigestHex(smuggled),
          session: smuggled,
        },
      ],
    });
    expect((refusal as HistoryImportError).reason).toBe("secret_material");
    expect(after.length).toBe(0);
  });

  test("a mid-apply storage failure rolls back this import's writes and refuses typed", async () => {
    // Quota-style driver: writes fail after a budget; deletes always work
    // (freeing space) — the realistic browser-storage failure shape.
    const quotaStringKv = (allowedSets: number): StringKv & { snapshot: () => Record<string, string> } => {
      const inner = memoryStringKv();
      let sets = 0;
      return {
        get: inner.get,
        keys: inner.keys,
        delete: inner.delete,
        set: (key, value) => {
          sets += 1;
          return sets > allowedSets
            ? Effect.fail(new StorageDriverError({ operation: "set", key, detail: "quota exceeded" }))
            : inner.set(key, value);
        },
        snapshot: inner.snapshot,
      };
    };
    const document = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* seedStore();
        return yield* exportPrivateHistory(source);
      }),
    );
    // Budget covers the first session's two-phase commit (staging + base)
    // and dies inside the second session's write.
    const kv = quotaStringKv(2);
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const target = yield* openSessionStore({ kv });
        const refusal = yield* Effect.flip(importPrivateHistory(target, document));
        const after = yield* target.list();
        return { refusal, after };
      }),
    );
    expect(outcome.refusal).toBeInstanceOf(HistoryImportError);
    expect((outcome.refusal as HistoryImportError).reason).toBe("storage_failure");
    // Rolled back: the store is as it was, and storage holds no session.
    expect(outcome.after.length).toBe(0);
    expect(Object.keys(kv.snapshot()).length).toBe(0);
  });

  test("a session id that exists locally with different content refuses with conflicting_session", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* seedStore();
        const document = yield* exportPrivateHistory(source);
        // The target already has swap-a — but with different content.
        const target = yield* cleanStore();
        yield* target
          .create(sampleSession("swap-a", { relayUrl: "wss://other.example" }))
          .pipe(Effect.orDie);
        const refusal = yield* Effect.flip(importPrivateHistory(target, document));
        const after = yield* target.list();
        return { refusal, after };
      }),
    );
    expect((outcome.refusal as HistoryImportError).reason).toBe("conflicting_session");
    // Nothing applied: swap-b was valid but must not land either.
    expect(outcome.after.map((session) => session.sessionId)).toEqual(["swap-a"]);
    expect(outcome.after[0]?.relayUrl).toBe("wss://other.example");
  });
});
