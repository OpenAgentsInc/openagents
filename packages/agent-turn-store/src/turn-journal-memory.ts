import { Effect, Layer, Ref, Schema as S } from "effect";

import {
  CandidateRef,
  ProviderTurnRef,
  TurnLifecycleState,
  TurnProviderRef,
  TurnRefusalReason,
  TurnRequestRef,
  TurnThreadRef,
} from "@openagentsinc/agent-runtime-schema";
import {
  TurnJournal,
  TurnJournalError,
  type TurnStateRecord,
} from "@openagentsinc/agent-turn-runtime";

/**
 * AFS-01 driver-neutral turn journal state, migrations, and in-memory adapter.
 *
 * The store owns the driver-neutral persisted state and its migrations; it does
 * not own a platform driver in this root export. A Node, Expo, or browser driver
 * lives in a platform subpath or an app composition root. The in-memory adapter
 * here is the deterministic driver both the store's own tests and the Desktop
 * adapter tests run the shared state-transition corpus against.
 */
export const TURN_STORE_RECORD_SCHEMA_LITERAL = "openagents.agent_turn_store_record.v1" as const;

const NullableInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

/**
 * The driver-neutral persisted turn record. It is exactly the kernel
 * `TurnStateRecord` fields plus a versioned schema literal. A migration adds a
 * new decoder path; it never reinterprets an old identifier.
 */
export const PersistedTurnRecord = S.Struct({
  schema: S.Literal(TURN_STORE_RECORD_SCHEMA_LITERAL),
  requestRef: TurnRequestRef,
  threadRef: TurnThreadRef,
  state: TurnLifecycleState,
  generation: NullableInt,
  providerTurnRef: S.NullOr(ProviderTurnRef),
  selected: S.NullOr(TurnProviderRef),
  effective: S.NullOr(TurnProviderRef),
  candidateRef: S.NullOr(CandidateRef),
  refusalReason: S.NullOr(TurnRefusalReason),
  progressCount: NullableInt,
});
export type PersistedTurnRecord = typeof PersistedTurnRecord.Type;

const decodePersisted = S.decodeUnknownSync(PersistedTurnRecord);

/**
 * Migrate an unknown persisted value to the current version. A record with no
 * schema literal is treated as the original unversioned shape and stamped with
 * the current literal. This is the single, additive migration path.
 */
export const migratePersistedTurnRecord = (value: unknown): unknown => {
  if (typeof value === "object" && value !== null && !("schema" in value)) {
    return { schema: TURN_STORE_RECORD_SCHEMA_LITERAL, ...value };
  }
  return value;
};

/** Serialize a kernel record to its driver-neutral persisted shape. */
export const encodeTurnRecord = (record: TurnStateRecord): PersistedTurnRecord =>
  decodePersisted({ schema: TURN_STORE_RECORD_SCHEMA_LITERAL, ...record });

/** Deserialize (and migrate) a persisted value back to a kernel record. */
export const decodeTurnRecord = (value: unknown): TurnStateRecord => {
  const { schema: _schema, ...record } = decodePersisted(migratePersistedTurnRecord(value));
  return record;
};

const journalError = (reason: "storage_unavailable" | "invalid_record") =>
  new TurnJournalError({ reason });

/**
 * The in-memory turn journal layer. It round-trips every record through the
 * driver-neutral persisted schema, so a serialization defect fails a test the
 * same way a real driver would.
 */
export const inMemoryTurnJournalLayer = Layer.effect(
  TurnJournal,
  Effect.gen(function* () {
    const store = yield* Ref.make(new Map<string, PersistedTurnRecord>());
    return TurnJournal.of({
      record: (state) =>
        Effect.try({ try: () => encodeTurnRecord(state), catch: () => journalError("invalid_record") }).pipe(
          Effect.flatMap((encoded) =>
            Ref.update(store, (map) => new Map(map).set(state.requestRef, encoded)),
          ),
        ),
      load: (requestRef: TurnRequestRef) =>
        Ref.get(store).pipe(
          Effect.flatMap((map) => {
            const persisted = map.get(requestRef);
            return persisted === undefined
              ? Effect.succeed<TurnStateRecord | null>(null)
              : Effect.try({ try: () => decodeTurnRecord(persisted), catch: () => journalError("invalid_record") });
          }),
        ),
      list: Ref.get(store).pipe(
        Effect.flatMap((map) =>
          Effect.try({
            try: () => [...map.values()].map((persisted) => decodeTurnRecord(persisted)),
            catch: () => journalError("invalid_record"),
          }),
        ),
      ),
    });
  }),
);
