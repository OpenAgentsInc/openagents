/**
 * Sol claim-ledger durable EventStore (forge Stage 2, #9185).
 *
 * The prior slices in this issue built the two halves that this store sits
 * between:
 *   - `@openagentsinc/forge-protocol` — the pure projection of the Sol claim
 *     record to and from unsigned NIP-34 event templates.
 *   - `./sol-claim-ledger-relay` — the wire layer: sign, serialize to NIP-01
 *     relay frames, parse back, verify the schnorr signature, and recover the
 *     typed claim entry.
 *
 * The top residual was a *durable relay `EventStore` for the claim-ledger
 * coordinate*: append signed claim-ledger events and query them back by the
 * NIP-34 repository coordinate (`#a` = `30617:<pubkey>:<repoId>`). That is what
 * this module owns.
 *
 * Design:
 * - The store owns all trust and semantics. Every event is verified on the way
 *   IN (append) and again on the way OUT (query), so a shared or tampered
 *   persistence layer can never make the store return an event whose signature
 *   or ledger shape does not hold. Only entries that verify and carry the
 *   ledger tag are admitted; a repository coordinate (`#a`) is required.
 * - Persistence is a narrow port (`SolClaimLedgerEventPersistence`): store one
 *   frame keyed by event id (append-only, dedup by id), and fetch every stored
 *   frame for one repository coordinate newest-first. The store applies the
 *   kind / work-item / author / since / limit narrowing itself, so the filter
 *   semantics have a single source of truth shared by every backend.
 * - Two backends implement the port: an in-memory adapter (fully exercised by
 *   the test suite) and a Postgres adapter over the repo's `SyncSql` seam
 *   (the same durable Cloud SQL seam the token ledger uses). Because the store
 *   holds the semantics, both backends stay trivial and behave identically.
 *
 * This keeps the durable store on the Effect / Node host contract, dependency
 * light (no new dependency; it consumes the already-pinned `nostr-effect`
 * signer through the relay module), and does not edit `nostr-effect`.
 */
import { Data, Effect } from "effect";

import {
  type SignedNostrEvent,
  type SolClaimLedgerFilterOptions,
  type VerifiedSolClaimLedgerEntry,
  SolClaimLedgerNotAnEntryError,
  SolClaimLedgerRelayFrameError,
  SolClaimLedgerSignatureError,
  parseRelayEventMessage,
  verifiedSolClaimLedgerEntry,
} from "./sol-claim-ledger-relay";

// =============================================================================
// Errors
// =============================================================================

/** Raised when the backing persistence layer fails an operation. */
export class SolClaimLedgerStorageError extends Data.TaggedError("SolClaimLedgerStorageError")<{
  readonly operation: string;
  readonly messageSafe: string;
}> {}

/** Raised when an event carries no NIP-34 repository coordinate (`#a`) tag. */
export class SolClaimLedgerMissingCoordinateError extends Data.TaggedError(
  "SolClaimLedgerMissingCoordinateError",
)<{
  readonly eventId: string;
}> {}

// =============================================================================
// Stored row
// =============================================================================

/**
 * One persisted claim-ledger event: the canonical signed NIP-01 event object
 * serialized as JSON (`frame`) plus the indexed fields the coordinate query
 * needs. `frame` is the whole signed event, so the store can re-verify it on
 * read without trusting any index column.
 */
export type StoredSolClaimLedgerEvent = Readonly<{
  id: string;
  kind: number;
  pubkey: string;
  createdAt: number;
  repositoryCoordinate: string;
  workItemRef: string | null;
  frame: string;
}>;

// =============================================================================
// Persistence port
// =============================================================================

/**
 * The narrow durable seam the store sits on. Implementations persist one frame
 * per event id (append-only; a repeat id is a no-op duplicate) and fetch every
 * frame for one repository coordinate, newest `created_at` first. All filter
 * narrowing beyond the coordinate lives in the store, so every backend stays
 * identical in behavior.
 */
export interface SolClaimLedgerEventPersistence {
  /**
   * Persist one frame. Returns `true` when the row is new, `false` when an
   * event with this id already exists (dedup by id — NIP-01 events are
   * content-addressed, so a repeat id is the same event).
   */
  readonly insert: (
    row: StoredSolClaimLedgerEvent,
  ) => Effect.Effect<boolean, SolClaimLedgerStorageError>;
  /** Fetch every stored frame for one repository coordinate, newest first. */
  readonly queryByCoordinate: (
    repositoryCoordinate: string,
  ) => Effect.Effect<ReadonlyArray<StoredSolClaimLedgerEvent>, SolClaimLedgerStorageError>;
  /** Total stored frame count (all coordinates). */
  readonly count: () => Effect.Effect<number, SolClaimLedgerStorageError>;
}

