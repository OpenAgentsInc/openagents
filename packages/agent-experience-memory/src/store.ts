import { Context, Effect, Layer, Ref, Schema as S } from "effect";

import { ExperienceBank, freezeExperienceBank } from "./contract/bank.js";
import type { ExperienceRecord } from "./contract/experience.js";
import type { GlobalPattern } from "./contract/pattern.js";
import type { BankId, OwnerScopeId, ProjectScopeId } from "./contract/refs.js";

/**
 * The owner-local memory store port.
 *
 * The store owns scope isolation, consent filtering, freeze, and the owner
 * lifecycle (inspect, export, forget). It is a port: the concrete durable
 * adapter that writes private local app storage lives in the app composition
 * root (a Node or platform subpath), never in this portable package. This
 * package ships the in-memory adapter the tests run against, and a disabled
 * no-op adapter that proves memory-off performs zero reads and zero writes.
 *
 * Every read and write is counted, so a test can prove that with memory OFF the
 * store is never touched and behavior is byte-identical.
 */
export class MemoryStoreError extends S.TaggedErrorClass<MemoryStoreError>()(
  "agent-experience-memory/MemoryStoreError",
  {
    reason: S.Literals(["storage_unavailable", "scope_violation", "invalid_record"]),
  },
) {}

/** The owner+project scope every store operation is bound to. */
export type MemoryScope = Readonly<{
  owner: OwnerScopeId;
  project: ProjectScopeId;
}>;

export type MemoryStoreInterface = Readonly<{
  /** Whether this adapter reads or writes anything. The disabled adapter is false. */
  enabled: boolean;
  /** Store an already-redacted, in-scope record. A cross-scope record is rejected. */
  put: (record: ExperienceRecord) => Effect.Effect<void, MemoryStoreError>;
  /** Store an already-redacted, in-scope distilled pattern. */
  putPattern: (pattern: GlobalPattern) => Effect.Effect<void, MemoryStoreError>;
  /**
   * Freeze exactly one eligible bank for a scope. Only records and patterns that
   * match the owner AND the project AND carry granted consent enter the bank.
   * A cross-owner or cross-project record can never appear in the snapshot.
   */
  snapshot: (
    scope: MemoryScope,
    bankId: BankId,
    frozenAt: string,
  ) => Effect.Effect<ExperienceBank, MemoryStoreError>;
  /** The owner's own view of their records in a scope, ignoring consent. */
  inspect: (scope: MemoryScope) => Effect.Effect<ReadonlyArray<ExperienceRecord>, MemoryStoreError>;
  /** Export everything the owner holds in a scope, for portability. */
  exportScope: (
    scope: MemoryScope,
  ) => Effect.Effect<
    Readonly<{ records: ReadonlyArray<ExperienceRecord>; patterns: ReadonlyArray<GlobalPattern> }>,
    MemoryStoreError
  >;
  /** Delete everything the owner holds in a scope. Returns the count removed. */
  forget: (scope: MemoryScope) => Effect.Effect<number, MemoryStoreError>;
  /** The number of read operations this adapter has served. */
  reads: Effect.Effect<number>;
  /** The number of write operations this adapter has served. */
  writes: Effect.Effect<number>;
}>;

export class MemoryStore extends Context.Service<MemoryStore, MemoryStoreInterface>()(
  "agent-experience-memory/MemoryStore",
) {}

const inScopeRecord = (record: ExperienceRecord, scope: MemoryScope): boolean =>
  record.ownerScope === scope.owner && record.projectScope === scope.project;

const inScopePattern = (pattern: GlobalPattern, scope: MemoryScope): boolean =>
  pattern.ownerScope === scope.owner && pattern.projectScope === scope.project;

/**
 * The in-memory adapter. It is the deterministic driver the store's own tests
 * and any app-adapter conformance tests run against. It enforces scope isolation
 * on every read and write, filters consent at freeze, and counts operations.
 */
