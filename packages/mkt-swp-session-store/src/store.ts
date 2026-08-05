/**
 * The local swap session store (openagents#9320, SWAP-5).
 *
 * - Journaled, digest-verified writes (`journal.ts`): a crash mid-write can
 *   never surface a partial record as complete.
 * - Serialised read-modify-write under a named per-session lock plus one
 *   store-wide lock for create/delete/import, because a status fold and a
 *   background task WILL race (teardown §5: Boltz serialises per swap id and
 *   takes a second global lock for claims).
 * - Schema versioning with sequential migrations that rewrite every record
 *   at open (`migrate.ts`); future versions are refused loudly.
 * - The custody tripwire runs on every payload before it touches storage.
 *
 * One store instance is the write authority for its backing area within a
 * tab. Cross-tab write coordination (Web Locks) is app-shell wiring that
 * arrives with SWAP-0/SWAP-7; the journal keeps even uncoordinated writers
 * from tearing records.
 */
import { Effect, Schema, Semaphore } from "effect";

import { contentDigestHex } from "./canonical.js";
import {
  EffectBindingConflictError,
  MigrationStepMissingError,
  SecretMaterialError,
  SessionAlreadyExistsError,
  SessionNotFoundError,
  SessionRecordInvalidError,
  SignedRecordConflictError,
  StorageDriverError,
  TornSessionRecordError,
  UnsupportedSchemaVersionError,
} from "./errors.js";
import { commitRecord, deleteRecord, loadRecord, sealEnvelope } from "./journal.js";
import type { StringKv } from "./kv.js";
import { CURRENT_SCHEMA_VERSION, SESSION_STORE_MIGRATIONS, migrateEnvelopePayload, type SessionSchemaMigration } from "./migrate.js";
import { StoredSwapSession, type ExternalEffectRecord, type SignedNostrRecord } from "./model.js";
import { assertNoSecretMaterial } from "./secret-boundary.js";

export const DEFAULT_KEY_PREFIX = "oa.swp.";

export type SessionWriteError =
  | StorageDriverError
  | SessionRecordInvalidError
  | SecretMaterialError;

export type SessionStoreOpenError =
  | StorageDriverError
  | TornSessionRecordError
  | UnsupportedSchemaVersionError
  | MigrationStepMissingError
  | SessionRecordInvalidError;

export interface SessionStore {
  /** Every stored session, unordered. */
  readonly list: () => Effect.Effect<ReadonlyArray<StoredSwapSession>>;
  readonly get: (sessionId: string) => Effect.Effect<StoredSwapSession, SessionNotFoundError>;
  readonly create: (
    session: StoredSwapSession,
  ) => Effect.Effect<void, SessionWriteError | SessionAlreadyExistsError>;
  /**
   * Serialised read-modify-write under this session's named lock. `modify`
   * may be effectful (a status fold, a background observation task); two
   * concurrent updates to one session always see each other's writes.
   */
  readonly update: <E>(
    sessionId: string,
    modify: (session: StoredSwapSession) => Effect.Effect<StoredSwapSession, E>,
  ) => Effect.Effect<StoredSwapSession, E | SessionNotFoundError | SessionWriteError>;
  /**
   * Append one accepted signed record. Exact replay (same id, same bytes) is
   * idempotent; changed bytes at an existing event id fail closed.
   */
  readonly appendSignedRecord: (
    sessionId: string,
    record: SignedNostrRecord,
  ) => Effect.Effect<void, SessionNotFoundError | SessionWriteError | SignedRecordConflictError>;
  /**
   * Durably record an external effect request BEFORE the side effect runs.
   * Re-recording the same effectId with the same request digest is a no-op;
   * a different digest fails closed.
   */
  readonly recordEffectRequest: (
    sessionId: string,
    effectId: string,
    request: unknown,
  ) => Effect.Effect<void, SessionNotFoundError | SessionWriteError | EffectBindingConflictError>;
  readonly recordEffectResult: (
    sessionId: string,
    effectId: string,
    result: unknown,
    externalId: string | null,
  ) => Effect.Effect<void, SessionNotFoundError | SessionWriteError | EffectBindingConflictError>;
  /**
   * The idempotency gate for resume: a persisted result for this effectId
   * (with a matching request digest) means the external operation already
   * ran — the caller must suppress the callback and reuse the result.
   */
  readonly priorEffectResult: (
    sessionId: string,
    effectId: string,
    request: unknown,
  ) => Effect.Effect<ExternalEffectRecord | null, SessionNotFoundError | EffectBindingConflictError>;
  readonly delete: (sessionId: string) => Effect.Effect<void, StorageDriverError>;
  /** Raw upsert used by import after full validation. Takes the store lock. */
  readonly putValidated: (
    session: StoredSwapSession,
  ) => Effect.Effect<void, SessionWriteError>;
}

