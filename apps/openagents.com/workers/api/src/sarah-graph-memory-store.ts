import {
  GraphMemoryPersistenceError,
  GraphMemoryStore,
  graphMemoryStoreLayer,
  type GraphMemoryScope,
  type GraphMemoryStateStore,
} from "@openagentsinc/agent-experience-memory";
import type { SyncSql } from "@openagentsinc/khala-sync-server";
import { Effect, type Layer } from "effect";

/**
 * Cloud SQL (Postgres) backing store for hosted Sarah graph memory (#9189
 * composition root).
 *
 * The portable SDK (`@openagentsinc/agent-experience-memory`,
 * `GraphMemoryStore`) owns ALL graph-memory semantics: consent, redaction,
 * generation ordering, compare-and-set retries, receipts, and the opaque
 * persisted envelope. This adapter owns ONLY durable, atomic per-scope state,
 * exactly like the Desktop SQLite adapter (`desktop-graph-memory-store.ts`) —
 * but the hosted runtime executes on Cloud Run's ephemeral filesystem, so
 * durability has to live in Cloud SQL (`khala-sync-pg`), the same Postgres the
 * monolith already reaches through the Khala Sync `SyncSql` seam.
 *
 * Durability contract (matches `GraphMemoryStateStore`):
 * - One atomically-replaced opaque envelope row per owner+project scope.
 * - `compareAndSet` is ONE parameterized statement, so the SDK's CAS loop is
 *   linearizable across concurrent Cloud Run instances: a first insert only
 *   wins when no row exists, and an update only wins when the stored revision
 *   equals the expected revision.
 * - Postgres holds ONLY the owner-hashed scope refs, a revision, and the
 *   already-redacted SDK envelope JSON. It never sees a raw owner id, secret,
 *   token, private path, or email — the SDK redaction boundary and the
 *   owner-hashed `sarahGraphMemoryScope` guarantee that upstream.
 *
 * Failures map to `GraphMemoryPersistenceError`, which the SDK folds into its
 * own store-error taxonomy; recall stays fail-soft (a store failure yields an
 * empty slice and never breaks a Sarah turn — see `recallSarahGraphMemory`).
 */

/** The single Cloud SQL table that backs hosted Sarah graph memory. */
export const SARAH_GRAPH_MEMORY_TABLE = "sarah_graph_memory_scopes" as const;

const persistenceError = (
  operation: string,
  reason: GraphMemoryPersistenceError["reason"],
  detailSafe: string,
): GraphMemoryPersistenceError =>
  new GraphMemoryPersistenceError({ operation, reason, detailSafe });

const mapError = (operation: string) => (error: unknown): GraphMemoryPersistenceError =>
  error instanceof GraphMemoryPersistenceError
    ? error
    : persistenceError(operation, "unavailable", "Hosted Sarah graph memory storage is unavailable.");

/**
 * Derive the non-negative envelope revision the CAS row is keyed on. The SDK
 * always stamps a `revision` field on the persisted envelope; a value that is
 * not a safe non-negative integer is treated as invalid state rather than
 * silently coerced.
 */
const revisionOf = (value: unknown, operation: string): number => {
  const revision =
    typeof value === "object" && value !== null && "revision" in value
      ? (value as { revision: unknown }).revision
      : undefined;
  if (typeof revision !== "number" || !Number.isSafeInteger(revision) || revision < 0) {
    throw persistenceError(operation, "invalid_state", "The graph memory envelope revision is invalid.");
  }
  return revision;
};

/**
 * Build a Cloud SQL-backed `GraphMemoryStateStore` over one Khala Sync `SyncSql`
 * handle. The store is enabled whenever a handle is present; the table it reads
 * and writes is created by migration `0094_sarah_graph_memory_scopes.sql`.
 */
export const makeSarahGraphMemoryStateStore = (sql: SyncSql): GraphMemoryStateStore => {
  let reads = 0;
  let writes = 0;

  const load: GraphMemoryStateStore["load"] = (scope: GraphMemoryScope) =>
    Effect.tryPromise({
      try: async () => {
        reads += 1;
        const rows: Array<{ envelope_json: unknown; revision: number | string }> = await sql`
          SELECT envelope_json, revision
            FROM sarah_graph_memory_scopes
           WHERE owner_scope = ${scope.owner}
             AND project_scope = ${scope.project}
           LIMIT 1
        `;
        const row = rows[0];
        if (row === undefined) return null;
        const envelope = row.envelope_json;
        // Defensive: the sealed revision column and the envelope's own revision
        // must agree, or the row is corrupt state, not silently-usable state.
        if (revisionOf(envelope, "load") !== Number(row.revision)) {
          throw persistenceError(
            "load",
            "invalid_state",
            "The stored graph memory revision does not match its envelope.",
          );
        }
        return envelope;
      },
      catch: mapError("load"),
    });

  const compareAndSet: GraphMemoryStateStore["compareAndSet"] = (
    scope: GraphMemoryScope,
    expectedRevision: number | null,
    next: unknown,
  ) =>
    Effect.tryPromise({
      try: async () => {
        if (
          expectedRevision !== null &&
          (!Number.isSafeInteger(expectedRevision) || expectedRevision < 0)
        ) {
          throw persistenceError(
            "compareAndSet",
            "invalid_state",
            "The expected graph memory revision is invalid.",
          );
        }
        const nextRevision = revisionOf(next, "compareAndSet");
        const payload = JSON.stringify(next);
        writes += 1;
        if (expectedRevision === null) {
          // First write for this scope: win only when no row exists yet.
          const inserted: Array<{ owner_scope: string }> = await sql`
            INSERT INTO sarah_graph_memory_scopes
                (owner_scope, project_scope, revision, envelope_json, updated_at)
            VALUES
                (${scope.owner}, ${scope.project}, ${nextRevision}, ${payload}::jsonb, now())
            ON CONFLICT (owner_scope, project_scope) DO NOTHING
            RETURNING owner_scope
          `;
          return inserted.length > 0;
        }
        // Subsequent write: win only when the stored revision is exactly the
        // one the SDK read. This is the linearizable compare-and-set point.
        const updated: Array<{ owner_scope: string }> = await sql`
          UPDATE sarah_graph_memory_scopes
             SET revision = ${nextRevision},
                 envelope_json = ${payload}::jsonb,
                 updated_at = now()
           WHERE owner_scope = ${scope.owner}
             AND project_scope = ${scope.project}
             AND revision = ${expectedRevision}
          RETURNING owner_scope
        `;
        return updated.length > 0;
      },
      catch: mapError("compareAndSet"),
    });

  return {
    enabled: true,
    load,
    compareAndSet,
    reads: Effect.sync(() => reads),
    writes: Effect.sync(() => writes),
  } satisfies GraphMemoryStateStore;
};

/**
 * Compose the portable graph-memory lifecycle service over the Cloud SQL state
 * store. This is the hosted composition-root layer the recall path consumes
 * when `SARAH_GRAPH_MEMORY_RECALL_ENABLED` is on.
 */
export const sarahGraphMemoryStoreLayer = (sql: SyncSql): Layer.Layer<GraphMemoryStore> =>
  graphMemoryStoreLayer(makeSarahGraphMemoryStateStore(sql));
