/**
 * Storage-error taxonomy for the Khala Sync server substrate, plus the
 * mapping from raw Bun SQL / Postgres failures into that taxonomy.
 *
 * `messageSafe` is public-safe by construction: it carries SQLSTATE codes,
 * constraint names, and short descriptions — never row values, post-images,
 * or raw SQL text (Postgres `detail` can embed values, so it is dropped).
 */

export type KhalaSyncStorageErrorReason =
  | "connection_failed"
  | "transaction_conflict"
  | "constraint_violation"
  | "unavailable"

export class KhalaSyncStorageError extends Error {
  readonly _tag = "KhalaSyncStorageError"
  override readonly name = "KhalaSyncStorageError"
  constructor(
    readonly reason: KhalaSyncStorageErrorReason,
    readonly messageSafe: string,
    options?: { readonly cause?: unknown },
  ) {
    super(messageSafe, options)
  }
}

// ---------------------------------------------------------------------------
// Read-path protocol errors (KS-2.2)
// ---------------------------------------------------------------------------

/**
 * The requested log range (or bootstrap snapshot stitch point) predates the
 * scope's retained window (`khala_sync_scopes.retained_from_version`), so
 * serving it would silently drop compacted entries. Maps to the wire-level
 * `MustRefetch(reason: "cursor_behind_retained_window")` — the client must
 * clear scope-local state and re-bootstrap (SPEC §3, invariant 6: a cursor
 * behind the retained window MUST receive MustRefetch, never a partial log).
 */
export class KhalaSyncCursorBehindRetainedWindowError extends Error {
  readonly _tag = "KhalaSyncCursorBehindRetainedWindowError"
  override readonly name = "KhalaSyncCursorBehindRetainedWindowError"
  /** Wire `SyncErrorCode` / `MustRefetchReason` this maps to. */
  readonly code = "cursor_behind_retained_window" as const
  constructor(
    readonly scope: string,
    /** The watermark the client asked to resume after (0 = scope start). */
    readonly afterVersion: number,
    readonly retainedFromVersion: number,
  ) {
    super(
      `cursor ${afterVersion} is behind the retained window of ${scope} ` +
        `(retained_from_version ${retainedFromVersion}) — must re-bootstrap`,
    )
  }
}

/**
 * A bootstrap `pageToken` failed to decode, belongs to a different scope, or
 * carries out-of-range fields. Tokens are server-minted and opaque, so this
 * is a client (or cross-scope replay) error, never a storage failure; the
 * client recovers by restarting the bootstrap without a token.
 */
export class KhalaSyncInvalidPageTokenError extends Error {
  readonly _tag = "KhalaSyncInvalidPageTokenError"
  override readonly name = "KhalaSyncInvalidPageTokenError"
  constructor(readonly messageSafe: string) {
    super(messageSafe)
  }
}

// ---------------------------------------------------------------------------
// Bun SQL / Postgres error mapping
// ---------------------------------------------------------------------------

interface BunSqlErrorLike extends Error {
  /** Bun error class code, e.g. "ERR_POSTGRES_SERVER_ERROR". */
  readonly code: string
  /** For server errors, the five-character SQLSTATE (e.g. "23514"). */
  readonly errno?: unknown
  /** Violated constraint name, when the server reported one. */
  readonly constraint?: unknown
}

const isBunSqlErrorLike = (error: unknown): error is BunSqlErrorLike => {
  if (!(error instanceof Error)) return false
  const code = (error as { code?: unknown }).code
  return (
    typeof code === "string" &&
    (code.startsWith("ERR_POSTGRES") || code.startsWith("ERR_SQL"))
  )
}

const SQLSTATE_PATTERN = /^[0-9A-Z]{5}$/

const reasonForSqlState = (sqlState: string): KhalaSyncStorageErrorReason => {
  // serialization_failure / deadlock_detected / lock_not_available
  if (sqlState === "40001" || sqlState === "40P01" || sqlState === "55P03") {
    return "transaction_conflict"
  }
  // integrity_constraint_violation class (23xxx)
  if (sqlState.startsWith("23")) return "constraint_violation"
  // connection_exception class (08xxx)
  if (sqlState.startsWith("08")) return "connection_failed"
  // everything else (insufficient_resources 53xxx, operator_intervention
  // 57xxx, system_error 58xxx, unexpected server errors)
  return "unavailable"
}

/**
 * Map a raw error thrown by Bun SQL into a {@link KhalaSyncStorageError},
 * or return `null` when the error is not a SQL-layer failure. Caller domain
 * errors must pass through transactions unchanged so rollbacks stay
 * attributable to the code that requested them.
 */
export const storageErrorFromUnknown = (
  error: unknown,
): KhalaSyncStorageError | null => {
  if (error instanceof KhalaSyncStorageError) return error
  if (!isBunSqlErrorLike(error)) return null

  const sqlState =
    typeof error.errno === "string" && SQLSTATE_PATTERN.test(error.errno)
      ? error.errno
      : null

  if (sqlState !== null) {
    const constraint =
      typeof error.constraint === "string" && error.constraint.length > 0
        ? ` (constraint ${error.constraint})`
        : ""
    return new KhalaSyncStorageError(
      reasonForSqlState(sqlState),
      `postgres error ${sqlState}${constraint}`,
      { cause: error },
    )
  }

  const reason: KhalaSyncStorageErrorReason =
    error.code.includes("CONNECTION") ||
    error.code.includes("AUTHENTICATION") ||
    error.code.includes("TLS")
      ? "connection_failed"
      : "unavailable"
  return new KhalaSyncStorageError(reason, `sql failure ${error.code}`, {
    cause: error,
  })
}