export interface OpenSessionStoreOptions {
  readonly kv: StringKv;
  readonly keyPrefix?: string;
  readonly migrations?: ReadonlyArray<SessionSchemaMigration>;
  /** Overridable for migration tests only. */
  readonly currentVersion?: number;
  readonly now?: () => number;
}

const encodeSession = Schema.encodeUnknownSync(StoredSwapSession);
const decodeSession = Schema.decodeUnknownSync(StoredSwapSession);

export const openSessionStore = (
  options: OpenSessionStoreOptions,
): Effect.Effect<SessionStore, SessionStoreOpenError> =>
  Effect.gen(function* () {
    const kv = options.kv;
    const prefix = options.keyPrefix ?? DEFAULT_KEY_PREFIX;
    const sessionPrefix = `${prefix}session/`;
    const migrations = options.migrations ?? SESSION_STORE_MIGRATIONS;
    const currentVersion = options.currentVersion ?? CURRENT_SCHEMA_VERSION;
    const now = options.now ?? (() => Date.now());

    const keyOf = (sessionId: string) => `${sessionPrefix}${encodeURIComponent(sessionId)}`;

    const cache = new Map<string, StoredSwapSession>();
    const writeSeqs = new Map<string, number>();

    // Open: load every record through journal recovery, run migrations,
    // decode, and rewrite migrated records at the current version.
    const allKeys = yield* kv.keys(sessionPrefix);
    const recordKeys = new Set(
      allKeys.map((key) => (key.endsWith("!next") ? key.slice(0, -"!next".length) : key)),
    );
    for (const key of recordKeys) {
      const envelope = yield* loadRecord(kv, key);
      if (envelope === null) continue;
      const migrated = yield* migrateEnvelopePayload(key, envelope, migrations, currentVersion);
      const session = yield* Effect.try({
        try: () => decodeSession(migrated.payload),
        catch: (error) =>
          new SessionRecordInvalidError({
            key,
            detail: error instanceof Error ? error.message : String(error),
          }),
      });
      if (migrated.rewritten) {
        const nextSeq = envelope.writeSeq + 1;
        yield* commitRecord(kv, key, sealEnvelope(currentVersion, nextSeq, migrated.payload));
        writeSeqs.set(key, nextSeq);
      } else {
        writeSeqs.set(key, envelope.writeSeq);
      }
      cache.set(session.sessionId, session);
    }

    const storeLock = Semaphore.makeUnsafe(1);
    const sessionLocks = new Map<string, Semaphore.Semaphore>();
    const lockOf = (sessionId: string): Semaphore.Semaphore => {
      const existing = sessionLocks.get(sessionId);
      if (existing !== undefined) return existing;
      const created = Semaphore.makeUnsafe(1);
      sessionLocks.set(sessionId, created);
      return created;
    };

    const persist = (
      session: StoredSwapSession,
    ): Effect.Effect<void, SessionWriteError> =>
      Effect.gen(function* () {
        const payload = yield* Effect.try({
          try: () => encodeSession(session),
          catch: (error) =>
            new SessionRecordInvalidError({
              key: keyOf(session.sessionId),
              detail: error instanceof Error ? error.message : String(error),
            }),
        });
        yield* assertNoSecretMaterial(payload, `session(${session.sessionId})`);
        const key = keyOf(session.sessionId);
        const nextSeq = (writeSeqs.get(key) ?? 0) + 1;
        yield* commitRecord(kv, key, sealEnvelope(currentVersion, nextSeq, payload));
        writeSeqs.set(key, nextSeq);
        cache.set(session.sessionId, session);
      });

    const getOrFail = (sessionId: string): Effect.Effect<StoredSwapSession, SessionNotFoundError> => {
      const session = cache.get(sessionId);
      return session === undefined
        ? Effect.fail(new SessionNotFoundError({ sessionId }))
        : Effect.succeed(session);
    };

    const update = <E>(
      sessionId: string,
      modify: (session: StoredSwapSession) => Effect.Effect<StoredSwapSession, E>,
    ): Effect.Effect<StoredSwapSession, E | SessionNotFoundError | SessionWriteError> =>
      lockOf(sessionId).withPermits(1)(
        Effect.gen(function* () {
          const current = yield* getOrFail(sessionId);
          const modified = yield* modify(current);
          const stamped: StoredSwapSession = { ...modified, sessionId, updatedAt: now() };
          yield* persist(stamped);
          return stamped;
        }),
      );

    const digestOf = (value: unknown, sessionId: string): Effect.Effect<string, SessionRecordInvalidError> =>
      Effect.try({
        try: () => contentDigestHex(value),
        catch: (error) =>
          new SessionRecordInvalidError({
            key: keyOf(sessionId),
            detail: error instanceof Error ? error.message : String(error),
          }),
      });

    const store: SessionStore = {
      list: () => Effect.sync(() => [...cache.values()]),
      get: getOrFail,
      create: (session) =>
        storeLock.withPermits(1)(
          Effect.gen(function* () {
            if (cache.has(session.sessionId)) {
              return yield* new SessionAlreadyExistsError({ sessionId: session.sessionId });
            }
            yield* persist({ ...session, updatedAt: now() });
          }),
        ),
      update,
      appendSignedRecord: (sessionId, record) =>
        update(sessionId, (session) =>
          Effect.gen(function* () {
            const existing = session.signedRecords.find((candidate) => candidate.id === record.id);
            if (existing !== undefined) {
              const identical = contentDigestHex(existing) === contentDigestHex(record);
              if (identical) return session; // exact replay is idempotent
              return yield* new SignedRecordConflictError({ sessionId, eventId: record.id });
            }
            return { ...session, signedRecords: [...session.signedRecords, record] };
          }),
        ).pipe(Effect.asVoid),
      recordEffectRequest: (sessionId, effectId, request) =>
        Effect.gen(function* () {
          const requestDigestHex = yield* digestOf(request, sessionId);
          yield* update(sessionId, (session) =>
            Effect.gen(function* () {
              const existing = session.effectLedger.find((entry) => entry.effectId === effectId);
              if (existing !== undefined) {
                if (existing.requestDigestHex !== requestDigestHex) {
                  return yield* new EffectBindingConflictError({
                    sessionId,
                    effectId,
                    reason: "request_digest_mismatch",
                  });
                }
                return session; // durable request already recorded
              }
              const entry: ExternalEffectRecord = {
                effectId,
                requestDigestHex,
                request,
                result: null,
              };
              return { ...session, effectLedger: [...session.effectLedger, entry] };
            }),
          );
        }),
      recordEffectResult: (sessionId, effectId, result, externalId) =>
        Effect.gen(function* () {
          const resultDigestHex = yield* digestOf(result, sessionId);
          yield* update(sessionId, (session) =>
            Effect.gen(function* () {
              const existing = session.effectLedger.find((entry) => entry.effectId === effectId);
              if (existing === undefined) {
                return yield* new EffectBindingConflictError({
                  sessionId,
                  effectId,
                  reason: "result_without_request",
                });
              }
              if (existing.result !== null) {
                if (existing.result.resultDigestHex !== resultDigestHex) {
                  return yield* new EffectBindingConflictError({
                    sessionId,
                    effectId,
                    reason: "result_digest_mismatch",
                  });
                }
                return session;
              }
              const updated: ExternalEffectRecord = {
                ...existing,
                result: { resultDigestHex, externalId, observedAt: now(), result },
              };
              return {
                ...session,
                effectLedger: session.effectLedger.map((entry) =>
                  entry.effectId === effectId ? updated : entry,
                ),
              };
            }),
          );
        }),
      priorEffectResult: (sessionId, effectId, request) =>
        Effect.gen(function* () {
          const session = yield* getOrFail(sessionId);
          const entry = session.effectLedger.find((candidate) => candidate.effectId === effectId);
          if (entry === undefined) return null;
          const requestDigestHex = contentDigestHex(request);
          if (entry.requestDigestHex !== requestDigestHex) {
            return yield* new EffectBindingConflictError({
              sessionId,
              effectId,
              reason: "request_digest_mismatch",
            });
          }
          return entry.result === null ? null : entry;
        }),
      delete: (sessionId) =>
        storeLock.withPermits(1)(
          Effect.gen(function* () {
            yield* deleteRecord(kv, keyOf(sessionId));
            cache.delete(sessionId);
            sessionLocks.delete(sessionId);
          }),
        ),
      putValidated: (session) =>
        storeLock.withPermits(1)(persist({ ...session })),
    };

    return store;
  });