// =============================================================================
// Tag extraction (bounded deterministic index fields)
// =============================================================================

const safeMessage = (error: unknown): string =>
  (error instanceof Error ? error.message : String(error)).replaceAll(/\s+/g, " ").slice(0, 200);

const firstTagValue = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  name: string,
): string | undefined => {
  for (const tag of tags) {
    if (tag[0] === name && typeof tag[1] === "string" && tag[1].length > 0) {
      return tag[1];
    }
  }
  return undefined;
};

// =============================================================================
// Store
// =============================================================================

/** The result of appending one signed event to the durable ledger. */
export type SolClaimLedgerAppendResult = Readonly<{
  /** `true` when newly stored, `false` when it was already present (dedup). */
  stored: boolean;
  /** The verified typed entry recovered from the event. */
  entry: VerifiedSolClaimLedgerEntry;
  /** The event id (NIP-01 content address). */
  eventId: string;
  /** The NIP-34 repository coordinate the event was filed under. */
  repositoryCoordinate: string;
}>;

/**
 * A durable relay EventStore scoped to the Sol claim-ledger coordinate.
 *
 * `append` verifies a signed event, requires it to be a tagged ledger entry
 * with a repository coordinate, and persists it (dedup by id).
 * `queryRepositoryCoordinate` returns the verified entries for one coordinate,
 * newest first, after applying the same kind / work-item / author / since /
 * limit narrowing as the relay subscription filter.
 */
export type SolClaimLedgerAppendError =
  | SolClaimLedgerStorageError
  | SolClaimLedgerMissingCoordinateError
  | SolClaimLedgerSignatureError
  | SolClaimLedgerNotAnEntryError;

export interface SolClaimLedgerEventStore {
  readonly append: (
    event: SignedNostrEvent,
  ) => Effect.Effect<SolClaimLedgerAppendResult, SolClaimLedgerAppendError>;
  readonly appendFromRelayMessage: (
    raw: string,
  ) => Effect.Effect<
    SolClaimLedgerAppendResult,
    SolClaimLedgerAppendError | SolClaimLedgerRelayFrameError
  >;
  readonly queryRepositoryCoordinate: (
    repositoryCoordinate: string,
    options?: SolClaimLedgerFilterOptions,
  ) => Effect.Effect<ReadonlyArray<VerifiedSolClaimLedgerEntry>, SolClaimLedgerStorageError>;
  readonly count: () => Effect.Effect<number, SolClaimLedgerStorageError>;
}

const canonicalFrame = (event: SignedNostrEvent): string =>
  JSON.stringify({
    id: event.id,
    pubkey: event.pubkey,
    created_at: event.created_at,
    kind: event.kind,
    tags: event.tags,
    content: event.content,
    sig: event.sig,
  });

/**
 * The bounded index fields a claim-ledger subscription filter narrows on
 * (everything beyond the repository coordinate itself). A `StoredSolClaimLedgerEvent`
 * satisfies this shape structurally, and the live subscription runtime reuses
 * the same predicate so a stored query and a live delivery filter identically.
 */
export interface SolClaimLedgerFilterableFields {
  readonly kind: number;
  readonly pubkey: string;
  readonly createdAt: number;
  readonly workItemRef: string | null;
}

/**
 * The single source of truth for the non-coordinate filter narrowing (kind /
 * work-item / author / since). The coordinate match is applied separately by
 * the caller. Shared by the durable coordinate query and the live subscription
 * runtime so a fresh read and a streamed delivery never diverge.
 */
export const solClaimLedgerEventMatchesOptions = (
  fields: SolClaimLedgerFilterableFields,
  options: SolClaimLedgerFilterOptions,
): boolean => {
  if (options.kinds !== undefined && !options.kinds.includes(fields.kind)) {
    return false;
  }
  if (options.workItemRef !== undefined && fields.workItemRef !== options.workItemRef) {
    return false;
  }
  if (options.authors !== undefined && !options.authors.includes(fields.pubkey)) {
    return false;
  }
  if (options.sinceEpochSeconds !== undefined && fields.createdAt < options.sinceEpochSeconds) {
    return false;
  }
  return true;
};

