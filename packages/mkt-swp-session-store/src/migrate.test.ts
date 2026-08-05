/**
 * Schema versioning and migration (issue #9320): every version step runs
 * over a fixture corpus and rewrites every record; an unknown FUTURE version
 * is refused loudly rather than silently mangled; a missing step refuses.
 */
import { Effect, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { MigrationStepMissingError, UnsupportedSchemaVersionError } from "./errors.js";
import { parseEnvelope, sealEnvelope } from "./journal.js";
import { memoryStringKv } from "./kv.js";
import { migrateEnvelopePayload, type SessionSchemaMigration } from "./migrate.js";
import { StoredSwapSession } from "./model.js";
import { openSessionStore } from "./store.js";
import { sampleSession } from "./testkit.js";

const encodeSession = Schema.encodeUnknownSync(StoredSwapSession);

/**
 * A synthetic two-step chain exercising the machinery the first real
 * migration will use: v1→v2 wraps the offering address, v2→v3 unwraps it
 * and stamps a marker tag into the projection state.
 */
const TEST_MIGRATIONS: ReadonlyArray<SessionSchemaMigration> = [
  {
    from: 1,
    to: 2,
    migrate: (payload) => {
      const session = payload as Record<string, unknown>;
      return { ...session, offeringAddress: { wrapped: session.offeringAddress } };
    },
  },
  {
    from: 2,
    to: 3,
    migrate: (payload) => {
      const session = payload as Record<string, unknown>;
      const wrapped = session.offeringAddress as { readonly wrapped: string };
      return { ...session, offeringAddress: `migrated:${wrapped.wrapped}` };
    },
  },
];

/** Fixture corpus: three v1 records persisted through the real store. */
const seedV1Corpus = () =>
  Effect.gen(function* () {
    const kv = memoryStringKv();
    const store = yield* openSessionStore({ kv });
    for (const id of ["m1", "m2", "m3"]) {
      yield* store.create(sampleSession(id));
    }
    return kv;
  });

describe("schema migrations", () => {
  test("every step runs over every record in the corpus and rewrites it", async () => {
    const { sessions, snapshot } = await Effect.runPromise(
      Effect.gen(function* () {
        const kv = yield* seedV1Corpus();
        const store = yield* openSessionStore({
          kv,
          migrations: TEST_MIGRATIONS,
          currentVersion: 3,
        });
        return { sessions: yield* store.list(), snapshot: kv.snapshot() };
      }),
    );
    expect(sessions.length).toBe(3);
    for (const session of sessions) {
      expect(session.offeringAddress.startsWith("migrated:39601:")).toBe(true);
    }
    // The stored envelopes themselves were rewritten at the new version.
    for (const [key, raw] of Object.entries(snapshot)) {
      if (key.endsWith("!next")) continue;
      expect(parseEnvelope(raw)?.schemaVersion).toBe(3);
    }
  });

  test("migrated records reopen cleanly at the new version with no further steps", async () => {
    const sessions = await Effect.runPromise(
      Effect.gen(function* () {
        const kv = yield* seedV1Corpus();
        yield* openSessionStore({ kv, migrations: TEST_MIGRATIONS, currentVersion: 3 });
        const reopened = yield* openSessionStore({ kv, migrations: [], currentVersion: 3 });
        return yield* reopened.list();
      }),
    );
    expect(sessions.length).toBe(3);
  });

  test("a record from a FUTURE schema version refuses store open loudly", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const corpusKv = yield* seedV1Corpus();
        // A build far in the future wrote this record.
        const futureKv = memoryStringKv();
        const futureStore = yield* openSessionStore({ kv: futureKv, currentVersion: 99 });
        yield* futureStore.create(sampleSession("from-the-future"));
        const merged = memoryStringKv({ ...corpusKv.snapshot(), ...futureKv.snapshot() });
        // Today's build must refuse rather than guess at the payload shape.
        return yield* Effect.flip(openSessionStore({ kv: merged }));
      }),
    );
    expect(outcome).toBeInstanceOf(UnsupportedSchemaVersionError);
    const error = outcome as UnsupportedSchemaVersionError;
    expect(error.found).toBe(99);
    expect(error.supported).toBe(1);
  });

  test("a missing sequential step refuses instead of skipping", async () => {
    const envelope = sealEnvelope(1, 1, encodeSession(sampleSession("gap")));
    const outcome = await Effect.runPromise(
      Effect.flip(
        migrateEnvelopePayload("k", envelope, [TEST_MIGRATIONS[1]!], 3),
      ),
    );
    expect(outcome).toBeInstanceOf(MigrationStepMissingError);
    expect((outcome as MigrationStepMissingError).from).toBe(1);
  });
});
