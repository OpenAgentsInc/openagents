/**
 * Schema versioning — from the first commit (issue #9320 scope item 1).
 *
 * Records are versioned per envelope. Migration is sequential: every step
 * moves exactly one version forward and every stored record is rewritten at
 * the current version when the store opens (the Boltz pattern: storage
 * version 7, seven steps, each rewriting every record).
 *
 * Forward compatibility is a refusal, not a guess: a record whose version is
 * NEWER than this build understands fails store open loudly with
 * `UnsupportedSchemaVersionError`. There is no "best effort" read of a
 * future format on a funds path.
 */
import { Effect } from "effect";

import { MigrationStepMissingError, UnsupportedSchemaVersionError } from "./errors.js";
import type { RecordEnvelope } from "./journal.js";

/**
 * Version 1 was the first shipped schema. Version 2 added the definitive
 * `failure` state to every effect-ledger entry (`model.ts`
 * `ExternalEffectRecord.failure`), so a cancelled or rejected wallet call
 * can release the reload guard without teaching `priorEffectResult` to
 * suppress a legitimate retry.
 */
export const CURRENT_SCHEMA_VERSION = 2;

export interface SessionSchemaMigration {
  /** Source version. `to` is always `from + 1`; steps never skip. */
  readonly from: number;
  readonly to: number;
  /** Rewrite one record payload from `from`-shape to `to`-shape. */
  readonly migrate: (payload: unknown) => unknown;
}

/**
 * The shipped migration chain. Every step rewrites one whole record payload;
 * `openSessionStore` runs the chain over every stored record at open, and
 * `importPrivateHistory` runs it per imported session, so a document from an
 * older build always ingests.
 */
export const SESSION_STORE_MIGRATIONS: ReadonlyArray<SessionSchemaMigration> = [
  {
    from: 1,
    to: 2,
    // v1 effect-ledger entries had no `failure` member (pending was simply
    // `result === null`). v2 makes "definitively failed" a distinct state;
    // every v1 entry migrates as never-failed.
    migrate: (payload) => {
      const session = payload as { readonly effectLedger?: ReadonlyArray<Record<string, unknown>> };
      return {
        ...(payload as Record<string, unknown>),
        effectLedger: (session.effectLedger ?? []).map((entry) => ({ ...entry, failure: null })),
      };
    },
  },
];

export interface MigratedPayload {
  readonly payload: unknown;
  /** True when at least one step ran and the record must be rewritten. */
  readonly rewritten: boolean;
}

export const migrateEnvelopePayload = (
  key: string,
  envelope: RecordEnvelope,
  migrations: ReadonlyArray<SessionSchemaMigration>,
  currentVersion: number,
): Effect.Effect<MigratedPayload, UnsupportedSchemaVersionError | MigrationStepMissingError> =>
  Effect.gen(function* () {
    if (envelope.schemaVersion > currentVersion) {
      return yield* new UnsupportedSchemaVersionError({
        key,
        found: envelope.schemaVersion,
        supported: currentVersion,
      });
    }
    let payload = envelope.payload;
    let version = envelope.schemaVersion;
    while (version < currentVersion) {
      const step = migrations.find((migration) => migration.from === version);
      if (step === undefined || step.to !== version + 1) {
        return yield* new MigrationStepMissingError({ from: version, to: version + 1 });
      }
      payload = step.migrate(payload);
      version = step.to;
    }
    return { payload, rewritten: version !== envelope.schemaVersion };
  });