/**
 * Assemble a durable claim-ledger EventStore over a persistence backend. The
 * store owns verification and filtering; the backend only stores and fetches
 * frames.
 */
export const makeSolClaimLedgerEventStore = (
  persistence: SolClaimLedgerEventPersistence,
): SolClaimLedgerEventStore => {
  const append = (
    event: SignedNostrEvent,
  ): Effect.Effect<SolClaimLedgerAppendResult, SolClaimLedgerAppendError> =>
    Effect.gen(function* () {
      // Verify signature + recover the typed entry. A bad signature or a
      // non-ledger event throws synchronously from the relay layer; convert
      // those into typed failures (never a silent admit, never a defect).
      const entry = yield* Effect.try({
        try: () => verifiedSolClaimLedgerEntry(event),
        catch: (error) =>
          error instanceof SolClaimLedgerNotAnEntryError
            ? error
            : new SolClaimLedgerSignatureError(),
      });
      const repositoryCoordinate = firstTagValue(event.tags, "a");
      if (repositoryCoordinate === undefined) {
        return yield* new SolClaimLedgerMissingCoordinateError({
          eventId: event.id,
        });
      }
      const row: StoredSolClaimLedgerEvent = {
        id: event.id,
        kind: event.kind,
        pubkey: event.pubkey,
        createdAt: event.created_at,
        repositoryCoordinate,
        workItemRef: firstTagValue(event.tags, "sol.work_item") ?? null,
        frame: canonicalFrame(event),
      };
      const stored = yield* persistence.insert(row);
      return { stored, entry, eventId: event.id, repositoryCoordinate };
    });

  const reverifyStoredRow = (
    row: StoredSolClaimLedgerEvent,
  ): Effect.Effect<VerifiedSolClaimLedgerEntry, SolClaimLedgerStorageError> =>
    Effect.try({
      try: () =>
        verifiedSolClaimLedgerEntry(
          parseRelayEventMessage(JSON.stringify(["EVENT", JSON.parse(row.frame)])),
        ),
      catch: (error) =>
        new SolClaimLedgerStorageError({
          operation: "verifyStoredFrame",
          messageSafe: safeMessage(error),
        }),
    });

  return {
    append,
    appendFromRelayMessage: (raw) =>
      Effect.try({
        try: () => parseRelayEventMessage(raw),
        catch: (error) =>
          error instanceof SolClaimLedgerRelayFrameError
            ? error
            : new SolClaimLedgerRelayFrameError(safeMessage(error)),
      }).pipe(Effect.flatMap(append)),
    queryRepositoryCoordinate: (repositoryCoordinate, options = {}) =>
      Effect.gen(function* () {
        const rows = yield* persistence.queryByCoordinate(repositoryCoordinate);
        const matched = rows.filter((row) => solClaimLedgerEventMatchesOptions(row, options));
        // Newest first; persistence should already return this order, but the
        // store re-sorts so the contract does not depend on backend ordering.
        // Non-mutating: sort a spread copy (toSorted is not in this tsc lib target).
        const ordered = [...matched].sort((left, right) => {
          if (right.createdAt !== left.createdAt) {
            return right.createdAt - left.createdAt;
          }
          return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
        });
        const limited = options.limit !== undefined ? ordered.slice(0, options.limit) : ordered;
        // Re-verify every frame on the way out. The store trusts nothing it
        // reads back; only cryptographically valid ledger entries are returned.
        return yield* Effect.all(limited.map(reverifyStoredRow));
      }),
    count: () => persistence.count(),
  };
};

// =============================================================================
// In-memory backend
// =============================================================================

/**
 * An in-memory persistence backend for tests and local single-process runs.
 * Append-only, dedup by id, newest-first coordinate reads. Deterministic and
 * dependency-free, so the store's semantics are proven without a live database.
 */
export const makeInMemorySolClaimLedgerEventPersistence = (): SolClaimLedgerEventPersistence => {
  const byId = new Map<string, StoredSolClaimLedgerEvent>();
  return {
    insert: (row) =>
      Effect.sync(() => {
        if (byId.has(row.id)) {
          return false;
        }
        byId.set(row.id, row);
        return true;
      }),
    queryByCoordinate: (repositoryCoordinate) =>
      Effect.sync(() =>
        [...byId.values()]
          .filter((row) => row.repositoryCoordinate === repositoryCoordinate)
          .sort((left, right) => right.createdAt - left.createdAt),
      ),
    count: () => Effect.sync(() => byId.size),
  };
};