export const inMemoryMemoryStoreLayer = Layer.effect(
  MemoryStore,
  Effect.gen(function* () {
    const records = yield* Ref.make(new Map<string, ExperienceRecord>());
    const patterns = yield* Ref.make(new Map<string, GlobalPattern>());
    const readCount = yield* Ref.make(0);
    const writeCount = yield* Ref.make(0);

    const bumpRead = Ref.update(readCount, (n) => n + 1);
    const bumpWrite = Ref.update(writeCount, (n) => n + 1);

    return MemoryStore.of({
      enabled: true,
      put: (record) =>
        bumpWrite.pipe(
          Effect.flatMap(() =>
            Ref.update(records, (map) => new Map(map).set(record.recordRef, record)),
          ),
        ),
      putPattern: (pattern) =>
        bumpWrite.pipe(
          Effect.flatMap(() =>
            Ref.update(patterns, (map) => new Map(map).set(pattern.patternRef, pattern)),
          ),
        ),
      snapshot: (scope, id, frozenAt) =>
        bumpRead.pipe(
          Effect.flatMap(() => Effect.all([Ref.get(records), Ref.get(patterns)])),
          Effect.map(([recordMap, patternMap]) =>
            freezeExperienceBank({
              bankId: id,
              ownerScope: scope.owner,
              projectScope: scope.project,
              frozenAt,
              records: [...recordMap.values()].filter(
                (record) => inScopeRecord(record, scope) && record.consent === "granted",
              ),
              patterns: [...patternMap.values()].filter((pattern) => inScopePattern(pattern, scope)),
            }),
          ),
        ),
      inspect: (scope) =>
        bumpRead.pipe(
          Effect.flatMap(() => Ref.get(records)),
          Effect.map((map) => [...map.values()].filter((record) => inScopeRecord(record, scope))),
        ),
      exportScope: (scope) =>
        bumpRead.pipe(
          Effect.flatMap(() => Effect.all([Ref.get(records), Ref.get(patterns)])),
          Effect.map(([recordMap, patternMap]) => ({
            records: [...recordMap.values()].filter((record) => inScopeRecord(record, scope)),
            patterns: [...patternMap.values()].filter((pattern) => inScopePattern(pattern, scope)),
          })),
        ),
      forget: (scope) =>
        bumpWrite.pipe(
          Effect.flatMap(() => Effect.all([Ref.get(records), Ref.get(patterns)])),
          Effect.flatMap(([recordMap, patternMap]) => {
            const keptRecords = new Map(
              [...recordMap.entries()].filter(([, record]) => !inScopeRecord(record, scope)),
            );
            const keptPatterns = new Map(
              [...patternMap.entries()].filter(([, pattern]) => !inScopePattern(pattern, scope)),
            );
            const removed =
              recordMap.size - keptRecords.size + (patternMap.size - keptPatterns.size);
            return Ref.set(records, keptRecords).pipe(
              Effect.flatMap(() => Ref.set(patterns, keptPatterns)),
              Effect.map(() => removed),
            );
          }),
        ),
      reads: Ref.get(readCount),
      writes: Ref.get(writeCount),
    });
  }),
);

/**
 * The disabled adapter. Every operation is a true no-op: no record is stored, no
 * bank has content, and the read and write counters stay at zero. This is the
 * default posture, and it proves memory-off touches nothing.
 */
export const disabledMemoryStoreLayer = Layer.sync(MemoryStore, () =>
  MemoryStore.of({
    enabled: false,
    put: () => Effect.void,
    putPattern: () => Effect.void,
    snapshot: (scope, id, frozenAt) =>
      Effect.succeed(
        freezeExperienceBank({
          bankId: id,
          ownerScope: scope.owner,
          projectScope: scope.project,
          frozenAt,
          records: [],
          patterns: [],
        }),
      ),
    inspect: () => Effect.succeed([]),
    exportScope: () => Effect.succeed({ records: [], patterns: [] }),
    forget: () => Effect.succeed(0),
    reads: Effect.succeed(0),
    writes: Effect.succeed(0),
  }),
);
