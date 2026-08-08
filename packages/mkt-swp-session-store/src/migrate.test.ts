/**
 * Schema versioning and migration (issue #9320): the SHIPPED chain runs over
 * a hand-built v1 fixture; every synthetic version step runs over a fixture
 * corpus and rewrites every record; an unknown FUTURE version is refused
 * loudly rather than silently mangled; a missing step refuses; and a
 * migration step that introduced secret material refuses at open instead of
 * being committed (and later exported) unscanned.
 */
import { Effect, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { MigrationStepMissingError, SecretMaterialError, UnsupportedSchemaVersionError } from "./errors.js";
import { parseEnvelope, sealEnvelope, serializeEnvelope } from "./journal.js";
import { memoryStringKv } from "./kv.js";
import { CURRENT_SCHEMA_VERSION, migrateEnvelopePayload, type SessionSchemaMigration } from "./migrate.js";
import { StoredSwapSession } from "./model.js";
import { DEFAULT_KEY_PREFIX, openSessionStore } from "./store.js";
import { sampleSession } from "./testkit.js";

const encodeSession = Schema.encodeUnknownSync(StoredSwapSession);

/**
 * A synthetic two-step chain from the CURRENT version, exercising the
 * machinery every future migration will use: the first step wraps the
 * offering address, the second unwraps it and stamps a marker.
 */
const TEST_MIGRATIONS: ReadonlyArray<SessionSchemaMigration> = [
  {
    from: CURRENT_SCHEMA_VERSION,
    to: CURRENT_SCHEMA_VERSION + 1,
    migrate: (payload) => {
      const session = payload as Record<string, unknown>;
      return { ...session, offeringAddress: { wrapped: session.offeringAddress } };
    },
  },
  {
    from: CURRENT_SCHEMA_VERSION + 1,
    to: CURRENT_SCHEMA_VERSION + 2,
    migrate: (payload) => {
      const session = payload as Record<string, unknown>;
      const wrapped = session.offeringAddress as { readonly wrapped: string };
      return { ...session, offeringAddress: `migrated:${wrapped.wrapped}` };
    },
  },
];

const SYNTHETIC_TARGET = CURRENT_SCHEMA_VERSION + 2;

/** Fixture corpus: three current-version records persisted through the real store. */
const seedCorpus = () =>
  Effect.gen(function* () {
    const kv = memoryStringKv();
    const store = yield* openSessionStore({ kv });
    for (const id of ["m1", "m2", "m3"]) {
      yield* store.create(sampleSession(id));
    }
    return kv;
  });

describe("schema migrations", () => {
  test("the SHIPPED v1→v2 step migrates a real v1 record at open with default options", async () => {
    // A hand-built v1 record: ledger entries had no `failure` member.
    const request = { operation: "funding_broadcast", templateDigest: "ab".repeat(32) };
    const modern = encodeSession(
      sampleSession("legacy", {
        effectLedger: [
          {
            effectId: "fund-1",
            requestDigestHex: "cd".repeat(32),
            request,
            result: null,
            failure: null,
          },
        ],
      }),
    ) as { readonly effectLedger: ReadonlyArray<Record<string, unknown>> };
    const v1Payload = {
      ...modern,
      effectLedger: modern.effectLedger.map(({ failure: _failure, ...entry }) => entry),
    };
    const key = `${DEFAULT_KEY_PREFIX}session/legacy`;
    const kv = memoryStringKv({ [key]: serializeEnvelope(sealEnvelope(1, 1, v1Payload)) });
    const { session, snapshot } = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        return { session: yield* store.get("legacy"), snapshot: kv.snapshot() };
      }),
    );
    expect(session.effectLedger[0]?.failure).toBeNull();
    expect(session.effectLedger[0]?.effectId).toBe("fund-1");
    expect(parseEnvelope(snapshot[key]!)?.schemaVersion).toBe(CURRENT_SCHEMA_VERSION);
  });

  test("every step runs over every record in the corpus and rewrites it", async () => {
    const { sessions, snapshot } = await Effect.runPromise(
      Effect.gen(function* () {
        const kv = yield* seedCorpus();
        const store = yield* openSessionStore({
          kv,
          migrations: TEST_MIGRATIONS,
          currentVersion: SYNTHETIC_TARGET,
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
      expect(parseEnvelope(raw)?.schemaVersion).toBe(SYNTHETIC_TARGET);
    }
  });

  test("migrated records reopen cleanly at the new version with no further steps", async () => {
    const sessions = await Effect.runPromise(
      Effect.gen(function* () {
        const kv = yield* seedCorpus();
        yield* openSessionStore({ kv, migrations: TEST_MIGRATIONS, currentVersion: SYNTHETIC_TARGET });
        const reopened = yield* openSessionStore({ kv, migrations: [], currentVersion: SYNTHETIC_TARGET });
        return yield* reopened.list();
      }),
    );
    expect(sessions.length).toBe(3);
  });

  test("a record from a FUTURE schema version refuses store open loudly", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const corpusKv = yield* seedCorpus();
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
    expect(error.supported).toBe(CURRENT_SCHEMA_VERSION);
  });

  test("a missing sequential step refuses instead of skipping", async () => {
    const envelope = sealEnvelope(CURRENT_SCHEMA_VERSION, 1, encodeSession(sampleSession("gap")));
    const outcome = await Effect.runPromise(
      Effect.flip(
        migrateEnvelopePayload("k", envelope, [TEST_MIGRATIONS[1]!], SYNTHETIC_TARGET),
      ),
    );
    expect(outcome).toBeInstanceOf(MigrationStepMissingError);
    expect((outcome as MigrationStepMissingError).from).toBe(CURRENT_SCHEMA_VERSION);
  });

  test("a migration step that introduces secret material refuses at open, uncommitted", async () => {
    // The migration rewrite is a persist path: a step smuggling key-shaped
    // material must never be committed (and later exported) unscanned.
    const poison: SessionSchemaMigration = {
      from: CURRENT_SCHEMA_VERSION,
      to: CURRENT_SCHEMA_VERSION + 1,
      migrate: (payload) => ({
        ...(payload as Record<string, unknown>),
        engineSnapshot: { preimage: "never-persisted" },
      }),
    };
    const { refusal, snapshot } = await Effect.runPromise(
      Effect.gen(function* () {
        const kv = yield* seedCorpus();
        const refused = yield* Effect.flip(
          openSessionStore({ kv, migrations: [poison], currentVersion: CURRENT_SCHEMA_VERSION + 1 }),
        );
        return { refusal: refused, snapshot: kv.snapshot() };
      }),
    );
    expect(refusal).toBeInstanceOf(SecretMaterialError);
    expect(JSON.stringify(refusal)).not.toContain("never-persisted");
    // Nothing was rewritten at the poisoned version.
    for (const [key, raw] of Object.entries(snapshot)) {
      if (key.endsWith("!next")) continue;
      expect(parseEnvelope(raw)?.schemaVersion).toBe(CURRENT_SCHEMA_VERSION);
    }
  });
});