// =============================================================================
// Postgres backend (repo `SyncSql` seam / Cloud SQL)
// =============================================================================

/**
 * The minimal structural SQL seam this backend needs — a tagged-template
 * client returning row arrays, matching the `SyncSql` shape the token ledger
 * store uses. Declared locally so this module carries no extra dependency and
 * stays importable in the Node test host.
 */
export interface SolClaimLedgerSql {
  <Row = Record<string, unknown>>(
    template: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ): Promise<Array<Row>>;
}

/** Acquire a SQL client (Hyperdrive/Cloud SQL in prod, a local URL in tests). */
export type SolClaimLedgerSqlAcquire = () => Promise<{
  readonly sql: SolClaimLedgerSql;
  readonly end: () => Promise<void>;
}>;

/** DDL for the durable claim-ledger event table. Idempotent. */
export const SOL_CLAIM_LEDGER_EVENTS_TABLE_DDL = `
CREATE TABLE IF NOT EXISTS sol_claim_ledger_events (
  id text PRIMARY KEY,
  kind integer NOT NULL,
  pubkey text NOT NULL,
  created_at bigint NOT NULL,
  repository_coordinate text NOT NULL,
  work_item_ref text,
  frame text NOT NULL,
  ingested_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS sol_claim_ledger_events_by_coordinate
  ON sol_claim_ledger_events (repository_coordinate, created_at DESC);
CREATE INDEX IF NOT EXISTS sol_claim_ledger_events_by_work_item
  ON sol_claim_ledger_events (work_item_ref);
` as const;

const toRow = (value: Record<string, unknown>): StoredSolClaimLedgerEvent => ({
  id: String(value.id),
  kind: Number(value.kind),
  pubkey: String(value.pubkey),
  createdAt: Number(value.created_at),
  repositoryCoordinate: String(value.repository_coordinate),
  workItemRef:
    value.work_item_ref === null || value.work_item_ref === undefined
      ? null
      : String(value.work_item_ref),
  frame: String(value.frame),
});

/**
 * The Postgres-backed persistence over the repo's `SyncSql` seam. One client
 * per operation, always ended (the same discipline as the token ledger store).
 * `insert` dedups by primary key with `ON CONFLICT DO NOTHING`, mirroring the
 * in-memory backend's dedup-by-id.
 */
export const makePostgresSolClaimLedgerEventPersistence = (
  acquire: SolClaimLedgerSqlAcquire,
  now: () => string = () => new Date().toISOString(),
): SolClaimLedgerEventPersistence => {
  const withSql = <A>(
    operation: string,
    run: (sql: SolClaimLedgerSql) => Promise<A>,
  ): Effect.Effect<A, SolClaimLedgerStorageError> =>
    Effect.tryPromise({
      try: async () => {
        const client = await acquire();
        try {
          return await run(client.sql);
        } finally {
          try {
            await client.end();
          } catch {
            // best-effort teardown, same discipline as the push route.
          }
        }
      },
      catch: (error) =>
        new SolClaimLedgerStorageError({
          operation,
          messageSafe: safeMessage(error),
        }),
    });

  return {
    insert: (row) =>
      withSql("insert", async (sql) => {
        const inserted = await sql<{ id: string }>`
          INSERT INTO sol_claim_ledger_events (
            id, kind, pubkey, created_at, repository_coordinate,
            work_item_ref, frame, ingested_at
          ) VALUES (
            ${row.id}, ${row.kind}, ${row.pubkey}, ${row.createdAt},
            ${row.repositoryCoordinate}, ${row.workItemRef}, ${row.frame},
            ${now()}
          )
          ON CONFLICT (id) DO NOTHING
          RETURNING id`;
        return inserted.length > 0;
      }),
    queryByCoordinate: (repositoryCoordinate) =>
      withSql("queryByCoordinate", async (sql) => {
        const rows = await sql<Record<string, unknown>>`
          SELECT id, kind, pubkey, created_at, repository_coordinate,
                 work_item_ref, frame
            FROM sol_claim_ledger_events
           WHERE repository_coordinate = ${repositoryCoordinate}
           ORDER BY created_at DESC, id ASC`;
        return rows.map(toRow);
      }),
    count: () =>
      withSql("count", async (sql) => {
        const rows = await sql<{ count: unknown }>`
          SELECT COUNT(*) AS count FROM sol_claim_ledger_events`;
        return Math.max(0, Math.trunc(Number(rows[0]?.count ?? 0)));
      }),
  };
};
