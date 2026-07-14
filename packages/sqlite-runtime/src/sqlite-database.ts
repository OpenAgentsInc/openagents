/**
 * Dual-runtime SQLite seam — the ONE typed surface production stores use to
 * touch embedded SQLite, so `bun:sqlite` never leaks past this package
 * (BUN-1, openagents#8779; Option C of
 * docs/fable/2026-07-13-bun-vs-vite-plus-analysis.md).
 *
 * Modeled on T3 Code's `apps/server/src/persistence/Layers/Sqlite.ts` +
 * `NodeSqliteClient.ts` (reference clone `projects/repos/t3code`): the
 * runtime is detected once via `process.versions.bun` and the matching
 * client — `bun:sqlite` under Bun, `node:sqlite` under Node — is loaded
 * lazily at open time. Unlike T3 (which targets `effect/unstable/sql`),
 * this seam is shaped like the synchronous `SqlDriver` our stores already
 * consume (khala-sync-client `store-core.ts`), with Effect wrappers in
 * `effect.ts` for Effect-surfaced callers.
 *
 * Every method is synchronous and throws on failure — exactly the contract
 * `SqlDriver` requires so driver-agnostic store cores can run multi-row
 * semantics inside ONE SQLite transaction.
 */

/** Which embedded-SQLite implementation backs a database handle. */
export type SqliteRuntime = "bun" | "node"

/**
 * Runtime selection, T3-style (`Sqlite.ts` line 27): Bun advertises itself
 * via `process.versions.bun`; anything else is treated as Node.
 */
export const detectSqliteRuntime = (): SqliteRuntime =>
  process.versions.bun !== undefined ? "bun" : "node"

/**
 * Bind-parameter values accepted by both `bun:sqlite` and `node:sqlite`.
 * A superset of khala-sync-client's `SqlValue` (`string | number`), so the
 * seam slots under existing `SqlDriver` consumers without a cast.
 */
export type SqliteValue = string | number | bigint | null | Uint8Array

export interface SqliteStatement<Row = any, Params extends Array<any> = Array<any>> {
  // Driver compatibility requires the same variadic binding surface exposed
  // by both bun:sqlite and node:sqlite.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly run: (...params: Params) => { readonly changes: number | bigint }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly get: (...params: Params) => Row | null
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly all: (...params: Params) => Array<Row>
}

export interface LegacySqliteDatabase {
  exec(sql: string): unknown
  run(sql: string, ...params: ReadonlyArray<never>): unknown
  query<Row = any, Params extends Array<any> = Array<any>>(sql: string): SqliteStatement<Row, Params>
  transaction<A>(fn: () => A): (() => A) & { readonly immediate: () => A }
  close(): void
}

export interface SqliteDatabaseOptions {
  /** Open read-only; the file must already exist. Default: read-write, creating the file if missing. */
  readonly readonly?: boolean
}

/**
 * The seam's database handle. The four query members are signature-identical
 * to khala-sync-client's `SqlDriver`, so a handle can be passed directly to
 * `createKhalaSyncStoreCore` (and future store cores) with no adapter.
 * Implementations must throw on failure; `transaction` must roll back and
 * rethrow when `fn` throws, leaving the database byte-for-byte unchanged.
 */
export interface SqliteDatabase {
  /** Which runtime client backs this handle. */
  readonly runtime: SqliteRuntime
  /** Execute one or more statements without bind parameters (DDL/pragmas). */
  readonly exec: (sql: string) => void
  /** Execute one statement with positional bind parameters. */
  readonly run: (sql: string, params?: ReadonlyArray<SqliteValue>) => void
  /** All result rows of one statement, as objects keyed by column name. */
  readonly all: <Row>(
    sql: string,
    params?: ReadonlyArray<SqliteValue>,
  ) => ReadonlyArray<Row>
  /** Prepared-statement compatibility for retained stores during VP-2. */
  readonly query: <Row = any, Params extends Array<any> = Array<any>>(sql: string) => SqliteStatement<Row, Params>
  /** Run `fn` in ONE transaction (nesting via savepoints); rethrow after rollback on failure. */
  readonly transaction: <A>(fn: () => A) => A
  /** Close the underlying handle; later calls throw. */
  readonly close: () => void
}

export type SqliteRuntimeErrorReason =
  | "open_failure"
  | "close_failure"
  | "unsupported_runtime"

/**
 * Typed error for seam lifecycle failures. Plain class fields only (no
 * TypeScript parameter properties) so this module stays loadable under
 * Node's erasable-syntax type stripping for the Node-path test suite.
 */
export class SqliteRuntimeError extends Error {
  readonly _tag = "SqliteRuntimeError"
  override readonly name = "SqliteRuntimeError"
  readonly reason: SqliteRuntimeErrorReason
  constructor(
    reason: SqliteRuntimeErrorReason,
    message: string,
    options?: { readonly cause?: unknown },
  ) {
    super(message, options)
    this.reason = reason
  }
}
